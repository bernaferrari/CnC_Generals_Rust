//! Weapon Store System
//!
//! This module provides global weapon template management functionality
//! matching the C++ implementation, including template storage, weapon
//! creation, temporary weapons, and delayed damage processing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::common::types::WeaponBonusConditionFlags;
use crate::common::{Coord3D, ObjectID};
use crate::weapon::{Weapon, WeaponBonus, WeaponSlotType, WeaponTemplate, INVALID_OBJECT_ID};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;

/// Delayed damage information for weapons with projectile flight time
#[derive(Debug, Clone)]
pub struct WeaponDelayedDamageInfo {
    /// The weapon template that will deal damage
    pub delayed_weapon: Arc<WeaponTemplate>,
    /// Position where damage will be dealt
    pub delay_damage_pos: Coord3D,
    /// Frame when damage should be dealt
    pub delay_damage_frame: u32,
    /// Object ID that caused the damage
    pub delay_source_id: ObjectID,
    /// Intended victim object ID (or INVALID_OBJECT_ID for area damage)
    pub delay_intended_victim_id: ObjectID,
    /// Weapon bonus to apply when dealing damage
    pub bonus: WeaponBonus,
}

impl WeaponDelayedDamageInfo {
    pub fn new(
        weapon: Arc<WeaponTemplate>,
        pos: Coord3D,
        frame: u32,
        source_id: ObjectID,
        victim_id: ObjectID,
        bonus: WeaponBonus,
    ) -> Self {
        Self {
            delayed_weapon: weapon,
            delay_damage_pos: pos,
            delay_damage_frame: frame,
            delay_source_id: source_id,
            delay_intended_victim_id: victim_id,
            bonus,
        }
    }
}

/// Wave 77: save/load residual snapshot of a delayed-damage queue entry.
///
/// Mirrors the live `WeaponDelayedDamageInfo` identity fields so mid-flight
/// projectile delay can be bookkept consistently without Arc template Xfer.
/// Fail-closed: not full C++ WeaponStore::xfer (templates are not reloaded here).
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponDelayedDamageSnapshotResidual {
    /// Weapon template name residual (rebind via store on load).
    pub weapon_name: String,
    /// World position residual where damage applies.
    pub delay_damage_pos: Coord3D,
    /// Absolute logic frame when damage should apply.
    pub delay_damage_frame: u32,
    /// Source object id residual.
    pub delay_source_id: ObjectID,
    /// Intended victim id residual (`INVALID_OBJECT_ID` for area).
    pub delay_intended_victim_id: ObjectID,
}

impl WeaponDelayedDamageSnapshotResidual {
    /// Build residual snapshot from a live delayed-damage entry.
    pub fn from_info(info: &WeaponDelayedDamageInfo) -> Self {
        Self {
            weapon_name: info.delayed_weapon.name.clone(),
            delay_damage_pos: info.delay_damage_pos,
            delay_damage_frame: info.delay_damage_frame,
            delay_source_id: info.delay_source_id,
            delay_intended_victim_id: info.delay_intended_victim_id,
        }
    }

    /// Honesty: residual fields are self-consistent (non-empty name, finite pos).
    pub fn honesty_ok(&self) -> bool {
        !self.weapon_name.is_empty()
            && self.delay_damage_pos.x.is_finite()
            && self.delay_damage_pos.y.is_finite()
            && self.delay_damage_pos.z.is_finite()
    }
}

/// Honesty: delayed-damage residual snapshot pack matches live queue (Wave 77).
///
/// Fail-closed: not full C++ WeaponStore Xfer / dealDamageInternal rebind.
pub fn honesty_weapon_store_delayed_damage_residual_ok(store: &WeaponStore) -> bool {
    let snaps = store.delayed_damage_snapshot_residual();
    if snaps.len() != store.get_delayed_damage_count() {
        return false;
    }
    snaps.iter().all(|s| s.honesty_ok())
        && store
            .delayed_damage_info
            .iter()
            .zip(snaps.iter())
            .all(|(info, snap)| {
                snap.weapon_name == info.delayed_weapon.name
                    && snap.delay_damage_frame == info.delay_damage_frame
                    && snap.delay_source_id == info.delay_source_id
                    && snap.delay_intended_victim_id == info.delay_intended_victim_id
                    && snap.delay_damage_pos == info.delay_damage_pos
            })
}

