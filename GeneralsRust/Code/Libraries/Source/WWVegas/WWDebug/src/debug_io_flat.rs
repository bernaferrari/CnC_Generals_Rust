use crate::debug_debug::Debug;
use crate::debug_io::{DebugIOInterface, StringType};
use chrono::{Datelike, NaiveDateTime, Timelike};
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BUFFER_SIZE: usize = 0x10000;

struct OutputStream {
    filename: PathBuf,
    limited_file_size: bool,
    buffer_size: usize,
    buffer: Vec<u8>,
    buffer_used: usize,
    next_char: usize,
    file: Option<File>,
}

impl OutputStream {
    fn create(filename: &Path, max_size: usize) -> Self {
        let limited_file_size = max_size > 0;
        let buffer_size = if limited_file_size {
            max_size
        } else {
            DEFAULT_BUFFER_SIZE
        };
        let file = if limited_file_size {
            None
        } else {
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(filename)
                .ok()
        };

        Self {
            filename: filename.to_path_buf(),
            limited_file_size,
            buffer_size,
            buffer: vec![0; buffer_size],
            buffer_used: 0,
            next_char: 0,
            file,
        }
    }

    fn filename(&self) -> &Path {
        &self.filename
    }

    fn write(&mut self, src: Option<&str>) {
        match src {
            None => {
                if !self.limited_file_size {
                    self.flush();
                }
            }
            Some(src) => {
                let mut bytes = src.as_bytes();
                while bytes.len() > self.buffer_size {
                    let (head, tail) = bytes.split_at(self.buffer_size);
                    self.internal_write(head);
                    bytes = tail;
                }
                self.internal_write(bytes);
            }
        }
    }

    fn internal_write(&mut self, src: &[u8]) {
        if !self.limited_file_size {
            if self.buffer_used + src.len() > self.buffer_size {
                self.flush();
            }
            let end = self.buffer_used + src.len();
            self.buffer[self.buffer_used..end].copy_from_slice(src);
            self.buffer_used = end;
        } else {
            self.buffer_used = (self.buffer_used + src.len()).min(self.buffer_size);
            let mut remaining = src;
            while !remaining.is_empty() {
                let space = self.buffer_size - self.next_char;
                let to_write = space.min(remaining.len());
                let end = self.next_char + to_write;
                self.buffer[self.next_char..end].copy_from_slice(&remaining[..to_write]);
                self.next_char = if end >= self.buffer_size { 0 } else { end };
                remaining = &remaining[to_write..];
            }
        }
    }

    fn flush(&mut self) {
        if !self.limited_file_size {
            if let Some(file) = self.file.as_mut() {
                let _ = file.write_all(&self.buffer[..self.buffer_used]);
                let _ = file.flush();
            }
            self.buffer_used = 0;
        } else {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&self.filename)
            {
                if self.buffer_used < self.buffer_size {
                    let _ = file.write_all(&self.buffer[..self.buffer_used]);
                } else {
                    let tail = &self.buffer[self.next_char..];
                    let head = &self.buffer[..self.next_char];
                    let _ = file.write_all(tail);
                    let _ = file.write_all(head);
                }
                let _ = file.flush();
            }
        }
    }

    fn delete(self, copy_dir: Option<&Path>) {
        let mut stream = self;
        stream.flush();
        stream.file.take();

        if let Some(copy_dir) = copy_dir {
            let filename = stream
                .filename
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "debug.log".to_string());
            let mut target = copy_dir.join(&filename);
            let mut run = 0;
            while target.exists() {
                run += 1;
                let stem = Path::new(&filename)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| filename.clone());
                let ext = Path::new(&filename)
                    .extension()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let mut candidate = format!("{stem}({run})");
                if !ext.is_empty() {
                    candidate.push('.');
                    candidate.push_str(&ext);
                }
                target = copy_dir.join(candidate);
            }
            let _ = std::fs::copy(&stream.filename, target);
        }
    }
}

struct SplitEntry {
    string_types: u32,
    filter: String,
    name: String,
    stream_index: usize,
}

