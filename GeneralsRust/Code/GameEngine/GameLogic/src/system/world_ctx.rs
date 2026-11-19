//! Explicit world context for object/module operations (V3.0).
//!
//! `ModuleCtx` is a bundle of *borrowed* references to the services of the one
//! world that is running an operation: partition manager, shroud, object index
//! and player list, plus the world's frame clock. It owns nothing, is `Copy`,
//! and is never stored on an `Object` (no world back-pointer): the owner builds
//! it for the duration of an operation and passes it down.
//!
//! This is the destination of the migration that removes
//! `Object::with_active_world_vision`: as each object/module entry point grows
//! an explicit-ctx twin, its ambient lookup is replaced by `ctx`, with the same
//! inline timing as C++ (no deferral, no admission/try-lock branch).
//! V3.0 only introduces the type and builds it in the `GameLogic::update` drain
//! of `PartitionManager::take_cell_changed_events`, which is its first user.

use crate::object::collide::PartitionManager;
use crate::object::Object;
use crate::object_manager::ObjectManager;
use crate::player::PlayerList;
use crate::system::shroud_manager::ShroudManager;
use std::sync::{Mutex, RwLock};

/// Borrowed services of the world running the current operation.
#[derive(Clone, Copy)]
pub struct ModuleCtx<'a> {
    pub partition: &'a RwLock<PartitionManager>,
    pub shroud: &'a Mutex<ShroudManager>,
    pub objects: &'a RwLock<ObjectManager>,
    pub players: &'a RwLock<PlayerList>,
    pub frame: u32,
    pub unlook_persist_frames: u32,
}

impl ModuleCtx<'_> {
    /// `Object::onPartitionCellChange` for `object` against this ctx's world.
    ///
    /// Locks (blocking, never try-locked; partition, shroud, object index,
    /// players, in that order) are taken per object, so timing never depends on
    /// lock contention, then the unconditional borrowed route runs inline.
    pub fn maintain_partition_cell(&self, object: &mut Object) {
        let mut partition = self.partition.write().unwrap_or_else(|e| e.into_inner());
        let mut shroud = self.shroud.lock().unwrap_or_else(|e| e.into_inner());
        let mut objects = self.objects.write().unwrap_or_else(|e| e.into_inner());
        let players = self.players.read().unwrap_or_else(|e| e.into_inner());
        object.handle_partition_cell_maintenance_borrowed(
            &mut partition,
            &mut shroud,
            &mut objects,
            &players,
            self.frame,
            self.unlook_persist_frames,
        );
    }
}
