//! Base DrawModule trait and interfaces
//!
//! Port of C++ DrawModule.h
//! Reference: /GeneralsMD/Code/GameEngine/Include/Common/DrawModule.h

use crate::common::*;
use crate::effects::FXList;
use game_engine::common::thing::module::{Module, ModuleData, ModuleInterfaceType, ModuleType};
use std::any::Any;
use std::fmt::Debug;

/// Terrain decal types for ground effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainDecalType {
    Demoralized,
    Horde,
    HordeWithNationalismUpgrade,
    HordeVehicle,
    HordeWithNationalismUpgradeVehicle,
    Crate,
    HordeWithFanaticismUpgrade,
    ChemSuit,
    None,
    ShadowTexture,
}

/// Shadow types for drawable objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    None,
    Volume, // 3D shadow volume
    Decal,  // Projected shadow decal
    Blob,   // Simple blob shadow
    Hybrid, // Combination approach
}

/// Base trait for all DrawModule data
pub trait DrawModuleData: ModuleData + Debug {
    fn as_any(&self) -> &dyn Any;
}

/// Base trait for all Draw Modules
///
/// Draw modules handle the visual representation of game objects.
/// They are responsible for rendering, animation, and visual effects.
///
/// Reference: DrawModule in /GeneralsMD/Code/GameEngine/Include/Common/DrawModule.h
pub trait DrawModule: Module {
    /// Render the module with the given transform
    ///
    /// # Arguments
    /// * `transform_mtx` - World transform matrix for positioning
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D);

    /// Enable or disable shadow casting
    ///
    /// # Arguments
    /// * `enable` - True to enable shadows, false to disable
    fn set_shadows_enabled(&mut self, enable: bool);

    /// Release shadow resources (for Options screen)
    fn release_shadows(&mut self);

    /// Allocate shadow resources (for Options screen)
    fn allocate_shadows(&mut self);

    /// Set terrain decal type
    ///
    /// # Arguments
    /// * `decal_type` - Type of terrain decal to display
    fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        // Default implementation does nothing
        let _ = decal_type;
    }

    /// Set terrain decal size
    ///
    /// # Arguments
    /// * `x` - Width of decal
    /// * `y` - Height of decal
    fn set_terrain_decal_size(&mut self, _x: Real, _y: Real) {
        // Default implementation does nothing
    }

    /// Set drawable hidden state (matches C++ DrawModule::setHidden).
    fn set_hidden(&mut self, _hidden: bool) {
        // PARITY_NOTE: C++ DrawModule has no shared hidden-state storage at the base class.
        // Modules that own concrete client primitives must override this hook themselves.
    }

    /// Set terrain decal opacity
    ///
    /// # Arguments
    /// * `opacity` - Opacity value (0.0 = transparent, 1.0 = opaque)
    fn set_terrain_decal_opacity(&mut self, _opacity: Real) {
        // Default implementation does nothing
    }

    /// Set whether this object is fully obscured by shroud
    ///
    /// # Arguments
    /// * `fully_obscured` - True if completely hidden by fog of war
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool);

    /// Check if this drawable is currently visible
    ///
    /// Used for limiting tree sway and other performance optimizations
    /// to visible objects only.
    ///
    /// # Returns
    /// True if visible, false if culled or hidden
    fn is_visible(&self) -> bool {
        true // Default to visible
    }

    /// React to object transform change
    ///
    /// Called when the object's position, rotation, or scale changes.
    ///
    /// # Arguments
    /// * `old_mtx` - Previous transform matrix
    /// * `old_pos` - Previous position
    /// * `old_angle` - Previous rotation angle
    fn react_to_transform_change(&mut self, old_mtx: &Matrix3D, old_pos: &Coord3D, old_angle: Real);

    /// React to geometry change
    ///
    /// Called when the object's geometry (model, mesh, etc.) changes.
    fn react_to_geometry_change(&mut self);

    /// Check if this is a laser beam
    ///
    /// # Returns
    /// True if this is a laser draw module
    fn is_laser(&self) -> bool {
        false
    }

    /// Get ObjectDrawInterface if this module supports it
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        None
    }

    /// Get mutable ObjectDrawInterface if this module supports it
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        None
    }

    /// Get DebrisDrawInterface if this module supports it
    fn get_debris_draw_interface(&self) -> Option<&dyn DebrisDrawInterface> {
        None
    }

    /// Get mutable DebrisDrawInterface if this module supports it
    fn get_debris_draw_interface_mut(&mut self) -> Option<&mut dyn DebrisDrawInterface> {
        None
    }

    /// Get TracerDrawInterface if this module supports it
    fn get_tracer_draw_interface(&self) -> Option<&dyn TracerDrawInterface> {
        None
    }

    /// Get mutable TracerDrawInterface if this module supports it
    fn get_tracer_draw_interface_mut(&mut self) -> Option<&mut dyn TracerDrawInterface> {
        None
    }

    /// Get RopeDrawInterface if this module supports it
    fn get_rope_draw_interface(&self) -> Option<&dyn RopeDrawInterface> {
        None
    }

    /// Get mutable RopeDrawInterface if this module supports it
    fn get_rope_draw_interface_mut(&mut self) -> Option<&mut dyn RopeDrawInterface> {
        None
    }

    /// Get LaserDrawInterface if this module supports it
    fn get_laser_draw_interface(&self) -> Option<&dyn LaserDrawInterface> {
        None
    }

    /// Get mutable LaserDrawInterface if this module supports it
    fn get_laser_draw_interface_mut(&mut self) -> Option<&mut dyn LaserDrawInterface> {
        None
    }
}

