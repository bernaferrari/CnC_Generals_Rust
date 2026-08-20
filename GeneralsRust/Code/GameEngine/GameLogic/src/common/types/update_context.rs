// UpdateContext and subsystem interface traits
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Missing types that are referenced in various modules
/// Drawable ID for referencing drawable objects
pub type DrawableID = u32;

/// Wide character type (UTF-16)
pub type WideChar = u16;

/// Unicode string type
pub type UnicodeString = std::string::String;

/// Kind of mask type for object classification (matches C++ `BitFlags<KINDOF_COUNT>`).
/// 116 retail bits (ALLOW_SURRENDER off) need more than 64 positions.
pub type KindOfMaskType = u128;

/// Alias without Type suffix (matches C++ usage)
pub type KindOfMask = KindOfMaskType;

/// Bitmask with all KindOf flags enabled.
pub const KIND_OF_MASK_ALL: KindOfMaskType = u128::MAX;
/// Bitmask with no KindOf flags enabled.
pub const KIND_OF_MASK_NONE: KindOfMaskType = 0;

// Additional missing types found during compilation

/// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        GameDifficulty::Medium
    }
}

/// Area type for geographical regions
#[derive(Debug, Clone)]
pub struct Area {
    pub name: String,
    pub boundary: Region3D,
    pub properties: HashMap<String, String>,
}

impl Area {
    pub fn new(name: String, boundary: Region3D) -> Self {
        Self {
            name,
            boundary,
            properties: HashMap::new(),
        }
    }
}

/// Terrain type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Sand,
    Rock,
    Water,
    Cliff,
    Beach,
    Road,
    // Add more as needed
}

impl Default for TerrainType {
    fn default() -> Self {
        TerrainType::Grass
    }
}

// DamageTypeFlags is defined in src/damage.rs and re-exported via common module
// Use crate::damage::DamageTypeFlags or crate::common::DamageTypeFlags
// Helper functions for coordinate types (replaces removed impl blocks)

/// Coordinate helper functions
pub mod coord_helpers {
    use super::*;

    pub fn coord3d_zero() -> Coord3D {
        Coord3D::origin()
    }

    pub fn coord2d_zero() -> Coord2D {
        Coord2D::origin()
    }

    pub fn icoord2d_zero() -> ICoord2D {
        ICoord2D::origin()
    }

    pub fn icoord2d_new(x: i32, y: i32) -> ICoord2D {
        ICoord2D::new(x, y)
    }

    pub fn icoord3d_zero() -> ICoord3D {
        ICoord3D::origin()
    }

    pub fn icoord3d_new(x: i32, y: i32, z: i32) -> ICoord3D {
        ICoord3D::new(x, y, z)
    }
}

/// Update context for object update modules
///
/// This context provides access to game subsystems needed by update modules.
/// It matches the pattern used in AIUpdateContext for AI modules.
///
/// # Fields
///
/// * `game_logic` - Reference to the GameLogic system for object queries and game state
/// * `terrain_logic` - Reference to terrain system for height queries and edge detection
/// * `object_creation_list` - System for creating new objects via OCLs
/// * `partition_manager` - Spatial partitioning for distance and proximity queries
/// * `particle_system_manager` - (Optional) Particle system management for visual effects
/// * `control_bar` - (Optional) Control bar interface for command buttons and command sets
/// * `thing_factory` - (Optional) Thing factory for creating objects from templates
/// * `upgrade_center` - (Optional) Upgrade center for managing upgrades
/// * `weapon_store` - (Optional) Weapon store for weapon template lookups
/// * `game_client` - (Optional) Game client interface for drawables and rendering
/// * `fx_list` - (Optional) FX list manager for special effects
/// * `audio` - (Optional) Audio interface for sound management
#[derive(Debug)]
pub struct UpdateContext<'a> {
    /// Reference to the main GameLogic system
    pub game_logic: &'a mut dyn GameLogicInterface,

    /// Reference to terrain system
    pub terrain_logic: &'a dyn TerrainLogicInterface,

    /// Reference to object creation list system
    pub object_creation_list: &'a mut dyn ObjectCreationListInterface,

    /// Reference to partition manager for spatial queries
    pub partition_manager: &'a dyn PartitionManagerInterface,

    /// Reference to particle system manager for visual effects (optional)
    pub particle_system_manager: Option<&'a dyn ParticleSystemManagerInterface>,

    /// Reference to control bar for command buttons (optional)
    pub control_bar: Option<&'a dyn ControlBarInterface>,

    /// Reference to thing factory for object creation (optional)
    pub thing_factory: Option<&'a dyn ThingFactoryInterface>,

    /// Reference to upgrade center for upgrade management (optional)
    pub upgrade_center: Option<&'a dyn UpgradeCenterInterface>,

    /// Reference to weapon store for weapon template lookups (optional)
    pub weapon_store: Option<&'a mut dyn WeaponStoreInterface>,

    /// Reference to game client for drawables and rendering (optional)
    pub game_client: Option<&'a dyn GameClientInterface>,

    /// Reference to FX list manager for special effects (optional)
    pub fx_list: Option<&'a dyn FXListManagerInterface>,

    /// Reference to object creation list manager for creating objects (optional)
    pub object_creation_list_manager: Option<&'a mut dyn ObjectCreationListInterface>,

    /// Reference to FX list manager for special effects (optional)
    pub fx_list_manager: Option<&'a dyn FXListManagerInterface>,

    /// Reference to audio system for sound management (optional)
    pub audio: Option<&'a mut dyn AudioInterface>,

    /// Reference to build assistant for construction management (optional)
    pub build_assistant: Option<&'a dyn BuildAssistantInterface>,
}

