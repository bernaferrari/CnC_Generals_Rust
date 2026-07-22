//! WeaponSet Management System
//!
//! This module provides weapon set management functionality matching the C++ implementation,
//! including weapon selection, locking, bonus coordination, and multi-weapon targeting logic.

use super::{
    DamageType, Weapon, WeaponBonus, WeaponBonusConditionFlags, WeaponSlotType, WeaponStatus,
    WeaponTemplate,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::{
    CommandSourceType, Coord3D, ModelConditionFlags, ObjectID, Xfer, XferMode, XferVersion,
    WEAPONSLOT_COUNT,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::kind_of::KindOfMask;
use game_engine::common::system::Snapshotable;
use game_engine::thing::thing_template::WeaponTemplateSet as EngineWeaponTemplateSet;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use thiserror::Error;

/// Weapon set type conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponSetType {
    Veteran = 0,
    Elite,
    Hero,
    PlayerUpgrade,
    CrateUpgradeOne,
    CrateUpgradeTwo,
    VehicleHijack,
    CarBomb,
    MineClearingDetail,
    WeaponRider1,
    WeaponRider2,
    WeaponRider3,
    WeaponRider4,
    WeaponRider5,
    WeaponRider6,
    WeaponRider7,
    WeaponRider8,
}

/// Weapon set flags for conditional weapon sets
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponSetFlags(u32);

impl WeaponSetFlags {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, weapon_set_type: WeaponSetType) {
        self.0 |= 1 << (weapon_set_type as u8);
    }

    pub fn clear(&mut self, weapon_set_type: WeaponSetType) {
        self.0 &= !(1 << (weapon_set_type as u8));
    }

    pub fn test(&self, weapon_set_type: WeaponSetType) -> bool {
        (self.0 & (1 << (weapon_set_type as u8))) != 0
    }

    pub fn clear_all(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Error)]
pub enum WeaponSetConversionError {
    #[error("unknown weapon set condition '{0}'")]
    UnknownCondition(String),
    #[error("weapon template '{0}' not found")]
    MissingWeaponTemplate(String),
}

/// Weapon template set defining weapons for specific conditions
#[derive(Debug, Clone)]
pub struct WeaponTemplateSet {
    /// Conditions when this weapon set applies
    pub conditions: WeaponSetFlags,
    /// Weapon templates for each slot
    pub weapon_templates: [Option<Arc<WeaponTemplate>>; 3], // PRIMARY, SECONDARY, TERTIARY
    /// Auto-choose mask for each weapon slot
    pub auto_choose_mask: [u32; 3],
    /// Preferred target kinds for each weapon
    pub preferred_against: [KindOfMask; 3],
    /// Whether reload time is shared across weapons
    pub is_reload_time_shared: bool,
    /// Whether weapon locks are shared across weapon sets
    pub is_weapon_lock_shared_across_sets: bool,
}

impl WeaponTemplateSet {
    pub fn new() -> Self {
        Self {
            conditions: WeaponSetFlags::new(),
            weapon_templates: [None, None, None],
            auto_choose_mask: [0xffffffff; 3], // Allow all command sources by default
            preferred_against: [KindOfMask::empty(); 3], // No preference by default
            is_reload_time_shared: false,
            is_weapon_lock_shared_across_sets: false,
        }
    }

    /// Clear all weapon template data
    pub fn clear(&mut self) {
        self.conditions.clear_all();
        self.weapon_templates = [None, None, None];
        self.auto_choose_mask = [0xffffffff; 3];
        self.preferred_against = [KindOfMask::empty(); 3];
        self.is_reload_time_shared = false;
        self.is_weapon_lock_shared_across_sets = false;
    }

    /// Build a game-logic weapon template set from an engine weapon set definition.
    pub fn from_engine_set<F>(
        engine_set: &EngineWeaponTemplateSet,
        mut resolver: F,
    ) -> Result<Self, WeaponSetConversionError>
    where
        F: FnMut(&AsciiString) -> Option<Arc<WeaponTemplate>>,
    {
        let mut result = WeaponTemplateSet::new();
        let engine_flags = engine_set.types();
        for index in 0..engine_flags.size() {
            if engine_flags.test(index) {
                let weapon_set_type = weapon_set_type_from_index(index).ok_or_else(|| {
                    let name = engine_flags
                        .get_bit_name_if_set(index)
                        .unwrap_or("<unknown>");
                    WeaponSetConversionError::UnknownCondition(name.to_string())
                })?;
                result.conditions.set(weapon_set_type);
            }
        }

        for slot_index in 0..WEAPONSLOT_COUNT {
            if let Some(name) = engine_set.weapon_template_name(slot_index) {
                if !name.is_empty() {
                    let ascii_name = AsciiString::from(name.as_str());
                    let weapon = resolver(&ascii_name).ok_or_else(|| {
                        WeaponSetConversionError::MissingWeaponTemplate(
                            ascii_name.as_str().to_string(),
                        )
                    })?;
                    result.weapon_templates[slot_index] = Some(weapon);
                }
            }

            result.auto_choose_mask[slot_index] = engine_set.auto_choose_mask(slot_index);
            result.preferred_against[slot_index] = engine_set.preferred_against_mask(slot_index);
        }

        result.is_reload_time_shared = engine_set.is_reload_time_shared();
        result.is_weapon_lock_shared_across_sets = engine_set.is_weapon_lock_shared_across_sets();

        Ok(result)
    }

    /// Check if this template set matches the given conditions
    pub fn matches_conditions(&self, conditions: &WeaponSetFlags) -> bool {
        // Check if all required conditions are met
        for weapon_set_type in [
            WeaponSetType::Veteran,
            WeaponSetType::Elite,
            WeaponSetType::Hero,
            WeaponSetType::PlayerUpgrade,
            WeaponSetType::CrateUpgradeOne,
            WeaponSetType::CrateUpgradeTwo,
            WeaponSetType::VehicleHijack,
            WeaponSetType::CarBomb,
            WeaponSetType::MineClearingDetail,
            WeaponSetType::WeaponRider1,
            WeaponSetType::WeaponRider2,
            WeaponSetType::WeaponRider3,
            WeaponSetType::WeaponRider4,
            WeaponSetType::WeaponRider5,
            WeaponSetType::WeaponRider6,
            WeaponSetType::WeaponRider7,
            WeaponSetType::WeaponRider8,
        ] {
            if self.conditions.test(weapon_set_type) && !conditions.test(weapon_set_type) {
                return false;
            }
        }
        true
    }

    /// Check if this set has any weapons
    pub fn has_any_weapons(&self) -> bool {
        self.weapon_templates.iter().any(|w| w.is_some())
    }