/// Interface for full object drawing with model conditions
///
/// Reference: ObjectDrawInterface in DrawModule.h
pub trait ObjectDrawInterface {
    /// Get render object information (client-only)
    ///
    /// WARNING: This method must ONLY be called from the client, NEVER from game logic.
    ///
    /// # Arguments
    /// * `pos` - Output position
    /// * `bounding_sphere_radius` - Output bounding sphere radius
    /// * `transform` - Output transform matrix
    ///
    /// # Returns
    /// True if successful, false if render object not available
    fn client_only_get_render_obj_info(
        &self,
        pos: &mut Coord3D,
        bounding_sphere_radius: &mut Real,
        transform: &mut Matrix3D,
    ) -> bool;

    /// Get render object bounding box (client-only)
    ///
    /// # Arguments
    /// * `boundbox` - Output oriented bounding box
    ///
    /// # Returns
    /// True if successful
    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut BoundingBox) -> bool;

    /// Get bone transform by name (client-only)
    ///
    /// # Arguments
    /// * `bone_name` - Name of bone to query
    /// * `transform` - Output transform matrix
    ///
    /// # Returns
    /// True if bone found
    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &AsciiString,
        transform: &mut Matrix3D,
    ) -> bool;

    /// Get pristine bone positions for a condition state
    ///
    /// Returns bone positions from the default model state (at origin, default rotation).
    /// Looks for bones named "boneNamePrefixQQ" where QQ is 01, 02, 03, etc.
    ///
    /// # Arguments
    /// * `condition` - Model condition to query
    /// * `bone_name_prefix` - Prefix of bone names to search for
    /// * `start_index` - Starting index (1-based, use 0 for no suffix)
    /// * `positions` - Output array for bone positions
    /// * `transforms` - Output array for bone transforms
    /// * `max_bones` - Maximum number of bones to retrieve
    ///
    /// # Returns
    /// Number of bones found and copied
    fn get_pristine_bone_positions(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize;

    /// Get current bone positions in world space
    ///
    /// # Arguments
    /// * `bone_name_prefix` - Prefix of bone names
    /// * `start_index` - Starting index
    /// * `positions` - Output positions
    /// * `transforms` - Output transforms
    /// * `max_bones` - Maximum bones to retrieve
    ///
    /// # Returns
    /// Number of bones found
    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize;

    /// Get projectile launch offset for weapon
    ///
    /// # Arguments
    /// * `condition` - Current model condition
    /// * `weapon_slot` - Weapon slot index
    /// * `barrel_index` - Specific barrel to use
    /// * `launch_pos` - Output launch position
    /// * `turret_type` - Turret to use for pivot data
    /// * `turret_rot_pos` - Output turret rotation pivot position
    /// * `turret_pitch_pos` - Output turret pitch pivot position
    ///
    /// # Returns
    /// True if launch position found
    fn get_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        launch_pos: &mut Matrix3D,
        turret_type: TurretType,
        turret_rot_pos: &mut Coord3D,
        turret_pitch_pos: &mut Coord3D,
    ) -> bool;

    /// Update projectile clip status
    ///
    /// Shows/hides projectile geometry based on remaining ammo.
    ///
    /// # Arguments
    /// * `shots_remaining` - Shots left in clip
    /// * `max_shots` - Maximum shots in clip
    /// * `weapon_slot` - Weapon slot index
    fn update_projectile_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: usize,
    );

    /// Update visual representation of carried supplies
    ///
    /// # Arguments
    /// * `max_supply` - Maximum supply capacity
    /// * `current_supply` - Current supply amount
    fn update_supply_status(&mut self, max_supply: i32, current_supply: i32);

    /// Set hidden state
    ///
    /// # Arguments
    /// * `hidden` - True to hide, false to show
    fn set_hidden(&mut self, hidden: bool);

    /// Notify the draw module that a dependent drawable is ready for explicit draw.
    /// Mirrors C++ `ObjectDrawInterface::notifyDrawModuleDependencyCleared`.
    fn notify_draw_module_dependency_cleared(&mut self) {
        // PARITY_NOTE: only object/model draw modules participate in dependency-driven
        // explicit draw unblocking; non-model implementations intentionally do nothing.
    }

    /// Replace model condition state
    ///
    /// # Arguments
    /// * `condition` - New model condition flags
    fn replace_model_condition_state(&mut self, condition: &ModelConditionFlags);

    /// Handle weapon fire FX
    ///
    /// # Arguments
    /// * `weapon_slot` - Weapon slot that fired
    /// * `barrel_index` - Specific barrel that fired
    /// * `victim_pos` - Position of target
    ///
    /// # Returns
    /// True if FX handled
    fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: usize,
        barrel_index: i32,
        victim_pos: &Coord3D,
    ) -> bool;

    /// Get number of weapon barrels for a slot
    ///
    /// # Arguments
    /// * `weapon_slot` - Weapon slot to query
    ///
    /// # Returns
    /// Number of barrels
    fn get_barrel_count(&self, weapon_slot: usize) -> i32;
}

