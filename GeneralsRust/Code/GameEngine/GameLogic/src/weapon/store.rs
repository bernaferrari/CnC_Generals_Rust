//! Leftover WeaponStore (canonical crate::weapon::WeaponStore) extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{INVALID_ID, ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion};
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
    get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::helpers::{INVALID_OBJECT_ID, ObjectId};
use super::masks_enums::*;
use super::template::WeaponTemplate;
use super::weapon_instance::Weapon;
use game_engine::common::name_key_generator::NameKeyGenerator;

/// Weapon store managing all weapon templates
#[derive(Debug)]
pub struct WeaponStore {
    pub(crate) weapon_templates: HashMap<String, Arc<WeaponTemplate>>,
    pub(crate) weapon_templates_by_key: HashMap<u32, Arc<WeaponTemplate>>,
    pub(crate) delayed_damage_info: Vec<WeaponDelayedDamageInfo>,
}

/// Delayed damage information
#[derive(Debug)]
pub struct WeaponDelayedDamageInfo {
    pub(crate) delayed_weapon: Arc<WeaponTemplate>,
    pub(crate) delay_damage_pos: Coord3D,
    pub(crate) delay_damage_frame: u32,
    pub(crate) delay_source_id: ObjectId,
    pub(crate) delay_intended_victim_id: ObjectId,
    pub(crate) bonus: WeaponBonus,
}

/// Wave 77: save/load residual snapshot of a delayed-damage queue entry.
///
/// Mirrors the live `WeaponDelayedDamageInfo` identity fields so mid-flight
/// projectile delay can be bookkept consistently without Arc template Xfer.
/// Fail-closed: not full C++ WeaponStore::xfer (templates are not reloaded here).
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponDelayedDamageSnapshotResidual {
    pub weapon_name: String,
    pub delay_damage_pos: Coord3D,
    pub delay_damage_frame: u32,
    pub delay_source_id: ObjectId,
    pub delay_intended_victim_id: ObjectId,
}

impl WeaponDelayedDamageSnapshotResidual {
    pub fn from_info(info: &WeaponDelayedDamageInfo) -> Self {
        Self {
            weapon_name: info.delayed_weapon.name.clone(),
            delay_damage_pos: info.delay_damage_pos,
            delay_damage_frame: info.delay_damage_frame,
            delay_source_id: info.delay_source_id,
            delay_intended_victim_id: info.delay_intended_victim_id,
        }
    }

    pub fn honesty_ok(&self) -> bool {
        !self.weapon_name.is_empty()
            && self.delay_damage_pos.x.is_finite()
            && self.delay_damage_pos.y.is_finite()
            && self.delay_damage_pos.z.is_finite()
    }
}

/// Honesty: delayed-damage residual snapshot pack matches live queue (Wave 77).
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

impl WeaponStore {
    pub fn new() -> Self {
        Self {
            weapon_templates: HashMap::new(),
            weapon_templates_by_key: HashMap::new(),
            delayed_damage_info: Vec::new(),
        }
    }

    /// Initialize the weapon store
    pub fn init(&mut self) -> GameLogicResult<()> {
        // Initialization logic would go here
        Ok(())
    }

    /// Reset the weapon store
    pub fn reset(&mut self) -> GameLogicResult<()> {
        self.weapon_templates.clear();
        self.weapon_templates_by_key.clear();
        self.delayed_damage_info.clear();
        Ok(())
    }

    /// Update the weapon store (process delayed damage)
    pub fn update(&mut self) -> GameLogicResult<()> {
        let current_frame = TheGameLogic::get_frame();

        // Process delayed damage
        let mut i = 0;
        while i < self.delayed_damage_info.len() {
            if self.delayed_damage_info[i].delay_damage_frame <= current_frame {
                let damage_info = self.delayed_damage_info.remove(i);
                // Process the delayed damage here
                self.process_delayed_damage(damage_info)?;
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    /// Find weapon template by name.
    ///
    /// C++ WeaponStore::findWeaponTemplate treats the token `"None"` as missing
    /// (Weapon.cpp lookup). Case-insensitive so INI `NONE`/`none` match.
    pub fn find_weapon_template(&self, name: &str) -> Option<&Arc<WeaponTemplate>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        self.weapon_templates.get(name)
    }

    /// Name lookup that also matches INI case variants (C++ NameKey is case-insensitive).
    pub fn find_weapon_template_ci(&self, name: &str) -> Option<&Arc<WeaponTemplate>> {
        if let Some(found) = self.find_weapon_template(name) {
            return Some(found);
        }
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        self.weapon_templates
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, template)| template)
    }

    /// Find weapon template by name key
    pub fn find_weapon_template_by_name_key(&self, key: u32) -> Option<&Arc<WeaponTemplate>> {
        self.weapon_templates_by_key.get(&key)
    }

    /// Create a new weapon instance
    pub fn allocate_new_weapon(
        &self,
        template: &Arc<WeaponTemplate>,
        weapon_slot: WeaponSlotType,
    ) -> Weapon {
        Weapon::new(Arc::clone(template), weapon_slot)
    }