/// Trait for GameLogic interface used by UpdateContext
///
/// This allows update modules to access game logic functionality without
/// tight coupling to the concrete GameLogic implementation.
pub trait GameLogicInterface: std::fmt::Debug {
    /// Find an object by its ID
    fn find_object(&self, id: ThingId) -> Option<&Object>;

    /// Find a mutable object by its ID
    fn find_object_mut(&mut self, id: ThingId) -> Option<&mut Object>;

    /// Get the current game frame
    fn get_frame(&self) -> u32;

    /// Destroy an object
    fn destroy_object(&mut self, id: ThingId);
}

/// Trait for terrain logic interface used by UpdateContext
pub trait TerrainLogicInterface: std::fmt::Debug {
    /// Get ground height at a given position
    fn get_ground_height(&self, x: f32, y: f32) -> f32;

    /// Find closest edge point on the map
    fn find_closest_edge_point(&self, position: &Coord3D) -> Coord3D;
}

/// Trait for object creation list interface used by UpdateContext
pub trait ObjectCreationListInterface: std::fmt::Debug {
    /// Create objects from an OCL (Object Creation List)
    fn create(
        &mut self,
        ocl_id: ObjectCreationListId,
        source_object: Option<&Object>,
        position: &Coord3D,
        source_position: &Coord3D,
        orientation: f32,
    );
}

/// Trait for partition manager interface used by UpdateContext
pub trait PartitionManagerInterface: std::fmt::Debug {
    /// Get distance squared between two objects or points
    fn get_distance_squared(
        &self,
        a: &Object,
        b: &Object,
        distance_type: PartitionDistanceType,
    ) -> f32;

    /// Get distance squared between an object and a position
    fn get_distance_squared_to_pos(
        &self,
        obj: &Object,
        pos: &Coord3D,
        distance_type: PartitionDistanceType,
    ) -> f32;

    /// Get closest object matching filters
    fn get_closest_object(
        &self,
        from: &Object,
        max_range: f32,
        distance_type: PartitionDistanceType,
        filters: &[PartitionFilter],
    ) -> Option<Arc<RwLock<Object>>>;
}

/// Distance type for partition manager queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionDistanceType {
    /// Distance from center to center
    Center2D,
    /// Distance from bounding sphere edge to edge
    FromBoundingSphere2D,
    /// 3D distance
    Center3D,
}

/// Filter type for partition manager queries
/// Matches C++ PartitionFilter enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionFilter {
    /// Filter for flammable objects
    Flammable,
    /// Filter for enemy objects
    Enemy,
    /// Filter for friendly objects
    Friendly,
    /// Filter for neutral objects
    Neutral,
    /// Filter for targetable objects
    Targetable,
    /// Filter for attackable objects
    Attackable,
    /// Filter for objects that can heal
    CanHeal,
    /// Filter for objects that can repair
    CanRepair,
    /// Filter for objects with specific kindof
    KindOf(KindOf),
}

