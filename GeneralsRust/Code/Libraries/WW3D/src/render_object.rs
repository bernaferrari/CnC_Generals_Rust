// Render Object System
// Ported from rendobj.h

use crate::math::*;
use crate::hierarchy::HTree;
use crate::animation::HAnimation;
use crate::material::MaterialInfo;
use crate::{Result, W3DError};
use std::sync::Arc;

// Class IDs for render objects
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderObjectClassId {
    Unknown = 0xFFFFFFFF,
    Mesh = 0,
    HModel = 1,
    DistLod = 2,
    PredLodGroup = 3,
    HLod = 23,
    NullObject = 20,
    Collection = 21,
    Last = 0x0000FFFF,
}

// Animation modes
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    Manual = 0,
    Loop = 1,
    Once = 2,
    LoopPingPong = 3,
    LoopBackwards = 4,
    OnceBackwards = 5,
}

// Render object flags
bitflags::bitflags! {
    pub struct RenderObjectFlags: u32 {
        const COLLISION_TYPE_MASK = 0x000000FF;
        const IS_VISIBLE = 0x00000100;
        const IS_NOT_HIDDEN = 0x00000200;
        const IS_NOT_ANIMATION_HIDDEN = 0x00000400;
        const IS_FORCE_VISIBLE = 0x00000800;
        const BOUNDING_VOLUMES_VALID = 0x00002000;
        const IS_TRANSLUCENT = 0x00004000;
        const IGNORE_LOD_COST = 0x00008000;
        const SUBOBJS_MATCH_LOD = 0x00010000;
        const SUBOBJ_TRANSFORMS_DIRTY = 0x00020000;
        const IS_ALPHA = 0x00040000;
        const IS_ADDITIVE = 0x00100000;
        const IS_SELF_SHADOWED = 0x00080000;

        const IS_REALLY_VISIBLE = Self::IS_VISIBLE.bits | Self::IS_NOT_HIDDEN.bits | Self::IS_NOT_ANIMATION_HIDDEN.bits;
        const IS_NOT_HIDDEN_AT_ALL = Self::IS_NOT_HIDDEN.bits | Self::IS_NOT_ANIMATION_HIDDEN.bits;
        const DEFAULT_BITS = 0x000000FF | Self::IS_NOT_HIDDEN.bits | Self::IS_NOT_ANIMATION_HIDDEN.bits;
    }
}

// Render information for rendering
pub struct RenderInfo {
    pub camera_position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

// Special render modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialRenderMode {
    Shadow,
    GBuffer,
    Outline,
}

pub struct SpecialRenderInfo {
    pub mode: SpecialRenderMode,
    pub render_info: RenderInfo,
}

