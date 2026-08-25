//! Common `ObjectDataProvider` backed by GameLogic objects.
//!
//! Installed during `GameLogic::init` / `reset` so Common ActionManager
//! queries read `OBJECT_REGISTRY` / `GameLogic.objects` instead of the
//! fail-closed dead/immobile/neutral defaults.

use crate::common::{INVALID_ID, KindOf, ObjectID, ObjectStatusTypes, Relationship};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{PlayerType, player_list};
use game_engine::common::rts::action_manager::{
    ObjectDataProvider, ObjectShroudStatus, PlayerType as CommonPlayerType, kind_of_bit,
    set_object_data_provider,
};
use game_engine::common::rts::handles::ObjectHandle;
use std::sync::{Arc, RwLock};

/// Maps a C++ `KindOfType` bit index (Common `kind_of_bit::*`) onto GameLogic `KindOf`.
/// Unknown bits fail closed (`None` → `is_kind_of` false).
fn kind_of_from_cpp_bit(kind_of: u32) -> Option<KindOf> {
    match kind_of {
        kind_of_bit::OBSTACLE => Some(KindOf::Obstacle),
        kind_of_bit::SELECTABLE => Some(KindOf::Selectable),
        kind_of_bit::IMMOBILE => Some(KindOf::Immobile),
        kind_of_bit::CAN_ATTACK => Some(KindOf::CanAttack),
        kind_of_bit::STRUCTURE => Some(KindOf::Structure),
        kind_of_bit::INFANTRY => Some(KindOf::Infantry),
        kind_of_bit::VEHICLE => Some(KindOf::Vehicle),
        kind_of_bit::AIRCRAFT => Some(KindOf::Aircraft),
        kind_of_bit::DOZER => Some(KindOf::Dozer),
        kind_of_bit::HARVESTER => Some(KindOf::Harvester),
        kind_of_bit::TRANSPORT => Some(KindOf::Transport),
        kind_of_bit::BRIDGE => Some(KindOf::Bridge),
        kind_of_bit::BRIDGE_TOWER => Some(KindOf::BridgeTower),
        kind_of_bit::REPAIR_PAD => Some(KindOf::RepairPad),
        kind_of_bit::HEAL_PAD => Some(KindOf::HealPad),
        kind_of_bit::REBUILD_HOLE => Some(KindOf::RebuildHole),
        kind_of_bit::FS_AIRFIELD => Some(KindOf::FSAirfield),
        _ => None,
    }
}

fn object_id(handle: ObjectHandle) -> Option<ObjectID> {
    let id = handle.value();
    if id == INVALID_ID { None } else { Some(id) }
}

fn object_arc(handle: ObjectHandle) -> Option<Arc<RwLock<Object>>> {
    OBJECT_REGISTRY.get_object(object_id(handle)?)
}

fn with_object<R>(handle: ObjectHandle, f: impl FnOnce(&Object) -> R) -> Option<R> {
    let arc = object_arc(handle)?;
    let guard = arc.read().ok()?;
    Some(f(&guard))
}

fn convert_shroud(status: crate::common::ObjectShroudStatus) -> ObjectShroudStatus {
    match status {
        crate::common::ObjectShroudStatus::Invalid => ObjectShroudStatus::Invalid,
        crate::common::ObjectShroudStatus::Clear => ObjectShroudStatus::Clear,
        crate::common::ObjectShroudStatus::PartialClear => ObjectShroudStatus::PartialClear,
        crate::common::ObjectShroudStatus::Fogged => ObjectShroudStatus::Fogged,
        crate::common::ObjectShroudStatus::Shrouded => ObjectShroudStatus::Shrouded,
        crate::common::ObjectShroudStatus::InvalidButPreviousValid => ObjectShroudStatus::Invalid,
    }
}

