use std::sync::{Arc, Mutex, RwLock};

use game_engine::rts::resource_gathering_manager::{ObjectId, ResourceWorld};

use crate::common::Relationship;
use crate::object::Object;
use crate::system::game_logic::{GameLogic, get_game_logic};

/// Default implementation of `ResourceWorld` backed by the live `GameLogic` singleton.
///
/// This adapter lets common systems (written in the shared `game_engine` crate)
/// query world state without taking a direct dependency on the heavy GameLogic
/// structures.  It mirrors the behaviour of the C++ helper layer that routed
/// resource lookups through `TheGameLogic`, `TheActionManager`, and
/// `ThePartitionManager`.
#[derive(Clone, Copy)]
pub struct LiveResourceWorld {
    logic: &'static Mutex<GameLogic>,
}

impl LiveResourceWorld {
    /// Create a new adapter using the global `GameLogic` instance.
    pub fn new() -> Self {
        Self {
            logic: get_game_logic(),
        }
    }

    fn lock_logic(&self) -> Option<std::sync::MutexGuard<'_, GameLogic>> {
        self.logic.lock().ok()
    }

impl Default for LiveResourceWorldimpl Default for LiveResourceWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceWorld for LiveResourceWorld {
    fn object_exists(&self, id: ObjectId) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, |_| ())
            .is_some()
    }

    fn has_ai(&self, id: ObjectId) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, |guard| {
                if guard.is_destroyed() {
                    return false;
                }
                guard.get_ai_update_interface().is_some()
            })
            .unwrap_or(false)
    }

    fn can_transfer_supplies_at(&self, query_id: ObjectId, dest_id: ObjectId) -> bool {
        if query_id == dest_id {
            return false;
        }

        crate::object::registry::OBJECT_REGISTRY
            .with_object(query_id, |query_guard| {
                if query_guard.is_destroyed() {
                    return false;
                }
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(dest_id, |dest_guard| {
                        if dest_guard.is_destroyed() {
                            return false;
                        }
                        crate::action_manager::ActionManager::can_transfer_supplies_at(
                            query_guard,
                            dest_guard,
                        )
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    fn is_clear_to_approach(&self, dest_id: ObjectId, query_id: ObjectId) -> bool {
        let _ = query_id;
        crate::object::registry::OBJECT_REGISTRY
            .with_object(dest_id, |dest_guard| {
                if let Some(is_clear) = dest_guard.with_dock_update_interface(|dock| {
                    dock.is_clear_to_approach(query_id as crate::common::ObjectID)
                        .unwrap_or(false)
                }) {
                    return is_clear;
                }
                false
            })
            .unwrap_or(false)
    }

    fn distance_squared(&self, query_id: ObjectId, dest_id: ObjectId) -> Option<f32> {
        crate::object::registry::OBJECT_REGISTRY.with_object(query_id, |query_guard| {
            crate::object::registry::OBJECT_REGISTRY.with_object(dest_id, |dest_guard| {
                crate::helpers::ThePartitionManager::get_distance_squared(
                    query_guard,
                    dest_guard,
                    crate::common::FROM_CENTER_3D,
                )
            })
        })?
    }

    fn is_supply_warehouse_dock(&self, dock_id: ObjectId) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(dock_id, |guard| {
                if guard.is_destroyed() {
                    return false;
                }
                guard.find_update_module("SupplyWarehouseDockUpdate").is_some()
            })
            .unwrap_or(false)
    }

    fn is_supply_center_dock(&self, dock_id: ObjectId) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(dock_id, |guard| {
                if guard.is_destroyed() {
                    return false;
                }
                guard.find_update_module("SupplyCenterDockUpdate").is_some()
            })
            .unwrap_or(false)
    }

    fn preferred_dock(&self, _query_id: ObjectId) -> Option<ObjectId> {
        crate::object::registry::OBJECT_REGISTRY.with_object(_query_id, |query_guard| {
            let ai = query_guard.get_ai_update_interface()?;
            let supply_truck = ai.get_supply_truck_ai_interface()?;
            supply_truck.get_preferred_dock_id()
        })?
    }

    fn warehouse_scan_distance(&self, _query_id: ObjectId) -> Option<f32> {
        crate::object::registry::OBJECT_REGISTRY.with_object(_query_id, |query_guard| {
        let ai = query_guard.get_ai_update_interface()?;
        let supply_truck = ai.get_supply_truck_ai_interface()?;

        let is_ai_player = query_guard
            .get_controlling_player_id()
            .and_then(|player_id| {
                let Ok(list) = crate::player::ThePlayerList().read() else {
                    return None;
                };
                list.get_player(player_id as i32).cloned()
            })
            .and_then(|player| player.read().ok().map(|guard| guard.is_skirmish_ai()))
            .unwrap_or(false);

        supply_truck.get_warehouse_scan_distance(is_ai_player)
        })?
    }
}