/// Global weapon store managing all weapon templates and delayed damage
#[derive(Debug)]
pub struct WeaponStore {
    /// Weapon templates by name
    weapon_templates: HashMap<String, Arc<WeaponTemplate>>,
    /// Weapon templates by name key (for fast lookup)
    weapon_templates_by_key: HashMap<u32, Arc<WeaponTemplate>>,
    /// Delayed damage information (for projectiles and timed weapons)
    delayed_damage_info: Vec<WeaponDelayedDamageInfo>,
}

impl WeaponStore {
    /// Create a new weapon store
    pub fn new() -> Self {
        Self {
            weapon_templates: HashMap::new(),
            weapon_templates_by_key: HashMap::new(),
            delayed_damage_info: Vec::new(),
        }
    }

    /// Initialize the weapon store
    pub fn init(&mut self) -> GameLogicResult<()> {
        log::info!("Initializing weapon store");

        // This would load weapon templates from configuration files
        // For now, we'll just ensure the store is ready

        Ok(())
    }

    /// Reset the weapon store (clear all data)
    pub fn reset(&mut self) -> GameLogicResult<()> {
        log::info!("Resetting weapon store");

        self.weapon_templates.clear();
        self.weapon_templates_by_key.clear();
        self.delayed_damage_info.clear();

        Ok(())
    }

