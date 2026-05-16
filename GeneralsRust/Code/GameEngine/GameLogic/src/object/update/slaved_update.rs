// SlavedUpdate - Slaved unit(s) remain close to their master
// Author: Matt Campbell, March 2002
// Updated: Kris Morness, July 2002 - Add support for advanced scout drone abilities
// Ported to Rust

use std::sync::{Arc, RwLock};

use crate::common::xfer::XferExt;
use crate::common::ObjectStatus;
use crate::common::{
    AsciiString, Bool, CommandSourceType, Coord3D, DisabledType, Int, ModelConditionFlag, ObjectID,
    ObjectStatusMaskType, Real, Relationship, UnsignedInt, WeaponBonusConditionType,
    FROM_BOUNDING_SPHERE_2D, FROM_CENTER_3D, INVALID_ID, LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{
    TheAudio, TheGameLogic, TheParticleSystemManager, ThePartitionManager, TheTerrainLogic,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, BodyModuleInterfaceExt, SlavedUpdateInterface,
    StealthControllerExt, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const SLAVED_UPDATE_RATE: Int = (LOGICFRAMES_PER_SECOND / 4) as Int;
const STRAY_MULTIPLIER: Real = 2.0;
const CLOSE_ENOUGH: Real = 15.0;
const CLOSE_ENOUGH_SQR: Real = CLOSE_ENOUGH * CLOSE_ENOUGH;

#[derive(Debug, Clone)]
pub struct SlavedUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    pub guard_max_range: Int,
    pub guard_wander_range: Int,
    pub attack_range: Int,
    pub attack_wander_range: Int,
    pub scout_range: Int,
    pub scout_wander_range: Int,
    pub dist_to_target_to_grant_range_bonus: Int,
    pub repair_range: Int,
    pub repair_min_altitude: Real,
    pub repair_max_altitude: Real,
    pub repair_rate_per_second: Real,
    pub repair_when_health_below_percentage: Int,
    pub min_ready_frames: UnsignedInt,
    pub max_ready_frames: UnsignedInt,
    pub min_weld_frames: UnsignedInt,
    pub max_weld_frames: UnsignedInt,
    pub welding_sys_name: AsciiString,
    pub welding_fx_bone: AsciiString,
    pub stay_on_same_layer_as_master: Bool,
}

impl Default for SlavedUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            guard_max_range: 0,
            guard_wander_range: 0,
            attack_range: 0,
            attack_wander_range: 0,
            scout_range: 0,
            scout_wander_range: 0,
            dist_to_target_to_grant_range_bonus: 0,
            repair_range: 0,
            repair_min_altitude: 0.0,
            repair_max_altitude: 0.0,
            repair_rate_per_second: 0.0,
            repair_when_health_below_percentage: 0,
            min_ready_frames: 0,
            max_ready_frames: 0,
            min_weld_frames: 0,
            max_weld_frames: 0,
            welding_sys_name: AsciiString::new(),
            welding_fx_bone: AsciiString::new(),
            stay_on_same_layer_as_master: false,
        }
    }
}

impl SlavedUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SLAVED_UPDATE_FIELDS)
    }
}

impl Snapshotable for SlavedUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.guard_max_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.guard_wander_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.attack_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.attack_wander_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.scout_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.scout_wander_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.dist_to_target_to_grant_range_bonus)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.repair_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.repair_min_altitude)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.repair_max_altitude)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.repair_rate_per_second)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.repair_when_health_below_percentage)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.min_ready_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.max_ready_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.min_weld_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.max_weld_frames)
            .map_err(|e| e.to_string())?;
        let mut welding_sys = self.welding_sys_name.to_string();
        xfer.xfer_ascii_string(&mut welding_sys)
            .map_err(|e| e.to_string())?;
        let mut welding_bone = self.welding_fx_bone.to_string();
        xfer.xfer_ascii_string(&mut welding_bone)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.stay_on_same_layer_as_master)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.welding_sys_name = crate::common::AsciiString::from(welding_sys.as_str());
            self.welding_fx_bone = crate::common::AsciiString::from(welding_bone.as_str());
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(SlavedUpdateModuleData, module_tag_name_key);

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_int_field(tokens: &[&str]) -> Result<Int, INIError> {
    let token = first_value_token(tokens)?;
    INI::parse_int(token)
}

fn parse_real_field(tokens: &[&str]) -> Result<Real, INIError> {
    let token = first_value_token(tokens)?;
    INI::parse_real(token)
}

