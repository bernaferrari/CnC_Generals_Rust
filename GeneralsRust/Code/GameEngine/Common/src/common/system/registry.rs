//! Registry Access Utilities
//!
//! Provides a modern, cross-platform configuration store that mirrors the
//! intent of the original Windows registry access while embracing portable
//! persistence for the Rust-era engine.

use directories::ProjectDirs;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use thiserror::Error;

/// Default qualifier used when creating application specific storage paths.
const DEFAULT_QUALIFIER: &str = "com";
/// Default organisation name for user-visible folders.
const DEFAULT_ORGANISATION: &str = "CnCGenerals";
/// Default application name for configuration storage.
const DEFAULT_APPLICATION: &str = "ZeroHour";
/// Filename used to persist registry values to disk.
const REGISTRY_FILENAME: &str = "registry.json";

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Platform not supported")]
    PlatformNotSupported,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Registry key types serialisable for persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RegistryValue {
    String(String),
    DWord(u32),
    QWord(u64),
    Binary(Vec<u8>),
}

/// Cross-platform registry interface with optional on-disk persistence.
#[derive(Debug)]
pub struct Registry {
    storage_path: Option<PathBuf>,
    values: RwLock<HashMap<String, RegistryValue>>,
}

impl Registry {
    /// Create a registry using the default namespace.
    ///
    /// If the platform does not provide conventional configuration directories
    /// a purely in-memory registry is returned.
    pub fn new() -> Self {
        match Self::with_namespace(DEFAULT_QUALIFIER, DEFAULT_ORGANISATION, DEFAULT_APPLICATION) {
            Ok(registry) => registry,
            Err(err) => {
                log::warn!("Falling back to in-memory registry storage: {}", err);
                Self::in_memory()
            }
        }
    }

