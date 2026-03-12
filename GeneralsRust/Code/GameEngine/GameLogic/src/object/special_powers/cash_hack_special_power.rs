// Port of CashHackSpecialPower.h and CashHackSpecialPower.cpp
// Author: Rust Port
// Desc: The Cash Hack will steal money from an enemy player

use crate::common::science::ScienceType;
use crate::common::{AsciiString, Color, Coord3D, LegacyModuleData};
use crate::helpers::{TheGameLogic, TheGameText, TheInGameUI};
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface as ObjSpecialPowerModuleInterface, Waypoint,
};

use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::SpecialPowerTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::get_science_store;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Upgrade pair for cash hack with science requirement
#[derive(Debug, Clone)]
pub struct CashHackUpgrade {
    /// Science required for this upgrade
    pub science: ScienceType,
    /// Amount to steal when this science is available
    pub amount_to_steal: i32,
}

/// Module data for cash hack special power
#[derive(Debug, Clone)]
pub struct CashHackSpecialPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,

    /// Upgrade amounts based on science
    pub upgrades: Vec<CashHackUpgrade>,

    /// Default amount to steal
    pub default_amount_to_steal: i32,
}

impl Default for CashHackSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            upgrades: Vec::new(),
            default_amount_to_steal: 0,
        }
    }
}

impl CashHackSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CASH_HACK_SPECIAL_POWER_FIELDS)
    }
}

impl Snapshotable for CashHackSpecialPowerModuleData {
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

crate::impl_legacy_module_data_with_key_field!(CashHackSpecialPowerModuleData, module_tag_name_key);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*token);
    data.base.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_money_amount(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.default_amount_to_steal = INI::parse_int(token)?;
    Ok(())
}

fn parse_upgrade_money_amount(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 2 {
        return Err(INIError::InvalidData);
    }
    let store = get_science_store().ok_or(INIError::InvalidData)?;
    let science = store.get_science_from_internal_name(tokens[0].trim());
    let amount = INI::parse_int(tokens[1])?;
    data.upgrades.push(CashHackUpgrade {
        science,
        amount_to_steal: amount,
    });
    Ok(())
}

const CASH_HACK_SPECIAL_POWER_FIELDS: &[FieldParse<CashHackSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "UpdateModuleStartsAttack",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.update_module_starts_attack = v, tokens)
        },
    },
    FieldParse {
        token: "StartsPaused",
        parse: |_, data, tokens| parse_bool_field(&mut |v| data.base.starts_paused = v, tokens),
    },
    FieldParse {
        token: "InitiateSound",
        parse: parse_audio_event,
    },
    FieldParse {
        token: "ScriptedSpecialPowerOnly",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.scripted_special_power_only = v, tokens)
        },
    },
    FieldParse {
        token: "MoneyAmount",
        parse: parse_money_amount,
    },
    FieldParse {
        token: "UpgradeMoneyAmount",
        parse: parse_upgrade_money_amount,
    },
];

/// Cash hack special power implementation
/// Steals money from an enemy player's building
#[derive(Debug, Clone)]
pub struct CashHackSpecialPower {
    /// Base special power module
    base: SpecialPowerModule,

    /// Cash hack-specific module data
    cash_hack_data: Arc<CashHackSpecialPowerModuleData>,
    module_name_key: NameKeyType,
}

