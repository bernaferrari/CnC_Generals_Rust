//! Leftover WeaponStore (canonical crate::weapon::WeaponStore) extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::Relationship;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::common::{ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion, INVALID_ID};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    get_game_logic_random_value, get_game_logic_random_value_real, TheGameLogic, TheTerrainLogic,
    TheThingFactory,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    module_projectile_launch_kind, ProjectileLaunchKindMut,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::helpers::{ObjectId, INVALID_OBJECT_ID};
use super::masks_enums::*;
use super::template::WeaponTemplate;
use super::weapon_instance::Weapon;

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

    /// Find weapon template by name
    pub fn find_weapon_template(&self, name: &str) -> Option<&Arc<WeaponTemplate>> {
        self.weapon_templates.get(name)
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

    /// Add a new weapon template
    pub fn add_weapon_template(&mut self, template: WeaponTemplate) -> Arc<WeaponTemplate> {
        let arc_template = Arc::new(template);
        let name = arc_template.name.clone();
        let name_key = arc_template.name_key;

        self.weapon_templates
            .insert(name, Arc::clone(&arc_template));
        if name_key != 0 {
            self.weapon_templates_by_key
                .insert(name_key, Arc::clone(&arc_template));
        }

        arc_template
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
    #[allow(dead_code)]
    pub(crate) fn set_delayed_damage_from_template(
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