/// Constant for 3D center distance (uses PartitionDistanceType enum)
pub const PARTITION_FROM_CENTER_3D: PartitionDistanceType = PartitionDistanceType::Center3D;

/// Radius decal for visual effects
/// Matches C++ RadiusDecal class
#[derive(Debug, Clone)]
pub struct RadiusDecal {
    /// Position in world space
    pub position: Coord3D,
    /// Radius of the decal
    pub radius: f32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Color of the decal
    pub color: u32,
    /// Minimum opacity for throb effects
    pub min_opacity: f32,
    /// Maximum opacity for throb effects
    pub max_opacity: f32,
    /// Opacity throb time (frames)
    pub opacity_throb_time: u32,
    /// Template that created this decal
    pub template: Option<RadiusDecalTemplateId>,
}

impl RadiusDecal {
    /// Create a new radius decal
    pub fn new(position: Coord3D, radius: f32) -> Self {
        Self {
            position,
            radius,
            opacity: 1.0,
            color: 0xFFFFFFFF,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            template: None,
        }
    }

    /// Set decal opacity (0.0 to 1.0).
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    /// Set decal position.
    pub fn set_position(&mut self, position: Coord3D) {
        self.position = position;
    }

    /// Returns true if decal is effectively empty.
    pub fn is_empty(&self) -> bool {
        self.radius <= 0.0
    }

    /// Update throb opacity using current game frame.
    /// Matches C++ RadiusDecal::update behavior including draw-icon visibility gating.
    pub fn update(&mut self) {
        let draw_icon_ui = crate::helpers::TheGameLogic::get_draw_icon_ui();
        self.update_with_draw_icon_ui(draw_icon_ui);
    }

    /// Deterministic update helper that allows direct visibility control in tests/callers.
    pub fn update_with_draw_icon_ui(&mut self, draw_icon_ui: bool) {
        if !draw_icon_ui {
            self.opacity = 0.0;
            return;
        }

        if self.opacity_throb_time == 0 {
            self.opacity = self.max_opacity;
            return;
        }

        let now = crate::helpers::TheGameLogic::get_frame();
        let theta = (2.0 * std::f32::consts::PI)
            * ((now % self.opacity_throb_time) as f32 / self.opacity_throb_time as f32);
        let percent = 0.5 * (theta.sin() + 1.0);
        let lo = self.min_opacity.min(self.max_opacity);
        let hi = self.min_opacity.max(self.max_opacity);
        self.opacity = lo + percent * (hi - lo);
    }

    /// Reset the decal to an empty state (matches C++ RadiusDecal::clear).
    pub fn clear(&mut self) {
        self.position = Coord3D::origin();
        self.radius = 0.0;
        self.opacity = 1.0;
        self.color = 0xFFFFFFFF;
        self.min_opacity = 1.0;
        self.max_opacity = 1.0;
        self.opacity_throb_time = LOGICFRAMES_PER_SECOND;
        self.template = None;
    }
}

#[cfg(test)]
mod radius_decal_tests {
    use super::{Coord3D, CoordOrigin, RadiusDecal};

    #[test]
    fn radius_decal_update_hides_when_draw_icon_ui_disabled() {
        let mut decal = RadiusDecal::new(Coord3D::origin(), 10.0);
        decal.min_opacity = 0.2;
        decal.max_opacity = 0.9;
        decal.update_with_draw_icon_ui(false);
        assert_eq!(decal.opacity, 0.0);
    }

    #[test]
    fn radius_decal_update_uses_max_when_throb_time_is_zero() {
        let mut decal = RadiusDecal::new(Coord3D::origin(), 10.0);
        decal.min_opacity = 0.1;
        decal.max_opacity = 0.8;
        decal.opacity_throb_time = 0;
        decal.update_with_draw_icon_ui(true);
        assert!((decal.opacity - 0.8).abs() < f32::EPSILON);
    }
}

/// ID type for radius decal templates
pub type RadiusDecalTemplateId = u32;