    /// Get weapon template for specific slot
    pub fn get_weapon_template(&self, slot: WeaponSlotType) -> Option<&Arc<WeaponTemplate>> {
        self.weapon_templates.get(slot as usize)?.as_ref()
    }

    /// Set weapon template for specific slot
    pub fn set_weapon_template(&mut self, slot: WeaponSlotType, template: Arc<WeaponTemplate>) {
        if let Some(slot_ref) = self.weapon_templates.get_mut(slot as usize) {
            *slot_ref = Some(template);
        }
    }

    /// Get auto-choose mask for specific slot
    pub fn get_auto_choose_mask(&self, slot: WeaponSlotType) -> u32 {
        self.auto_choose_mask
            .get(slot as usize)
            .copied()
            .unwrap_or(0xffffffff)
    }

    /// Get preferred against mask for specific slot
    pub fn get_preferred_against_mask(&self, slot: WeaponSlotType) -> KindOfMask {
        *self
            .preferred_against
            .get(slot as usize)
            .unwrap_or(&KindOfMask::empty())
    }
}

impl Default for WeaponTemplateSet {
    fn default() -> Self {
        Self::new()
    }
}

fn weapon_set_type_from_index(index: usize) -> Option<WeaponSetType> {
    match index {
        0 => Some(WeaponSetType::Veteran),
        1 => Some(WeaponSetType::Elite),
        2 => Some(WeaponSetType::Hero),
        3 => Some(WeaponSetType::PlayerUpgrade),
        4 => Some(WeaponSetType::CrateUpgradeOne),
        5 => Some(WeaponSetType::CrateUpgradeTwo),
        6 => Some(WeaponSetType::VehicleHijack),
        7 => Some(WeaponSetType::CarBomb),
        8 => Some(WeaponSetType::MineClearingDetail),
        9 => Some(WeaponSetType::WeaponRider1),
        10 => Some(WeaponSetType::WeaponRider2),
        11 => Some(WeaponSetType::WeaponRider3),
        12 => Some(WeaponSetType::WeaponRider4),
        13 => Some(WeaponSetType::WeaponRider5),
        14 => Some(WeaponSetType::WeaponRider6),
        15 => Some(WeaponSetType::WeaponRider7),
        16 => Some(WeaponSetType::WeaponRider8),
        _ => None,
    }
}

/// Weapon set condition types for state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeaponSetConditionType {
    None,
    Firing,
    Between,
    Reloading,
    PreAttack,
}

/// Weapon choice criteria for target selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponChoiceCriteria {
    /// Choose weapon that will do the most damage
    PreferMostDamage,
    /// Choose weapon with the longest range that can do damage
    PreferLongestRange,
}

/// Weapon lock type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponLockType {
    /// Weapon is not locked
    NotLocked,
    /// Weapon is locked until clip is empty or current attack state exits
    LockedTemporarily,
    /// Weapon is locked until explicitly unlocked
    LockedPermanently,
}

fn weapon_slot_to_u32(slot: WeaponSlotType) -> u32 {
    match slot {
        WeaponSlotType::Primary => 0,
        WeaponSlotType::Secondary => 1,
        WeaponSlotType::Tertiary => 2,
    }
}

fn weapon_slot_from_u32(value: u32) -> WeaponSlotType {
    match value {
        1 => WeaponSlotType::Secondary,
        2 => WeaponSlotType::Tertiary,
        _ => WeaponSlotType::Primary,
    }
}

fn weapon_lock_type_to_u32(lock: WeaponLockType) -> u32 {
    match lock {
        WeaponLockType::NotLocked => 0,
        WeaponLockType::LockedTemporarily => 1,
        WeaponLockType::LockedPermanently => 2,
    }
}

fn weapon_lock_type_from_u32(value: u32) -> WeaponLockType {
    match value {
        1 => WeaponLockType::LockedTemporarily,
        2 => WeaponLockType::LockedPermanently,
        _ => WeaponLockType::NotLocked,
    }
}

// DamageTypeFlags is defined in src/damage.rs
// Import with: use crate::damage::DamageTypeFlags;

/// Main weapon set managing multiple weapons and their coordination
#[derive(Debug)]
pub struct WeaponSet {
    /// Current active weapon template set
    current_weapon_template_set: Option<Arc<WeaponTemplateSet>>,
    /// All available weapon template sets
    weapon_template_sets: Vec<Arc<WeaponTemplateSet>>,
    /// Actual weapon instances
    weapons: [Option<Weapon>; 3], // PRIMARY, SECONDARY, TERTIARY
    /// Currently selected weapon
    current_weapon: WeaponSlotType,
    /// Current weapon lock status
    current_weapon_locked_status: WeaponLockType,
    /// Mask of filled weapon slots
    filled_weapon_slot_mask: u8,
    /// Combined anti-mask of all weapons
    total_anti_mask: u32,
    /// Combined damage type mask of all weapons
    total_damage_type_mask: crate::damage::DamageTypeFlags,
    /// Whether any weapon has pitch limitations
    has_pitch_limit: bool,
    /// Whether any weapon does damage
    has_damage_weapon: bool,
}

impl WeaponSet {
    pub fn new() -> Self {
        Self {
            current_weapon_template_set: None,
            weapon_template_sets: Vec::new(),
            weapons: [None, None, None],
            current_weapon: WeaponSlotType::Primary,
            current_weapon_locked_status: WeaponLockType::NotLocked,
            filled_weapon_slot_mask: 0,
            total_anti_mask: 0,
            total_damage_type_mask: crate::damage::DamageTypeFlags::empty(),
            has_pitch_limit: false,
            has_damage_weapon: false,
        }
    }

    /// Add a weapon template set
    pub fn add_weapon_template_set(&mut self, template_set: WeaponTemplateSet) {
        self.weapon_template_sets.push(Arc::new(template_set));
    }

    /// Serialize WeaponSet state for save/load parity with C++ WeaponSet::xfer.
    pub fn xfer_state(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            let index = slot as usize;
            let mut has_weapon = self.weapons[index].is_some();
            xfer.xfer_bool(&mut has_weapon).map_err(|e| e.to_string())?;

            if has_weapon {
                if xfer.get_xfer_mode() == XferMode::Load && self.weapons[index].is_none() {
                    let template = self.current_weapon_template_set.as_ref().and_then(|set| {
                        set.get_weapon_template(slot)
                            .cloned()
                            .or_else(|| set.get_weapon_template(WeaponSlotType::Primary).cloned())
                    });

                    if let Some(template) = template {
                        self.weapons[index] = Some(Weapon::new(template, slot));
                    }
                }

                let weapon = self.weapons[index].as_mut().ok_or_else(|| {
                    format!("WeaponSet::xfer_state missing weapon in slot {:?}", slot)
                })?;
                weapon.xfer(xfer)?;
            } else if xfer.get_xfer_mode() == XferMode::Load {
                self.weapons[index] = None;
            }
        }

