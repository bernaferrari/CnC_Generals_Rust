//! AssaultTransportAIUpdate - AI update logic for assault transports.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/AssaultTransportAIUpdate.cpp.

use std::any::Any;
use std::sync::Arc;

use crate::ai::object_registry::get_legacy_object;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Bool, Coord3D, ObjectID, Real, UnsignedInt, INVALID_ID};
use crate::helpers::TheGameLogic;
use crate::modules::{AIUpdateInterface, AIUpdateInterfaceExt, AssaultTransportAIUpdateInterface};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::weapon::NO_MAX_SHOTS_LIMIT;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const MAX_TRANSPORT_SLOTS: usize = 10;

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssaultStateType {
    Idle,
    #[allow(dead_code)]
    Assaulting,
}

/// AssaultTransportAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct AssaultTransportAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub members_get_healed_at_life_ratio: Real,
    pub clear_range_required_to_continue_attack_move: Real,
}

impl Default for AssaultTransportAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            members_get_healed_at_life_ratio: 0.0,
            clear_range_required_to_continue_attack_move: 50.0,
        }
    }
}

impl AssaultTransportAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ASSAULT_TRANSPORT_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for AssaultTransportAIUpdateModuleData {
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

impl Snapshotable for AssaultTransportAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_real(&mut self.members_get_healed_at_life_ratio))?;
        xfer_io(xfer.xfer_real(&mut self.clear_range_required_to_continue_attack_move))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Vec<&'a str> {
    tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect()
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut AssaultTransportAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens);
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
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

const ASSAULT_TRANSPORT_AI_UPDATE_FIELDS: &[FieldParse<AssaultTransportAIUpdateModuleData>] = &[
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
        token: "MembersGetHealedAtLifeRatio",
        parse: |_, data, tokens| {
            parse_real_field(
                &mut |value| data.members_get_healed_at_life_ratio = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "ClearRangeRequiredToContinueAttackMove",
        parse: |_, data, tokens| {
            parse_real_field(
                &mut |value| data.clear_range_required_to_continue_attack_move = value,
                tokens,
            )
        },
    },
];

/// Module wrapper for AssaultTransportAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct AssaultTransportAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<AssaultTransportAIUpdateModuleData>,
}

impl AssaultTransportAIUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<AssaultTransportAIUpdateModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for AssaultTransportAIUpdateModule {
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

impl Snapshotable for AssaultTransportAIUpdateModule {
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

/// Runtime configuration for AssaultTransportAIUpdate.
#[derive(Debug, Clone)]
pub struct AssaultTransportAIUpdateData {
    pub members_get_healed_at_life_ratio: Real,
    pub clear_range_required_to_continue_attack_move: Real,
}

impl Default for AssaultTransportAIUpdateData {
    fn default() -> Self {
        Self {
            members_get_healed_at_life_ratio: 0.0,
            clear_range_required_to_continue_attack_move: 50.0,
        }
    }
}

/// Assault transport AI runtime logic.
#[derive(Debug, Clone)]
pub struct AssaultTransportAIUpdate {
    data: AssaultTransportAIUpdateData,
    owner_id: ObjectID,
    member_ids: [ObjectID; MAX_TRANSPORT_SLOTS],
    member_healing: [Bool; MAX_TRANSPORT_SLOTS],
    new_member: [Bool; MAX_TRANSPORT_SLOTS],
    attack_move_goal_pos: Coord3D,
    designated_target: ObjectID,
    state: AssaultStateType,
    frames_remaining: UnsignedInt,
    current_members: usize,
    is_attack_move: Bool,
    is_attack_object: Bool,
    new_occupants_are_new_members: Bool,
}

impl AssaultTransportAIUpdate {
    pub fn new(data: AssaultTransportAIUpdateData, owner_id: ObjectID) -> Self {
        let mut update = Self {
            data,
            owner_id,
            member_ids: [INVALID_ID; MAX_TRANSPORT_SLOTS],
            member_healing: [false; MAX_TRANSPORT_SLOTS],
            new_member: [false; MAX_TRANSPORT_SLOTS],
            attack_move_goal_pos: Coord3D::new(0.0, 0.0, 0.0),
            designated_target: INVALID_ID,
            state: AssaultStateType::Idle,
            frames_remaining: 0,
            current_members: 0,
            is_attack_move: false,
            is_attack_object: false,
            new_occupants_are_new_members: false,
        };
        update.reset();
        update
    }

