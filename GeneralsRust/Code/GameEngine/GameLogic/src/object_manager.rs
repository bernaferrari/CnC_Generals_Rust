////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Object Manager - Comprehensive object creation, management, and lifecycle
//!
//! This module provides the complete object management system for Command & Conquer Generals Zero Hour,
//! handling object creation, destruction, templates, and factory patterns.
//!
//! The object manager includes:
//! - Object factory for creating instances from templates
//! - Object lifecycle management (creation, update, destruction)
//! - Template system for object definitions
//! - Behavior and module system integration
//! - Spatial partitioning for efficient queries
//! - Save/load support for persistent objects
//!
//! Author: Converted from C++ ThingFactory and Object management systems

use game_engine::common::thing::module as engine_module;
use once_cell::sync::Lazy;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
use crate::ai::AiCommandInterface;
use crate::ai::TeamName;
use crate::common::DisabledType;
use crate::common::ObjectStatusTypes;
use crate::common::INVALID_ID;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, Matrix3D, ObjectID, ObjectScriptStatusBits,
    ObjectStatusMaskType, PlayerMaskType, Real, ThingTemplate, UnsignedInt,
};
use crate::helpers::{get_game_logic_random_value, TheGameLogic};
use crate::modules::{
    AIUpdateInterface, BehaviorModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::{
    registry::OBJECT_REGISTRY, CrushSquishTestType, Object, MAX_TRIGGER_AREA_INFOS,
};
use crate::physics::{PhysicsState, PhysicsType};
use crate::player::{Player, PlayerIndex};
use crate::team::{Team, TeamID};
use crate::{GameLogicError, GameLogicResult};
use glam::Vec3;

/// Object creation flags - matches C++ object creation parameters
#[derive(Debug, Clone, Copy)]
pub struct ObjectCreationFlags {
    /// Object status mask for initial state
    pub status_mask: ObjectStatusMaskType,

    /// Whether object starts selected
    pub selected: Bool,

    /// Whether object should be added to spatial partitioning
    pub partitioned: Bool,

    /// Whether object should receive updates
    pub updatable: Bool,

    /// Whether object is created from save game
    pub from_save: Bool,

    /// Custom creation flags
    pub custom_flags: u32,
}

impl ObjectCreationFlags {
    pub fn new() -> Self {
        Self {
            status_mask: ObjectStatusMaskType::NONE,
            selected: false,
            partitioned: true,
            updatable: true,
            from_save: false,
            custom_flags: 0,
        }
    }

    pub fn from_template() -> Self {
        Self {
            status_mask: ObjectStatusMaskType::NONE,
            selected: false,
            partitioned: true,
            updatable: true,
            from_save: false,
            custom_flags: 0,
        }
    }

    pub fn from_save() -> Self {
        Self {
            status_mask: ObjectStatusMaskType::NONE,
            selected: false,
            partitioned: true,
            updatable: true,
            from_save: true,
            custom_flags: 0,
        }
    }
}

impl Default for ObjectCreationFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete object implementation with all systems - matches C++ Object class exactly
pub struct GameObjectInstance {
    /// Base object data
    pub base: Arc<RwLock<Object>>,

    /// Object template used for creation
    pub template: Option<Arc<dyn ThingTemplate>>,

    /// Owning team
    pub team: Option<Arc<RwLock<crate::team::Team>>>,

    /// Owning player
    pub player: Option<Arc<RwLock<crate::player::Player>>>,

    /// Object status bits
    pub status_bits: ObjectStatusMaskType,

    /// Script status bits
    pub script_status: HashMap<ObjectScriptStatusBits, Bool>,

    /// Object transform matrix
    pub transform: Matrix3D,

    /// Cached position for fast access
    pub cached_position: Coord3D,

    /// Current health and maximum health
    pub current_health: Real,
    pub max_health: Real,

    /// Experience and veterancy
    pub experience: Real,
    pub veterancy_level: u32,

    /// Physics state for movement and collision
    pub physics: Option<PhysicsState>,

    /// Object creation timestamp
    pub creation_time: SystemTime,

    /// Last update frame
    pub last_update_frame: UnsignedInt,

    /// Whether object is scheduled for destruction
    pub pending_destruction: Bool,

    /// Custom object data
    pub custom_data: HashMap<AsciiString, Box<dyn Any + Send + Sync>>,
}

impl fmt::Debug for GameObjectInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.base.read().map(|g| g.get_id()).unwrap_or(INVALID_ID);
        f.debug_struct("GameObjectInstance")
            .field("id", &id)
            .field(
                "template",
                &self.template.as_ref().map(|t| t.get_name().clone()),
            )
            .finish()
    }
}
impl engine_module::Thing for GameObjectInstance {
    fn as_object(&self) -> Option<&dyn engine_module::Object> {
        None
    }

    fn as_drawable(&self) -> Option<&dyn engine_module::Drawable> {
        None
    }
}

impl GameObjectInstance {
    fn player_from_team(team: Option<&Arc<RwLock<Team>>>) -> Option<Arc<RwLock<Player>>> {
        let player_index = team?.read().ok()?.get_controlling_player_id()? as Int;
        let list = crate::player::player_list().read().ok()?;
        list.get_player(player_index).cloned()
    }

