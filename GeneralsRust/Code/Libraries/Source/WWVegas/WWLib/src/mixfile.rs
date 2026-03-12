use crate::crc::Crc32;
use crate::ffactory::{the_file_factory, FactoryFile, FileFactoryClass};
use crate::rawfile::{FileRights, SeekOrigin};
use std::cmp::Ordering;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct MixHeader {
    signature: [u8; 4],
    header_offset: u32,
    names_offset: u32,
}

#[derive(Clone)]
struct FileInfo {
    crc: u32,
    offset: u32,
    size: u32,
}

#[derive(Clone)]
struct AddInfo {
    full_path: String,
    filename: String,
}

pub struct MixFileFactory {
    factory: Arc<dyn FileFactoryClass>,
    mix_filename: String,
    file_info: Vec<FileInfo>,
    base_offset: u32,
    file_count: u32,
    names_offset: u32,
    is_valid: bool,
    filename_list: Vec<String>,
    pending_add: Vec<AddInfo>,
    is_modified: bool,
}

impl MixFileFactory {
    pub fn new(mix_filename: &str) -> Self {
        Self::new_with_factory(mix_filename, the_file_factory())
    }

    pub fn new_with_factory(mix_filename: &str, factory: Arc<dyn FileFactoryClass>) -> Self {
        let mut instance = Self {
            factory,
            mix_filename: mix_filename.to_string(),
            file_info: Vec::new(),
            base_offset: 0,
            file_count: 0,
            names_offset: 0,
            is_valid: false,
            filename_list: Vec::new(),
            pending_add: Vec::new(),
            is_modified: false,
        };
        instance.load();
        instance
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn get_file(&self, filename: &str) -> Option<FactoryFile> {
        if self.file_info.is_empty() {
            return None;
        }

        let crc = crc_string_i(filename);
        let info = match binary_search_crc(&self.file_info, crc) {
            Some(info) => info,
            None => return None,
        };

        let mut file = self.factory.get_file(&self.mix_filename);
        file.open(FileRights::READ);
        file.bias(
            self.base_offset as u64 + info.offset as u64,
            Some(info.size as u64),
        );
        Some(file)
    }

    pub fn return_file(&self, mut file: FactoryFile) {
        file.close();
        self.factory.return_file(file);
    }

    pub fn build_filename_list(&mut self) -> bool {
        if !self.is_valid {
            return false;
        }

        let mut file = self.factory.get_file(&self.mix_filename);
        if !file.open(FileRights::READ) {
            return false;
        }

        if file.seek(self.names_offset as i64, SeekOrigin::Start) < 0 {
            file.close();
            return false;
        }

        let mut count_buf = [0u8; 4];
        if file.read(&mut count_buf) != 4 {
            file.close();
            return false;
        }
        let file_count = u32::from_le_bytes(count_buf) as usize;

        let mut list = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            let mut len_buf = [0u8; 1];
            if file.read(&mut len_buf) != 1 {
                break;
            }
            let name_len = len_buf[0] as usize;
            if name_len == 0 {
                list.push(String::new());
                continue;
            }
            let mut name_buf = vec![0u8; name_len];
            if file.read(&mut name_buf) != name_len {
                break;
            }
            if let Some(pos) = name_buf.iter().position(|&c| c == 0) {
                name_buf.truncate(pos);
            }
            let name = String::from_utf8_lossy(&name_buf).to_string();
            list.push(name);
        }

        self.filename_list = list;
        file.close();
        self.factory.return_file(file);
        true
    }