// Base trait for all render objects
pub trait RenderObject: Send + Sync {
    // Identity and classification
    fn class_id(&self) -> RenderObjectClassId;
    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: String);
    fn get_num_polys(&self) -> u32;

    // Rendering
    fn render(&mut self, rinfo: &RenderInfo) -> Result<()>;
    fn special_render(&mut self, _rinfo: &SpecialRenderInfo) -> Result<()> {
        Ok(())
    }
    fn on_frame_update(&mut self) {}
    fn restart(&mut self) {}

    // Transform
    fn get_transform(&self) -> &Mat4;
    fn set_transform(&mut self, transform: Mat4);
    fn set_position(&mut self, position: Vec3);
    fn get_position(&self) -> Vec3 {
        Vec3::new(
            self.get_transform()[(0, 3)],
            self.get_transform()[(1, 3)],
            self.get_transform()[(2, 3)],
        )
    }

    // Hierarchy
    fn get_num_sub_objects(&self) -> usize {
        0
    }
    fn get_sub_object(&self, _index: usize) -> Option<&dyn RenderObject> {
        None
    }
    fn get_sub_object_mut(&mut self, _index: usize) -> Option<&mut dyn RenderObject> {
        None
    }
    fn add_sub_object(&mut self, _object: Box<dyn RenderObject>) -> Result<()> {
        Err(W3DError::RenderError("Sub-objects not supported".to_string()))
    }

    // Animation
    fn set_animation_none(&mut self) {}
    fn set_animation(&mut self, _motion: Arc<dyn HAnimation>, _frame: f32, _mode: AnimationMode) {}
    fn set_animation_blend(
        &mut self,
        _motion0: Arc<dyn HAnimation>,
        _frame0: f32,
        _motion1: Arc<dyn HAnimation>,
        _frame1: f32,
        _percentage: f32,
    ) {}

    fn get_num_bones(&self) -> usize {
        0
    }
    fn get_bone_name(&self, _bone_index: usize) -> Option<&str> {
        None
    }
    fn get_bone_index(&self, _bone_name: &str) -> Option<usize> {
        None
    }
    fn get_bone_transform(&self, _bone_index: usize) -> Option<&Mat4> {
        None
    }
    fn get_htree(&self) -> Option<&HTree> {
        None
    }

    // Bounding volumes
    fn get_bounding_sphere(&self) -> Sphere;
    fn get_bounding_box(&self) -> AABox;
    fn get_obj_space_bounding_sphere(&self) -> Sphere;
    fn get_obj_space_bounding_box(&self) -> AABox;
    fn update_obj_space_bounding_volumes(&mut self) {}

    // Collision detection
    fn cast_ray(&self, _ray: &Ray) -> Option<f32> {
        None
    }
    fn intersect_aabox(&self, _box: &AABox) -> bool {
        false
    }
    fn intersect_obbox(&self, _box: &OBBox) -> bool {
        false
    }

    // LOD
    fn prepare_lod(&mut self, _camera_position: &Vec3) {}
    fn set_lod_level(&mut self, _level: usize) {}
    fn get_lod_level(&self) -> usize {
        0
    }
    fn get_lod_count(&self) -> usize {
        1
    }
    fn increment_lod(&mut self) {}
    fn decrement_lod(&mut self) {}
    fn get_cost(&self) -> f32 {
        self.get_num_polys() as f32
    }

    // Visibility and flags
    fn get_flags(&self) -> RenderObjectFlags;
    fn set_flags(&mut self, flags: RenderObjectFlags);

    fn is_visible(&self) -> bool {
        self.get_flags().contains(RenderObjectFlags::IS_VISIBLE)
    }
    fn set_visible(&mut self, visible: bool) {
        let mut flags = self.get_flags();
        flags.set(RenderObjectFlags::IS_VISIBLE, visible);
        self.set_flags(flags);
    }

    fn is_hidden(&self) -> bool {
        !self.get_flags().contains(RenderObjectFlags::IS_NOT_HIDDEN)
    }
    fn set_hidden(&mut self, hidden: bool) {
        let mut flags = self.get_flags();
        flags.set(RenderObjectFlags::IS_NOT_HIDDEN, !hidden);
        self.set_flags(flags);
    }

    fn is_translucent(&self) -> bool {
        self.get_flags().contains(RenderObjectFlags::IS_TRANSLUCENT)
    }
    fn set_translucent(&mut self, translucent: bool) {
        let mut flags = self.get_flags();
        flags.set(RenderObjectFlags::IS_TRANSLUCENT, translucent);
        self.set_flags(flags);
    }

    // Materials
    fn get_material_info(&self) -> Option<&MaterialInfo> {
        None
    }

    // Scaling
    fn scale(&mut self, scale: f32);
    fn scale_xyz(&mut self, _scale_x: f32, _scale_y: f32, _scale_z: f32) {}
}

// Base structure for render objects containing common data
pub struct RenderObjectBase {
    pub name: String,
    pub transform: Mat4,
    pub flags: RenderObjectFlags,
    pub cached_bounding_sphere: Sphere,
    pub cached_bounding_box: AABox,
    pub native_screen_size: f32,
    pub object_scale: f32,
}

impl RenderObjectBase {
    pub fn new(name: String) -> Self {
        Self {
            name,
            transform: Mat4::identity(),
            flags: RenderObjectFlags::DEFAULT_BITS,
            cached_bounding_sphere: Sphere::new(Vec3::zeros(), 0.0),
            cached_bounding_box: AABox::new(Vec3::zeros(), Vec3::zeros()),
            native_screen_size: 1.0,
            object_scale: 1.0,
        }
    }

    pub fn invalidate_bounding_volumes(&mut self) {
        self.flags.remove(RenderObjectFlags::BOUNDING_VOLUMES_VALID);
    }

    pub fn validate_bounding_volumes(&mut self) {
        self.flags.insert(RenderObjectFlags::BOUNDING_VOLUMES_VALID);
    }

    pub fn are_bounding_volumes_valid(&self) -> bool {
        self.flags.contains(RenderObjectFlags::BOUNDING_VOLUMES_VALID)
    }
}