// Shadow type bit flags (matches GameClient/Shadow.h TheShadowNames order)
pub const SHADOW_DECAL: u32 = 0x0000_0001;
pub const SHADOW_VOLUME: u32 = 0x0000_0002;
pub const SHADOW_PROJECTION: u32 = 0x0000_0004;
pub const SHADOW_DYNAMIC_PROJECTION: u32 = 0x0000_0008;
pub const SHADOW_DIRECTIONAL_PROJECTION: u32 = 0x0000_0010;
pub const SHADOW_ALPHA_DECAL: u32 = 0x0000_0020;
pub const SHADOW_ADDITIVE_DECAL: u32 = 0x0000_0040;

pub const SHADOW_NAMES: [&str; 7] = [
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

/// Template for radius decals
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplate {
    /// Default radius
    pub radius: f32,
    /// Default opacity
    pub opacity: f32,
    /// Default color
    pub color: u32,
    /// Texture name (if applicable)
    pub texture_name: AsciiString,
    /// Shadow/decal style flags (bitset, matches ShadowType)
    pub shadow_type: u32,
    /// Minimum opacity for throb effects
    pub min_opacity: f32,
    /// Maximum opacity for throb effects
    pub max_opacity: f32,
    /// Opacity throb time (frames)
    pub opacity_throb_time: u32,
    /// Visibility restricted to owning player
    pub only_visible_to_owning_player: bool,
}

impl Default for RadiusDecalTemplate {
    fn default() -> Self {
        Self {
            radius: 0.0,
            opacity: 1.0,
            color: 0,
            texture_name: AsciiString::new(),
            shadow_type: SHADOW_ALPHA_DECAL,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            only_visible_to_owning_player: true,
        }
    }
}

impl RadiusDecalTemplate {
    /// Create a radius decal from this template
    pub fn create_radius_decal(&self, position: Coord3D) -> RadiusDecal {
        if self.texture_name.is_empty() || self.radius <= 0.0 {
            return RadiusDecal::new(Coord3D::origin(), 0.0);
        }

        RadiusDecal {
            position,
            radius: self.radius,
            opacity: self.max_opacity,
            color: self.color,
            min_opacity: self.min_opacity,
            max_opacity: self.max_opacity,
            opacity_throb_time: self.opacity_throb_time,
            template: None,
        }
    }

    /// Create a radius decal using an explicit radius (matches C++ createRadiusDecal parameter).
    pub fn create_radius_decal_with_radius(&self, position: Coord3D, radius: f32) -> RadiusDecal {
        if self.texture_name.is_empty() || radius <= 0.0 {
            return RadiusDecal::new(Coord3D::origin(), 0.0);
        }

        RadiusDecal {
            position,
            radius,
            opacity: self.max_opacity,
            color: self.color,
            min_opacity: self.min_opacity,
            max_opacity: self.max_opacity,
            opacity_throb_time: self.opacity_throb_time,
            template: None,
        }
    }
}

/// Particle emission volume type (mirrors C++ EmissionVolumeType, subset used by gameplay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVolumeType {
    None,
    Sphere,
    Cylinder,
}

impl Default for EmissionVolumeType {
    fn default() -> Self {
        EmissionVolumeType::None
    }
}

/// Trait for particle system manager interface used by UpdateContext
pub trait ParticleSystemManagerInterface: std::fmt::Debug + Send + Sync {
    /// Find a particle system template by name
    fn find_template(&self, name: &str) -> Option<ParticleSystemTemplateId>;

    /// Create a particle system from a template
    fn create_particle_system(
        &self,
        template_id: ParticleSystemTemplateId,
    ) -> Option<ParticleSystemId>;

    /// Create an attached particle system and return its ID
    fn create_attached_particle_system_id(
        &self,
        template_id: ParticleSystemTemplateId,
        object_id: ObjectID,
    ) -> Option<ParticleSystemId>;

    /// Find a particle system by ID
    fn find_particle_system(&self, system_id: ParticleSystemId) -> Option<Box<dyn std::any::Any>>;

    /// Set particle system position (mirrors ParticleSystem::setPosition)
    fn set_particle_system_position(&self, system_id: ParticleSystemId, position: &Coord3D);

    /// Get particle system position (mirrors ParticleSystem::getPosition)
    fn get_particle_system_position(&self, system_id: ParticleSystemId) -> Option<Coord3D>;

