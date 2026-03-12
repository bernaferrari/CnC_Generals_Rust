//! Registry helpers mirroring WWDownload Registry.cpp/.h.

use crate::config::ConfigManager;

const BASE_KEY: &str =
    "SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour";

#[cfg(target_os = "windows")]
use windows::core::PCSTR;
#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExA, RegOpenKeyExA, RegQueryValueExA, RegSetValueExA, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    REG_SZ,
};

#[cfg(target_os = "windows")]
fn read_string(root: HKEY, path: &str, key: &str, val: &mut String) -> bool {
    let path_c = std::ffi::CString::new(path).ok();
    let key_c = std::ffi::CString::new(key).ok();
    if path_c.is_none() || key_c.is_none() {
        return false;
    }
    let mut handle = HKEY::default();
    let result = unsafe {
        RegOpenKeyExA(
            root,
            PCSTR(path_c.unwrap().as_ptr() as _),
            0,
            KEY_READ,
            &mut handle,
        )
    };
    if result.is_err() {
        return false;
    }

    let mut buffer = [0u8; 256];
    let mut size: u32 = buffer.len() as u32;
    let mut value_type: u32 = 0;
    let query = unsafe {
        RegQueryValueExA(
            handle,
            PCSTR(key_c.unwrap().as_ptr() as _),
            None,
            Some(&mut value_type),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(handle);
    }

    if query.is_err() || value_type != REG_SZ.0 {
        return false;
    }

    let slice = &buffer[..size as usize];
    let end = slice.iter().position(|b| *b == 0).unwrap_or(slice.len());
    if let Ok(str_val) = std::str::from_utf8(&slice[..end]) {
        *val = str_val.to_string();
        return true;
    }
    false
}

#[cfg(target_os = "windows")]
fn read_u32(root: HKEY, path: &str, key: &str, val: &mut u32) -> bool {
    let path_c = std::ffi::CString::new(path).ok();
    let key_c = std::ffi::CString::new(key).ok();
    if path_c.is_none() || key_c.is_none() {
        return false;
    }
    let mut handle = HKEY::default();
    let result = unsafe {
        RegOpenKeyExA(
            root,
            PCSTR(path_c.unwrap().as_ptr() as _),
            0,
            KEY_READ,
            &mut handle,
        )
    };
    if result.is_err() {
        return false;
    }

    let mut buffer: u32 = 0;
    let mut size: u32 = std::mem::size_of::<u32>() as u32;
    let mut value_type: u32 = 0;
    let query = unsafe {
        RegQueryValueExA(
            handle,
            PCSTR(key_c.unwrap().as_ptr() as _),
            None,
            Some(&mut value_type),
            Some((&mut buffer as *mut u32).cast()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(handle);
    }
    if query.is_err() || value_type != REG_DWORD.0 {
        return false;
    }
    *val = buffer;
    true
}

#[cfg(target_os = "windows")]
fn write_string(root: HKEY, path: &str, key: &str, val: &str) -> bool {
    let path_c = std::ffi::CString::new(path).ok();
    let key_c = std::ffi::CString::new(key).ok();
    let val_c = std::ffi::CString::new(val).ok();
    if path_c.is_none() || key_c.is_none() || val_c.is_none() {
        return false;
    }
    let mut handle = HKEY::default();
    let result = unsafe {
        RegCreateKeyExA(
            root,
            PCSTR(path_c.unwrap().as_ptr() as _),
            0,
            PCSTR(b"REG_NONE\0".as_ptr()),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut handle,
            None,
        )
    };
    if result.is_err() {
        return false;
    }

    let data = val_c.unwrap();
    let set = unsafe {
        RegSetValueExA(
            handle,
            PCSTR(key_c.unwrap().as_ptr() as _),
            0,
            REG_SZ,
            Some(data.as_ptr().cast()),
            data.as_bytes_with_nul().len() as u32,
        )
    };
    unsafe {
        let _ = RegCloseKey(handle);
    }
    set.is_ok()
}

#[cfg(target_os = "windows")]
fn write_u32(root: HKEY, path: &str, key: &str, val: u32) -> bool {
    let path_c = std::ffi::CString::new(path).ok();
    let key_c = std::ffi::CString::new(key).ok();
    if path_c.is_none() || key_c.is_none() {
        return false;
    }
    let mut handle = HKEY::default();
    let result = unsafe {
        RegCreateKeyExA(
            root,
            PCSTR(path_c.unwrap().as_ptr() as _),
            0,
            PCSTR(b"REG_NONE\0".as_ptr()),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut handle,
            None,
        )
    };
    if result.is_err() {
        return false;
    }
    let set = unsafe {
        RegSetValueExA(
            handle,
            PCSTR(key_c.unwrap().as_ptr() as _),
            0,
            REG_DWORD,
            Some((&val as *const u32).cast()),
            std::mem::size_of::<u32>() as u32,
        )
    };
    unsafe {
        let _ = RegCloseKey(handle);
    }
    set.is_ok()
}

pub fn get_string_from_registry(path: &str, key: &str, val: &mut String) -> bool {
    let full_path = format!("{BASE_KEY}{path}");
    #[cfg(target_os = "windows")]
    {
        if read_string(HKEY_LOCAL_MACHINE, &full_path, key, val) {
            return true;
        }
        return read_string(HKEY_CURRENT_USER, &full_path, key, val);
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(manager) = ConfigManager::new() {
            if let Ok(Some(value)) = manager.get_string(path, key) {
                *val = value;
                return true;
            }
        }
        false
    }
}

pub fn get_unsigned_int_from_registry(path: &str, key: &str, val: &mut u32) -> bool {
    let full_path = format!("{BASE_KEY}{path}");
    #[cfg(target_os = "windows")]
    {
        if read_u32(HKEY_LOCAL_MACHINE, &full_path, key, val) {
            return true;
        }
        return read_u32(HKEY_CURRENT_USER, &full_path, key, val);
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(manager) = ConfigManager::new() {
            if let Ok(Some(value)) = manager.get_unsigned_int(path, key) {
                *val = value;
                return true;
            }
        }
        false
    }
}

pub fn set_string_in_registry(path: &str, key: &str, val: &str) -> bool {
    let full_path = format!("{BASE_KEY}{path}");
    #[cfg(target_os = "windows")]
    {
        if write_string(HKEY_LOCAL_MACHINE, &full_path, key, val) {
            return true;
        }
        return write_string(HKEY_CURRENT_USER, &full_path, key, val);
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(manager) = ConfigManager::new() {
            return manager.set_string(path, key, val.to_string()).is_ok();
        }
        false
    }
}

pub fn set_unsigned_int_in_registry(path: &str, key: &str, val: u32) -> bool {
    let full_path = format!("{BASE_KEY}{path}");
    #[cfg(target_os = "windows")]
    {
        if write_u32(HKEY_LOCAL_MACHINE, &full_path, key, val) {
            return true;
        }
        return write_u32(HKEY_CURRENT_USER, &full_path, key, val);
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(manager) = ConfigManager::new() {
            return manager.set_unsigned_int(path, key, val).is_ok();
        }
        false
    }
}
