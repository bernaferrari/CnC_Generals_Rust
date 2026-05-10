//! Salvage crate behaviour – awards weapon/armor upgrades, veterancy, or money.

use crate::common::audio::AudioEventRts;
use crate::common::ModelConditionFlags;
use crate::common::*;
use crate::experience::ExperienceTracker;
use crate::helpers::{TheAudio, TheGameText, TheInGameUI};
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as BaseCrateCollide, CrateCollideBehavior, CrateCollideModuleData,
};
use crate::object::collide::{
    CollideModule, CollisionError, Coord3D as CollisionCoord3D, GameObject,
};
use crate::object::{ArmorSetFlag, Object};
use crate::player::Player;
use crate::weapon::WeaponSetType;
use crate::{GameLogicRandomValue, GameLogicRandomValueReal};
use std::sync::{Arc, Mutex, RwLock};

/// INI configuration for salvage crates.
#[derive(Debug, Clone)]
pub struct SalvageCrateCollideModuleData {
    /// Base crate metadata (required/forbidden kinds etc.).
    pub base: CrateCollideModuleData,
    /// Chance to award a weapon upgrade if the object qualifies.
    pub weapon_chance: f32,
    /// Chance to award a veterancy level if weapon bonus fails.
    pub level_chance: f32,
    /// Chance to award money if weapon and level both fail.
    pub money_chance: f32,
    /// Minimum cash payout.
    pub minimum_money: i32,
    /// Maximum cash payout.
    pub maximum_money: i32,
}

impl Default for SalvageCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
            weapon_chance: 1.0,
            level_chance: 0.25,
            money_chance: 0.75,
            minimum_money: 25,
            maximum_money: 75,
        }
    }
}

impl SalvageCrateCollideModuleData {
    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = CrateCollideModuleData::build_field_parse();
        fields.extend([
            FieldParse::new("WeaponChance", FieldType::PercentToReal, "weapon_chance"),
            FieldParse::new("LevelChance", FieldType::PercentToReal, "level_chance"),
            FieldParse::new("MoneyChance", FieldType::PercentToReal, "money_chance"),
            FieldParse::new("MinMoney", FieldType::Int, "minimum_money"),
            FieldParse::new("MaxMoney", FieldType::Int, "maximum_money"),
        ]);
        fields
    }
}

/// Concrete implementation of the Salvage crate behaviour.
pub struct SalvageCrateCollide {
    base: BaseCrateCollide,
    module_data: SalvageCrateCollideModuleData,
}

impl SalvageCrateCollide {
    pub fn new(object_id: ObjectId, module_data: SalvageCrateCollideModuleData) -> Self {
        Self {
            base: BaseCrateCollide::new(object_id, module_data.base.clone()),
            module_data,
        }
    }

    fn require_object_handle(
        &self,
        other: &dyn GameObject,
    ) -> Result<Arc<RwLock<Object>>, CollisionError> {
        other.as_object_handle().ok_or_else(|| {
            CollisionError::InvalidObject(
                "Salvage crate requires a concrete Object handle".to_string(),
            )
        })
    }

    fn determine_salvage_type(
        &self,
        other: &Arc<RwLock<Object>>,
    ) -> Result<SalvageType, CollisionError> {
        if self.eligible_for_armor_set(other)? {
            return Ok(SalvageType::ArmorSet);
        }

        if self.eligible_for_weapon_set(other)? && self.test_weapon_chance() {
            return Ok(SalvageType::WeaponSet);
        }

        if self.eligible_for_level(other)? && self.test_level_chance() {
            return Ok(SalvageType::Level);
        }

        Ok(SalvageType::Money)
    }

    fn eligible_for_weapon_set(&self, other: &Arc<RwLock<Object>>) -> Result<bool, CollisionError> {
        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if !guard.is_kind_of(KindOf::WeaponSalvager) {
            return Ok(false);
        }

        Ok(!guard.test_weapon_set_flag(WeaponSetType::CrateUpgradeTwo))
    }

    fn eligible_for_armor_set(&self, other: &Arc<RwLock<Object>>) -> Result<bool, CollisionError> {
        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if !guard.is_kind_of(KindOf::ArmorSalvager) {
            return Ok(false);
        }

        Ok(!guard.test_armor_set_flag(ArmorSetFlag::CrateUpgradeTwo))
    }

    fn eligible_for_level(&self, other: &Arc<RwLock<Object>>) -> Result<bool, CollisionError> {
        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        let tracker = match guard.get_experience_tracker() {
            Some(tracker) => tracker,
            None => return Ok(false),
        };

        let tracker = tracker.lock().map_err(|_| {
            CollisionError::InvalidObject("experience tracker lock poisoned".into())
        })?;

        if tracker.get_veterancy_level() == VeterancyLevel::Heroic {
            return Ok(false);
        }

        Ok(tracker.can_gain_exp_for_level(1))
    }

    fn test_weapon_chance(&self) -> bool {
        if self.module_data.weapon_chance >= 1.0 {
            return true;
        }
        GameLogicRandomValueReal(0.0, 1.0) < self.module_data.weapon_chance
    }

    fn test_level_chance(&self) -> bool {
        if self.module_data.level_chance >= 1.0 {
            return true;
        }
        GameLogicRandomValueReal(0.0, 1.0) < self.module_data.level_chance
    }

