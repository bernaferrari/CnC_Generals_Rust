//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

/// Work order for unit production tracking
#[derive(Debug, Clone)]
pub struct WorkOrder {
    pub thing_template: String,       // Template name of thing to build
    pub factory_id: Option<ObjectID>, // ID of factory building this, or None if none
    pub num_completed: i32,           // Number built so far
    pub num_required: i32,            // Number needed total
    pub required: bool,               // True if part of minimum requirement
    pub is_resource_gatherer: bool,   // True if this is a resource gatherer
}

impl WorkOrder {
    pub fn new(thing_template: String) -> Self {
        Self {
            thing_template,
            factory_id: None,
            num_completed: 0,
            num_required: 1,
            required: false,
            is_resource_gatherer: false,
        }
    }

    /// Returns true if nothing is building this unit yet
    pub fn is_waiting_to_build(&self) -> bool {
        self.factory_id.is_none() && self.num_completed < self.num_required
    }

    /// Validate that factory ID still refers to an active object.
    ///
    /// Matches C++ AIPlayer.cpp:3688 WorkOrder::validateFactory.
    /// Checks if the factory object still exists, is alive (not destroyed),
    /// and is still owned by the specified player. If any check fails,
    /// the factory_id is cleared to INVALID_ID.
    pub fn validate_factory(&mut self, player_id: u32) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if self.factory_id.is_none() {
            // C++ parity: if m_factoryID == INVALID_ID, return immediately (valid)
            return Ok(());
        }
        let factory_id = self.factory_id.unwrap();

        // C++ parity: TheGameLogic->findObjectByID(m_factoryID)
        // C++ parity: factory == NULL -> m_factoryID = INVALID_ID
        let Some(owner) = OBJECT_REGISTRY.with_object(factory_id, |factory_guard| {
            factory_guard.get_controlling_player_id()
        }) else {
            self.factory_id = None;
            return Ok(());
        };

        // C++ parity: factory->getControllingPlayer() != thisPlayer
        if owner != Some(player_id as UnsignedInt) {
            self.factory_id = None;
        }

        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut thing_template = self.thing_template.clone();
        let _ = xfer.xfer_ascii_string(&mut thing_template);
        if xfer.is_loading() {
            self.thing_template = thing_template;
        }

        let mut factory_id = self.factory_id.unwrap_or(INVALID_ID);
        let _ = xfer.xfer_object_id(&mut factory_id);
        if xfer.is_loading() {
            self.factory_id = if factory_id == INVALID_ID {
                None
            } else {
                Some(factory_id)
            };
        }

        let mut num_completed = self.num_completed;
        let _ = xfer.xfer_int(&mut num_completed);
        if xfer.is_loading() {
            self.num_completed = num_completed;
        }

        let mut num_required = self.num_required;
        let _ = xfer.xfer_int(&mut num_required);
        if xfer.is_loading() {
            self.num_required = num_required;
        }

        let mut required = self.required;
        let _ = xfer.xfer_bool(&mut required);
        if xfer.is_loading() {
            self.required = required;
        }

        let mut is_resource_gatherer = self.is_resource_gatherer;
        let _ = xfer.xfer_bool(&mut is_resource_gatherer);
        if xfer.is_loading() {
            self.is_resource_gatherer = is_resource_gatherer;
        }
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) {
        let _ = xfer;
    }
}