fn parse_duration_field(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = first_value_token(tokens)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_ascii_field(tokens: &[&str]) -> Result<AsciiString, INIError> {
    let token = first_value_token(tokens)?;
    let value = INI::parse_ascii_string(token)?;
    Ok(AsciiString::from(value.as_str()))
}

fn parse_bool_field(tokens: &[&str]) -> Result<Bool, INIError> {
    let token = first_value_token(tokens)?;
    INI::parse_bool(token)
}

fn parse_guard_max_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.guard_max_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_guard_wander_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.guard_wander_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_attack_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attack_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_attack_wander_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.attack_wander_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_scout_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scout_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_scout_wander_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scout_wander_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_repair_range(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repair_range = parse_int_field(tokens)?;
    Ok(())
}

fn parse_repair_min_altitude(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repair_min_altitude = parse_real_field(tokens)?;
    Ok(())
}

fn parse_repair_max_altitude(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repair_max_altitude = parse_real_field(tokens)?;
    Ok(())
}

fn parse_dist_to_target_to_grant_range_bonus(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.dist_to_target_to_grant_range_bonus = parse_int_field(tokens)?;
    Ok(())
}

fn parse_repair_rate_per_second(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repair_rate_per_second = parse_real_field(tokens)?;
    Ok(())
}

fn parse_repair_when_health_below_percentage(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repair_when_health_below_percentage = parse_int_field(tokens)?;
    Ok(())
}

fn parse_repair_min_ready_time(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_ready_frames = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_repair_max_ready_time(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_ready_frames = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_repair_min_weld_time(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_weld_frames = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_repair_max_weld_time(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_weld_frames = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_repair_welding_sys(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.welding_sys_name = parse_ascii_field(tokens)?;
    Ok(())
}

fn parse_repair_welding_fx_bone(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.welding_fx_bone = parse_ascii_field(tokens)?;
    Ok(())
}

fn parse_stay_on_same_layer_as_master(
    _ini: &mut INI,
    data: &mut SlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.stay_on_same_layer_as_master = parse_bool_field(tokens)?;
    Ok(())
}

const SLAVED_UPDATE_FIELDS: &[FieldParse<SlavedUpdateModuleData>] = &[
    FieldParse {
        token: "GuardMaxRange",
        parse: parse_guard_max_range,
    },
    FieldParse {
        token: "GuardWanderRange",
        parse: parse_guard_wander_range,
    },
    FieldParse {
        token: "AttackRange",
        parse: parse_attack_range,
    },
    FieldParse {
        token: "AttackWanderRange",
        parse: parse_attack_wander_range,
    },
    FieldParse {
        token: "ScoutRange",
        parse: parse_scout_range,
    },
    FieldParse {
        token: "ScoutWanderRange",
        parse: parse_scout_wander_range,
    },
    FieldParse {
        token: "RepairRange",
        parse: parse_repair_range,
    },
    FieldParse {
        token: "RepairMinAltitude",
        parse: parse_repair_min_altitude,
    },
    FieldParse {
        token: "RepairMaxAltitude",
        parse: parse_repair_max_altitude,
    },
    FieldParse {
        token: "DistToTargetToGrantRangeBonus",
        parse: parse_dist_to_target_to_grant_range_bonus,
    },
    FieldParse {
        token: "RepairRatePerSecond",
        parse: parse_repair_rate_per_second,
    },
    FieldParse {
        token: "RepairWhenBelowHealth%",
        parse: parse_repair_when_health_below_percentage,
    },
    FieldParse {
        token: "RepairMinReadyTime",
        parse: parse_repair_min_ready_time,
    },
    FieldParse {
        token: "RepairMaxReadyTime",
        parse: parse_repair_max_ready_time,
    },
    FieldParse {
        token: "RepairMinWeldTime",
        parse: parse_repair_min_weld_time,
    },
    FieldParse {
        token: "RepairMaxWeldTime",
        parse: parse_repair_max_weld_time,
    },
    FieldParse {
        token: "RepairWeldingSys",
        parse: parse_repair_welding_sys,
    },
    FieldParse {
        token: "RepairWeldingFXBone",
        parse: parse_repair_welding_fx_bone,
    },
    FieldParse {
        token: "StayOnSameLayerAsMaster",
        parse: parse_stay_on_same_layer_as_master,
    },
];

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepairState {
    None = 0,
    Unpacking = 1,
    Packing = 2,
    Ready = 3,
    Extending = 4,
    Retracting = 5,
    Welding = 6,
}

#[derive(Debug)]
pub struct SlavedUpdate {
    object_id: ObjectID,
    module_data: Arc<SlavedUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    slaver: ObjectID,
    guard_point_offset: Coord3D,
    frames_to_wait: Int,
    repair_state: RepairState,
    repairing: Bool,
}

impl SlavedUpdate {
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<SlavedUpdateModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            object_id,
            module_data,
            next_call_frame_and_phase: 0,
            slaver: INVALID_ID,
            guard_point_offset: Coord3D::ZERO,
            frames_to_wait: 0,
            repair_state: RepairState::None,
            repairing: false,
        })
    }

    fn object_arc(&self) -> Option<Arc<RwLock<GameObject>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    fn master_arc(&self) -> Option<Arc<RwLock<GameObject>>> {
        if self.slaver == INVALID_ID {
            return None;
        }
        TheGameLogic::find_object_by_id(self.slaver)
    }

    fn on_object_created_internal(&mut self) {
        if self.module_data.repair_rate_per_second > 0.0 {
            if let Some(object_arc) = self.object_arc() {
                if let Ok(mut object) = object_arc.write() {
                    object.set_model_condition_state(ModelConditionFlag::Packing);
                }
            }
        }
    }

    fn start_slaved_effects(&mut self, slaver: &GameObject) {
        self.slaver = slaver.get_id();

        let random_direction = crate::GameLogicRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);
        self.guard_point_offset = Coord3D::ZERO;
        self.guard_point_offset.x +=
            self.module_data.guard_max_range as Real * random_direction.cos();
        self.guard_point_offset.y +=
            self.module_data.guard_max_range as Real * random_direction.sin();

        if let Some(object_arc) = self.object_arc() {
            if let Ok(mut object) = object_arc.write() {
                object.set_status(ObjectStatusMaskType::UNSELECTABLE, true);

                if slaver.test_status(ObjectStatus::Stealthed) {
                    if let Some(stealth) = object.get_stealth() {
                        stealth.receive_grant(true, 0, 0);
                    }
                }
            }
        }
    }

    fn stop_slaved_effects(&mut self) {
        self.slaver = INVALID_ID;
        self.guard_point_offset = Coord3D::ZERO;

        if let Some(object_arc) = self.object_arc() {
            if let Ok(mut object) = object_arc.write() {
                object.set_status(ObjectStatusMaskType::UNSELECTABLE, false);
                object.clear_disabled(DisabledType::Held);
            }
        }
    }

    fn do_attack_logic(&mut self, target_id: ObjectID) {
        let Some(me_arc) = self.object_arc() else {
            return;
        };
        let Some(master_arc) = self.master_arc() else {
            return;
        };
        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };

        let (_me_pos, master_pos, target_pos) = {
            let Ok(me) = me_arc.read() else { return };
            let Ok(master) = master_arc.read() else {
                return;
            };
            let Ok(target) = target_arc.read() else {
                return;
            };
            (
                *me.get_position(),
                *master.get_position(),
                *target.get_position(),
            )
        };

        let dist_sqr = {
            let Ok(me) = me_arc.read() else { return };
            ThePartitionManager::get_distance_squared_to_pos(
                &*me,
                &target_pos,
                FROM_BOUNDING_SPHERE_2D,
            )
        };

        let attack_range = self.module_data.attack_range as Real;
        let mut attack_position = if dist_sqr > attack_range * attack_range {
            let mut vector = target_pos - master_pos;
            if vector.length_squared() > 0.0 {
                vector = vector.normalize() * attack_range;
            }
            master_pos + vector
        } else {
            target_pos
        };

        if self.module_data.attack_wander_range > 0 {
            let random_direction =
                crate::GameLogicRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);
            self.guard_point_offset = Coord3D::ZERO;
            self.guard_point_offset.x +=
                self.module_data.attack_wander_range as Real * random_direction.cos();
            self.guard_point_offset.y +=
                self.module_data.attack_wander_range as Real * random_direction.sin();

            attack_position.x += self.guard_point_offset.x;
            attack_position.y += self.guard_point_offset.y;
            if let Some(terrain) = TheTerrainLogic::get() {
                self.guard_point_offset.z =
                    terrain.get_ground_height(attack_position.x, attack_position.y, None);
            }
        }

        if dist_sqr < (self.module_data.dist_to_target_to_grant_range_bonus as Real).powi(2) {
            if let Ok(mut master) = master_arc.write() {
                master.set_weapon_bonus_condition(WeaponBonusConditionType::DroneSpotting);
            }
        }

        let ai = {
            let Ok(me) = me_arc.read() else { return };
            me.get_ai_update_interface()
        };
        if let Some(ai) = ai {
            ai.ai_move_to_position(&attack_position, false, CommandSourceType::FromAi);
        }
    }

    fn do_scout_logic(&mut self, masters_destination: &Coord3D) {
        let Some(me_arc) = self.object_arc() else {
            return;
        };
        let Some(master_arc) = self.master_arc() else {
            return;
        };

        let (master_pos, dist_sqr) = {
            let Ok(master) = master_arc.read() else {
                return;
            };
            let Ok(me) = me_arc.read() else { return };
            let dist_sqr = ThePartitionManager::get_distance_squared_to_pos(
                &*me,
                masters_destination,
                FROM_BOUNDING_SPHERE_2D,
            );
            (*master.get_position(), dist_sqr)
        };

        let scout_range = self.module_data.scout_range as Real;
        let mut scout_position = if dist_sqr > scout_range * scout_range {
            let mut vector = *masters_destination - master_pos;
            if vector.length_squared() > 0.0 {
                vector = vector.normalize() * scout_range;
            }
            master_pos + vector
        } else {
            *masters_destination
        };

        if self.module_data.scout_wander_range > 0 {
            let random_direction =
                crate::GameLogicRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);
            self.guard_point_offset = Coord3D::ZERO;
            self.guard_point_offset.x +=
                self.module_data.scout_wander_range as Real * random_direction.cos();
            self.guard_point_offset.y +=
                self.module_data.scout_wander_range as Real * random_direction.sin();

            scout_position.x += self.guard_point_offset.x;
            scout_position.y += self.guard_point_offset.y;
            if let Some(terrain) = TheTerrainLogic::get() {
                self.guard_point_offset.z =
                    terrain.get_ground_height(scout_position.x, scout_position.y, None);
            }
        }

        let ai = {
            let Ok(me) = me_arc.read() else { return };
            me.get_ai_update_interface()
        };
        if let Some(ai) = ai {
            ai.ai_move_to_position(&scout_position, false, CommandSourceType::FromAi);
        }
    }

    fn do_guard_logic(&mut self, pinned_position: &Coord3D) {
        let Some(me_arc) = self.object_arc() else {
            return;
        };

        let mut target_position = *pinned_position;
        if self.module_data.guard_wander_range > 0 {
            let random_direction =
                crate::GameLogicRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);
            self.guard_point_offset = Coord3D::ZERO;
            self.guard_point_offset.x +=
                self.module_data.guard_max_range as Real * random_direction.cos();
            self.guard_point_offset.y +=
                self.module_data.guard_max_range as Real * random_direction.sin();

            target_position.x += self.guard_point_offset.x;
            target_position.y += self.guard_point_offset.y;
            if let Some(terrain) = TheTerrainLogic::get() {
                self.guard_point_offset.z =
                    terrain.get_ground_height(target_position.x, target_position.y, None);
            }
        }

        let ai = {
            let Ok(me) = me_arc.read() else { return };
            me.get_ai_update_interface()
        };
        if let Some(ai) = ai {
            ai.ai_move_to_position(&target_position, false, CommandSourceType::FromAi);
        }
    }

    fn do_repair_logic(&mut self) {
        let Some(me_arc) = self.object_arc() else {
            return;
        };
        let Some(master_arc) = self.master_arc() else {
            return;
        };

        let (dist_sqr, master_pos, master_body, master_radius) = {
            let Ok(me) = me_arc.read() else { return };
            let Ok(master) = master_arc.read() else {
                return;
            };
            let dist_sqr =
                ThePartitionManager::get_distance_squared(&*me, &*master, FROM_BOUNDING_SPHERE_2D);
            let master_pos = *master.get_position();
            let master_body = master.get_body_module();
            let master_radius = master.get_geometry_info().get_bounding_sphere_radius();
            (dist_sqr, master_pos, master_body, master_radius)
        };

        let close_enough = dist_sqr < 12.0 * 12.0;
        if close_enough {
            match self.repair_state {
                RepairState::None => self.set_repair_state(RepairState::Ready),
                RepairState::Ready | RepairState::Extending => {
                    if self.frames_to_wait == 0 {
                        self.set_repair_state(RepairState::Welding);
                    }
                }
                RepairState::Unpacking | RepairState::Welding | RepairState::Retracting => {
                    if self.frames_to_wait == 0 {
                        self.set_repair_state(RepairState::Ready);
                    }
                }
                _ => {}
            }
        } else {
            self.repairing = false;

            let close_enough_for_z_precision =
                dist_sqr < (master_radius * 2.0) * (master_radius * 2.0);
            if let Ok(me) = me_arc.read() {
                if let Some(ai) = me.get_ai_update_interface() {
                    if let Some(locomotor) = ai.get_cur_locomotor() {
                        if let Ok(mut locomotor_guard) = locomotor.lock() {
                            locomotor_guard.set_precise_z_pos(close_enough_for_z_precision);
                        }
                    }

                    let mut pos = master_pos;
                    let altitude = crate::GameLogicRandomValueReal!(
                        self.module_data.repair_min_altitude,
                        self.module_data.repair_max_altitude
                    );
                    pos.z += altitude;
                    ai.ai_move_to_position(&pos, false, CommandSourceType::FromAi);

                    if self.frames_to_wait == 0 {
                        self.set_repair_state(RepairState::Ready);
                    }
                }
            }
        }

        if close_enough && self.repairing {
            if let Some(body) = master_body {
                let repair_amount =
                    self.module_data.repair_rate_per_second / LOGICFRAMES_PER_SECOND as Real;
                let mut healing_info = DamageInfo::new();
                healing_info.input.amount = repair_amount;
                healing_info.input.damage_type = DamageType::Healing;
                healing_info.input.death_type = DeathType::None;
                healing_info.sync_from_input();
                body.attempt_healing(&mut healing_info);
            }
        }
    }

    fn end_repair(&mut self) {
        if self.repair_state != RepairState::None {
            self.repair_state = RepairState::None;
            self.frames_to_wait = SLAVED_UPDATE_RATE;
            self.repairing = false;
            self.set_repair_model_condition_states(ModelConditionFlag::Packing);
        }

        if let Some(object_arc) = self.object_arc() {
            if let Ok(object) = object_arc.read() {
                if let Some(ai) = object.get_ai_update_interface() {
                    ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    ai.set_ultra_accurate(false);
                    if let Some(locomotor) = ai.get_cur_locomotor() {
                        if let Ok(mut locomotor_guard) = locomotor.lock() {
                            locomotor_guard.set_precise_z_pos(false);
                        }
                    }
                }
            }
        }
    }

    fn set_repair_model_condition_states(&self, flag: ModelConditionFlag) {
        if let Some(object_arc) = self.object_arc() {
            if let Ok(mut object) = object_arc.write() {
                object.clear_model_condition_state(ModelConditionFlag::Packing);
                object.clear_model_condition_state(ModelConditionFlag::Unpacking);
                object.clear_model_condition_state(ModelConditionFlag::FiringB);
                object.clear_model_condition_state(ModelConditionFlag::FiringC);
                object.clear_model_condition_state(ModelConditionFlag::BetweenFiringShotsB);
                object.clear_model_condition_state(ModelConditionFlag::BetweenFiringShotsC);
                object.clear_model_condition_state(ModelConditionFlag::ReloadingB);
                object.clear_model_condition_state(ModelConditionFlag::ReloadingC);
                object.set_model_condition_state(flag);
            }
        }
    }

    fn move_to_new_repair_spot(&mut self) {
        let Some(me_arc) = self.object_arc() else {
            return;
        };
        let Some(master_arc) = self.master_arc() else {
            return;
        };
        if self.module_data.repair_range <= 0 {
            return;
        }

        let master_pos = {
            let Ok(master) = master_arc.read() else {
                return;
            };
            *master.get_position()
        };

        let random_direction = crate::GameLogicRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);
        self.guard_point_offset = master_pos;
        self.guard_point_offset.x += self.module_data.repair_range as Real * random_direction.cos();
        self.guard_point_offset.y += self.module_data.repair_range as Real * random_direction.sin();
        if let Some(terrain) = TheTerrainLogic::get() {
            self.guard_point_offset.z = terrain.get_ground_height(
                self.guard_point_offset.x,
                self.guard_point_offset.y,
                None,
            );
        }
        let altitude = crate::GameLogicRandomValueReal!(
            self.module_data.repair_min_altitude,
            self.module_data.repair_max_altitude
        );
        self.guard_point_offset.z += altitude;

        let ai = {
            let Ok(me) = me_arc.read() else { return };
            me.get_ai_update_interface()
        };
        if let Some(ai) = ai {
            ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
            ai.set_ultra_accurate(true);
            ai.ai_move_to_position(&self.guard_point_offset, false, CommandSourceType::FromAi);
            if let Some(locomotor) = ai.get_cur_locomotor() {
                if let Ok(mut locomotor_guard) = locomotor.lock() {
                    locomotor_guard.set_precise_z_pos(true);
                }
            }
        }
    }

    fn set_repair_state(&mut self, repair_state: RepairState) {
        if repair_state == self.repair_state {
            return;
        }

        match repair_state {
            RepairState::Unpacking => {
                self.set_repair_model_condition_states(ModelConditionFlag::Unpacking);
                self.frames_to_wait = 15;
                self.repair_state = RepairState::Unpacking;
                return;
            }
            RepairState::Packing => {
                self.set_repair_model_condition_states(ModelConditionFlag::Packing);
                self.frames_to_wait = 15;
                self.repair_state = RepairState::Packing;
                return;
            }
            RepairState::Ready => match self.repair_state {
                RepairState::None => {
                    self.set_repair_model_condition_states(ModelConditionFlag::Unpacking);
                    self.repair_state = RepairState::Unpacking;
                    self.frames_to_wait = 15;
                    return;
                }
                RepairState::Welding => {
                    self.repair_state = RepairState::Retracting;
                    self.frames_to_wait = 5;
                    self.set_repair_model_condition_states(ModelConditionFlag::FiringC);
                    self.move_to_new_repair_spot();
                    return;
                }
                _ => {
                    self.repair_state = RepairState::Ready;
                    self.frames_to_wait = crate::GameLogicRandomValue!(
                        self.module_data.min_ready_frames as Int,
                        self.module_data.max_ready_frames as Int
                    );
                    return;
                }
            },
            RepairState::Welding => {
                if self.repair_state == RepairState::Ready {
                    self.repair_state = RepairState::Extending;
                    self.frames_to_wait = 5;
                    self.set_repair_model_condition_states(ModelConditionFlag::FiringB);
                    return;
                }

                self.repair_state = RepairState::Welding;
                self.frames_to_wait = crate::GameLogicRandomValue!(
                    self.module_data.min_weld_frames as Int,
                    self.module_data.max_weld_frames as Int
                );

                if !self.module_data.welding_sys_name.is_empty() {
                    self.spawn_welding_fx();
                }

                if !self.repairing {
                    self.repairing = true;
                }
                return;
            }
            _ => {}
        }
    }

    fn spawn_welding_fx(&self) {
        let Some(object_arc) = self.object_arc() else {
            return;
        };
        let Ok(object) = object_arc.read() else {
            return;
        };
        let Some(drawable) = object.get_drawable() else {
            return;
        };
        let Ok(drawable) = drawable.read() else {
            return;
        };

        let Some(manager) = TheParticleSystemManager::get() else {
            return;
        };

        let particle_id =
            manager.create_particle_system(Some(self.module_data.welding_sys_name.as_str()));
        let Some(particle_id) = particle_id else {
            return;
        };

        let mut pos = *object.get_position();
        let positions =
            drawable.get_pristine_bone_positions(self.module_data.welding_fx_bone.as_str(), 0, 1);
        if let Some(local_pos) = positions.first() {
            pos = *local_pos + *object.get_position();
        }

        manager.set_particle_system_position(particle_id, &pos);

        if let Some(audio) = TheAudio::get() {
            if let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio() {
                let misc_audio = misc_audio.read();
                if !misc_audio.repair_sparks.sound_file.is_empty() {
                    let mut sound = crate::common::audio::AudioEventRts::new(
                        misc_audio.repair_sparks.sound_file.as_str(),
                    );
                    sound.set_position(&(pos.x, pos.y, pos.z));
                    audio.add_audio_event(&sound);
                }
            }
        }
    }

    fn save(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("SlavedUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut slaver = self.slaver;
        let mut guard_point_offset = self.guard_point_offset;
        let mut frames_to_wait = self.frames_to_wait;
        let mut repair_state = self.repair_state as i32;
        let mut repairing = self.repairing;

        xfer_io(xfer.xfer_object_id(&mut slaver), "slaver");
        xfer.xfer_coord3d(&mut guard_point_offset);
        xfer_io(xfer.xfer_i32(&mut frames_to_wait), "frames_to_wait");
        xfer_io(xfer.xfer_i32(&mut repair_state), "repair_state");
        xfer_io(xfer.xfer_bool(&mut repairing), "repairing");

        Ok(())
    }

    fn load(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("SlavedUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

            xfer_io(xfer.xfer_object_id(&mut self.slaver), "slaver");
            xfer.xfer_coord3d(&mut self.guard_point_offset);
            xfer_io(xfer.xfer_i32(&mut self.frames_to_wait), "frames_to_wait");

            let mut repair_state = 0i32;
            xfer_io(xfer.xfer_i32(&mut repair_state), "repair_state");
            self.repair_state = match repair_state {
                1 => RepairState::Unpacking,
                2 => RepairState::Packing,
                3 => RepairState::Ready,
                4 => RepairState::Extending,
                5 => RepairState::Retracting,
                6 => RepairState::Welding,
                _ => RepairState::None,
            };

            xfer_io(xfer.xfer_bool(&mut self.repairing), "repairing");
        }
        Ok(())
    }
}