pub struct DebugIOFlat {
    base_filename: String,
    copy_dir: Option<PathBuf>,
    streams: Vec<OutputStream>,
    stream_lookup: HashMap<PathBuf, usize>,
    splits: Vec<SplitEntry>,
}

impl DebugIOFlat {
    pub fn create() -> Box<dyn DebugIOInterface> {
        Box::new(Self::new())
    }

    fn new() -> Self {
        Self {
            base_filename: "*eMN".to_string(),
            copy_dir: None,
            streams: Vec::new(),
            stream_lookup: HashMap::new(),
            splits: Vec::new(),
        }
    }

    fn ensure_default_stream(&mut self, max_size: usize) {
        if !self.streams.is_empty() {
            return;
        }
        let filename = expand_magic(&self.base_filename, None);
        let stream = OutputStream::create(Path::new(&filename), max_size);
        self.stream_lookup.insert(PathBuf::from(&filename), 0);
        self.streams.push(stream);
    }

    fn get_or_create_stream(&mut self, filename: &str, max_size: usize) -> usize {
        let path = PathBuf::from(filename);
        if let Some(index) = self.stream_lookup.get(&path).copied() {
            return index;
        }
        let index = self.streams.len();
        let stream = OutputStream::create(&path, max_size);
        self.streams.push(stream);
        self.stream_lookup.insert(path, index);
        index
    }
}

