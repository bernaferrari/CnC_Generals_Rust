//! DeployStyleAIUpdate - AI update logic for deploy/pack units.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/DeployStyleAIUpdate.cpp.

use std::any::Any;
use std::sync::Arc;

use crate::ai::states::AIStateType;
use crate::common::{
    Bool, ObjectID, ObjectStatusMaskType, UnsignedInt, MODELCONDITION_DEPLOYED,
    MODELCONDITION_MOVING, MODELCONDITION_PACKING, MODELCONDITION_UNPACKING,
};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::modules::AIUpdateInterface;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeployStateType {
    ReadyToMove,
    Deploy,
    ReadyToAttack,
    Undeploy,
    AligningTurrets,
}

/// Module data for DeployStyleAIUpdate.
#[derive(Debug, Clone)]
pub struct DeployStyleAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub unpack_time: UnsignedInt,
    pub pack_time: UnsignedInt,
    pub reset_turret_before_packing: Bool,
    pub turrets_function_only_when_deployed: Bool,
    pub turrets_must_center_before_packing: Bool,
    pub manual_deploy_animations: Bool,
}

impl Default for DeployStyleAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            unpack_time: 0,
            pack_time: 0,
            reset_turret_before_packing: false,
            turrets_function_only_when_deployed: false,
            turrets_must_center_before_packing: false,
            manual_deploy_animations: false,
        }
    }
}

impl DeployStyleAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEPLOY_STYLE_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for DeployStyleAIUpdateModuleData {
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

impl Snapshotable for DeployStyleAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.unpack_time))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.pack_time))?;
        xfer_io(xfer.xfer_bool(&mut self.reset_turret_before_packing))?;
        xfer_io(xfer.xfer_bool(&mut self.turrets_function_only_when_deployed))?;
        xfer_io(xfer.xfer_bool(&mut self.turrets_must_center_before_packing))?;
        xfer_io(xfer.xfer_bool(&mut self.manual_deploy_animations))?;
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
    data: &mut DeployStyleAIUpdateModuleData,
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

const DEPLOY_STYLE_AI_UPDATE_FIELDS: &[FieldParse<DeployStyleAIUpdateModuleData>] = &[
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
        token: "UnpackTime",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.unpack_time = v, tokens),
    },
    FieldParse {
        token: "PackTime",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.pack_time = v, tokens),
    },
    FieldParse {
        token: "ResetTurretBeforePacking",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.reset_turret_before_packing = v, tokens)
        },
    },
    FieldParse {
        token: "TurretsFunctionOnlyWhenDeployed",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |v| data.turrets_function_only_when_deployed = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsMustCenterBeforePacking",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.turrets_must_center_before_packing = v, tokens)
        },
    },
    FieldParse {
        token: "ManualDeployAnimations",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.manual_deploy_animations = v, tokens)
        },
    },
];

/// Module wrapper for DeployStyleAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct DeployStyleAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<DeployStyleAIUpdateModuleData>,
}

impl DeployStyleAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<DeployStyleAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_style_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut data = DeployStyleAIUpdateModuleData::default();

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
        parse_duration_field(&mut |value| data.unpack_time = value, &["=", "1500"]).unwrap();
        parse_duration_field(&mut |value| data.pack_time = value, &["=", "2500"]).unwrap();
        parse_bool_field(
            &mut |value| data.reset_turret_before_packing = value,
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.turrets_function_only_when_deployed = value,
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.turrets_must_center_before_packing = value,
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.manual_deploy_animations = value,
            &["=", "Yes"],
        )
        .unwrap();

        assert_eq!(data.base.auto_acquire_enemies_when_idle(), 0b10001);
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.unpack_time, 45);
        assert_eq!(data.pack_time, 75);
        assert!(data.reset_turret_before_packing);
        assert!(data.turrets_function_only_when_deployed);
        assert!(data.turrets_must_center_before_packing);
        assert!(data.manual_deploy_animations);
    }
}

impl Module for DeployStyleAIUpdateModule {
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

impl Snapshotable for DeployStyleAIUpdateModule {
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

/// Runtime data for DeployStyleAIUpdate.
#[derive(Debug, Clone)]
pub struct DeployStyleAIUpdateData {
    pub unpack_time: UnsignedInt,
    pub pack_time: UnsignedInt,
    pub reset_turret_before_packing: Bool,
    pub turrets_function_only_when_deployed: Bool,
    pub turrets_must_center_before_packing: Bool,
    pub manual_deploy_animations: Bool,
}

impl Default for DeployStyleAIUpdateData {
    fn default() -> Self {
        Self {
            unpack_time: 0,
            pack_time: 0,
            reset_turret_before_packing: false,
            turrets_function_only_when_deployed: false,
            turrets_must_center_before_packing: false,
            manual_deploy_animations: false,
        }
    }
}

/// Deploy style AI update logic.
#[derive(Debug, Clone)]
pub struct DeployStyleAIUpdate {
    data: DeployStyleAIUpdateData,
    owner_id: ObjectID,
    state: DeployStateType,
    frame_to_wait_for_deploy: UnsignedInt,
}

impl DeployStyleAIUpdate {
    pub fn new(data: DeployStyleAIUpdateData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            state: DeployStateType::ReadyToMove,
            frame_to_wait_for_deploy: 0,
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };

        let weapon = owner_guard.get_current_weapon().map(|(weapon, _)| weapon);
        let now = TheGameLogic::get_frame();

        let is_trying_to_move = ai.is_waiting_for_path() || ai.get_path().is_some();
        let is_trying_to_attack = ai.is_in_attack_state();
        let is_in_guard_idle_state = ai.is_in_guard_idle_state();

        let mut is_in_range = false;
        if is_trying_to_attack {
            if let Some(weapon) = weapon {
                let _source_pos = owner_guard.get_position();
                let mut target_pos = None;

                if let Some(victim_id) = ai.get_current_victim() {
                    if let Some(victim) = TheGameLogic::find_object_by_id(victim_id) {
                        if let Ok(victim_guard) = victim.read() {
                            target_pos = Some(*victim_guard.get_position());
                        }
                    }
                }

                if target_pos.is_none() {
                    target_pos = ai.get_original_victim_pos();
                }

                if let Some(target_pos) = target_pos {
                    is_in_range = weapon.is_within_attack_range(
                        owner_guard.get_id(),
                        ai.get_current_victim(),
                        Some(&target_pos),
                    );
                }
            }
        }

        if self.frame_to_wait_for_deploy != 0 && now >= self.frame_to_wait_for_deploy {
            match self.state {
                DeployStateType::Deploy => self.set_my_state(DeployStateType::ReadyToAttack, false),
                DeployStateType::Undeploy => self.set_my_state(DeployStateType::ReadyToMove, false),
                _ => {}
            }
        }

        if is_in_range || is_in_guard_idle_state {
            match self.state {
                DeployStateType::ReadyToMove => {
                    self.set_my_state(DeployStateType::Deploy, false);
                }
                DeployStateType::ReadyToAttack => {}
                DeployStateType::Deploy => {}
                DeployStateType::Undeploy => {
                    if self.frame_to_wait_for_deploy != 0 {
                        self.set_my_state(DeployStateType::Deploy, true);
                    }
                }
                DeployStateType::AligningTurrets => {
                    self.set_my_state(DeployStateType::ReadyToAttack, false);
                }
            }
        } else if is_trying_to_move {
            match self.state {
                DeployStateType::ReadyToMove => {}
                DeployStateType::ReadyToAttack => {
                    let turret = ai.get_which_turret_for_cur_weapon();
                    if turret != crate::common::TurretType::Invalid
                        && self.data.turrets_must_center_before_packing
                    {
                        self.set_my_state(DeployStateType::AligningTurrets, false);
                    } else {
                        self.set_my_state(DeployStateType::Undeploy, false);
                    }
                }
                DeployStateType::Deploy => {
                    if self.frame_to_wait_for_deploy != 0 {
                        self.set_my_state(DeployStateType::Undeploy, true);
                    }
                }
                DeployStateType::Undeploy => {}
                DeployStateType::AligningTurrets => {
                    let turret = ai.get_which_turret_for_cur_weapon();
                    if turret != crate::common::TurretType::Invalid
                        && ai.is_turret_in_natural_position(turret)
                    {
                        self.set_my_state(DeployStateType::Undeploy, false);
                    }
                }
            }
        }

        drop(owner_guard);
        if let Ok(mut owner_guard) = owner.write() {
            match self.state {
                DeployStateType::ReadyToMove => {
                    if is_trying_to_move {
                        owner_guard.set_model_condition_state(MODELCONDITION_MOVING);
                    }
                }
                DeployStateType::ReadyToAttack => {}
                DeployStateType::Deploy => {
                    if self.data.manual_deploy_animations {
                        let total_frames = self.get_pack_time();
                        let frames_left = self.frame_to_wait_for_deploy.saturating_sub(now);
                        owner_guard.set_animation_frame((total_frames - frames_left) as i32);
                    }
                }
                DeployStateType::Undeploy => {
                    if self.data.manual_deploy_animations {
                        let frames_left = self.frame_to_wait_for_deploy.saturating_sub(now);
                        owner_guard.set_animation_frame(frames_left as i32);
                    }
                }
                DeployStateType::AligningTurrets => {}
            }
        }

        if matches!(
            self.state,
            DeployStateType::Deploy | DeployStateType::Undeploy | DeployStateType::AligningTurrets
        ) {
            ai.set_temporary_state(AIStateType::Busy, 0);
            ai.set_locomotor_goal_none();
        }

        Ok(())
    }

    fn get_unpack_time(&self) -> UnsignedInt {
        self.data.unpack_time
    }

