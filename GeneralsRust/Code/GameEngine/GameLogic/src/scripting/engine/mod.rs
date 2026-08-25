//! Script Engine Implementation
//!
//! This module provides the main script engine that matches the C++ ScriptEngine class.
//! It handles script execution, condition evaluation, action processing, and state management.
//!
//! Split into types, init, update, named trackers, and leftover.

use super::core::*;
use super::events::{AreaTracker, EventManager, NamedObjectTracker};
use crate::ObjectId;
use crate::common::{
    AsciiString, INVALID_ID, KindOf, LOGICFRAMES_PER_SECOND, ObjectID, kind_of_indices,
};
use crate::helpers::{TheAudio, TheGameLogic, TheThingFactory};
use crate::object::object_types::ObjectTypes;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::XferSnapshot;
use crate::team::{TEAM_ID_INVALID, TeamID, TheTeamFactory, get_team_factory};
use crate::{GameLogicError, GameLogicResult};
use futures::executor::block_on;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::ScienceType;
use game_engine::common::system::{Xfer, XferMode, XferStatus, XferVersion};
use game_engine::common::thing::thing_factory::get_thing_factory;
use game_engine::common::thing::thing_template::ThingTemplate as EngineThingTemplate;
use std::cell::Cell;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut}; // InnerMutGuard only; ScriptEngine has no Deref.
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

/// Concatenated live sources for residual `include_str!` scans.
pub const SCRIPT_ENGINE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("init.rs"),
    include_str!("leftover.rs"),
    include_str!("named_trackers.rs"),
    include_str!("types.rs"),
    include_str!("update.rs"),
);
