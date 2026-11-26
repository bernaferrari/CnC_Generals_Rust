//! The single seam between host simulation code and GameLogic-crate
//! `Object`s.
//!
//! Live match objects belong to Main's object store, outside the legacy
//! `OBJECT_REGISTRY`.
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