/// Interface for debris particles
///
/// Reference: DebrisDrawInterface in DrawModule.h
pub trait DebrisDrawInterface {
    /// Set debris model name and appearance
    ///
    /// # Arguments
    /// * `name` - Model name
    /// * `color` - Color tint
    /// * `shadow_type` - Type of shadow to cast
    fn set_model_name(&mut self, name: AsciiString, color: Color, shadow_type: ShadowType);

    /// Set debris animation names
    ///
    /// # Arguments
    /// * `initial` - Initial animation
    /// * `flying` - Animation while airborne
    /// * `final_anim` - Final landing animation
    /// * `final_fx` - FX to play on final impact
    fn set_anim_names(
        &mut self,
        initial: AsciiString,
        flying: AsciiString,
        final_anim: AsciiString,
        final_fx: Option<&FXList>,
    );
}

/// Interface for tracer bullets
///
/// Reference: TracerDrawInterface in DrawModule.h
pub trait TracerDrawInterface {
    /// Set tracer parameters
    ///
    /// # Arguments
    /// * `speed` - Speed of tracer in units/second
    /// * `length` - Length of tracer trail
    /// * `width` - Width of tracer
    /// * `color` - RGB color
    /// * `initial_opacity` - Starting opacity (0.0-1.0)
    fn set_tracer_parms(
        &mut self,
        speed: Real,
        length: Real,
        width: Real,
        color: &RGBColor,
        initial_opacity: Real,
    );
}

/// Interface for rope rendering
///
/// Reference: RopeDrawInterface in DrawModule.h
pub trait RopeDrawInterface {
    /// Initialize rope parameters
    fn init_rope_parms(
        &mut self,
        length: Real,
        width: Real,
        color: &RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    );

    /// Set current rope length
    fn set_rope_cur_len(&mut self, length: Real);

    /// Set rope speed and acceleration
    fn set_rope_speed(&mut self, cur_speed: Real, max_speed: Real, accel: Real);
}

/// Interface for laser beams
///
/// Reference: LaserDrawInterface in DrawModule.h
pub trait LaserDrawInterface {
    /// Get laser template width
    ///
    /// # Returns
    /// Width of laser beam from template
    fn get_laser_template_width(&self) -> Real;
}

/// RGB color (no alpha)
#[derive(Debug, Clone, Copy)]
pub struct RGBColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGBColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0)
    }
}

/// Oriented bounding box
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub center: Coord3D,
    pub extents: Coord3D,
    pub rotation: Matrix3D,
}

impl BoundingBox {
    pub fn new() -> Self {
        Self {
            center: Coord3D::origin(),
            extents: Coord3D::new(1.0, 1.0, 1.0),
            rotation: Matrix3D::IDENTITY,
        }
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::new()
    }
}