    /// Create and fire a temporary weapon
    pub fn create_and_fire_temp_weapon(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        target: Option<ObjectId>,
        position: Option<&Coord3D>,
    ) -> GameLogicResult<()> {
        let mut temp_weapon = self.allocate_new_weapon(template, WeaponSlotType::Primary);
        temp_weapon.load_ammo_now(source)?;

        match (target, position) {
            (Some(target_id), None) => {
                temp_weapon
                    .fire_weapon_at_object(source, target_id)
                    .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;
            }
            (None, Some(pos)) => {
                temp_weapon
                    .fire_weapon_at_position(source, pos)
                    .map_err(|err| GameLogicError::ModuleError(err.to_string()))?;
            }
            _ => {
                return Err(GameLogicError::Configuration(
                    "Invalid target specification".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Handle projectile detonation
    pub fn handle_projectile_detonation(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        position: &Coord3D,
        extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
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

        Ok(())
    }

    /// Add a new weapon template. Assigns a NameKey when the leftover key is 0.
    pub fn add_weapon_template(&mut self, mut template: WeaponTemplate) -> Arc<WeaponTemplate> {
        if template.name_key == 0 && !template.name.is_empty() {
            template.name_key = NameKeyGenerator::name_to_key(&template.name);
        }
        template.fill_historic_bonus_weapon_name();
        if template.historic_bonus_weapon.is_none()
            && !template.historic_bonus_weapon_name.is_empty()
        {
            if let Some(bonus) = self
                .find_weapon_template_ci(&template.historic_bonus_weapon_name)
                .cloned()
            {
                template.historic_bonus_weapon = Some(Arc::downgrade(&bonus));
            }
        }
        let name = template.name.clone();
        let name_key = template.name_key;
        let arc_template = Arc::new(template);

        self.weapon_templates
            .insert(name, Arc::clone(&arc_template));
        if name_key != 0 {
            self.weapon_templates_by_key
                .insert(name_key, Arc::clone(&arc_template));
        }

        arc_template
    }

    pub fn create_weapon_template(&mut self, name: String) -> Arc<WeaponTemplate> {
        let template = WeaponTemplate::new(name);
        self.add_weapon_template(template)
    }

    pub fn create_weapon_override(
        &mut self,
        base_template: &Arc<WeaponTemplate>,
        override_name: String,
    ) -> GameLogicResult<Arc<WeaponTemplate>> {
        let mut override_template = (**base_template).clone();
        override_template.name = override_name;
        override_template.name_key = 0;
        override_template.set_next_template((**base_template).clone());
        Ok(self.add_weapon_template(override_template))
    }

    pub fn create_and_fire_temp_weapon_at_pos(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        position: &Coord3D,
    ) -> GameLogicResult<()> {
        self.create_and_fire_temp_weapon(template, source, None, Some(position))
    }

    pub fn create_and_fire_temp_weapon_at_target(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        target: ObjectId,
    ) -> GameLogicResult<()> {
        self.create_and_fire_temp_weapon(template, source, Some(target), None)
    }

    pub fn handle_projectile_detonation_at_pos(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        position: &Coord3D,
        extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
        inflict_damage: bool,
    ) -> GameLogicResult<()> {
        self.handle_projectile_detonation(
            template,
            source,
            position,
            extra_bonus_flags,
            inflict_damage,
        )
    }

    pub fn handle_projectile_detonation_at_target(
        &self,
        template: &Arc<WeaponTemplate>,
        source: ObjectId,
        target: ObjectId,
        extra_bonus_flags: crate::common::types::WeaponBonusConditionFlags,
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
        Ok(())
    }

    pub fn get_template_count(&self) -> usize {
        self.weapon_templates.len()
    }

    pub fn get_delayed_damage_count(&self) -> usize {
        self.delayed_damage_info.len()
    }

    pub fn delayed_damage_snapshot_residual(&self) -> Vec<WeaponDelayedDamageSnapshotResidual> {
        self.delayed_damage_info
            .iter()
            .map(WeaponDelayedDamageSnapshotResidual::from_info)
            .collect()
    }

    pub fn get_template_names(&self) -> Vec<String> {
        self.weapon_templates.keys().cloned().collect()
    }

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
        Ok(())
    }

    /// Set delayed damage
    pub(crate) fn set_delayed_damage(
        &mut self,
        weapon: &Arc<WeaponTemplate>,
        pos: &Coord3D,
        which_frame: u32,
        source_id: ObjectId,
        victim_id: ObjectId,
        bonus: &WeaponBonus,
    ) {
        let damage_info = WeaponDelayedDamageInfo {
            delayed_weapon: Arc::clone(weapon),
            delay_damage_pos: *pos,
            delay_damage_frame: which_frame,
            delay_source_id: source_id,
            delay_intended_victim_id: victim_id,
            bonus: bonus.clone(),
        };

        self.delayed_damage_info.push(damage_info);
    }

    /// Set delayed damage when only a template reference is available.
    pub fn set_delayed_damage_from_template(
        &mut self,
        weapon: &WeaponTemplate,
        pos: &Coord3D,
        which_frame: u32,
        source_id: ObjectId,
        victim_id: ObjectId,
        bonus: &WeaponBonus,
    ) {
        let weapon = Arc::new(weapon.clone());
        self.set_delayed_damage(&weapon, pos, which_frame, source_id, victim_id, bonus);
    }

    /// Process delayed damage
    fn process_delayed_damage(&self, damage_info: WeaponDelayedDamageInfo) -> GameLogicResult<()> {
        let mut temp_weapon =
            self.allocate_new_weapon(&damage_info.delayed_weapon, WeaponSlotType::Primary);
        temp_weapon.load_ammo_now(damage_info.delay_source_id)?;

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
}

impl Default for WeaponStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global weapon store instance
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
    }

    Ok(())
}

/// Get reference to the global weapon store
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

/// Shutdown the leftover global weapon store.
pub fn shutdown_weapon_store() -> GameLogicResult<()> {
    let mut store = WEAPON_STORE.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire weapon store lock: {}", e))
    })?;
    *store = None;
    Ok(())
}
