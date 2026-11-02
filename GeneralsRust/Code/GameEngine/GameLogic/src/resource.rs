//! Supply-center / warehouse lookups routed through the controlling player's
//! owned `ResourceGatheringManager`. There is no process-wide resource service.

use std::collections::HashMap;

use crate::common::{Coord3D, ObjectID, Real};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player;

type ObjectId = u32;

fn with_owner_resource_manager<R>(
    object_id: ObjectId,
    f: impl FnOnce(&mut crate::supply_system::ResourceGatheringManager, ObjectID, Coord3D) -> R,
) -> Option<R> {
    let object = OBJECT_REGISTRY.get_object(object_id)?;
    let (player_id, position) = {
        let guard = object.read().ok()?;
        let player_id = guard.get_controlling_player_id()?;
        let position = *guard.get_position();
        (player_id, position)
    };
    player::with_player_mut(player_id as i32, |player| {
        let manager = player.get_resource_manager_mut()?;
        Some(f(manager, object_id, position))
    })?
}

fn dock_maps(
    query_id: ObjectID,
    ids: &[ObjectID],
) -> (HashMap<ObjectID, Coord3D>, HashMap<ObjectID, bool>) {
    let mut positions = HashMap::new();
    let mut available = HashMap::new();
    for &id in ids {
        let Some(object) = OBJECT_REGISTRY.get_object(id) else {
            available.insert(id, false);
            continue;
        };
        let Ok(guard) = object.read() else {
            available.insert(id, false);
            continue;
        };
        if guard.is_destroyed() {
            available.insert(id, false);
            continue;
        }
        positions.insert(id, *guard.get_position());
        let is_clear = guard
            .with_dock_update_interface(|dock| {
                dock.is_clear_to_approach(query_id).unwrap_or(true)
            })
            .unwrap_or(true);
        available.insert(id, is_clear);
    }
    (positions, available)
}

fn preferred_dock_and_scan(query_id: ObjectId) -> (Option<ObjectID>, Real) {
    let Some(object) = OBJECT_REGISTRY.get_object(query_id) else {
        return (None, Real::MAX);
    };
    let Ok(guard) = object.read() else {
        return (None, Real::MAX);
    };
    let is_ai_player = guard
        .get_controlling_player_id()
        .and_then(|player_id| {
            player::with_player(player_id as i32, |player| player.is_skirmish_ai())
        })
        .unwrap_or(false);
    let Some(ai) = guard.get_ai_update_interface() else {
        return (None, Real::MAX);
    };
    let Ok(ai_guard) = ai.lock() else {
        return (None, Real::MAX);
    };
    let Some(truck) = ai_guard.get_supply_truck_ai_interface() else {
        return (None, Real::MAX);
    };
    let preferred = truck.get_preferred_dock_id();
    let scan = truck
        .get_warehouse_scan_distance(is_ai_player)
        .unwrap_or(Real::MAX);
    (preferred, scan)
}

pub fn add_supply_center(center_id: ObjectId) {
    let _ = with_owner_resource_manager(center_id, |manager, id, _| {
        manager.add_supply_center(id);
    });
}

pub fn remove_supply_center(center_id: ObjectId) {
    let _ = with_owner_resource_manager(center_id, |manager, id, _| {
        manager.remove_supply_center(id);
    });
}

pub fn add_supply_warehouse(warehouse_id: ObjectId) {
    let _ = with_owner_resource_manager(warehouse_id, |manager, id, _| {
        manager.add_supply_warehouse(id);
    });
}

pub fn remove_supply_warehouse(warehouse_id: ObjectId) {
    let _ = with_owner_resource_manager(warehouse_id, |manager, id, _| {
        manager.remove_supply_warehouse(id);
    });
}

pub fn find_best_supply_warehouse(query_id: ObjectId) -> Option<ObjectId> {
    let (preferred, max_distance) = preferred_dock_and_scan(query_id);
    with_owner_resource_manager(query_id, |manager, _, truck_position| {
        let warehouses = manager.get_supply_warehouses().to_vec();
        let (positions, available) = dock_maps(query_id, &warehouses);
        manager.find_best_supply_warehouse(
            &truck_position,
            preferred,
            max_distance,
            &positions,
            &available,
        )
    })
    .flatten()
}

pub fn find_best_supply_center(query_id: ObjectId) -> Option<ObjectId> {
    let (preferred, _) = preferred_dock_and_scan(query_id);
    with_owner_resource_manager(query_id, |manager, _, truck_position| {
        let centers = manager.get_supply_centers().to_vec();
        let (positions, available) = dock_maps(query_id, &centers);
        manager.find_best_supply_center(&truck_position, preferred, &positions, &available)
    })
    .flatten()
}

pub fn reset() {
    // Per-player managers are cleared with their owning Player/session reset.
    // Kept as a no-op so leftover call sites compile without a process store.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Player, player_list};
    use std::sync::{Arc, Barrier, Mutex, RwLock};
    use std::thread;

    fn admit_player(index: i32) -> Arc<RwLock<Player>> {
        let player = Arc::new(RwLock::new(Player::new(index)));
        player.write().unwrap().init_from_dict_defaults();
        assert!(player.read().unwrap().get_resource_manager().is_some());
        player_list().write().unwrap().add_player(Arc::clone(&player));
        player
    }

    #[test]
    fn parallel_players_keep_independent_supply_lists() {
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for (index, center_id) in [(0i32, 100u32), (1, 101)] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                // Each thread uses its own Player owner; no process resource service.
                let player = Arc::new(RwLock::new(Player::new(index)));
                player.write().unwrap().init_from_dict_defaults();
                barrier.wait();
                {
                    let mut guard = player.write().unwrap();
                    let mgr = guard.get_resource_manager_mut().expect("manager");
                    mgr.add_supply_center(center_id);
                    mgr.add_supply_warehouse(center_id + 100);
                }
                // Mutate after peer may have written its own manager.
                barrier.wait();
                let centers = player
                    .read()
                    .unwrap()
                    .get_resource_manager()
                    .unwrap()
                    .get_supply_centers()
                    .to_vec();
                results.lock().unwrap().push((index, centers));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(i, _)| *i);
        assert_eq!(got[0].1, vec![100]);
        assert_eq!(got[1].1, vec![101]);
        assert_ne!(got[0].1, got[1].1);
        let _ = admit_player;
    }
}
