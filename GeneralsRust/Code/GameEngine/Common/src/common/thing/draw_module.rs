////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Draw module base types and traits for rendering system

use crate::common::{
    ini::{Coord3D, Matrix3D},
    rts::Real,
    system::{Snapshotable, Xfer},
    thing::module::{
        BaseDrawableModule, BaseModuleData, Drawable, DrawableModule as BaseDrawableModuleTrait,
        Module, ModuleData, NameKeyType,
    },
};
use std::sync::Arc;

/// Module type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Behavior = 0,
    Draw = 1,
    ClientUpdate = 2,
}

/// Module interface flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleInterfaceFlags(pub u32);

impl ModuleInterfaceFlags {
    pub const UPDATE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000001);
    pub const DIE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000002);
    pub const DAMAGE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000004);
    pub const CREATE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000008);
    pub const COLLIDE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000010);
    pub const BODY: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000020);
    pub const CONTAIN: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000040);
    pub const UPGRADE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000080);
    pub const SPECIAL_POWER: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000100);
    pub const DESTROY: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000200);
    pub const DRAW: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000400);
    pub const CLIENT_UPDATE: ModuleInterfaceFlags = ModuleInterfaceFlags(0x00000800);
}

/// Shadow types for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    None,
    Volume,
    Decal,
}

/// Terrain decal types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainDecalType {
    None,
    Crater,
    Scorch,
}

/// RGB color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Render cost estimation for performance analysis
#[cfg(any(debug_assertions, feature = "internal"))]
#[derive(Debug, Default, Clone)]
pub struct RenderCost {
    draw_call_count: i32,
    sorted_mesh_count: i32,
    skin_mesh_count: i32,
    bone_count: i32,
    shadow_draw_count: i32,
}

#[cfg(any(debug_assertions, feature = "internal"))]
impl RenderCost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.draw_call_count = 0;
        self.sorted_mesh_count = 0;
        self.skin_mesh_count = 0;
        self.bone_count = 0;
        self.shadow_draw_count = 0;
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

    // Getters
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

/// Draw interface for debris rendering
pub trait DebrisDrawInterface {
    fn set_model_name(&mut self, name: String, color: RgbColor, shadow_type: ShadowType);
    fn set_anim_names(
        &mut self,
        initial: String,
        flying: String,
        final_: String,
        final_fx: Option<Arc<dyn std::any::Any>>,
    );
}

/// Draw interface for tracer rendering
pub trait TracerDrawInterface {
    fn set_tracer_params(
        &mut self,
        speed: Real,
        length: Real,
        width: Real,
        color: RgbColor,
        initial_opacity: Real,
    );
}

/// Draw interface for rope rendering
pub trait RopeDrawInterface {
    fn init_rope_params(
        &mut self,
        length: Real,
        width: Real,
        color: RgbColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    );
    fn set_rope_cur_len(&mut self, length: Real);
    fn set_rope_speed(&mut self, cur_speed: Real, max_speed: Real, accel: Real);
}

/// Draw interface for laser rendering
pub trait LaserDrawInterface {
    fn get_laser_template_width(&self) -> Real;
}

/// Main object drawing interface
pub trait ObjectDrawInterface {
    // Position and transform queries (client-only)
    fn client_only_get_render_obj_info(&self) -> Option<(Coord3D, Real, Matrix3D)>;

    // Bone and animation methods
    fn get_pristine_bone_positions_for_condition_state(
        &self,
        condition: u32,
        bone_name_prefix: &str,
        start_index: i32,
        max_bones: i32,
    ) -> Vec<(Coord3D, Matrix3D)>;
    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        max_bones: i32,
    ) -> Vec<(Coord3D, Matrix3D)>;
    fn get_current_worldspace_client_bone_positions(&self, bone_name: &str) -> Option<Matrix3D>;

    // Weapon and projectile methods
    fn update_projectile_clip_status(&mut self, shots_remaining: u32, max_shots: u32, slot: u32);
    fn update_draw_module_supply_status(&mut self, max_supply: i32, current_supply: i32);
    fn notify_draw_module_dependency_cleared(&mut self);

    // State management
    fn set_hidden(&mut self, hidden: bool);
    fn replace_model_condition_state(&mut self, condition: u32);
    fn replace_indicator_color(&mut self, color: RgbColor);
    fn set_selectable(&mut self, selectable: bool);

    // Animation control
    fn set_animation_loop_duration(&mut self, num_frames: u32);
    fn set_animation_completion_time(&mut self, num_frames: u32);
    fn set_animation_frame(&mut self, frame: i32);
    fn set_pause_animation(&mut self, pause_anim: bool);
    fn update_bones_for_client_particle_systems(&mut self) -> bool;

    // Sub-object management
    fn update_sub_objects(&mut self);
    fn show_sub_object(&mut self, name: &str, show: bool);

    #[cfg(feature = "allow_anim_inquiries")]
    fn get_animation_scrub_scalar(&self) -> Real {
        0.0
    }
}