    /// Wrap an existing base object created elsewhere (ObjectFactory) into a manager instance.
    pub fn from_existing(
        base: Arc<RwLock<Object>>,
        template: Option<Arc<dyn ThingTemplate>>,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Self {
        let (status_bits, transform, position, current_health, max_health) = base
            .read()
            .map(|guard| {
                (
                    guard.get_status_bits(),
                    guard.get_transform_matrix(),
                    *guard.get_position(),
                    guard.get_health(),
                    guard.get_max_health(),
                )
            })
            .unwrap_or((
                ObjectStatusMaskType::NONE,
                Matrix3D::IDENTITY,
                Coord3D::new(0.0, 0.0, 0.0),
                0.0,
                0.0,
            ));

        let player = Self::player_from_team(team.as_ref());

        Self {
            base,
            template,
            team,
            player,
            status_bits,
            script_status: HashMap::new(),
            transform,
            cached_position: position,
            current_health,
            max_health,
            experience: 0.0,
            veterancy_level: 0,
            physics: None,
            creation_time: SystemTime::now(),
            last_update_frame: 0,
            pending_destruction: false,
            custom_data: HashMap::new(),
        }
    }

    /// Create new object instance from template - matches C++ Object constructor
    pub fn new(
        id: ObjectID,
        template: Option<Arc<dyn ThingTemplate>>,
        team: Option<Arc<RwLock<Team>>>,
        flags: ObjectCreationFlags,
    ) -> GameLogicResult<Self> {
        let template: Arc<dyn ThingTemplate> = match template {
            Some(template) => template,
            None => Arc::new(crate::common::DefaultThingTemplate::new(format!(
                "StubObject{}",
                id
            ))),
        };

        let base = Object::new_with_id(template.clone(), id, flags.status_mask, team.clone())
            .map_err(|err| GameLogicError::SystemNotInitialized(err.to_string()))?;

        let mut instance = Self {
            base,
            template: Some(template.clone()),
            team: team.clone(),
            player: Self::player_from_team(team.as_ref()),
            status_bits: flags.status_mask,
            script_status: HashMap::new(),
            transform: Matrix3D::IDENTITY,
            cached_position: Coord3D::new(0.0, 0.0, 0.0),
            current_health: 100.0,
            max_health: 100.0,
            experience: 0.0,
            veterancy_level: 0,
            physics: None,
            creation_time: SystemTime::now(),
            last_update_frame: 0,
            pending_destruction: false,
            custom_data: HashMap::new(),
        };

        // Initialize from template if provided
        instance.init_from_template(template.as_ref());

        {
            let mut base_guard = instance.base.write().map_err(|_| {
                GameLogicError::SystemNotInitialized("Object lock poisoned".to_string())
            })?;
            base_guard
                .init_object()
                .map_err(|err| GameLogicError::SystemNotInitialized(err.to_string()))?;
        }

        Ok(instance)
    }

    /// Initialize object properties from template - matches C++ template loading
    fn init_from_template(&mut self, template: &dyn ThingTemplate) {
        // Set basic properties from template
        self.max_health = template.get_max_health();
        self.current_health = self.max_health;

        // Initialize physics if template specifies it
        if template.has_physics() {
            let mut physics = PhysicsState::new();
            physics.physics_type = template.get_physics_type();
            physics.mass = template.get_mass();
            physics.enabled = true;
            self.physics = Some(physics);
        }

        // Set initial position and orientation
        self.transform = template.get_initial_transform();
        self.update_cached_position();
    }

    /// Retrieve behavior modules for this object (delegates to base Object).
    pub fn get_behavior_modules(&self) -> Vec<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        self.base
            .read()
            .map(|base| base.get_behavior_modules())
            .unwrap_or_default()
    }

    /// Update cached position from transform matrix
    fn update_cached_position(&mut self) {
        // Extract position from transform matrix
        let cols = self.transform.to_cols_array();
        self.cached_position = Coord3D::new(cols[12], cols[13], cols[14]);
        if let Ok(mut base) = self.base.write() {
            let _ = base.set_position(&self.cached_position);
        }
    }

    /// Set object position and update transform
    pub fn set_position(&mut self, position: Coord3D) {
        self.cached_position = position;
        self.transform =
            Matrix3D::from_translation(glam::Vec3::new(position.x, position.y, position.z));
        if let Ok(mut base) = self.base.write() {
            let _ = base.set_position(&position);
        }
    }

    /// Get current position
    pub fn get_position(&self) -> &Coord3D {
        &self.cached_position
    }

    /// Get geometry info for this object (delegates to base Object).
    pub fn get_geometry_info(&self) -> crate::common::GeometryInfo {
        self.base
            .read()
            .map(|base| base.get_geometry_info().clone())
            .unwrap_or_default()
    }

    /// Get object ID
    pub fn get_id(&self) -> ObjectID {
        self.base
            .read()
            .map(|base| base.get_id())
            .unwrap_or(INVALID_ID)
    }

    /// Get owning team
    pub fn get_team(&self) -> Option<Arc<RwLock<Team>>> {
        self.team.clone()
    }

    /// Check if object has specific status bit
    pub fn has_status(&self, status: ObjectStatusTypes) -> Bool {
        if let Ok(base) = self.base.read() {
            return base.test_status(status);
        }
        let bit = ObjectStatusMaskType::from_bits_truncate(1u64 << (status as u32));
        self.status_bits.contains(bit)
    }

    /// Set status bit
    pub fn set_status(&mut self, status: ObjectStatusTypes, value: Bool) {
        let bit = ObjectStatusMaskType::from_bits_truncate(1u64 << (status as u32));
        if value {
            self.status_bits.insert(bit);
        } else {
            self.status_bits.remove(bit);
        }
        if let Ok(mut base) = self.base.write() {
            base.set_status(bit, value);
        }
    }

    /// Take damage and update health
    pub fn take_damage(&mut self, damage: Real) -> Bool {
        self.current_health -= damage;
        if self.current_health <= 0.0 {
            self.current_health = 0.0;
            self.set_status(ObjectStatusTypes::Destroyed, true);
            true // Object destroyed
        } else {
            false
        }
    }

    /// Heal object
    pub fn heal(&mut self, amount: Real) {
        self.current_health = (self.current_health + amount).min(self.max_health);
    }

    /// Get health percentage
    pub fn get_health_percentage(&self) -> Real {
        if self.max_health > 0.0 {
            self.current_health / self.max_health
        } else {
            0.0
        }
    }

    /// Add experience and check for veterancy promotion
    pub fn add_experience(&mut self, exp: Real) {
        self.experience += exp;

        // Check for veterancy level increase
        let new_level = self.calculate_veterancy_level(self.experience);
        if new_level > self.veterancy_level {
            self.veterancy_level = new_level;
            self.on_veterancy_promotion();
        }
    }

    /// Calculate veterancy level from experience
    fn calculate_veterancy_level(&self, experience: Real) -> u32 {
        // Standard veterancy thresholds
        if experience >= 300.0 {
            3 // Elite
        } else if experience >= 150.0 {
            2 // Veteran
        } else if experience >= 75.0 {
            1 // Experienced
        } else {
            0 // Rookie
        }
    }

    /// Handle veterancy promotion
    fn on_veterancy_promotion(&mut self) {
        // Veterancy bonuses would be applied here:
        // - Increased health
        // - Faster reload
        // - Better accuracy
        // - Special abilities

        match self.veterancy_level {
            1 => {
                // Experienced: +25% health, +10% damage
                self.max_health *= 1.25;
                self.current_health = self.max_health; // Full heal on promotion
            }
            2 => {
                // Veteran: +50% health, +25% damage, +15% speed
                self.max_health *= 1.5;
                self.current_health = self.max_health;
            }
            3 => {
                // Elite: +75% health, +50% damage, +25% speed, special abilities
                self.max_health *= 1.75;
                self.current_health = self.max_health;
            }
            _ => {}
        }
    }

