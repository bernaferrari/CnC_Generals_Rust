//! Script Engine Implementation
//!
//! This module provides the main script engine that matches the C++ ScriptEngine class.
//! It handles script execution, condition evaluation, action processing, and state management.
//!
//! Split into types, init, update, named trackers, and leftover.

use super::core::*;
use super::events::{AreaTracker, EventManager, NamedObjectTracker};
use crate::common::{
    kind_of_indices, AsciiString, KindOf, ObjectID, INVALID_ID, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{TheAudio, TheGameLogic, TheThingFactory};
use crate::object::object_types::ObjectTypes;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::XferSnapshot;
use crate::team::{get_team_factory, TeamID, TheTeamFactory, TEAM_ID_INVALID};
use crate::ObjectId;
use crate::{GameLogicError, GameLogicResult};
use futures::executor::block_on;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::ScienceType;
use game_engine::common::system::{Xfer, XferMode, XferStatus, XferVersion};
use game_engine::common::thing::thing_factory::get_thing_factory;
use game_engine::common::thing::thing_template::ThingTemplate as EngineThingTemplate;
use std::cell::{Cell, UnsafeCell};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};


/// Wave 348: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

include!("types.rs");
include!("init.rs");
include!("update.rs");
include!("named_trackers.rs");
include!("leftover.rs");

#[cfg(test)]
mod tests;
