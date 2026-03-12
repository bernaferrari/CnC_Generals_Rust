// FILE: draw_module.rs
// Draw module base class and interfaces
// Ported from C++ DrawModule.h and DrawModule.cpp
// Author: Colin Day, September 2002

use crate::Common::game_type::{Real, Bool, Int, UnsignedInt, WeaponSlotType, Color};
use crate::Common::model_state::ModelConditionFlags;
use crate::WWMath::matrix3d::Matrix3D;
use crate::Common::coord3d::Coord3D;

/// Terrain decal types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TerrainDecalType {
    DemoralizedObsolete = 0,
    Horde,
    HordeWithNationalismUpgrade,
    HordeVehicle,
    HordeWithNationalismUpgradeVehicle,
    Crate,
    HordeWithFanaticismUpgrade,
    Chemsuit,
    None,
    ShadowTexture,
    Max,
}

/// Shadow types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    None,
    Additive,
    Volumetric,
    AdditiveDecal,
    VolumetricDecal,
}

/// Render cost estimation structure
#[derive(Debug, Default, Clone)]
pub struct RenderCost {
    draw_call_count: i32,
    sorted_mesh_count: i32,
    skin_mesh_count: i32,
    bone_count: i32,
    shadow_draw_count: i32,
}

impl RenderCost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn add_draw_calls(&mut self, count: i32) {
        self.draw_call_count += count;
    }

    pub fn add_sorted_meshes(&mut self, count: i32) {
        self.sorted_mesh_count += count;
    }

    pub fn add_skin_meshes(&mut self, count: i32) {
        self.skin_mesh_count += count;
    }

    pub fn add_bones(&mut self, count: i32) {
        self.bone_count += count;
    }

    pub fn add_shadow_draw_calls(&mut self, count: i32) {
        self.shadow_draw_count += count;
    }

    pub fn get_draw_call_count(&self) -> i32 {
        self.draw_call_count
    }

    pub fn get_sorted_mesh_count(&self) -> i32 {
        self.sorted_mesh_count
    }

    pub fn get_skin_mesh_count(&self) -> i32 {
        self.skin_mesh_count
    }

    pub fn get_bone_count(&self) -> i32 {
        self.bone_count
    }

    pub fn get_shadow_draw_count(&self) -> i32 {
        self.shadow_draw_count
    }
}

/// Which turret type for projectile launch calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhichTurretType {
    Primary,
    Secondary,
}

/// Base trait for all draw modules
pub trait DrawModule {
    /// Perform the drawing operation
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D);

    /// Enable or disable shadows
    fn set_shadows_enabled(&mut self, enable: Bool);

    /// Free all shadow resources (used by Options screen)
    fn release_shadows(&mut self);

    /// Create shadow resources if not already present (used by Options screen)
    fn allocate_shadows(&mut self);

    /// Set terrain decal type
    fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {}

    /// Set terrain decal size
    fn set_terrain_decal_size(&mut self, x: Real, y: Real) {}

    /// Set terrain decal opacity
    fn set_terrain_decal_opacity(&mut self, opacity: Real) {}

    /// Set whether fully obscured by shroud
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: Bool);

    /// Check if this draw module is visible
    fn is_visible(&self) -> bool {
        true
    }

    /// React to transform change
    fn react_to_transform_change(&mut self, old_mtx: &Matrix3D, old_pos: &Coord3D, old_angle: Real);

    /// React to geometry change
    fn react_to_geometry_change(&mut self);

    /// Check if this is a laser draw module
    fn is_laser(&self) -> bool {
        false
    }

    /// Get render cost estimation
    fn get_render_cost(&self, rc: &mut RenderCost) {}

    /// Try to get ObjectDrawInterface
    fn get_object_draw_interface(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        None
    }

    /// Try to get const ObjectDrawInterface
    fn get_object_draw_interface_const(&self) -> Option<&dyn ObjectDrawInterface> {
        None
    }

    /// Try to get DebrisDrawInterface
    fn get_debris_draw_interface(&mut self) -> Option<&mut dyn DebrisDrawInterface> {
        None
    }

    /// Try to get TracerDrawInterface
    fn get_tracer_draw_interface(&mut self) -> Option<&mut dyn TracerDrawInterface> {
        None
    }

    /// Try to get RopeDrawInterface
    fn get_rope_draw_interface(&mut self) -> Option<&mut dyn RopeDrawInterface> {
        None
    }

    /// Try to get LaserDrawInterface
    fn get_laser_draw_interface(&mut self) -> Option<&mut dyn LaserDrawInterface> {
        None
    }
}

/// Interface for object drawing operations
pub trait ObjectDrawInterface {
    /// Get render object information (client-only!)
    fn client_only_get_render_obj_info(
        &self,
        pos: &mut Coord3D,
        bounding_sphere_radius: &mut Real,
        transform: &mut Matrix3D,
    ) -> Bool;