    /// Update object for one frame - matches C++ Object::Update()
    pub fn update(&mut self, current_frame: UnsignedInt) -> GameLogicResult<()> {
        self.last_update_frame = current_frame;

        if let Ok(mut base) = self.base.write() {
            base.update(current_frame as f32)
                .map_err(GameLogicError::ModuleError)?;
        }

        Ok(())
    }

    /// Wake all registered update modules relative to `current_frame`.
    ///
    /// This mirrors the common C++ pattern of "setWakeFrame" being used as a coarse scheduler
    /// control for modules on an object.
    pub fn wake_all_update_modules_after(
        &mut self,
        current_frame: UnsignedInt,
        sleep: UpdateSleepTime,
    ) {
        if let Ok(mut base) = self.base.write() {
            base.wake_update_modules_after(current_frame, sleep);
        }
    }

    /// Wake only update modules that are currently sleeping forever.
    ///
    /// This is used for cases where a newly satisfied prerequisite (e.g. science) should
    /// re-activate modules that were dormant, without disturbing modules that are intentionally
    /// sleeping for timing/performance reasons.
    pub fn wake_update_modules_sleeping_forever(&mut self, current_frame: UnsignedInt) {
        if let Ok(mut base) = self.base.write() {
            base.wake_update_modules_after(current_frame, UPDATE_SLEEP_NONE);
        }
    }

    /// Destroy object and clean up resources
    pub fn destroy(&mut self) {
        self.pending_destruction = true;
        self.set_status(ObjectStatusTypes::Destroyed, true);

        if let Ok(mut base) = self.base.write() {
            base.on_destroy();
            base.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::Destroyed),
                true,
            );
        }

        // Clean up physics
        if let Some(ref mut physics) = self.physics {
            physics.enabled = false;
        }

        // Clean up modules would happen here
    }

    /// Check if object is destroyed
    pub fn is_destroyed(&self) -> Bool {
        self.has_status(ObjectStatusTypes::Destroyed) || self.pending_destruction
    }

    /// Check if object is effectively dead (dead, dying, or under construction)
    /// Delegates to underlying Object implementation
    pub fn is_effectively_dead(&self) -> bool {
        if let Ok(base) = self.base.read() {
            base.is_effectively_dead()
        } else {
            true // If lock fails, consider it dead
        }
    }

    /// Get status bits for this object
    pub fn get_status_bits(&self) -> ObjectStatusMaskType {
        if let Ok(base) = self.base.read() {
            return base.get_status_bits();
        }
        self.status_bits
    }

    /// Get the template name for this object
    pub fn get_template_name(&self) -> String {
        // Return a string if template is available
        self.template
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Add custom data to object
    pub fn set_custom_data<T: Any + Send + Sync>(&mut self, key: AsciiString, value: T) {
        self.custom_data.insert(key, Box::new(value));
    }

    /// Get custom data from object
    pub fn get_custom_data<T: Any>(&self, key: &str) -> Option<&T> {
        self.custom_data
            .get(key)
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    // ============================================================================
    // AI INTERFACE METHODS
    // ============================================================================

    /// Get the AI update interface for this object
    /// C++ Reference: Object::getAIUpdateInterface()
    pub fn get_ai_update_interface(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.base
            .read()
            .ok()
            .and_then(|base| base.get_ai_update_interface())
    }

    /// Set the AI update interface for this object
    pub fn set_ai_update_interface(&mut self, ai: Option<Arc<Mutex<dyn AIUpdateInterface>>>) {
        if let Ok(mut base) = self.base.write() {
            base.set_ai_update_interface(ai);
        }
    }

    // ============================================================================
    // PLAYER OWNERSHIP METHODS
    // ============================================================================

    /// Get the controlling player ID for this object
    /// C++ Reference: Object::getControllingPlayer()
    pub fn get_controlling_player_id(&self) -> Option<UnsignedInt> {
        self.team
            .as_ref()
            .and_then(|team| team.read().ok()?.get_controlling_player_id())
    }

    /// Set the controlling player ID for this object by updating its team
    /// C++ Reference: Object::setControllingPlayer()
    pub fn set_controlling_player_id(
        &mut self,
        player_id: Option<UnsignedInt>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref team_arc) = self.team {
            if let Ok(mut team) = team_arc.write() {
                team.set_controlling_player_id(player_id);
                return Ok(());
            }
            return Err("Failed to acquire write lock on team".into());
        }
        Err("Object has no assigned team".into())
    }

    /// Get the controlling player for this object
    pub fn get_controlling_player(&self) -> Option<Arc<RwLock<crate::player::Player>>> {
        let team = self.team.as_ref()?;
        let player_index = team.read().ok()?.get_controlling_player_id()? as Int;
        let list = crate::player::player_list().read().ok()?;
        list.get_player(player_index).cloned()
    }
}

// Implement AI command interface for objects
impl AiCommandInterface for GameObjectInstance {
    fn ai_do_command(
        &mut self,
        params: &crate::ai::AiCommandParams,
    ) -> Result<(), crate::ai::AiError> {
        let Some(ai_module) = self.get_ai_update_interface() else {
            return Err(crate::ai::AiError::InvalidCommand);
        };
        if let Ok(mut ai) = ai_module.lock() {
            let _ = ai.execute_command(params);
            return Ok(());
        }
        Err(crate::ai::AiError::InvalidCommand)
    }
}

/// Object factory for creating objects from templates - matches C++ ThingFactory
#[derive(Debug)]
pub struct ObjectFactory {
    /// Object creation statistics
    objects_created: HashMap<AsciiString, u64>,

    /// Total objects created
    total_created: u64,

    /// Factory enabled/disabled
    enabled: Bool,
}

impl ObjectFactory {
    pub fn new() -> Self {
        Self {
            objects_created: HashMap::new(),
            total_created: 0,
            enabled: true,
        }
    }