    /// Attach particle system to an object (mirrors ParticleSystem::attachToObject)
    fn attach_particle_system_to_object(&self, system_id: ParticleSystemId, object_id: ObjectID);

    /// Attach particle system to a drawable (mirrors ParticleSystem::attachToDrawable)
    fn attach_particle_system_to_drawable(
        &self,
        system_id: ParticleSystemId,
        drawable_id: ObjectID,
    );

    /// Set particle system local transform (mirrors ParticleSystem::setLocalTransform)
    fn set_particle_system_transform(&self, system_id: ParticleSystemId, transform: &Matrix3D);

    /// Destroy a particle system by ID
    fn destroy_particle_system(&self, system_id: ParticleSystemId);

    /// Get emission volume type for a particle system
    fn get_particle_system_emission_volume_type(
        &self,
        system_id: ParticleSystemId,
    ) -> Option<EmissionVolumeType>;

    /// Set emission volume sphere radius for a particle system
    fn set_particle_system_emission_volume_sphere_radius(
        &self,
        system_id: ParticleSystemId,
        radius: Real,
    );

    /// Set emission volume cylinder radius for a particle system
    fn set_particle_system_emission_volume_cylinder_radius(
        &self,
        system_id: ParticleSystemId,
        radius: Real,
    );

    /// Start emitting particles from a system (mirrors ParticleSystem::start).
    fn start_particle_system(&self, _system_id: ParticleSystemId) {}

    /// Stop emitting new particles from a system (mirrors ParticleSystem::stop).
    fn stop_particle_system(&self, _system_id: ParticleSystemId) {}

    /// Mark whether the system should be saved (mirrors ParticleSystem::setSaveable).
    fn set_particle_system_saveable(&self, _system_id: ParticleSystemId, _saveable: bool) {}

    /// Rotate the particle system local transform around Z (mirrors ParticleSystem::rotateLocalTransformZ).
    fn rotate_particle_system_local_transform_z(&self, _system_id: ParticleSystemId, _angle: Real) {
    }

    /// Skip parent transform composition (mirrors ParticleSystem::setSkipParentXfrm).
    fn set_particle_system_skip_parent_xfrm(&self, _system_id: ParticleSystemId, _enable: bool) {}

    /// Tint all active particle-system colors (mirrors ParticleSystem::tintAllColors).
    fn tint_particle_system_all_colors(&self, _system_id: ParticleSystemId, _color: Color) {}

    /// Scale particle velocity on an active system (mirrors ParticleSystem::setVelocityMultiplier).
    fn set_particle_system_velocity_multiplier(
        &self,
        _system_id: ParticleSystemId,
        _multiplier: &Coord3D,
    ) {
    }

    /// Scale burst count on an active system (mirrors ParticleSystem::setBurstCountMultiplier).
    fn set_particle_system_burst_count_multiplier(
        &self,
        _system_id: ParticleSystemId,
        _multiplier: Real,
    ) {
    }

    /// Destroy all particle systems attached to the given object (mirrors ParticleSystemManager::destroyAttachedSystems).
    fn destroy_attached_systems(&self, _object_id: ObjectID) {}
}

/// Trait for control bar interface used by UpdateContext
pub trait ControlBarInterface: std::fmt::Debug + Send + Sync {
    /// Find a command set by name
    fn find_command_set(&self, name: &str) -> Option<&dyn std::any::Any>;

    /// Get a command button by ID
    fn get_command_button(
        &self,
        button_id: CommandButtonId,
    ) -> Option<&crate::command_button::CommandButton>;
}

/// Trait for thing factory interface used by UpdateContext
pub trait ThingFactoryInterface: std::fmt::Debug + Send + Sync {
    /// Find a template by name
    fn find_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>>;

    /// Get a template by ID
    fn get_template(&self, template_id: u32) -> Option<Arc<dyn ThingTemplate>>;

    /// Create a new object from a template
    fn new_object(
        &self,
        template: Arc<dyn ThingTemplate>,
        team: &dyn std::any::Any,
    ) -> Result<Arc<dyn std::any::Any>, String>;
}

/// Trait for upgrade center interface used by UpdateContext
pub trait UpgradeCenterInterface: std::fmt::Debug + Send + Sync {
    /// Check if a player can afford an upgrade
    fn can_afford_upgrade(&self, player: &dyn std::any::Any, upgrade: &dyn std::any::Any) -> bool;

