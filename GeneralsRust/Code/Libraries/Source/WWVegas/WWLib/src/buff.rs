//! Buffer container (ported from WWLib buff.h).

use std::ptr::NonNull;

#[derive(Debug)]
pub struct Buffer {
    ptr: Option<NonNull<u8>>,
    size: i32,
    owned: Option<Vec<u8>>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            ptr: None,
            size: 0,
            owned: None,
        }
    }

    pub fn with_size(size: i32) -> Self {
        if size <= 0 {
            return Self::new();
        }
        let mut owned = vec![0u8; size as usize];
        let ptr = NonNull::new(owned.as_mut_ptr());
        Self {
            ptr,
            size,
            owned: Some(owned),
        }
    }

    pub fn from_ptr(ptr: *mut u8, size: i32) -> Self {
        Self {
            ptr: NonNull::new(ptr),
            size,
            owned: None,
        }
    }

    pub fn from_const(ptr: *const u8, size: i32) -> Self {
        Self {
            ptr: NonNull::new(ptr as *mut u8),
            size,
            owned: None,
        }
    }

    pub fn reset(&mut self) {
        self.ptr = None;
        self.size = 0;
        self.owned = None;
    }

    pub fn get_buffer(&self) -> *mut u8 {
        self.ptr.map(|p| p.as_ptr()).unwrap_or(std::ptr::null_mut())
    }

    pub fn get_size(&self) -> i32 {
        self.size
    }

    pub fn is_valid(&self) -> bool {
        self.ptr.is_some()
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        let ptr = self.ptr?;
        if self.size <= 0 {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(ptr.as_ptr(), self.size as usize)) }
    }

    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        let ptr = self.ptr?;
        if self.size <= 0 {
            return None;
        }
        unsafe {
            Some(std::slice::from_raw_parts_mut(
                ptr.as_ptr(),
                self.size as usize,
            ))
        }
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        if let Some(owned) = &self.owned {
            let mut cloned = owned.clone();
            let ptr = NonNull::new(cloned.as_mut_ptr());
            return Self {
                ptr,
                size: self.size,
                owned: Some(cloned),
            };
        }

        Self {
            ptr: self.ptr,
            size: self.size,
            owned: None,
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