    /// Update the weapon store (process delayed damage)
    pub fn update(&mut self) -> GameLogicResult<()> {
        let current_frame = self.get_current_frame();

        // Process delayed damage that's ready to execute
        let mut i = 0;
        while i < self.delayed_damage_info.len() {
            if self.delayed_damage_info[i].delay_damage_frame <= current_frame {
                let damage_info = self.delayed_damage_info.remove(i);
                self.process_delayed_damage(damage_info)?;
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    /// Post-process load (resolve template references)
    pub fn post_process_load(&mut self) -> GameLogicResult<()> {
        log::info!("Post-processing weapon templates");

        // Resolve all template references and validate data
        for template in self.weapon_templates.values() {
            // This would be done on a mutable reference in real implementation
            log::debug!("Post-processing template: {}", template.name);
        }

        Ok(())
    }

    // ===== TEMPLATE MANAGEMENT =====

    /// Find weapon template by name
    pub fn find_weapon_template(&self, name: &str) -> Option<&Arc<WeaponTemplate>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        self.weapon_templates.get(name)
    }

    /// Find weapon template by name key
    pub fn find_weapon_template_by_name_key(&self, key: u32) -> Option<&Arc<WeaponTemplate>> {
        self.weapon_templates_by_key.get(&key)
    }

    /// Add a weapon template to the store
    pub fn add_weapon_template(&mut self, mut template: WeaponTemplate) -> Arc<WeaponTemplate> {
        if template.name_key == 0 && !template.name.is_empty() {
            template.name_key = NameKeyGenerator::name_to_key(&template.name);
        }

        let name = template.name.clone();
        let name_key = template.name_key;

        let arc_template = Arc::new(template);

        // Store by name
        self.weapon_templates
            .insert(name, Arc::clone(&arc_template));

        // Store by name key if it exists
        if name_key != 0 {
            self.weapon_templates_by_key
                .insert(name_key, Arc::clone(&arc_template));
        }

        log::debug!(
            "Added weapon template: {} (key: {})",
            arc_template.name,
            name_key
        );

        arc_template
    }

    /// Create a new weapon template with given name
    pub fn create_weapon_template(&mut self, name: String) -> Arc<WeaponTemplate> {
        let template = WeaponTemplate::new(name);
        self.add_weapon_template(template)
    }

    /// Create a weapon template override
    pub fn create_weapon_override(
        &mut self,
        base_template: &Arc<WeaponTemplate>,
        override_name: String,
    ) -> GameLogicResult<Arc<WeaponTemplate>> {
        let mut override_template = (**base_template).clone();
        override_template.name = override_name;

        // Set up inheritance chain
        override_template.set_next_template((**base_template).clone());

        let override_arc = self.add_weapon_template(override_template);

        log::debug!(
            "Created weapon override: {} -> {}",
            override_arc.name,
            base_template.name
        );

        Ok(override_arc)
    }

    // ===== WEAPON INSTANCE MANAGEMENT =====

    /// Allocate a new weapon instance
    pub fn allocate_new_weapon(
        &self,
        template: &Arc<WeaponTemplate>,
        weapon_slot: WeaponSlotType,
    ) -> Weapon {
        Weapon::new(Arc::clone(template), weapon_slot)
    }

    /// Create and fire a temporary weapon at a position
    pub fn create_and_fire_temp_weapon_at_pos(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectID,
        position: &Coord3D,
    ) -> GameLogicResult<()> {
        let mut temp_weapon = self.allocate_new_weapon(template, WeaponSlotType::Primary);
        temp_weapon.load_ammo_now(source)?;

        temp_weapon
            .fire_weapon_at_position(source, position)
            .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;

        log::debug!(
            "Fired temporary weapon '{}' from {} at {:?}",
            template.name,
            source,
            position
        );

        Ok(())
    }

    /// Create and fire a temporary weapon at a target object
    pub fn create_and_fire_temp_weapon_at_target(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectID,
        target: ObjectID,
    ) -> GameLogicResult<()> {
        let mut temp_weapon = self.allocate_new_weapon(template, WeaponSlotType::Primary);
        temp_weapon.load_ammo_now(source)?;

        temp_weapon
            .fire_weapon_at_object(source, target)
            .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;

        log::debug!(
            "Fired temporary weapon '{}' from {} at target {}",
            template.name,
            source,
            target
        );

        Ok(())
    }

    // ===== PROJECTILE DETONATION HANDLING =====

    /// Handle projectile detonation with position
    pub fn handle_projectile_detonation_at_pos(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectID,
        position: &Coord3D,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> GameLogicResult<()> {
        let mut temp_weapon = self.allocate_new_weapon(template, WeaponSlotType::Primary);

        temp_weapon
            .fire_projectile_detonation_weapon(
                source,
                None,
                Some(position),
                extra_bonus_flags,
                inflict_damage,
            )
            .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;

        log::debug!(
            "Handled projectile detonation for '{}' at {:?} (damage: {})",
            template.name,
            position,
            inflict_damage
        );

        Ok(())
    }

    /// Handle projectile detonation with target
    pub fn handle_projectile_detonation_at_target(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectID,
        target: ObjectID,
        extra_bonus_flags: WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> GameLogicResult<()> {
        let mut temp_weapon = self.allocate_new_weapon(template, WeaponSlotType::Primary);

        temp_weapon
            .fire_projectile_detonation_weapon(
                source,
                Some(target),
                None,
                extra_bonus_flags,
                inflict_damage,
            )
            .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;

        log::debug!(
            "Handled projectile detonation for '{}' at target {} (damage: {})",
            template.name,
            target,
            inflict_damage
        );

        Ok(())
    }

    // ===== DELAYED DAMAGE SYSTEM =====

    /// Set delayed damage to be processed later
    pub fn set_delayed_damage(
        &mut self,
        weapon: &Arc<WeaponTemplate>,
        pos: &Coord3D,
        which_frame: u32,
        source_id: ObjectID,
        victim_id: ObjectID,
        bonus: &WeaponBonus,
    ) {
        let damage_info = WeaponDelayedDamageInfo::new(
            Arc::clone(weapon),
            *pos,
            which_frame,
            source_id,
            victim_id,
            bonus.clone(),
        );

        self.delayed_damage_info.push(damage_info);

        log::debug!(
            "Set delayed damage for '{}' at frame {} (source: {}, victim: {})",
            weapon.name,
            which_frame,
            source_id,
            victim_id
        );
    }

    /// Set delayed damage from a by-value template reference.
    ///
    /// This is useful in call sites where only `&WeaponTemplate` is available
    /// (for example during `WeaponTemplate::fire_weapon_template`), while the
    /// delayed queue still stores an `Arc<WeaponTemplate>`.
    pub fn set_delayed_damage_from_template(
        &mut self,
        weapon: &WeaponTemplate,
        pos: &Coord3D,
        which_frame: u32,
        source_id: ObjectID,
        victim_id: ObjectID,
        bonus: &WeaponBonus,
    ) {
        let weapon = Arc::new(weapon.clone());
        self.set_delayed_damage(&weapon, pos, which_frame, source_id, victim_id, bonus);
    }

    /// Process delayed damage (private implementation)
    fn process_delayed_damage(&self, damage_info: WeaponDelayedDamageInfo) -> GameLogicResult<()> {
        log::debug!(
            "Processing delayed damage for '{}' at {:?}",
            damage_info.delayed_weapon.name,
            damage_info.delay_damage_pos
        );

        // Create a temporary weapon to handle the delayed damage
        let mut temp_weapon =
            self.allocate_new_weapon(&damage_info.delayed_weapon, WeaponSlotType::Primary);
        temp_weapon.load_ammo_now(damage_info.delay_source_id)?;

        // Fire the weapon with the stored parameters
        if damage_info.delay_intended_victim_id != INVALID_OBJECT_ID {
            temp_weapon
                .fire_projectile_detonation_weapon_with_bonus(
                    damage_info.delay_source_id,
                    Some(damage_info.delay_intended_victim_id),
                    None,
                    &damage_info.bonus,
                    true,
                )
                .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;
        } else {
            temp_weapon
                .fire_projectile_detonation_weapon_with_bonus(
                    damage_info.delay_source_id,
                    None,
                    Some(&damage_info.delay_damage_pos),
                    &damage_info.bonus,
                    true,
                )
                .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;
        }

        Ok(())
    }

    /// Delete all delayed damage (used during reset)
    #[allow(dead_code)]
    fn delete_all_delayed_damage(&mut self) {
        self.delayed_damage_info.clear();
        log::debug!("Cleared all delayed damage entries");
    }

    // ===== UTILITY METHODS =====

    /// Get current game frame
    fn get_current_frame(&self) -> u32 {
        crate::helpers::TheGameLogic::get_frame()
    }

    /// Get number of weapon templates
    pub fn get_template_count(&self) -> usize {
        self.weapon_templates.len()
    }

    /// Get number of pending delayed damage events
    pub fn get_delayed_damage_count(&self) -> usize {
        self.delayed_damage_info.len()
    }

    /// Snapshot residual view of a pending delayed-damage entry (Wave 77).
    ///
    /// Freezes template name + frame + source/victim + position for save/load
    /// honesty without holding live Arc template across Xfer. Fail-closed: not
    /// full C++ WeaponStore Xfer table (C++ clears delayed damage on reset).
    pub fn delayed_damage_snapshot_residual(&self) -> Vec<WeaponDelayedDamageSnapshotResidual> {
        self.delayed_damage_info
            .iter()
            .map(WeaponDelayedDamageSnapshotResidual::from_info)
            .collect()
    }

    /// Get all weapon template names (for debugging/tools)
    pub fn get_template_names(&self) -> Vec<String> {
        self.weapon_templates.keys().cloned().collect()
    }

    /// Validate all weapon templates
    pub fn validate_templates(&self) -> GameLogicResult<()> {
        for (name, template) in &self.weapon_templates {
            if template.name != *name {
                return Err(GameLogicError::Configuration(format!(
                    "Template name mismatch: '{}' vs '{}'",
                    template.name, name
                )));
            }

            if template.attack_range < template.minimum_attack_range {
                return Err(GameLogicError::Configuration(format!(
                    "Template '{}': attack range ({}) < minimum range ({})",
                    template.name, template.attack_range, template.minimum_attack_range
                )));
            }

            if template.clip_size < 0 {
                return Err(GameLogicError::Configuration(format!(
                    "Template '{}': invalid clip size ({})",
                    template.name, template.clip_size
                )));
            }
        }

        log::info!("Validated {} weapon templates", self.weapon_templates.len());
        Ok(())
    }
}

impl Default for WeaponStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global weapon store instance (thread-safe)
static WEAPON_STORE: RwLock<Option<WeaponStore>> = RwLock::new(None);

/// Initialize the global weapon store
pub fn initialize_weapon_store() -> GameLogicResult<()> {
    let mut store = WEAPON_STORE.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire weapon store lock: {}", e))
    })?;

    if store.is_none() {
        let mut weapon_store = WeaponStore::new();
        weapon_store.init()?;
        *store = Some(weapon_store);
        log::info!("Initialized global weapon store");
    } else {
        log::warn!("Weapon store already initialized");
    }

    Ok(())
}

/// Get read-only reference to the global weapon store
pub fn with_weapon_store<F, R>(f: F) -> GameLogicResult<R>
where
    F: FnOnce(&WeaponStore) -> R,
{
    let store = WEAPON_STORE.read().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire weapon store lock: {}", e))
    })?;