    /// Find an upgrade by ID
    fn find_upgrade(&self, upgrade_id: u32) -> Option<&dyn std::any::Any>;
}

/// Trait for weapon store interface used by UpdateContext
pub trait WeaponStoreInterface: std::fmt::Debug + Send + Sync {
    /// Find a weapon template by name
    fn find_weapon_template(&self, name: &str) -> Option<&dyn std::any::Any>;

    /// Get a weapon template by ID
    fn get_weapon_template(&self, template_id: WeaponTemplateId) -> Option<&dyn std::any::Any>;

    /// Allocate a new weapon instance from a template
    fn allocate_new_weapon(
        &mut self,
        template_id: WeaponTemplateId,
        slot_type: WeaponSlotType,
    ) -> WeaponId {
        // Default implementation returns invalid ID
        let _ = (template_id, slot_type);
        0
    }

    /// Get a weapon by ID (immutable)
    fn get_weapon(&self, weapon_id: WeaponId) -> Option<&crate::weapon::Weapon> {
        // Default implementation returns None
        let _ = weapon_id;
        None
    }

    /// Get a mutable weapon by ID
    fn get_weapon_mut(&mut self, weapon_id: WeaponId) -> Option<&mut crate::weapon::Weapon> {
        // Default implementation returns None
        let _ = weapon_id;
        None
    }
}

/// Trait for game client interface used by UpdateContext
/// Provides access to client-side rendering and drawable systems
pub trait GameClientInterface: std::fmt::Debug + Send + Sync {
    /// Find a drawable by its ID
    fn find_drawable_by_id(&self, id: DrawableId) -> Option<&dyn std::any::Any>;
}

/// Trait for FX list manager interface used by UpdateContext
/// Manages special effects execution
pub trait FXListManagerInterface: std::fmt::Debug + Send + Sync {
    /// Execute FX at a position
    fn do_fx_pos(&self, fx_list: FXListId, position: &Coord3D, matrix: Option<&Mat4>);

    /// C++ `FXList::doFXPos` with bone matrix, weapon speed, victim, radius.
    fn do_fx_pos_ex(
        &self,
        fx_list: FXListId,
        position: &Coord3D,
        matrix: Option<&Mat4>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        self.do_fx_pos(fx_list, position, matrix);
    }

    /// Execute FX on an object
    fn do_fx_obj(&self, fx_list: FXListId, object_id: ThingId);

    /// Execute FX on an object with an optional source object for orientation.
    fn do_fx_obj_with_source(
        &self,
        fx_list: FXListId,
        object_id: ThingId,
        _source_id: Option<ThingId>,
    ) {
        self.do_fx_obj(fx_list, object_id);
    }
}

/// Trait for audio interface used by UpdateContext
/// Manages game audio events
pub trait AudioInterface: std::fmt::Debug + Send + Sync {
    /// Add an audio event and return its handle
    fn add_audio_event(&mut self, event: &dyn std::any::Any) -> u32;

    /// Remove an audio event by handle
    fn remove_audio_event(&mut self, handle: u32);
}

/// Build assistant interface for construction validation
pub trait BuildAssistantInterface: std::fmt::Debug + Send + Sync {
    /// Check if a unit can be made (including prerequisites and money check)
    fn can_make_unit(
        &self,
        builder: &dyn std::any::Any,
        what_to_build: &dyn ThingTemplate,
    ) -> crate::object::update::production_update::CanMakeType;
}

impl<'a> UpdateContext<'a> {
    /// Create a new update context with only the required core interfaces.
    ///
    /// Optional interfaces (particle_system_manager, control_bar, thing_factory,
    /// upgrade_center, weapon_store, game_client, fx_list, object_creation_list_manager,
    /// fx_list_manager, audio) are set to None by default.
    /// Use the builder methods (with_*) to add them as needed.
    pub fn new(
        game_logic: &'a mut dyn GameLogicInterface,
        terrain_logic: &'a dyn TerrainLogicInterface,
        object_creation_list: &'a mut dyn ObjectCreationListInterface,
        partition_manager: &'a dyn PartitionManagerInterface,
    ) -> Self {
        Self {
            game_logic,
            terrain_logic,
            object_creation_list,
            partition_manager,
            particle_system_manager: None,
            control_bar: None,
            thing_factory: None,
            upgrade_center: None,
            weapon_store: None,
            game_client: None,
            fx_list: None,
            object_creation_list_manager: None,
            fx_list_manager: None,
            audio: None,
            build_assistant: None,
        }
    }

