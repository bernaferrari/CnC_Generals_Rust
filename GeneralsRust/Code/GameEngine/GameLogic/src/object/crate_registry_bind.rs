//! Opt-in crate OBJECT_REGISTRY bind.
//!
//! Host GameWorld create/couple must **not** call this. Production/AI see
//! crate-created objects only after a crate `object_manager` path binds them.

use super::registry::OBJECT_REGISTRY;
use crate::common::{INVALID_ID, ObjectID};
use crate::object::Object;
use std::sync::{Arc, RwLock};

/// Bind a crate-created GameLogic object into [`OBJECT_REGISTRY`].
///
/// Host create/couple tests assert the registry store stays empty; they must
/// never call this helper.
pub fn bind_crate_object(id: ObjectID, object: &Arc<RwLock<Object>>) {
    if id == INVALID_ID {
        return;
    }
    OBJECT_REGISTRY.register_object(id, object);
}