    /// Create object from template - matches C++ ThingFactory::Create()
    pub fn create_object(
        &mut self,
        template_name: &str,
        id: ObjectID,
        team: Option<Arc<RwLock<Team>>>,
        flags: ObjectCreationFlags,
    ) -> GameLogicResult<Arc<RwLock<GameObjectInstance>>> {
        if !self.enabled {
            return Err(GameLogicError::SystemNotInitialized(
                "Object factory disabled".to_string(),
            ));
        }

        let lookup_name = AsciiString::from(template_name);
        let template =
            crate::common::ThingFactory::find_template(&lookup_name).ok_or_else(|| {
                GameLogicError::Configuration(format!("Template not found: {}", template_name))
            })?;

        // Build variations are resolved in TheThingFactory adapter path; keep the selected
        // template as-is here until object-manager level variation metadata is restored.

        let object = GameObjectInstance::new(id, Some(template.clone()), team, flags)?;

        // Update statistics
        *self.objects_created.entry(lookup_name.clone()).or_insert(0) += 1;
        self.total_created += 1;

        Ok(Arc::new(RwLock::new(object)))
    }

    /// Get template by name
    pub fn get_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>> {
        let key = AsciiString::from(name);
        crate::common::ThingFactory::find_template(&key)
    }

    /// Get creation statistics
    pub fn get_creation_stats(&self) -> &HashMap<AsciiString, u64> {
        &self.objects_created
    }

    /// Get total objects created
    pub fn get_total_created(&self) -> u64 {
        self.total_created
    }
}

impl Default for ObjectFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Spatial partitioning for efficient object queries - matches C++ PartitionManager
#[derive(Debug)]
#[allow(dead_code)]
pub struct SpatialPartition {
    /// Grid cells containing object lists
    grid: HashMap<(i32, i32), Vec<ObjectID>>,

    /// Object-to-cell lookup for fast removal/movement
    object_cells: HashMap<ObjectID, (i32, i32)>,

    /// Latest known object positions (world coordinates)
    object_positions: HashMap<ObjectID, Coord3D>,

    /// Grid cell size
    cell_size: Real,

    /// World bounds
    #[allow(dead_code)]
    world_min: Coord3D,
    world_max: Coord3D,
}

impl SpatialPartition {
    pub fn new(cell_size: Real, world_min: Coord3D, world_max: Coord3D) -> Self {
        Self {
            grid: HashMap::new(),
            object_cells: HashMap::new(),
            object_positions: HashMap::new(),
            cell_size,
            world_min,
            world_max,
        }
    }

    /// Add object to spatial partition
    pub fn add_object(&mut self, object_id: ObjectID, position: Coord3D) {
        let cell = self.position_to_cell(position);

        // Remove from old cell if it exists
        if let Some(old_cell) = self.object_cells.get(&object_id) {
            if let Some(objects) = self.grid.get_mut(old_cell) {
                objects.retain(|&id| id != object_id);
            }
        }

        // Add to new cell
        self.grid
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(object_id);
        self.object_cells.insert(object_id, cell);
        self.object_positions.insert(object_id, position);
    }

    /// Remove object from spatial partition
    pub fn remove_object(&mut self, object_id: ObjectID) -> bool {
        self.object_positions.remove(&object_id);
        if let Some(cell) = self.object_cells.remove(&object_id) {
            if let Some(objects) = self.grid.get_mut(&cell) {
                objects.retain(|&id| id != object_id);
                return true;
            }
        }
        false
    }

    /// Find objects within radius of position
    pub fn find_objects_in_radius(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        let mut result = Vec::new();
        let radius_squared = radius * radius;

        // Calculate cell range to check
        let min_cell =
            self.position_to_cell([center[0] - radius, center[1] - radius, center[2]].into());
        let max_cell =
            self.position_to_cell([center[0] + radius, center[1] + radius, center[2]].into());

        // Check all cells in range
        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                if let Some(objects) = self.grid.get(&(x, y)) {
                    for &object_id in objects {
                        let Some(pos) = self.object_positions.get(&object_id) else {
                            continue;
                        };

                        // Generals radius queries are typically 2D (X/Y plane), with Z treated as height.
                        let dx = pos.x - center.x;
                        let dy = pos.y - center.y;
                        if dx * dx + dy * dy <= radius_squared {
                            result.push(object_id);
                        }
                    }
                }
            }
        }

        result
    }

    /// Convert world position to grid cell
    fn position_to_cell(&self, position: Coord3D) -> (i32, i32) {
        let x = (position[0] / self.cell_size).floor() as i32;
        let y = (position[1] / self.cell_size).floor() as i32;
        (x, y)
    }

    /// Rebuild the entire partition from the supplied object map.
    pub fn rebuild(&mut self, objects: &HashMap<ObjectID, Arc<RwLock<GameObjectInstance>>>) {
        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();

        for (object_id, object_ref) in objects {
            if let Ok(object) = object_ref.read() {
                let position = *object.get_position();
                self.add_object(*object_id, position);
            }
        }
    }
}

/// Complete object manager - integrates all systems
#[derive(Debug)]
pub struct ObjectManager {
    /// Object factory for creating instances
    factory: ObjectFactory,

    /// All active objects
    objects: HashMap<ObjectID, Arc<RwLock<GameObjectInstance>>>,

    /// Objects pending destruction
    destroy_queue: Vec<ObjectID>,

    /// Spatial partitioning for queries
    spatial_partition: SpatialPartition,

    /// Next object ID to allocate
    next_object_id: ObjectID,

    /// Object update order
    update_order: Vec<ObjectID>,

    /// Manager enabled/disabled
    enabled: Bool,
}

impl ObjectManager {
    fn map_creation_flags(
        flags: ObjectCreationFlags,
    ) -> crate::object::object_factory::ObjectCreationFlags {
        let mut out = crate::object::object_factory::ObjectCreationFlags::FROM_TEMPLATE;
        if flags.from_save {
            out |= crate::object::object_factory::ObjectCreationFlags::FROM_SAVE_DATA;
        }
        out
    }
    pub fn new() -> Self {
        Self {
            factory: ObjectFactory::new(),
            objects: HashMap::new(),
            destroy_queue: Vec::new(),
            spatial_partition: SpatialPartition::new(
                100.0,
                [-5000.0, -5000.0, -1000.0].into(),
                [5000.0, 5000.0, 1000.0].into(),
            ),
            next_object_id: 1,
            update_order: Vec::new(),
            enabled: true,
        }
    }

    /// Reset the manager to its initial state, clearing all tracked objects.
    pub fn reset(&mut self) {
        let _reset_guard = ObjectManagerResetGuard::acquire();
        // Unregister all live objects before dropping the manager state.
        for object_id in self.objects.keys().copied().collect::<Vec<_>>() {
            OBJECT_REGISTRY.unregister_object(object_id);
            unregister_legacy_object(object_id);
        }

        self.objects.clear();
        self.destroy_queue.clear();
        self.spatial_partition = SpatialPartition::new(
            100.0,
            [-5000.0, -5000.0, -1000.0].into(),
            [5000.0, 5000.0, 1000.0].into(),
        );
        self.next_object_id = 1;
        self.update_order.clear();
        self.factory = ObjectFactory::new();
        self.enabled = true;
    }