impl UpdateModuleInterface for SlavedUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.frames_to_wait > 0 {
            self.frames_to_wait -= 1;
        }

        if self.repair_state == RepairState::None {
            if self.frames_to_wait > 0 {
                return UpdateSleepTime::None;
            }
            self.frames_to_wait = SLAVED_UPDATE_RATE;
        }

        if self.slaver == INVALID_ID {
            return UpdateSleepTime::None;
        }

        let Some(me_arc) = self.object_arc() else {
            return UpdateSleepTime::None;
        };
        let Some(master_arc) = self.master_arc() else {
            return UpdateSleepTime::None;
        };

        {
            let Ok(me) = me_arc.read() else {
                return UpdateSleepTime::None;
            };
            let Some(my_ai) = me.get_ai_update_interface() else {
                return UpdateSleepTime::None;
            };
            if my_ai.get_cur_locomotor().is_none() {
                return UpdateSleepTime::None;
            }
        }

        let (
            target_id,
            health_percentage,
            master_position,
            my_ai_idle,
            master_layer,
            master_team,
            my_team,
        ) = {
            let Ok(me) = me_arc.read() else {
                return UpdateSleepTime::None;
            };
            let Ok(master) = master_arc.read() else {
                return UpdateSleepTime::None;
            };

            if master.is_effectively_dead() || master.is_disabled_by_type(DisabledType::Unmanned) {
                drop(master);
                drop(me);
                self.stop_slaved_effects();
                if let Ok(mut me_write) = me_arc.write() {
                    me_write.set_disabled(DisabledType::Unmanned);
                    if let Some(ai) = me_write.get_ai_update_interface() {
                        ai.ai_idle(CommandSourceType::FromAi);
                    }
                }
                return UpdateSleepTime::None;
            }

            let target_id = master
                .get_ai_update_interface()
                .and_then(|ai| ai.get_current_victim());

            let mut health_percentage = 100;
            if self.module_data.repair_rate_per_second > 0.0 {
                if let Some(body) = master.get_body_module() {
                    let health = body.get_health();
                    let max_health = body.get_max_health();
                    health_percentage = ((health / max_health) * 100.0) as Int;
                }
            }

            let my_ai_idle = me
                .get_ai_update_interface()
                .map(|ai| ai.is_idle())
                .unwrap_or(false);

            (
                target_id,
                health_percentage,
                *master.get_position(),
                my_ai_idle,
                master.get_layer(),
                master.get_team(),
                me.get_team(),
            )
        };

        if let (Some(master_team), Some(my_team)) = (master_team, my_team) {
            if let (Ok(master_team_ref), Ok(my_team_ref)) = (master_team.read(), my_team.read()) {
                if master_team_ref.get_relationship(&*my_team_ref) != Relationship::Allies {
                    if let Ok(mut me_write) = me_arc.write() {
                        me_write.defect(Some(master_team.clone()), 0);
                    }
                }
            }
        }

        if self.module_data.stay_on_same_layer_as_master {
            if let Ok(mut me_write) = me_arc.write() {
                me_write.set_layer(master_layer);
            }
        }

        if let Ok(mut master_write) = master_arc.write() {
            master_write.clear_weapon_bonus_condition(WeaponBonusConditionType::DroneSpotting);
        }

        if health_percentage <= self.module_data.repair_when_health_below_percentage {
            self.do_repair_logic();
            return UpdateSleepTime::None;
        }

        if self.module_data.attack_range > 0 {
            if let Some(target_id) = target_id {
                self.end_repair();
                self.do_attack_logic(target_id);
                return UpdateSleepTime::None;
            }
        }

        if self.module_data.scout_range > 0 {
            if let Ok(master) = master_arc.read() {
                if let Some(master_ai) = master.get_ai_update_interface() {
                    if let Some(master_dest) = master_ai.get_path_destination() {
                        let dist_sqr = ThePartitionManager::get_distance_squared_to_pos(
                            &*master,
                            &master_dest,
                            FROM_BOUNDING_SPHERE_2D,
                        );
                        let guard_half = self.module_data.guard_max_range as Real * 0.5;
                        if dist_sqr > guard_half * guard_half {
                            self.end_repair();
                            self.do_scout_logic(&master_dest);
                            return UpdateSleepTime::None;
                        }
                    }
                }
            }
        }

        if health_percentage < 100 {
            self.do_repair_logic();
            return UpdateSleepTime::None;
        }

        let mut pinned_position = master_position;
        pinned_position.x += self.guard_point_offset.x;
        pinned_position.y += self.guard_point_offset.y;
        if let Some(terrain) = TheTerrainLogic::get() {
            self.guard_point_offset.z =
                terrain.get_ground_height(pinned_position.x, pinned_position.y, None);
        }

        if self.module_data.guard_max_range > 0 {
            if my_ai_idle {
                if let Ok(me) = me_arc.read() {
                    let dist_sqr = ThePartitionManager::get_distance_squared_to_pos(
                        &*me,
                        &pinned_position,
                        FROM_CENTER_3D,
                    );
                    if dist_sqr > CLOSE_ENOUGH_SQR {
                        self.end_repair();
                        self.do_guard_logic(&pinned_position);
                    }
                }
            }

            if let (Ok(me), Ok(master)) = (me_arc.read(), master_arc.read()) {
                let dist_sqr =
                    ThePartitionManager::get_distance_squared(&*me, &*master, FROM_CENTER_3D);
                let max_dist = STRAY_MULTIPLIER * self.module_data.guard_max_range as Real;
                if dist_sqr > max_dist * max_dist {
                    self.end_repair();
                    self.do_guard_logic(&pinned_position);
                }
            }
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for SlavedUpdate {
    fn get_module_name(&self) -> &'static str {
        "SlavedUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_object_created_internal();
        Ok(())
    }

    fn get_slaved_update_interface(&mut self) -> Option<&mut dyn SlavedUpdateInterface> {
        Some(self)
    }
}

