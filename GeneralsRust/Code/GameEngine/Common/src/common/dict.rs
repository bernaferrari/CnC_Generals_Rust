// dict.rs - Typed dictionary implementation matching the legacy Dict API.

use std::collections::HashMap;

/// Dict value types used for serialization and lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictType {
    Bool,
    Int,
    Real,
    AsciiString,
    UnicodeString,
}

/// Stored value for a dict entry.
#[derive(Debug, Clone)]
pub enum DictValue {
    Bool(bool),
    Int(i32),
    Real(f32),
    AsciiString(String),
    UnicodeString(String),
}

impl DictValue {
    pub fn dict_type(&self) -> DictType {
        match self {
            DictValue::Bool(_) => DictType::Bool,
            DictValue::Int(_) => DictType::Int,
            DictValue::Real(_) => DictType::Real,
            DictValue::AsciiString(_) => DictType::AsciiString,
            DictValue::UnicodeString(_) => DictType::UnicodeString,
        }
    }

    fn copy_from(that: &DictValue) -> DictValue {
        that.clone()
    }
}

/// Dictionary structure keyed by name keys (NameKeyType = u32).
#[derive(Debug, Clone, Default)]
pub struct Dict {
    pairs: HashMap<u32, DictValue>,
    order: Vec<u32>,
}