    /// Create an in-memory registry without persistence.
    pub fn in_memory() -> Self {
        Self {
            storage_path: None,
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Create a registry scoped to the provided namespace.
    pub fn with_namespace(
        qualifier: &str,
        organisation: &str,
        application: &str,
    ) -> Result<Self, RegistryError> {
        let dirs = ProjectDirs::from(qualifier, organisation, application)
            .ok_or(RegistryError::PlatformNotSupported)?;
        let config_dir = dirs.config_dir();
        let storage_path = config_dir.join(REGISTRY_FILENAME);
        Self::with_storage_path(storage_path)
    }

    /// Create a registry backed by the supplied storage path.
    pub fn with_storage_path<P: Into<PathBuf>>(path: P) -> Result<Self, RegistryError> {
        let path = path.into();
        let values = if path.exists() {
            let data = fs::read(&path)?;
            if data.is_empty() {
                HashMap::new()
            } else {
                serde_json::from_slice(&data)?
            }
        } else {
            HashMap::new()
        };

        Ok(Self {
            storage_path: Some(path),
            values: RwLock::new(values),
        })
    }

    /// Retrieve a clone of a registry value if present.
    pub fn get(&self, key: &str) -> Option<RegistryValue> {
        self.values.read().get(key).cloned()
    }

    /// Read a string value from the registry.
    pub fn read_string(&self, key: &str) -> Result<String, RegistryError> {
        match self.get(key) {
            Some(RegistryValue::String(value)) => Ok(value),
            Some(_) => Err(RegistryError::KeyNotFound(format!(
                "Key '{}' does not contain a string",
                key
            ))),
            None => Err(RegistryError::KeyNotFound(key.to_string())),
        }
    }

    /// Read a DWORD value from the registry.
    pub fn read_dword(&self, key: &str) -> Result<u32, RegistryError> {
        match self.get(key) {
            Some(RegistryValue::DWord(value)) => Ok(value),
            Some(_) => Err(RegistryError::KeyNotFound(format!(
                "Key '{}' does not contain a DWORD",
                key
            ))),
            None => Err(RegistryError::KeyNotFound(key.to_string())),
        }
    }

    /// Read a QWORD value from the registry.
    pub fn read_qword(&self, key: &str) -> Result<u64, RegistryError> {
        match self.get(key) {
            Some(RegistryValue::QWord(value)) => Ok(value),
            Some(_) => Err(RegistryError::KeyNotFound(format!(
                "Key '{}' does not contain a QWORD",
                key
            ))),
            None => Err(RegistryError::KeyNotFound(key.to_string())),
        }
    }

    /// Read raw bytes from the registry.
    pub fn read_binary(&self, key: &str) -> Result<Vec<u8>, RegistryError> {
        match self.get(key) {
            Some(RegistryValue::Binary(value)) => Ok(value),
            Some(_) => Err(RegistryError::KeyNotFound(format!(
                "Key '{}' does not contain binary data",
                key
            ))),
            None => Err(RegistryError::KeyNotFound(key.to_string())),
        }
    }

    /// Write a string value to the registry.
    pub fn write_string(&self, key: &str, value: impl Into<String>) -> Result<(), RegistryError> {
        self.set_value(key, RegistryValue::String(value.into()))
    }

    /// Write a DWORD value to the registry.
    pub fn write_dword(&self, key: &str, value: u32) -> Result<(), RegistryError> {
        self.set_value(key, RegistryValue::DWord(value))
    }

    /// Write a QWORD value to the registry.
    pub fn write_qword(&self, key: &str, value: u64) -> Result<(), RegistryError> {
        self.set_value(key, RegistryValue::QWord(value))
    }

    /// Write raw bytes to the registry.
    pub fn write_binary(&self, key: &str, value: Vec<u8>) -> Result<(), RegistryError> {
        self.set_value(key, RegistryValue::Binary(value))
    }

    /// Remove a key from the registry if present.
    pub fn remove(&self, key: &str) -> Result<(), RegistryError> {
        let mut values = self.values.write();
        values.remove(key);
        drop(values);
        self.persist()
    }

    /// Flush the current value set to disk (if persistence is enabled).
    pub fn flush(&self) -> Result<(), RegistryError> {
        self.persist()
    }

    /// Enumerate all keys currently stored.
    pub fn keys(&self) -> Vec<String> {
        self.values.read().keys().cloned().collect::<Vec<_>>()
    }

    fn set_value(&self, key: &str, value: RegistryValue) -> Result<(), RegistryError> {
        {
            let mut values = self.values.write();
            values.insert(key.to_string(), value);
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), RegistryError> {
        let Some(path) = self.storage_path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let snapshot = self.values.read().clone();
        let json = serde_json::to_vec_pretty(&snapshot)?;
        fs::write(path, json)?;
        Ok(())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// C++ `SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour`.
pub const ZH_REGISTRY_ROOT: &str =
    r"SOFTWARE\Electronic Arts\EA Games\Command and Conquer Generals Zero Hour";

const DEFAULT_REGISTRY_LANGUAGE: &str = "english";
const DEFAULT_REGISTRY_SKU: &str = "GeneralsMPTest";
const DEFAULT_REGISTRY_VERSION: u32 = 65536;

static ZH_REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// Process-wide ZH config store (JSON-backed, with C++ key names).
pub fn zh_registry() -> &'static Registry {
    &ZH_REGISTRY
}

fn registry_lookup_keys(path: &str, key: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let trimmed_path = path.trim_matches('\\');
    // C++ opens HKLM `SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour` + path.
    if trimmed_path.is_empty() {
        keys.push(format!("{}\\{}", ZH_REGISTRY_ROOT, key));
        keys.push(key.to_string());
    } else {
        keys.push(format!("{}\\{}\\{}", ZH_REGISTRY_ROOT, trimmed_path, key));
        keys.push(format!("{}\\{}", trimmed_path, key));
    }
    keys
}

#[cfg(all(windows, feature = "windows"))]
fn hive_query(
    root: winapi::shared::minwindef::HKEY,
    path: &str,
    key: &str,
    string: bool,
) -> Option<Vec<u8>> {
    use std::ffi::CString;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::winnt::{KEY_READ, REG_DWORD, REG_SZ};
    use winapi::um::winreg::{RegCloseKey, RegOpenKeyExA, RegQueryValueExA};

    let full = registry_lookup_keys(path, key).into_iter().next()?;
    let (subkey, value_name) = full.rsplit_once('\\')?;
    let subkey = CString::new(subkey).ok()?;
    let value_name = CString::new(value_name).ok()?;
    unsafe {
        let mut hkey = std::ptr::null_mut();
        if RegOpenKeyExA(root, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return None;
        }
        let mut kind: DWORD = 0;
        let mut size: DWORD = 0;
        if RegQueryValueExA(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        ) != 0
            || size == 0
            || (string && kind != REG_SZ)
            || (!string && kind != REG_DWORD)
        {
            RegCloseKey(hkey);
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let ok = RegQueryValueExA(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            buf.as_mut_ptr(),
            &mut size,
        ) == 0;
        RegCloseKey(hkey);
        ok.then_some(buf)
    }
}

#[cfg(all(windows, feature = "windows"))]
fn hklm_set(path: &str, key: &str, bytes: &[u8], string: bool) -> bool {
    use std::ffi::CString;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::winnt::{KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ};
    use winapi::um::winreg::{RegCloseKey, RegCreateKeyExA, RegSetValueExA, HKEY_LOCAL_MACHINE};

    let Some(full) = registry_lookup_keys(path, key).into_iter().next() else {
        return false;
    };
    let Some((subkey, value_name)) = full.rsplit_once('\\') else {
        return false;
    };
    let Ok(subkey) = CString::new(subkey) else {
        return false;
    };
    let Ok(value_name) = CString::new(value_name) else {
        return false;
    };
    unsafe {
        let mut hkey = std::ptr::null_mut();
        if RegCreateKeyExA(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null_mut(),
            &mut hkey,
            std::ptr::null_mut(),
        ) != 0
        {
            return false;
        }
        let kind: DWORD = if string { REG_SZ } else { REG_DWORD };
        let ok = RegSetValueExA(
            hkey,
            value_name.as_ptr(),
            0,
            kind,
            bytes.as_ptr(),
            bytes.len() as u32,
        ) == 0;
        RegCloseKey(hkey);
        ok
    }
}

/// C++ `GetStringFromRegistry`. HKLM on Windows, then the JSON store.
pub fn get_string_from_registry(path: &str, key: &str) -> Option<String> {
    #[cfg(all(windows, feature = "windows"))]
    {
        use winapi::um::winreg::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        if let Some(buf) = hive_query(HKEY_LOCAL_MACHINE, path, key, true)
            .or_else(|| hive_query(HKEY_CURRENT_USER, path, key, true))
        {
            let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
            if let Ok(text) = String::from_utf8(buf[..end].to_vec()) {
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    let registry = zh_registry();
    for candidate in registry_lookup_keys(path, key) {
        if let Ok(value) = registry.read_string(&candidate) {
            return Some(value);
        }
    }
    None
}

/// C++ `GetUnsignedIntFromRegistry`. HKLM on Windows, then the JSON store.
pub fn get_unsigned_int_from_registry(path: &str, key: &str) -> Option<u32> {
    #[cfg(all(windows, feature = "windows"))]
    {
        use winapi::um::winreg::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        if let Some(buf) = hive_query(HKEY_LOCAL_MACHINE, path, key, false)
            .or_else(|| hive_query(HKEY_CURRENT_USER, path, key, false))
        {
            if buf.len() >= 4 {
                return Some(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]));
            }
        }
    }
    let registry = zh_registry();
    for candidate in registry_lookup_keys(path, key) {
        if let Ok(value) = registry.read_dword(&candidate) {
            return Some(value);
        }
    }
    None
}

/// C++ `setStringInRegistry` for the Zero Hour HKLM key, then the JSON store.
pub fn set_string_in_registry(path: &str, key: &str, value: &str) -> bool {
    #[cfg(all(windows, feature = "windows"))]
    {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        if !hklm_set(path, key, &bytes, true) {
            return false;
        }
    }
    let full = registry_lookup_keys(path, key).into_iter().next();
    let Some(full) = full else {
        return false;
    };
    zh_registry().write_string(&full, value).is_ok()
}

/// C++ `setUnsignedIntInRegistry` for the Zero Hour HKLM key, then the JSON store.
pub fn set_unsigned_int_in_registry(path: &str, key: &str, value: u32) -> bool {
    #[cfg(all(windows, feature = "windows"))]
    {
        if !hklm_set(path, key, &value.to_le_bytes(), false) {
            return false;
        }
    }
    let full = registry_lookup_keys(path, key).into_iter().next();
    let Some(full) = full else {
        return false;
    };
    zh_registry().write_dword(&full, value).is_ok()
}


/// C++ `GetRegistryLanguage` — default `"english"`.
pub fn get_registry_language() -> String {
    get_string_from_registry("", "Language")
        .unwrap_or_else(|| DEFAULT_REGISTRY_LANGUAGE.to_string())
}

/// C++ `GetRegistryGameName` — SKU, default `"GeneralsMPTest"`.
pub fn get_registry_game_name() -> String {
    get_string_from_registry("", "SKU").unwrap_or_else(|| DEFAULT_REGISTRY_SKU.to_string())
}

/// C++ `GetRegistryVersion` — default `65536`.
pub fn get_registry_version() -> u32 {
    get_unsigned_int_from_registry("", "Version").unwrap_or(DEFAULT_REGISTRY_VERSION)
}

/// C++ `GetRegistryMapPackVersion` — default `65536`.
pub fn get_registry_map_pack_version() -> u32 {
    get_unsigned_int_from_registry("", "MapPackVersion").unwrap_or(DEFAULT_REGISTRY_VERSION)
}

/// `UserData` / `UserDataLeafName` leaf used by GlobalData user-data path.
pub fn get_registry_user_data() -> Option<String> {
    get_string_from_registry("", "UserData")
        .or_else(|| get_string_from_registry("", "UserDataLeafName"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_string_value_to_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let registry = Registry::with_storage_path(&path).unwrap();
        registry
            .write_string("player.name", "Aurora")
            .expect("write succeeds");
        drop(registry);

        let reloaded = Registry::with_storage_path(&path).unwrap();
        let value = reloaded.read_string("player.name").expect("value present");
        assert_eq!(value, "Aurora");
    }

    #[test]
    fn supports_multiple_value_types() {
        let registry = Registry::in_memory();
        registry.write_dword("graphics.quality", 3).unwrap();
        registry.write_qword("stats.bytes", 42).unwrap();
        registry
            .write_binary("cache.signature", vec![1, 2, 3])
            .unwrap();

        assert_eq!(registry.read_dword("graphics.quality").unwrap(), 3);
        assert_eq!(registry.read_qword("stats.bytes").unwrap(), 42);
        assert_eq!(
            registry.read_binary("cache.signature").unwrap(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn keys_listing_reflects_changes() {
        let registry = Registry::in_memory();
        registry.write_string("a", "1").unwrap();
        registry.write_dword("b", 2).unwrap();
        registry.write_qword("c", 3).unwrap();
        let mut keys = registry.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);

        registry.remove("b").unwrap();
        let mut keys = registry.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "c"]);
    }

    #[test]
    fn zh_hklm_language_is_readable() {
        let registry = zh_registry();
        let full = format!("{}\\Language", ZH_REGISTRY_ROOT);
        registry
            .write_string(&full, "german")
            .expect("seed zh language");
        assert_eq!(
            get_string_from_registry("", "Language").as_deref(),
            Some("german")
        );
        registry.remove(&full).ok();
    }
}