    match store.as_ref() {
        Some(weapon_store) => Ok(f(weapon_store)),
        None => Err(GameLogicError::SystemNotInitialized(
            "Weapon store not initialized".to_string(),
        )),
    }
}

/// Get mutable reference to the global weapon store
pub fn with_weapon_store_mut<F, R>(f: F) -> GameLogicResult<R>
where
    F: FnOnce(&mut WeaponStore) -> R,
{
    let mut store = WEAPON_STORE.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire weapon store lock: {}", e))
    })?;

    match store.as_mut() {
        Some(weapon_store) => Ok(f(weapon_store)),
        None => Err(GameLogicError::SystemNotInitialized(
            "Weapon store not initialized".to_string(),
        )),
    }
}

/// Shutdown the global weapon store
pub fn shutdown_weapon_store() -> GameLogicResult<()> {
    let mut store = WEAPON_STORE.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire weapon store lock: {}", e))
    })?;

    if let Some(mut weapon_store) = store.take() {
        weapon_store.reset()?;
        log::info!("Shutdown global weapon store");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_store_creation() {
        let mut store = WeaponStore::new();
        store.init().unwrap();

        assert_eq!(store.get_template_count(), 0);
        assert_eq!(store.get_delayed_damage_count(), 0);
    }

    #[test]
    fn test_template_management() {
        let mut store = WeaponStore::new();

        let template = WeaponTemplate::new("TestWeapon".to_string());
        let arc_template = store.add_weapon_template(template);

        assert_eq!(store.get_template_count(), 1);

        let found = store.find_weapon_template("TestWeapon");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "TestWeapon");

        let not_found = store.find_weapon_template("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_none_weapon_template_name_is_missing() {
        let mut store = WeaponStore::new();
        store.add_weapon_template(WeaponTemplate::new("None".to_string()));

        assert!(store.find_weapon_template("None").is_none());
        assert!(store.find_weapon_template("none").is_none());
        assert!(store.find_weapon_template("NONE").is_none());
    }

    #[test]
    fn test_add_weapon_template_computes_name_key() {
        let mut store = WeaponStore::new();
        let expected_key = NameKeyGenerator::name_to_key("KeyedWeapon");

        let arc_template =
            store.add_weapon_template(WeaponTemplate::new("KeyedWeapon".to_string()));

        assert_eq!(arc_template.name_key, expected_key);
        let found = store.find_weapon_template_by_name_key(expected_key);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "KeyedWeapon");
    }

    #[test]
    fn test_add_weapon_template_preserves_explicit_name_key() {
        let mut store = WeaponStore::new();
        let mut template = WeaponTemplate::new("ExplicitKeyWeapon".to_string());
        template.name_key = 12345;

        let arc_template = store.add_weapon_template(template);

        assert_eq!(arc_template.name_key, 12345);
        let found = store.find_weapon_template_by_name_key(12345);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "ExplicitKeyWeapon");
    }

    #[test]
    fn test_weapon_allocation() {
        let mut store = WeaponStore::new();

        let template = WeaponTemplate::new("TestWeapon".to_string());
        let arc_template = store.add_weapon_template(template);

        let weapon = store.allocate_new_weapon(&arc_template, WeaponSlotType::Primary);

        assert_eq!(weapon.get_name(), "TestWeapon");
        assert_eq!(weapon.get_weapon_slot(), WeaponSlotType::Primary);
    }

    #[test]
    fn test_template_validation() {
        let mut store = WeaponStore::new();

        // Valid template. C++ uses ClipSize 0 as unlimited, so zero is valid too.
        let mut valid_template = WeaponTemplate::new("Valid".to_string());
        valid_template.attack_range = 100.0;
        valid_template.minimum_attack_range = 10.0;
        valid_template.clip_size = 0;
        store.add_weapon_template(valid_template);

        assert!(store.validate_templates().is_ok());

        // Invalid template (min range > max range)
        let mut invalid_template = WeaponTemplate::new("Invalid".to_string());
        invalid_template.attack_range = 50.0;
        invalid_template.minimum_attack_range = 100.0; // Invalid!
        store.add_weapon_template(invalid_template);

        assert!(store.validate_templates().is_err());
    }

    #[test]
    fn test_template_validation_rejects_negative_clip_size() {
        let mut store = WeaponStore::new();
        let mut template = WeaponTemplate::new("NegativeClip".to_string());
        template.clip_size = -1;
        store.add_weapon_template(template);

        assert!(store.validate_templates().is_err());
    }

    #[test]
    fn test_delayed_damage() {
        let mut store = WeaponStore::new();

        let template = WeaponTemplate::new("DelayedWeapon".to_string());
        let arc_template = store.add_weapon_template(template);

        let pos = Coord3D::new(100.0, 100.0, 0.0);
        let bonus = WeaponBonus::new();

        store.set_delayed_damage(&arc_template, &pos, 1000, 123, 456, &bonus);

        assert_eq!(store.get_delayed_damage_count(), 1);
    }

    #[test]
    fn test_delayed_damage_from_template_ref() {
        let mut store = WeaponStore::new();
        let template = WeaponTemplate::new("DelayedWeaponFromRef".to_string());

        let pos = Coord3D::new(42.0, 24.0, 0.0);
        let bonus = WeaponBonus::new();
        store.set_delayed_damage_from_template(&template, &pos, 77, 10, 20, &bonus);

        assert_eq!(store.get_delayed_damage_count(), 1);
        let queued = &store.delayed_damage_info[0];
        assert_eq!(queued.delayed_weapon.name, "DelayedWeaponFromRef");
        assert_eq!(queued.delay_damage_frame, 77);
        assert_eq!(queued.delay_source_id, 10);
        assert_eq!(queued.delay_intended_victim_id, 20);
        assert_eq!(queued.delay_damage_pos, pos);
    }

    /// Wave 77 residual: delayed-damage queue snapshot bookkeeping honesty.
    #[test]
    fn delayed_damage_snapshot_residual_wave77_honesty() {
        let mut store = WeaponStore::new();
        assert!(honesty_weapon_store_delayed_damage_residual_ok(&store));
        assert!(store.delayed_damage_snapshot_residual().is_empty());

        let template = WeaponTemplate::new("PatriotMissileWeapon".to_string());
        let arc = store.add_weapon_template(template);
        let pos = Coord3D::new(100.0, 50.0, 25.0);
        let bonus = WeaponBonus::new();
        store.set_delayed_damage(&arc, &pos, 900, 1, 2, &bonus);
        store.set_delayed_damage_from_template(
            &WeaponTemplate::new("RangerAdvancedCombatRifle".to_string()),
            &Coord3D::new(0.0, 0.0, 0.0),
            901,
            3,
            INVALID_OBJECT_ID,
            &bonus,
        );

        assert_eq!(store.get_delayed_damage_count(), 2);
        assert!(honesty_weapon_store_delayed_damage_residual_ok(&store));
        let snaps = store.delayed_damage_snapshot_residual();
        assert_eq!(snaps[0].weapon_name, "PatriotMissileWeapon");
        assert_eq!(snaps[0].delay_damage_frame, 900);
        assert_eq!(snaps[0].delay_source_id, 1);
        assert_eq!(snaps[0].delay_intended_victim_id, 2);
        assert_eq!(snaps[0].delay_damage_pos, pos);
        assert_eq!(snaps[1].weapon_name, "RangerAdvancedCombatRifle");
        assert_eq!(snaps[1].delay_intended_victim_id, INVALID_OBJECT_ID);
        assert!(snaps.iter().all(|s| s.honesty_ok()));
    }

    #[test]
    fn test_global_weapon_store() {
        // This test would be run in isolation to avoid conflicts
        initialize_weapon_store().unwrap();

        let result = with_weapon_store(|store| store.get_template_count());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        shutdown_weapon_store().unwrap();
    }
}
