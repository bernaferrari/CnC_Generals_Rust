//! File factory utilities mirroring WWLib's ffactory.cpp

use crate::bufffile::BufferedFile;
use crate::rawfile::{FileRights, RawFile};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

pub trait FileFactoryClass: Send + Sync {
    fn get_file(&self, filename: &str) -> FactoryFile;
    fn return_file(&self, _file: FactoryFile) {}
}

pub enum FactoryFile {
    Raw(RawFile),
    Buffered(BufferedFile),
}

impl FactoryFile {
    pub fn set_name(&mut self, filename: &str) -> &str {
        match self {
            FactoryFile::Raw(file) => file.set_name(filename),
            FactoryFile::Buffered(file) => file.set_name(filename),
        }
    }

    pub fn file_name(&self) -> Option<&str> {
        match self {
            FactoryFile::Raw(file) => file.filename(),
            FactoryFile::Buffered(file) => file.file_name(),
        }
    }

    pub fn is_available(&self, forced: bool) -> bool {
        match self {
            FactoryFile::Raw(file) => file.is_available(forced),
            FactoryFile::Buffered(file) => file.is_available(forced),
        }
    }

    pub fn open(&mut self, rights: FileRights) -> bool {
        match self {
            FactoryFile::Raw(file) => file.open(rights).is_ok(),
            FactoryFile::Buffered(file) => file.open(rights).is_ok(),
        }
    }

    pub fn open_read(&mut self) -> bool {
        match self {
            FactoryFile::Raw(file) => file.open(FileRights::READ).is_ok(),
            FactoryFile::Buffered(file) => file.open(FileRights::READ).is_ok(),
        }
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        match self {
            FactoryFile::Raw(file) => file.read(buffer).unwrap_or(0) as usize,
            FactoryFile::Buffered(file) => file.read(buffer),
        }
    }

    pub fn seek(&mut self, pos: i64, origin: crate::rawfile::SeekOrigin) -> i64 {
        match self {
            FactoryFile::Raw(file) => file.seek(pos, origin).unwrap_or(0) as i64,
            FactoryFile::Buffered(file) => file.seek(pos, origin),
        }
    }

    pub fn size(&mut self) -> Option<u64> {
        match self {
            FactoryFile::Raw(file) => file.size().ok(),
            FactoryFile::Buffered(file) => file.size().ok(),
        }
    }

    pub fn bias(&mut self, start: u64, length: Option<u64>) {
        match self {
            FactoryFile::Raw(file) => file.bias(start, length),
            FactoryFile::Buffered(file) => file.bias(start, length),
        }
    }

    pub fn close(&mut self) {
        match self {
            FactoryFile::Raw(file) => {
                let _ = file.close();
            }
            FactoryFile::Buffered(file) => file.close(),
        }
    }
}

pub struct FileAutoPtr {
    file: FactoryFile,
    factory: Arc<dyn FileFactoryClass>,
}

impl FileAutoPtr {
    pub fn new(factory: Arc<dyn FileFactoryClass>, filename: &str) -> Self {
        let mut file = factory.get_file(filename);
        file.set_name(filename);
        Self { file, factory }
    }

    pub fn get(&self) -> &FactoryFile {
        &self.file
    }

    pub fn get_mut(&mut self) -> &mut FactoryFile {
        &mut self.file
    }
}

impl Drop for FileAutoPtr {
    fn drop(&mut self) {
        let file = std::mem::replace(&mut self.file, FactoryFile::Raw(RawFile::new()));
        self.factory.return_file(file);
    }
}

pub struct RawFileFactory;

impl FileFactoryClass for RawFileFactory {
    fn get_file(&self, filename: &str) -> FactoryFile {
        FactoryFile::Raw(RawFile::with_name(filename))
    }

    fn return_file(&self, _file: FactoryFile) {}
}

pub struct SimpleFileFactory {
    sub_directory: Mutex<String>,
    strip_path: Mutex<bool>,
}

impl Default for SimpleFileFactory {
    fn default() -> Self {
        Self {
            sub_directory: Mutex::new(String::new()),
            strip_path: Mutex::new(false),
        }
    }
}

