//! Frame and trigger geometry for one synchronous script dispatch.

use super::TriggerWorld;
use game_engine::polygon_trigger::PolygonTrigger;
use std::sync::{Arc, Mutex};

/// Trigger history for the receiving simulation and its authored map polygons.
pub type ScriptTriggerWorld = TriggerWorld<PolygonTrigger>;

/// A program frame supplied by the caller, with its map's trigger history
/// when the dispatch belongs to a receiving match.
#[derive(Clone)]
pub struct ScriptContext {
    pub current_frame: u32,
    trigger_world: Option<Arc<Mutex<ScriptTriggerWorld>>>,
}

impl ScriptContext {
    pub fn at_frame(current_frame: u32) -> Self {
        Self {
            current_frame,
            trigger_world: None,
        }
    }

    pub fn for_session_at_frame(
        current_frame: u32,
        trigger_world: Arc<Mutex<ScriptTriggerWorld>>,
    ) -> Self {
        Self {
            current_frame,
            trigger_world: Some(trigger_world),
        }
    }

    /// Read the trigger state lent for this dispatch, or an empty standalone
    /// view when the caller supplied only a frame.
    pub fn with_trigger_world<R>(&self, read: impl FnOnce(&ScriptTriggerWorld) -> R) -> R {
        match &self.trigger_world {
            Some(world) => read(&world.lock().unwrap_or_else(|error| error.into_inner())),
            None => read(&ScriptTriggerWorld::default()),
        }
    }

    /// Installed empty maps are authoritative, just like maps with triggers.
    pub fn owned_trigger_area(&self, area_name: &str) -> Option<Option<PolygonTrigger>> {
        self.with_trigger_world(|world| {
            world
                .has_authored_trigger_geometry()
                .then(|| world.trigger_area_by_name(area_name))
        })
    }
}
