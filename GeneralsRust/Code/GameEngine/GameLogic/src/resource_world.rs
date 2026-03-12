use std::sync::{Arc, Mutex, RwLock};

use game_engine::rts::resource_gathering_manager::{ObjectId, ResourceWorld};

use crate::common::Relationship;
use crate::object::Object;
use crate::system::game_logic::{get_game_logic, GameLogic};

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

    fn clone_objects(
        &self,
        query_id: ObjectId,
        dest_id: ObjectId,
    ) -> Option<(Arc<RwLock<Object>>, Arc<RwLock<Object>>)> {
        let logic = self.lock_logic()?;
        let query = logic.find_object_by_id(query_id)?;
        let dest = logic.find_object_by_id(dest_id)?;
        Some((query, dest))
    }

    fn clone_object(&self, id: ObjectId) -> Option<Arc<RwLock<Object>>> {
        self.lock_logic()?.find_object_by_id(id)
    }
}

impl Default for LiveResourceWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceWorld for LiveResourceWorld {
    fn object_exists(&self, id: ObjectId) -> bool {
        self.clone_object(id).is_some()
    }

    fn has_ai(&self, id: ObjectId) -> bool {
        let query = match self.clone_object(id) {
            Some(obj) => obj,
            None => return false,
        };
        let Ok(guard) = query.read() else {
            return false;
        };
        if guard.is_destroyed() {
            return false;
        }
        guard.get_ai_update_interface().is_some()
    }

    fn can_transfer_supplies_at(&self, query_id: ObjectId, dest_id: ObjectId) -> bool {
        if query_id == dest_id {
            return false;
        }

        let (query_arc, dest_arc) = match self.clone_objects(query_id, dest_id) {
            Some(pair) => pair,
            None => return false,
        };

        let query_guard = match query_arc.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let dest_guard = match dest_arc.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if query_guard.is_destroyed() || dest_guard.is_destroyed() {
            return false;
        }

        crate::action_manager::ActionManager::can_transfer_supplies_at(&query_guard, &dest_guard)
    }

    fn is_clear_to_approach(&self, dest_id: ObjectId, query_id: ObjectId) -> bool {
        let (query_arc, dest_arc) = match self.clone_objects(query_id, dest_id) {
            Some(pair) => pair,
            None => return false,
        };

        let dest_guard = match dest_arc.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if let Some(is_clear) = dest_guard.with_dock_update_interface(|dock| {
            dock.is_clear_to_approach(&query_arc).unwrap_or(false)
        }) {
            return is_clear;
        }

        // C++ expects a dock interface for supply transfer targets; if missing, disallow approach.
        false
    }

    fn distance_squared(&self, query_id: ObjectId, dest_id: ObjectId) -> Option<f32> {
        let (query, dest) = self.clone_objects(query_id, dest_id)?;
        let query_guard = query.read().ok()?;
        let dest_guard = dest.read().ok()?;
        Some(crate::helpers::ThePartitionManager::get_distance_squared(
            &query_guard,
            &dest_guard,
            crate::common::FROM_CENTER_3D,
        ))
    }

    fn is_supply_warehouse_dock(&self, dock_id: ObjectId) -> bool {
        let dock = match self.clone_object(dock_id) {
            Some(obj) => obj,
            None => return false,
        };
        let Ok(guard) = dock.read() else {
            return false;
        };
        if guard.is_destroyed() {
            return false;
        }
        guard
            .find_update_module("SupplyWarehouseDockUpdate")
            .is_some()
    }

    fn is_supply_center_dock(&self, dock_id: ObjectId) -> bool {
        let dock = match self.clone_object(dock_id) {
            Some(obj) => obj,
            None => return false,
        };
        let Ok(guard) = dock.read() else {
            return false;
        };
        if guard.is_destroyed() {
            return false;
        }
        guard.find_update_module("SupplyCenterDockUpdate").is_some()
    }

    fn preferred_dock(&self, _query_id: ObjectId) -> Option<ObjectId> {
        let query = self.clone_object(_query_id)?;
        let query_guard = query.read().ok()?;
        let ai = query_guard.get_ai_update_interface()?;
        let ai_guard = ai.lock().ok()?;
        let supply_truck = ai_guard.get_supply_truck_ai_interface()?;
        supply_truck.get_preferred_dock_id()
    }

    fn warehouse_scan_distance(&self, _query_id: ObjectId) -> Option<f32> {
        let query = self.clone_object(_query_id)?;
        let query_guard = query.read().ok()?;
        let ai = query_guard.get_ai_update_interface()?;
        let ai_guard = ai.lock().ok()?;
        let supply_truck = ai_guard.get_supply_truck_ai_interface()?;

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
    }
}