fn convert_player_type(player_type: PlayerType) -> CommonPlayerType {
    match player_type {
        PlayerType::Human => CommonPlayerType::Human,
        _ => CommonPlayerType::Computer,
    }
}

fn warehouse_boxes(object: &Object) -> u32 {
    object
        .find_update_module("SupplyWarehouseDockUpdate")
        .and_then(|module| {
            module.with_module(|module| {
                module
                    .get_supply_warehouse_dock_interface()
                    .map(|warehouse| warehouse.boxes_stored().max(0) as u32)
            })
        })
        .unwrap_or(0)
}

/// GameLogic-backed provider for Common ActionManager object queries.
pub struct GameLogicObjectDataProvider;

impl ObjectDataProvider for GameLogicObjectDataProvider {
    fn is_valid_object(&self, id: ObjectHandle) -> bool {
        object_arc(id).is_some()
    }

    fn get_relationship(&self, source: ObjectHandle, target: ObjectHandle) -> Relationship {
        let Some(source_arc) = object_arc(source) else {
            return Relationship::Neutral;
        };
        let Some(target_arc) = object_arc(target) else {
            return Relationship::Neutral;
        };
        let relationship = match (source_arc.read(), target_arc.read()) {
            (Ok(source_obj), Ok(target_obj)) => source_obj.relationship_to(&target_obj),
            _ => Relationship::Neutral,
        };
        relationship
    }

    fn get_team_relationship(&self, source: ObjectHandle, player_id: u32) -> Relationship {
        with_object(source, |obj| {
            let Some(team) = obj.get_team() else {
                return Relationship::Neutral;
            };
            let Ok(team_guard) = team.read() else {
                return Relationship::Neutral;
            };
            team_guard.get_relationship_with_player(player_id as crate::common::Int)
        })
        .unwrap_or(Relationship::Neutral)
    }

