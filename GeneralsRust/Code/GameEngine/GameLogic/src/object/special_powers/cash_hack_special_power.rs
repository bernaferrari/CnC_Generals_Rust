//! CashHackSpecialPower
//!
//! Port of CashHackSpecialPower.h and CashHackSpecialPower.cpp
//! Author: Amit Kumar, July 2002 (C++), Rust Port
//!
//! Hacker (Black Lotus) steals money from an enemy player's building.
//! The amount stolen depends on upgrade science the hacker's player has.
//! Shows floating text indicating cash gained/lost.
//!
//! Key behaviors:
//! - doSpecialPowerAtLocation: returns immediately (only allowed at objects)
//! - doSpecialPowerAtObject: steals min(desiredAmount, targetMoney) from victim
//! - Upgrade pairs: science -> amount to steal (checked in order)

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use log::warn;

use crate::common::science::ScienceType;
use crate::common::{Coord3D, Int, ObjectID, Real};
use crate::helpers::{TheGameLogic, TheInGameUI};
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;

/// Cash hack upgrade pair: a science prerequisite maps to a different steal amount.
/// Matches C++ CashHackSpecialPowerModuleData::Upgrades.
#[derive(Debug, Clone)]
pub struct CashHackUpgrade {
    /// Science that unlocks this steal amount
    pub science: ScienceType,
    /// Amount to steal when this science is active
    pub amount_to_steal: Int,
}

impl Default for CashHackUpgrade {
    fn default() -> Self {
        Self {
            science: crate::common::science::SCIENCE_INVALID,
            amount_to_steal: 0,
        }
    }
}

/// Module data for CashHackSpecialPower.
/// Matches C++ CashHackSpecialPowerModuleData.
#[derive(Debug, Clone)]
pub struct CashHackSpecialPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Upgrade pairs: science -> amount to steal.
    /// Matches C++ m_upgrades.
    pub upgrades: Vec<CashHackUpgrade>,
    /// Default amount to steal. Matches C++ m_defaultAmountToSteal.
    pub default_amount_to_steal: Int,
}

impl Default for CashHackSpecialPowerModuleData {
    fn default() -> Self {
        Self {
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

impl ModuleData for CashHackSpecialPowerModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.base.get_module_tag_name_key()
    }
}

impl Snapshotable for CashHackSpecialPowerModuleData {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// CashHackSpecialPower module.
///
/// Matches C++ CashHackSpecialPower which extends SpecialPowerModule.
/// Steals money from an enemy building when activated on it.
pub struct CashHackSpecialPower {
    module_name_key: NameKeyType,
    data: Arc<CashHackSpecialPowerModuleData>,
    owner_object_id: ObjectID,
}

impl CashHackSpecialPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<CashHackSpecialPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Find the amount to steal based on player's science.
    /// Matches C++ CashHackSpecialPower::findAmountToSteal().
    fn find_amount_to_steal(&self) -> Int {
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(player) = owner_guard.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        for upgrade in &self.data.upgrades {
                            if player_guard.has_science(upgrade.science) {
                                return upgrade.amount_to_steal;
                            }
                        }
                    }
                }
            }
        }
        self.data.default_amount_to_steal
    }

    /// Execute cash hack at a location - returns immediately.
    /// Matches C++ CashHackSpecialPower::doSpecialPowerAtLocation() which only allows at objects.
    pub fn do_special_power_at_location(&self, _loc: &Coord3D) -> Result<(), String> {
        // C++: "only allowed at objects" - returns immediately
        Ok(())
    }

    /// Execute cash hack on a victim object.
    /// Matches C++ CashHackSpecialPower::doSpecialPowerAtObject().
    pub fn do_special_power_at_object(&self, victim_id: ObjectID) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        // Sanity check
        let Some(victim) = TheGameLogic::find_object_by_id(victim_id) else {
            return Ok(());
        };

        let desired_amount = self.find_amount_to_steal();
        if desired_amount <= 0 {
            return Ok(());
        }

        // Get owner and victim players and positions
        let self_pos = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            *owner_guard.get_position()
        };
        let victim_pos = {
            let Ok(victim_guard) = victim.read() else {
                return Ok(());
            };
            *victim_guard.get_position()
        };

        let self_player = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            owner_guard.get_controlling_player()
        };
        let victim_player = {
            let Ok(victim_guard) = victim.read() else {
                return Ok(());
            };
            victim_guard.get_controlling_player()
        };

        let Some((self_player, victim_player)) = self_player.zip(victim_player) else {
            return Ok(());
        };

        // Steal cash (C++: targetMoney->withdraw, selfMoney->deposit)
        let cash = {
            let Ok(victim_player_guard) = victim_player.read() else {
                return Ok(());
            };
            let available = victim_player_guard.get_money().get_money() as u32;
            drop(victim_player_guard);
            std::cmp::min(desired_amount as u32, available)
        };

        if cash == 0 {
            return Ok(());
        }

        // Withdraw from victim
        if let Ok(mut victim_player_guard) = victim_player.write() {
            let _ = victim_player_guard
                .get_money_mut()
                .subtract_money(cash as i32);
        }

        // Deposit to self
        if let Ok(mut self_player_guard) = self_player.write() {
            let _ = self_player_guard.get_money_mut().add_money(cash as i32);
            self_player_guard
                .get_score_keeper_mut()
                .add_money_earned(cash);
        }

        // Display floating text: cash gained (green) over the hacker
        // C++: TheInGameUI->addFloatingText(moneyString, &pos, GameMakeColor(0, 255, 0, 255))
        {
            let mut gain_pos = self_pos;
            gain_pos.z += 20.0; // C++: pos.z += 20.0f
            if let Err(err) = TheInGameUI::add_floating_text(
                &format!("+${}", cash),
                &gain_pos,
                crate::common::Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                }, // green
            ) {
                warn!("Failed to add cash hack gain floating text: {err}");
            }
        }

        // Display floating text: cash lost (red) over the target
        // C++: TheInGameUI->addFloatingText(moneyString, &pos, GameMakeColor(255, 0, 0, 255))
        {
            let mut loss_pos = victim_pos;
            loss_pos.z += 30.0; // C++: pos.z += 30.0f
            if let Err(err) = TheInGameUI::add_floating_text(
                &format!("-${}", cash),
                &loss_pos,
                crate::common::Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                }, // red
            ) {
                warn!("Failed to add cash hack loss floating text: {err}");
            }
        }

        Ok(())
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
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for CashHackSpecialPower {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("CashHackSpecialPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ CashHackSpecialPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for CashHackSpecialPower {
    fn get_module_name(&self) -> &'static str {
        "CashHackSpecialPower"
    }
}