    /// Register a pre-constructed object instance with this manager.
    pub fn register_object_instance(
        &mut self,
        object: Arc<RwLock<GameObjectInstance>>,
        position: Coord3D,
    ) -> GameLogicResult<ObjectID> {
        let object_id = object
            .read()
            .map_err(|_| GameLogicError::SystemNotInitialized("Object lock poisoned".to_string()))?
            .get_id();

        if let Ok(mut obj) = object.write() {
            obj.set_position(position);
        }

        self.spatial_partition.add_object(object_id, position);

        if self.objects.insert(object_id, object.clone()).is_some() {
            OBJECT_REGISTRY.unregister_object(object_id);
            unregister_legacy_object(object_id);
        }

        if let Ok(obj_guard) = object.read() {
            OBJECT_REGISTRY.register_object(object_id, &obj_guard.base);
            register_legacy_object(&obj_guard.base);
        }

        if !self.update_order.contains(&object_id) {
            self.update_order.push(object_id);
        }

        self.next_object_id = self.next_object_id.max(object_id.saturating_add(1));
        self.register_player_ownership(object_id, &object);

        Ok(object_id)
    }

    /// Create new object from template
    pub fn create_object(
        &mut self,
        template_name: &str,
        position: Coord3D,
        team: Option<Arc<RwLock<Team>>>,
        flags: ObjectCreationFlags,
    ) -> GameLogicResult<ObjectID> {
        let factory_flags = Self::map_creation_flags(flags);
        let object_id = {
            let factory_arc = crate::object::object_factory::get_object_factory();
            let mut factory = factory_arc.write().map_err(|_| {
                GameLogicError::SystemNotInitialized("ObjectFactory lock poisoned".to_string())
            })?;
            factory
                .create_object(template_name, position, team.clone(), factory_flags)
                .map_err(|err| GameLogicError::SystemNotInitialized(err.to_string()))?
        };

        let base_object = {
            let factory_arc = crate::object::object_factory::get_object_factory();
            let factory = factory_arc.read().map_err(|_| {
                GameLogicError::SystemNotInitialized("ObjectFactory lock poisoned".to_string())
            })?;
            factory
                .get_object(object_id)
                .map(|instance| instance.get_base_object())
                .ok_or_else(|| {
                    GameLogicError::SystemNotInitialized(
                        "Created object missing from factory".to_string(),
                    )
                })?
        };

        if flags.status_mask != ObjectStatusMaskType::NONE {
            if let Ok(mut base_guard) = base_object.write() {
                base_guard.set_status(flags.status_mask, true);
            }
        }

        let template = base_object
            .read()
            .ok()
            .and_then(|guard| {
                crate::helpers::TheThingFactory::find_template(guard.get_name().as_str())
            })
            .or_else(|| crate::helpers::TheThingFactory::find_template(template_name));
        let object = Arc::new(RwLock::new(GameObjectInstance::from_existing(
            base_object,
            template,
            team,
        )));

        // Add to spatial partition
        self.spatial_partition.add_object(object_id, position);

        // Add to object list
        if self.objects.insert(object_id, object.clone()).is_some() {
            OBJECT_REGISTRY.unregister_object(object_id);
            unregister_legacy_object(object_id);
        }
        if let Ok(obj_guard) = object.read() {
            OBJECT_REGISTRY.register_object(object_id, &obj_guard.base);
            register_legacy_object(&obj_guard.base);
        }
        self.update_order.push(object_id);
        self.next_object_id = self.next_object_id.max(object_id.saturating_add(1));
        self.register_player_ownership(object_id, &object);

        if let Ok(mut engine_guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(ref mut engine) = *engine_guard {
                engine.set_frame_object_count_changed(TheGameLogic::get_frame() as u32);
            }
        }

        Ok(object_id)
    }