impl SimpleFileFactory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_sub_directory(&self) -> String {
        self.sub_directory
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub fn set_sub_directory(&self, sub_directory: &str) {
        if let Ok(mut guard) = self.sub_directory.lock() {
            *guard = sub_directory.to_string();
        }
    }

    pub fn prepend_sub_directory(&self, sub_directory: &str) {
        let mut sub = normalize_sub_directory(sub_directory, true);
        if sub.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.sub_directory.lock() {
            sub.push_str(guard.as_str());
            *guard = sub;
        }
    }

    pub fn append_sub_directory(&self, sub_directory: &str) {
        let mut sub = normalize_sub_directory(sub_directory, false);
        if sub.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.sub_directory.lock() {
            if !guard.is_empty() && !guard.ends_with(';') {
                guard.push(';');
            }
            guard.push_str(&sub);
        }
    }

    pub fn get_strip_path(&self) -> bool {
        self.strip_path.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn set_strip_path(&self, enabled: bool) {
        if let Ok(mut guard) = self.strip_path.lock() {
            *guard = enabled;
        }
    }

    fn should_strip_path(&self) -> bool {
        self.strip_path.lock().map(|v| *v).unwrap_or(false)
    }
}

impl FileFactoryClass for SimpleFileFactory {
    fn get_file(&self, filename: &str) -> FactoryFile {
        let stripped_name = if self.should_strip_path() {
            strip_path(filename)
        } else {
            filename.to_string()
        };

        let mut file = BufferedFile::new();
        let mut new_name = stripped_name.clone();

        if !is_full_path(&new_name) {
            if let Ok(guard) = self.sub_directory.lock() {
                if !guard.is_empty() {
                    if guard.contains(';') {
                        let mut found = false;
                        for path in guard.split(';').filter(|p| !p.is_empty()) {
                            new_name = format!("{}{}", path, stripped_name);
                            file.set_name(&new_name);
                            if file.open(FileRights::READ).is_ok() {
                                file.close();
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            if let Some(last) = guard.split(';').filter(|p| !p.is_empty()).last() {
                                new_name = format!("{}{}", last, stripped_name);
                            }
                        }
                    } else {
                        new_name = format!("{}{}", guard.as_str(), stripped_name);
                    }
                }
            }
        }

        file.set_name(&new_name);
        FactoryFile::Buffered(file)
    }

    fn return_file(&self, _file: FactoryFile) {}
}

fn normalize_sub_directory(sub_directory: &str, add_semicolon: bool) -> String {
    if sub_directory.is_empty() {
        return String::new();
    }
    let mut sub = sub_directory.replace('/', "\\");
    if !sub.ends_with('\\') {
        sub.push('\\');
    }
    if add_semicolon {
        sub.push(';');
    }
    sub
}

fn strip_path(path: &str) -> String {
    path.rsplit(|c| c == '\\' || c == '/')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn is_full_path(path: &str) -> bool {
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return true;
    }
    if path.starts_with("\\\\") {
        return true;
    }
    Path::new(path).is_absolute()
}

static DEFAULT_FILE_FACTORY: OnceLock<Arc<SimpleFileFactory>> = OnceLock::new();
static DEFAULT_WRITING_FACTORY: OnceLock<Arc<RawFileFactory>> = OnceLock::new();
static DEFAULT_SIMPLE_FACTORY: OnceLock<Arc<SimpleFileFactory>> = OnceLock::new();

pub fn the_file_factory() -> Arc<dyn FileFactoryClass> {
    DEFAULT_FILE_FACTORY
        .get_or_init(|| Arc::new(SimpleFileFactory::new()))
        .clone()
}

pub fn the_writing_file_factory() -> Arc<dyn FileFactoryClass> {
    DEFAULT_WRITING_FACTORY
        .get_or_init(|| Arc::new(RawFileFactory))
        .clone()
}

pub fn the_simple_file_factory() -> Arc<SimpleFileFactory> {
    DEFAULT_SIMPLE_FACTORY
        .get_or_init(|| Arc::new(SimpleFileFactory::new()))
        .clone()
}
