//! DeliverPayloadAIUpdate - AI update logic for airborne payload delivery.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, Bool, Coord3D, Int, ModelConditionFlags, ObjectID, RadiusDecal,
    RadiusDecalTemplate, Real, TheWeaponStore, ThingTemplate, UnsignedInt, Vec3D, FROM_CENTER_2D,
    FROM_CENTER_3D, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING,
};
use crate::helpers::{
    get_game_logic_random_value_real, TheAudio, TheGameLogic, ThePartitionManager, TheTerrainLogic,
    TheThingFactory,
};
use crate::locomotor::Locomotor;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt,
    DeliverPayloadAIUpdateInterface,
};
use crate::object::behavior::{
    generate_minefield_behavior::GenerateMinefieldBehavior,
    smart_bomb_target_homing_update::SmartBombTargetHomingUpdate,
};
use crate::object::drawable::DrawableArcExt;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::state_machine::StateReturnType;
use crate::weapon::WeaponLockType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use super::deliver_payload_data::{DeliverPayloadData, RADIUS_DECAL_TEMPLATE_FIELDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeliverPayloadState {
    Approach,
    Delivering,
    ConsiderNewApproach,
    RecoverFromOffMap,
    HeadOffMap,
    CleanUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiveState {
    PreDive = 0,
    Diving = 1,
    PostDive = 2,
}

/// DeliverPayloadAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct DeliverPayloadAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub door_delay: UnsignedInt,
    pub max_distance_to_target: Real,
    pub max_number_attempts: Int,
    pub drop_delay: UnsignedInt,
    pub drop_offset: Coord3D,
    pub drop_variance: Coord3D,
    pub put_in_container_name: AsciiString,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: Real,
}

impl Default for DeliverPayloadAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            door_delay: 0,
            max_distance_to_target: 0.0,
            max_number_attempts: 0,
            drop_delay: 0,
            drop_offset: Coord3D::ZERO,
            drop_variance: Coord3D::ZERO,
            put_in_container_name: AsciiString::new(),
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

impl DeliverPayloadAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DELIVER_PAYLOAD_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for DeliverPayloadAIUpdateModuleData {
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

impl Snapshotable for DeliverPayloadAIUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_int_field(setter: &mut dyn FnMut(Int), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_coord3d_field(setter: &mut dyn FnMut(Coord3D), tokens: &[&str]) -> Result<(), INIError> {
    let coord = if tokens.len() >= 3 {
        let x = INI::parse_real(tokens[0])?;
        let y = INI::parse_real(tokens[1])?;
        let z = INI::parse_real(tokens[2])?;
        Coord3D::new(x, y, z)
    } else {
        let Some(token) = tokens.first().copied() else {
            return Err(INIError::InvalidData);
        };
        let parts: Vec<&str> = token
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() != 3 {
            return Err(INIError::InvalidData);
        }
        let x = INI::parse_real(parts[0])?;
        let y = INI::parse_real(parts[1])?;
        let z = INI::parse_real(parts[2])?;
        Coord3D::new(x, y, z)
    };
    setter(coord);
    Ok(())
}

fn parse_ascii_field(setter: &mut dyn FnMut(AsciiString), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(AsciiString::from(token));
    Ok(())
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut DeliverPayloadAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
        "YES",
        "STEALTHED",
        "NO",
        "NOTWHILEATTACKING",
        "ATTACK_BUILDINGS",
    ];
    let value = INI::parse_bit_string_32(tokens, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn parse_delivery_decal(
    ini: &mut INI,
    data: &mut DeliverPayloadAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.delivery_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

const DELIVER_PAYLOAD_AI_UPDATE_FIELDS: &[FieldParse<DeliverPayloadAIUpdateModuleData>] = &[
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
    },
    FieldParse {
        token: "MoodAttackCheckRate",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.base.set_surrender_duration_frames(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.base.set_forbid_player_commands(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.base.set_turrets_linked(value), tokens)
        },
    },
    FieldParse {
        token: "DoorDelay",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.door_delay = v, tokens),
    },
    FieldParse {
        token: "PutInContainer",
        parse: |_, data, tokens| parse_ascii_field(&mut |v| data.put_in_container_name = v, tokens),
    },
    FieldParse {
        token: "DeliveryDistance",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.max_distance_to_target = v, tokens),
    },
    FieldParse {
        token: "MaxAttempts",
        parse: |_, data, tokens| parse_int_field(&mut |v| data.max_number_attempts = v, tokens),
    },
    FieldParse {
        token: "DropDelay",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.drop_delay = v, tokens),
    },
    FieldParse {
        token: "DropOffset",
        parse: |_, data, tokens| parse_coord3d_field(&mut |v| data.drop_offset = v, tokens),
    },
    FieldParse {
        token: "DropVariance",
        parse: |_, data, tokens| parse_coord3d_field(&mut |v| data.drop_variance = v, tokens),
    },
    FieldParse {
        token: "DeliveryDecal",
        parse: parse_delivery_decal,
    },
    FieldParse {
        token: "DeliveryDecalRadius",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.delivery_decal_radius = v, tokens),
    },
];

