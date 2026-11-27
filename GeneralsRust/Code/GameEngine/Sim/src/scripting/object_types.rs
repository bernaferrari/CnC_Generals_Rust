//! Ordered authored object-type lists used during script evaluation.

use game_engine::common::ascii_string::AsciiString;

/// Data structure tracking a named set of object type identifiers.
#[derive(Debug, Clone)]
pub struct ObjectTypes {
    list_name: AsciiString,
    object_types: Vec<AsciiString>,
}

impl Default for ObjectTypes {
    fn default() -> Self {
        Self {
            list_name: AsciiString::from(""),
            object_types: Vec::new(),
        }
    }
}

impl ObjectTypes {
    /// Construct an empty list with no name.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct an object type list with the supplied name.
    pub fn with_list_name(list_name: AsciiString) -> Self {
        Self {
            list_name,
            object_types: Vec::new(),
        }
    }

    /// Add an object type identifier if it is not already present.
    pub fn add_object_type(&mut self, object_type: AsciiString) {
        if !self.is_in_set(&object_type) {
            self.object_types.push(object_type);
        }
    }

    /// Remove an object type identifier, returning whether it was present.
    pub fn remove_object_type(&mut self, object_type: &AsciiString) -> bool {
        if let Some(index) = self
            .object_types
            .iter()
            .position(|entry| entry == object_type)
        {
            self.object_types.remove(index);
            true
        } else {
            debug_assert!(
                false,
                "Attempted to remove '{}' from '{}', but it wasn't there.",
                object_type.as_str(),
                self.list_name.as_str()
            );
            log::warn!(
                "Attempted to remove '{}' from '{}', but it wasn't there.",
                object_type.as_str(),
                self.list_name.as_str()
            );
            false
        }
    }

    /// Retrieve the list name.
    pub fn list_name(&self) -> &AsciiString {
        &self.list_name
    }

    /// Set list name.
    pub fn set_list_name(&mut self, list_name: AsciiString) {
        self.list_name = list_name;
    }

    /// Return `true` if given identifier is contained.
    pub fn is_in_set(&self, object_type: &AsciiString) -> bool {
        self.object_types.iter().any(|entry| entry == object_type)
    }

    /// Number of entries.
    pub fn list_size(&self) -> u32 {
        self.object_types.len() as u32
    }

    /// Retrieve the `index`th entry if it exists.
    pub fn nth_in_list(&self, index: i32) -> Option<&AsciiString> {
        if index < 0 {
            return None;
        }
        self.object_types.get(index as usize)
    }

    /// Lend the ordered fields to the existing positional snapshot decoder.
    pub fn snapshot_fields_mut(&mut self) -> (&mut AsciiString, &mut Vec<AsciiString>) {
        (&mut self.list_name, &mut self.object_types)
    }

    /// Iterate over stored identifiers.
    pub fn iter(&self) -> impl Iterator<Item = &AsciiString> {
        self.object_types.iter()
    }
}