impl CashHackSpecialPower {
    /// Create a new cash hack special power
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<CashHackSpecialPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            cash_hack_data: data,
            module_name_key,
        }
    }

    /// Get the cash hack module data
    pub fn get_cash_hack_module_data(&self) -> &CashHackSpecialPowerModuleData {
        &self.cash_hack_data
    }

    /// Find the amount to steal based on available sciences
    /// Matches C++ CashHackSpecialPower::findAmountToSteal
    fn find_amount_to_steal(&self) -> i32 {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if let Some(player) = obj_read.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        for upgrade in &self.cash_hack_data.upgrades {
                            if player_guard.has_science(upgrade.science) {
                                return upgrade.amount_to_steal;
                            }
                        }
                    }
                }
            }
        }

        self.cash_hack_data.default_amount_to_steal
    }

    /// Steal cash from target object
    /// Matches C++ CashHackSpecialPower::stealCashFromObject
    fn steal_cash_from_object(&self, victim_id: ObjectId) {
        let desired_amount = self.find_amount_to_steal() as u32;
        if desired_amount == 0 {
            return;
        }

        let Some(victim_obj) = TheGameLogic::find_object_by_id(victim_id) else {
            return;
        };
        let Ok(victim_read) = victim_obj.read() else {
            return;
        };
        let Some(victim_player) = victim_read.get_controlling_player() else {
            return;
        };

        let Some(hacker_obj) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id())
        else {
            return;
        };
        let Ok(hacker_read) = hacker_obj.read() else {
            return;
        };
        let Some(hacker_player) = hacker_read.get_controlling_player() else {
            return;
        };

        let Ok(victim_guard) = victim_player.read() else {
            return;
        };
        let Ok(hacker_guard) = hacker_player.read() else {
            return;
        };
        let victim_index = victim_guard.get_player_index();
        let hacker_index = hacker_guard.get_player_index();
        drop(victim_guard);
        drop(hacker_guard);

        if victim_index == hacker_index {
            let Ok(mut player_guard) = victim_player.write() else {
                return;
            };
            let available_cash = player_guard.get_money().count_money();
            let cash_to_steal = desired_amount.min(available_cash);
            if cash_to_steal == 0 {
                return;
            }
            let withdrawn = match player_guard.get_money_mut().withdraw(cash_to_steal) {
                Ok(amount) => amount,
                Err(_) => 0,
            };
            if withdrawn == 0 {
                return;
            }
            let _ = player_guard.get_money_mut().deposit(cash_to_steal);
            player_guard
                .get_score_keeper_mut()
                .add_money_earned(cash_to_steal);

            let add_cash_text = TheGameText::fetch("GUI:AddCash");
            let lose_cash_text = TheGameText::fetch("GUI:LoseCash");
            let add_caption = format_cash_text(&add_cash_text, cash_to_steal);
            let lose_caption = format_cash_text(&lose_cash_text, cash_to_steal);

            let mut pos = *hacker_read.get_position();
            pos.z += 20.0;
            let _ = TheInGameUI::add_floating_text(&add_caption, &pos, Color::new(0, 255, 0, 255));
            let mut loss_pos = *victim_read.get_position();
            loss_pos.z += 30.0;
            let _ = TheInGameUI::add_floating_text(
                &lose_caption,
                &loss_pos,
                Color::new(255, 0, 0, 255),
            );
            return;
        }

        let (first_player, second_player, victim_first) = if victim_index <= hacker_index {
            (victim_player.clone(), hacker_player.clone(), true)
        } else {
            (hacker_player.clone(), victim_player.clone(), false)
        };

        let Ok(mut first_guard) = first_player.write() else {
            return;
        };
        let Ok(mut second_guard) = second_player.write() else {
            return;
        };

        let (victim_guard, hacker_guard) = if victim_first {
            (&mut first_guard, &mut second_guard)
        } else {
            (&mut second_guard, &mut first_guard)
        };

        let available_cash = victim_guard.get_money().count_money();
        let cash_to_steal = desired_amount.min(available_cash);
        if cash_to_steal == 0 {
            return;
        }

        let withdrawn = match victim_guard.get_money_mut().withdraw(cash_to_steal) {
            Ok(amount) => amount,
            Err(_) => 0,
        };
        if withdrawn == 0 {
            return;
        }
        let _ = hacker_guard.get_money_mut().deposit(cash_to_steal);
        hacker_guard
            .get_score_keeper_mut()
            .add_money_earned(cash_to_steal);

        let add_cash_text = TheGameText::fetch("GUI:AddCash");
        let lose_cash_text = TheGameText::fetch("GUI:LoseCash");
        let add_caption = format_cash_text(&add_cash_text, cash_to_steal);
        let lose_caption = format_cash_text(&lose_cash_text, cash_to_steal);

        let mut hacker_pos = *hacker_read.get_position();
        hacker_pos.z += 20.0;
        let mut victim_pos = *victim_read.get_position();
        victim_pos.z += 30.0;

        let _ =
            TheInGameUI::add_floating_text(&add_caption, &hacker_pos, Color::new(0, 255, 0, 255));
        let _ =
            TheInGameUI::add_floating_text(&lose_caption, &victim_pos, Color::new(255, 0, 0, 255));
    }
}

fn format_cash_text(template: &str, amount: u32) -> String {
    if template.contains("%d") || template.contains("%i") {
        template
            .replace("%d", &amount.to_string())
            .replace("%i", &amount.to_string())
    } else if template.contains("%f") {
        template.replace("%f", &format!("{:.0}", amount))
    } else {
        format!("{}: {}", template, amount)
    }
}