// INI field parsers

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_money_amount(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.default_amount_to_steal = INI::parse_int(token)?;
    Ok(())
}

fn parse_upgrade_money_amount(
    _ini: &mut INI,
    data: &mut CashHackSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    // C++ parseCashHackUpgradePair: first token is science, second is amount
    let non_eq: Vec<&&str> = tokens.iter().filter(|t| **t != "=").collect();
    if non_eq.len() < 2 {
        return Err(INIError::InvalidData);
    }

    // Parse science by name hash (ScienceType = i32)
    // In the full implementation, this would use ScienceStore lookup
    let science_name = *non_eq[0];
    let mut hash: i32 = 0;
    for c in science_name.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    let science = if science_name.is_empty() {
        ScienceType::default()
    } else {
        hash.abs()
    };
    let amount = INI::parse_int(non_eq[1])?;

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
        token: "MoneyAmount",
        parse: parse_money_amount,
    },
    FieldParse {
        token: "UpgradeMoneyAmount",
        parse: parse_upgrade_money_amount,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cash_hack_default() {
        let data = CashHackSpecialPowerModuleData::default();
        assert_eq!(data.default_amount_to_steal, 0);
        assert!(data.upgrades.is_empty());
    }

    #[test]
    fn test_cash_hack_module_name() {
        let data = CashHackSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = CashHackSpecialPower::new(0, 0, arc_data);
        assert_eq!(power.get_module_name(), "CashHackSpecialPower");
    }

    #[test]
    fn test_find_amount_to_steal_no_upgrades() {
        let mut data = CashHackSpecialPowerModuleData::default();
        data.default_amount_to_steal = 1000;
        let arc_data = Arc::new(data);
        let power = CashHackSpecialPower::new(0, 0, arc_data);
        assert_eq!(power.find_amount_to_steal(), 1000);
    }

    #[test]
    fn test_do_special_power_at_location_returns_ok() {
        let data = CashHackSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = CashHackSpecialPower::new(0, 0, arc_data);
        // Should return Ok (does nothing - only objects allowed)
        assert!(power
            .do_special_power_at_location(&Coord3D::new(0.0, 0.0, 0.0))
            .is_ok());
    }

    #[test]
    fn test_do_special_power_at_object_no_owner() {
        let data = CashHackSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = CashHackSpecialPower::new(0, 0, arc_data);
        // Should return Ok without panicking when owner doesn't exist
        assert!(power.do_special_power_at_object(999).is_ok());
    }
}