impl Dict {
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.release_data();
    }

    pub fn get_pair_count(&self) -> usize {
        self.order.len()
    }

    pub fn get_nth_key(&self, index: usize) -> Option<u32> {
        self.order.get(index).copied()
    }

    pub fn get_nth_type(&self, index: usize) -> Option<DictType> {
        let key = self.get_nth_key(index)?;
        self.pairs.get(&key).map(|value| value.dict_type())
    }

    pub fn get_type(&self, key: u32) -> Option<DictType> {
        self.find_pair_by_key(key).map(|value| value.dict_type())
    }

    pub fn get_nth_bool(&self, index: usize) -> bool {
        let Some(key) = self.get_nth_key(index) else {
            return false;
        };
        self.get_bool(key)
    }

    pub fn get_nth_int(&self, index: usize) -> i32 {
        let Some(key) = self.get_nth_key(index) else {
            return 0;
        };
        self.get_int(key)
    }

    pub fn get_nth_real(&self, index: usize) -> f32 {
        let Some(key) = self.get_nth_key(index) else {
            return 0.0;
        };
        self.get_real(key)
    }

    pub fn get_nth_ascii_string(&self, index: usize) -> String {
        let Some(key) = self.get_nth_key(index) else {
            return String::new();
        };
        self.get_ascii_string(key)
    }

    pub fn get_nth_unicode_string(&self, index: usize) -> String {
        let Some(key) = self.get_nth_key(index) else {
            return String::new();
        };
        self.get_unicode_string(key)
    }

    /// Matches C++ Dict::known — true only when the key exists with that type.
    pub fn known(&self, key: u32, dtype: DictType) -> bool {
        self.get_type(key) == Some(dtype)
    }

    pub fn set_bool(&mut self, key: u32, value: bool) {
        self.set_prep(key, DictValue::Bool(value));
    }

    pub fn get_bool(&self, key: u32) -> bool {
        match self.find_pair_by_key(key) {
            Some(DictValue::Bool(value)) => *value,
            Some(other) => {
                log::warn!(
                    "Dict::get_bool({key}) found {:?}, expected Bool",
                    other.dict_type()
                );
                false
            }
            None => false,
        }
    }

    pub fn set_int(&mut self, key: u32, value: i32) {
        self.set_prep(key, DictValue::Int(value));
    }

    pub fn get_int(&self, key: u32) -> i32 {
        match self.find_pair_by_key(key) {
            Some(DictValue::Int(value)) => *value,
            Some(other) => {
                log::warn!(
                    "Dict::get_int({key}) found {:?}, expected Int",
                    other.dict_type()
                );
                0
            }
            None => 0,
        }
    }

    pub fn set_real(&mut self, key: u32, value: f32) {
        self.set_prep(key, DictValue::Real(value));
    }

    pub fn get_real(&self, key: u32) -> f32 {
        match self.find_pair_by_key(key) {
            Some(DictValue::Real(value)) => *value,
            Some(other) => {
                log::warn!(
                    "Dict::get_real({key}) found {:?}, expected Real",
                    other.dict_type()
                );
                0.0
            }
            None => 0.0,
        }
    }

    pub fn set_ascii_string(&mut self, key: u32, value: impl Into<String>) {
        self.set_prep(key, DictValue::AsciiString(value.into()));
    }

    pub fn get_ascii_string(&self, key: u32) -> String {
        match self.find_pair_by_key(key) {
            Some(DictValue::AsciiString(value)) => value.clone(),
            Some(other) => {
                log::warn!(
                    "Dict::get_ascii_string({key}) found {:?}, expected AsciiString",
                    other.dict_type()
                );
                String::new()
            }
            None => String::new(),
        }
    }

    pub fn set_unicode_string(&mut self, key: u32, value: impl Into<String>) {
        self.set_prep(key, DictValue::UnicodeString(value.into()));
    }

    pub fn get_unicode_string(&self, key: u32) -> String {
        match self.find_pair_by_key(key) {
            Some(DictValue::UnicodeString(value)) => value.clone(),
            Some(other) => {
                log::warn!(
                    "Dict::get_unicode_string({key}) found {:?}, expected UnicodeString",
                    other.dict_type()
                );
                String::new()
            }
            None => String::new(),
        }
    }

    /// Matches C++ Dict::remove: true if the pair existed.
    pub fn remove(&mut self, key: u32) -> bool {
        if self.pairs.remove(&key).is_some() {
            self.order.retain(|entry| *entry != key);
            true
        } else {
            false
        }
    }

    /// Matches C++ Dict::copyPairFrom: copy type+value, or drop the local key
    /// when `that` has no pair for it.
    pub fn copy_pair_from(&mut self, that: &Dict, key: u32) {
        if let Some(value) = that.find_pair_by_key(key) {
            self.set_prep(key, DictValue::copy_from(value));
        } else if self.find_pair_by_key(key).is_some() {
            self.remove(key);
        }
    }

    fn find_pair_by_key(&self, key: u32) -> Option<&DictValue> {
        self.pairs.get(&key)
    }

    fn set_prep(&mut self, key: u32, value: DictValue) {
        self.insert_value(key, value);
        self.sort_pairs();
    }

    fn sort_pairs(&mut self) {
        self.order.sort_unstable();
    }

    fn release_data(&mut self) {
        self.pairs.clear();
        self.order.clear();
    }

    fn ensure_unique(&mut self, num_pairs_needed: usize) {
        self.pairs.reserve(num_pairs_needed);
        self.order.reserve(num_pairs_needed);
    }

    fn validate(&self) {
        debug_assert!(self.order.len() == self.pairs.len());
        debug_assert!(self.order.windows(2).all(|w| w[0] < w[1]));
    }

    fn insert_value(&mut self, key: u32, value: DictValue) {
        if self.pairs.contains_key(&key) {
            self.pairs.insert(key, value);
            return;
        }
        self.ensure_unique(self.order.len() + 1);
        let pos = self.order.binary_search(&key).unwrap_or_else(|e| e);
        self.order.insert(pos, key);
        self.pairs.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_pairs_are_sorted_by_name_key() {
        let mut dict = Dict::new();
        dict.set_int(30, 3);
        dict.set_int(10, 1);
        dict.set_int(20, 2);
        assert_eq!(dict.get_pair_count(), 3);
        assert_eq!(dict.get_nth_key(0), Some(10));
        assert_eq!(dict.get_nth_key(1), Some(20));
        assert_eq!(dict.get_nth_key(2), Some(30));
        assert_eq!(dict.get_nth_int(0), 1);
        assert_eq!(dict.get_nth_int(1), 2);
        assert_eq!(dict.get_nth_int(2), 3);
    }

    #[test]
    fn replace_same_key_keeps_sorted_slot_and_can_change_type() {
        let mut dict = Dict::new();
        dict.set_int(7, 42);
        dict.set_ascii_string(7, "replaced");
        assert_eq!(dict.get_pair_count(), 1);
        assert_eq!(dict.get_type(7), Some(DictType::AsciiString));
        assert_eq!(dict.get_ascii_string(7), "replaced");
        assert_eq!(dict.get_int(7), 0);
        assert!(dict.known(7, DictType::AsciiString));
        assert!(!dict.known(7, DictType::Int));
    }

    #[test]
    fn missing_or_wrong_type_returns_cpp_defaults() {
        let mut dict = Dict::new();
        dict.set_bool(1, true);
        assert!(!dict.get_bool(99));
        assert_eq!(dict.get_int(1), 0);
        assert_eq!(dict.get_real(1), 0.0);
        assert_eq!(dict.get_ascii_string(1), "");
        assert_eq!(dict.get_unicode_string(1), "");
        assert_eq!(dict.get_type(99), None);
        assert_eq!(dict.get_nth_key(5), None);
        assert_eq!(dict.get_nth_type(5), None);
        assert!(!dict.get_nth_bool(5));
        assert_eq!(dict.get_nth_int(5), 0);
    }

    #[test]
    fn remove_returns_whether_the_pair_existed() {
        let mut dict = Dict::new();
        dict.set_real(4, 1.5);
        assert!(dict.remove(4));
        assert!(!dict.remove(4));
        assert_eq!(dict.get_pair_count(), 0);
    }

    #[test]
    fn copy_pair_from_copies_or_removes_like_cpp() {
        let mut src = Dict::new();
        src.set_unicode_string(8, "α");
        src.set_int(2, 11);

        let mut dst = Dict::new();
        dst.set_int(8, 99);
        dst.set_bool(3, true);
        dst.copy_pair_from(&src, 8);
        dst.copy_pair_from(&src, 2);
        dst.copy_pair_from(&src, 3);

        assert_eq!(dst.get_unicode_string(8), "α");
        assert_eq!(dst.get_int(2), 11);
        assert_eq!(dst.get_type(3), None);
        assert_eq!(dst.get_nth_key(0), Some(2));
        assert_eq!(dst.get_nth_key(1), Some(8));
        assert_eq!(dst.get_pair_count(), 2);
    }
}