    fn is_effectively_dead(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| obj.is_effectively_dead()).unwrap_or(true)
    }

    fn is_mobile(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            !obj.is_kind_of(KindOf::Immobile) && !obj.is_disabled()
        })
        .unwrap_or(false)
    }

    fn test_status(&self, id: ObjectHandle, status_bit: u32) -> bool {
        with_object(id, |obj| {
            obj.test_status(ObjectStatusTypes::from_u32(status_bit))
        })
        .unwrap_or(false)
    }

    fn is_kind_of(&self, id: ObjectHandle, kind_of: u32) -> bool {
        let Some(kind) = kind_of_from_cpp_bit(kind_of) else {
            return false;
        };
        with_object(id, |obj| obj.is_kind_of(kind)).unwrap_or(false)
    }

    fn is_above_terrain(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| obj.is_above_terrain()).unwrap_or(false)
    }

    fn get_health(&self, id: ObjectHandle) -> f32 {
        with_object(id, |obj| obj.get_health()).unwrap_or(100.0)
    }

    fn get_max_health(&self, id: ObjectHandle) -> f32 {
        with_object(id, |obj| obj.get_max_health()).unwrap_or(100.0)
    }

    fn get_controlling_player_id(&self, id: ObjectHandle) -> Option<u32> {
        with_object(id, |obj| obj.get_controlling_player_id()).flatten()
    }

    fn get_controlling_player_type(&self, id: ObjectHandle) -> CommonPlayerType {
        with_object(id, |obj| {
            let Some(player) = obj.get_controlling_player() else {
                return CommonPlayerType::Human;
            };
            player
                .read()
                .ok()
                .map(|guard| convert_player_type(guard.get_player_type()))
                .unwrap_or(CommonPlayerType::Human)
        })
        .unwrap_or(CommonPlayerType::Human)
    }

    fn get_shrouded_status(
        &self,
        target: ObjectHandle,
        viewer_player_id: u32,
    ) -> ObjectShroudStatus {
        with_object(target, |obj| {
            convert_shroud(obj.get_shrouded_status(viewer_player_id as i32))
        })
        .unwrap_or(ObjectShroudStatus::Clear)
    }

    fn has_supply_truck_ai(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            let Some(ai) = obj.get_ai_update_interface() else {
                return false;
            };
            ai.lock()
                .ok()
                .is_some_and(|guard| guard.get_supply_truck_ai_interface().is_some())
        })
        .unwrap_or(false)
    }

    fn is_supply_warehouse(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            obj.find_update_module("SupplyWarehouseDockUpdate")
                .is_some()
        })
        .unwrap_or(false)
    }

    fn is_supply_center(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            obj.find_update_module("SupplyCenterDockUpdate").is_some()
        })
        .unwrap_or(false)
    }

    fn get_warehouse_boxes(&self, id: ObjectHandle) -> u32 {
        with_object(id, warehouse_boxes).unwrap_or(0)
    }

    fn get_supply_boxes(&self, id: ObjectHandle) -> u32 {
        with_object(id, |obj| {
            let Some(ai) = obj.get_ai_update_interface() else {
                return 0;
            };
            ai.lock()
                .ok()
                .and_then(|guard| {
                    guard
                        .get_supply_truck_ai_interface()
                        .map(|truck| truck.get_number_boxes().max(0) as u32)
                })
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    fn is_available_for_supplying(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            let Some(ai) = obj.get_ai_update_interface() else {
                return false;
            };
            ai.lock()
                .ok()
                .and_then(|guard| {
                    guard
                        .get_supply_truck_ai_interface()
                        .map(|truck| truck.is_available_for_supplying())
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn has_dock_update_interface(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| obj.with_dock_update_interface(|_| ()).is_some()).unwrap_or(false)
    }

    fn is_railed_transport_dock(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            obj.find_update_module("RailedTransportDockUpdate")
                .is_some()
                || obj
                    .with_railed_transport_dock_update_interface(|_| ())
                    .is_some()
        })
        .unwrap_or(false)
    }

    fn is_contained(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| obj.is_contained()).unwrap_or(false)
    }

    fn is_surrendered(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| {
            let Some(ai) = obj.get_ai_update_interface() else {
                return false;
            };
            ai.lock().ok().is_some_and(|guard| guard.is_surrendered())
        })
        .unwrap_or(false)
    }

    fn get_surrendered_player_index(&self, id: ObjectHandle) -> Option<u32> {
        with_object(id, |obj| {
            let ai = obj.get_ai_update_interface()?;
            ai.lock()
                .ok()
                .and_then(|guard| guard.get_surrendered_player_index().map(|idx| idx as u32))
        })
        .flatten()
    }

    fn has_contain_module(&self, id: ObjectHandle) -> bool {
        with_object(id, |obj| obj.get_contain().is_some()).unwrap_or(false)
    }

    fn get_apparent_controlling_player(
        &self,
        id: ObjectHandle,
        viewer_player_id: u32,
    ) -> Option<u32> {
        with_object(id, |obj| {
            let contain = obj.get_contain()?;
            let contain_guard = contain.lock().ok()?;
            let list = player_list().read().ok()?;
            let viewer = list.get_player(viewer_player_id as crate::player::PlayerIndex)?;
            let viewer_guard = viewer.read().ok()?;
            let apparent = contain_guard.get_apparent_controlling_player(Some(&viewer_guard))?;
            apparent
                .read()
                .ok()
                .map(|player| player.get_player_index() as u32)
        })
        .flatten()
    }
}

/// Install the GameLogic-backed Common ActionManager object provider.
///
/// Safe to call more than once (init + reset); later calls replace the Arc.
pub fn install_object_data_provider() {
    set_object_data_provider(Arc::new(GameLogicObjectDataProvider));
}

/// Install on first use if GameLogic init has not run yet.
pub fn ensure_object_data_provider() {
    if !game_engine::common::rts::action_manager::object_data_provider_is_set() {
        install_object_data_provider();
    }
}
