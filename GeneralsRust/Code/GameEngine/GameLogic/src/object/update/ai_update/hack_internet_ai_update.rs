//! HackInternetAIUpdate - AI update logic for internet hacking (cash generation).
//!
//! Ported from GameLogic/Object/Update/AIUpdate/HackInternetAIUpdate.cpp.

use std::any::Any;
use std::sync::Arc;

use crate::ai::AiCommandParams;
use crate::common::CommandSourceType;
use crate::common::{
    Bool, Color, GameLogicRandomValueReal, Int, ObjectID, Real, UnsignedInt, VeterancyLevel,
};
use crate::helpers::{
    game_client_random_value_real, TheAudio, TheGameLogic, TheGameText, TheInGameUI,
};
use crate::modules::{AIUpdateInterface, HackInternetAIUpdateInterface};
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
enum HackInternetState {
    Idle,
    Unpacking { frames_remaining: UnsignedInt },
    Hacking { frames_remaining: UnsignedInt },
    Packing { frames_remaining: UnsignedInt },
}

/// HackInternetAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct HackInternetAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub unpack_time: UnsignedInt,
    pub pack_time: UnsignedInt,
    pub cash_update_delay: UnsignedInt,
    pub cash_update_delay_fast: UnsignedInt,
    pub regular_cash_amount: UnsignedInt,
    pub veteran_cash_amount: UnsignedInt,
    pub elite_cash_amount: UnsignedInt,
    pub heroic_cash_amount: UnsignedInt,
    pub xp_per_cash_update: UnsignedInt,
    pub pack_unpack_variation_factor: Real,
}

impl Default for HackInternetAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            unpack_time: 0,
            pack_time: 0,
            cash_update_delay: 0,
            cash_update_delay_fast: 0,
            regular_cash_amount: 0,
            veteran_cash_amount: 0,
            elite_cash_amount: 0,
            heroic_cash_amount: 0,
            xp_per_cash_update: 0,
            pack_unpack_variation_factor: 0.0,
        }
    }
}

impl HackInternetAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HACK_INTERNET_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for HackInternetAIUpdateModuleData {
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

impl Snapshotable for HackInternetAIUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.unpack_time))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.pack_time))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.cash_update_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.cash_update_delay_fast))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.regular_cash_amount))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.veteran_cash_amount))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.elite_cash_amount))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.heroic_cash_amount))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.xp_per_cash_update))?;
        xfer_io(xfer.xfer_real(&mut self.pack_unpack_variation_factor))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut HackInternetAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a str>, INIError> {
    let values: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if values.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(values)
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

fn parse_unsigned_int_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_unsigned_int(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

const HACK_INTERNET_AI_UPDATE_FIELDS: &[FieldParse<HackInternetAIUpdateModuleData>] = &[
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
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.unpack_time = value, tokens)
        },
    },
    FieldParse {
        token: "PackTime",
        parse: |_, data, tokens| parse_duration_field(&mut |value| data.pack_time = value, tokens),
    },
    FieldParse {
        token: "PackUnpackVariationFactor",
        parse: |_, data, tokens| {
            parse_real_field(
                &mut |value| data.pack_unpack_variation_factor = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "CashUpdateDelay",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.cash_update_delay = value, tokens)
        },
    },
    FieldParse {
        token: "CashUpdateDelayFast",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.cash_update_delay_fast = value, tokens)
        },
    },
    FieldParse {
        token: "RegularCashAmount",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |value| data.regular_cash_amount = value, tokens)
        },
    },
    FieldParse {
        token: "VeteranCashAmount",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |value| data.veteran_cash_amount = value, tokens)
        },
    },
    FieldParse {
        token: "EliteCashAmount",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |value| data.elite_cash_amount = value, tokens)
        },
    },
    FieldParse {
        token: "HeroicCashAmount",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |value| data.heroic_cash_amount = value, tokens)
        },
    },
    FieldParse {
        token: "XpPerCashUpdate",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |value| data.xp_per_cash_update = value, tokens)
        },
    },
];

/// Module wrapper for HackInternetAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct HackInternetAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<HackInternetAIUpdateModuleData>,
}