    /// Get render object bounding box (client-only!)
    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut OBBox) -> Bool;

    /// Get render object bone transform (client-only!)
    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &str,
        set_tm: &mut Matrix3D,
    ) -> Bool;

    /// Get pristine bone positions for a condition state
    fn get_pristine_bone_positions_for_condition_state(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &str,
        start_index: Int,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: Int,
    ) -> Int;

    /// Get current bone positions
    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: Int,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: Int,
    ) -> Int;

    /// Get current worldspace client bone positions
    fn get_current_worldspace_client_bone_positions(
        &self,
        bone_name: &str,
        transform: &mut Matrix3D,
    ) -> Bool;

    /// Get projectile launch offset
    fn get_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        wslot: WeaponSlotType,
        specific_barrel_to_use: Int,
        launch_pos: &mut Matrix3D,
        tur: WhichTurretType,
        turret_rot_pos: &mut Coord3D,
        turret_pitch_pos: Option<&mut Coord3D>,
    ) -> Bool;

    /// Update projectile clip status for visual feedback
    fn update_projectile_clip_status(
        &mut self,
        shots_remaining: UnsignedInt,
        max_shots: UnsignedInt,
        slot: WeaponSlotType,
    );

    /// Update draw module supply status for visual feedback
    fn update_draw_module_supply_status(&mut self, max_supply: Int, current_supply: Int);

    /// Notify that a dependency has been cleared
    fn notify_draw_module_dependency_cleared(&mut self);

    /// Set hidden state
    fn set_hidden(&mut self, hidden: Bool);

    /// Replace model condition state
    fn replace_model_condition_state(&mut self, condition: &ModelConditionFlags);

    /// Replace indicator color
    fn replace_indicator_color(&mut self, color: Color);

    /// Handle weapon fire FX
    fn handle_weapon_fire_fx(
        &mut self,
        wslot: WeaponSlotType,
        specific_barrel_to_use: Int,
        fxl: *const std::ffi::c_void, // FXList pointer
        weapon_speed: Real,
        victim_pos: &Coord3D,
        damage_radius: Real,
    ) -> Bool;

    /// Get barrel count for weapon slot
    fn get_barrel_count(&self, wslot: WeaponSlotType) -> Int;

    /// Set selectable state
    fn set_selectable(&mut self, selectable: Bool);

    /// Set animation loop duration
    fn set_animation_loop_duration(&mut self, num_frames: UnsignedInt);

    /// Set animation completion time
    fn set_animation_completion_time(&mut self, num_frames: UnsignedInt);

    /// Update bones for client particle systems
    fn update_bones_for_client_particle_systems(&mut self) -> Bool;

    /// Set animation frame
    fn set_animation_frame(&mut self, frame: i32);

    /// Set pause animation
    fn set_pause_animation(&mut self, pause_anim: Bool);

    /// Update sub-objects
    fn update_sub_objects(&mut self);

    /// Show or hide sub-object
    fn show_sub_object(&mut self, name: &str, show: Bool);

    /// Get animation scrub scalar (0.0 to 1.0)
    fn get_animation_scrub_scalar(&self) -> Real {
        0.0
    }
}

/// Interface for debris drawing
pub trait DebrisDrawInterface {
    fn set_model_name(&mut self, name: String, color: Color, shadow_type: ShadowType);
    fn set_anim_names(
        &mut self,
        initial: String,
        flying: String,
        final_anim: String,
        final_fx: *const std::ffi::c_void,
    );
}

/// Interface for tracer drawing
pub trait TracerDrawInterface {
    fn set_tracer_parms(
        &mut self,
        speed: Real,
        length: Real,
        width: Real,
        color: &RGBColor,
        initial_opacity: Real,
    );
}

/// Interface for rope drawing
pub trait RopeDrawInterface {
    fn init_rope_parms(
        &mut self,
        length: Real,
        width: Real,
        color: &RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    );
    fn set_rope_cur_len(&mut self, length: Real);
    fn set_rope_speed(&mut self, cur_speed: Real, max_speed: Real, accel: Real);
}

/// Interface for laser drawing
pub trait LaserDrawInterface {
    fn get_laser_template_width(&self) -> Real;
}

/// RGB Color structure
#[derive(Debug, Clone, Copy)]
pub struct RGBColor {
    pub red: Real,
    pub green: Real,
    pub blue: Real,
}

impl RGBColor {
    pub fn new(r: Real, g: Real, b: Real) -> Self {
        Self {
            red: r,
            green: g,
            blue: b,
        }
    }

    pub fn from_int(value: u32) -> Self {
        Self {
            red: ((value >> 16) & 0xFF) as Real / 255.0,
            green: ((value >> 8) & 0xFF) as Real / 255.0,
            blue: (value & 0xFF) as Real / 255.0,
        }
    }

    pub fn set_from_int(&mut self, value: u32) {
        *self = Self::from_int(value);
    }
}

/// Oriented bounding box
#[derive(Debug, Clone)]
pub struct OBBox {
    pub center: Coord3D,
    pub extent: Coord3D,
    pub axes: [Coord3D; 3],
}

impl OBBox {
    pub fn new() -> Self {
        Self {
            center: Coord3D::new(0.0, 0.0, 0.0),
            extent: Coord3D::new(0.0, 0.0, 0.0),
            axes: [
                Coord3D::new(1.0, 0.0, 0.0),
                Coord3D::new(0.0, 1.0, 0.0),
                Coord3D::new(0.0, 0.0, 1.0),
            ],
        }
    }
}

impl Default for OBBox {
    fn default() -> Self {
        Self::new()
    }
}
