//! Sabotage Supply Dropzone Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that resets the timer on the target supply dropzone.
//! Author: Kris Morness, June 2003 (original C++), converted to Rust

use crate::common::ObjectID;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::sync::{Arc, Mutex, RwLock};

fn resolve_crate_object(
    id: ObjectID,
) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
    if id == crate::common::INVALID_ID {
        return None;
    }
    crate::helpers::TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
}

// Import types that would be defined in other modules
use super::format_cash_template;
use crate::ai::*;
use crate::common::*;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::*;
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};

/// Module data for sabotage supply dropzone crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageSupplyDropzoneCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Amount of cash to steal
    pub steal_cash_amount: u32,
}

impl Default for SabotageSupplyDropzoneCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            steal_cash_amount: 0,
        }
    }
}

impl SabotageSupplyDropzoneCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SABOTAGE_SUPPLY_DROPZONE_CRATE_COLLIDE_FIELDS)
    }

    /// Build field parser for INI configuration
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = LegacyCrateCollideModuleData::build_field_parse();
        fields.extend(vec![FieldParse::new(
            "StealCashAmount",
            FieldType::UnsignedInt,
            "steal_cash_amount",
        )]);
        fields
    }
}

fn parse_kind_of_mask(tokens: &[&str]) -> Result<u64, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut mask = 0u64;
    for token in tokens
        .iter()
        .filter(|token| **token != "=")
        .flat_map(|token| token.split('|'))
    {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let Some(kind) = kindof_from_name(token) else {
            return Err(INIError::InvalidData);
        };
        mask |= 1u64 << (kind as u32);
    }
    Ok(mask)
}

fn first_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_required_kind_of(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_steal_cash_amount(
    _ini: &mut INI,
    data: &mut SabotageSupplyDropzoneCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.steal_cash_amount = INI::parse_unsigned_int(first_token(tokens)?)?;
    Ok(())
}

const SABOTAGE_SUPPLY_DROPZONE_CRATE_COLLIDE_FIELDS: &[IniFieldParse<
    SabotageSupplyDropzoneCrateCollideModuleData,
>] = &[
    IniFieldParse {
        token: "RequiredKindOf",
        parse: parse_required_kind_of,
    },
    IniFieldParse {
        token: "ForbiddenKindOf",
        parse: parse_forbidden_kind_of,
    },
    IniFieldParse {
        token: "ForbidOwnerPlayer",
        parse: parse_forbid_owner_player,
    },
    IniFieldParse {
        token: "BuildingPickup",
        parse: parse_building_pickup,
    },
    IniFieldParse {
        token: "HumanOnly",
        parse: parse_human_only,
    },
    IniFieldParse {
        token: "PickupScience",
        parse: parse_pickup_science,
    },
    IniFieldParse {
        token: "ExecuteFX",
        parse: parse_execute_fx,
    },
    IniFieldParse {
        token: "ExecuteAnimation",
        parse: parse_execute_animation,
    },
    IniFieldParse {
        token: "ExecuteAnimationTime",
        parse: parse_execute_animation_time,
    },
    IniFieldParse {
        token: "ExecuteAnimationZRise",
        parse: parse_execute_animation_z_rise,
    },
    IniFieldParse {
        token: "ExecuteAnimationFades",
        parse: parse_execute_animation_fades,
    },
    IniFieldParse {
        token: "StealCashAmount",
        parse: parse_steal_cash_amount,
    },
];

/// Sabotage Supply Dropzone Crate Collide module
#[derive(Debug)]
pub struct SabotageSupplyDropzoneCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageSupplyDropzoneCrateCollideModuleData>>,
}

impl SabotageSupplyDropzoneCrateCollide {
    /// Create new sabotage supply dropzone crate collide module
    pub fn new(
        object: &Arc<RwLock<Object>>,
        module_data: SabotageSupplyDropzoneCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(&object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    /// Check if this is a valid target for execution
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        // First check base validation
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Can't sabotage dead structures
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        // We can only sabotage supply dropzones
        if !other_lock.is_kind_of(KindOf::FSSupplyDropzone) {
            return Ok(false);
        }

        // Can only sabotage enemy buildings
        let relationship = self
            .base
            .get_object()
            .map_err(GameError::from)?
            .read()
            .map_err(|_| GameError::LockError)?
            .relationship_to(&other_lock);

        if relationship != Relationship::Enemies {
            return Ok(false);
        }

        Ok(true)
    }

    /// Execute the crate behavior
    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        if let Some(ai) = object_lock.get_ai_update_interface() {
            let goal_id = ai
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal| goal.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                return Ok(false);
            }
        }
        drop(object_lock);

        // C++ feedback calls are void side effects; sabotage still completes if they fail.
        let _ = TheRadar::try_infiltration_event(other.clone());