impl HackInternetAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<HackInternetAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for HackInternetAIUpdateModule {
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

impl Snapshotable for HackInternetAIUpdateModule {
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

/// Runtime configuration for HackInternetAIUpdate.
#[derive(Debug, Clone)]
pub struct HackInternetAIUpdateData {
    pub unpack_time: UnsignedInt,
    pub pack_time: UnsignedInt,
    pub cash_update_delay: UnsignedInt,
    pub cash_update_delay_fast: UnsignedInt,
    pub regular_cash_amount: UnsignedInt,
    pub veteran_cash_amount: UnsignedInt,
    pub elite_cash_amount: UnsignedInt,
    pub heroic_cash_amount: UnsignedInt,
    pub xp_per_cash_update: UnsignedInt,
    pub pack_unpack_variation_factor: Real,
}

impl Default for HackInternetAIUpdateData {
    fn default() -> Self {
        Self {
            unpack_time: 0,
            pack_time: 0,
            cash_update_delay: 0,
            cash_update_delay_fast: 0,
            regular_cash_amount: 0,
            veteran_cash_amount: 0,
            elite_cash_amount: 0,
            heroic_cash_amount: 0,
            xp_per_cash_update: 0,
            pack_unpack_variation_factor: 0.0,
        }
    }
}

/// HackInternet AI runtime logic.
#[derive(Debug, Clone)]
pub struct HackInternetAIUpdate {
    data: HackInternetAIUpdateData,
    owner_id: ObjectID,
    state: HackInternetState,
    pending_command: Option<AiCommandParams>,
}

impl HackInternetAIUpdate {
    pub fn new(data: HackInternetAIUpdateData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            state: HackInternetState::Idle,
            pending_command: None,
        }
    }

    pub fn has_pending_command(&self) -> bool {
        self.pending_command.is_some()
    }

    pub fn is_hacking_state(&self) -> bool {
        matches!(self.state, HackInternetState::Hacking { .. })
    }

    pub fn is_hacking_packing_or_unpacking_state(&self) -> bool {
        matches!(
            self.state,
            HackInternetState::Hacking { .. }
                | HackInternetState::Packing { .. }
                | HackInternetState::Unpacking { .. }
        )
    }

    pub fn handle_command(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn AIUpdateInterface,
    ) -> bool {
        if matches!(
            self.state,
            HackInternetState::Hacking { .. } | HackInternetState::Packing { .. }
        ) {
            self.pending_command = Some(params.clone());
            if matches!(self.state, HackInternetState::Hacking { .. }) {
                ai.set_last_command_source(CommandSourceType::FromAi);
                self.enter_packing();
            }
            return true;
        }
        false
    }

    pub fn hack_internet(&mut self) {
        self.enter_unpacking();
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if matches!(self.state, HackInternetState::Idle) {
            if let Some(command) = self.pending_command.clone() {
                if ai.is_idle_unrestricted() {
                    self.pending_command = None;
                    let _ = ai.execute_command(&command);
                }
            }
        }

        match self.state {
            HackInternetState::Idle => {}
            HackInternetState::Unpacking { frames_remaining } => {
                self.update_unpacking(frames_remaining);
            }
            HackInternetState::Packing { frames_remaining } => {
                self.update_packing(frames_remaining);
            }
            HackInternetState::Hacking { frames_remaining } => {
                self.update_hacking(frames_remaining)?;
            }
        }

        Ok(())
    }

    fn enter_unpacking(&mut self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };

        if let Err(err) = owner_guard.clear_model_condition_flags(
            crate::common::ModelConditionFlags::Packing
                | crate::common::ModelConditionFlags::FiringA
                | crate::common::ModelConditionFlags::Unpacking,
        ) {
            log::debug!(
                "HackInternetAIUpdate::enter_unpacking clear_model_condition_flags failed: {}",
                err
            );
        }
        owner_guard.set_model_condition_state(crate::common::ModelConditionFlags::Unpacking);