/// Module wrapper for DeliverPayloadAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct DeliverPayloadAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<DeliverPayloadAIUpdateModuleData>,
}

impl DeliverPayloadAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<DeliverPayloadAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for DeliverPayloadAIUpdateModule {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

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

impl Snapshotable for DeliverPayloadAIUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// DeliverPayload AI runtime data + state.
#[derive(Debug, Clone)]
pub struct DeliverPayloadAIUpdate {
    owner_id: ObjectID,
    module_data: DeliverPayloadAIUpdateModuleData,
    data: DeliverPayloadData,
    target_pos: Coord3D,
    move_to_pos: Coord3D,
    visible_items_delivered: Int,
    delivery_decal: RadiusDecal,
    previous_distance_sqr: Real,
    free_to_exit: Bool,
    accepting_commands: Bool,
    dive_state: DiveState,
    state: DeliverPayloadState,
    state_active: Bool,
    drop_delay_left: UnsignedInt,
    did_open: Bool,
    consider_entries: Int,
    re_entry_frame: UnsignedInt,
    facing_direction_upon_delivery: Coord3D,
}

impl DeliverPayloadAIUpdate {
    fn to_locomotor_damage(
        damage: crate::common::BodyDamageType,
    ) -> crate::locomotor::core::BodyDamageType {
        match damage {
            crate::common::BodyDamageType::Pristine => {
                crate::locomotor::core::BodyDamageType::Pristine
            }
            crate::common::BodyDamageType::Damaged => {
                crate::locomotor::core::BodyDamageType::Damaged
            }
            crate::common::BodyDamageType::ReallyDamaged => {
                crate::locomotor::core::BodyDamageType::ReallyDamaged
            }
            crate::common::BodyDamageType::Rubble => crate::locomotor::core::BodyDamageType::Rubble,
        }
    }

    pub fn new(module_data: DeliverPayloadAIUpdateModuleData, owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            module_data,
            data: DeliverPayloadData::default(),
            target_pos: Coord3D::ZERO,
            move_to_pos: Coord3D::ZERO,
            visible_items_delivered: 0,
            delivery_decal: RadiusDecal::new(Coord3D::ZERO, 0.0),
            previous_distance_sqr: 0.0,
            free_to_exit: false,
            accepting_commands: true,
            dive_state: DiveState::PreDive,
            state: DeliverPayloadState::Approach,
            state_active: false,
            drop_delay_left: 0,
            did_open: false,
            consider_entries: 0,
            re_entry_frame: 0,
            facing_direction_upon_delivery: Coord3D::ZERO,
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.delivery_decal.update();

        if self.state_active && !ai.is_ai_in_dead_state() {
            self.update_state(ai);
        }

        self.update_dive_logic(ai);

        Ok(())
    }

    pub fn is_delivering_payload(&self) -> bool {
        self.state_active
    }

    pub fn get_target_pos(&self) -> &Coord3D {
        &self.target_pos
    }

    pub fn get_move_to_pos(&self) -> &Coord3D {
        &self.move_to_pos
    }

    pub fn get_put_in_container_template_via_module_data(&self) -> Option<Arc<dyn ThingTemplate>> {
        if self.module_data.put_in_container_name.is_empty() {
            return None;
        }
        TheThingFactory::find_template(self.module_data.put_in_container_name.as_str())
    }

    fn kill_delivery_decal(&mut self) {
        self.delivery_decal.clear();
    }

    pub fn is_allowed_to_respond_to_ai_commands(&self) -> bool {
        self.accepting_commands
    }