// Implement the special power module interface
impl ObjSpecialPowerModuleInterface for CashHackSpecialPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        ObjSpecialPowerModuleInterface::is_module_for_power(&self.base, special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        ObjSpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn get_power_name(&self) -> String {
        ObjSpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        ObjSpecialPowerModuleInterface::get_special_power_template_full(&self.base)
    }

    fn get_required_science(&self) -> ScienceType {
        ObjSpecialPowerModuleInterface::get_required_science(&self.base)
    }

    fn on_special_power_creation(&mut self) {
        ObjSpecialPowerModuleInterface::on_special_power_creation(&mut self.base)
    }

    fn set_ready_frame(&mut self, frame: u32) {
        ObjSpecialPowerModuleInterface::set_ready_frame(&mut self.base, frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        ObjSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        // Cash hack requires a target object, not location-less
        ObjSpecialPowerModuleInterface::do_special_power(&mut self.base, command_options)
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if obj_read.is_disabled() {
                    return;
                }
            }
        }

        // Sanity check - need valid victim (matches C++ NULL check)
        if object_id == 0 {
            return;
        }

        // Call base class to handle triggers and sounds
        ObjSpecialPowerModuleInterface::do_special_power_at_object(
            &mut self.base,
            object_id,
            command_options,
        );

        // Execute the cash theft
        self.steal_cash_from_object(object_id);
    }

    fn do_special_power_at_location(
        &mut self,
        _location: &Coord3D,
        _angle: f32,
        _command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(object) = TheGameLogic::find_object_by_id(self.base.get_owner_object_id()) {
            if let Ok(obj_read) = object.read() {
                if obj_read.is_disabled() {
                    return;
                }
            }
        }

        // Cash hack requires a target object, not a location
        // This matches C++ behavior - cash hack is object-targeted only
        // Do nothing (silently ignore location-based activation)
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Waypoints not used for cash hack
        ObjSpecialPowerModuleInterface::do_special_power_using_waypoints(
            &mut self.base,
            waypoint,
            command_options,
        )
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        ObjSpecialPowerModuleInterface::mark_special_power_triggered(&mut self.base, location)
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        ObjSpecialPowerModuleInterface::start_power_recharge_at(&mut self.base, current_frame)
    }

    fn get_initiate_sound(&self) -> &crate::object::special_power_template::AudioEventRts {
        ObjSpecialPowerModuleInterface::get_initiate_sound(&self.base)
    }

    fn is_script_only(&self) -> bool {
        ObjSpecialPowerModuleInterface::is_script_only(&self.base)
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        None
    }
}

impl EngineSpecialPowerModuleInterface for CashHackSpecialPower {
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        EngineSpecialPowerModuleInterface::activate(&mut self.base)
    }

    fn can_activate(&self) -> bool {
        EngineSpecialPowerModuleInterface::can_activate(&self.base)
    }

    fn get_power_type(&self) -> u32 {
        EngineSpecialPowerModuleInterface::get_power_type(&self.base)
    }

    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        EngineSpecialPowerModuleInterface::start_power_recharge(&mut self.base)
    }

    fn get_ready_frame(&self) -> u32 {
        EngineSpecialPowerModuleInterface::get_ready_frame(&self.base)
    }

    fn is_ready(&self) -> bool {
        EngineSpecialPowerModuleInterface::is_ready(&self.base)
    }

    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
        EngineSpecialPowerModuleInterface::get_special_power_template(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        EngineSpecialPowerModuleInterface::get_special_power_template_full(&self.base)
    }

    fn get_power_name(&self) -> String {
        EngineSpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_percent_ready(&self) -> f32 {
        EngineSpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn pause_countdown(&mut self, pause: bool) {
        EngineSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&crate::common::Coord3D>) {
        EngineSpecialPowerModuleInterface::mark_special_power_triggered(&mut self.base, location)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power(
            self,
            command_options,
        );
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_object(
            self,
            object_id,
            command_options,
        );
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            location,
            angle,
            command_options,
        );
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_using_waypoints(
            self,
            waypoint,
            command_options,
        );
    }
}

impl Snapshotable for CashHackSpecialPower {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Module for CashHackSpecialPower {
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
        LegacyModuleData::get_module_tag_name_key(self.cash_hack_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.cash_hack_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for CashHackSpecialPower {
    fn get_special_power(&mut self) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface_const(
        &self,
    ) -> Option<&dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::science::SCIENCE_INVALID;

    #[test]
    fn test_cash_hack_special_power_creation() {
        let data = CashHackSpecialPowerModuleData::default();
        let power = CashHackSpecialPower::new(0, 1, Arc::new(data));

        assert!(power.is_ready());
    }

    #[test]
    fn test_find_amount_to_steal_default() {
        let data = CashHackSpecialPowerModuleData {
            default_amount_to_steal: 1000,
            ..Default::default()
        };

        let power = CashHackSpecialPower::new(0, 1, Arc::new(data));
        assert_eq!(power.find_amount_to_steal(), 1000);
    }

    #[test]
    fn test_cash_hack_with_upgrades() {
        let upgrade = CashHackUpgrade {
            science: SCIENCE_INVALID,
            amount_to_steal: 2000,
        };

        let data = CashHackSpecialPowerModuleData {
            default_amount_to_steal: 1000,
            upgrades: vec![upgrade],
            ..Default::default()
        };

        let power = CashHackSpecialPower::new(0, 1, Arc::new(data));
        // Without science check, should return default
        assert_eq!(power.find_amount_to_steal(), 1000);
    }

    #[test]
    fn test_do_special_power_at_location_ignored() {
        let data = CashHackSpecialPowerModuleData::default();
        let mut power = CashHackSpecialPower::new(0, 1, Arc::new(data));

        // Should not panic - just returns without doing anything
        let location: Coord3D = [100.0, 200.0, 0.0].into();
        crate::modules::SpecialPowerModuleInterface::do_special_power_at_location(
            &mut power,
            &location,
            0.0,
            SpecialPowerCommandOptions::empty(),
        );
    }
}