    fn do_weapon_set(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let mut guard = other
            .write()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if guard.test_weapon_set_flag(WeaponSetType::CrateUpgradeOne) {
            guard.clear_weapon_set_flag(WeaponSetType::CrateUpgradeOne);
            guard.set_weapon_set_flag(WeaponSetType::CrateUpgradeTwo);
        } else {
            guard.set_weapon_set_flag(WeaponSetType::CrateUpgradeOne);
        }
        Ok(())
    }

    fn do_armor_set(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let mut guard = other
            .write()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if guard.test_armor_set_flag(ArmorSetFlag::CrateUpgradeOne) {
            guard.clear_armor_set_flag(ArmorSetFlag::CrateUpgradeOne);
            guard.set_armor_set_flag(ArmorSetFlag::CrateUpgradeTwo);
            guard.clear_model_condition_state(ModelConditionFlags::ArmorsetCrateUpgradeOne);
            guard.set_model_condition_state(ModelConditionFlags::ArmorsetCrateUpgradeTwo);
        } else {
            guard.set_armor_set_flag(ArmorSetFlag::CrateUpgradeOne);
            guard.set_model_condition_state(ModelConditionFlags::ArmorsetCrateUpgradeOne);
        }
        Ok(())
    }

    fn do_level_gain(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
        let tracker = match guard.get_experience_tracker() {
            Some(tracker) => tracker,
            None => return Ok(()),
        };
        drop(guard);

        let mut tracker = tracker.lock().map_err(|_| {
            CollisionError::InvalidObject("experience tracker lock poisoned".into())
        })?;
        tracker.gain_exp_for_level(1, false, &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED);
        Ok(())
    }

    fn do_money(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let payout = if self.module_data.minimum_money != self.module_data.maximum_money {
            GameLogicRandomValue(
                self.module_data.minimum_money,
                self.module_data.maximum_money,
            )
        } else {
            self.module_data.minimum_money
        };

        if payout <= 0 {
            return Ok(());
        }

        let owner = {
            let guard = other
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            guard.get_controlling_player()
        };

        let Some(player_arc) = owner else {
            return Ok(());
        };

        {
            let mut player = player_arc
                .write()
                .map_err(|_| CollisionError::InvalidObject("player lock poisoned".into()))?;
            player.get_money_mut().add_money(payout);
            player
                .get_score_keeper_mut()
                .add_money_earned(payout as u32);
        }

        self.display_money_floating_text(payout as u32, other, &player_arc)?;
        self.play_money_sound(other)?;
        Ok(())
    }

    fn display_money_floating_text(
        &self,
        amount: u32,
        object: &Arc<RwLock<Object>>,
        player: &Arc<RwLock<Player>>,
    ) -> Result<(), CollisionError> {
        let (position, color) = {
            let obj = object
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            let mut pos = *obj.get_position();
            pos.z += 10.0;

            let player_guard = player
                .read()
                .map_err(|_| CollisionError::InvalidObject("player lock poisoned".into()))?;
            let color = player_guard.get_player_color();
            (pos, color)
        };

        let caption = format!("{}: {}", TheGameText::fetch("GUI:AddCash"), amount);
        TheInGameUI::add_floating_text(&caption, &position, color)
            .map_err(|err| CollisionError::InvalidObject(err.to_string()))
    }

    fn play_salvage_sound(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let id = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
            .get_id();
        if let Some(audio) = TheAudio::get() {
            let event = TheAudio::get_misc_audio().crate_salvage.clone();
            let mut audio_event = AudioEventRts::new(event.sound_type);
            audio_event.set_object_id(id);
            audio.add_audio_event(&audio_event);
        }
        Ok(())
    }

    fn play_money_sound(&self, other: &Arc<RwLock<Object>>) -> Result<(), CollisionError> {
        let id = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
            .get_id();
        if let Some(audio) = TheAudio::get() {
            let event = TheAudio::get_misc_audio().crate_money.clone();
            let mut audio_event = AudioEventRts::new(event.sound_type);
            audio_event.set_object_id(id);
            audio.add_audio_event(&audio_event);
        }
        Ok(())
    }
}

impl CrateCollideBehavior for SalvageCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        let handle = self.require_object_handle(other)?;
        let salvage_type = self.determine_salvage_type(&handle)?;

        match salvage_type {
            SalvageType::ArmorSet => {
                self.do_armor_set(&handle)?;
                self.play_salvage_sound(&handle)?;
            }
            SalvageType::WeaponSet => {
                self.do_weapon_set(&handle)?;
                self.play_salvage_sound(&handle)?;
            }
            SalvageType::Level => {
                self.do_level_gain(&handle)?;
            }
            SalvageType::Money => {
                self.do_money(&handle)?;
            }
        }

        Ok(true)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        if !self.base.is_valid_to_execute(other) {
            return false;
        }

        let Ok(handle) = self.require_object_handle(other) else {
            return false;
        };

        let Ok(guard) = handle.read() else {
            return false;
        };

        guard.get_template().is_kind_of(KindOf::Salvager)
    }
}

impl CollideModule for SalvageCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &CollisionCoord3D,
        _normal: &CollisionCoord3D,
    ) -> Result<(), CollisionError> {
        let Some(other_obj) = other else {
            return Ok(());
        };

        if !self.base.is_valid_to_execute(other_obj) {
            return Ok(());
        }

        if self.execute_crate_behavior(other_obj)? {
            self.base.finalize_collection(other_obj)?;
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        CrateCollideBehavior::is_valid_to_execute(self, other)
    }

    fn is_salvage_crate_collide(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SalvageType {
    ArmorSet,
    WeaponSet,
    Level,
    Money,
}

impl game_engine::common::system::Snapshotable for SalvageCrateCollide {
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