    pub fn build_ordered_filename_list(&mut self) -> bool {
        if !self.build_filename_list() {
            return false;
        }

        let mut combined: Vec<(String, u32)> = self
            .filename_list
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                self.file_info
                    .get(i)
                    .map(|info| (name.clone(), info.offset))
            })
            .collect();

        combined.sort_by(|a, b| a.1.cmp(&b.1));
        self.filename_list = combined.into_iter().map(|pair| pair.0).collect();
        true
    }

    pub fn get_filename_list(&self) -> &Vec<String> {
        &self.filename_list
    }

    pub fn add_file(&mut self, full_path: &str, filename: &str) {
        self.pending_add.push(AddInfo {
            full_path: full_path.to_string(),
            filename: filename.to_string(),
        });
        self.is_modified = true;
    }

    pub fn delete_file(&mut self, filename: &str) {
        if let Some(index) = self
            .filename_list
            .iter()
            .position(|name| name.eq_ignore_ascii_case(filename))
        {
            self.filename_list.remove(index);
            self.is_modified = true;
        }
    }

    pub fn flush_changes(&mut self) {
        if !self.is_modified {
            return;
        }

        let mix_path = Path::new(&self.mix_filename);
        let parent = mix_path.parent().unwrap_or_else(|| Path::new(""));
        let temp_path = self.get_temp_filename(parent);
        if temp_path.is_none() {
            return;
        }
        let temp_path = temp_path.unwrap();

        let mut creator = MixFileCreator::new(&temp_path);

        for filename in &self.filename_list {
            if let Some(mut file) = self.get_file(filename) {
                let mut data = Vec::new();
                let _ = read_to_end(&mut file, &mut data);
                creator.add_file_data(filename, &data);
                self.return_file(file);

                if let Some(pos) = self
                    .pending_add
                    .iter()
                    .position(|item| item.filename.eq_ignore_ascii_case(filename))
                {
                    self.pending_add.remove(pos);
                }
            }
        }

        for pending in &self.pending_add {
            creator.add_file_path(&pending.full_path, &pending.filename);
        }

        creator.finish();

        let _ = fs::remove_file(&self.mix_filename);
        let _ = fs::rename(&temp_path, &self.mix_filename);

        self.pending_add.clear();
        self.is_modified = false;
        self.load();
    }

    fn get_temp_filename(&self, path: &Path) -> Option<PathBuf> {
        for index in 0..20 {
            let candidate = path.join(format!("_tmpmix{:02}.dat", index + 1));
            if !candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    fn load(&mut self) {
        self.is_valid = false;
        self.file_info.clear();
        self.filename_list.clear();

        let mut file = self.factory.get_file(&self.mix_filename);
        if !file.open(FileRights::READ) {
            return;
        }

        let mut header_buf = [0u8; 12];
        if file.read(&mut header_buf) != header_buf.len() {
            file.close();
            return;
        }
        let header = MixHeader {
            signature: header_buf[0..4].try_into().unwrap(),
            header_offset: u32::from_le_bytes(header_buf[4..8].try_into().unwrap()),
            names_offset: u32::from_le_bytes(header_buf[8..12].try_into().unwrap()),
        };

        if &header.signature != b"MIX1" {
            let _ = file.close();
            return;
        }

        if file.seek(header.header_offset as i64, SeekOrigin::Start) < 0 {
            file.close();
            return;
        }

        let mut count_buf = [0u8; 4];
        if file.read(&mut count_buf) != 4 {
            file.close();
            return;
        }
        self.file_count = u32::from_le_bytes(count_buf);

        let count = self.file_count as usize;
        let mut info = Vec::with_capacity(count);
        for _ in 0..count {
            let mut entry = [0u8; 12];
            if file.read(&mut entry) != entry.len() {
                break;
            }
            let crc = u32::from_le_bytes(entry[0..4].try_into().unwrap());
            let offset = u32::from_le_bytes(entry[4..8].try_into().unwrap());
            let size = u32::from_le_bytes(entry[8..12].try_into().unwrap());
            info.push(FileInfo { crc, offset, size });
        }

        self.file_info = info;
        self.base_offset = 0;
        self.names_offset = header.names_offset;
        self.is_valid = true;

        file.close();
        self.factory.return_file(file);
    }
}

fn crc_string_i(name: &str) -> u32 {
    Crc32::string(&name.to_ascii_lowercase())
}

fn binary_search_crc<'a>(list: &'a [FileInfo], crc: u32) -> Option<&'a FileInfo> {
    let mut stride = list.len();
    let mut pointer = 0usize;
    while stride > 0 {
        let pivot = stride / 2;
        let index = pointer + pivot;
        let value = &list[index];
        match crc.cmp(&value.crc) {
            Ordering::Less => stride = pivot,
            Ordering::Equal => return Some(value),
            Ordering::Greater => {
                pointer = index + 1;
                stride -= pivot + 1;
            }
        }
    }
    None
}

