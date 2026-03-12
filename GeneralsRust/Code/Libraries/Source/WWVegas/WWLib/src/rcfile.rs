#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HMODULE;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::{
    FindResourceA, LoadResource, LockResource, SizeofResource,
};

pub struct ResourceFile {
    resource_name: String,
    #[cfg(target_os = "windows")]
    module: HMODULE,
    data: Option<&'static [u8]>,
    offset: usize,
}

impl ResourceFile {
    #[cfg(target_os = "windows")]
    pub fn new(module: HMODULE, filename: &str) -> Self {
        let mut instance = Self {
            resource_name: filename.to_string(),
            module,
            data: None,
            offset: 0,
        };
        instance.load();
        instance
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new(_module: (), filename: &str) -> Self {
        Self {
            resource_name: filename.to_string(),
            data: None,
            offset: 0,
        }
    }

    #[cfg(target_os = "windows")]
    fn load(&mut self) {
        unsafe {
            let name = self.resource_name.clone();
            let res = FindResourceA(self.module, name.as_str(), "File");
            if let Ok(res) = res {
                if let Ok(handle) = LoadResource(self.module, res) {
                    let size = SizeofResource(self.module, res);
                    let ptr = LockResource(handle);
                    if !ptr.is_null() && size > 0 {
                        let slice = std::slice::from_raw_parts(ptr as *const u8, size as usize);
                        self.data = Some(slice);
                        self.offset = 0;
                    }
                }
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.data.is_some()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        if let Some(data) = self.data {
            let remaining = data.len().saturating_sub(self.offset);
            let amount = buffer.len().min(remaining);
            buffer[..amount].copy_from_slice(&data[self.offset..self.offset + amount]);
            self.offset += amount;
            return amount;
        }
        0
    }

    pub fn seek(&mut self, pos: i64, origin: i32) -> i32 {
        if let Some(data) = self.data {
            let len = data.len() as i64;
            let mut new_pos = match origin {
                0 => pos,
                1 => self.offset as i64 + pos,
                _ => len + pos,
            };
            if new_pos < 0 {
                new_pos = 0;
            }
            if new_pos > len {
                new_pos = len;
            }
            self.offset = new_pos as usize;
            return self.offset as i32;
        }
        0
    }

    pub fn size(&self) -> usize {
        self.data.map(|d| d.len()).unwrap_or(0)
    }

    pub fn peek_data(&self) -> Option<&[u8]> {
        self.data
    }
}
