use std::path::Path;

#[cfg(target_os = "windows")]
use std::{ffi::CString, os::windows::ffi::OsStrExt};
#[cfg(target_os = "windows")]
use windows::{
    core::{Owned, PCSTR, PCWSTR},
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{
            FindResourceA, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
            LOAD_LIBRARY_AS_DATAFILE_EXCLUSIVE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        },
    },
};

/// The executable or resource-bearing image from which to load a `File` resource.
pub enum ResourceSource<'a> {
    /// The executable used to create this process, which cannot be unloaded.
    CurrentExecutable,
    /// Load the image for resources only; it remains owned until the bytes are copied.
    File(&'a Path),
}

pub struct ResourceFile {
    data: Option<Vec<u8>>,
    offset: usize,
}

impl ResourceFile {
    /// Copies a named `File` resource while its image is owned.
    ///
    /// A raw `HMODULE` cannot safely be used here: another thread can unload it,
    /// and data-file mappings cannot be retained with `GetModuleHandleEx`.
    /// A missing image or resource leaves the file closed.
    #[cfg(target_os = "windows")]
    pub fn new(source: ResourceSource<'_>, filename: &str) -> Self {
        let data = (|| {
            let name = CString::new(filename).ok()?;
            let path = match source {
                ResourceSource::CurrentExecutable => None,
                ResourceSource::File(path) => {
                    let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
                    if path.contains(&0) {
                        return None;
                    }
                    path.push(0);
                    Some(path)
                }
            };

            // SAFETY: A loaded image is held by Owned until after copying, including
            // every failure path. The current executable cannot be unloaded. Both
            // strings are NUL-terminated and the locked resource covers the
            // reported nonzero size for the lifetime of its image.
            unsafe {
                let owner = match &path {
                    Some(path) => Some(Owned::new(
                        LoadLibraryExW(
                            PCWSTR(path.as_ptr()),
                            None,
                            LOAD_LIBRARY_AS_DATAFILE_EXCLUSIVE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
                        )
                        .ok()?,
                    )),
                    None => None,
                };
                let module: Option<HMODULE> = owner.as_ref().map(|handle| **handle);
                let resource = FindResourceA(
                    module,
                    PCSTR(name.as_ptr().cast()),
                    PCSTR(b"File\0".as_ptr()),
                )
                .ok()?;
                let handle = LoadResource(module, resource).ok()?;
                let size = SizeofResource(module, resource) as usize;
                let ptr = LockResource(handle);
                if ptr.is_null() || size == 0 || size > isize::MAX as usize {
                    return None;
                }
                Some(std::slice::from_raw_parts(ptr.cast::<u8>(), size).to_vec())
            }
        })();
        Self { data, offset: 0 }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new(_source: ResourceSource<'_>, _filename: &str) -> Self {
        Self {
            data: None,
            offset: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.data.is_some()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        if let Some(data) = &self.data {
            let remaining = data.len().saturating_sub(self.offset);
            let amount = buffer.len().min(remaining);
            buffer[..amount].copy_from_slice(&data[self.offset..self.offset + amount]);
            self.offset += amount;
            return amount;
        }
        0
    }

    pub fn seek(&mut self, pos: i64, origin: i32) -> i32 {
        if let Some(data) = &self.data {
            let len = data.len() as i64;
            let mut new_pos = match origin {
                0 => pos,
                1 => (self.offset as i64).saturating_add(pos),
                _ => len.saturating_add(pos),
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
        self.data.as_ref().map(|d| d.len()).unwrap_or(0)
    }

    pub fn peek_data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}