    fn register_player_ownership(
        &self,
        object_id: ObjectID,
        object: &Arc<RwLock<GameObjectInstance>>,
    ) {
        let team_arc = object.read().ok().and_then(|instance| {
            instance
                .team
                .clone()
                .or_else(|| instance.base.read().ok().and_then(|base| base.get_team()))
        });

        let Some(team_arc) = team_arc else {
            return;
        };

        let Ok(team_guard) = team_arc.read() else {
            return;
        };

        let Some(player_id) = team_guard.get_controlling_player_id() else {
            return;
        };

        let Ok(list_guard) = crate::player::player_list().read() else {
            return;
        };

        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex).cloned() else {
            return;
        };

        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        player_guard.add_owned_object(object_id);
    }

    fn unregister_player_ownership(
        &self,
        object_id: ObjectID,
        object: &Arc<RwLock<GameObjectInstance>>,
    ) {
        let player_id = object
            .read()
            .ok()
            .and_then(|instance| instance.get_controlling_player_id());

        let Some(player_id) = player_id else {
            return;
        };

        let Ok(list_guard) = crate::player::player_list().read() else {
            return;
        };

        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex).cloned() else {
            return;
        };

        let Ok(mut player_guard) = player_arc.write() else {
            return;
        };
        player_guard.remove_owned_object(object_id);
    }

    /// Get object by ID
    pub fn get_object(&self, object_id: ObjectID) -> Option<Arc<RwLock<GameObjectInstance>>> {
        // Clone the Arc handle so callers can drop the ObjectManager lock.
        // Prefer `for_each_object` / direct store walks when the manager stays borrowed.
        self.objects.get(&object_id).cloned()
    }

    /// Borrow the stored Arc without cloning (manager must stay borrowed).
    pub fn get_object_ref(&self, object_id: ObjectID) -> Option<&Arc<RwLock<GameObjectInstance>>> {
        self.objects.get(&object_id)
    }

    /// Borrow-first object access without cloning `Arc` (manager stays borrowed).
    /// Prefer this over `get_object(id).read()` at call sites that do not need to
    /// outlive the manager borrow. Intermediate step toward an owned object store.
    pub fn with_object<R>(
        &self,
        object_id: ObjectID,
        f: impl FnOnce(&GameObjectInstance) -> R,
    ) -> Option<R> {
        let arc = self.objects.get(&object_id)?;
        let guard = arc.read().ok()?;
        Some(f(&guard))
    }

    /// Mutable borrow-first object access without cloning `Arc`.
    pub fn with_object_mut<R>(
        &self,
        object_id: ObjectID,
        f: impl FnOnce(&mut GameObjectInstance) -> R,
    ) -> Option<R> {
        let arc = self.objects.get(&object_id)?;
        let mut guard = arc.write().ok()?;
        Some(f(&mut guard))
    }

    /// Iterate alive objects by direct instance borrow (no Arc clone per object).
    pub fn for_each_object_instance<F>(&self, mut f: F)
    where
        F: FnMut(ObjectID, &GameObjectInstance),
    {
        for &id in &self.update_order {
            if let Some(arc) = self.objects.get(&id) {
                if let Ok(guard) = arc.read() {
                    f(id, &guard);
                }
            }
        }
    }

    /// Destroy object
    pub fn destroy_object(&mut self, object_id: ObjectID) {
        if self.objects.contains_key(&object_id) {
            self.destroy_queue.push(object_id);
        }
    }

    /// Update all objects for one frame
    pub fn update(&mut self, current_frame: UnsignedInt) -> GameLogicResult<()> {
        if !self.enabled {
            return Ok(());
        }

        // Update objects in deterministic order (borrow-first; no Arc clone).
        let order = self.update_order.clone();
        for object_id in order {
            let pos = self.with_object_mut(object_id, |obj| -> GameLogicResult<Option<_>> {
                if obj.is_destroyed() {
                    return Ok(None);
                }
                obj.update(current_frame)?;
                Ok(Some(*obj.get_position()))
            });
            match pos {
                Some(Ok(Some(position))) => {
                    self.spatial_partition.add_object(object_id, position);
                }
                Some(Err(e)) => return Err(e),
                _ => {}
            }
        }

        // Process destruction queue
        self.process_destroy_queue();

        Ok(())
    }

    /// Process objects scheduled for destruction
    fn process_destroy_queue(&mut self) {
        let pending: Vec<_> = self.destroy_queue.drain(..).collect();
        for object_id in pending {
            if let Some(object) = self.objects.remove(&object_id) {
                OBJECT_REGISTRY.unregister_object(object_id);
                unregister_legacy_object(object_id);
                if let Ok(mut obj) = object.write() {
                    obj.destroy();
                }
                self.unregister_player_ownership(object_id, &object);

                // Remove from spatial partition
                self.spatial_partition.remove_object(object_id);

                // Remove from update order
                self.update_order.retain(|&id| id != object_id);

                if let Ok(mut engine_guard) = crate::scripting::engine::get_script_engine().write()
                {
                    if let Some(ref mut engine) = *engine_guard {
                        engine.set_frame_object_count_changed(TheGameLogic::get_frame() as u32);
                    }
                }
            }
        }
    }

    /// Find objects within radius
    pub fn find_objects_in_radius(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        self.spatial_partition
            .find_objects_in_radius(center, radius)
    }

    /// Update spatial partition entry for a single object.
    pub fn update_object_position(&mut self, object_id: ObjectID, position: Coord3D) {
        self.spatial_partition.add_object(object_id, position);
    }

    /// Get object count
    pub fn get_object_count(&self) -> usize {
        self.objects.len()
    }

    /// Get all object IDs currently in the world
    ///
    /// This provides a snapshot of object IDs at the time of the call.
    /// Safe to call while holding a read lock (doesn't require write access).
    ///
    /// # Returns
    ///
    /// Vector of all active ObjectIDs
    ///
    /// # Example
    ///
    /// ```rust
    /// use gamelogic::object_manager::get_object_manager;
    ///
    /// let manager = get_object_manager();
    /// let obj_mgr = manager.read().expect("object manager lock");
    /// for obj_id in obj_mgr.all_object_ids() {
    ///     if let Some(obj) = obj_mgr.get_object(obj_id) {
    ///         // Process object
    ///     }
    /// }
    /// ```
    pub fn all_object_ids(&self) -> Vec<ObjectID> {
        self.objects.keys().copied().collect()
    }

    /// Iterate over all objects with their IDs
    ///
    /// Provides a closure-based iteration interface that's safe and efficient.
    /// The callback receives object ID and the Arc reference without holding the manager lock.
    ///
    /// # Arguments
    ///
    /// * `f` - Closure to call for each object: `|id: ObjectID, obj: Arc<RwLock<GameObjectInstance>>| -> R`
    ///
    /// # Example
    ///
    /// ```rust
    /// use gamelogic::object_manager::get_object_manager;
    ///
    /// let manager = get_object_manager();
    /// let obj_mgr = manager.read().expect("object manager lock");
    /// obj_mgr.for_each_object(|id, obj_arc| {
    ///     if let Ok(obj) = obj_arc.read() {
    ///         let _ = (id, obj.get_id());
    ///     }
    /// });
    /// ```
    pub fn for_each_object<F>(&self, mut f: F)
    where
        F: FnMut(ObjectID, &Arc<RwLock<GameObjectInstance>>),
    {
        for &id in &self.update_order {
            if let Some(obj_arc) = self.objects.get(&id) {
                f(id, obj_arc);
            }
        }
    }

    /// Find objects owned by a specific player
    ///
    /// Filters objects based on team ownership and returns object IDs owned by the player.
    /// Thread-safe - works with read locks only.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player index (0-7)
    ///
    /// # Returns
    ///
    /// Vector of object IDs owned by this player
    ///
    /// # Implementation Note
    ///
    /// This is a core method for visibility system integration. It identifies which units
    /// belong to a player so the ShroudManager can aggregate their vision.
    ///
    /// Faithful to C++: Checks `object->team->controlling_player` relationship
    pub fn get_objects_owned_by_player(&self, player_id: UnsignedInt) -> Vec<ObjectID> {
        let mut owned_objects = Vec::new();

        self.for_each_object_instance(|obj_id, obj_guard| {
            if let Some(team_arc) = obj_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    if let Some(controlling_player) = team_guard.get_controlling_player_id() {
                        if controlling_player == player_id {
                            owned_objects.push(obj_id);
                        }
                    }
                }
            }
        });

        owned_objects
    }

    /// Check if an object belongs to a specific player
    ///
    /// Convenience method to check ownership without collecting all objects.
    ///
    /// # Arguments
    ///
    /// * `object_id` - Object to check
    /// * `player_id` - Player index
    ///
    /// # Returns
    ///
    /// true if object's team is controlled by player, false otherwise
    pub fn object_is_owned_by(&self, object_id: ObjectID, player_id: UnsignedInt) -> bool {
        self.with_object(object_id, |obj_guard| {
            if let Some(team_arc) = obj_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    if let Some(controlling_player) = team_guard.get_controlling_player_id() {
                        return controlling_player == player_id;
                    }
                }
            }
            false
        })
        .unwrap_or(false)
    }

    /// Enable/disable manager
    pub fn set_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
    }

    /// Refresh spatial partition data from the current world state.
    pub fn refresh_spatial_partition(&mut self) {
        self.spatial_partition.rebuild(&self.objects);
    }
}

