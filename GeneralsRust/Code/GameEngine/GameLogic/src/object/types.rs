//! Object type list utilities (Rust port of `ObjectTypes`).

use std::sync::Arc;

use crate::common::xfer::{Xfer, XferExt};
use crate::common::Snapshot;
use crate::common::{AsciiString, Bool, Int, ThingTemplate, UnsignedInt};
use crate::helpers::TheThingFactory;
use crate::player::Player;

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
    pub fn remove_object_type(&mut self, object_type: &AsciiString) -> Bool {
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
    pub fn is_in_set(&self, object_type: &AsciiString) -> Bool {
        self.object_types.iter().any(|entry| entry == object_type)
    }

    /// Test membership using a thing template reference.
    pub fn contains_template(&self, template: Option<&dyn ThingTemplate>) -> Bool {
        template
            .map(|templ| self.is_in_set(templ.get_name()))
            .unwrap_or(false)
    }

    /// Number of entries.
    pub fn list_size(&self) -> UnsignedInt {
        self.object_types.len() as UnsignedInt
    }

    /// Retrieve the `index`th entry if it exists.
    pub fn nth_in_list(&self, index: Int) -> Option<&AsciiString> {
        if index < 0 {
            return None;
        }
        self.object_types.get(index as usize)
    }

    /// Gather templates referenced by this list and return zero-initialised counts array.
    pub fn prep_for_player_counting(&self) -> (Vec<Arc<dyn ThingTemplate>>, Vec<Int>) {
        let mut templates = Vec::new();
        for name in &self.object_types {
            if let Some(template) = TheThingFactory::find_template(name) {
                templates.push(template);
            }
        }
        let counts = vec![0; templates.len()];
        (templates, counts)
    }

    /// Populate the supplied vectors with templates and counts (C++ style).
    pub fn prep_for_player_counting_into(
        &self,
        templates: &mut Vec<Arc<dyn ThingTemplate>>,
        counts: &mut Vec<Int>,
    ) -> Int {
        for name in &self.object_types {
            if let Some(template) = TheThingFactory::find_template(name) {
                templates.push(template);
            }
        }
        let total = templates.len() as Int;
        counts.resize(total as usize, 0);
        total
    }

    /// Check whether the player can build any template named in this list.
    pub fn can_build_any(&self, player: &Player) -> Bool {
        for name in &self.object_types {
            if let Some(template) = TheThingFactory::find_template(name) {
                if player.can_build_template(template.as_ref()) {
                    return true;
                }
            }
        }
        false
    }

    /// Iterate over stored identifiers.
    pub fn iter(&self) -> impl Iterator<Item = &AsciiString> {
        self.object_types.iter()
    }
}

impl Snapshot for ObjectTypes {
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // No CRC state beyond contents; handled elsewhere if needed.
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        // Versioning retained for parity even though binary compatibility is not yet finalised.
        let current_version: u8 = 1;
        let mut version = current_version;
        if let Err(err) = xfer.xfer_version(&mut version, current_version) {
            log::warn!("ObjectTypes::xfer version negotiation failed: {}", err);
        }

        // Transfer list name.
        let _ = xfer.xfer_string(self.list_name.as_mut_string());

        // Transfer vector size and elements.
        let mut count = self.object_types.len() as u16;
        xfer.xfer_u16(&mut count);

        if !xfer.is_loading() {
            for name in &mut self.object_types {
                let _ = xfer.xfer_string(name.as_mut_string());
            }
        } else {
            if !self.object_types.is_empty() {
                debug_assert!(
                    self.object_types.is_empty(),
                    "ObjectTypes::xfer - object_types should be empty on load"
                );
            }
            self.object_types.clear();
            for _ in 0..count {
                let mut name = AsciiString::from("");
                let _ = xfer.xfer_string(name.as_mut_string());
                self.object_types.push(name);
            }
        }
    }

    fn load_post_process(&mut self) {
        // No additional post-load logic required.
    }
}
