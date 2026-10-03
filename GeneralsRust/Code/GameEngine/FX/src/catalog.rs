use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// Owned definitions or derived backend data. Exact names; `None` is null.
/// Admission replaces an earlier definition like C++ parseFXListDefinition.
/// Engine keys can retain their immutable name buffers without a String copy.
pub struct FxCatalog<Value, Name = String> {
    entries: HashMap<Name, Value>,
}
impl<Value, Name> Default for FxCatalog<Value, Name> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}
impl<Value, Name: Borrow<str> + Eq + Hash> FxCatalog<Value, Name> {
    pub fn find(&self, name: &str) -> Option<&Value> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        self.entries.get(name)
    }
    pub fn insert(&mut self, name: Name, value: Value) {
        self.entries.insert(name, value);
    }
}