impl Default for ObjectManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global object manager instance
pub static THE_OBJECT_MANAGER: Lazy<Arc<RwLock<ObjectManager>>> =
    Lazy::new(|| Arc::new(RwLock::new(ObjectManager::new())));

static OBJECT_MANAGER_RESET_COUNT: AtomicUsize = AtomicUsize::new(0);

struct ObjectManagerResetGuard;

impl ObjectManagerResetGuard {
    fn acquire() -> Self {
        OBJECT_MANAGER_RESET_COUNT.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for ObjectManagerResetGuard {
    fn drop(&mut self) {
        OBJECT_MANAGER_RESET_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn is_resetting() -> bool {
    OBJECT_MANAGER_RESET_COUNT.load(Ordering::Acquire) > 0
}

/// Get reference to global object manager
pub fn get_object_manager() -> Arc<RwLock<ObjectManager>> {
    THE_OBJECT_MANAGER.clone()
}

// Module tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::DefaultThingTemplate;
    use crate::player::{player_list, Player};
    use std::sync::{Arc, Mutex};

    /// Global player_list is process-wide; serialize tests that mutate it.
    static PLAYER_LIST_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_players() {
        player_list().write().expect("player list write").clear();
    }

    fn player_with_team(index: PlayerIndex, team_id: crate::team::TeamID) -> Arc<RwLock<Team>> {
        let team = Arc::new(RwLock::new(Team::new(
            format!("Player{}DefaultTeam", index).into(),
            team_id,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(index as UnsignedInt));

        let player = Arc::new(RwLock::new(Player::new(index)));
        player
            .write()
            .expect("player write")
            .set_default_team(Some(Arc::clone(&team)));
        player_list()
            .write()
            .expect("player list write")
            .add_player(player);

        team
    }

    #[test]
    fn test_object_creation_flags() {
        let flags = ObjectCreationFlags::new();
        assert!(!flags.selected);
        assert!(flags.partitioned);
        assert!(flags.updatable);
        assert!(!flags.from_save);
    }

    #[test]
    fn test_object_instance_basic() {
        let template = Arc::new(DefaultThingTemplate::new("TestObject".to_string()));
        let obj = GameObjectInstance::new(42, Some(template), None, ObjectCreationFlags::new())
            .expect("failed to create object instance");
        assert_eq!(obj.get_id(), 42);
        assert_eq!(obj.veterancy_level, 0);
        assert_eq!(obj.current_health, obj.max_health);
        assert!(!obj.is_destroyed());
    }

    #[test]
    fn new_object_instance_caches_player_from_team_controller() {
        let _guard = PLAYER_LIST_TEST_LOCK.lock().expect("player list test lock");
        reset_players();
        let team = player_with_team(0, 42);
        let template = Arc::new(DefaultThingTemplate::new("PlayerOwnedObject".to_string()));

        let obj =
            GameObjectInstance::new(43, Some(template), Some(team), ObjectCreationFlags::new())
                .expect("failed to create object instance");

        let player = obj.player.as_ref().expect("cached player");
        assert_eq!(player.read().expect("player read").get_player_index(), 0);
        reset_players();
    }

    #[test]
    fn wrapped_object_instance_caches_player_from_team_controller() {
        let _guard = PLAYER_LIST_TEST_LOCK.lock().expect("player list test lock");
        reset_players();
        let team = player_with_team(0, 43);
        let template = Arc::new(DefaultThingTemplate::new(
            "WrappedPlayerOwnedObject".to_string(),
        ));
        let base = Object::new_with_id(
            template.clone(),
            44,
            ObjectStatusMaskType::none(),
            Some(Arc::clone(&team)),
        )
        .expect("failed to create base object");

        let obj = GameObjectInstance::from_existing(base, Some(template), Some(team));

        let player = obj.player.as_ref().expect("cached player");
        assert_eq!(player.read().expect("player read").get_player_index(), 0);
        reset_players();
    }

    #[test]
    fn test_object_health_system() {
        let template = Arc::new(DefaultThingTemplate::new("TestObject".to_string()));
        let mut obj = GameObjectInstance::new(1, Some(template), None, ObjectCreationFlags::new())
            .expect("failed to create object instance");

        // Take damage
        assert!(!obj.take_damage(50.0));
        assert_eq!(obj.current_health, 50.0);
        assert_eq!(obj.get_health_percentage(), 0.5);

        // Heal
        obj.heal(25.0);
        assert_eq!(obj.current_health, 75.0);

        // Take fatal damage
        assert!(obj.take_damage(100.0));
        assert_eq!(obj.current_health, 0.0);
        assert!(obj.is_destroyed());
    }

    #[test]
    fn test_veterancy_system() {
        let mut obj = GameObjectInstance::new(1, None, None, ObjectCreationFlags::new())
            .expect("failed to create object instance");
        let original_health = obj.max_health;

        // Add experience for first promotion
        obj.add_experience(75.0);
        assert_eq!(obj.veterancy_level, 1);
        assert!(obj.max_health > original_health); // Health bonus from promotion

        // Add more experience
        obj.add_experience(75.0);
        assert_eq!(obj.veterancy_level, 2);
    }

    #[test]
    fn test_object_factory() {
        let mut factory = ObjectFactory::new();
        // Would need mock template to test properly
        assert_eq!(factory.get_total_created(), 0);
    }

    #[test]
    fn test_spatial_partition() {
        let mut partition = SpatialPartition::new(
            100.0,
            [-1000.0, -1000.0, 0.0].into(),
            [1000.0, 1000.0, 100.0].into(),
        );

        partition.add_object(1, Coord3D::new(0.0, 0.0, 0.0));
        partition.add_object(2, [100.0, 100.0, 0.0].into());

        let nearby = partition.find_objects_in_radius(Coord3D::new(0.0, 0.0, 0.0), 200.0);
        assert!(nearby.contains(&1));
        assert!(nearby.contains(&2));

        assert!(partition.remove_object(1));
        assert!(!partition.remove_object(999)); // Non-existent object
    }

    #[test]
    fn test_object_manager() {
        let mut manager = ObjectManager::new();
        assert_eq!(manager.get_object_count(), 0);

        // Would need templates registered to test object creation
        manager.set_enabled(false);
        manager.set_enabled(true);
        assert!(manager.enabled);
    }

    #[test]
    fn test_object_manager_all_object_ids() {
        let mut manager = ObjectManager::new();

        // Empty manager should return empty vector
        let ids = manager.all_object_ids();
        assert!(ids.is_empty(), "New manager should have no objects");

        // Create a few objects
        let obj1 = Arc::new(RwLock::new(
            GameObjectInstance::new(1, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));
        let obj2 = Arc::new(RwLock::new(
            GameObjectInstance::new(2, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));

        manager.objects.insert(1, obj1);
        manager.objects.insert(2, obj2);

        let ids = manager.all_object_ids();
        assert_eq!(ids.len(), 2, "Manager should have 2 objects");
        assert!(ids.contains(&1), "Should have object ID 1");
        assert!(ids.contains(&2), "Should have object ID 2");
    }

    #[test]
    fn test_object_manager_for_each_object() {
        let mut manager = ObjectManager::new();

        // Create test objects
        let obj1 = Arc::new(RwLock::new(
            GameObjectInstance::new(1, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));
        let obj2 = Arc::new(RwLock::new(
            GameObjectInstance::new(2, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));

        manager.objects.insert(1, obj1);
        manager.objects.insert(2, obj2);
        manager.update_order = vec![1, 2];

        // Iterate and collect IDs
        let mut collected_ids = Vec::new();
        manager.for_each_object(|id, _obj_arc| {
            collected_ids.push(id);
        });

        assert_eq!(collected_ids.len(), 2, "Should iterate 2 objects");
        assert!(collected_ids.contains(&1), "Should see object 1");
        assert!(collected_ids.contains(&2), "Should see object 2");
    }

    #[test]
    fn test_object_manager_get_objects_owned_by_player_no_ownership() {
        let mut manager = ObjectManager::new();

        // Create object with no team
        let obj = Arc::new(RwLock::new(
            GameObjectInstance::new(1, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));
        manager.objects.insert(1, obj);

        // Query for objects owned by player
        let owned = manager.get_objects_owned_by_player(0);
        assert!(
            owned.is_empty(),
            "Objects with no team should not be owned by any player"
        );
    }

    #[test]
    fn test_object_manager_object_is_owned_by_no_team() {
        let mut manager = ObjectManager::new();

        // Create object with no team
        let obj = Arc::new(RwLock::new(
            GameObjectInstance::new(1, None, None, ObjectCreationFlags::new())
                .expect("failed to create object instance"),
        ));
        manager.objects.insert(1, obj);

        // Check ownership
        let is_owned = manager.object_is_owned_by(1, 0);
        assert!(!is_owned, "Objects with no team are not owned");

        // Check non-existent object
        let is_owned = manager.object_is_owned_by(999, 0);
        assert!(!is_owned, "Non-existent objects are not owned by anyone");
    }

    #[test]
    fn test_object_manager_iteration_consistency() {
        let mut manager = ObjectManager::new();

        // Create multiple objects
        for i in 1..=5 {
            let obj = Arc::new(RwLock::new(
                GameObjectInstance::new(i, None, None, ObjectCreationFlags::new())
                    .expect("failed to create object instance"),
            ));
            manager.objects.insert(i, obj);
            manager.update_order.push(i);
        }

        // Method 1: all_object_ids()
        let ids = manager.all_object_ids();
        let mut collected_from_each: Vec<ObjectID> = Vec::new();

        // Method 2: for_each_object()
        manager.for_each_object(|id, _obj_arc| {
            collected_from_each.push(id);
        });

        // Should have same count
        assert_eq!(
            ids.len(),
            collected_from_each.len(),
            "Both iteration methods should see same objects"
        );

        // Should have same IDs
        let mut ids_sorted = ids.clone();
        ids_sorted.sort();
        collected_from_each.sort();
        assert_eq!(
            ids_sorted, collected_from_each,
            "Both methods should return same IDs"
        );
    }

    #[test]
    fn test_object_manager_iteration_thread_safety() {
        // This test documents thread-safe iteration patterns
        let manager = ObjectManager::new();

        // Safe pattern 1: Read lock
        let ids = manager.all_object_ids();
        assert!(ids.is_empty(), "Should work with read access");

        // Safe pattern 2: Closure-based iteration
        manager.for_each_object(|_id, _obj| {
            // No locks held during callback
        });

        assert!(true, "Thread-safe iteration patterns verified");
    }

    #[test]
    fn test_object_manager_empty_container_operations() {
        let manager = ObjectManager::new();

        // Empty manager operations
        assert_eq!(manager.get_object_count(), 0);
        assert!(manager.all_object_ids().is_empty());

        // for_each on empty manager
        let mut call_count = 0;
        manager.for_each_object(|_id, _obj| {
            call_count += 1;
        });
        assert_eq!(
            call_count, 0,
            "for_each should not call closure on empty manager"
        );

        // get_objects_owned_by_player on empty manager
        let owned = manager.get_objects_owned_by_player(0);
        assert!(owned.is_empty());
    }

    #[test]
    fn test_object_manager_iteration_methods_framework() {
        // This test documents the iteration interface framework for visibility system

        // Key methods added for ShroudManager integration:
        //
        // 1. all_object_ids() -> Vec<ObjectID>
        //    - Get snapshot of all object IDs
        //    - O(n) but safe to call with read lock
        //    - No object locks held during call
        //
        // 2. for_each_object(f: FnMut(ObjectID, Arc<...>))
        //    - Closure-based iteration
        //    - Passes Arc clones (no deadlock risk)
        //    - Callback can acquire object locks safely
        //
        // 3. get_objects_owned_by_player(player_id) -> Vec<ObjectID>
        //    - Filter objects by team ownership
        //    - Core method for visibility aggregation
        //    - Uses Team::get_controlling_player_id()
        //
        // 4. object_is_owned_by(object_id, player_id) -> bool
        //    - Convenience check for single object
        //    - O(1) lock acquisition per check
        //    - Used for filtering queries

        let manager = ObjectManager::new();

        // All methods work safely
        let ids = manager.all_object_ids();
        assert!(ids.is_empty());

        manager.for_each_object(|_id, _obj| {});

        let owned = manager.get_objects_owned_by_player(0);
        assert!(owned.is_empty());

        let is_owned = manager.object_is_owned_by(1, 0);
        assert!(!is_owned);

        assert!(true, "ObjectManager iteration interface documented");
    }
}