    fn get_pack_time(&self) -> UnsignedInt {
        self.data.pack_time
    }

    fn set_my_state(&mut self, state: DeployStateType, reverse_deploy: Bool) {
        self.state = state;
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };
        let ai = owner_guard.get_ai_update_interface();
        let now = TheGameLogic::get_frame();
        let mut turret_action: Option<(crate::common::TurretType, bool, bool)> = None;

        match state {
            DeployStateType::Deploy => {
                let clear = MODELCONDITION_PACKING;
                let set = MODELCONDITION_UNPACKING;
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);

                if reverse_deploy {
                    let total_frames = self.get_unpack_time();
                    let frames_left = self.frame_to_wait_for_deploy.saturating_sub(now);
                    self.frame_to_wait_for_deploy = now + total_frames.saturating_sub(frames_left);
                    if self.data.manual_deploy_animations {
                        owner_guard.set_animation_frame((total_frames - frames_left) as i32);
                    }
                } else {
                    self.frame_to_wait_for_deploy = self.get_unpack_time() + now;
                }

                if let Some(mut sound) = owner_guard.get_template().get_per_unit_sound("Deploy") {
                    sound.set_object_id(owner_guard.get_id());
                    if let Some(audio) = TheAudio::get() {
                        let _ = audio.add_audio_event(&sound);
                    }
                }
            }
            DeployStateType::Undeploy => {
                owner_guard.clear_status(ObjectStatusMaskType::DEPLOYED);
                let clear = MODELCONDITION_UNPACKING | MODELCONDITION_DEPLOYED;
                let set = MODELCONDITION_PACKING;
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);

                if reverse_deploy {
                    let total_frames = self.get_unpack_time();
                    let frames_left = self.frame_to_wait_for_deploy.saturating_sub(now);
                    self.frame_to_wait_for_deploy = now + total_frames.saturating_sub(frames_left);
                    if self.data.manual_deploy_animations {
                        owner_guard.set_animation_frame(frames_left as i32);
                    }
                } else {
                    self.frame_to_wait_for_deploy = self.get_pack_time() + now;
                }

                if self.data.turrets_function_only_when_deployed {
                    let turret = ai
                        .as_ref()
                        .and_then(|ai| {
                            ai.lock()
                                .ok()
                                .map(|guard| guard.get_which_turret_for_cur_weapon())
                        })
                        .unwrap_or(crate::common::TurretType::Invalid);
                    if turret != crate::common::TurretType::Invalid {
                        turret_action = Some((turret, false, false));
                    }
                }

                if let Some(mut sound) = owner_guard.get_template().get_per_unit_sound("Undeploy") {
                    sound.set_object_id(owner_guard.get_id());
                    if let Some(audio) = TheAudio::get() {
                        let _ = audio.add_audio_event(&sound);
                    }
                }
            }
            DeployStateType::ReadyToMove => {
                self.frame_to_wait_for_deploy = 0;
                if let Err(err) = owner_guard.clear_model_condition_flags(MODELCONDITION_PACKING) {
                    log::warn!(
                        "DeployStyleAIUpdate: failed clearing PACKING model condition for object {}: {}",
                        owner_guard.get_id(),
                        err
                    );
                }
            }
            DeployStateType::ReadyToAttack => {
                owner_guard.set_status(ObjectStatusMaskType::DEPLOYED, true);
                self.frame_to_wait_for_deploy = 0;
                let clear = MODELCONDITION_UNPACKING;
                let set = MODELCONDITION_DEPLOYED;
                let _ = owner_guard.clear_and_set_model_condition_flags(clear, set);

                if self.data.turrets_function_only_when_deployed {
                    let turret = ai
                        .as_ref()
                        .and_then(|ai| {
                            ai.lock()
                                .ok()
                                .map(|guard| guard.get_which_turret_for_cur_weapon())
                        })
                        .unwrap_or(crate::common::TurretType::Invalid);
                    if turret != crate::common::TurretType::Invalid {
                        turret_action = Some((turret, true, false));
                    }
                }
            }
            DeployStateType::AligningTurrets => {
                self.frame_to_wait_for_deploy = 0;
                let turret = ai
                    .as_ref()
                    .and_then(|ai| {
                        ai.lock()
                            .ok()
                            .map(|guard| guard.get_which_turret_for_cur_weapon())
                    })
                    .unwrap_or(crate::common::TurretType::Invalid);
                if turret != crate::common::TurretType::Invalid {
                    turret_action = Some((turret, true, true));
                }
            }
        }

        drop(owner_guard);
        if let (Some(ai), Some((turret, enable, recenter))) = (ai.as_ref(), turret_action) {
            if let Ok(mut guard) = ai.lock() {
                if recenter {
                    guard.recenter_turret(turret);
                } else {
                    guard.set_turret_enabled(turret, enable);
                }
            }
        }
    }
}