    /// Add particle system manager to the context (builder pattern)
    pub fn with_particle_system_manager(
        mut self,
        particle_system_manager: &'a dyn ParticleSystemManagerInterface,
    ) -> Self {
        self.particle_system_manager = Some(particle_system_manager);
        self
    }

    /// Add control bar to the context (builder pattern)
    pub fn with_control_bar(mut self, control_bar: &'a dyn ControlBarInterface) -> Self {
        self.control_bar = Some(control_bar);
        self
    }

    /// Add thing factory to the context (builder pattern)
    pub fn with_thing_factory(mut self, thing_factory: &'a dyn ThingFactoryInterface) -> Self {
        self.thing_factory = Some(thing_factory);
        self
    }

    /// Add upgrade center to the context (builder pattern)
    pub fn with_upgrade_center(mut self, upgrade_center: &'a dyn UpgradeCenterInterface) -> Self {
        self.upgrade_center = Some(upgrade_center);
        self
    }

    /// Add weapon store to the context (builder pattern)
    pub fn with_weapon_store(mut self, weapon_store: &'a mut dyn WeaponStoreInterface) -> Self {
        self.weapon_store = Some(weapon_store);
        self
    }

    /// Add game client to the context (builder pattern)
    pub fn with_game_client(mut self, game_client: &'a dyn GameClientInterface) -> Self {
        self.game_client = Some(game_client);
        self
    }

    /// Add FX list manager to the context (builder pattern)
    pub fn with_fx_list(mut self, fx_list: &'a dyn FXListManagerInterface) -> Self {
        self.fx_list = Some(fx_list);
        self
    }

    /// Add object creation list manager to the context (builder pattern)
    pub fn with_object_creation_list_manager(
        mut self,
        object_creation_list_manager: &'a mut dyn ObjectCreationListInterface,
    ) -> Self {
        self.object_creation_list_manager = Some(object_creation_list_manager);
        self
    }

    /// Add FX list manager to the context (builder pattern)
    pub fn with_fx_list_manager(mut self, fx_list_manager: &'a dyn FXListManagerInterface) -> Self {
        self.fx_list_manager = Some(fx_list_manager);
        self
    }

    /// Add audio system to the context (builder pattern)
    pub fn with_audio(mut self, audio: &'a mut dyn AudioInterface) -> Self {
        self.audio = Some(audio);
        self
    }

    /// Get particle system manager if available
    pub fn particle_system_manager(&self) -> Option<&dyn ParticleSystemManagerInterface> {
        self.particle_system_manager
    }

    /// Get control bar if available
    pub fn control_bar(&self) -> Option<&dyn ControlBarInterface> {
        self.control_bar
    }

    /// Get thing factory if available
    pub fn thing_factory(&self) -> Option<&dyn ThingFactoryInterface> {
        self.thing_factory
    }

    /// Get upgrade center if available
    pub fn upgrade_center(&self) -> Option<&dyn UpgradeCenterInterface> {
        self.upgrade_center
    }

    /// Get weapon store if available
    pub fn weapon_store(&self) -> Option<&dyn WeaponStoreInterface> {
        self.weapon_store
            .as_ref()
            .map(|ws| *ws as &dyn WeaponStoreInterface)
    }

    /// Get the current game frame number
    pub fn get_frame(&self) -> u32 {
        self.game_logic.get_frame()
    }

    /// Set the wake frame for an update module
    /// This schedules when the module should next be updated
    ///
    /// # Arguments
    /// * `object_id` - The object ID or thing ID to set wake frame for
    /// * `sleep_time` - When the module should wake up next
    pub fn set_wake_frame(
        &mut self,
        object_id: impl Into<ThingId>,
        sleep_time: crate::object::helper::UpdateSleepTime,
    ) {
        crate::helpers::TheGameLogic::set_wake_frame(object_id.into(), sleep_time);
    }
}

