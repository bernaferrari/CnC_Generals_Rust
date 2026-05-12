//! JetAIUpdate module data + minimal runtime hooks.
//!
//! Ported from GameLogic/Module/JetAIUpdate.h and
//! GameLogic/Object/Update/AIUpdate/JetAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, RwLock};

use crate::ai::states::AICommandParmsStorage;
use crate::ai::{AiCommandParams, AiCommandType};
use crate::common::audio::AudioEventRts;
use crate::common::SECONDS_PER_LOGICFRAME_REAL;
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, KindOf, LocomotorSetType, Matrix3D,
    ModelConditionFlags, ObjectID, ObjectStatusTypes, Real, UnsignedInt, INVALID_ID,
    WEAPONSLOT_COUNT,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheAudio, TheGameClient, TheGameLogic, TheThingFactory};
use crate::modules::AIUpdateInterface;
use crate::modules::BodyModuleInterfaceExt;
use crate::object::behavior::behavior_module::PPInfo;
use crate::object::drawable::DrawableArcExt;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::terrain::get_terrain_logic;
use crate::waypoint::Waypoint;
use crate::weapon::{WeaponReloadType, WeaponSlotType, WeaponStatus};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const LOCOMOTOR_SET_NAMES: &[&str] = &[
    "SET_NORMAL",
    "SET_NORMAL_UPGRADED",
    "SET_FREEFALL",
    "SET_WANDER",
    "SET_PANIC",
    "SET_TAXIING",
    "SET_SUPERSONIC",
    "SET_SLUGGISH",
];

/// Module data for JetAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct JetAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub out_of_ammo_damage_per_second: Real,
    pub needs_runway: Bool,
    pub keeps_parking_space_when_airborne: Bool,
    pub takeoff_dist_for_max_lift: Real,
    pub takeoff_pause: UnsignedInt,
    pub min_height: Real,
    pub parking_offset: Real,
    pub sneaky_offset_when_attacking: Real,
    pub attacking_loco: LocomotorSetType,
    pub attack_loco_persist_time: UnsignedInt,
    pub attackers_miss_persist_time: UnsignedInt,
    pub returning_loco: LocomotorSetType,
    pub lockon_time: UnsignedInt,
    pub lockon_cursor: AsciiString,
    pub lockon_initial_dist: Real,
    pub lockon_freq: Real,
    pub lockon_angle_spin: Real,
    pub lockon_blinky: Bool,
    pub return_to_base_idle_time: UnsignedInt,
}

impl Default for JetAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            out_of_ammo_damage_per_second: 0.0,
            needs_runway: true,
            keeps_parking_space_when_airborne: true,
            takeoff_dist_for_max_lift: 0.0,
            takeoff_pause: 0,
            min_height: 0.0,
            parking_offset: 0.0,
            sneaky_offset_when_attacking: 0.0,
            attacking_loco: LocomotorSetType::Normal,
            attack_loco_persist_time: 0,
            attackers_miss_persist_time: 0,
            returning_loco: LocomotorSetType::Normal,
            lockon_time: 0,
            lockon_cursor: AsciiString::new(),
            lockon_initial_dist: 100.0,
            lockon_freq: 0.5,
            lockon_angle_spin: 720.0,
            lockon_blinky: false,
            return_to_base_idle_time: 0,
        }
    }
}

impl JetAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, JET_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for JetAIUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn is_ai_module_data(&self) -> bool {
        true
    }
}

impl Snapshotable for JetAIUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_real(&mut self.out_of_ammo_damage_per_second))?;
        xfer_io(xfer.xfer_bool(&mut self.needs_runway))?;
        xfer_io(xfer.xfer_bool(&mut self.keeps_parking_space_when_airborne))?;
        xfer_io(xfer.xfer_real(&mut self.takeoff_dist_for_max_lift))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.takeoff_pause))?;
        xfer_io(xfer.xfer_real(&mut self.min_height))?;
        xfer_io(xfer.xfer_real(&mut self.parking_offset))?;
        xfer_io(xfer.xfer_real(&mut self.sneaky_offset_when_attacking))?;
        let mut attacking_loco = self.attacking_loco as i32;
        xfer_io(xfer.xfer_int(&mut attacking_loco))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.attack_loco_persist_time))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.attackers_miss_persist_time))?;
        let mut returning_loco = self.returning_loco as i32;
        xfer_io(xfer.xfer_int(&mut returning_loco))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.lockon_time))?;
        xfer_io(xfer.xfer_ascii_string(self.lockon_cursor.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_initial_dist))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_freq))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_angle_spin))?;
        xfer_io(xfer.xfer_bool(&mut self.lockon_blinky))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.return_to_base_idle_time))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_percent_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_locomotor_set(
    setter: &mut dyn FnMut(LocomotorSetType),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let index =
        INI::parse_index_list(token, LOCOMOTOR_SET_NAMES).map_err(|_| INIError::InvalidData)?;
    let set = match index {
        0 => LocomotorSetType::Normal,
        1 => LocomotorSetType::NormalUpgraded,
        2 => LocomotorSetType::Freefall,
        3 => LocomotorSetType::Wander,
        4 => LocomotorSetType::Panic,
        5 => LocomotorSetType::Taxiing,
        6 => LocomotorSetType::Supersonic,
        7 => LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };
    setter(set);
    Ok(())
}

