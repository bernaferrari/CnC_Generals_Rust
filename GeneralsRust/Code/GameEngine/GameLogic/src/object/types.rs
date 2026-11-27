//! Object type list utilities (Rust port of `ObjectTypes`).

use std::sync::Arc;

use crate::common::Snapshot;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::{AsciiString, Bool, Int, ThingTemplate, UnsignedInt};
use crate::helpers::TheThingFactory;
use crate::player::Player;

pub use generals_sim::scripting::object_types::ObjectTypes;

pub(crate) trait LegacyObjectTypes {
    fn contains_template(&self, template: Option<&dyn ThingTemplate>) -> Bool;
    fn prep_for_player_counting(&self) -> (Vec<Arc<dyn ThingTemplate>>, Vec<Int>);
    fn can_build_any(&self, player: &Player) -> Bool;
}

impl LegacyObjectTypes for ObjectTypes {
    /// Test membership using a thing template reference.
    fn contains_template(&self, template: Option<&dyn ThingTemplate>) -> Bool {
        template
            .map(|templ| self.is_in_set(templ.get_name()))
            .unwrap_or(false)
    }

    /// Gather templates referenced by this list and return zero-initialised counts array.
    fn prep_for_player_counting(&self) -> (Vec<Arc<dyn ThingTemplate>>, Vec<Int>) {
        let templates: Vec<_> = self
            .iter()
            .filter_map(|name| TheThingFactory::find_template(name))
            .collect();
        let counts = vec![0; templates.len()];
        (templates, counts)
    }

    /// Check whether the player can build any template named in this list.
    fn can_build_any(&self, player: &Player) -> Bool {
        for name in self.iter() {
            if let Some(template) = TheThingFactory::find_template(name) {
                if player.can_build_template(template.as_ref()) {
                    return true;
                }
            }
        }
        false
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

        let (list_name, object_types) = self.snapshot_fields_mut();
        // Transfer list name.
        let _ = xfer.xfer_string(list_name.as_mut_string_buffer());

        // Transfer vector size and elements.
        let mut count = object_types.len() as u16;
        xfer.xfer_u16(&mut count);

        if !xfer.is_loading() {
            for name in &mut *object_types {
                let _ = xfer.xfer_string(name.as_mut_string_buffer());
            }
        } else {
            if !object_types.is_empty() {
                debug_assert!(
                    object_types.is_empty(),
                    "ObjectTypes::xfer - object_types should be empty on load"
                );
            }
            object_types.clear();
            for _ in 0..count {
                let mut name = AsciiString::from("");
                let _ = xfer.xfer_string(name.as_mut_string_buffer());
                object_types.push(name);
            }
        }
    }

    fn load_post_process(&mut self) {
        // No additional post-load logic required.
    }
}