impl DebugIOInterface for DebugIOFlat {
    fn read(&mut self, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&mut self, kind: StringType, src: Option<&str>, message: Option<&str>) {
        let Some(message) = message else {
            return;
        };
        self.ensure_default_stream(0);

        let mut handled = false;
        for split in &self.splits {
            if (split.string_types & (1 << kind as u32)) == 0 {
                continue;
            }
            if matches!(
                kind,
                StringType::Assert | StringType::Check | StringType::Log
            ) {
                if let Some(src) = src {
                    if !crate::debug_debug::simple_match(src, &split.filter) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if let Some(stream) = self.streams.get_mut(split.stream_index) {
                stream.write(Some(message));
            }
            handled = true;
            break;
        }

        if !handled {
            if let Some(stream) = self.streams.first_mut() {
                stream.write(Some(message));
            }
        }
    }

    fn emergency_flush(&mut self) {
        for stream in &mut self.streams {
            stream.flush();
        }
    }

    fn execute(&mut self, dbg: &mut Debug, cmd: &str, _structured: bool, argv: &[&str]) {
        match cmd {
            "help" => {
                if argv.is_empty() {
                    dbg.write_plain(
                        "flat I/O help:\nThe following I/O commands are defined:\n  add, copy, splitadd, splitview, splitremove\n",
                    );
                    return;
                }
                match argv[0] {
                    "add" => dbg.write_plain(
                        "add [ <filename> [ <size in kb> ] ]\n\nCreate flat file I/O (optionally specifying file name and file size).\n",
                    ),
                    "copy" => dbg.write_plain(
                        "copy <directory>\n\nCopies generated log file(s) into the given directory on exit.\n",
                    ),
                    "splitadd" => dbg.write_plain(
                        "splitadd <types> <filter> <name> [ <size in kb> ]\n\nSplits off part of the log data.\n",
                    ),
                    "splitview" => dbg.write_plain("splitview\n\nShows all existing splits.\n"),
                    "splitremove" => dbg.write_plain(
                        "splitremove <namepattern>\n\nRemoves all active splits matching the pattern.\n",
                    ),
                    _ => dbg.write_plain("Unknown flat I/O command\n"),
                }
            }
            "add" => {
                self.base_filename = argv.get(0).copied().unwrap_or("*eMN").to_string();
                let max_size = argv
                    .get(1)
                    .and_then(|v| v.parse::<usize>().ok())
                    .map(|kb| kb * 1024)
                    .unwrap_or(0);
                self.streams.clear();
                self.stream_lookup.clear();
                self.ensure_default_stream(max_size);
            }
            "copy" => {
                if let Some(dir) = argv.get(0) {
                    self.copy_dir = Some(PathBuf::from(dir));
                }
            }
            "splitadd" => {
                if argv.len() < 3 {
                    return;
                }
                let types = argv[0];
                let filter = argv[1];
                let name = argv[2];
                let max_size = argv
                    .get(3)
                    .and_then(|v| v.parse::<usize>().ok())
                    .map(|kb| kb * 1024)
                    .unwrap_or(0);

                let mut string_types = 0u32;
                for ch in types.chars() {
                    match ch {
                        'a' => string_types |= 1 << StringType::Assert as u32,
                        'c' => string_types |= 1 << StringType::Check as u32,
                        'l' => string_types |= 1 << StringType::Log as u32,
                        'h' => string_types |= 1 << StringType::Crash as u32,
                        'x' => string_types |= 1 << StringType::Exception as u32,
                        'r' => string_types |= 1 << StringType::CmdReply as u32,
                        'o' => string_types |= 1 << StringType::Other as u32,
                        _ => {}
                    }
                }
                if string_types == 0 {
                    string_types = u32::MAX;
                }

                let filename = expand_magic(&self.base_filename, Some(name));
                let stream_index = self.get_or_create_stream(&filename, max_size);

                self.splits.push(SplitEntry {
                    string_types,
                    filter: filter.to_string(),
                    name: name.to_string(),
                    stream_index,
                });
            }
            "splitview" => {
                for split in &self.splits {
                    let mut types = String::new();
                    for (char_key, kind) in [
                        ('a', StringType::Assert),
                        ('c', StringType::Check),
                        ('l', StringType::Log),
                        ('h', StringType::Crash),
                        ('x', StringType::Exception),
                        ('r', StringType::CmdReply),
                        ('o', StringType::Other),
                    ] {
                        if (split.string_types & (1 << kind as u32)) != 0 {
                            types.push(char_key);
                        }
                    }
                    dbg.write_plain(&format!("{} {} {}\n", types, split.filter, split.name));
                }
            }
            "splitremove" => {
                let pattern = argv.get(0).copied().unwrap_or("*");
                self.splits
                    .retain(|split| !crate::debug_debug::simple_match(&split.name, pattern));
            }
            _ => {}
        }
    }

    fn delete(self: Box<Self>) {
        let mut this = *self;
        for stream in this.streams.drain(..) {
            stream.delete(this.copy_dir.as_ref().map(|p| p.as_path()));
        }
    }
}

fn expand_magic(src: &str, split_name: Option<&str>) -> String {
    if !src.starts_with('*') {
        if let Some(split) = split_name {
            let path = Path::new(src);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(src);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.is_empty() {
                return format!("{stem}-{split}");
            }
            return format!("{stem}-{split}.{ext}");
        }
        return src.to_string();
    }

    let mut out = String::new();
    let mut chars = src.chars().skip(1);
    while let Some(ch) = chars.next() {
        match ch {
            'e' | 'E' => {
                if ch.is_uppercase() {
                    out.push('-');
                }
                out.push_str(&exe_name());
            }
            'm' | 'M' => {
                if ch.is_uppercase() {
                    out.push('-');
                }
                out.push_str(&machine_name());
            }
            'u' | 'U' => {
                if ch.is_uppercase() {
                    out.push('-');
                }
                out.push_str(&user_name());
            }
            't' | 'T' => {
                if ch.is_uppercase() {
                    out.push('-');
                }
                out.push_str(&timestamp());
            }
            'n' | 'N' => {
                if let Some(split) = split_name {
                    if ch.is_uppercase() {
                        out.push('-');
                    }
                    out.push_str(split);
                }
            }
            '-' => out.push('-'),
            _ => {
                out.push(ch);
            }
        }
        if out.len() > 250 {
            break;
        }
    }
    out.push_str(".log");
    out
}

fn exe_name() -> String {
    env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "app".to_string())
}

fn machine_name() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "machine".to_string())
}

fn user_name() -> String {
    env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "user".to_string())
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let tm = NaiveDateTime::from_timestamp_opt(now as i64, 0)
        .unwrap_or_else(|| NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
    format!(
        "{:04}{:02}{:02}-{:02}{:02}-{:02}",
        tm.year(),
        tm.month(),
        tm.day(),
        tm.hour(),
        tm.minute(),
        tm.second()
    )
}