pub struct MixFileCreator {
    file_info: Vec<CreatorInfo>,
    mix_file: Option<fs::File>,
}

fn read_to_end(file: &mut FactoryFile, buffer: &mut Vec<u8>) -> usize {
    let mut temp = [0u8; 4096];
    let mut total = 0usize;
    loop {
        let read = file.read(&mut temp);
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        total += read;
    }
    total
}
#[derive(Clone)]
struct CreatorInfo {
    crc: u32,
    offset: u32,
    size: u32,
    filename: String,
}

impl MixFileCreator {
    pub fn new<P: AsRef<Path>>(filename: P) -> Self {
        let mut mix_file = fs::File::create(filename).ok();
        if let Some(file) = mix_file.as_mut() {
            let _ = file.write_all(b"MIX1");
            let _ = file.write_all(&0u32.to_le_bytes());
            let _ = file.write_all(&0u32.to_le_bytes());
            let _ = file.write_all(&0u32.to_le_bytes());
        }
        Self {
            file_info: Vec::new(),
            mix_file,
        }
    }

    pub fn add_file_path(&mut self, source_filename: &str, saved_filename: &str) {
        if let Some(file) = fs::File::open(source_filename).ok() {
            let mut reader = file;
            let mut data = Vec::new();
            let _ = reader.read_to_end(&mut data);
            self.add_file_data(saved_filename, &data);
        }
    }

    pub fn add_file_data(&mut self, saved_filename: &str, data: &[u8]) {
        if let Some(file) = self.mix_file.as_mut() {
            let offset = file.seek(SeekFrom::Current(0)).unwrap_or(0) as u32;
            let size = data.len() as u32;
            let crc = crc_string_i(saved_filename);
            self.file_info.push(CreatorInfo {
                crc,
                offset,
                size,
                filename: saved_filename.to_string(),
            });
            let _ = file.write_all(data);
            let pad = (8 - (offset as usize + data.len()) % 8) % 8;
            if pad > 0 {
                let zeros = [0u8; 8];
                let _ = file.write_all(&zeros[..pad]);
            }
        }
    }

    pub fn finish(&mut self) {
        let Some(file) = self.mix_file.as_mut() else {
            return;
        };

        let header_offset = file.seek(SeekFrom::Current(0)).unwrap_or(0) as u32;
        let num_files = self.file_info.len() as u32;
        let _ = file.write_all(&num_files.to_le_bytes());

        if self.file_info.len() > 1 {
            self.file_info.sort_by(|a, b| a.crc.cmp(&b.crc));
        }

        for info in &self.file_info {
            let _ = file.write_all(&info.crc.to_le_bytes());
            let _ = file.write_all(&info.offset.to_le_bytes());
            let _ = file.write_all(&info.size.to_le_bytes());
        }

        let names_offset = file.seek(SeekFrom::Current(0)).unwrap_or(0) as u32;
        let _ = file.write_all(&num_files.to_le_bytes());

        for info in &self.file_info {
            let mut bytes = info.filename.clone().into_bytes();
            bytes.push(0);
            let size = bytes.len().min(255);
            let _ = file.write_all(&[size as u8]);
            let _ = file.write_all(&bytes[..size]);
        }

        let _ = file.seek(SeekFrom::Start(4));
        let _ = file.write_all(&header_offset.to_le_bytes());
        let _ = file.write_all(&names_offset.to_le_bytes());
        let _ = file.sync_all();
    }
}
