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
        self.pairs.clear();
        self.order.clear();
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
        self.pairs.get(&key).map(|value| value.dict_type())
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

    pub fn set_bool(&mut self, key: u32, value: bool) {
        self.insert_value(key, DictValue::Bool(value));
    }

    pub fn get_bool(&self, key: u32) -> bool {
        match self.pairs.get(&key) {
            Some(DictValue::Bool(value)) => *value,
            Some(other) => {
                log::warn!("Dict::get_bool({key}) found {:?}, expected Bool", other.dict_type());
                false
            }
            None => false,
        }
    }

    pub fn set_int(&mut self, key: u32, value: i32) {
        self.insert_value(key, DictValue::Int(value));
    }

    pub fn get_int(&self, key: u32) -> i32 {
        match self.pairs.get(&key) {
            Some(DictValue::Int(value)) => *value,
            Some(other) => {
                log::warn!("Dict::get_int({key}) found {:?}, expected Int", other.dict_type());
                0
            }
            None => 0,
        }
    }

    pub fn set_real(&mut self, key: u32, value: f32) {
        self.insert_value(key, DictValue::Real(value));
    }

    pub fn get_real(&self, key: u32) -> f32 {
        match self.pairs.get(&key) {
            Some(DictValue::Real(value)) => *value,
            Some(other) => {
                log::warn!("Dict::get_real({key}) found {:?}, expected Real", other.dict_type());
                0.0
            }
            None => 0.0,
        }
    }

    pub fn set_ascii_string(&mut self, key: u32, value: impl Into<String>) {
        self.insert_value(key, DictValue::AsciiString(value.into()));
    }

    pub fn get_ascii_string(&self, key: u32) -> String {
        match self.pairs.get(&key) {
            Some(DictValue::AsciiString(value)) => value.clone(),
            Some(other) => {
                log::warn!("Dict::get_ascii_string({key}) found {:?}, expected AsciiString", other.dict_type());
                String::new()
            }
            None => String::new(),
        }
    }

    pub fn set_unicode_string(&mut self, key: u32, value: impl Into<String>) {
        self.insert_value(key, DictValue::UnicodeString(value.into()));
    }

    pub fn get_unicode_string(&self, key: u32) -> String {
        match self.pairs.get(&key) {
            Some(DictValue::UnicodeString(value)) => value.clone(),
            Some(other) => {
                log::warn!("Dict::get_unicode_string({key}) found {:?}, expected UnicodeString", other.dict_type());
                String::new()
            }
            None => String::new(),
        }
    }

    pub fn remove(&mut self, key: u32) {
        if self.pairs.remove(&key).is_some() {
            self.order.retain(|entry| *entry != key);
        }
    }

    fn insert_value(&mut self, key: u32, value: DictValue) {
        if !self.pairs.contains_key(&key) {
            self.order.push(key);
        }
        self.pairs.insert(key, value);
    }
}