        if let Some(mut sound) = owner_guard.get_template().get_per_unit_sound("UnitUnpack") {
            sound.set_object_id(owner_guard.get_id());
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&sound);
            }
        }

        let variation = self.random_pack_unpack_variation();
        let frames = (self.data.unpack_time as Real * variation) as UnsignedInt;
        owner_guard.set_animation_loop_duration(frames);

        self.state = HackInternetState::Unpacking {
            frames_remaining: frames,
        };
    }

    fn enter_packing(&mut self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };

        let clear = crate::common::ModelConditionFlags::FiringA;
        let set = crate::common::ModelConditionFlags::Packing;
        if let Err(err) = owner_guard.clear_and_set_model_condition_flags(clear, set) {
            log::debug!(
                "HackInternetAIUpdate::enter_packing clear_and_set_model_condition_flags failed: {}",
                err
            );
        }

        if let Some(mut sound) = owner_guard.get_template().get_per_unit_sound("UnitPack") {
            sound.set_object_id(owner_guard.get_id());
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&sound);
            }
        }

        let variation = self.random_pack_unpack_variation();
        let frames = (self.get_pack_time() as Real * variation) as UnsignedInt;
        owner_guard.set_animation_loop_duration(frames);

        self.state = HackInternetState::Packing {
            frames_remaining: frames,
        };
    }

    fn enter_hacking(&mut self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };

        let clear = crate::common::ModelConditionFlags::Unpacking;
        let set = crate::common::ModelConditionFlags::FiringA;
        if let Err(err) = owner_guard.clear_and_set_model_condition_flags(clear, set) {
            log::debug!(
                "HackInternetAIUpdate::enter_hacking clear_and_set_model_condition_flags failed: {}",
                err
            );
        }

        self.state = HackInternetState::Hacking {
            frames_remaining: self.get_cash_update_delay(),
        };
    }

    fn update_unpacking(&mut self, frames_remaining: UnsignedInt) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };

        owner_guard.set_model_condition_state(crate::common::ModelConditionFlags::Unpacking);

        if frames_remaining > 0 {
            self.state = HackInternetState::Unpacking {
                frames_remaining: frames_remaining.saturating_sub(1),
            };
        } else {
            owner_guard.clear_model_condition_state(crate::common::ModelConditionFlags::Unpacking);
            self.enter_hacking();
        }
    }

    fn update_packing(&mut self, frames_remaining: UnsignedInt) {
        if frames_remaining > 0 {
            self.state = HackInternetState::Packing {
                frames_remaining: frames_remaining.saturating_sub(1),
            };
        } else {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::Packing);
                }
            }
            self.state = HackInternetState::Idle;
        }
    }

    fn update_hacking(
        &mut self,
        frames_remaining: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };

        if owner_guard.is_disabled_by_type(crate::common::DisabledType::DisabledHacked) {
            return Ok(());
        }

        if frames_remaining > 0 {
            self.state = HackInternetState::Hacking {
                frames_remaining: frames_remaining.saturating_sub(1),
            };
            return Ok(());
        }

        drop(owner_guard);
        self.do_cash_update()?;
        self.state = HackInternetState::Hacking {
            frames_remaining: self.get_cash_update_delay(),
        };
        Ok(())
    }

    fn do_cash_update(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };

        let Some(player) = owner_guard.get_controlling_player() else {
            return Ok(());
        };
        let mut player_guard = player.write().map_err(|_| "player lock poisoned")?;

        let amount = self.cash_amount_for_level(owner_guard.get_veterancy_level());
        player_guard.get_money_mut().add_money(amount as Int);
        player_guard
            .get_score_keeper_mut()
            .add_money_earned(amount as u32);

        if let Some(tracker) = owner_guard.get_experience_tracker() {
            if let Ok(mut tracker_guard) = tracker.lock() {
                let _ = tracker_guard.add_experience_points(
                    self.data.xp_per_cash_update as i32,
                    false,
                    &crate::experience::ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                );
            }
        }

        let mut display_money = true;
        if owner_guard.test_status(crate::common::ObjectStatusTypes::Stealthed) {
            if !owner_guard.is_locally_controlled()
                && !owner_guard.test_status(crate::common::ObjectStatusTypes::Detected)
            {
                display_money = false;
            }
        }

        if let Some(container_id) = owner_guard.get_contained_by() {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if container_guard.test_status(crate::common::ObjectStatusTypes::Stealthed) {
                        if !container_guard.is_locally_controlled()
                            && !container_guard
                                .test_status(crate::common::ObjectStatusTypes::Detected)
                        {
                            display_money = false;
                        }
                    }
                }
            }
        }

        if display_money {
            let caption = format_add_cash(amount as Int);
            let mut pos = *owner_guard.get_position();
            pos.z += 20.0;

            if let Some(container_id) = owner_guard.get_contained_by() {
                if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                    if let Ok(container_guard) = container.read() {
                        let geom = container_guard.get_geometry_info();
                        let width = geom.get_major_radius() * 0.3;
                        let depth = geom.get_minor_radius() * 0.3;
                        pos.x += game_client_random_value_real(-width, width);
                        pos.y += game_client_random_value_real(-depth, depth);
                    }
                }
            }

            let _ = TheInGameUI::add_floating_text(&caption, &pos, Color::new(0, 255, 0, 255));
        }

        if let Some(mut sound) = owner_guard
            .get_template()
            .get_per_unit_sound("UnitCashPing")
        {
            sound.set_object_id(owner_guard.get_id());
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&sound);
            }
        }

        Ok(())
    }

    fn get_pack_time(&self) -> UnsignedInt {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return self.data.pack_time;
        };
        let Ok(owner_guard) = owner.read() else {
            return self.data.pack_time;
        };
        if owner_guard.get_contained_by().is_some() {
            return 0;
        }
        self.data.pack_time
    }

    fn get_cash_update_delay(&self) -> UnsignedInt {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return self.data.cash_update_delay;
        };
        let Ok(owner_guard) = owner.read() else {
            return self.data.cash_update_delay;
        };
        if owner_guard.get_contained_by().is_some() {
            return self.data.cash_update_delay_fast;
        }
        self.data.cash_update_delay
    }

    fn cash_amount_for_level(&self, level: VeterancyLevel) -> UnsignedInt {
        let mut amount = match level {
            VeterancyLevel::Heroic => self.data.heroic_cash_amount,
            VeterancyLevel::Elite => self.data.elite_cash_amount,
            VeterancyLevel::Veteran => self.data.veteran_cash_amount,
            VeterancyLevel::Regular => self.data.regular_cash_amount,
        };

        if amount == 0 {
            if level == VeterancyLevel::Heroic {
                amount = self.data.elite_cash_amount;
            }
            if amount == 0 && level >= VeterancyLevel::Elite {
                amount = self.data.veteran_cash_amount;
            }
            if amount == 0 && level >= VeterancyLevel::Veteran {
                amount = self.data.regular_cash_amount;
            }
            if amount == 0 {
                amount = 1;
            }
        }

        amount
    }

    fn random_pack_unpack_variation(&self) -> Real {
        let factor = self.data.pack_unpack_variation_factor;
        if factor <= 0.0 {
            return 1.0;
        }
        GameLogicRandomValueReal(1.0 - factor, 1.0 + factor)
    }
}

