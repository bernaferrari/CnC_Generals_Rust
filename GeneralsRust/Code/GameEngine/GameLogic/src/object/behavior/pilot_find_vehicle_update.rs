//! PilotFindVehicleUpdate - Ejected pilot finds and enters vehicle
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::ai::integration::{with_ai_integration, IntegratedAiPlayer};
use crate::common::{CommandSourceType, Coord3D, KindOf, ModuleData, ObjectID, Real, UnsignedInt};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::Object as GameObject;
use crate::player::PlayerType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct PilotFindVehicleUpdateModuleData {
    pub base: BehaviorModuleData,
    pub scan_rate: UnsignedInt,
    pub scan_range: Real,
    pub min_health: Real,
}

impl Default for PilotFindVehicleUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            scan_rate: 0,
            scan_range: 0.0,
            min_health: 0.5,
        }
    }
}

crate::impl_behavior_module_data_via_base!(PilotFindVehicleUpdateModuleData, base);

impl PilotFindVehicleUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PILOT_FIND_VEHICLE_UPDATE_FIELDS)
    }
}

fn parse_scan_rate(
    _ini: &mut INI,
    data: &mut PilotFindVehicleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.scan_rate = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_scan_range(
    _ini: &mut INI,
    data: &mut PilotFindVehicleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.scan_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_min_health(
    _ini: &mut INI,
    data: &mut PilotFindVehicleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.min_health = INI::parse_real(token)?;
    Ok(())
}

const PILOT_FIND_VEHICLE_UPDATE_FIELDS: &[FieldParse<PilotFindVehicleUpdateModuleData>] = &[
    FieldParse {
        token: "ScanRate",
        parse: parse_scan_rate,
    },
    FieldParse {
        token: "ScanRange",
        parse: parse_scan_range,
    },
    FieldParse {
        token: "MinHealth",
        parse: parse_min_health,
    },
];

pub struct PilotFindVehicleUpdate {
    object_id: ObjectID,
    module_data: Arc<PilotFindVehicleUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    did_move_to_base: bool,
}

impl PilotFindVehicleUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<PilotFindVehicleUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            did_move_to_base: false,
        })
    }

    fn owner_base_center(owner: &GameObject) -> Option<Coord3D> {
        let player_id = owner.get_controlling_player_id()? as u32;
        with_ai_integration(|manager| {
            manager.with_ai_player(player_id, |ai_player| match ai_player {
                IntegratedAiPlayer::Standard(player) => player.get_base_center(),
                IntegratedAiPlayer::Skirmish(player) => player.get_base_center(),
            })
        })
        .flatten()
        .flatten()
    }
}

impl UpdateModuleInterface for PilotFindVehicleUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(owner_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return UpdateSleepTime::Forever;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        if owner_guard.is_destroyed() || owner_guard.get_container_id().is_some() {
            return UpdateSleepTime::Forever;
        }

        let is_human = owner_guard
            .get_controlling_player()
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_type()))
            == Some(PlayerType::Human);
        if is_human {
            return UpdateSleepTime::Forever;
        }

        let Some(ai) = owner_guard.get_ai() else {
            return UpdateSleepTime::Forever;
        };
        if let Ok(ai_guard) = ai.lock() {
            if !ai_guard.is_idle() {
                return UpdateSleepTime::from_u32(self.module_data.scan_rate);
            }
        }

        let owner_id = owner_guard.get_id();
        let owner_pos = *owner_guard.get_position();

        let object_ids = ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&owner_pos, self.module_data.scan_range))
            .unwrap_or_default();

        let mut best_target = None;
        let mut best_dist_sqr = Real::MAX;

        for obj_id in object_ids {
            if obj_id == owner_id {
                continue;
            }

            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.is_destroyed() || !obj_guard.is_kind_of(KindOf::Vehicle) {
                continue;
            }

            if owner_guard.get_relationship_to(&obj_guard) != ObjectRelationship::Ally {
                continue;
            }

            let Some(body) = obj_guard.get_body_module() else {
                continue;
            };
            let Ok(body_guard) = body.lock() else {
                continue;
            };
            if body_guard.get_health() < body_guard.get_max_health() * self.module_data.min_health {
                continue;
            }
            drop(body_guard);

            let Some(contain_arc) = obj_guard.get_contain() else {
                continue;
            };
            let Ok(contain_guard) = contain_arc.lock() else {
                continue;
            };
            if contain_guard.get_contained_count() >= contain_guard.get_max_capacity() {
                continue;
            }

            let pos = obj_guard.get_position();
            let dx = pos.x - owner_pos.x;
            let dy = pos.y - owner_pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if dist_sqr < best_dist_sqr {
                best_dist_sqr = dist_sqr;
                best_target = Some(obj_id);
            }
        }

        if let Some(target_id) = best_target {
            ai.ai_enter(target_id, CommandSourceType::FromAi);
            self.did_move_to_base = false;
        } else if !self.did_move_to_base {
            if let Some(base_center) = Self::owner_base_center(&owner_guard) {
                ai.ai_move_to_position(&base_center, false, CommandSourceType::FromAi);
                self.did_move_to_base = true;
            }
        }

        UpdateSleepTime::from_u32(self.module_data.scan_rate)
    }
}

impl BehaviorModuleInterface for PilotFindVehicleUpdate {
    fn get_module_name(&self) -> &'static str {
        "PilotFindVehicleUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for PilotFindVehicleUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("PilotFindVehicleUpdate xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_bool(&mut self.did_move_to_base)
            .map_err(|e| format!("PilotFindVehicleUpdate xfer did_move_to_base: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct PilotFindVehicleUpdateFactory;
impl PilotFindVehicleUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(PilotFindVehicleUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_find_vehicle_defaults_match_cpp_constructor() {
        let data = PilotFindVehicleUpdateModuleData::default();

        assert_eq!(data.scan_rate, 0);
        assert_eq!(data.scan_range, 0.0);
        assert_eq!(data.min_health, 0.5);
    }

    #[test]
    fn pilot_find_vehicle_fields_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = PilotFindVehicleUpdateModuleData::default();

        parse_scan_rate(&mut ini, &mut data, &["=", "1s"]).unwrap();
        parse_scan_range(&mut ini, &mut data, &["=", "275.5"]).unwrap();
        parse_min_health(&mut ini, &mut data, &["=", "0.25"]).unwrap();

        assert_eq!(data.scan_rate, 30);
        assert_eq!(data.scan_range, 275.5);
        assert_eq!(data.min_health, 0.25);
    }

    #[test]
    fn pilot_find_vehicle_rejects_missing_values_like_cpp_parsers() {
        let mut ini = INI::new();
        let mut data = PilotFindVehicleUpdateModuleData::default();

        assert!(matches!(
            parse_scan_rate(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_scan_range(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_min_health(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
    }
}