/// Base trait for all drawable modules
pub trait DrawableModuleTrait {
    fn get_module_type() -> ModuleType {
        ModuleType::Draw
    }
    fn get_interface_mask() -> ModuleInterfaceFlags {
        ModuleInterfaceFlags::DRAW
    }

    fn do_draw_module(&self, transform_mtx: &Matrix3D);

    fn set_shadows_enabled(&mut self, enable: bool);
    fn release_shadows(&mut self);
    fn allocate_shadows(&mut self);

    #[cfg(any(debug_assertions, feature = "internal"))]
    fn get_render_cost(&self) -> RenderCost {
        RenderCost::new()
    }

    fn set_terrain_decal(&mut self, _decal_type: TerrainDecalType) {}
    fn set_terrain_decal_size(&mut self, _x: Real, _y: Real) {}
    fn set_terrain_decal_opacity(&mut self, _opacity: Real) {}

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool);

    fn is_visible(&self) -> bool {
        true
    }

    fn react_to_transform_change(&mut self, old_mtx: &Matrix3D, old_pos: &Coord3D, old_angle: Real);
    fn react_to_geometry_change(&mut self);

    fn is_laser(&self) -> bool {
        false
    }

    // Interface acquisition - return None by default, override as needed
    fn as_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        None
    }
    fn as_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        None
    }
    fn as_debris_draw_interface(&self) -> Option<&dyn DebrisDrawInterface> {
        None
    }
    fn as_debris_draw_interface_mut(&mut self) -> Option<&mut dyn DebrisDrawInterface> {
        None
    }
    fn as_tracer_draw_interface(&self) -> Option<&dyn TracerDrawInterface> {
        None
    }
    fn as_tracer_draw_interface_mut(&mut self) -> Option<&mut dyn TracerDrawInterface> {
        None
    }
    fn as_rope_draw_interface(&self) -> Option<&dyn RopeDrawInterface> {
        None
    }
    fn as_rope_draw_interface_mut(&mut self) -> Option<&mut dyn RopeDrawInterface> {
        None
    }
    fn as_laser_draw_interface(&self) -> Option<&dyn LaserDrawInterface> {
        None
    }
    fn as_laser_draw_interface_mut(&mut self) -> Option<&mut dyn LaserDrawInterface> {
        None
    }

    // Serialization support
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn load_post_process(&mut self);
}

/// Concrete draw module implementation
pub struct DrawModule {
    base: BaseDrawableModule,
}

impl DrawModule {
    pub fn new() -> Self {
        Self::new_with_base(Arc::new(BaseModuleData::new()), 0, None)
    }

    pub fn new_with_base(
        module_data: Arc<dyn ModuleData>,
        module_name_key: NameKeyType,
        drawable: Option<Arc<dyn Drawable>>,
    ) -> Self {
        Self {
            base: BaseDrawableModule::new(module_data, module_name_key, drawable),
        }
    }

    pub fn get_drawable(&self) -> Option<&dyn Drawable> {
        self.base.get_drawable()
    }
}

impl Default for DrawModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for DrawModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ DrawModule::xfer writes its own version before DrawableModule::xfer.
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("DrawModule::xfer version failed: {}", e))?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Module for DrawModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.base.get_module_data()
    }
}

impl BaseDrawableModuleTrait for DrawModule {
    fn get_drawable(&self) -> Option<&dyn Drawable> {
        self.base.get_drawable()
    }
}

impl DrawableModuleTrait for DrawModule {
    fn do_draw_module(&self, _transform_mtx: &Matrix3D) {
        // Base implementation - would be overridden by concrete types
    }

    fn set_shadows_enabled(&mut self, _enable: bool) {
        // Base implementation
    }

    fn release_shadows(&mut self) {
        // Base implementation
    }

    fn allocate_shadows(&mut self) {
        // Base implementation
    }

    fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {
        // Base implementation
    }

    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        // Base implementation
    }

    fn react_to_geometry_change(&mut self) {
        // Base implementation
    }

    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        <Self as Snapshotable>::crc(self, _xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        <Self as Snapshotable>::xfer(self, xfer)
    }

    fn load_post_process(&mut self) {
        let _ = <Self as Snapshotable>::load_post_process(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn draw_module_xfer_extends_drawable_and_module_versions() {
        let mut module = DrawModule::new();
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = XferSave::new(cursor, 0);
            DrawableModuleTrait::xfer(&mut module, &mut xfer).unwrap();
        }

        assert_eq!(bytes, vec![1, 1, 1]);
    }
}
