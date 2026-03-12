//! Sabotage Supply Center Crate Collide Module
//!
//! A crate (actually a saboteur - mobile crate) that steals cash from the target supply center.
//! Author: Kris Morness, June 2003 (original C++), converted to Rust

use serde::{Deserialize, Serialize};
use std::cmp;
use std::sync::{Arc, Mutex, RwLock};

// Import types that would be defined in other modules
use crate::ai::*;
use crate::common::*;
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as LegacyCrateCollide, CrateCollideModuleData as LegacyCrateCollideModuleData,
};
use crate::object::collide::crate_collide::*;
use crate::object::collide::Coord3D as CollideCoord3D;
use crate::object::collide::LegacyCollideAdapter;
use crate::object::*;

/// Module data for sabotage supply center crate collide behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageSupplyCenterCrateCollideModuleData {
    /// Base crate collide module data
    pub base: LegacyCrateCollideModuleData,
    /// Amount of cash to steal
    pub steal_cash_amount: u32,
}

impl Default for SabotageSupplyCenterCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: LegacyCrateCollideModuleData::default(),
            steal_cash_amount: 0,
        }
    }
}

impl SabotageSupplyCenterCrateCollideModuleData {
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

/// Sabotage Supply Center Crate Collide module
#[derive(Debug)]
pub struct SabotageSupplyCenterCrateCollide {
    /// Base crate collide functionality
    pub base: LegacyCrateCollide,
    /// Module-specific data
    pub module_data: Arc<Mutex<SabotageSupplyCenterCrateCollideModuleData>>,
}

impl SabotageSupplyCenterCrateCollide {
    /// Create new sabotage supply center crate collide module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: SabotageSupplyCenterCrateCollideModuleData,
    ) -> Self {
        Self {
            base: LegacyCrateCollide::from_object_handle(object, module_data.base.clone()),
            module_data: Arc::new(Mutex::new(module_data)),
        }
    }

    /// Check if this is a valid target for execution
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // First check base validation
        if !self.base.is_valid_to_execute(&other) {
            return Ok(false);
        }

        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        // Can't sabotage dead structures
        if other_lock.is_effectively_dead() {
            return Ok(false);
        }

        // We can only sabotage supply centers
        if !other_lock.is_kind_of(KindOf::FSSupplyCenter) {
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

        if relationship != Relationship::Enemy {
            return Ok(false);
        }

        Ok(true)
    }

    /// Execute the crate behavior
    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        // Check to make sure that the other object is also the goal object in the AIUpdateInterface
        // in order to prevent an unintentional conversion simply by having the terrorist walk too close
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_id = other.read().map_err(|_| GameError::LockError)?.get_id();

        // Check AI goal object - only execute if this is the intentional target
        if let Some(ai_update) = object_lock.get_ai_update_interface() {
            let goal_id = ai_update
                .lock()
                .ok()
                .and_then(|ai_guard| ai_guard.get_goal_object())
                .and_then(|goal_obj| goal_obj.read().ok().map(|goal_guard| goal_guard.get_id()));
            if goal_id != Some(other_id) {
                log::debug!(
                    "SabotageSupplyCenter: Skipping - target {} is not current goal {:?}",
                    other_id,
                    goal_id
                );
                return Ok(false);
            }
        }
        drop(object_lock);

        // Try infiltration event
        TheRadar::try_infiltration_event(other.clone())?;

        // Do sabotage feedback FX
        self.base
            .do_sabotage_feedback_fx(&other, SabotageVictimType::SupplyCenter)?;

        // Steal cash!
        let cash_stolen = self.steal_cash(other.clone())?;

        if cash_stolen > 0 {
            // Play the "cash stolen" EVA event if the local player is the victim!
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                TheEva::set_should_play(EvaEvent::CashStolen)?;
            }
            drop(other_lock);

            // Display floating text for cash changes
            self.display_cash_floating_text(other.clone(), cash_stolen)?;
        } else {
            // No cash stolen, just play building sabotaged sound
            let other_lock = other.read().map_err(|_| GameError::LockError)?;
            if other_lock.is_locally_controlled() {
                TheEva::set_should_play(EvaEvent::BuildingSabotaged)?;
            }
        }

        Ok(true)
    }

    /// Steal cash from the target and give to the attacker
    fn steal_cash(&self, other: Arc<RwLock<Object>>) -> Result<u32, GameError> {
        let object = self.base.get_object().map_err(GameError::from)?;
        let object_lock = object.read().map_err(|_| GameError::LockError)?;
        let other_lock = other.read().map_err(|_| GameError::LockError)?;

        let target_player = other_lock
            .get_controlling_player()
            .ok_or(GameError::InvalidOperation)?;
        let attacker_player = object_lock
            .get_controlling_player()
            .ok_or(GameError::InvalidOperation)?;

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

            // Update score keeper
            let mut attacker_player_lock =
                attacker_player.write().map_err(|_| GameError::LockError)?;
            attacker_player_lock
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
        let add_cash_text = TheGameText::fetch("GUI:AddCash");
        let money_string = format!("{}: {}", add_cash_text, cash_amount);
        let mut pos = *object_lock.get_position();
        pos.z += 20.0; // Add a little z to make it show up above the unit
        let green_color = Color::new(0, 255, 0, 255);
        TheInGameUI::add_floating_text(&money_string, &pos, green_color)?;

        // Display cash lost floating over the target
        let lose_cash_text = TheGameText::fetch("GUI:LoseCash");
        let loss_string = format!("{}: {}", lose_cash_text, cash_amount);
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

impl LegacyCollideAdapter for SabotageSupplyCenterCrateCollide {
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &CollideCoord3D,
        normal: &CollideCoord3D,
    ) -> Result<(), GameError> {
        let _ = (loc, normal);

        if SabotageSupplyCenterCrateCollide::is_valid_to_execute(self, other.clone())?
            && SabotageSupplyCenterCrateCollide::execute_crate_behavior(self, other.clone())?
        {
            self.base
                .finalize_collection(&other)
                .map_err(GameError::from)?;
        }

        Ok(())
    }

    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError> {
        SabotageSupplyCenterCrateCollide::is_valid_to_execute(self, other)
    }
}

impl CrateCollideModule for SabotageSupplyCenterCrateCollide {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageSupplyCenterCrateCollide::is_valid_to_execute(self, other)
    }

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError> {
        SabotageSupplyCenterCrateCollide::execute_crate_behavior(self, other)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        true
    }
}

impl game_engine::common::system::Snapshotable for SabotageSupplyCenterCrateCollide {
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