        let mut current_weapon = weapon_slot_to_u32(self.current_weapon);
        xfer.xfer_unsigned_int(&mut current_weapon)
            .map_err(|e| e.to_string())?;
        self.current_weapon = weapon_slot_from_u32(current_weapon);

        let mut lock_status = weapon_lock_type_to_u32(self.current_weapon_locked_status);
        xfer.xfer_unsigned_int(&mut lock_status)
            .map_err(|e| e.to_string())?;
        self.current_weapon_locked_status = weapon_lock_type_from_u32(lock_status);

        let mut filled_weapon_slot_mask = self.filled_weapon_slot_mask as u32;
        xfer.xfer_unsigned_int(&mut filled_weapon_slot_mask)
            .map_err(|e| e.to_string())?;
        self.filled_weapon_slot_mask = (filled_weapon_slot_mask & 0xFF) as u8;

        let mut total_anti_mask = self.total_anti_mask as i32;
        xfer.xfer_int(&mut total_anti_mask)
            .map_err(|e| e.to_string())?;
        self.total_anti_mask = total_anti_mask as u32;

        let mut has_damage_weapon_a = self.has_damage_weapon;
        xfer.xfer_bool(&mut has_damage_weapon_a)
            .map_err(|e| e.to_string())?;
        self.has_damage_weapon = has_damage_weapon_a;

        // Intentional duplicate field xfer matches C++ WeaponSet::xfer legacy behavior.
        let mut has_damage_weapon_b = self.has_damage_weapon;
        xfer.xfer_bool(&mut has_damage_weapon_b)
            .map_err(|e| e.to_string())?;