    fn ai_move_to_position(&self, pos: &Coord3D) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return;
        };
        ai.ai_move_to_position(pos, false, CommandSourceType::FromAi);
    }

    fn ai_set_allow_invalid_position(&self, allow: Bool) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let ai = if let Ok(owner_guard) = owner.read() {
            owner_guard.get_ai_update_interface()
        } else {
            None
        };
        let Some(ai) = ai else {
            return;
        };
        {
            let Ok(mut ai_guard) = ai.lock() else {
                return;
            };
            let _ = ai_guard.set_allow_invalid_position(allow);
        }
    }

    fn ai_set_ultra_accurate(&self, ultra: Bool) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let ai = if let Ok(owner_guard) = owner.read() {
            owner_guard.get_ai_update_interface()
        } else {
            None
        };
        let Some(ai) = ai else {
            return;
        };
        {
            let Ok(mut ai_guard) = ai.lock() else {
                return;
            };
            let _ = ai_guard.set_ultra_accurate(ultra);
        }
    }

    fn ai_get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        let owner = TheGameLogic::find_object_by_id(self.owner_id)?;
        let owner_guard = owner.read().ok()?;
        let ai = owner_guard.get_ai_update_interface()?;
        ai.lock().ok().and_then(|guard| guard.get_cur_locomotor())
    }

    fn ai_is_moving(&self) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return false;
        };
        let Ok(owner_guard) = owner.read() else {
            return false;
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return false;
        };
        ai.lock()
            .ok()
            .map(|guard| guard.is_moving())
            .unwrap_or(false)
    }

    fn ai_is_idle(&self) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return false;
        };
        let Ok(owner_guard) = owner.read() else {
            return false;
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return false;
        };
        ai.lock().ok().map(|guard| guard.is_idle()).unwrap_or(false)
    }

    fn calc_min_turn_radius(&self, time_to_travel: Option<&mut Real>) -> Real {
        let owner = TheGameLogic::find_object_by_id(self.owner_id);
        let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
        let Some(owner_guard) = owner_guard else {
            return 999999.0;
        };
        let body = owner_guard.get_body_module();
        let locomotor = self.ai_get_cur_locomotor();
        let (Some(body), Some(locomotor)) = (body, locomotor) else {
            return 999999.0;
        };
        let Ok(body_guard) = body.lock() else {
            return 999999.0;
        };
        let Ok(loco_guard) = locomotor.lock() else {
            return 999999.0;
        };
        let condition = Self::to_locomotor_damage(body_guard.get_damage_state());
        let max_speed = loco_guard.get_max_speed_for_condition(condition);
        let max_turn_rate = loco_guard.get_max_turn_rate(condition);
        let min_turn_radius = if max_turn_rate > 0.0 {
            max_speed / max_turn_rate
        } else {
            999999.0
        };
        if let Some(out) = time_to_travel {
            if max_speed > 0.0 {
                *out = min_turn_radius / max_speed;
            }
        }
        min_turn_radius
    }

    fn is_close_enough_to_target(&mut self) -> Bool {
        let allowed_distance_sqr = self.data.dist_to_target * self.data.dist_to_target;
        let current_distance_sqr = if let Some(obj) = TheGameLogic::find_object_by_id(self.owner_id)
        {
            if let Ok(guard) = obj.read() {
                ThePartitionManager::get_distance_squared_to_pos(
                    &guard,
                    &self.target_pos,
                    FROM_CENTER_2D,
                )
            } else {
                0.0
            }
        } else {
            0.0
        };

        let inbound = self.previous_distance_sqr > current_distance_sqr;
        self.previous_distance_sqr = current_distance_sqr;

        let mut allowed = allowed_distance_sqr;
        if inbound {
            let distance = self.data.dist_to_target + self.data.pre_open_distance;
            allowed = distance * distance;
        }

        allowed > current_distance_sqr
    }

    fn is_off_map(&self) -> Bool {
        let Some(terrain) = TheTerrainLogic::get() else {
            return false;
        };
        let map_region = terrain.get_extent_including_border();
        let owner = TheGameLogic::find_object_by_id(self.owner_id);
        let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
        let Some(owner_guard) = owner_guard else {
            return true;
        };
        let pos = owner_guard.get_position();
        pos.x < map_region.lo.x
            || pos.x > map_region.hi.x
            || pos.y < map_region.lo.y
            || pos.y > map_region.hi.y
    }

    fn update_state(&mut self, ai: &mut dyn AIUpdateInterface) {
        match self.state {
            DeliverPayloadState::Approach => match self.update_approach(ai) {
                StateReturnType::Success => self.enter_state(DeliverPayloadState::Delivering),
                StateReturnType::Failure => {
                    self.enter_state(DeliverPayloadState::ConsiderNewApproach)
                }
                _ => {}
            },
            DeliverPayloadState::Delivering => match self.update_delivering() {
                StateReturnType::Success => self.enter_state(DeliverPayloadState::HeadOffMap),
                StateReturnType::Failure => {
                    self.enter_state(DeliverPayloadState::ConsiderNewApproach)
                }
                _ => {}
            },
            DeliverPayloadState::ConsiderNewApproach => {
                if self.is_off_map() {
                    self.enter_state(DeliverPayloadState::RecoverFromOffMap);
                } else {
                    match self.update_consider_new_approach() {
                        StateReturnType::Success => self.enter_state(DeliverPayloadState::Approach),
                        StateReturnType::Failure => {
                            self.enter_state(DeliverPayloadState::HeadOffMap)
                        }
                        _ => {}
                    }
                }
            }
            DeliverPayloadState::RecoverFromOffMap => match self.update_recover_from_off_map() {
                StateReturnType::Success => self.enter_state(DeliverPayloadState::Approach),
                StateReturnType::Failure => self.enter_state(DeliverPayloadState::Approach),
                _ => {}
            },
            DeliverPayloadState::HeadOffMap => match self.update_head_off_map() {
                StateReturnType::Success => self.enter_state(DeliverPayloadState::CleanUp),
                StateReturnType::Failure => self.enter_state(DeliverPayloadState::CleanUp),
                _ => {}
            },
            DeliverPayloadState::CleanUp => {
                let _ = self.enter_cleanup();
                self.state_active = false;
            }
        }
    }

    fn enter_state(&mut self, new_state: DeliverPayloadState) {
        self.exit_state(self.state);
        self.state = new_state;
        self.enter_state_impl(new_state);
    }

    fn enter_state_impl(&mut self, state: DeliverPayloadState) {
        match state {
            DeliverPayloadState::Approach => {
                self.enter_approach();
            }
            DeliverPayloadState::Delivering => {
                self.enter_delivering();
            }
            DeliverPayloadState::ConsiderNewApproach => {
                let _ = self.enter_consider_new_approach();
            }
            DeliverPayloadState::RecoverFromOffMap => {
                let _ = self.enter_recover_from_off_map();
            }
            DeliverPayloadState::HeadOffMap => {
                let _ = self.enter_head_off_map();
            }
            DeliverPayloadState::CleanUp => {
                let _ = self.enter_cleanup();
            }
        }
    }

    fn exit_state(&mut self, state: DeliverPayloadState) {
        if state == DeliverPayloadState::Delivering {
            self.exit_delivering();
        }
        if state == DeliverPayloadState::ConsiderNewApproach {
            self.ai_set_allow_invalid_position(true);
        }
    }

    fn enter_approach(&mut self) {
        self.ai_move_to_position(&self.move_to_pos);
    }

    fn update_approach(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        if ai.is_ai_in_dead_state() {
            return StateReturnType::Failure;
        }

        if self.is_close_enough_to_target() {
            return StateReturnType::Success;
        }

        if !self.ai_is_moving() {
            if self.ai_is_idle() {
                return StateReturnType::Failure;
            }
            self.ai_move_to_position(&self.move_to_pos);
        }

        StateReturnType::Continue
    }

    fn enter_delivering(&mut self) {
        let owner = TheGameLogic::find_object_by_id(self.owner_id);
        let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
        let Some(owner_guard) = owner_guard else {
            return;
        };
        let mut flags_to_clear = ModelConditionFlags::empty();
        flags_to_clear.insert(MODELCONDITION_DOOR_1_CLOSING);
        let mut flags_to_set = ModelConditionFlags::empty();
        flags_to_set.insert(MODELCONDITION_DOOR_1_OPENING);
        drop(owner_guard);
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut guard) = owner.write() {
                let _ = guard.clear_and_set_model_condition_flags(flags_to_clear, flags_to_set);
            }
        }
        self.drop_delay_left = self.module_data.door_delay;
        self.did_open = false;
    }

    fn update_delivering(&mut self) -> StateReturnType {
        if self.drop_delay_left > 0 {
            self.drop_delay_left = self.drop_delay_left.saturating_sub(1);
            return StateReturnType::Continue;
        }

        self.free_to_exit = true;
        self.did_open = true;
        self.drop_delay_left = self.data.drop_delay;

        if !self.is_close_enough_to_target() {
            return StateReturnType::Failure;
        }

        let owner = TheGameLogic::find_object_by_id(self.owner_id);
        let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
        let Some(owner_guard) = owner_guard else {
            return StateReturnType::Failure;
        };

        let contained_ids = owner_guard
            .get_contain()
            .map(|contain| contain.get_contained_objects().to_vec())
            .unwrap_or_default();
        drop(owner_guard);

        if contained_ids.is_empty() && self.visible_items_delivered >= self.data.visible_num_bones {
            return StateReturnType::Success;
        }

        if let Some(item_id) = contained_ids.first().copied() {
            if let Some(item) = TheGameLogic::find_object_by_id(item_id) {
                if self.data.fire_weapon {
                    if let Ok(mut owner_guard) = owner.as_ref().unwrap().write() {
                        let mut pos = self.target_pos;
                        pos.x += self.data.drop_offset.x;
                        pos.y += self.data.drop_offset.y;
                        pos.z += self.data.drop_offset.z;
                        let _ = owner_guard.fire_current_weapon_at_position(&pos);
                    }
                    let _ = TheGameLogic::destroy_object_by_id(item_id);
                } else {
                    if let Ok(item_guard) = item.read() {
                        if let Some(ai) = item_guard.get_ai_update_interface() {
                            let mut params = AiCommandParams::new(
                                AiCommandType::Exit,
                                CommandSourceType::FromAi,
                            );
                            params.obj = Some(self.owner_id);
                            let _ = ai.execute_command(&params);
                        }
                    }

                    if let Ok(mut item_guard) = item.write() {
                        let mut pos = *item_guard.get_position();
                        if self.data.drop_variance.x > 0.0 {
                            pos.x += get_game_logic_random_value_real(
                                -self.data.drop_variance.x,
                                self.data.drop_variance.x,
                            );
                        }
                        if self.data.drop_variance.y > 0.0 {
                            pos.y += get_game_logic_random_value_real(
                                -self.data.drop_variance.y,
                                self.data.drop_variance.y,
                            );
                        }
                        if self.data.drop_variance.z > 0.0 {
                            pos.z += get_game_logic_random_value_real(
                                -self.data.drop_variance.z,
                                self.data.drop_variance.z,
                            );
                        }
                        pos.x += self.data.drop_offset.x;
                        pos.y += self.data.drop_offset.y;
                        pos.z += self.data.drop_offset.z;
                        let _ = item_guard.set_position(&pos);

                        if self.data.is_parachute_directly {
                            if let Some(contain) = item_guard.get_contain() {
                                contain.set_override_destination(&self.target_pos);
                            }
                        } else if let Some(ai) = item_guard.get_ai_update_interface() {
                            ai.ai_move_to_position(
                                &self.move_to_pos,
                                false,
                                CommandSourceType::FromAi,
                            );
                        }
                    }

                    if let Ok(item_guard) = item.read() {
                        if let Some(module) =
                            item_guard.find_update_module("GenerateMinefieldBehavior")
                        {
                            let _ = module.with_module_downcast::<
                                crate::object::behavior::generate_minefield_behavior::GenerateMinefieldBehaviorModule,
                                _,
                                _,
                            >(|module| {
                                module.behavior_mut().set_minefield_target(Some(self.move_to_pos));
                            });
                        }

                        if let Some(module) =
                            item_guard.find_update_module("SmartBombTargetHomingUpdate")
                        {
                            let _ = module.with_module_downcast::<
                                crate::object::behavior::smart_bomb_target_homing_update::SmartBombTargetHomingUpdateModule,
                                _,
                                _,
                            >(|module| {
                                module.behavior_mut().set_target_position(&self.move_to_pos);
                            });
                        }
                    }

                    if self.data.inherit_transport_velocity {
                        let owner_velocity = owner
                            .as_ref()
                            .and_then(|obj| obj.read().ok())
                            .and_then(|guard| guard.get_physics())
                            .and_then(|physics| physics.lock().ok().map(|p| p.get_velocity()));
                        if let Some(owner_velocity) = owner_velocity {
                            if let Ok(mut item_guard) = item.write() {
                                if let Some(physics) = item_guard.get_physics() {
                                    if let Ok(mut phys_guard) = physics.lock() {
                                        phys_guard.apply_force(&owner_velocity);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.visible_items_delivered < self.data.visible_num_bones {
            let mut attempt_drops = self.data.visible_items_dropped_per_interval;
            let owner = TheGameLogic::find_object_by_id(self.owner_id);
            let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
            if let Some(owner_guard) = owner_guard {
                let draw = owner_guard.get_drawable();
                let mut update_sub_objects = false;
                while attempt_drops > 0
                    && self.visible_items_delivered < self.data.visible_num_bones
                {
                    if let Some(draw) = draw.as_ref() {
                        if let Ok(mut draw_guard) = draw.write() {
                            if !self.data.visible_sub_object_name.is_empty() {
                                let name = format!(
                                    "{}{:02}",
                                    self.data.visible_sub_object_name.as_str(),
                                    self.visible_items_delivered + 1
                                );
                                draw_guard.show_sub_object(&name, false);
                                update_sub_objects = true;
                            }
                        }
                    }

                    if !self.data.visible_payload_template_name.is_empty() {
                        if let Some(template) = TheThingFactory::find_template(
                            self.data.visible_payload_template_name.as_str(),
                        ) {
                            let factory = TheThingFactory::get();
                            if let Ok(factory) = factory {
                                if let Some(team) = owner_guard
                                    .get_controlling_player()
                                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                                {
                                    let Ok(team_guard) = team.read() else {
                                        continue;
                                    };
                                    if let Ok(payload) = factory.new_object(template, &*team_guard)
                                    {
                                        if let Ok(mut payload_guard) = payload.write() {
                                            if let Some(owner) = owner.as_ref() {
                                                if let Ok(owner_guard) = owner.read() {
                                                    payload_guard.set_producer(Some(&*owner_guard));
                                                }
                                            }

                                            if !self.data.visible_drop_bone_name.is_empty() {
                                                if let Some(draw) = draw.as_ref() {
                                                    if let Ok(draw_guard) = draw.read() {
                                                        let mut positions = draw_guard
                                                            .get_pristine_bone_positions(
                                                                self.data
                                                                    .visible_drop_bone_name
                                                                    .as_str(),
                                                                (self.visible_items_delivered + 1)
                                                                    as usize,
                                                                1,
                                                            );
                                                        if let Some(local_pos) = positions.pop() {
                                                            let world = draw
                                                                .get_transform()
                                                                .transform_point3(local_pos);
                                                            let _ =
                                                                payload_guard.set_position(&world);
                                                        } else {
                                                            let _ = payload_guard.set_position(
                                                                owner_guard.get_position(),
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    let _ = payload_guard
                                                        .set_position(owner_guard.get_position());
                                                }
                                            } else {
                                                let _ = payload_guard
                                                    .set_position(owner_guard.get_position());
                                            }
                                            let _ = payload_guard
                                                .set_orientation(owner_guard.get_orientation());

                                            if self.data.inherit_transport_velocity {
                                                let owner_velocity =
                                                    owner_guard.get_physics().and_then(|p| {
                                                        p.lock().ok().map(|p| p.get_velocity())
                                                    });
                                                if let Some(owner_velocity) = owner_velocity {
                                                    if let Some(physics) =
                                                        payload_guard.get_physics()
                                                    {
                                                        if let Ok(mut phys_guard) = physics.lock() {
                                                            let mut starting_force = owner_velocity;
                                                            starting_force *= phys_guard.get_mass();
                                                            phys_guard.apply_motive_force(
                                                                &starting_force,
                                                            );

                                                            let mut back_position =
                                                                owner_velocity * -1.0;
                                                            back_position +=
                                                                *payload_guard.get_position();
                                                            let _ = payload_guard
                                                                .set_position(&back_position);
                                                        }
                                                    }
                                                }
                                            }

                                            let mut projectile_fired = false;
                                            if let Some(weapon) =
                                                self.data.visible_payload_weapon_template.as_ref()
                                            {
                                                let _ = TheWeaponStore::get().map(|store| {
                                                    store.create_and_fire_temp_weapon_at_pos(
                                                        weapon,
                                                        payload_guard.get_id(),
                                                        &self.target_pos,
                                                    )
                                                });
                                                projectile_fired = true;
                                            }

                                            if !projectile_fired {
                                                if self.data.exit_pitch_rate != 0.0 {
                                                    if let Some(physics) =
                                                        payload_guard.get_physics()
                                                    {
                                                        if let Ok(mut phys_guard) = physics.lock() {
                                                            phys_guard.set_pitch_rate(
                                                                self.data.exit_pitch_rate,
                                                            );
                                                        }
                                                    }
                                                }

                                                if let Some(ai) =
                                                    payload_guard.get_ai_update_interface()
                                                {
                                                    ai.ai_move_to_position(
                                                        &self.move_to_pos,
                                                        false,
                                                        CommandSourceType::FromAi,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    attempt_drops -= 1;
                    self.visible_items_delivered += 1;
                }
                if update_sub_objects {
                    if let Some(draw) = draw.as_ref() {
                        if let Ok(mut draw_guard) = draw.write() {
                            draw_guard.update_sub_objects();
                        }
                    }
                }
            }
        }

        StateReturnType::Continue
    }

    fn exit_delivering(&mut self) {
        self.free_to_exit = false;
        if !self.did_open {
            log::warn!("DeliverPayloadAIUpdate: doors closed before opening.");
        }
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut guard) = owner.write() {
                let mut flags_to_clear = ModelConditionFlags::empty();
                flags_to_clear.insert(MODELCONDITION_DOOR_1_OPENING);
                let mut flags_to_set = ModelConditionFlags::empty();
                flags_to_set.insert(MODELCONDITION_DOOR_1_CLOSING);
                let _ = guard.clear_and_set_model_condition_flags(flags_to_clear, flags_to_set);
            }
        }
    }

    fn enter_consider_new_approach(&mut self) -> StateReturnType {
        self.consider_entries += 1;
        if self.consider_entries > self.data.max_attempts {
            return StateReturnType::Failure;
        }

        let min_turn_radius = self.calc_min_turn_radius(None);
        let min_reapproach_dist = min_turn_radius * 2.2;

        let owner = TheGameLogic::find_object_by_id(self.owner_id);
        let owner_guard = owner.as_ref().and_then(|obj| obj.read().ok());
        let Some(owner_guard) = owner_guard else {
            return StateReturnType::Failure;
        };
        let (dir_x, dir_y) = owner_guard.get_unit_direction_vector_2d();

        let re_approach_point = Coord3D::new(
            owner_guard.get_position().x + dir_x * min_reapproach_dist,
            owner_guard.get_position().y + dir_y * min_reapproach_dist,
            0.0,
        );

        self.ai_move_to_position(&re_approach_point);
        self.ai_set_allow_invalid_position(true);

        StateReturnType::Continue
    }

    fn update_consider_new_approach(&self) -> StateReturnType {
        if !self.ai_is_moving() {
            return StateReturnType::Success;
        }
        StateReturnType::Continue
    }

    fn enter_recover_from_off_map(&mut self) -> StateReturnType {
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                self.ai_move_to_position(owner_guard.get_position());
            }
        }

        let mut time_to_travel = 0.0;
        let _ = self.calc_min_turn_radius(Some(&mut time_to_travel));
        self.re_entry_frame =
            TheGameLogic::get_frame() + time_to_travel.ceil().max(0.0) as UnsignedInt;

        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                if let Some(physics) = owner_guard.get_physics() {
                    if let Ok(mut phys_guard) = physics.lock() {
                        phys_guard.set_velocity(&Vec3D::ZERO);
                        phys_guard.set_yaw_rate(0.0);
                        phys_guard.set_pitch_rate(0.0);
                        phys_guard.set_roll_rate(0.0);
                    }
                }
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        let _ = draw_guard.set_drawable_hidden(true);
                    }
                }
            }
        }

        StateReturnType::Continue
    }

    fn update_recover_from_off_map(&mut self) -> StateReturnType {
        if TheGameLogic::get_frame() < self.re_entry_frame {
            return StateReturnType::Continue;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return StateReturnType::Failure;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return StateReturnType::Failure;
        };

        if let Some(drawable) = owner_guard.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                let _ = draw_guard.set_drawable_hidden(false);
            }
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            return StateReturnType::Failure;
        };
        let mut enter_coord = terrain.find_closest_edge_point(owner_guard.get_position());
        if owner_guard.is_above_terrain() {
            enter_coord.z = owner_guard.get_position().z;
        }
        let _ = owner_guard.set_position(&enter_coord);

        let angle = (self.move_to_pos.y - enter_coord.y).atan2(self.move_to_pos.x - enter_coord.x);
        let _ = owner_guard.set_orientation(angle);

        if let Some(physics) = owner_guard.get_physics() {
            if let Ok(mut phys_guard) = physics.lock() {
                phys_guard.set_velocity(&Vec3D::ZERO);
                phys_guard.set_yaw_rate(0.0);
                phys_guard.set_pitch_rate(0.0);
                phys_guard.set_roll_rate(0.0);
            }
        }

        StateReturnType::Success
    }

    fn enter_head_off_map(&mut self) -> StateReturnType {
        self.kill_delivery_decal();

        if self.data.self_destruct_object {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(owner_guard) = owner.read() {
                    let _ = TheGameLogic::destroy_object(&owner_guard);
                }
            }
            return StateReturnType::Continue;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let (dir_x, dir_y) = owner_guard.get_unit_direction_vector_2d();
        self.facing_direction_upon_delivery = Coord3D::new(dir_x, dir_y, 0.0);

        let Some(terrain) = TheTerrainLogic::get() else {
            return StateReturnType::Failure;
        };
        let extent = terrain.get_maximum_pathfind_extent();
        let huge_dist = 1.2
            * ((extent.hi.x - extent.lo.x).powi(2) + (extent.hi.y - extent.lo.y).powi(2)).sqrt();
        let mut exit_coord = *owner_guard.get_position();
        exit_coord.x += dir_x * huge_dist;
        exit_coord.y += dir_y * huge_dist;

        drop(owner_guard);
        self.ai_set_allow_invalid_position(true);
        self.ai_set_ultra_accurate(true);
        self.ai_move_to_position(&exit_coord);

        self.accepting_commands = false;

        StateReturnType::Continue
    }

    fn update_head_off_map(&mut self) -> StateReturnType {
        if self.is_off_map() {
            return StateReturnType::Success;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return StateReturnType::Failure;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return StateReturnType::Failure;
        };
        if let Some(physics) = owner_guard.get_physics() {
            if let Ok(phys_guard) = physics.lock() {
                if phys_guard.get_turning() != 0.0 {
                    let (dir_x, dir_y) = owner_guard.get_unit_direction_vector_2d();
                    let current_direction = Coord3D::new(dir_x, dir_y, 0.0);
                    let dot = self.facing_direction_upon_delivery.x * current_direction.x
                        + self.facing_direction_upon_delivery.y * current_direction.y
                        + self.facing_direction_upon_delivery.z * current_direction.z;
                    if dot < 0.3 {
                        owner_guard.kill(None, None);
                    }
                }
            }
        }

        StateReturnType::Continue
    }

    fn enter_cleanup(&mut self) -> StateReturnType {
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(contain) = owner_guard.get_contain() {
                    if contain.get_contained_count() > 0 {
                        log::warn!("DeliverPayloadAIUpdate: cleanup before all items dropped.");
                    }
                }
                let _ = TheGameLogic::destroy_object(&owner_guard);
            }
        }

        StateReturnType::Continue
    }

    fn update_dive_logic(&mut self, ai: &mut dyn AIUpdateInterface) {
        if self.dive_state == DiveState::PostDive {
            return;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };

        if self.dive_state == DiveState::PreDive {
            let start_dive_distance_sqr =
                self.data.dive_start_distance * self.data.dive_start_distance;
            let current_distance_sqr = ThePartitionManager::get_distance_squared_to_pos(
                &owner_guard,
                &self.target_pos,
                FROM_CENTER_2D,
            );
            if current_distance_sqr <= start_dive_distance_sqr {
                self.dive_state = DiveState::Diving;
                if let Some(loco) = ai.get_cur_locomotor() {
                    if let Ok(mut loco_guard) = loco.lock() {
                        loco_guard.set_precise_z_pos(true);
                    }
                }

                if let Some(mut sound) = owner_guard.get_template().get_per_unit_sound("StartDive")
                {
                    let pos = owner_guard.get_position();
                    sound.set_position(&(pos.x, pos.y, pos.z));
                    if let Some(audio) = TheAudio::get() {
                        audio.add_audio_event(&sound);
                    }
                }
            }
        } else {
            let end_dive_distance_sqr = self.data.dive_end_distance * self.data.dive_end_distance;
            let current_distance_sqr = ThePartitionManager::get_distance_squared_to_pos(
                &owner_guard,
                &self.target_pos,
                FROM_CENTER_3D,
            );
            if current_distance_sqr <= end_dive_distance_sqr {
                self.dive_state = DiveState::PostDive;
                if let Some(loco) = ai.get_cur_locomotor() {
                    if let Ok(mut loco_guard) = loco.lock() {
                        loco_guard.set_precise_z_pos(false);
                    }
                }
            }

            if let Some(slot) = self.data.strafing_weapon_slot {
                if let Some(physics) = owner_guard.get_physics() {
                    if let Ok(phys_guard) = physics.lock() {
                        if phys_guard.get_velocity().z < 5.0 {
                            let start_dive_distance = self.data.dive_start_distance;
                            let end_dive_distance = end_dive_distance_sqr.sqrt();
                            let current_distance = current_distance_sqr.sqrt();
                            let denom = (start_dive_distance - end_dive_distance).max(0.001);
                            let dive_ratio = (start_dive_distance - current_distance) / denom;

                            let mut velocity = phys_guard.get_velocity();
                            velocity.z = 0.0;
                            if velocity.length() > 0.0 {
                                velocity = velocity.normalize();
                            }
                            velocity *= dive_ratio * 100.0;

                            let backwards = velocity * 0.33;
                            let mut strafe_point = self.target_pos;
                            strafe_point.x -= backwards.x;
                            strafe_point.y -= backwards.y;
                            strafe_point.z -= backwards.z;
                            strafe_point.x += velocity.x;
                            strafe_point.y += velocity.y;
                            strafe_point.z += velocity.z;

                            if let Some(terrain) = TheTerrainLogic::get() {
                                strafe_point.z =
                                    terrain.get_ground_height(strafe_point.x, strafe_point.y, None);
                            }

                            drop(phys_guard);
                            drop(owner_guard);

                            if let Ok(mut owner_guard) = owner.write() {
                                let weapon_slot = match slot {
                                    crate::common::WeaponSlotType::Primary => {
                                        crate::weapon::WeaponSlotType::Primary
                                    }
                                    crate::common::WeaponSlotType::Secondary => {
                                        crate::weapon::WeaponSlotType::Secondary
                                    }
                                    crate::common::WeaponSlotType::Tertiary => {
                                        crate::weapon::WeaponSlotType::Tertiary
                                    }
                                };
                                owner_guard.set_weapon_lock(
                                    weapon_slot,
                                    WeaponLockType::LockedTemporarily,
                                );
                                let _ = owner_guard.fire_current_weapon_at_position(&strafe_point);
                            }

                            if let Some(fx) = self.data.strafe_fx.as_ref() {
                                let _ = fx.do_fx_at_position_with_radius(
                                    &strafe_point,
                                    self.data.strafe_length,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DeliverPayloadAIUpdateInterface for DeliverPayloadAIUpdate {
    fn deliver_payload(
        &mut self,
        move_to_pos: &Coord3D,
        target_pos: &Coord3D,
        data: &DeliverPayloadData,
    ) {
        self.move_to_pos = *move_to_pos;
        self.target_pos = *target_pos;
        self.data = data.clone();

        self.delivery_decal.clear();
        if self.data.delivery_decal_radius > 0.0 {
            let mut decal = self
                .data
                .delivery_decal_template
                .create_radius_decal(*target_pos);
            decal.radius = self.data.delivery_decal_radius;
            self.delivery_decal = decal;
        }

        if self.data.dive_start_distance <= 0.0 {
            self.dive_state = DiveState::PostDive;
        } else {
            self.dive_state = DiveState::PreDive;
        }

        self.visible_items_delivered = 0;
        self.free_to_exit = false;
        self.accepting_commands = true;
        self.consider_entries = 0;
        self.state_active = true;
        self.state = DeliverPayloadState::Approach;
        self.drop_delay_left = 0;
        self.did_open = false;

        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        let mut update_sub_objects = false;
                        for i in 1..=self.data.visible_num_bones {
                            if !self.data.visible_sub_object_name.is_empty() {
                                let name = format!(
                                    "{}{:02}",
                                    self.data.visible_sub_object_name.as_str(),
                                    i
                                );
                                draw_guard.show_sub_object(&name, true);
                                update_sub_objects = true;
                            }
                        }
                        if update_sub_objects {
                            draw_guard.update_sub_objects();
                        }
                    }
                }
            }
        }

        self.enter_state_impl(DeliverPayloadState::Approach);
    }

    fn deliver_payload_via_module_data(&mut self, move_to_pos: &Coord3D) {
        let mut dp_data = DeliverPayloadData::default();
        dp_data.drop_offset = self.module_data.drop_offset;
        dp_data.drop_variance = self.module_data.drop_variance;
        dp_data.dist_to_target = self.module_data.max_distance_to_target;
        dp_data.max_attempts = self.module_data.max_number_attempts;
        dp_data.drop_delay = self.module_data.drop_delay;
        dp_data.delivery_decal_template = self.module_data.delivery_decal_template.clone();
        dp_data.delivery_decal_radius = self.module_data.delivery_decal_radius;

        self.deliver_payload(move_to_pos, move_to_pos, &dp_data);
    }

    fn is_delivering_payload(&self) -> Bool {
        self.is_delivering_payload()
    }

    fn is_allowed_to_respond_to_ai_commands(&self) -> Bool {
        self.is_allowed_to_respond_to_ai_commands()
    }
}
