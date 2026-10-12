//! The single seam between host simulation code and GameLogic-crate
//! `Object`s.
//!
//! A host match never admits crate Objects (host objects stay out of
//! `OBJECT_REGISTRY`, see `gameworld_shadow`), so every probe misses there.
//! Answering from the empty registry skips the crate `GameLogic` singleton
//! `try_lock` on per-frame paths, and gives the crate Object model one
//! quarantine point.

use gamelogic::object::Object as CrateObject;
use gamelogic::object::registry::OBJECT_REGISTRY;
use std::sync::{Arc, RwLock};

/// C++ `TheGameLogic->findObjectByID` for a crate Object, if one exists.
pub(crate) fn crate_object(id: u32) -> Option<Arc<RwLock<CrateObject>>> {
    if OBJECT_REGISTRY.is_empty() {
        return None;
    }
    gamelogic::helpers::TheGameLogic::find_object_by_id(id)
}
