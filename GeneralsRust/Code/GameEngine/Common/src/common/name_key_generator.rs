// name_key_generator.rs - Name to key registry mirroring the legacy engine

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

/// Name key type - must match rts::NameKeyType
pub type NameKeyType = u32;

/// Invalid name-key value (matches C++ `NAMEKEY_INVALID`)
pub const NAMEKEY_INVALID: NameKeyType = 0;
/// Maximum name-key value (matches C++ `NAMEKEY_MAX`)
pub const NAMEKEY_MAX: NameKeyType = 1 << 23;

/// Hash table size used by the legacy generator (`SOCKET_COUNT`)
const SOCKET_COUNT: usize = 45_007;

/// Bucket entry storing a generated name/key pair.
#[derive(Debug, Clone)]
struct BucketEntry {
    key: NameKeyType,
    name: String,
}

/// Internal state for the name-key registry.
#[derive(Debug)]
struct NameKeyGeneratorState {
    buckets: Vec<Vec<BucketEntry>>,
    next_id: NameKeyType,
    reverse_lookup: HashMap<NameKeyType, String>,
}

impl Default for NameKeyGeneratorState {
    fn default() -> Self {
        Self::new()
    }
}

impl NameKeyGeneratorState {
    fn new() -> Self {
        let mut buckets = Vec::with_capacity(SOCKET_COUNT);
        buckets.resize_with(SOCKET_COUNT, Vec::new);
        Self {
            buckets,
            next_id: 1,
            reverse_lookup: HashMap::new(),
        }
    }

    fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.next_id = 1;
        self.reverse_lookup.clear();
    }

    fn allocate_key(&mut self, name: &str) -> NameKeyType {
        let key = self.next_id;
        debug_assert!(
            key < NAMEKEY_MAX,
            "NameKeyGenerator exhausted available key space"
        );
        // Wrapping add keeps behaviour deterministic even in release builds.
        self.next_id = self.next_id.wrapping_add(1);
        self.reverse_lookup.insert(key, name.to_string());
        key
    }

    fn name_to_key(&mut self, name: &str) -> NameKeyType {
        let index = calc_hash(name, false);
        if let Some(entry) = self.buckets[index].iter().find(|entry| entry.name == name) {
            return entry.key;
        }

        let key = self.allocate_key(name);
        self.buckets[index].push(BucketEntry {
            key,
            name: name.to_string(),
        });
        key
    }

    fn name_to_lowercase_key(&mut self, name: &str) -> NameKeyType {
        let index = calc_hash(name, true);
        if let Some(entry) = self.buckets[index]
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        {
            return entry.key;
        }

        let key = self.allocate_key(name);
        self.buckets[index].push(BucketEntry {
            key,
            name: name.to_string(),
        });
        key
    }

    fn key_to_name(&self, key: NameKeyType) -> Option<String> {
        self.reverse_lookup.get(&key).cloned()
    }
}

/// Shared state for the generator.
static NAME_KEY_STATE: Lazy<Mutex<NameKeyGeneratorState>> =
    Lazy::new(|| Mutex::new(NameKeyGeneratorState::new()));

/// Name key generator mirroring the original C++ behaviour.
pub struct NameKeyGenerator;

impl NameKeyGenerator {
    /// Reset the generator to its initial state (`init` in the legacy engine).
    pub fn init() {
        let mut state = NAME_KEY_STATE
            .lock()
            .expect("NameKeyGenerator mutex poisoned");
        state.reset();
    }

    /// Convert a name string to a key (case-sensitive, matches `nameToKey`).
    pub fn name_to_key(name: &str) -> NameKeyType {
        let mut state = NAME_KEY_STATE
            .lock()
            .expect("NameKeyGenerator mutex poisoned");
        state.name_to_key(name)
    }

    /// Convert a name string to a key with case sensitivity explicitly requested.
    ///
    /// This is an alias for [`name_to_key`] preserved for legacy call-sites.
    pub fn name_to_key_case_sensitive(name: &str) -> NameKeyType {
        Self::name_to_key(name)
    }

    /// Convert a name to a key using case-insensitive comparison
    /// (matches `nameToLowercaseKey`).
    pub fn name_to_key_lowercase(name: &str) -> NameKeyType {
        let mut state = NAME_KEY_STATE
            .lock()
            .expect("NameKeyGenerator mutex poisoned");
        state.name_to_lowercase_key(name)
    }

    /// Resolve a key back to the string that first produced it.
    pub fn key_to_name(key: NameKeyType) -> Option<String> {
        let state = NAME_KEY_STATE
            .lock()
            .expect("NameKeyGenerator mutex poisoned");
        state.key_to_name(key)
    }

    /// Clear all registered names (used by save/load and tests).
    pub fn reset() {
        Self::init();
    }
}

/// Global name key generator instance (placeholder for legacy API parity).
pub static THE_NAME_KEY_GENERATOR: NameKeyGenerator = NameKeyGenerator;

fn calc_hash(name: &str, lowercase: bool) -> usize {
    let mut result: u32 = 0;
    for byte in name.bytes() {
        let b = if lowercase {
            byte.to_ascii_lowercase()
        } else {
            byte
        };
        result = result.wrapping_mul(33).wrapping_add(b as u32);
    }
    (result as usize) % SOCKET_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        NameKeyGenerator::reset();
    }

    #[test]
    fn same_name_same_key() {
        reset();
        let key1 = NameKeyGenerator::name_to_key("ActiveShroudUpgrade");
        let key2 = NameKeyGenerator::name_to_key("ActiveShroudUpgrade");
        assert_eq!(key1, key2);
        assert_ne!(key1, NAMEKEY_INVALID);
    }

    #[test]
    fn different_names_unique_keys() {
        reset();
        let key1 = NameKeyGenerator::name_to_key("ActiveShroudUpgrade");
        let key2 = NameKeyGenerator::name_to_key("ArmorUpgrade");
        assert_ne!(key1, key2);
    }

    #[test]
    fn lowercase_lookup_ignores_case() {
        reset();
        let key1 = NameKeyGenerator::name_to_key_lowercase("ControlBar.wnd:MoneyDisplay");
        let key2 = NameKeyGenerator::name_to_key_lowercase("controlbar.wnd:moneydisplay");
        assert_eq!(key1, key2);
    }

    #[test]
    fn lowercase_lookup_matches_lowercase_inserted_with_sensitive_api() {
        reset();
        let key_sensitive = NameKeyGenerator::name_to_key("controlbar.wnd:moneydisplay");
        let key_lookup = NameKeyGenerator::name_to_key_lowercase("ControlBar.wnd:MoneyDisplay");
        assert_eq!(key_sensitive, key_lookup);
    }

    #[test]
    fn key_to_name_round_trips() {
        reset();
        let key = NameKeyGenerator::name_to_key("SpectreGunshipUpdate");
        let name = NameKeyGenerator::key_to_name(key).unwrap();
        assert_eq!(name, "SpectreGunshipUpdate");
    }
}
