//! Windows registry utilities (ported from WWLib registry.cpp/h).

use crate::ini::INIClass;
use crate::string_system::AsciiString;
use crate::vector_class::DynamicVectorClass;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "unicode")]
use crate::string_system::UnicodeString;
#[cfg(not(feature = "unicode"))]
type UnicodeString = AsciiString;

#[cfg(target_os = "windows")]
use std::ffi::{CStr, CString};

#[cfg(target_os = "windows")]
use windows::core::{PCSTR, PCWSTR, PSTR};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::ERROR_SUCCESS;

#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExA, RegDeleteKeyA, RegDeleteValueA, RegEnumKeyExA, RegEnumValueA,
    RegOpenKeyExA, RegOpenKeyExW, RegQueryInfoKeyA, RegQueryValueExA, RegQueryValueExW,
    RegSetValueExA, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, REG_BINARY,
    REG_DWORD, REG_SZ,
};

static IS_LOCKED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct RegistryClass {
    #[cfg(target_os = "windows")]
    key: HKEY,
    is_valid: bool,
}

impl RegistryClass {
    pub fn exists(sub_key: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            let Ok(name) = CString::new(sub_key) else {
                return false;
            };
            let mut key = HKEY::default();
            let result = unsafe {
                RegOpenKeyExA(
                    HKEY_LOCAL_MACHINE,
                    PCSTR(name.as_ptr() as _),
                    0,
                    KEY_READ,
                    &mut key,
                )
            };
            if result == ERROR_SUCCESS.0 {
                unsafe {
                    RegCloseKey(key);
                }
                return true;
            }
            return false;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = sub_key;
            false
        }
    }

    pub fn new(sub_key: &str, create: bool) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut key = HKEY::default();
            let Ok(name) = CString::new(sub_key) else {
                return Self {
                    key,
                    is_valid: false,
                };
            };

            let mut result = -1i32;
            if create && !IS_LOCKED.load(Ordering::Relaxed) {
                let mut disposition = 0u32;
                result = unsafe {
                    RegCreateKeyExA(
                        HKEY_LOCAL_MACHINE,
                        PCSTR(name.as_ptr() as _),
                        0,
                        PCSTR::null(),
                        0,
                        KEY_ALL_ACCESS,
                        None,
                        &mut key,
                        Some(&mut disposition),
                    )
                };
            } else {
                let access = if IS_LOCKED.load(Ordering::Relaxed) {
                    KEY_READ
                } else {
                    KEY_ALL_ACCESS
                };
                result = unsafe {
                    RegOpenKeyExA(
                        HKEY_LOCAL_MACHINE,
                        PCSTR(name.as_ptr() as _),
                        0,
                        access,
                        &mut key,
                    )
                };
            }

            Self {
                key,
                is_valid: result == ERROR_SUCCESS.0,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (sub_key, create);
            Self { is_valid: false }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn set_read_only(set: bool) {
        IS_LOCKED.store(set, Ordering::Relaxed);
    }

    pub fn get_int(&self, name: &str, def_value: i32) -> i32 {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                return def_value;
            }
            let Ok(value_name) = CString::new(name) else {
                return def_value;
            };
            let mut data: u32 = 0;
            let mut data_len: u32 = std::mem::size_of::<u32>() as u32;
            let mut data_type: u32 = 0;
            let result = unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    Some(&mut data_type),
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_len),
                )
            };
            if result == ERROR_SUCCESS.0 && data_type == REG_DWORD.0 {
                data as i32
            } else {
                def_value
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, def_value);
            def_value
        }
    }

    pub fn set_int(&self, name: &str, value: i32) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            let data = value as u32;
            unsafe {
                RegSetValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    0,
                    REG_DWORD,
                    Some(&data as *const u32 as *const u8),
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, value);
        }
    }

    pub fn get_bool(&self, name: &str, def_value: bool) -> bool {
        self.get_int(name, if def_value { 1 } else { 0 }) != 0
    }

    pub fn set_bool(&self, name: &str, value: bool) {
        self.set_int(name, if value { 1 } else { 0 });
    }

    pub fn get_float(&self, name: &str, def_value: f32) -> f32 {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                return def_value;
            }
            let Ok(value_name) = CString::new(name) else {
                return def_value;
            };
            let mut data: u32 = 0;
            let mut data_len: u32 = std::mem::size_of::<u32>() as u32;
            let mut data_type: u32 = 0;
            let result = unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    Some(&mut data_type),
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_len),
                )
            };
            if result == ERROR_SUCCESS.0 && data_type == REG_DWORD.0 {
                f32::from_bits(data)
            } else {
                def_value
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, def_value);
            def_value
        }
    }

    pub fn set_float(&self, name: &str, value: f32) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            let data = value.to_bits();
            unsafe {
                RegSetValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    0,
                    REG_DWORD,
                    Some(&data as *const u32 as *const u8),
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, value);
        }
    }

    pub fn get_bin_size(&self, name: &str) -> usize {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                return 0;
            }
            let Ok(value_name) = CString::new(name) else {
                return 0;
            };
            let mut size: u32 = 0;
            unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    None,
                    None,
                    Some(&mut size),
                );
            }
            size as usize
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            0
        }
    }

    pub fn get_bin(&self, name: &str, buffer: &mut [u8]) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || buffer.is_empty() {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            let mut size: u32 = buffer.len() as u32;
            unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    None,
                    Some(buffer.as_mut_ptr()),
                    Some(&mut size),
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, buffer);
        }
    }

    pub fn set_bin(&self, name: &str, buffer: &[u8]) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) || buffer.is_empty() {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            unsafe {
                RegSetValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    0,
                    REG_BINARY,
                    Some(buffer.as_ptr()),
                    buffer.len() as u32,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, buffer);
        }
    }

    pub fn get_string_into(
        &self,
        name: &str,
        string: &mut AsciiString,
        default_string: Option<&str>,
    ) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                let _ = string.set_str(default_string.unwrap_or(""));
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                let _ = string.set_str(default_string.unwrap_or(""));
                return;
            };

            let mut data_size: u32 = 0;
            let mut data_type: u32 = 0;
            let result = unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    Some(&mut data_type),
                    None,
                    Some(&mut data_size),
                )
            };

            if result == ERROR_SUCCESS.0 && data_type == REG_SZ.0 && data_size > 0 {
                let mut buffer = vec![0u8; data_size as usize];
                let result = unsafe {
                    RegQueryValueExA(
                        self.key,
                        PCSTR(value_name.as_ptr() as _),
                        None,
                        Some(&mut data_type),
                        Some(buffer.as_mut_ptr()),
                        Some(&mut data_size),
                    )
                };
                if result == ERROR_SUCCESS.0 {
                    if let Ok(cstr) = CStr::from_bytes_with_nul(&buffer) {
                        let _ = string.set_str(&cstr.to_string_lossy());
                        return;
                    } else {
                        if let Some(pos) = buffer.iter().position(|b| *b == 0) {
                            let _ = string.set_str(&String::from_utf8_lossy(&buffer[..pos]));
                            return;
                        }
                    }
                }
            }

            let _ = string.set_str(default_string.unwrap_or(""));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            let _ = string.set_str(default_string.unwrap_or(""));
        }
    }

    pub fn get_string_buf(
        &self,
        name: &str,
        value: &mut [u8],
        default_string: Option<&str>,
    ) -> usize {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || value.is_empty() {
                return 0;
            }
            let Ok(value_name) = CString::new(name) else {
                return 0;
            };
            let mut data_type: u32 = 0;
            let mut data_len: u32 = value.len() as u32;
            let result = unsafe {
                RegQueryValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    None,
                    Some(&mut data_type),
                    Some(value.as_mut_ptr()),
                    Some(&mut data_len),
                )
            };
            if result == ERROR_SUCCESS.0 && data_type == REG_SZ.0 {
                return data_len as usize;
            }

            if let Some(default_string) = default_string {
                let bytes = default_string.as_bytes();
                let len = bytes.len().min(value.len().saturating_sub(1));
                value[..len].copy_from_slice(&bytes[..len]);
                value[len] = 0;
                return len;
            }

            if !value.is_empty() {
                value[0] = 0;
            }
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            if let Some(default_string) = default_string {
                let bytes = default_string.as_bytes();
                let len = bytes.len().min(value.len().saturating_sub(1));
                value[..len].copy_from_slice(&bytes[..len]);
                value[len] = 0;
                return len;
            }
            if !value.is_empty() {
                value[0] = 0;
            }
            0
        }
    }

    pub fn set_string(&self, name: &str, value: &str) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            let Ok(value_cstr) = CString::new(value) else {
                return;
            };
            unsafe {
                RegSetValueExA(
                    self.key,
                    PCSTR(value_name.as_ptr() as _),
                    0,
                    REG_SZ,
                    Some(value_cstr.as_ptr() as *const u8),
                    (value_cstr.as_bytes_with_nul().len()) as u32,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, value);
        }
    }

    pub fn get_wide_string_into(
        &self,
        name: &UnicodeString,
        string: &mut UnicodeString,
        default_string: Option<&UnicodeString>,
    ) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                if let Some(default_string) = default_string {
                    let _ = string.set_str(default_string.as_str());
                } else {
                    string.clear();
                }
                return;
            }

            let name_wide: Vec<u16> = name.as_str().encode_utf16().chain([0]).collect();
            let mut data_size: u32 = 0;
            let mut data_type: u32 = 0;
            let result = unsafe {
                RegQueryValueExW(
                    self.key,
                    PCWSTR(name_wide.as_ptr()),
                    None,
                    Some(&mut data_type),
                    None,
                    Some(&mut data_size),
                )
            };
            if result == ERROR_SUCCESS.0 && data_type == REG_SZ.0 && data_size > 0 {
                let mut buffer = vec![0u16; (data_size as usize / 2) + 1];
                let result = unsafe {
                    RegQueryValueExW(
                        self.key,
                        PCWSTR(name_wide.as_ptr()),
                        None,
                        Some(&mut data_type),
                        Some(buffer.as_mut_ptr() as *mut u8),
                        Some(&mut data_size),
                    )
                };
                if result == ERROR_SUCCESS.0 {
                    if let Some(pos) = buffer.iter().position(|b| *b == 0) {
                        let utf8 = String::from_utf16_lossy(&buffer[..pos]);
                        let _ = string.set_str(&utf8);
                        return;
                    }
                }
            }

            if let Some(default_string) = default_string {
                let _ = string.set_str(default_string.as_str());
            } else {
                string.clear();
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            if let Some(default_string) = default_string {
                let _ = string.set_str(default_string.as_str());
            } else {
                string.clear();
            }
        }
    }

    pub fn set_wide_string(&self, name: &UnicodeString, value: &UnicodeString) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) {
                return;
            }
            let name_wide: Vec<u16> = name.as_str().encode_utf16().chain([0]).collect();
            let value_wide: Vec<u16> = value.as_str().encode_utf16().chain([0]).collect();
            let size = (value_wide.len() * 2) as u32;
            unsafe {
                RegSetValueExW(
                    self.key,
                    PCWSTR(name_wide.as_ptr()),
                    0,
                    REG_SZ,
                    Some(value_wide.as_ptr() as *const u8),
                    size,
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (name, value);
        }
    }

    pub fn get_value_list(&self, list: &mut DynamicVectorClass<AsciiString>) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid {
                return;
            }
            let mut index: u32 = 0;
            loop {
                let mut name_buffer = [0u8; 128];
                let mut name_size: u32 = name_buffer.len() as u32;
                let result = unsafe {
                    RegEnumValueA(
                        self.key,
                        index,
                        PSTR(name_buffer.as_mut_ptr()),
                        &mut name_size,
                        None,
                        None,
                        None,
                        None,
                    )
                };
                if result != ERROR_SUCCESS.0 {
                    break;
                }
                let name_len = name_size as usize;
                if name_len < name_buffer.len() {
                    name_buffer[name_len] = 0;
                }
                if let Ok(cstr) = CStr::from_bytes_with_nul(&name_buffer[..name_len + 1]) {
                    let _ = list
                        .add(AsciiString::from_str(&cstr.to_string_lossy()).unwrap_or_default());
                } else if let Some(pos) = name_buffer.iter().position(|b| *b == 0) {
                    let _ = list.add(
                        AsciiString::from_str(&String::from_utf8_lossy(&name_buffer[..pos]))
                            .unwrap_or_default(),
                    );
                }
                index += 1;
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list;
        }
    }

    pub fn delete_value(&self, name: &str) {
        #[cfg(target_os = "windows")]
        {
            if !self.is_valid || IS_LOCKED.load(Ordering::Relaxed) {
                return;
            }
            let Ok(value_name) = CString::new(name) else {
                return;
            };
            unsafe {
                RegDeleteValueA(self.key, PCSTR(value_name.as_ptr() as _));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
        }
    }

    pub fn delete_all_values(&self) {
        if IS_LOCKED.load(Ordering::Relaxed) {
            return;
        }
        let mut list: DynamicVectorClass<AsciiString> = DynamicVectorClass::new(0, None);
        self.get_value_list(&mut list);
        for index in 0..list.count() {
            let value = list[index].as_str();
            self.delete_value(value);
        }
    }

    pub fn save_registry(filename: &str, path: &str) {
        let mut ini = INIClass::new();
        #[cfg(target_os = "windows")]
        Self::save_registry_tree(path, &mut ini);
        #[cfg(not(target_os = "windows"))]
        let _ = path;
        let _ = ini.save(filename);
    }

    pub fn load_registry(filename: &str, old_path: &str, new_path: &str) {
        if IS_LOCKED.load(Ordering::Relaxed) {
            return;
        }

        let mut ini = INIClass::new();
        if ini.load(filename).is_err() {
            return;
        }

        let old_path_len = old_path.len();
        let section_names = ini.get_section_names();

        for section_name in section_names {
            let mut path = String::from(new_path);
            if let Some(cut) = section_name.find(old_path) {
                path.push_str(&section_name[cut + old_path_len..]);
            }

            let reg = RegistryClass::new(&path, true);
            if !reg.is_valid() {
                continue;
            }

            let entry_keys = ini.get_entry_keys(section_name);
            for entry in entry_keys {
                if let Some(rest) = entry.strip_prefix("BIN_") {
                    let mut buffer = vec![0u8; 8192];
                    let len = ini.get_uublock_entry(section_name, entry, &mut buffer);
                    reg.set_bin(rest, &buffer[..len]);
                } else if let Some(rest) = entry.strip_prefix("DWORD_") {
                    let temp = ini.get_int(section_name, entry, 0);
                    reg.set_int(rest, temp);
                } else if let Some(rest) = entry.strip_prefix("STRING_") {
                    let value = ini.get_string(section_name, entry, "");
                    reg.set_string(rest, &value);
                }
            }
        }
    }

    pub fn delete_registry_tree(path: &str) {
        if IS_LOCKED.load(Ordering::Relaxed) {
            return;
        }
        #[cfg(target_os = "windows")]
        {
            let Ok(path_c) = CString::new(path) else {
                return;
            };
            unsafe {
                let mut base_key = HKEY::default();
                let result = RegOpenKeyExA(
                    HKEY_LOCAL_MACHINE,
                    PCSTR(path_c.as_ptr() as _),
                    0,
                    KEY_ALL_ACCESS,
                    &mut base_key,
                );
                if result != ERROR_SUCCESS.0 {
                    return;
                }

                Self::delete_registry_values(base_key);

                let mut index: u32 = 0;
                let mut max_times = 1000;
                loop {
                    let mut name_buffer = [0u8; 256];
                    let mut name_size: u32 = name_buffer.len() as u32;
                    let result = RegEnumKeyExA(
                        base_key,
                        index,
                        PSTR(name_buffer.as_mut_ptr()),
                        &mut name_size,
                        None,
                        None,
                        None,
                        None,
                    );
                    if result != ERROR_SUCCESS.0 {
                        break;
                    }

                    let name_len = name_size as usize;
                    if name_len < name_buffer.len() {
                        name_buffer[name_len] = 0;
                    }
                    if let Some(pos) = name_buffer.iter().position(|b| *b == 0) {
                        let name = String::from_utf8_lossy(&name_buffer[..pos]);
                        let new_key_path = format!("{}\\{}", path, name);
                        let Ok(new_key_c) = CString::new(new_key_path.clone()) else {
                            break;
                        };

                        let mut sub_key = HKEY::default();
                        let new_result = RegOpenKeyExA(
                            HKEY_LOCAL_MACHINE,
                            PCSTR(new_key_c.as_ptr() as _),
                            0,
                            KEY_ALL_ACCESS,
                            &mut sub_key,
                        );
                        if new_result == ERROR_SUCCESS.0 {
                            let mut num_subs: u32 = 0;
                            let mut num_values: u32 = 0;
                            let info_result = RegQueryInfoKeyA(
                                sub_key,
                                None,
                                None,
                                None,
                                Some(&mut num_subs),
                                None,
                                None,
                                Some(&mut num_values),
                                None,
                                None,
                                None,
                                None,
                            );
                            if info_result == ERROR_SUCCESS.0 {
                                if num_subs > 0 {
                                    Self::delete_registry_tree(&new_key_path);
                                }
                                if num_values > 0 {
                                    Self::delete_registry_values(sub_key);
                                }
                            }
                            RegCloseKey(sub_key);
                            if let Ok(name_c) = CString::new(name.as_ref()) {
                                RegDeleteKeyA(base_key, PCSTR(name_c.as_ptr() as _));
                            }
                        }
                    }

                    max_times -= 1;
                    if max_times <= 0 {
                        break;
                    }
                    index += 1;
                }

                RegCloseKey(base_key);
                let _ = RegDeleteKeyA(HKEY_LOCAL_MACHINE, PCSTR(path_c.as_ptr() as _));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = path;
        }
    }

    #[cfg(target_os = "windows")]
    fn save_registry_tree(path: &str, ini: &mut INIClass) {
        let Ok(path_c) = CString::new(path) else {
            return;
        };
        unsafe {
            let mut base_key = HKEY::default();
            let result = RegOpenKeyExA(
                HKEY_LOCAL_MACHINE,
                PCSTR(path_c.as_ptr() as _),
                0,
                KEY_ALL_ACCESS,
                &mut base_key,
            );
            if result != ERROR_SUCCESS.0 {
                return;
            }

            Self::save_registry_values(base_key, path, ini);

            let mut index: u32 = 0;
            loop {
                let mut name_buffer = [0u8; 256];
                let mut name_size: u32 = name_buffer.len() as u32;
                let result = RegEnumKeyExA(
                    base_key,
                    index,
                    PSTR(name_buffer.as_mut_ptr()),
                    &mut name_size,
                    None,
                    None,
                    None,
                    None,
                );
                if result != ERROR_SUCCESS.0 {
                    break;
                }

                let name_len = name_size as usize;
                if name_len < name_buffer.len() {
                    name_buffer[name_len] = 0;
                }
                if let Some(pos) = name_buffer.iter().position(|b| *b == 0) {
                    let name = String::from_utf8_lossy(&name_buffer[..pos]);
                    let new_key_path = format!("{}\\{}", path, name);
                    let Ok(new_key_c) = CString::new(new_key_path.clone()) else {
                        break;
                    };

                    let mut sub_key = HKEY::default();
                    let new_result = RegOpenKeyExA(
                        HKEY_LOCAL_MACHINE,
                        PCSTR(new_key_c.as_ptr() as _),
                        0,
                        KEY_ALL_ACCESS,
                        &mut sub_key,
                    );
                    if new_result == ERROR_SUCCESS.0 {
                        let mut num_subs: u32 = 0;
                        let mut num_values: u32 = 0;
                        let info_result = RegQueryInfoKeyA(
                            sub_key,
                            None,
                            None,
                            None,
                            Some(&mut num_subs),
                            None,
                            None,
                            Some(&mut num_values),
                            None,
                            None,
                            None,
                            None,
                        );
                        if info_result == ERROR_SUCCESS.0 {
                            if num_subs > 0 {
                                Self::save_registry_tree(&new_key_path, ini);
                            }
                            if num_values > 0 {
                                Self::save_registry_values(sub_key, &new_key_path, ini);
                            }
                        }
                        RegCloseKey(sub_key);
                    }
                }
                index += 1;
            }

            RegCloseKey(base_key);
        }
    }

    #[cfg(target_os = "windows")]
    fn save_registry_values(key: HKEY, path: &str, ini: &mut INIClass) {
        let mut index: u32 = 0;
        loop {
            let mut value_name = [0u8; 256];
            let mut value_name_size: u32 = value_name.len() as u32;
            let mut data = [0u8; 8192];
            let mut data_size: u32 = data.len() as u32;
            let mut data_type: u32 = 0;

            let result = unsafe {
                RegEnumValueA(
                    key,
                    index,
                    PSTR(value_name.as_mut_ptr()),
                    &mut value_name_size,
                    None,
                    Some(&mut data_type),
                    Some(data.as_mut_ptr()),
                    Some(&mut data_size),
                )
            };
            if result != ERROR_SUCCESS.0 {
                break;
            }

            if (value_name_size as usize) < value_name.len() {
                value_name[value_name_size as usize] = 0;
            }
            let name = if let Some(pos) = value_name.iter().position(|b| *b == 0) {
                String::from_utf8_lossy(&value_name[..pos]).to_string()
            } else {
                String::new()
            };

            match data_type {
                t if t == REG_DWORD.0 => {
                    if data_size >= 4 {
                        let value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as i32;
                        let entry = format!("DWORD_{}", name);
                        ini.put_int(path, &entry, value);
                    }
                }
                t if t == REG_SZ.0 => {
                    let entry = format!("STRING_{}", name);
                    let value =
                        if let Ok(cstr) = CStr::from_bytes_with_nul(&data[..data_size as usize]) {
                            cstr.to_string_lossy().to_string()
                        } else if let Some(pos) = data.iter().position(|b| *b == 0) {
                            String::from_utf8_lossy(&data[..pos]).to_string()
                        } else {
                            String::new()
                        };
                    ini.put_string(path, &entry, &value);
                }
                t if t == REG_BINARY.0 => {
                    let entry = format!("BIN_{}", name);
                    let _ = ini.put_uublock_entry(path, &entry, &data[..data_size as usize]);
                }
                _ => {}
            }
            index += 1;
        }
    }

    #[cfg(target_os = "windows")]
    fn delete_registry_values(key: HKEY) {
        let mut index: u32 = 0;
        loop {
            let mut value_name = [0u8; 256];
            let mut value_name_size: u32 = value_name.len() as u32;
            let mut data = [0u8; 8192];
            let mut data_size: u32 = data.len() as u32;
            let mut data_type: u32 = 0;

            let result = unsafe {
                RegEnumValueA(
                    key,
                    index,
                    PSTR(value_name.as_mut_ptr()),
                    &mut value_name_size,
                    None,
                    Some(&mut data_type),
                    Some(data.as_mut_ptr()),
                    Some(&mut data_size),
                )
            };
            if result != ERROR_SUCCESS.0 {
                break;
            }

            if (value_name_size as usize) < value_name.len() {
                value_name[value_name_size as usize] = 0;
            }
            if let Some(pos) = value_name.iter().position(|b| *b == 0) {
                let name = CString::new(&value_name[..pos]).unwrap_or_default();
                unsafe {
                    let _ = RegDeleteValueA(key, PCSTR(name.as_ptr() as _));
                }
            }
            index += 1;
        }
    }
}

impl Drop for RegistryClass {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if self.is_valid {
                unsafe {
                    let _ = RegCloseKey(self.key);
                }
                self.is_valid = false;
            }
        }
    }
}