fn format_add_cash(amount: Int) -> String {
    let template = TheGameText::fetch("GUI:AddCash");
    if template.contains("%d") || template.contains("%i") {
        template
            .replace("%d", &amount.to_string())
            .replace("%i", &amount.to_string())
    } else if template.contains("%f") {
        template.replace("%f", &format!("{:.0}", amount))
    } else {
        format!("+${}", amount)
    }
}

impl HackInternetAIUpdateInterface for HackInternetAIUpdate {
    fn is_hacking(&self) -> bool {
        self.is_hacking_state()
    }

    fn is_hacking_packing_or_unpacking(&self) -> bool {
        self.is_hacking_packing_or_unpacking_state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_field(data: &mut HackInternetAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = HACK_INTERNET_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn hack_internet_fields_accept_ini_equals_token() {
        let mut data = HackInternetAIUpdateModuleData::default();

        parse_field(
            &mut data,
            "AutoAcquireEnemiesWhenIdle",
            &["=", "YES", "ATTACK_BUILDINGS"],
        );
        parse_field(&mut data, "MoodAttackCheckRate", &["=", "2000"]);
        parse_field(&mut data, "SurrenderDuration", &["=", "3000"]);
        parse_field(&mut data, "ForbidPlayerCommands", &["=", "Yes"]);
        parse_field(&mut data, "TurretsLinked", &["=", "Yes"]);
        parse_field(&mut data, "UnpackTime", &["=", "1200"]);
        parse_field(&mut data, "PackTime", &["=", "900"]);
        parse_field(&mut data, "PackUnpackVariationFactor", &["=", "0.25"]);
        parse_field(&mut data, "CashUpdateDelay", &["=", "4000"]);
        parse_field(&mut data, "CashUpdateDelayFast", &["=", "1000"]);
        parse_field(&mut data, "RegularCashAmount", &["=", "5"]);
        parse_field(&mut data, "VeteranCashAmount", &["=", "6"]);
        parse_field(&mut data, "EliteCashAmount", &["=", "7"]);
        parse_field(&mut data, "HeroicCashAmount", &["=", "8"]);
        parse_field(&mut data, "XpPerCashUpdate", &["=", "9"]);

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.unpack_time, 36);
        assert_eq!(data.pack_time, 27);
        assert_eq!(data.pack_unpack_variation_factor, 0.25);
        assert_eq!(data.cash_update_delay, 120);
        assert_eq!(data.cash_update_delay_fast, 30);
        assert_eq!(data.regular_cash_amount, 5);
        assert_eq!(data.veteran_cash_amount, 6);
        assert_eq!(data.elite_cash_amount, 7);
        assert_eq!(data.heroic_cash_amount, 8);
        assert_eq!(data.xp_per_cash_update, 9);
    }
}