impl SlavedUpdateInterface for SlavedUpdate {
    fn slaved_update(&mut self, _object_id: ObjectID, _delta_time: Real) {
        let _ = self.update_simple();
    }

    fn slaver_id(&self) -> Option<ObjectID> {
        (self.slaver != INVALID_ID).then_some(self.slaver)
    }

    fn on_enslave(
        &mut self,
        master: &Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let master_guard = master.read().map_err(|_| "slaver lock poisoned")?;
        self.start_slaved_effects(&*master_guard);
        Ok(())
    }

    fn is_self_tasking(&self) -> bool {
        false
    }

    fn on_slaver_die(
        &mut self,
        _damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.stop_slaved_effects();
        Ok(())
    }

    fn on_slaver_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(object_arc) = self.object_arc() {
            if let Ok(object) = object_arc.read() {
                if let Some(ai) = object.get_ai_update_interface() {
                    ai.ai_go_prone(damage_info, CommandSourceType::FromAi);
                }
            }
        }
        Ok(())
    }
}

impl Snapshotable for SlavedUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_version_write(1);
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let mut slaver = self.slaver;
        let mut guard_point_offset = self.guard_point_offset.clone();
        let mut frames_to_wait = self.frames_to_wait;
        let mut repair_state = self.repair_state as i32;
        let mut repairing = self.repairing;

        xfer.xfer_object_id(&mut slaver).map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut guard_point_offset);
        xfer.xfer_i32(&mut frames_to_wait).map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut repair_state).map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut repairing).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if xfer.is_writing() {
            self.save(xfer)?;
        } else {
            self.load(xfer)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes SlavedUpdate through the common Module trait.
pub struct SlavedUpdateModule {
    behavior: SlavedUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SlavedUpdateModuleData>,
}

impl SlavedUpdateModule {
    pub fn new(
        behavior: SlavedUpdate,
        module_name: &AsciiString,
        module_data: Arc<SlavedUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SlavedUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SlavedUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for SlavedUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slaver_id_reports_current_slaver() {
        let data = Arc::new(SlavedUpdateModuleData::default());
        let mut update = SlavedUpdate::new(7, data).expect("slaved update");

        assert_eq!(update.slaver_id(), None);
        update.slaver = 99;
        assert_eq!(update.slaver_id(), Some(99));
    }
}
