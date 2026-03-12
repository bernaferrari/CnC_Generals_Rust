//! InstantDeathBehavior - Rust conversion of C++ InstantDeathBehavior class
//! 
//! A death behavior that instantly destroys an object and optionally triggers
//! effects, object creation lists, or weapons on death.
//! 
//! Author: Steven Johnson, Sep 2002 (C++ version)
//! Rust conversion: 2025

use std::sync::{Arc, Mutex};
use crate::common::{AsciiString, Real, Int, UnsignedInt};

// Forward declarations - assume these exist in other modules
pub trait FXList: Send + Sync {
    fn do_fx_obj(&self, object: &dyn Object, target: Option<&dyn Object>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub trait ObjectCreationList: Send + Sync {
    fn create(&self, source: &dyn Object, target: Option<&dyn Object>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub trait WeaponTemplate: Send + Sync {
    fn get_name(&self) -> &AsciiString;
}

pub trait WeaponStore: Send + Sync {
    fn create_and_fire_temp_weapon(
        &self,
        weapon_template: &dyn WeaponTemplate,
        source: &dyn Object,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub trait GameLogic: Send + Sync {
    fn destroy_object(&self, object: Arc<Mutex<dyn Object>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn game_logic_random_value(&self, min: Int, max: Int) -> Int;
}

pub trait AIUpdateInterface: Send + Sync {
    fn is_ai_in_dead_state(&self) -> bool;
    fn mark_as_dead(&mut self);
}

pub trait Object: Send + Sync {
    fn get_ai_update_interface(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>>;
    fn get_position(&self) -> &Coord3D;
    fn get_id(&self) -> ObjectID;
}

pub trait DieModule: Send + Sync {
    fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn is_die_applicable(&self, damage_info: Option<&DamageInfo>) -> bool;
}

// Type aliases
pub type ObjectID = u32;

// Mock types for dependencies
#[derive(Debug, Clone)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

#[derive(Debug, Clone)]
pub struct DamageInfo {
    // Simplified damage info structure
    pub amount: Real,
    pub source_id: ObjectID,
}

/// Configuration data for InstantDeathBehavior
#[derive(Debug)]
pub struct InstantDeathBehaviorModuleData {
    /// List of FX lists to randomly choose from on death
    pub fx_lists: Vec<Option<Arc<dyn FXList>>>,
    /// List of Object Creation Lists to randomly choose from on death
    pub ocl_lists: Vec<Option<Arc<dyn ObjectCreationList>>>,
    /// List of weapon templates to randomly choose from on death
    pub weapon_templates: Vec<Option<Arc<dyn WeaponTemplate>>>,
}

impl InstantDeathBehaviorModuleData {
    /// Creates a new InstantDeathBehaviorModuleData with empty lists
    pub fn new() -> Self {
        Self {
            fx_lists: Vec::new(),
            ocl_lists: Vec::new(),
            weapon_templates: Vec::new(),
        }
    }

    /// Adds an FX list to the collection
    pub fn add_fx_list(&mut self, fx_list: Option<Arc<dyn FXList>>) {
        self.fx_lists.push(fx_list);
    }

    /// Adds an Object Creation List to the collection
    pub fn add_ocl(&mut self, ocl: Option<Arc<dyn ObjectCreationList>>) {
        self.ocl_lists.push(ocl);
    }

    /// Adds a weapon template to the collection
    pub fn add_weapon_template(&mut self, weapon_template: Option<Arc<dyn WeaponTemplate>>) {
        self.weapon_templates.push(weapon_template);
    }
}

impl Default for InstantDeathBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// InstantDeathBehavior - Handles instant death with optional effects
/// 
/// This behavior module destroys the object immediately when triggered and can:
/// - Play random FX effects
/// - Create objects via Object Creation Lists
/// - Fire temporary weapons at the death location
/// 
/// All effects are chosen randomly from their respective collections.
#[derive(Debug)]
pub struct InstantDeathBehavior {
    /// Reference to the object this behavior is attached to
    object: Arc<Mutex<dyn Object>>,
    /// Configuration data for this behavior
    module_data: Arc<InstantDeathBehaviorModuleData>,
    /// Reference to the game logic singleton
    game_logic: Arc<dyn GameLogic>,
    /// Reference to the weapon store for temporary weapons
    weapon_store: Arc<dyn WeaponStore>,
}

impl InstantDeathBehavior {
    /// Creates a new InstantDeathBehavior
    pub fn new(
        object: Arc<Mutex<dyn Object>>,
        module_data: Arc<InstantDeathBehaviorModuleData>,
        game_logic: Arc<dyn GameLogic>,
        weapon_store: Arc<dyn WeaponStore>,
    ) -> Self {
        Self {
            object,
            module_data,
            game_logic,
            weapon_store,
        }
    }

    /// Gets the module data for this behavior
    pub fn get_instant_death_behavior_module_data(&self) -> &InstantDeathBehaviorModuleData {
        &self.module_data
    }

    /// Gets a reference to the object this behavior is attached to
    pub fn get_object(&self) -> Arc<Mutex<dyn Object>> {
        Arc::clone(&self.object)
    }

    /// Executes a random FX list if any are available
    fn execute_random_fx(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fx_lists = &self.module_data.fx_lists;
        if fx_lists.is_empty() {
            return Ok(());
        }

        let idx = self.game_logic.game_logic_random_value(0, fx_lists.len() as Int - 1) as usize;
        if let Some(Some(fx_list)) = fx_lists.get(idx) {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            fx_list.do_fx_obj(&**object, None)?;
        }

        Ok(())
    }

    /// Executes a random Object Creation List if any are available
    fn execute_random_ocl(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ocl_lists = &self.module_data.ocl_lists;
        if ocl_lists.is_empty() {
            return Ok(());
        }

        let idx = self.game_logic.game_logic_random_value(0, ocl_lists.len() as Int - 1) as usize;
        if let Some(Some(ocl)) = ocl_lists.get(idx) {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            ocl.create(&**object, None)?;
        }

        Ok(())
    }

    /// Fires a random weapon if any are available
    fn fire_random_weapon(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let weapon_templates = &self.module_data.weapon_templates;
        if weapon_templates.is_empty() {
            return Ok(());
        }

        let idx = self.game_logic.game_logic_random_value(0, weapon_templates.len() as Int - 1) as usize;
        if let Some(Some(weapon_template)) = weapon_templates.get(idx) {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            let position = object.get_position().clone();
            drop(object); // Release lock before calling weapon store
            
            self.weapon_store.create_and_fire_temp_weapon(
                &**weapon_template,
                &**self.object.lock().map_err(|_| "Failed to lock object")?,
                &position,
            )?;
        }

        Ok(())
    }
}

impl DieModule for InstantDeathBehavior {
    /// Called when the object dies - handles the instant death behavior
    fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_die_applicable(damage_info) {
            return Ok(());
        }

        // Check if AI is already dead to avoid duplicate processing
        {
            let object = self.object.lock().map_err(|_| "Failed to lock object")?;
            if let Some(ai) = object.get_ai_update_interface() {
                let mut ai_guard = ai.lock().map_err(|_| "Failed to lock AI")?;
                if ai_guard.is_ai_in_dead_state() {
                    return Ok(());
                }
                ai_guard.mark_as_dead();
            }
        }

        // Execute random effects
        if let Err(e) = self.execute_random_fx() {
            eprintln!("Failed to execute FX: {}", e);
        }

        if let Err(e) = self.execute_random_ocl() {
            eprintln!("Failed to execute OCL: {}", e);
        }

        if let Err(e) = self.fire_random_weapon() {
            eprintln!("Failed to fire weapon: {}", e);
        }

        // Destroy the object
        self.game_logic.destroy_object(Arc::clone(&self.object))?;

        Ok(())
    }

    /// Checks if death processing should be applied for the given damage
    fn is_die_applicable(&self, _damage_info: Option<&DamageInfo>) -> bool {
        // InstantDeathBehavior applies to all death scenarios by default
        // Override in subclasses if different logic is needed
        true
    }
}

// Thread safety implementations
unsafe impl Send for InstantDeathBehavior {}
unsafe impl Sync for InstantDeathBehavior {}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