fn parse_angle_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_angle_real(token)?);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const JET_AI_UPDATE_FIELDS: &[FieldParse<JetAIUpdateModuleData>] = &[
    FieldParse {
        token: "OutOfAmmoDamagePerSecond",
        parse: |_, data, tokens| {
            parse_percent_field(
                &mut |value| data.out_of_ammo_damage_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "NeedsRunway",
        parse: |_, data, tokens| parse_bool_field(&mut |value| data.needs_runway = value, tokens),
    },
    FieldParse {
        token: "KeepsParkingSpaceWhenAirborne",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.keeps_parking_space_when_airborne = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "TakeoffDistForMaxLift",
        parse: |_, data, tokens| {
            parse_percent_field(&mut |value| data.takeoff_dist_for_max_lift = value, tokens)
        },
    },
    FieldParse {
        token: "TakeoffPause",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.takeoff_pause = value, tokens)
        },
    },
    FieldParse {
        token: "MinHeight",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.min_height = value, tokens),
    },
    FieldParse {
        token: "ParkingOffset",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.parking_offset = value, tokens),
    },
    FieldParse {
        token: "SneakyOffsetWhenAttacking",
        parse: |_, data, tokens| {
            parse_real_field(
                &mut |value| data.sneaky_offset_when_attacking = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "AttackLocomotorType",
        parse: |_, data, tokens| {
            parse_locomotor_set(&mut |value| data.attacking_loco = value, tokens)
        },
    },
    FieldParse {
        token: "AttackLocomotorPersistTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.attack_loco_persist_time = value, tokens)
        },
    },
    FieldParse {
        token: "AttackersMissPersistTime",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.attackers_miss_persist_time = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "ReturnForAmmoLocomotorType",
        parse: |_, data, tokens| {
            parse_locomotor_set(&mut |value| data.returning_loco = value, tokens)
        },
    },
    FieldParse {
        token: "LockonTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.lockon_time = value, tokens)
        },
    },
    FieldParse {
        token: "LockonCursor",
        parse: |_, data, tokens| {
            let token = required_value(tokens)?;
            data.lockon_cursor = AsciiString::from(token);
            Ok(())
        },
    },
    FieldParse {
        token: "LockonInitialDist",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.lockon_initial_dist = value, tokens)
        },
    },
    FieldParse {
        token: "LockonFreq",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.lockon_freq = value, tokens),
    },
    FieldParse {
        token: "LockonAngleSpin",
        parse: |_, data, tokens| {
            parse_angle_field(&mut |value| data.lockon_angle_spin = value, tokens)
        },
    },
    FieldParse {
        token: "LockonBlinky",
        parse: |_, data, tokens| parse_bool_field(&mut |value| data.lockon_blinky = value, tokens),
    },
    FieldParse {
        token: "ReturnToBaseIdleTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.return_to_base_idle_time = value, tokens)
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JetFlag {
    HasPendingCommand = 0,
    AllowAirLoco = 1,
    HasProducerLocation = 2,
    TakeoffInProgress = 3,
    LandingInProgress = 4,
    UseSpecialReturnLoco = 5,
    AllowCircling = 6,
    AllowInterruptAndResumeOfCurStateForReload = 7,
    TaxiInProgress = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JetAIStateType {
    ReturningForLanding,
    TakingOffAwaitClearance,
    TaxiToTakeoff,
    PauseBeforeTakeoff,
    TakingOff,
    LandingAwaitClearance,
    Landing,
    TaxiFromLanding,
    TaxiFromHangar,
    OrientForParkingPlace,
    ReloadAmmo,
    ReturnToDeadAirfield,
    CirclingDeadAirfield,
}

struct JetStateMachine {
    state: Option<JetAIStateType>,
    needs_runway: bool,
    pause_until: UnsignedInt,
    pause_transfer: UnsignedInt,
    reload_time: UnsignedInt,
    reload_done_frame: UnsignedInt,
    circling_check_frame: UnsignedInt,
    landing_sound_played: bool,
    takeoff_max_lift: Real,
    takeoff_max_speed: Real,
}

impl JetStateMachine {
    fn new(needs_runway: bool) -> Self {
        Self {
            state: None,
            needs_runway,
            pause_until: 0,
            pause_transfer: 0,
            reload_time: 0,
            reload_done_frame: 0,
            circling_check_frame: 0,
            landing_sound_played: false,
            takeoff_max_lift: 0.0,
            takeoff_max_speed: 0.0,
        }
    }

    fn set_state(
        &mut self,
        state: JetAIStateType,
        ai: &mut dyn AIUpdateInterface,
        jet_ai: &mut JetAIUpdate,
    ) {
        if let Some(prev) = self.state {
            self.on_exit(prev, ai, jet_ai);
        }
        self.state = Some(state);
        self.on_enter(ai, jet_ai);
    }

    fn clear(&mut self, ai: &mut dyn AIUpdateInterface, jet_ai: &mut JetAIUpdate) {
        if let Some(prev) = self.state {
            self.on_exit(prev, ai, jet_ai);
        }
        self.state = None;
    }

    fn update(&mut self, ai: &mut dyn AIUpdateInterface, jet_ai: &mut JetAIUpdate) {
        let Some(state) = self.state else {
            return;
        };
        let status = self.on_update(state, ai, jet_ai);
        if status.is_success() {
            if let Some(next) = self.next_state_success(state) {
                self.set_state(next, ai, jet_ai);
            } else {
                self.on_exit(state, ai, jet_ai);
                self.state = None;
            }
        } else if status.is_failure() {
            if let Some(next) = self.next_state_failure(state) {
                self.set_state(next, ai, jet_ai);
            } else {
                self.on_exit(state, ai, jet_ai);
                self.state = None;
            }
        }
    }

    fn next_state_success(&self, state: JetAIStateType) -> Option<JetAIStateType> {
        if self.needs_runway {
            match state {
                JetAIStateType::ReturningForLanding => Some(JetAIStateType::LandingAwaitClearance),
                JetAIStateType::TakingOffAwaitClearance => Some(JetAIStateType::TaxiToTakeoff),
                JetAIStateType::TaxiToTakeoff => Some(JetAIStateType::PauseBeforeTakeoff),
                JetAIStateType::PauseBeforeTakeoff => Some(JetAIStateType::TakingOff),
                JetAIStateType::TakingOff => None,
                JetAIStateType::LandingAwaitClearance => Some(JetAIStateType::Landing),
                JetAIStateType::Landing => Some(JetAIStateType::TaxiFromLanding),
                JetAIStateType::TaxiFromLanding => Some(JetAIStateType::OrientForParkingPlace),
                JetAIStateType::TaxiFromHangar => Some(JetAIStateType::OrientForParkingPlace),
                JetAIStateType::OrientForParkingPlace => Some(JetAIStateType::ReloadAmmo),
                JetAIStateType::ReloadAmmo => None,
                JetAIStateType::ReturnToDeadAirfield => Some(JetAIStateType::CirclingDeadAirfield),
                JetAIStateType::CirclingDeadAirfield => None,
            }
        } else {
            match state {
                JetAIStateType::ReturningForLanding => Some(JetAIStateType::LandingAwaitClearance),
                JetAIStateType::TakingOffAwaitClearance => Some(JetAIStateType::TakingOff),
                JetAIStateType::TakingOff => None,
                JetAIStateType::LandingAwaitClearance => {
                    Some(JetAIStateType::OrientForParkingPlace)
                }
                JetAIStateType::OrientForParkingPlace => Some(JetAIStateType::Landing),
                JetAIStateType::Landing => Some(JetAIStateType::ReloadAmmo),
                JetAIStateType::ReloadAmmo => None,
                JetAIStateType::ReturnToDeadAirfield => Some(JetAIStateType::CirclingDeadAirfield),
                JetAIStateType::CirclingDeadAirfield => None,
                JetAIStateType::TaxiFromHangar => None,
                JetAIStateType::TaxiToTakeoff
                | JetAIStateType::PauseBeforeTakeoff
                | JetAIStateType::TaxiFromLanding => None,
            }
        }
    }

    fn next_state_failure(&self, state: JetAIStateType) -> Option<JetAIStateType> {
        if self.needs_runway {
            match state {
                JetAIStateType::ReturningForLanding => Some(JetAIStateType::ReturnToDeadAirfield),
                JetAIStateType::ReturnToDeadAirfield => Some(JetAIStateType::ReturnToDeadAirfield),
                _ => None,
            }
        } else {
            match state {
                JetAIStateType::ReturningForLanding => Some(JetAIStateType::ReturnToDeadAirfield),
                JetAIStateType::ReturnToDeadAirfield => Some(JetAIStateType::ReturnToDeadAirfield),
                _ => None,
            }
        }
    }

    fn on_enter(&mut self, ai: &mut dyn AIUpdateInterface, jet_ai: &mut JetAIUpdate) {
        let assume = self.state;
        match assume {
            Some(JetAIStateType::TakingOffAwaitClearance) => {
                jet_ai.set_takeoff_in_progress(true);
                jet_ai.set_landing_in_progress(false);
                jet_ai.set_allow_circling(true);
            }
            Some(JetAIStateType::LandingAwaitClearance) => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(true);
                jet_ai.set_allow_circling(true);
            }
            Some(JetAIStateType::TaxiToTakeoff)
            | Some(JetAIStateType::TaxiFromLanding)
            | Some(JetAIStateType::TaxiFromHangar) => {
                jet_ai.set_takeoff_in_progress(matches!(
                    assume,
                    Some(JetAIStateType::TaxiToTakeoff) | Some(JetAIStateType::TaxiFromHangar)
                ));
                jet_ai.set_landing_in_progress(matches!(
                    assume,
                    Some(JetAIStateType::TaxiFromLanding)
                ));
                jet_ai.set_taxi_in_progress(true);
                jet_ai.set_allow_air_loco(false);
                let _ = ai.set_can_path_through_units(true);
                let _ = ai.choose_locomotor_set(LocomotorSetType::Taxiing);
            }
            Some(JetAIStateType::PauseBeforeTakeoff) => {
                jet_ai.set_takeoff_in_progress(true);
                jet_ai.set_landing_in_progress(false);
                let now = TheGameLogic::get_frame();
                let pause = jet_ai.data.takeoff_pause.max(1);
                self.pause_until = now.saturating_add(pause);
                self.pause_transfer = now.saturating_add(1);
                if let Some(obj) = jet_ai.get_object() {
                    if let Ok(mut guard) = obj.write() {
                        jet_ai.friend_enable_afterburners(&mut guard, true);
                    }
                }
            }
            Some(JetAIStateType::TakingOff) | Some(JetAIStateType::Landing) => {
                let landing = matches!(assume, Some(JetAIStateType::Landing));
                jet_ai.set_takeoff_in_progress(!landing);
                jet_ai.set_landing_in_progress(landing);
                jet_ai.set_allow_air_loco(true);
                let _ = ai.choose_locomotor_set(LocomotorSetType::Normal);
                if let Some(loco) = ai.get_cur_locomotor() {
                    if let Ok(mut loco_guard) = loco.lock() {
                        loco_guard.set_max_lift(99999.0);
                        let max_speed = loco_guard.get_max_speed_for_condition(
                            crate::locomotor::core::BodyDamageType::Pristine,
                        );
                        self.takeoff_max_lift = loco_guard
                            .get_max_lift(crate::locomotor::core::BodyDamageType::Pristine);
                        self.takeoff_max_speed = max_speed;
                        if landing {
                            let min_speed = loco_guard.template.min_speed;
                            loco_guard.set_max_speed(min_speed);
                        } else {
                            loco_guard.set_max_lift(0.0);
                        }
                        loco_guard.set_precise_z_pos(true);
                        loco_guard.set_ultra_accurate(true);
                    }
                }
                let producer = jet_ai.producer_object();
                let _ = ai.ignore_obstacle(producer.as_ref());
            }
            Some(JetAIStateType::OrientForParkingPlace) => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(true);
                let producer = jet_ai.producer_object();
                let _ = ai.ignore_obstacle(producer.as_ref());
            }
            Some(JetAIStateType::ReloadAmmo) => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
                jet_ai.set_use_special_return_loco(false);
                self.reload_time = 0;
                if let Some(obj) = jet_ai.get_object() {
                    if let Ok(guard) = obj.read() {
                        for slot_index in 0..WEAPONSLOT_COUNT {
                            let slot = match slot_index {
                                0 => crate::weapon::WeaponSlotType::Primary,
                                1 => crate::weapon::WeaponSlotType::Secondary,
                                _ => crate::weapon::WeaponSlotType::Tertiary,
                            };
                            let Some(weapon) = guard.get_weapon_in_weapon_slot(slot) else {
                                continue;
                            };
                            let remaining = weapon.get_remaining_ammo();
                            let clip_size = weapon.get_template().clip_size.max(0) as u32;
                            let mut reload_time =
                                weapon.get_clip_reload_time(guard.get_id()).max(0) as u32;
                            if clip_size > 0 {
                                let needed = clip_size.saturating_sub(remaining);
                                reload_time = reload_time.saturating_mul(needed) / clip_size.max(1);
                            }
                            if reload_time > self.reload_time {
                                self.reload_time = reload_time;
                            }
                        }
                    }
                }
                if self.reload_time < 1 {
                    self.reload_time = 1;
                }
                self.reload_done_frame = TheGameLogic::get_frame().saturating_add(self.reload_time);
            }
            Some(JetAIStateType::ReturningForLanding) => {
                let _ = ai.set_adjusts_destination(false);
            }
            Some(JetAIStateType::ReturnToDeadAirfield) => {
                let _ = ai.set_adjusts_destination(true);
            }
            Some(JetAIStateType::CirclingDeadAirfield) => {
                self.circling_check_frame =
                    TheGameLogic::get_frame().saturating_add(crate::common::LOGICFRAMES_PER_SECOND);
                if let Some(obj) = jet_ai.get_object() {
                    if let Ok(guard) = obj.read() {
                        if let Some(mut sound) =
                            guard.get_template().get_per_unit_sound("VoiceLowFuel")
                        {
                            sound.set_object_id(jet_ai.object_id);
                            if let Some(audio) = TheAudio::get() {
                                audio.add_audio_event(&sound);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_exit(
        &mut self,
        state: JetAIStateType,
        ai: &mut dyn AIUpdateInterface,
        jet_ai: &mut JetAIUpdate,
    ) {
        match state {
            JetAIStateType::TakingOffAwaitClearance | JetAIStateType::LandingAwaitClearance => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
                jet_ai.set_allow_circling(false);
            }
            JetAIStateType::TaxiToTakeoff
            | JetAIStateType::TaxiFromLanding
            | JetAIStateType::TaxiFromHangar => {
                if let Some(loco) = ai.get_cur_locomotor() {
                    if let Ok(mut guard) = loco.lock() {
                        guard.set_precise_z_pos(false);
                        guard.set_ultra_accurate(false);
                        guard.set_allow_invalid_position(false);
                    }
                }
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
                jet_ai.set_taxi_in_progress(false);
                let _ = ai.set_can_path_through_units(false);
            }
            JetAIStateType::PauseBeforeTakeoff => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
            }
            JetAIStateType::TakingOff | JetAIStateType::Landing => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
                if let Some(obj) = jet_ai.get_object() {
                    if let Ok(mut guard) = obj.write() {
                        jet_ai.friend_enable_afterburners(&mut guard, false);
                        if let Some(loco) = ai.get_cur_locomotor() {
                            if let Ok(mut loco_guard) = loco.lock() {
                                loco_guard.set_precise_z_pos(false);
                                loco_guard.set_ultra_accurate(false);
                                if !guard.is_effectively_dead() && self.takeoff_max_lift > 0.0 {
                                    loco_guard.set_max_lift(self.takeoff_max_lift);
                                }
                                if self.takeoff_max_speed > 0.0 {
                                    loco_guard.set_max_speed(self.takeoff_max_speed);
                                }
                            }
                        }
                    }
                }
                let _ = ai.ignore_obstacle(None);
                if matches!(state, JetAIStateType::Landing) {
                    jet_ai.set_allow_air_loco(false);
                    let _ = ai.choose_locomotor_set(LocomotorSetType::Taxiing);
                } else if !jet_ai.keeps_parking_space_when_airborne() {
                    let _ = jet_ai.with_producer_parking_place(|pp| {
                        pp.release_space(jet_ai.object_id);
                    });
                }
                let _ = jet_ai.with_producer_parking_place(|pp| {
                    pp.release_runway(jet_ai.object_id);
                });
            }
            JetAIStateType::OrientForParkingPlace => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
                let _ = ai.ignore_obstacle(None);
            }
            JetAIStateType::ReloadAmmo => {
                jet_ai.set_takeoff_in_progress(false);
                jet_ai.set_landing_in_progress(false);
            }
            _ => {}
        }
    }

    fn on_update(
        &mut self,
        state: JetAIStateType,
        ai: &mut dyn AIUpdateInterface,
        jet_ai: &mut JetAIUpdate,
    ) -> crate::state_machine::StateReturnType {
        use crate::state_machine::StateReturnType;
        let Some(obj) = jet_ai.get_object() else {
            return StateReturnType::Failure;
        };
        if let Ok(guard) = obj.read() {
            if guard.is_effectively_dead() {
                return StateReturnType::Failure;
            }
        }

        match state {
            JetAIStateType::TakingOffAwaitClearance | JetAIStateType::LandingAwaitClearance => {
                if !self.needs_runway {
                    return StateReturnType::Success;
                }
                let landing = state == JetAIStateType::LandingAwaitClearance;
                let mut take_taxi_path: Option<Vec<Coord3D>> = None;
                let Some(result) = jet_ai.with_producer_parking_place(|pp| {
                    let mut info = PPInfo::default();
                    if !pp.reserve_space(jet_ai.object_id, jet_ai.data.parking_offset, &mut info) {
                        return StateReturnType::Failure;
                    }
                    if pp.reserve_runway(jet_ai.object_id, landing) {
                        return StateReturnType::Success;
                    }
                    if let Some(obj) = jet_ai.get_object() {
                        if let Ok(guard) = obj.read() {
                            if guard.test_status(ObjectStatusTypes::DeckHeightOffset) && !landing {
                                let mut best_pos = Coord3D::ZERO;
                                if pp.calc_best_parking_assignment(
                                    jet_ai.object_id,
                                    &mut best_pos,
                                    None,
                                    None,
                                ) {
                                    let mut path = Vec::new();
                                    if let Ok(guard) = obj.read() {
                                        path.push(*guard.get_position());
                                    }
                                    path.push(best_pos);
                                    take_taxi_path = Some(path);
                                }
                            }
                        }
                    }
                    StateReturnType::Continue
                }) else {
                    return StateReturnType::Success;
                };
                if let Some(path) = take_taxi_path.take() {
                    jet_ai.set_taxi_in_progress(true);
                    jet_ai.set_allow_air_loco(false);
                    let _ = ai.choose_locomotor_set(LocomotorSetType::Taxiing);
                    let _ = ai.set_allow_invalid_position(true);
                    let _ = ai.set_ultra_accurate(true);
                    let _ = ai.set_precise_z_pos(true);
                    let producer = jet_ai.producer_object();
                    let _ = ai.ignore_obstacle(producer.as_ref());
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowPath,
                        crate::ai::CommandSourceType::FromAi,
                    );
                    params.coords = path;
                    let _ = ai.execute_command(&params);
                }

                ai.set_locomotor_goal_none();
                result
            }
            JetAIStateType::TaxiToTakeoff
            | JetAIStateType::TaxiFromLanding
            | JetAIStateType::TaxiFromHangar => {
                let taxi_mode = state;
                let Some(result) = jet_ai.with_producer_parking_place(|pp| {
                    let mut info = PPInfo::default();
                    if !pp.reserve_space(jet_ai.object_id, jet_ai.data.parking_offset, &mut info) {
                        return StateReturnType::Failure;
                    }
                    let mut path = Vec::new();
                    if let Ok(guard) = obj.read() {
                        path.push(*guard.get_position());
                    }
                    let is_deck = obj
                        .read()
                        .ok()
                        .map(|guard| guard.test_status(ObjectStatusTypes::DeckHeightOffset))
                        .unwrap_or(false);
                    if taxi_mode == JetAIStateType::TaxiFromLanding {
                        if is_deck {
                            if info.runway_start != info.runway_prep {
                                path.push(info.runway_start);
                            }
                        } else {
                            path.push(info.runway_prep);
                            path.push(info.runway_start);
                        }
                    } else if taxi_mode == JetAIStateType::TaxiToTakeoff {
                        if is_deck {
                            path.push(info.runway_start);
                        } else {
                            path.push(info.runway_prep);
                            path.push(info.runway_start);
                        }
                    } else {
                        // TaxiFromHangar
                        if is_deck {
                            path.push(info.runway_prep);
                        } else {
                            path.push(info.parking_space);
                        }
                    }
                    let _ = ai.set_allow_invalid_position(true);
                    let _ = ai.set_ultra_accurate(true);
                    let _ = ai.set_precise_z_pos(true);
                    let producer = jet_ai.producer_object();
                    let _ = ai.ignore_obstacle(producer.as_ref());
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowPath,
                        crate::ai::CommandSourceType::FromAi,
                    );
                    params.coords = path;
                    let _ = ai.execute_command(&params);
                    if matches!(
                        taxi_mode,
                        JetAIStateType::TaxiFromLanding | JetAIStateType::TaxiFromHangar
                    ) {
                        let mut best_pos = Coord3D::ZERO;
                        if pp.calc_best_parking_assignment(
                            jet_ai.object_id,
                            &mut best_pos,
                            None,
                            None,
                        ) {
                            let _ = ai.append_goal_position_to_path(&best_pos);
                        }
                    }
                    StateReturnType::Continue
                }) else {
                    return StateReturnType::Success;
                };
                if matches!(result, StateReturnType::Failure) {
                    return result;
                }
                if ai.is_idle() {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::PauseBeforeTakeoff => {
                let now = TheGameLogic::get_frame();
                let _ = jet_ai.with_producer_parking_place(|pp| {
                    if now >= self.pause_transfer {
                        pp.transfer_runway_reservation_to_next_in_line_for_takeoff(
                            jet_ai.object_id,
                        );
                    }
                });
                if now >= self.pause_until {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::TakingOff | JetAIStateType::Landing => {
                let landing = state == JetAIStateType::Landing;
                if let Some(result) = jet_ai.with_producer_parking_place(|pp| {
                    let mut info = PPInfo::default();
                    if !pp.reserve_space(jet_ai.object_id, jet_ai.data.parking_offset, &mut info) {
                        return StateReturnType::Failure;
                    }
                    if landing && !pp.reserve_runway(jet_ai.object_id, true) {
                        return StateReturnType::Failure;
                    }
                    let mut path = Vec::new();
                    if landing {
                        path.push(info.runway_approach);
                        path.push(info.runway_landing_start);
                        path.push(info.runway_landing_end);
                    } else {
                        let mut end = info.runway_end;
                        end.z = info.runway_approach.z;
                        path.push(end);
                        path.push(info.runway_exit);
                    }
                    let mut params = AiCommandParams::new(
                        AiCommandType::FollowPath,
                        crate::ai::CommandSourceType::FromAi,
                    );
                    params.coords = path;
                    let _ = ai.execute_command(&params);
                    StateReturnType::Continue
                }) {
                    if matches!(result, StateReturnType::Failure) {
                        return result;
                    }
                }

                if landing {
                    let _ = jet_ai.with_producer_parking_place(|_pp| {
                        let z_pos = obj.read().ok().map(|g| g.get_position().z).unwrap_or(0.0);
                        let ground = obj.read().ok().map(|g| g.get_position().z).unwrap_or(0.0);
                        if z_pos <= ground + 0.25 && !self.landing_sound_played {
                            if let Some(audio) = TheAudio::get() {
                                if let Some(misc_audio) =
                                    game_engine::common::ini::ini_misc_audio::get_misc_audio()
                                {
                                    let misc_audio = misc_audio.read();
                                    let mut sound = AudioEventRts::new(
                                        misc_audio.aircraft_wheel_screech.sound_file.clone(),
                                    );
                                    sound.set_object_id(jet_ai.object_id);
                                    audio.add_audio_event(&sound);
                                }
                            }
                            self.landing_sound_played = true;
                        }
                    });
                } else {
                    let _ = jet_ai.with_producer_parking_place(|pp| {
                        pp.transfer_runway_reservation_to_next_in_line_for_takeoff(
                            jet_ai.object_id,
                        );
                        let mut info = PPInfo::default();
                        pp.calc_pp_info(jet_ai.object_id, &mut info);
                        if info.runway_takeoff_dist > 0.0 {
                            if let Some(obj) = jet_ai.get_object() {
                                if let Ok(guard) = obj.read() {
                                    let vector = info.runway_end - *guard.get_position();
                                    let dist = vector.length();
                                    let mut ratio = 1.0 - (dist / info.runway_takeoff_dist);
                                    ratio *= ratio;
                                    if ratio < 0.0 {
                                        ratio = 0.0;
                                    }
                                    if ratio > 1.0 {
                                        ratio = 1.0;
                                    }
                                    if let Some(loco) = ai.get_cur_locomotor() {
                                        if let Ok(mut loco_guard) = loco.lock() {
                                            if self.takeoff_max_lift > 0.0 {
                                                loco_guard
                                                    .set_max_lift(self.takeoff_max_lift * ratio);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                }

                if ai.is_idle() {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::OrientForParkingPlace => {
                let Some(result) = jet_ai.with_producer_parking_place(|pp| {
                    let mut info = PPInfo::default();
                    if !pp.reserve_space(jet_ai.object_id, jet_ai.data.parking_offset, &mut info) {
                        return StateReturnType::Failure;
                    }
                    if let Ok(mut guard) = obj.write() {
                        if let Err(err) = guard.set_orientation(info.parking_orientation) {
                            log::debug!(
                                "JetAIMachine::OrientForParkingPlace set_orientation failed for {}: {}",
                                guard.get_id(),
                                err
                            );
                        }
                    }
                    StateReturnType::Success
                }) else {
                    return StateReturnType::Failure;
                };
                result
            }
            JetAIStateType::ReloadAmmo => {
                let now = TheGameLogic::get_frame();
                let mut all_done = true;
                if let Ok(mut guard) = obj.write() {
                    for slot_index in 0..WEAPONSLOT_COUNT {
                        let slot = match slot_index {
                            0 => crate::weapon::WeaponSlotType::Primary,
                            1 => crate::weapon::WeaponSlotType::Secondary,
                            _ => crate::weapon::WeaponSlotType::Tertiary,
                        };
                        let Some(weapon) = guard.get_weapon_in_slot_mut(slot) else {
                            continue;
                        };
                        if now >= self.reload_done_frame {
                            weapon.set_clip_percent_full(1.0, false);
                        } else {
                            let remaining = (self.reload_time - (self.reload_done_frame - now))
                                as Real
                                / self.reload_time.max(1) as Real;
                            weapon.set_clip_percent_full(remaining, false);
                        }
                        if weapon.get_remaining_ammo()
                            != weapon.get_template().clip_size.max(0) as u32
                        {
                            all_done = false;
                        }
                    }
                }
                if all_done {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::ReturningForLanding => {
                if let Some(result) = jet_ai.with_producer_parking_place(|pp| {
                    let mut goal = Coord3D::ZERO;
                    if let Some(obj) = jet_ai.get_object() {
                        if let Ok(guard) = obj.read() {
                            if guard.is_kind_of(KindOf::ProducedAtHelipad) {
                                goal = jet_ai.landing_pos_for_helipad;
                            } else {
                                let mut info = PPInfo::default();
                                if !pp.reserve_space(
                                    jet_ai.object_id,
                                    jet_ai.data.parking_offset,
                                    &mut info,
                                ) {
                                    return StateReturnType::Failure;
                                }
                                goal = if self.needs_runway {
                                    info.runway_approach
                                } else {
                                    info.parking_space
                                };
                            }
                        }
                    }
                    let _ = ai.ai_move_to_position(&goal);
                    StateReturnType::Continue
                }) {
                    if matches!(result, StateReturnType::Failure) {
                        return result;
                    }
                } else {
                    // try to find another airfield
                    if let Some(new_airfield) = jet_ai.find_suitable_airfield() {
                        if let Some(obj) = jet_ai.get_object() {
                            if let Ok(mut guard) = obj.write() {
                                if let Some(new_airfield_obj) =
                                    TheGameLogic::find_object_by_id(new_airfield)
                                {
                                    if let Ok(new_airfield_guard) = new_airfield_obj.read() {
                                        guard.set_producer(Some(&*new_airfield_guard));
                                    }
                                }
                            }
                        }
                    } else {
                        return StateReturnType::Failure;
                    }
                }
                if ai.is_idle() {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::ReturnToDeadAirfield => {
                if let Ok(_guard) = obj.read() {
                    let goal = jet_ai.producer_location;
                    let _ = ai.ai_move_to_position(&goal);
                }
                if ai.is_idle() {
                    return StateReturnType::Success;
                }
                StateReturnType::Continue
            }
            JetAIStateType::CirclingDeadAirfield => {
                if !jet_ai.is_out_of_special_reload_ammo() {
                    if let Ok(guard) = obj.read() {
                        if guard.get_producer_id() == INVALID_ID {
                            return StateReturnType::Failure;
                        }
                    }
                }
                ai.set_locomotor_goal_none();
                let damage_rate = jet_ai.data.out_of_ammo_damage_per_second;
                if damage_rate > 0.0 {
                    if let Ok(mut guard) = obj.write() {
                        if let Some(body) = guard.get_body_module() {
                            if let Ok(body_guard) = body.lock() {
                                let max_health = body_guard.get_max_health();
                                let amount = damage_rate * SECONDS_PER_LOGICFRAME_REAL * max_health;
                                let mut damage = DamageInfo::with_simple(
                                    amount,
                                    INVALID_ID,
                                    DamageType::Unresistable,
                                    DeathType::Normal,
                                );
                                let _ = guard.attempt_damage(&mut damage);
                            }
                        }
                    }
                }
                let now = TheGameLogic::get_frame();
                if now >= self.circling_check_frame {
                    self.circling_check_frame =
                        now.saturating_add(crate::common::LOGICFRAMES_PER_SECOND);
                    if let Some(new_airfield) = jet_ai.find_suitable_airfield() {
                        if let Ok(mut guard) = obj.write() {
                            if let Some(new_airfield_obj) =
                                TheGameLogic::find_object_by_id(new_airfield)
                            {
                                if let Ok(new_airfield_guard) = new_airfield_obj.read() {
                                    guard.set_producer(Some(&*new_airfield_guard));
                                }
                            }
                        }
                        return StateReturnType::Success;
                    }
                }
                StateReturnType::Continue
            }
        }
    }
}

/// JetAIUpdate runtime state (port of C++ JetAIUpdate fields).
pub struct JetAIUpdate {
    object_id: ObjectID,
    data: JetAIUpdateModuleData,
    producer_location: Coord3D,
    most_recent_command: AICommandParmsStorage,
    afterburner_sound: AudioEventRts,
    afterburners_on: bool,
    attack_loco_expire_frame: UnsignedInt,
    attackers_miss_expire_frame: UnsignedInt,
    return_to_base_frame: UnsignedInt,
    targeted_by: Vec<ObjectID>,
    untargetable_expire_frame: UnsignedInt,
    lockon_drawable: Option<DrawableID>,
    flags: u32,
    landing_pos_for_helipad: Coord3D,
    engines_on: Bool,
    suppress_command_store: Bool,
    state_machine: JetStateMachine,
}

impl JetAIUpdate {
    pub fn new(data: JetAIUpdateModuleData, object_id: ObjectID) -> Self {
        let needs_runway = data.needs_runway;
        Self {
            object_id,
            data,
            producer_location: Coord3D::new(0.0, 0.0, 0.0),
            most_recent_command: Self::default_command_storage(),
            afterburner_sound: AudioEventRts::new(""),
            afterburners_on: false,
            attack_loco_expire_frame: 0,
            attackers_miss_expire_frame: 0,
            return_to_base_frame: 0,
            targeted_by: Vec::new(),
            untargetable_expire_frame: 0,
            lockon_drawable: None,
            flags: 0,
            landing_pos_for_helipad: Coord3D::new(0.0, 0.0, 0.0),
            engines_on: false,
            suppress_command_store: false,
            state_machine: JetStateMachine::new(needs_runway),
        }
    }

    fn default_command_storage() -> AICommandParmsStorage {
        AICommandParmsStorage {
            cmd: AiCommandType::NoCommand,
            cmd_source: crate::common::CommandSourceType::FromAi,
            pos: Coord3D::new(0.0, 0.0, 0.0),
            obj: INVALID_ID,
            other_obj: INVALID_ID,
            team_name: String::new(),
            coords: Vec::new(),
            waypoint: None,
            polygon: None,
            int_value: 0,
            damage: crate::common::DamageInfo::new(),
            command_button: None,
            command_button_name: String::new(),
            path: None,
        }
    }

    fn get_object(&self) -> Option<Arc<std::sync::RwLock<crate::object::Object>>> {
        OBJECT_REGISTRY.get_object(self.object_id)
    }

    fn producer_object(&self) -> Option<Arc<RwLock<crate::object::Object>>> {
        let Some(obj) = self.get_object() else {
            return None;
        };
        let producer_id = obj
            .read()
            .ok()
            .map(|guard| guard.get_producer_id())
            .unwrap_or(INVALID_ID);
        TheGameLogic::find_object_by_id(producer_id)
    }

    fn with_producer_parking_place<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(
            &mut dyn crate::object::behavior::behavior_module::ParkingPlaceBehaviorInterface,
        ) -> R,
    {
        let airfield = self.producer_object()?;
        let guard = airfield.read().ok()?;
        let mut func = func;
        guard.with_parking_place_behavior(|parking| func(parking))
    }

    fn with_airfield_parking_place<F, R>(&self, airfield_id: ObjectID, func: F) -> Option<R>
    where
        F: FnMut(
            &mut dyn crate::object::behavior::behavior_module::ParkingPlaceBehaviorInterface,
        ) -> R,
    {
        let airfield = TheGameLogic::find_object_by_id(airfield_id)?;
        let guard = airfield.read().ok()?;
        let mut func = func;
        guard.with_parking_place_behavior(|parking| func(parking))
    }

    fn find_suitable_airfield(&self) -> Option<ObjectID> {
        let Some(obj) = self.get_object() else {
            return None;
        };
        let pos = obj.read().ok().map(|guard| *guard.get_position())?;
        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return None;
        };
        partition.get_closest_object(&pos, 999999.0, |candidate| {
            if !candidate.is_kind_of(KindOf::FSAirfield) {
                return false;
            }
            if candidate.is_effectively_dead() {
                return false;
            }
            if candidate.test_status(ObjectStatusTypes::UnderConstruction)
                || candidate.test_status(ObjectStatusTypes::Sold)
            {
                return false;
            }
            if let Ok(jet_guard) = obj.read() {
                let relationship = jet_guard.relationship_to(candidate);
                if !matches!(relationship, crate::common::Relationship::Allies) {
                    return false;
                }
            }
            let mut ok = false;
            candidate.with_parking_place_behavior(|pp| {
                let mut info = PPInfo::default();
                ok = pp.reserve_space(self.object_id, 0.0, &mut info);
            });
            ok
        })
    }

    pub fn on_object_created(&mut self, ai: &mut dyn AIUpdateInterface) {
        self.set_allow_air_loco(false);
        let _ = ai.choose_locomotor_set(LocomotorSetType::Taxiing);
        self.engines_on = true;
    }

    fn with_state_machine<R>(
        &mut self,
        mut func: impl FnMut(&mut JetStateMachine, &mut JetAIUpdate) -> R,
    ) -> R {
        let needs_runway = self.state_machine.needs_runway;
        let mut machine =
            std::mem::replace(&mut self.state_machine, JetStateMachine::new(needs_runway));
        let result = func(&mut machine, self);
        self.state_machine = machine;
        result
    }

    pub fn update_with_ai(&mut self, ai: &mut dyn AIUpdateInterface) {
        self.get_producer_location(Some(ai));
        self.update();
        let now = TheGameLogic::get_frame();
        let is_reloading = matches!(self.state_machine.state, Some(JetAIStateType::ReloadAmmo));
        let is_idle = ai.is_idle_unrestricted() || is_reloading;

        if let Some(obj) = self.get_object() {
            let mut allow_air_loco = self.allow_air_loco();
            let has_pending = self.get_flag(JetFlag::HasPendingCommand);
            if is_idle {
                if let Ok(guard) = obj.read() {
                    let is_helipad = guard.is_kind_of(crate::common::KindOf::ProducedAtHelipad);
                    let mut fully_healed = false;
                    if let Some(body) = guard.get_body_module() {
                        let max_health = body.get_max_health();
                        let health = body.get_health();
                        fully_healed = max_health > 0.0 && health >= max_health;
                    }

                    let mut should_takeoff = false;
                    let _ = self.with_producer_parking_place(|pp| {
                        if !allow_air_loco && !has_pending && is_helipad && fully_healed {
                            should_takeoff = true;
                            pp.set_healee(None, false);
                        } else {
                            pp.set_healee(
                                if allow_air_loco {
                                    None
                                } else {
                                    Some(Arc::clone(&obj))
                                },
                                !allow_air_loco,
                            );
                        }
                    });
                    if should_takeoff {
                        self.set_allow_air_loco(true);
                        allow_air_loco = true;
                        self.with_state_machine(|machine, jet| machine.clear(ai, jet));
                        ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                        self.with_state_machine(|machine, jet| {
                            machine.set_state(JetAIStateType::TakingOffAwaitClearance, ai, jet)
                        });
                    }
                }

                if self.is_out_of_special_reload_ammo() && allow_air_loco {
                    self.return_to_base_frame = 0;
                    self.prune_dead_targeters();
                    self.set_use_special_return_loco(true);
                    ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                    self.with_state_machine(|machine, jet| {
                        machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                    });
                } else if has_pending && !is_reloading {
                    self.return_to_base_frame = 0;
                    let params = self.reconstitute_command_params();
                    self.set_has_pending_command(false);
                    let _ = ai.execute_command(&params);
                } else if self.return_to_base_frame != 0
                    && now >= self.return_to_base_frame
                    && allow_air_loco
                {
                    self.return_to_base_frame = 0;
                    self.set_use_special_return_loco(false);
                    ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                    self.with_state_machine(|machine, jet| {
                        machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                    });
                } else if self.return_to_base_frame == 0
                    && self.data.return_to_base_idle_time > 0
                    && allow_air_loco
                {
                    self.return_to_base_frame =
                        now.saturating_add(self.data.return_to_base_idle_time);
                }
            } else {
                let _ = self.with_producer_parking_place(|pp| {
                    pp.set_healee(None, false);
                });
                self.return_to_base_frame = 0;
                if self.get_flag(JetFlag::AllowInterruptAndResumeOfCurStateForReload)
                    && self.is_out_of_special_reload_ammo()
                    && allow_air_loco
                {
                    self.set_use_special_return_loco(true);
                    self.set_has_pending_command(true);
                    self.set_flag(JetFlag::AllowInterruptAndResumeOfCurStateForReload, false);
                    ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                    self.with_state_machine(|machine, jet| {
                        machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                    });
                }
            }
        }
        self.with_state_machine(|machine, jet| machine.update(ai, jet));
    }

    pub fn handle_command(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn AIUpdateInterface,
    ) -> bool {
        self.get_producer_location(Some(ai));

        self.store_most_recent_command(params);

        if self.is_takeoff_or_landing_in_progress() {
            self.set_has_pending_command(true);
            return true;
        }

        if params.cmd == AiCommandType::Idle
            && matches!(self.state_machine.state, Some(JetAIStateType::ReloadAmmo))
        {
            self.set_has_pending_command(true);
            return true;
        }

        if params.cmd == AiCommandType::Idle {
            if let Some(obj) = self.get_object() {
                if let Ok(guard) = obj.read() {
                    if guard.is_airborne_target()
                        && !guard.is_kind_of(crate::common::KindOf::ProducedAtHelipad)
                    {
                        self.with_state_machine(|machine, jet| machine.clear(ai, jet));
                        ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                        self.with_state_machine(|machine, jet| {
                            machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                        });
                        return true;
                    }
                }
            }
        }

        if params.cmd == AiCommandType::Idle
            && matches!(self.state_machine.state, Some(JetAIStateType::ReloadAmmo))
        {
            self.set_has_pending_command(true);
            return true;
        }

        if params.cmd == AiCommandType::Idle {
            if let Some(obj) = self.get_object() {
                if let Ok(guard) = obj.read() {
                    if guard.is_airborne_target() && !guard.is_kind_of(KindOf::ProducedAtHelipad) {
                        self.with_state_machine(|machine, jet| machine.clear(ai, jet));
                        ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                        self.with_state_machine(|machine, jet| {
                            machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                        });
                        return true;
                    }
                }
            }
        }

        if !self.allow_air_loco() {
            match params.cmd {
                AiCommandType::Idle
                | AiCommandType::Busy
                | AiCommandType::FollowExitProductionPath => {}
                AiCommandType::Enter | AiCommandType::GetRepaired => {
                    if let Some(obj) = self.get_object() {
                        if let Ok(guard) = obj.read() {
                            if self.is_parked_at(params.obj, &guard) {
                                return true;
                            }
                        }
                    }
                }
                _ => {
                    self.set_has_pending_command(true);
                    self.with_state_machine(|machine, jet| machine.clear(ai, jet));
                    ai.set_last_command_source(crate::ai::CommandSourceType::FromAi);
                    self.with_state_machine(|machine, jet| {
                        machine.set_state(JetAIStateType::TakingOffAwaitClearance, ai, jet)
                    });
                    return true;
                }
            }
        }

        match params.cmd {
            AiCommandType::GuardArea
            | AiCommandType::GuardObject
            | AiCommandType::GuardPosition
            | AiCommandType::Hunt
            | AiCommandType::GuardRetaliate => {
                self.set_flag(JetFlag::AllowInterruptAndResumeOfCurStateForReload, true);
            }
            _ => {
                self.set_flag(JetFlag::AllowInterruptAndResumeOfCurStateForReload, false);
            }
        }

        self.set_has_pending_command(false);

        match params.cmd {
            AiCommandType::FollowExitProductionPath => {
                self.with_state_machine(|machine, jet| machine.clear(ai, jet));
                if let Some(ignore) = params
                    .obj
                    .and_then(|id| TheGameLogic::find_object_by_id(id))
                {
                    let _ = ai.ignore_obstacle(Some(&ignore));
                }
                ai.set_last_command_source(params.cmd_source);
                if let Some(obj) = self.get_object() {
                    if let Ok(guard) = obj.read() {
                        if guard.is_kind_of(KindOf::ProducedAtHelipad) {
                            self.with_state_machine(|machine, jet| {
                                machine.set_state(JetAIStateType::TakingOffAwaitClearance, ai, jet)
                            });
                        } else {
                            self.with_state_machine(|machine, jet| {
                                machine.set_state(JetAIStateType::TaxiFromHangar, ai, jet)
                            });
                        }
                    }
                }
                self.set_has_pending_command(true);
                return true;
            }
            AiCommandType::Enter => {
                if let Some(obj_id) = params.obj {
                    if let (Some(owner), Some(target)) =
                        (self.get_object(), TheGameLogic::find_object_by_id(obj_id))
                    {
                        if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                            if !crate::action_manager::TheActionManager::can_enter_object(
                                &*owner_guard,
                                &*target_guard,
                                params.cmd_source,
                                crate::action_manager::CanEnterType::DontCheckCapacity,
                            ) {
                                return true;
                            }
                        }
                    }
                    self.do_landing_command(obj_id, params.cmd_source, ai);
                    return true;
                }
            }
            AiCommandType::GetRepaired => {
                if let Some(obj_id) = params.obj {
                    if let (Some(owner), Some(target)) =
                        (self.get_object(), TheGameLogic::find_object_by_id(obj_id))
                    {
                        if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                            if !crate::action_manager::TheActionManager::can_get_repaired_at(
                                &*owner_guard,
                                &*target_guard,
                                params.cmd_source,
                            ) {
                                return true;
                            }
                        }
                    }
                    self.do_landing_command(obj_id, params.cmd_source, ai);
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    fn do_landing_command(
        &mut self,
        airfield_id: ObjectID,
        cmd_source: crate::ai::CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) {
        if let Some(obj) = self.get_object() {
            if let Ok(guard) = obj.read() {
                if guard.is_kind_of(KindOf::ProducedAtHelipad) {
                    self.landing_pos_for_helipad = *guard.get_position();
                    if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                        let mut options = crate::helpers::FindPositionOptions::default();
                        options.max_radius =
                            guard.get_geometry_info().get_bounding_circle_radius() * 10.0;
                        let mut tmp = Coord3D::ZERO;
                        if partition.find_position_around_with_options(
                            &self.landing_pos_for_helipad,
                            &options,
                            &mut tmp,
                        ) {
                            self.landing_pos_for_helipad = tmp;
                        }
                    }
                }
            }
        }

        if let Some(airfield) = TheGameLogic::find_object_by_id(airfield_id) {
            if let Ok(air_guard) = airfield.read() {
                let mut reserved = false;
                let is_helipad = air_guard.is_kind_of(KindOf::ProducedAtHelipad);
                let _ = self.with_airfield_parking_place(airfield_id, |pp| {
                    let mut info = PPInfo::default();
                    if pp.reserve_space(self.object_id, self.data.parking_offset, &mut info)
                        || is_helipad
                    {
                        reserved = true;
                    }
                });
                if reserved {
                    let old_producer_id = self
                        .get_object()
                        .and_then(|obj| obj.read().ok().map(|guard| guard.get_producer_id()))
                        .unwrap_or(INVALID_ID);
                    if old_producer_id != airfield_id {
                        let _ = self.with_producer_parking_place(|pp| {
                            let _ = pp.release_space(self.object_id);
                        });
                    }
                    if let Some(obj) = self.get_object() {
                        if let Ok(mut guard) = obj.write() {
                            if let Some(airfield) = TheGameLogic::find_object_by_id(airfield_id) {
                                if let Ok(airfield_guard) = airfield.read() {
                                    guard.set_producer(Some(&*airfield_guard));
                                }
                            }
                        }
                    }
                    self.set_use_special_return_loco(false);
                    self.set_flag(JetFlag::AllowInterruptAndResumeOfCurStateForReload, false);
                    ai.set_last_command_source(cmd_source);
                    self.with_state_machine(|machine, jet| {
                        machine.set_state(JetAIStateType::ReturningForLanding, ai, jet)
                    });
                }
            }
        }
    }

    fn is_parked_at(&self, obj_id: Option<ObjectID>, obj: &crate::object::Object) -> bool {
        if !self.allow_air_loco() && !obj.is_kind_of(KindOf::ProducedAtHelipad) && obj_id.is_some()
        {
            if let Some(airfield) = self.producer_object() {
                if let Ok(air_guard) = airfield.read() {
                    if air_guard.get_id() == obj_id.unwrap_or(INVALID_ID) {
                        let mut has_pp = false;
                        air_guard.with_parking_place_behavior(|_| {
                            has_pp = true;
                        });
                        return has_pp;
                    }
                }
            }
        }
        false
    }

    fn init_afterburner_sound(&mut self) {
        let Some(obj) = self.get_object() else {
            return;
        };
        let Ok(guard) = obj.read() else {
            return;
        };
        if let Some(mut sound) = guard.get_template().get_per_unit_sound("Afterburner") {
            sound.set_object_id(self.object_id);
            self.afterburner_sound = sound;
        }
    }

    fn friend_enable_afterburners(&mut self, obj: &mut crate::object::Object, enable: bool) {
        if enable {
            obj.set_model_condition_state(ModelConditionFlags::JETAFTERBURNER);
            if !self.afterburner_sound.is_currently_playing()
                && !self.afterburner_sound.event_name.is_empty()
            {
                self.afterburner_sound.set_object_id(self.object_id);
                if let Some(audio) = TheAudio::get() {
                    let handle = audio.add_audio_event(&self.afterburner_sound);
                    self.afterburner_sound.set_playing_handle(handle);
                }
            }
        } else {
            obj.clear_model_condition_state(ModelConditionFlags::JETAFTERBURNER);
            if self.afterburner_sound.is_currently_playing() {
                if let Some(audio) = TheAudio::get() {
                    audio.remove_audio_event(self.afterburner_sound.get_playing_handle());
                }
                self.afterburner_sound.set_playing_handle(0);
            }
        }
        self.afterburners_on = enable;
    }

    pub fn add_waypoint_to_goal_path(&self, ai: &mut dyn AIUpdateInterface, pos: &Coord3D) {
        let _ = ai.append_goal_position_to_path(pos);
    }

    fn get_producer_location(&mut self, ai: Option<&mut dyn AIUpdateInterface>) {
        if self.get_flag(JetFlag::HasProducerLocation) {
            return;
        }
        let Some(obj) = self.get_object() else {
            return;
        };
        let producer_id = obj
            .read()
            .ok()
            .map(|guard| guard.get_producer_id())
            .unwrap_or(INVALID_ID);

        let mut allow_air_loco = true;
        let mut has_parking_place = false;

        if let Some(airfield) = TheGameLogic::find_object_by_id(producer_id) {
            if let Ok(air_guard) = airfield.read() {
                self.producer_location = *air_guard.get_position();
                air_guard.with_parking_place_behavior(|pp| {
                    has_parking_place = true;
                    if pp.has_reserved_space(self.object_id) {
                        allow_air_loco = false;
                    }
                });
            }
        } else if let Ok(obj_guard) = obj.read() {
            self.producer_location = *obj_guard.get_position();
            allow_air_loco = true;
        }

        if !has_parking_place {
            allow_air_loco = true;
        }
        self.set_allow_air_loco(allow_air_loco);
        if let Some(ai) = ai {
            let _ = if allow_air_loco {
                ai.choose_locomotor_set(LocomotorSetType::Normal)
            } else {
                ai.choose_locomotor_set(LocomotorSetType::Taxiing)
            };
        }

        self.set_flag(JetFlag::HasProducerLocation, true);
    }

    fn get_flag(&self, flag: JetFlag) -> bool {
        (self.flags & (1 << flag as u32)) != 0
    }

    fn set_flag(&mut self, flag: JetFlag, value: bool) {
        if value {
            self.flags |= 1 << flag as u32;
        } else {
            self.flags &= !(1 << flag as u32);
        }
    }

    pub fn set_takeoff_in_progress(&mut self, value: bool) {
        self.set_flag(JetFlag::TakeoffInProgress, value);
    }

    pub fn set_landing_in_progress(&mut self, value: bool) {
        self.set_flag(JetFlag::LandingInProgress, value);
    }

    pub fn set_taxi_in_progress(&mut self, value: bool) {
        self.set_flag(JetFlag::TaxiInProgress, value);
    }

    pub fn set_allow_air_loco(&mut self, value: bool) {
        self.set_flag(JetFlag::AllowAirLoco, value);
    }

    pub fn allow_air_loco(&self) -> bool {
        self.get_flag(JetFlag::AllowAirLoco)
    }

    pub fn allow_circling(&self) -> bool {
        self.get_flag(JetFlag::AllowCircling)
    }

    pub fn set_suppress_command_store(&mut self, value: bool) {
        self.suppress_command_store = value;
    }

    pub fn suppress_command_store(&self) -> bool {
        self.suppress_command_store
    }

    pub fn set_has_pending_command(&mut self, value: bool) {
        self.set_flag(JetFlag::HasPendingCommand, value);
    }

    pub fn has_pending_command(&self) -> bool {
        self.get_flag(JetFlag::HasPendingCommand)
    }

    pub fn pending_command_type(&self) -> Option<AiCommandType> {
        if self.has_pending_command() {
            Some(self.most_recent_command.cmd)
        } else {
            None
        }
    }

    pub fn set_allow_circling(&mut self, value: bool) {
        self.set_flag(JetFlag::AllowCircling, value);
    }

    pub fn set_use_special_return_loco(&mut self, value: bool) {
        self.set_flag(JetFlag::UseSpecialReturnLoco, value);
    }

    pub fn is_takeoff_or_landing_in_progress(&self) -> bool {
        self.get_flag(JetFlag::TakeoffInProgress) || self.get_flag(JetFlag::LandingInProgress)
    }

    pub fn is_taxiing_to_parking(&self) -> bool {
        matches!(
            self.state_machine.state,
            Some(JetAIStateType::TaxiFromHangar)
                | Some(JetAIStateType::TaxiFromLanding)
                | Some(JetAIStateType::OrientForParkingPlace)
                | Some(JetAIStateType::ReloadAmmo)
                | Some(JetAIStateType::TakingOffAwaitClearance)
                | Some(JetAIStateType::TaxiToTakeoff)
                | Some(JetAIStateType::PauseBeforeTakeoff)
                | Some(JetAIStateType::TakingOff)
        )
    }

    pub fn parking_offset(&self) -> Real {
        self.data.parking_offset
    }

    pub fn needs_runway(&self) -> Bool {
        self.data.needs_runway
    }

    pub fn keeps_parking_space_when_airborne(&self) -> Bool {
        self.data.keeps_parking_space_when_airborne
    }

    pub fn should_block_idle(&self, pending_command: Option<AiCommandType>) -> bool {
        pending_command.is_some() || self.get_flag(JetFlag::HasPendingCommand)
    }

    pub fn is_out_of_special_reload_ammo(&self) -> bool {
        let Some(obj) = self.get_object() else {
            return false;
        };
        let Ok(guard) = obj.read() else {
            return false;
        };

        let mut specials = 0;
        let mut out = 0;
        for slot_index in 0..WEAPONSLOT_COUNT {
            let slot = match slot_index {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };
            let Some(weapon) = guard.get_weapon_in_weapon_slot(slot) else {
                continue;
            };
            if weapon.get_template().reload_type != WeaponReloadType::ReturnToBaseToReload {
                continue;
            }
            specials += 1;
            if weapon.get_status() == WeaponStatus::OutOfAmmo {
                out += 1;
            }
        }

        specials > 0 && out == specials
    }

    pub fn get_sneaky_targeting_offset(&self, offset: &mut Coord3D) -> bool {
        if self.attackers_miss_expire_frame == 0 {
            return false;
        }
        if TheGameLogic::get_frame() >= self.attackers_miss_expire_frame {
            return false;
        }
        let Some(obj) = self.get_object() else {
            return false;
        };
        let Ok(guard) = obj.read() else {
            return false;
        };
        let (dir_x, dir_y) = guard.get_unit_direction_vector_2d();
        offset.x = dir_x * self.data.sneaky_offset_when_attacking;
        offset.y = dir_y * self.data.sneaky_offset_when_attacking;
        offset.z = 0.0;
        true
    }

    pub fn prune_dead_targeters(&mut self) {
        if self.targeted_by.is_empty() {
            return;
        }
        self.targeted_by
            .retain(|id| OBJECT_REGISTRY.get_object(*id).is_some());
        if self.targeted_by.is_empty() {
            self.untargetable_expire_frame = 0;
        }
    }

    fn position_lockon(&mut self) {
        let Some(drawable_id) = self.lockon_drawable else {
            return;
        };
        if self.untargetable_expire_frame == 0 {
            if let Some(client) = TheGameClient::get() {
                client.destroy_drawable(drawable_id);
            }
            self.lockon_drawable = None;
            return;
        }

        let now = TheGameLogic::get_frame();
        let remaining = self.untargetable_expire_frame.saturating_sub(now);
        let lockon_time = self.data.lockon_time.max(1);
        let elapsed = lockon_time.saturating_sub(remaining);

        let Some(obj) = self.get_object() else {
            return;
        };
        let Ok(guard) = obj.read() else {
            return;
        };

        let mut pos = *guard.get_position();
        let frac = remaining as Real / lockon_time as Real;
        let final_dist = guard.get_geometry_info().get_bounding_circle_radius();
        let dist = final_dist + (self.data.lockon_initial_dist - final_dist) * frac;
        let angle = self.data.lockon_angle_spin * frac;

        pos.x += angle.cos() * dist;
        pos.y += angle.sin() * dist;

        if let Some(client) = TheGameClient::get() {
            client.set_drawable_position(drawable_id, &pos);
            let dx = guard.get_position().x - pos.x;
            let dy = guard.get_position().y - pos.y;
            if dx != 0.0 || dy != 0.0 {
                client.set_drawable_orientation(drawable_id, dy.atan2(dx));
            }
        }

        let elapsed_prev = elapsed.saturating_sub(1);
        let elapsed_time_sum_prev = 0.5 * (elapsed_prev as Real) * (elapsed as Real);
        let elapsed_time_sum_curr = elapsed_time_sum_prev + elapsed as Real;
        let factor = self.data.lockon_freq / lockon_time as Real;
        let last_phase = ((factor * elapsed_time_sum_prev) as i32 & 1) != 0;
        let this_phase = ((factor * elapsed_time_sum_curr) as i32 & 1) != 0;

        if last_phase && !this_phase {
            if let Some(audio) = TheAudio::get() {
                if let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio()
                {
                    let misc_audio = misc_audio.read();
                    let mut lockon_sound =
                        AudioEventRts::new(misc_audio.lockon_tick_sound.sound_file.clone());
                    lockon_sound.set_object_id(self.object_id);
                    audio.add_audio_event(&lockon_sound);
                }
            }
            if self.data.lockon_blinky {
                if let Some(client) = TheGameClient::get() {
                    client.set_drawable_hidden(drawable_id, false);
                }
            }
        } else if self.data.lockon_blinky {
            if let Some(client) = TheGameClient::get() {
                client.set_drawable_hidden(drawable_id, true);
            }
        }
    }

    fn build_lockon_drawable_if_necessary(&mut self) {
        if self.untargetable_expire_frame == 0 {
            return;
        }
        if self.data.lockon_cursor.is_empty() || self.lockon_drawable.is_some() {
            self.position_lockon();
            return;
        }
        let Some(template) = TheThingFactory::find_template(self.data.lockon_cursor.as_str())
        else {
            return;
        };
        if let Some(client) = TheGameClient::get() {
            self.lockon_drawable = Some(client.create_drawable(template.as_ref()));
        }
        self.position_lockon();
    }

    pub fn add_targeter(&mut self, id: ObjectID, add: bool) {
        let lockon_time = self.data.lockon_time;
        if lockon_time == 0 {
            return;
        }
        if add {
            if !self.targeted_by.contains(&id) {
                self.targeted_by.push(id);
                if self.untargetable_expire_frame == 0 && self.targeted_by.len() == 1 {
                    self.untargetable_expire_frame =
                        TheGameLogic::get_frame().saturating_add(lockon_time);
                    self.build_lockon_drawable_if_necessary();
                }
            }
        } else if let Some(pos) = self.targeted_by.iter().position(|entry| *entry == id) {
            self.targeted_by.remove(pos);
            if self.targeted_by.is_empty() {
                self.untargetable_expire_frame = 0;
            }
        }
    }

    pub fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.untargetable_expire_frame != 0
            && TheGameLogic::get_frame() < self.untargetable_expire_frame
    }

    pub fn is_allowed_to_move_away_from_unit(&self) -> bool {
        if !self.get_flag(JetFlag::AllowAirLoco)
            || self.get_flag(JetFlag::TakeoffInProgress)
            || self.get_flag(JetFlag::LandingInProgress)
        {
            return false;
        }
        true
    }

    pub fn is_doing_ground_movement(&self) -> bool {
        false
    }

    pub fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        if self.get_flag(JetFlag::TaxiInProgress) {
            return false;
        }
        true
    }

    pub fn notify_victim_is_dead(&mut self) {
        if self.data.needs_runway {
            self.return_to_base_frame = TheGameLogic::get_frame();
        }
    }

    pub fn update(&mut self) {
        if self.afterburner_sound.event_name.is_empty() {
            self.init_afterburner_sound();
        }
        self.get_producer_location(None);

        let now = TheGameLogic::get_frame();
        if self.attack_loco_expire_frame != 0 && now >= self.attack_loco_expire_frame {
            self.attack_loco_expire_frame = 0;
        }
        if self.attackers_miss_expire_frame != 0 && now >= self.attackers_miss_expire_frame {
            self.attackers_miss_expire_frame = 0;
        }
        if self.untargetable_expire_frame != 0 && now >= self.untargetable_expire_frame {
            self.untargetable_expire_frame = 0;
        }

        if let Some(obj) = self.get_object() {
            if let Ok(mut guard) = obj.write() {
                if guard.test_status(crate::common::ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING) {
                    self.attack_loco_expire_frame =
                        now.saturating_add(self.data.attack_loco_persist_time);
                    self.attackers_miss_expire_frame =
                        now.saturating_add(self.data.attackers_miss_persist_time);
                }

                let mut min_height = self.data.min_height;
                let producer_id = guard.get_producer_id();
                if let Some(airfield) = TheGameLogic::find_object_by_id(producer_id) {
                    if let Ok(air_guard) = airfield.read() {
                        air_guard.with_parking_place_behavior(|pp| {
                            min_height += pp.get_landing_deck_height_offset();
                        });
                    }
                }

                if let Some(drawable) = guard.get_drawable() {
                    let state_active = self.state_machine.state.is_some();
                    let need_min_height = state_active
                        || !guard.is_above_terrain()
                        || !self.get_flag(JetFlag::AllowAirLoco)
                        || guard.test_status(crate::common::ObjectStatusTypes::DeckHeightOffset);
                    if need_min_height {
                        let height = if guard.is_above_terrain() {
                            guard.get_height_above_terrain()
                        } else {
                            0.0
                        };
                        if height < min_height {
                            let offset = Matrix3D::from_translation(glam::Vec3::new(
                                0.0,
                                0.0,
                                min_height - height,
                            ));
                            drawable.set_instance_matrix(Some(&offset));
                        } else {
                            drawable.set_instance_matrix(None);
                        }
                    } else {
                        drawable.set_instance_matrix(None);
                    }
                }

                if let Some(physics) = guard.get_physics() {
                    if let Ok(phys_guard) = physics.lock() {
                        let speed = phys_guard.get_velocity().length();
                        let should_enable = speed > 0.0 && self.get_flag(JetFlag::AllowAirLoco);
                        if should_enable != self.afterburners_on {
                            self.friend_enable_afterburners(&mut guard, should_enable);
                        }
                        if should_enable {
                            guard.set_model_condition_state(ModelConditionFlags::JETEXHAUST);
                        } else {
                            guard.clear_model_condition_state(ModelConditionFlags::JETEXHAUST);
                        }
                    }
                }

                if !guard.is_kind_of(crate::common::KindOf::ProducedAtHelipad) {
                    let waiting_for_path = guard
                        .get_ai_update_interface()
                        .and_then(|ai| ai.lock().ok().map(|guard| guard.is_waiting_for_path()))
                        .unwrap_or(false);
                    if let Some(drawable) = guard.get_drawable() {
                        let should_enable = self.get_flag(JetFlag::TakeoffInProgress)
                            || self.get_flag(JetFlag::LandingInProgress)
                            || guard.is_significantly_above_terrain()
                            || guard.is_moving()
                            || waiting_for_path;
                        if should_enable && !self.engines_on {
                            if let Ok(mut drawable_guard) = drawable.write() {
                                drawable_guard.enable_ambient_sound_from_script(true);
                                self.engines_on = true;
                            }
                        } else if !should_enable && self.engines_on {
                            if let Ok(mut drawable_guard) = drawable.write() {
                                drawable_guard.enable_ambient_sound_from_script(false);
                                self.engines_on = false;
                            }
                        }
                    }
                }
            }
        }

        self.prune_dead_targeters();
        self.position_lockon();
    }

    pub fn store_most_recent_command(&mut self, params: &AiCommandParams) {
        let waypoint = if let Some(id) = params.waypoint {
            if let Ok(terrain) = get_terrain_logic().read() {
                terrain
                    .get_waypoint_by_id(id)
                    .map(|waypoint| Arc::new(Waypoint::from_terrain(waypoint)))
            } else {
                None
            }
        } else {
            None
        };
        let polygon = if let Some(id) = params.polygon {
            if let Ok(terrain) = get_terrain_logic().read() {
                terrain
                    .get_trigger_areas()
                    .get_by_id(id)
                    .map(|trigger| Arc::new(trigger.clone()))
            } else {
                None
            }
        } else {
            None
        };

        self.most_recent_command = AICommandParmsStorage {
            cmd: params.cmd,
            cmd_source: params.cmd_source,
            pos: params.pos,
            obj: params.obj.unwrap_or(INVALID_ID),
            other_obj: params.other_obj.unwrap_or(INVALID_ID),
            team_name: params.team.clone().unwrap_or_default(),
            coords: params.coords.clone(),
            waypoint,
            polygon,
            int_value: params.int_value,
            damage: crate::damage::DamageInfo::new(),
            command_button: None,
            command_button_name: String::new(),
            path: None,
        };
    }

    pub fn reconstitute_command_params(&self) -> AiCommandParams {
        let mut params = AiCommandParams::new(
            self.most_recent_command.cmd,
            self.most_recent_command.cmd_source,
        );
        params.pos = self.most_recent_command.pos;
        if self.most_recent_command.obj != INVALID_ID {
            params.obj = Some(self.most_recent_command.obj);
        }
        if self.most_recent_command.other_obj != INVALID_ID {
            params.other_obj = Some(self.most_recent_command.other_obj);
        }
        if !self.most_recent_command.team_name.is_empty() {
            params.team = Some(self.most_recent_command.team_name.clone());
        }
        params.coords = self.most_recent_command.coords.clone();
        params.waypoint = self.most_recent_command.waypoint.as_ref().map(|wp| wp.id);
        params.polygon = self
            .most_recent_command
            .polygon
            .as_ref()
            .map(|poly| poly.get_id());
        params.int_value = self.most_recent_command.int_value;
        params.damage = crate::ai::DamageInfo::default();
        params
    }

    pub fn desired_locomotor_set(&self) -> Option<LocomotorSetType> {
        if !self.get_flag(JetFlag::AllowAirLoco) {
            return Some(LocomotorSetType::Taxiing);
        }
        if self.attack_loco_expire_frame != 0 {
            return Some(self.data.attacking_loco);
        }
        if self.get_flag(JetFlag::UseSpecialReturnLoco) {
            return Some(self.data.returning_loco);
        }
        None
    }

    pub fn is_reloading_for_command(
        &self,
        current_command: Option<AiCommandType>,
        is_out_of_ammo: bool,
    ) -> bool {
        if !is_out_of_ammo {
            return false;
        }
        matches!(
            current_command,
            Some(AiCommandType::Enter) | Some(AiCommandType::Dock)
        )
    }

    pub fn is_reloading(&self) -> bool {
        matches!(self.state_machine.state, Some(JetAIStateType::ReloadAmmo))
    }
}

impl Drop for JetAIUpdate {
    fn drop(&mut self) {
        if let Some(obj) = self.get_object() {
            if let Ok(guard) = obj.read() {
                let producer_id = guard.get_producer_id();
                if let Some(airfield) = TheGameLogic::find_object_by_id(producer_id) {
                    if let Ok(air_guard) = airfield.read() {
                        air_guard.with_parking_place_behavior(|pp| {
                            pp.release_space(self.object_id);
                        });
                    }
                }
            }
        }

        if let Some(drawable_id) = self.lockon_drawable.take() {
            if let Some(client) = TheGameClient::get() {
                client.destroy_drawable(drawable_id);
            }
        }

        if self.afterburner_sound.is_currently_playing() {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.afterburner_sound.get_playing_handle());
            }
            self.afterburner_sound.set_playing_handle(0);
        }
    }
}

/// Module wrapper for JetAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct JetAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<JetAIUpdateModuleData>,
}

impl JetAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<JetAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for JetAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for JetAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_field(data: &mut JetAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = JET_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn jet_fields_accept_ini_equals_token() {
        let mut data = JetAIUpdateModuleData::default();

        parse_field(&mut data, "OutOfAmmoDamagePerSecond", &["=", "25%"]);
        parse_field(&mut data, "NeedsRunway", &["=", "No"]);
        parse_field(&mut data, "KeepsParkingSpaceWhenAirborne", &["=", "No"]);
        parse_field(&mut data, "TakeoffDistForMaxLift", &["=", "75%"]);
        parse_field(&mut data, "TakeoffPause", &["=", "1500"]);
        parse_field(&mut data, "MinHeight", &["=", "80.5"]);
        parse_field(&mut data, "ParkingOffset", &["=", "12.25"]);
        parse_field(&mut data, "SneakyOffsetWhenAttacking", &["=", "33.75"]);
        parse_field(&mut data, "AttackLocomotorType", &["=", "SET_SUPERSONIC"]);
        parse_field(&mut data, "AttackLocomotorPersistTime", &["=", "2400"]);
        parse_field(&mut data, "AttackersMissPersistTime", &["=", "900"]);
        parse_field(
            &mut data,
            "ReturnForAmmoLocomotorType",
            &["=", "SET_TAXIING"],
        );
        parse_field(&mut data, "LockonTime", &["=", "1200"]);
        parse_field(&mut data, "LockonCursor", &["=", "LaserGuidedMissile"]);
        parse_field(&mut data, "LockonInitialDist", &["=", "180.0"]);
        parse_field(&mut data, "LockonFreq", &["=", "0.25"]);
        parse_field(&mut data, "LockonAngleSpin", &["=", "360"]);
        parse_field(&mut data, "LockonBlinky", &["=", "Yes"]);
        parse_field(&mut data, "ReturnToBaseIdleTime", &["=", "3000"]);

        assert_eq!(data.out_of_ammo_damage_per_second, 0.25);
        assert!(!data.needs_runway);
        assert!(!data.keeps_parking_space_when_airborne);
        assert_eq!(data.takeoff_dist_for_max_lift, 0.75);
        assert_eq!(data.takeoff_pause, 45);
        assert_eq!(data.min_height, 80.5);
        assert_eq!(data.parking_offset, 12.25);
        assert_eq!(data.sneaky_offset_when_attacking, 33.75);
        assert_eq!(data.attacking_loco, LocomotorSetType::Supersonic);
        assert_eq!(data.attack_loco_persist_time, 72);
        assert_eq!(data.attackers_miss_persist_time, 27);
        assert_eq!(data.returning_loco, LocomotorSetType::Taxiing);
        assert_eq!(data.lockon_time, 36);
        assert_eq!(data.lockon_cursor.as_str(), "LaserGuidedMissile");
        assert_eq!(data.lockon_initial_dist, 180.0);
        assert_eq!(data.lockon_freq, 0.25);
        assert_eq!(
            data.lockon_angle_spin,
            INI::parse_angle_real("360").unwrap()
        );
        assert!(data.lockon_blinky);
        assert_eq!(data.return_to_base_idle_time, 90);
    }
}