    pub fn handle_command(&mut self, command: &AiCommandParams) {
        if command.cmd_source == CommandSourceType::FromAi {
            return;
        }

        match command.cmd {
            AiCommandType::AttackMoveToPosition => {
                self.reset();
                self.attack_move_goal_pos = command.pos;
                self.is_attack_move = true;
            }
            AiCommandType::AttackObject => {
                self.reset();
                self.is_attack_object = true;
            }
            AiCommandType::Idle => {
                self.designated_target = INVALID_ID;
                self.retrieve_members();
                self.reset();
            }
            _ => {
                self.reset();
            }
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(transport) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(transport_guard) = transport.read() else {
            return Ok(());
        };

        if transport_guard.is_effectively_dead() {
            drop(transport_guard);
            self.give_final_orders();
            return Ok(());
        }

        drop(transport_guard);
        self.prune_missing_members();
        self.add_new_members();

        if self.is_attack_pointless() {
            let _ = ai.execute_command(&AiCommandParams::new(
                AiCommandType::Idle,
                CommandSourceType::FromAi,
            ));
            return Ok(());
        }

        let mut fighting_members = 0;
        let mut fighter_centroid_pos = Coord3D::new(0.0, 0.0, 0.0);

        let designated_target =
            TheGameLogic::find_object_by_id(self.designated_target).and_then(|target| {
                let is_dead = {
                    let Ok(guard) = target.read() else {
                        return None;
                    };
                    guard.is_effectively_dead()
                };
                if is_dead {
                    None
                } else {
                    Some(target)
                }
            });

        if let Some(_target) = designated_target.as_ref() {
            let legacy_target = get_legacy_object(self.designated_target);
            for i in 0..self.current_members {
                let Some(member) = TheGameLogic::find_object_by_id(self.member_ids[i]) else {
                    continue;
                };
                let Ok(member_guard) = member.read() else {
                    continue;
                };
                let contained = member_guard.get_contained_by().is_some();
                let wounded = self.is_member_wounded(&member_guard);
                let healthy = self.is_member_healthy(&member_guard);

                drop(member_guard);
                let Some(member_ai) = member.read().ok().and_then(|guard| guard.get_ai()) else {
                    continue;
                };

                if contained && healthy && !self.new_member[i] {
                    if let Ok(mut ai_guard) = member_ai.lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::Exit, CommandSourceType::FromAi);
                        params.obj = Some(self.owner_id);
                        let _ = ai_guard.execute_command(&params);
                    }
                }

                if !contained {
                    if wounded {
                        let should_enter = member_ai
                            .lock()
                            .ok()
                            .and_then(|guard| guard.get_current_command())
                            != Some(AiCommandType::Enter);
                        if should_enter {
                            member_ai.ai_enter(self.owner_id, CommandSourceType::FromAi);
                        }
                    } else {
                        if let Ok(member_guard) = member.read() {
                            fighter_centroid_pos += *member_guard.get_position();
                            fighting_members += 1;

                            if let Ok(ai_guard) = member_ai.lock() {
                                if !ai_guard.is_moving() {
                                    if ai_guard.get_goal_object_id() != self.designated_target {
                                        drop(ai_guard);
                                        if let Some(target) = legacy_target.as_ref() {
                                            member_ai.ai_attack_object(
                                                target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                                NO_MAX_SHOTS_LIMIT,
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
        } else {
            if self.is_attack_move {
                let should_issue =
                    ai.get_current_command() != Some(AiCommandType::AttackMoveToPosition);
                if should_issue {
                    let _ = ai.execute_command(&{
                        let mut params = AiCommandParams::new(
                            AiCommandType::AttackMoveToPosition,
                            CommandSourceType::FromAi,
                        );
                        params.pos = self.attack_move_goal_pos;
                        params.int_value = NO_MAX_SHOTS_LIMIT;
                        params
                    });
                }
            } else if self.is_attack_object {
                self.retrieve_members();
            }
        }

        let _ = (fighting_members, fighter_centroid_pos);
        let _ = self.frames_remaining;
        Ok(())
    }

    fn reset(&mut self) {
        for slot in 0..MAX_TRANSPORT_SLOTS {
            self.member_ids[slot] = INVALID_ID;
            self.member_healing[slot] = false;
            self.new_member[slot] = false;
        }
        self.current_members = 0;
        self.attack_move_goal_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.designated_target = INVALID_ID;
        self.state = AssaultStateType::Idle;
        self.frames_remaining = 0;
        self.is_attack_move = false;
        self.is_attack_object = false;
        self.new_occupants_are_new_members = false;
    }

    fn prune_missing_members(&mut self) {
        if self.current_members == 0 {
            return;
        }

        let mut i = 0;
        while i < self.current_members {
            let member_id = self.member_ids[i];
            let member = TheGameLogic::find_object_by_id(member_id);
            let member_guard = member.as_ref().and_then(|obj| obj.read().ok());
            let member_ai = member_guard.as_ref().and_then(|guard| guard.get_ai());

            let should_remove = member_guard
                .as_ref()
                .map(|guard| guard.is_effectively_dead())
                .unwrap_or(true)
                || member_ai
                    .as_ref()
                    .map(|ai| ai.get_last_command_source() != CommandSourceType::FromAi)
                    .unwrap_or(true);

            if should_remove {
                if let Some(ai) = member_ai {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_allow_chase(false);
                    }
                }

                if self.current_members - 1 > i {
                    let last = self.current_members - 1;
                    self.member_ids[i] = self.member_ids[last];
                    self.member_healing[i] = self.member_healing[last];
                    self.new_member[i] = self.new_member[last];
                } else {
                    self.member_ids[i] = INVALID_ID;
                    self.member_healing[i] = false;
                    self.new_member[i] = false;
                }
                self.current_members = self.current_members.saturating_sub(1);
            } else {
                i += 1;
            }
        }
    }

    fn add_new_members(&mut self) {
        let Some(transport) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(transport_guard) = transport.read() else {
            return;
        };
        let Some(contain) = transport_guard.get_contain() else {
            return;
        };
        let Ok(contain_guard) = contain.lock() else {
            return;
        };

        let contained = contain_guard.get_contained_objects().to_vec();
        for passenger_id in contained {
            let already_present =
                (0..self.current_members).any(|idx| self.member_ids[idx] == passenger_id);
            if already_present {
                continue;
            }

            if self.current_members >= MAX_TRANSPORT_SLOTS {
                continue;
            }

            self.member_ids[self.current_members] = passenger_id;
            if let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id) {
                if let Ok(passenger_guard) = passenger.read() {
                    if let Some(ai) = passenger_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard.set_allow_chase(true);
                        }
                    }
                    if self.is_member_wounded(&passenger_guard) {
                        self.member_healing[self.current_members] = true;
                    }
                }
            }

            if self.new_occupants_are_new_members {
                self.new_member[self.current_members] = true;
            }
            self.current_members += 1;
        }

        self.new_occupants_are_new_members = true;
    }

    fn is_attack_pointless(&self) -> Bool {
        let Some(transport) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return false;
        };
        let Ok(transport_guard) = transport.read() else {
            return false;
        };

        if transport_guard.is_attacking() {
            for i in 0..self.current_members {
                if !self.new_member[i] {
                    return false;
                }
            }
            return true;
        }
        false
    }

    fn is_member_wounded(&self, member: &crate::object::Object) -> Bool {
        let Some(body) = member.get_body() else {
            return false;
        };
        let Ok(body_guard) = body.lock() else {
            return false;
        };
        let ratio = body_guard.get_health() / body_guard.get_max_health();
        ratio < self.data.members_get_healed_at_life_ratio
    }

    fn is_member_healthy(&self, member: &crate::object::Object) -> Bool {
        let Some(body) = member.get_body() else {
            return false;
        };
        let Ok(body_guard) = body.lock() else {
            return false;
        };
        body_guard.get_health() == body_guard.get_max_health()
    }

    fn retrieve_members(&self) {
        for i in 0..self.current_members {
            let Some(member) = TheGameLogic::find_object_by_id(self.member_ids[i]) else {
                continue;
            };
            let Ok(member_guard) = member.read() else {
                continue;
            };
            let contained = member_guard.get_contained_by().is_some();
            drop(member_guard);
            if !contained {
                if let Some(ai) = member.read().ok().and_then(|guard| guard.get_ai()) {
                    let should_enter = ai.lock().ok().and_then(|guard| guard.get_current_command())
                        != Some(AiCommandType::Enter);
                    if should_enter {
                        ai.ai_enter(self.owner_id, CommandSourceType::FromAi);
                    }
                }
            }
        }
    }

    fn give_final_orders(&self) {
        for i in 0..self.current_members {
            let Some(member) = TheGameLogic::find_object_by_id(self.member_ids[i]) else {
                continue;
            };
            let Ok(member_guard) = member.read() else {
                continue;
            };
            let Some(ai) = member_guard.get_ai() else {
                continue;
            };

            if self.is_attack_object {
                if let Some(target) = get_legacy_object(self.designated_target) {
                    ai.ai_attack_object(
                        target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                        NO_MAX_SHOTS_LIMIT,
                        CommandSourceType::FromPlayer,
                    );
                }
            } else if self.is_attack_move {
                ai.ai_attack_move_to_position(
                    &self.attack_move_goal_pos,
                    NO_MAX_SHOTS_LIMIT,
                    CommandSourceType::FromPlayer,
                );
            }

            {
                let Ok(mut ai_guard) = ai.lock() else {
                    continue;
                };
                ai_guard.set_allow_chase(false);
            }
        }
    }
}

impl AssaultTransportAIUpdateInterface for AssaultTransportAIUpdate {
    fn begin_assault(&mut self, designated_target: Option<ObjectID>) {
        if let Some(target) = designated_target {
            self.designated_target = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assault_transport_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut data = AssaultTransportAIUpdateModuleData::default();

        parse_auto_acquire_field(&mut ini, &mut data, &["=", "YES", "ATTACK_BUILDINGS"]).unwrap();
        parse_duration_field(
            &mut |value| data.base.set_mood_attack_check_rate(value),
            &["=", "2000"],
        )
        .unwrap();
        parse_duration_field(
            &mut |value| data.base.set_surrender_duration_frames(value),
            &["=", "3000"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_forbid_player_commands(value),
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_turrets_linked(value),
            &["=", "Yes"],
        )
        .unwrap();
        parse_real_field(
            &mut |value| data.members_get_healed_at_life_ratio = value,
            &["=", "0.6"],
        )
        .unwrap();
        parse_real_field(
            &mut |value| data.clear_range_required_to_continue_attack_move = value,
            &["=", "125.0"],
        )
        .unwrap();

        assert_eq!(data.base.auto_acquire_enemies_when_idle(), 0b10001);
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert!((data.members_get_healed_at_life_ratio - 0.6).abs() < f32::EPSILON);
        assert!((data.clear_range_required_to_continue_attack_move - 125.0).abs() < f32::EPSILON);
    }
}