        let mut total_damage_lo = (self.total_damage_type_mask.bits() & 0xFFFF_FFFF) as u32;
        let mut total_damage_hi = (self.total_damage_type_mask.bits() >> 32) as u32;
        xfer.xfer_unsigned_int(&mut total_damage_lo)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut total_damage_hi)
            .map_err(|e| e.to_string())?;
        let total_damage_type_mask = ((total_damage_hi as u64) << 32) | (total_damage_lo as u64);
        self.total_damage_type_mask =
            crate::damage::DamageTypeFlags::from_bits_retain(total_damage_type_mask);

        // Recompute derived fields such as pitch limits after load.
        if xfer.get_xfer_mode() == XferMode::Load {
            self.update_weapon_statistics();
            self.current_weapon = weapon_slot_from_u32(current_weapon);
            self.current_weapon_locked_status = weapon_lock_type_from_u32(lock_status);
        }

        Ok(())
    }

    /// Update weapon set based on current object conditions
    pub fn update_weapon_set(
        &mut self,
        object_id: ObjectID,
        conditions: &WeaponSetFlags,
    ) -> GameLogicResult<()> {
        // Find best matching weapon template set
        let best_set = self.find_best_weapon_template_set(conditions);

        if let Some(best_set) = best_set {
            if self
                .current_weapon_template_set
                .as_ref()
                .map_or(true, |current| !Arc::ptr_eq(current, &best_set))
            {
                // Switch to new weapon template set
                self.switch_weapon_template_set(best_set, object_id)?;
            }
        }

        // Update weapon statistics
        self.update_weapon_statistics();

        Ok(())
    }

    /// Find the best weapon template set for given conditions
    fn find_best_weapon_template_set(
        &self,
        conditions: &WeaponSetFlags,
    ) -> Option<Arc<WeaponTemplateSet>> {
        // Find the set with the most specific matching conditions
        let mut best_set: Option<Arc<WeaponTemplateSet>> = None;
        let mut best_match_count = 0;

        for template_set in &self.weapon_template_sets {
            if template_set.matches_conditions(conditions) {
                // Count how many conditions this set specifies
                let mut match_count = 0;
                for weapon_set_type in [
                    WeaponSetType::Veteran,
                    WeaponSetType::Elite,
                    WeaponSetType::Hero,
                    WeaponSetType::PlayerUpgrade,
                    WeaponSetType::CrateUpgradeOne,
                    WeaponSetType::CrateUpgradeTwo,
                    WeaponSetType::VehicleHijack,
                    WeaponSetType::CarBomb,
                    WeaponSetType::MineClearingDetail,
                    WeaponSetType::WeaponRider1,
                    WeaponSetType::WeaponRider2,
                    WeaponSetType::WeaponRider3,
                    WeaponSetType::WeaponRider4,
                    WeaponSetType::WeaponRider5,
                    WeaponSetType::WeaponRider6,
                    WeaponSetType::WeaponRider7,
                    WeaponSetType::WeaponRider8,
                ] {
                    if template_set.conditions.test(weapon_set_type) {
                        match_count += 1;
                    }
                }

                if best_set.is_none() || match_count > best_match_count {
                    best_match_count = match_count;
                    best_set = Some(Arc::clone(template_set));
                }
            }
        }

        best_set
    }

    /// Find the best weapon template set for the given conditions (public wrapper).
    pub fn find_weapon_template_set(
        &self,
        conditions: &WeaponSetFlags,
    ) -> Option<Arc<WeaponTemplateSet>> {
        self.find_best_weapon_template_set(conditions)
    }

    /// Switch to a new weapon template set
    fn switch_weapon_template_set(
        &mut self,
        new_set: Arc<WeaponTemplateSet>,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        self.current_weapon_template_set = Some(Arc::clone(&new_set));

        // C++ WeaponSet.cpp:281-286: If weapon lock is NOT shared across sets,
        // release ALL locks and reset curWeapon to PRIMARY.
        if !new_set.is_weapon_lock_shared_across_sets {
            if self.current_weapon_locked_status != WeaponLockType::NotLocked {
                log::debug!(
                    "changing WeaponSet while Weapon is Locked... implicit unlock occurring!"
                );
            }
            self.current_weapon_locked_status = WeaponLockType::NotLocked;
            self.current_weapon = WeaponSlotType::Primary;
        }

        // Create new weapons based on template set
        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            let slot_index = slot as usize;

            if let Some(template) = new_set.get_weapon_template(slot) {
                // Create new weapon
                let mut new_weapon = Weapon::new(Arc::clone(template), slot);

                // C++ WeaponSet.cpp:303: loadAmmoNow - start with full clips
                new_weapon.load_ammo_now(object_id).ok();

                self.weapons[slot_index] = Some(new_weapon);
                self.filled_weapon_slot_mask |= 1 << slot_index;
            } else {
                self.weapons[slot_index] = None;
                self.filled_weapon_slot_mask &= !(1 << slot_index);
            }
        }

        Ok(())
    }

    /// Update weapon statistics and capabilities
    fn update_weapon_statistics(&mut self) {
        self.total_anti_mask = 0;
        self.total_damage_type_mask.clear_all();
        self.has_pitch_limit = false;
        self.has_damage_weapon = false;

        for weapon_opt in &self.weapons {
            if let Some(weapon) = weapon_opt {
                let template = weapon.get_template();

                // Combine anti-masks
                self.total_anti_mask |= template.anti_mask.0;

                // Combine damage types
                self.total_damage_type_mask |= crate::damage::DamageTypeFlags::from_bits_truncate(
                    1 << template.damage_type as u64,
                );

                // Check capabilities
                if weapon.is_pitch_limited() {
                    self.has_pitch_limit = true;
                }

                if weapon.is_damage_weapon() {
                    self.has_damage_weapon = true;
                }
            }
        }
    }

    /// Choose best weapon for target
    ///
    /// Matches C++ WeaponSet::chooseBestWeaponForTarget() from WeaponSet.cpp lines 764-948
    ///
    /// The selection algorithm considers:
    /// 1. Weapon fitness - can the weapon hit the target?
    /// 2. Weapon readiness - is the weapon ready to fire?
    /// 3. Damage potential - how much damage would it do?
    /// 4. "Preferred against" bonuses - does this weapon prefer this target type?
    pub fn choose_best_weapon_for_target(
        &mut self,
        source_obj: ObjectID,
        target_obj: ObjectID,
        criteria: WeaponChoiceCriteria,
        command_source: CommandSourceType,
    ) -> GameLogicResult<bool> {
        // C++ line 782-783: If weapon is locked, return true immediately
        if self.is_current_weapon_locked() {
            return Ok(true);
        }

        // C++ line 785-791: If no target, default to primary weapon
        if target_obj == 0 {
            self.current_weapon = WeaponSlotType::Primary;
            return Ok(true);
        }

        let mut found = false; // A ready weapon has been found
        let mut found_backup = false; // An unready but valid weapon has been found

        let mut longest_range: f32 = 0.0;
        let mut best_damage: f32 = 0.0;
        let mut longest_range_backup: f32 = 0.0;
        let mut best_damage_backup: f32 = 0.0;

        let mut current_decision = WeaponSlotType::Primary;
        let mut current_decision_backup = WeaponSlotType::Primary;

        // C++ line 804-805: Go backwards so primary is preferred in case of ties
        for slot_idx in (0..=2).rev() {
            let slot = match slot_idx {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };

            // C++ line 812: No weapon in this slot
            let weapon = match self.get_weapon_in_slot(slot) {
                Some(w) => w,
                None => continue,
            };

            // C++ line 816-823: Check if weapon is allowed for this command source
            if let Some(template_set) = &self.current_weapon_template_set {
                let ok_srcs = template_set.get_auto_choose_mask(slot);
                let source_bit = 1u32 << (command_source as i32);
                if (ok_srcs & source_bit) == 0 {
                    // Check if CMD_DEFAULT_SWITCH_WEAPON is set
                    const CMD_DEFAULT_SWITCH_WEAPON: u32 = 0x80000000;
                    if (ok_srcs & CMD_DEFAULT_SWITCH_WEAPON) == 0 {
                        continue;
                    }
                }
            }

            // C++ line 834-835: Weapon out of ammo and doesn't auto-reload
            if weapon.get_status() == WeaponStatus::OutOfAmmo
                && !weapon.get_template().get_auto_reloads_clip()
            {
                continue;
            }

            // C++ line 838-840: Check anti-mask against victim KINDOF flags
            if let Some(victim_anti_mask) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_obj, |target_guard| target_guard.get_anti_mask())
            {
                if (weapon.get_template().anti_mask.0 & victim_anti_mask) == 0 {
                    continue;
                }
            }

            // C++ line 842-843: Check target pitch limits
            if !weapon.is_within_target_pitch(source_obj, target_obj) {
                continue;
            }

            let damage = weapon.estimate_weapon_damage(source_obj, Some(target_obj), None);
            let attack_range = weapon.get_attack_range(source_obj);

            // C++ line 847: Check if weapon is ready to fire
            let mut weapon_is_ready = weapon.get_status() == WeaponStatus::ReadyToFire;

            // C++ line 849-851: Check if weapon is on turret and aiming at target
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(source_obj, |source_guard| {
                    let Some(ai) = source_guard.get_ai() else {
                        return false;
                    };
                    ai.lock()
                        .ok()
                        .map(|ai_guard| {
                            ai_guard.is_weapon_slot_on_turret_and_aiming_at_target(slot, target_obj)
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false)
            {
                weapon_is_ready = false;
            }

            // C++ line 853-856: Weapon would do no damage (unless DAMAGE_UNRESISTABLE)
            if damage <= 0.0 && weapon.get_damage_type() != DamageType::Unresistable {
                continue;
            }

            // C++ lines 869-878: Check "preferred against" bonuses
            // If weapon is preferred against this target type, boost its score massively
            let mut damage = damage;
            let mut attack_range = attack_range;
            if let Some(template_set) = &self.current_weapon_template_set {
                let preferred_mask = template_set.get_preferred_against_mask(slot);
                if !preferred_mask.is_empty() {
                    // C++ line 870: victim->isKindOfMulti(preferredAgainst, KINDOFMASK_NONE)
                    if crate::object::registry::OBJECT_REGISTRY
                        .with_object(target_obj, |target_guard| {
                            target_guard.is_kind_of_mask(preferred_mask.bits() as u32)
                        })
                        .unwrap_or(false)
                    {
                        // C++ lines 872-878: Boost damage/range massively for preferred targets
                        const HUGE_DAMAGE: f32 = 1e10;
                        const HUGE_RANGE: f32 = 1e10;
                        damage = HUGE_DAMAGE;
                        attack_range = HUGE_RANGE;
                        // Preferred weapons are kept if merely reloading (not out of ammo)
                        weapon_is_ready = weapon.get_status() != WeaponStatus::OutOfAmmo;
                    }
                }
            }

            // C++ lines 880-925: Apply selection criteria
            match criteria {
                WeaponChoiceCriteria::PreferMostDamage => {
                    if !weapon_is_ready {
                        // Backup choice
                        if damage >= best_damage_backup {
                            best_damage_backup = damage;
                            current_decision_backup = slot;
                            found_backup = true;
                        }
                    } else {
                        // Ready choice
                        if damage >= best_damage {
                            best_damage = damage;
                            current_decision = slot;
                            found = true;
                        }
                    }
                }
                WeaponChoiceCriteria::PreferLongestRange => {
                    if !weapon_is_ready {
                        if attack_range > longest_range_backup {
                            longest_range_backup = attack_range;
                            current_decision_backup = slot;
                            found_backup = true;
                        }
                    } else {
                        if attack_range > longest_range {
                            longest_range = attack_range;
                            current_decision = slot;
                            found = true;
                        }
                    }
                }
            }
        }

        // C++ lines 928-943: Select final weapon
        if found {
            // Found a good ready weapon
            self.current_weapon = current_decision;
        } else if found_backup {
            // No ready weapon, use the best unready one
            self.current_weapon = current_decision_backup;
            found = true;
        } else {
            // No weapon at all, go back to primary
            self.current_weapon = WeaponSlotType::Primary;
        }

        Ok(found)
    }

    pub fn get_model_condition_for_weapon_slot(
        slot: WeaponSlotType,
        condition: WeaponSetConditionType,
    ) -> ModelConditionFlags {
        let mut flags = ModelConditionFlags::empty();
        let condition_flag = match (slot, condition) {
            (WeaponSlotType::Primary, WeaponSetConditionType::Firing) => {
                ModelConditionFlags::FiringA
            }
            (WeaponSlotType::Primary, WeaponSetConditionType::Between) => {
                ModelConditionFlags::BetweenFiringShotsA
            }
            (WeaponSlotType::Primary, WeaponSetConditionType::Reloading) => {
                ModelConditionFlags::ReloadingA
            }
            (WeaponSlotType::Primary, WeaponSetConditionType::PreAttack) => {
                ModelConditionFlags::PreAttackA
            }
            (WeaponSlotType::Secondary, WeaponSetConditionType::Firing) => {
                ModelConditionFlags::FiringB
            }
            (WeaponSlotType::Secondary, WeaponSetConditionType::Between) => {
                ModelConditionFlags::BetweenFiringShotsB
            }
            (WeaponSlotType::Secondary, WeaponSetConditionType::Reloading) => {
                ModelConditionFlags::ReloadingB
            }
            (WeaponSlotType::Secondary, WeaponSetConditionType::PreAttack) => {
                ModelConditionFlags::PreAttackB
            }
            (WeaponSlotType::Tertiary, WeaponSetConditionType::Firing) => {
                ModelConditionFlags::FiringC
            }
            (WeaponSlotType::Tertiary, WeaponSetConditionType::Between) => {
                ModelConditionFlags::BetweenFiringShotsC
            }
            (WeaponSlotType::Tertiary, WeaponSetConditionType::Reloading) => {
                ModelConditionFlags::ReloadingC
            }
            (WeaponSlotType::Tertiary, WeaponSetConditionType::PreAttack) => {
                ModelConditionFlags::PreAttackC
            }
            _ => ModelConditionFlags::empty(),
        };

        if condition_flag != ModelConditionFlags::empty() {
            flags |= condition_flag;
        }

        if condition != WeaponSetConditionType::None {
            let using_flag = match slot {
                WeaponSlotType::Primary => ModelConditionFlags::UsingWeaponA,
                WeaponSlotType::Secondary => ModelConditionFlags::UsingWeaponB,
                WeaponSlotType::Tertiary => ModelConditionFlags::UsingWeaponC,
            };
            flags |= using_flag;
        }

        flags
    }

    /// Set weapon lock for specific weapon
    ///
    /// Matches C++ WeaponSet::setWeaponLock() from WeaponSet.cpp lines 1035-1065
    ///
    /// Logic:
    /// - NOT_LOCKED: invalid call, returns false (use release_weapon_lock instead)
    /// - LOCKED_PERMANENTLY: always sets weapon and lock, overriding any existing lock
    /// - LOCKED_TEMPORARILY: only sets if current lock is NOT LOCKED_PERMANENTLY
    pub fn set_weapon_lock(
        &mut self,
        weapon_slot: WeaponSlotType,
        lock_type: WeaponLockType,
    ) -> bool {
        // C++ line 1037-1041: NOT_LOCKED is invalid for set
        if lock_type == WeaponLockType::NotLocked {
            log::warn!("set_weapon_lock called with NotLocked; use release_weapon_lock instead");
            return false;
        }

        // C++ line 1045: Verify weapon exists in the requested slot
        if self.get_weapon_in_slot(weapon_slot).is_none() {
            return false;
        }

        // C++ lines 1047-1063: Apply lock based on type
        if lock_type == WeaponLockType::LockedPermanently {
            // Permanent lock always wins (C++ line 1047-1051)
            self.current_weapon = weapon_slot;
            self.current_weapon_locked_status = lock_type;
            true
        } else if lock_type == WeaponLockType::LockedTemporarily {
            // Temporary lock only if not permanently locked (C++ line 1053-1055)
            if self.current_weapon_locked_status != WeaponLockType::LockedPermanently {
                self.current_weapon = weapon_slot;
                self.current_weapon_locked_status = lock_type;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Release weapon lock
    /// C++ Reference: WeaponSet.cpp:1070-1090
    ///
    /// - LOCKED_PERMANENTLY: releases ALL locks regardless of current lock type
    /// - LOCKED_TEMPORARILY: only releases if current lock is temporary
    pub fn release_weapon_lock(&mut self, lock_type: WeaponLockType) {
        if self.current_weapon_locked_status == WeaponLockType::NotLocked {
            return; // Nothing to do
        }

        if lock_type == WeaponLockType::LockedPermanently {
            // All locks released (matches C++ WeaponSet.cpp:1075-1078)
            self.current_weapon_locked_status = WeaponLockType::NotLocked;
        } else if lock_type == WeaponLockType::LockedTemporarily {
            // Only unlocked if the current lock is temporary
            // (matches C++ WeaponSet.cpp:1080-1084)
            if self.current_weapon_locked_status == WeaponLockType::LockedTemporarily {
                self.current_weapon_locked_status = WeaponLockType::NotLocked;
            }
        } else {
            debug_assert!(
                false,
                "release_weapon_lock called with NotLocked; this matches invalid C++ call path"
            );
            log::warn!(
                "release_weapon_lock called with NotLocked; call site should pass Temporary or Permanent"
            );
        }
    }

    /// Check if current weapon is locked
    pub fn is_current_weapon_locked(&self) -> bool {
        self.current_weapon_locked_status != WeaponLockType::NotLocked
    }

    /// Get weapon in specific slot
    pub fn get_weapon_in_slot(&self, slot: WeaponSlotType) -> Option<&Weapon> {
        self.weapons.get(slot as usize)?.as_ref()
    }

    /// Get weapon in weapon slot (alias for compatibility)
    pub fn get_weapon_in_weapon_slot(&self, slot: WeaponSlotType) -> Option<&Weapon> {
        self.get_weapon_in_slot(slot)
    }

    /// Get mutable weapon in specific slot
    pub fn get_weapon_in_slot_mut(&mut self, slot: WeaponSlotType) -> Option<&mut Weapon> {
        self.weapons.get_mut(slot as usize)?.as_mut()
    }

    /// Get current weapon
    pub fn get_current_weapon(&self) -> Option<(&Weapon, WeaponSlotType)> {
        self.get_weapon_in_slot(self.current_weapon)
            .map(|weapon| (weapon, self.current_weapon))
    }

    /// Get mutable current weapon
    pub fn get_current_weapon_mut(&mut self) -> Option<&mut Weapon> {
        self.get_weapon_in_slot_mut(self.current_weapon)
    }

    /// Get current weapon slot
    pub fn get_current_weapon_slot(&self) -> WeaponSlotType {
        self.current_weapon
    }

    /// Check if weapon set has any weapons
    pub fn has_any_weapon(&self) -> bool {
        self.filled_weapon_slot_mask != 0
    }

    /// Legacy alias used by object code during the C++ parity port.
    pub fn has_any_weapons(&self) -> bool {
        self.has_any_weapon()
    }

    /// Check if weapon set has any damage-dealing weapons
    pub fn has_any_damage_weapon(&self) -> bool {
        self.has_damage_weapon
    }

    /// Check if weapon set can deal specific damage type
    pub fn has_weapon_to_deal_damage_type(&self, damage_type: crate::damage::DamageType) -> bool {
        self.total_damage_type_mask.test(damage_type)
    }

    /// Check if weapon set deals only one damage type
    pub fn has_single_damage_type(&self, damage_type: crate::damage::DamageType) -> bool {
        self.total_damage_type_mask.test(damage_type) && self.total_damage_type_mask.count() == 1
    }

    /// Check if any weapon is out of ammo
    pub fn is_out_of_ammo(&self) -> bool {
        for weapon_opt in &self.weapons {
            if let Some(weapon) = weapon_opt {
                if weapon.get_status() != WeaponStatus::OutOfAmmo {
                    return false;
                }
            }
        }
        true
    }

    /// Reload all weapons
    pub fn reload_all_ammo(
        &mut self,
        source_obj: ObjectID,
        reload_now: bool,
    ) -> GameLogicResult<()> {
        for weapon_opt in &mut self.weapons {
            if let Some(weapon) = weapon_opt {
                if reload_now {
                    weapon.load_ammo_now(source_obj)?;
                } else {
                    weapon.reload_ammo(source_obj)?;
                }
            }
        }
        Ok(())
    }

    /// Get most ready weapon percentage
    pub fn get_most_percent_ready_to_fire_any_weapon(&self) -> f32 {
        let mut max_ready = 0.0;

        for weapon_opt in &self.weapons {
            if let Some(weapon) = weapon_opt {
                let ready_percent = weapon.get_percent_ready_to_fire();
                if ready_percent > max_ready {
                    max_ready = ready_percent;
                }
            }
        }

        max_ready
    }

    /// Find weapon capable of following waypoints
    ///
    /// Matches C++ WeaponSet::findWaypointFollowingCapableWeapon() from WeaponSet.cpp line 998
    /// C++ iterates REVERSE (WEAPONSLOT_COUNT-1 down to PRIMARY) so tertiary is preferred.
    pub fn find_waypoint_following_capable_weapon(&mut self) -> Option<&mut Weapon> {
        // First pass: find the index (avoids borrow checker issues)
        let mut found_idx: Option<usize> = None;
        for slot_idx in (0..crate::common::WEAPONSLOT_COUNT).rev() {
            if let Some(weapon) = &self.weapons[slot_idx] {
                if weapon.get_template().capable_of_following_waypoint {
                    found_idx = Some(slot_idx);
                    break;
                }
            }
        }

        // Second pass: return mutable reference at found index
        if let Some(idx) = found_idx {
            self.weapons[idx].as_mut()
        } else {
            None
        }
    }

    /// Find weapon that shows ammo pips
    pub fn find_ammo_pip_showing_weapon(&self) -> Option<&Weapon> {
        for weapon_opt in &self.weapons {
            if let Some(weapon) = weapon_opt {
                if weapon.get_template().is_shows_ammo_pips {
                    return Some(weapon);
                }
            }
        }
        None
    }

    /// Update all weapons when weapon bonus changes
    pub fn weapon_set_on_weapon_bonus_change(&mut self, source: ObjectID) -> GameLogicResult<()> {
        for weapon_opt in &mut self.weapons {
            if let Some(weapon) = weapon_opt {
                weapon.on_weapon_bonus_change(source)?;
            }
        }
        Ok(())
    }

    /// Clear leech range mode for all weapons
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        for weapon_opt in &mut self.weapons {
            if let Some(weapon) = weapon_opt {
                weapon.set_leech_range_active(false);
            }
        }
    }

    /// Check if reload time is shared across weapons
    pub fn is_shared_reload_time(&self) -> bool {
        self.current_weapon_template_set
            .as_ref()
            .map_or(false, |set| set.is_reload_time_shared)
    }

    /// Get the command source mask for a specific weapon slot
    ///
    /// Matches C++ WeaponSet::getNthCommandSourceMask() from WeaponSet.h line 215
    pub fn get_nth_command_source_mask(&self, slot: WeaponSlotType) -> u32 {
        self.current_weapon_template_set
            .as_ref()
            .map(|set| set.get_auto_choose_mask(slot))
            .unwrap_or(0)
    }

    /// Check weapon capability against specific object
    ///
    /// Matches C++ WeaponSet::getAbleToAttackSpecificObject() from WeaponSet.h line 231
    pub fn get_able_to_attack_specific_object(
        &self,
        attack_type: AbleToAttackType,
        source_obj: ObjectID,
        target_obj: ObjectID,
        command_source: CommandSourceType,
        specific_slot: Option<WeaponSlotType>,
    ) -> CanAttackResult {
        if let Some(slot) = specific_slot {
            // Check specific weapon slot
            if let Some(weapon) = self.get_weapon_in_slot(slot) {
                self.evaluate_weapon_against_target(
                    weapon,
                    source_obj,
                    target_obj,
                    attack_type,
                    command_source,
                )
            } else {
                CanAttackResult::NotPossible
            }
        } else {
            // Check all weapons and return best result
            let mut best_result = CanAttackResult::NotPossible;

            for weapon_opt in &self.weapons {
                if let Some(weapon) = weapon_opt {
                    let result = self.evaluate_weapon_against_target(
                        weapon,
                        source_obj,
                        target_obj,
                        attack_type,
                        command_source,
                    );

                    if result as u32 > best_result as u32 {
                        best_result = result;

                        if best_result == CanAttackResult::Possible {
                            break; // Found best possible result
                        }
                    }
                }
            }

            best_result
        }
    }

    /// Determine if the unit can use its weapon against a target position
    ///
    /// Matches C++ WeaponSet::getAbleToUseWeaponAgainstTarget() from WeaponSet.cpp line 581
    /// Supports both attacking an object and attacking the position on the ground.
    pub fn get_able_to_use_weapon_against_target(
        &self,
        _attack_type: AbleToAttackType,
        source_obj: ObjectID,
        target_obj: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        _command_source: CommandSourceType,
        specific_slot: Option<WeaponSlotType>,
    ) -> CanAttackResult {
        // C++ WeaponSet.cpp line 589-603: Determine anti-mask from target
        let target_anti_mask = if let Some(target_id) = target_obj {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |obj_guard| obj_guard.get_anti_mask())
                .unwrap_or(0xffffffff)
        } else {
            0xffffffff // Ground or no target
        };

        // C++ WeaponSet.cpp line 607-626: Check pitch limits when targeting an object
        let pitch_ok = if let Some(target_id) = target_obj {
            self.is_any_within_target_pitch(source_obj, target_id)
        } else {
            true
        };

        // Collect weapon references to check
        let slots_to_check: Vec<usize> = if let Some(slot) = specific_slot {
            vec![slot as usize]
        } else {
            vec![0, 1, 2]
        };

        let mut best_result = CanAttackResult::NotPossible;

        for slot_idx in slots_to_check {
            let weapon = match self.weapons.get(slot_idx).and_then(|w| w.as_ref()) {
                Some(w) => w,
                None => continue,
            };

            // C++ WeaponSet.cpp line 638-639: Check anti-mask against target
            let weapon_anti = weapon.template.get_anti_mask();
            if (weapon_anti & target_anti_mask) == 0 {
                continue;
            }

            // C++ WeaponSet.cpp line 641-644: Skip weapons out of pitch range
            if !pitch_ok && !weapon.is_within_target_pitch(source_obj, target_obj.unwrap_or(0)) {
                continue;
            }

            // C++ WeaponSet.cpp line 650-656: Check if out of ammo and not auto-reload
            if weapon.get_status() == WeaponStatus::OutOfAmmo
                && !weapon.template.get_auto_reloads_clip()
            {
                continue;
            }

            // C++ WeaponSet.cpp line 662-666: Check damage > 0 (unless unresistable)
            let damage = weapon.estimate_weapon_damage(source_obj, target_obj, target_pos);
            if damage <= 0.0 && weapon.get_damage_type() != DamageType::Unresistable {
                continue;
            }

            // C++ WeaponSet.cpp line 668-676: Check attack range
            if weapon.is_within_attack_range(source_obj, target_obj, target_pos) {
                return CanAttackResult::Possible;
            } else if !matches!(best_result, CanAttackResult::Possible) {
                best_result = CanAttackResult::PossibleAfterMoving;
            }
        }

        best_result
    }

    /// Check if any weapon is within target pitch limits
    ///
    /// Matches C++ WeaponSet::isAnyWithinTargetPitch() from WeaponSet.h line 187
    fn is_any_within_target_pitch(&self, source_obj: ObjectID, target_obj: ObjectID) -> bool {
        for weapon_opt in &self.weapons {
            if let Some(weapon) = weapon_opt {
                if weapon.is_within_target_pitch(source_obj, target_obj) {
                    return true;
                }
            }
        }
        false
    }

    /// Evaluate a specific weapon against target
    fn evaluate_weapon_against_target(
        &self,
        weapon: &Weapon,
        source_obj: ObjectID,
        target_obj: ObjectID,
        _attack_type: AbleToAttackType,
        _command_source: CommandSourceType,
    ) -> CanAttackResult {
        // Check anti-mask
        let weapon_anti = weapon.template.get_anti_mask();
        let target_anti = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_obj, |guard| guard.get_anti_mask())
            .unwrap_or(0xffffffff);
        if (weapon_anti & target_anti) == 0 {
            return CanAttackResult::InvalidShot;
        }

        // Check if weapon is in range
        if !weapon.is_within_attack_range(source_obj, Some(target_obj), None) {
            return CanAttackResult::PossibleAfterMoving;
        }

        // Check if weapon can do meaningful damage
        let estimated_damage = weapon.estimate_weapon_damage(source_obj, Some(target_obj), None);
        if estimated_damage <= 0.0 {
            return CanAttackResult::InvalidShot;
        }

        CanAttackResult::Possible
    }
}

impl Default for WeaponSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::WeaponTemplate;
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::bit_flags::WeaponSetFlags as EngineWeaponSetBits;
    use game_engine::common::system::kind_of::KindOfMask;
    use game_engine::thing::thing_template::WeaponTemplateSet as EngineWeaponTemplateSet;

    fn test_weapon_template(name: &str, clip_size: i32) -> Arc<WeaponTemplate> {
        let mut template = WeaponTemplate::new(name.to_string());
        template.clip_size = clip_size;
        Arc::new(template)
    }

    fn test_template_set(
        primary: Arc<WeaponTemplate>,
        secondary: Option<Arc<WeaponTemplate>>,
        shared_lock: bool,
    ) -> Arc<WeaponTemplateSet> {
        let mut set = WeaponTemplateSet::new();
        set.is_weapon_lock_shared_across_sets = shared_lock;
        set.set_weapon_template(WeaponSlotType::Primary, primary);
        if let Some(secondary) = secondary {
            set.set_weapon_template(WeaponSlotType::Secondary, secondary);
        }
        Arc::new(set)
    }

    #[test]
    fn engine_weapon_set_conversion_resolves_templates() {
        let mut engine_set = EngineWeaponTemplateSet::new();
        engine_set.types_mut().set(EngineWeaponSetBits::HERO, true);
        engine_set.set_weapon_template_name(0, Some(AsciiString::from("HeroWeapon").to_string()));
        engine_set.set_auto_choose_mask(0, 0x7);
        engine_set.set_preferred_against_mask(0, KindOfMask::from_bits_truncate(0x8));
        engine_set.set_reload_time_shared(true);
        engine_set.set_weapon_lock_shared_across_sets(true);

        let mut resolver: HashMap<String, Arc<WeaponTemplate>> = HashMap::new();
        resolver.insert(
            "HeroWeapon".to_string(),
            Arc::new(WeaponTemplate::new("HeroWeapon".to_string())),
        );

        let converted = WeaponTemplateSet::from_engine_set(&engine_set, |name| {
            resolver.get(name.as_str()).cloned()
        })
        .expect("convert weapon set");

        assert!(converted.conditions.test(WeaponSetType::Hero));
        assert!(converted.is_reload_time_shared);
        assert!(converted.is_weapon_lock_shared_across_sets);
        assert_eq!(converted.auto_choose_mask[0], 0x7);
        assert_eq!(
            converted.preferred_against[0],
            KindOfMask::from_bits_truncate(0x8)
        );
        assert!(converted.weapon_templates[0].is_some());
    }

    #[test]
    fn test_weapon_set_flags() {
        let mut flags = WeaponSetFlags::new();
        assert!(flags.is_empty());

        flags.set(WeaponSetType::Veteran);
        assert!(flags.test(WeaponSetType::Veteran));
        assert!(!flags.test(WeaponSetType::Elite));

        flags.clear(WeaponSetType::Veteran);
        assert!(!flags.test(WeaponSetType::Veteran));
    }

    #[test]
    fn test_weapon_template_set() {
        let mut template_set = WeaponTemplateSet::new();
        assert!(!template_set.has_any_weapons());

        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        template_set.set_weapon_template(WeaponSlotType::Primary, weapon_template);

        assert!(template_set.has_any_weapons());
        assert!(template_set
            .get_weapon_template(WeaponSlotType::Primary)
            .is_some());
        assert!(template_set
            .get_weapon_template(WeaponSlotType::Secondary)
            .is_none());
    }

    #[test]
    fn test_weapon_set_creation() {
        let weapon_set = WeaponSet::new();
        assert!(!weapon_set.has_any_weapon());
        assert!(!weapon_set.has_any_damage_weapon());
        assert!(weapon_set.is_out_of_ammo());
    }

    #[test]
    fn weapon_set_update_selects_default_unconditioned_set() {
        let primary = test_weapon_template("DefaultPrimary", 4);
        let default_set = test_template_set(primary, None, false);
        let mut weapon_set = WeaponSet::new();
        weapon_set.weapon_template_sets.push(default_set);

        weapon_set
            .update_weapon_set(77, &WeaponSetFlags::new())
            .expect("default weapon set");

        let weapon = weapon_set
            .get_current_weapon()
            .expect("current default weapon")
            .0;
        assert_eq!(weapon.get_name(), "DefaultPrimary");
        assert_eq!(weapon.get_status(), WeaponStatus::ReadyToFire);
    }

    #[test]
    fn test_weapon_template_set_conditions() {
        let mut template_set = WeaponTemplateSet::new();
        template_set.conditions.set(WeaponSetType::Veteran);

        let mut conditions = WeaponSetFlags::new();
        conditions.set(WeaponSetType::Veteran);
        conditions.set(WeaponSetType::Elite);

        // Template set requires Veteran, conditions have Veteran + Elite
        assert!(template_set.matches_conditions(&conditions));

        conditions.clear(WeaponSetType::Veteran);
        // Now conditions only have Elite, but template requires Veteran
        assert!(!template_set.matches_conditions(&conditions));
    }

    #[test]
    fn weapon_set_switch_shared_lock_keeps_lock_but_reloads_fresh_weapons() {
        let original_primary = test_weapon_template("OriginalPrimary", 8);
        let original_secondary = test_weapon_template("OriginalSecondary", 3);
        let replacement_primary = test_weapon_template("ReplacementPrimary", 5);
        let replacement_secondary = test_weapon_template("ReplacementSecondary", 7);

        let original_set = test_template_set(original_primary, Some(original_secondary), true);
        let replacement_set =
            test_template_set(replacement_primary, Some(replacement_secondary), true);

        let mut weapon_set = WeaponSet::new();
        weapon_set
            .switch_weapon_template_set(original_set, 100)
            .expect("initial weapon set");
        weapon_set.set_weapon_lock(WeaponSlotType::Secondary, WeaponLockType::LockedPermanently);

        {
            let primary = weapon_set
                .get_weapon_in_slot_mut(WeaponSlotType::Primary)
                .expect("primary weapon");
            primary.set_clip_percent_full(0.25, true);
            primary.set_status(WeaponStatus::BetweenFiringShots);
            primary.set_possible_next_shot_frame(900);
        }

        weapon_set
            .switch_weapon_template_set(replacement_set, 100)
            .expect("replacement weapon set");

        assert_eq!(
            weapon_set.get_current_weapon_slot(),
            WeaponSlotType::Secondary
        );
        assert_eq!(
            weapon_set.current_weapon_locked_status,
            WeaponLockType::LockedPermanently
        );

        let primary = weapon_set
            .get_weapon_in_slot(WeaponSlotType::Primary)
            .expect("replacement primary weapon");
        assert_eq!(primary.get_name(), "ReplacementPrimary");
        assert_eq!(primary.get_remaining_ammo(), 5);
        assert_eq!(primary.get_status(), WeaponStatus::ReadyToFire);
        assert_ne!(primary.get_possible_next_shot_frame(), 900);
    }

    #[test]
    fn weapon_set_switch_without_shared_lock_releases_lock_and_resets_primary() {
        let original_primary = test_weapon_template("OriginalPrimary", 8);
        let original_secondary = test_weapon_template("OriginalSecondary", 3);
        let replacement_primary = test_weapon_template("ReplacementPrimary", 5);

        let original_set = test_template_set(original_primary, Some(original_secondary), true);
        let replacement_set = test_template_set(replacement_primary, None, false);

        let mut weapon_set = WeaponSet::new();
        weapon_set
            .switch_weapon_template_set(original_set, 100)
            .expect("initial weapon set");
        weapon_set.set_weapon_lock(WeaponSlotType::Secondary, WeaponLockType::LockedPermanently);

        weapon_set
            .switch_weapon_template_set(replacement_set, 100)
            .expect("replacement weapon set");

        assert_eq!(
            weapon_set.get_current_weapon_slot(),
            WeaponSlotType::Primary
        );
        assert_eq!(
            weapon_set.current_weapon_locked_status,
            WeaponLockType::NotLocked
        );
        assert!(!weapon_set.is_current_weapon_locked());
    }
}