        let _ = self
            .base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::DropZone);

        // Reset the timer on the dropzone
        self.reset_dropzone_timer(other_id)?;

        // Steal cash!
        let cash_stolen = self.steal_cash(other_id)?;

        if cash_stolen > 0 {
            // Play the "cash stolen" EVA event if the local player is the victim!
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::CashStolen);
            }
            drop(other_lock);

            // Display floating text for cash changes
            let _ = self.display_cash_floating_text(other.clone(), cash_stolen);
        } else {
            // No cash stolen, just play building sabotaged sound
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                let _ = TheEva::set_should_play(EvaEvent::BuildingSabotaged);
            }
        }

        Ok(true)
    }

    /// Reset the timer on the dropzone by finding and resetting its OCL update
    fn reset_dropzone_timer(&self, other_id: ObjectID) -> Result<(), GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        if let Some(module) = other_lock.find_update_module("OCLUpdate") {
            let mut did_reset = false;
            module.with_module(|module| {
                if let Some(ocl_update) = module.get_ocl_update_control_interface() {
                    ocl_update.reset_timer();
                    did_reset = true;
                }
            });
            if did_reset {
                return Ok(());
            }
        }

        Ok(())
    }

    /// Steal cash from the target and give to the attacker
    fn steal_cash(&self, other_id: ObjectID) -> Result<u32, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(0);
        };

        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        let (Some(target_player), Some(attacker_player)) = (
            other_lock.get_controlling_player(),
            object_lock.get_controlling_player(),
        ) else {
            return Ok(0);
        };

        drop(object_lock);
        drop(other_lock);

        let mut target_player_guard = target_player.write().map_err(|_| GameError::LockError)?;
        let target_money = target_player_guard.get_money_mut();
        let mut attacker_player_guard =
            attacker_player.write().map_err(|_| GameError::LockError)?;
        let attacker_money = attacker_player_guard.get_money_mut();

        let available_cash = target_money.count_money();
        let module_data = self.module_data.lock().map_err(|_| GameError::LockError)?;
        let desired_amount = module_data.steal_cash_amount;
        drop(module_data);

        // Check to see if they have the cash, otherwise, take the remainder!
        let cash_to_steal = cmp::min(desired_amount, available_cash);

        if cash_to_steal > 0 {
            // Steal the cash
            target_money.withdraw(cash_to_steal)?;
            attacker_money.deposit(cash_to_steal)?;

            attacker_player_guard
                .get_score_keeper_mut()
                .add_money_earned(cash_to_steal);
        }

        Ok(cash_to_steal)
    }

    /// Display floating text for cash gain/loss
    fn display_cash_floating_text(
        &self,
        other: Arc<RwLock<Object>>,
        cash_amount: u32,
    ) -> Result<(), GameError> {
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Display cash income floating over the saboteur
        let money_string = super::format_add_cash(cash_amount);
        let mut pos = *object_lock.get_position();
        pos.z += 20.0; // Add a little z to make it show up above the unit
        let green_color = Color::new(0, 255, 0, 255);
        TheInGameUI::add_floating_text(&money_string, &pos, green_color)?;

        // Display cash lost floating over the target
        let loss_string = super::format_lose_cash(cash_amount);
        let mut target_pos = *other_lock.get_position();
        target_pos.z += 30.0; // Add a little z to make it show up above the unit
        let red_color = Color::new(255, 0, 0, 255);
        TheInGameUI::add_floating_text(&loss_string, &target_pos, red_color)?;

        Ok(())
    }

    /// Check if this is a sabotage building crate collide
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl LegacyCollideAdapter for SabotageSupplyDropzoneCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageSupplyDropzoneCrateCollide::is_valid_to_execute(
            self,
            other.read().map(|g| g.get_id()).unwrap_or(0),
        )? {
            let success = SabotageSupplyDropzoneCrateCollide::execute_crate_behavior(
                self,
                other.read().map(|g| g.get_id()).unwrap_or(0),
            )?;
            self.base
                .finish_execution_attempt(&other, success)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        SabotageSupplyDropzoneCrateCollide::is_valid_to_execute(
            self,
            other.read().map(|g| g.get_id()).unwrap_or(0),
        )
    }

    fn legacy_is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl CrateCollideModule for SabotageSupplyDropzoneCrateCollide {
    fn is_valid_to_execute(&self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageSupplyDropzoneCrateCollide::is_valid_to_execute(
            self,
            other.read().map(|g| g.get_id()).unwrap_or(0),
        )
    }

    fn execute_crate_behavior(&mut self, other_id: ObjectID) -> Result<bool, GameError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        SabotageSupplyDropzoneCrateCollide::execute_crate_behavior(
            self,
            other.read().map(|g| g.get_id()).unwrap_or(0),
        )
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl game_engine::common::system::Snapshotable for SabotageSupplyDropzoneCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steal_cash_amount_parse_from_ini_preserves_cpp_field() {
        let _lock = crate::test_sync::lock();

        let mut data = SabotageSupplyDropzoneCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "StealCashAmount = 1750\n\
             RequiredKindOf = FS_SUPPLY_DROPZONE\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("sabotage supply dropzone ini parses");

        assert_eq!(data.steal_cash_amount, 1750);
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::FSSupplyDropzone as u32)),
            0
        );
    }

    #[test]
    fn steal_cash_amount_rejects_missing_value_like_cpp() {
        let mut data = SabotageSupplyDropzoneCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = parse_steal_cash_amount(&mut ini, &mut data, &["="])
            .expect_err("missing cash amount should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.steal_cash_amount, 0);
    }

    #[test]
    fn cash_labels_format_cpp_style_templates() {
        assert_eq!(format_cash_template("+$%d", 125, "+"), "+$125");
        assert_eq!(format_cash_template("-$%u", 125, "-"), "-$125");
        assert_eq!(format_cash_template("-$%-6u", 125, "-"), "-$125");
        assert_eq!(format_cash_template("GUI:AddCash", 125, "+"), "+$125");
    }

    #[test]
    fn steal_cash_without_player_handles_is_best_effort_like_cpp() {
        let _lock = crate::test_sync::lock();

        let saboteur = Arc::new(RwLock::new(Object::new_test(58_201, 100.0)));
        let target = Arc::new(RwLock::new(Object::new_test(58_202, 100.0)));
        let module_data = SabotageSupplyDropzoneCrateCollideModuleData {
            steal_cash_amount: 500,
            ..Default::default()
        };
        let module = SabotageSupplyDropzoneCrateCollide::new(&saboteur, module_data);

        assert_eq!(
            module
                .steal_cash(target)
                .expect("cash steal is best effort"),
            0
        );
    }
}
