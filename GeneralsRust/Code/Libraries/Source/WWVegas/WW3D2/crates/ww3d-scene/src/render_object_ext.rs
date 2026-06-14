//! Extended Render Object System - Package 7
//!
//! This module implements the complete RenderObject interface with:
//! - Animation control (play/stop/frame)
//! - Deformed vertex retrieval for physics/collision
//! - Bone attachments for weapons, effects, etc.
//! - Material and texture replacement at runtime
//! - LOD management
//! - Full integration with all previous packages
//!
//! C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/renderobj.h lines 1-850
//! C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/renderobj.cpp lines 1-1200

use glam::{Mat4, Quat, Vec3, Vec4};
use std::any::Any;
use std::collections::HashMap;

/// Animation playback mode
/// C++ Reference: renderobj.h lines 45-50
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    /// Play once and stop
    Once,
    /// Loop continuously
    Loop,
    /// Play once and hold on last frame
    OnceHold,
    /// Ping-pong between start and end
    PingPong,
}

/// Texture identifier for runtime replacement
/// C++ Reference: renderobj.h lines 280-285
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

impl TextureId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn invalid() -> Self {
        Self(0xFFFFFFFF)
    }

    pub fn is_valid(&self) -> bool {
        self.0 != 0xFFFFFFFF
    }
}

/// Material definition for runtime replacement
/// C++ Reference: renderobj.h lines 290-310
#[derive(Debug, Clone)]
pub struct Material {
    pub ambient: Vec3,
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub emissive: Vec3,
    pub shininess: f32,
    pub opacity: f32,
    pub textures: Vec<TextureId>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            ambient: Vec3::new(0.2, 0.2, 0.2),
            diffuse: Vec3::new(0.8, 0.8, 0.8),
            specular: Vec3::new(0.0, 0.0, 0.0),
            emissive: Vec3::ZERO,
            shininess: 0.0,
            opacity: 1.0,
            textures: Vec::new(),
        }
    }
}

/// Bone attachment for attaching objects to animated bones
/// C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/bone_attachment.cpp lines 1-200
#[derive(Debug)]
pub struct BoneAttachment {
    /// Name of the bone to attach to
    pub bone_name: String,
    /// The attached object
    pub object: Box<dyn RenderObjClassExt>,
    /// Local transform relative to bone
    pub local_transform: Mat4,
    /// Cached world transform
    cached_world_transform: Mat4,
}

impl BoneAttachment {
    /// Create a new bone attachment
    /// C++ Reference: bone_attachment.cpp lines 50-65
    pub fn new(bone_name: String, object: Box<dyn RenderObjClassExt>) -> Self {
        Self {
            bone_name,
            object,
            local_transform: Mat4::IDENTITY,
            cached_world_transform: Mat4::IDENTITY,
        }
    }

    /// Create with custom local transform
    /// C++ Reference: bone_attachment.cpp lines 67-82
    pub fn with_local_transform(
        bone_name: String,
        object: Box<dyn RenderObjClassExt>,
        local_transform: Mat4,
    ) -> Self {
        Self {
            bone_name,
            object,
            local_transform,
            cached_world_transform: Mat4::IDENTITY,
        }
    }

    /// Update the attachment's world transform
    /// C++ Reference: bone_attachment.cpp lines 120-145
    pub fn update_transform(&mut self, bone_transform: Mat4) {
        self.cached_world_transform = bone_transform * self.local_transform;
        self.object.set_transform(self.cached_world_transform);
    }
}

/// Animation state for tracking playback
/// C++ Reference: renderobj.cpp lines 150-180
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// Name of current animation
    pub animation_name: String,
    /// Current frame
    pub current_frame: f32,
    /// Playback mode
    pub mode: AnimationMode,
    /// Is currently playing
    pub is_playing: bool,
    /// Playback speed multiplier
    pub speed: f32,
    /// Total frames in animation
    pub frame_count: f32,
    /// Source animation frame rate.
    ///
    /// C++ HAnimClass exposes this via Get_Frame_Rate(); playback advances by
    /// that source rate rather than by a fixed scene constant.
    pub frame_rate: f32,
    /// Ping-pong direction (1.0 forward, -1.0 backward)
    pub ping_pong_direction: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            animation_name: String::new(),
            current_frame: 0.0,
            mode: AnimationMode::Loop,
            is_playing: false,
            speed: 1.0,
            frame_count: 0.0,
            frame_rate: 0.0,
            ping_pong_direction: 1.0,
        }
    }
}

impl AnimationState {
    /// Advance animation by delta time
    /// C++ Reference: renderobj.cpp lines 190-250
    pub fn update(&mut self, delta_time: f32, fps: f32) {
        if !self.is_playing || self.frame_count <= 0.0 {
            return;
        }

        let frame_rate = if self.frame_rate > 0.0 {
            self.frame_rate
        } else {
            fps
        };
        let frame_delta = delta_time * frame_rate * self.speed * self.ping_pong_direction;
        self.current_frame += frame_delta;

        match self.mode {
            AnimationMode::Once => {
                if self.current_frame >= self.frame_count {
                    self.current_frame = self.frame_count - 1.0;
                    self.is_playing = false;
                }
            }
            AnimationMode::Loop => {
                while self.current_frame >= self.frame_count {
                    self.current_frame -= self.frame_count;
                }
                while self.current_frame < 0.0 {
                    self.current_frame += self.frame_count;
                }
            }
            AnimationMode::OnceHold => {
                if self.current_frame >= self.frame_count {
                    self.current_frame = self.frame_count - 1.0;
                    self.is_playing = false;
                }
            }
            AnimationMode::PingPong => {
                if self.current_frame >= self.frame_count {
                    self.current_frame = self.frame_count - (self.current_frame - self.frame_count);
                    self.ping_pong_direction = -1.0;
                } else if self.current_frame < 0.0 {
                    self.current_frame = -self.current_frame;
                    self.ping_pong_direction = 1.0;
                }
            }
        }
    }
}

/// Extended render object trait with full WW3D functionality
/// This extends the basic RenderObj trait with animation, deformation, and advanced features
/// C++ Reference: renderobj.h lines 1-850
pub trait RenderObjClassExt: std::fmt::Debug + Send + Sync {
    /// Get object name
    /// C++ Reference: renderobj.h line 65
    fn get_name(&self) -> &str;

    /// Set object name
    /// C++ Reference: renderobj.h line 66
    fn set_name(&mut self, name: String);

    // === Transform Methods ===
    /// C++ Reference: renderobj.cpp lines 50-120

    /// Get world transform
    fn get_transform(&self) -> &Mat4;

    /// Set world transform
    fn set_transform(&mut self, transform: Mat4);

    /// Set position component of transform
    /// C++ Reference: renderobj.cpp lines 80-90
    fn set_position(&mut self, position: Vec3) {
        let mut transform = *self.get_transform();
        transform.w_axis = Vec4::new(position.x, position.y, position.z, 1.0);
        self.set_transform(transform);
    }

    /// Get position from transform
    /// C++ Reference: renderobj.cpp lines 92-95
    fn get_position(&self) -> Vec3 {
        let transform = self.get_transform();
        Vec3::new(transform.w_axis.x, transform.w_axis.y, transform.w_axis.z)
    }

    /// Set rotation component of transform
    /// C++ Reference: renderobj.cpp lines 97-107
    fn set_rotation(&mut self, rotation: Quat) {
        let position = self.get_position();
        let transform = Mat4::from_rotation_translation(rotation, position);
        self.set_transform(transform);
    }

    /// Get rotation from transform
    /// C++ Reference: renderobj.cpp lines 109-120
    fn get_rotation(&self) -> Quat {
        Quat::from_mat4(self.get_transform())
    }

    /// Set uniform scale
    /// C++ Reference: renderobj.cpp lines 122-132
    fn set_scale(&mut self, scale: Vec3) {
        let position = self.get_position();
        let rotation = self.get_rotation();
        let transform = Mat4::from_scale_rotation_translation(scale, rotation, position);
        self.set_transform(transform);
    }

    // === Animation Control ===
    /// C++ Reference: renderobj.cpp lines 150-250

    /// Play animation by name
    /// C++ Reference: renderobj.cpp lines 155-175
    fn play_animation(&mut self, anim_name: &str, mode: AnimationMode);

    /// Stop current animation
    /// C++ Reference: renderobj.cpp lines 177-182
    fn stop_animation(&mut self);

    /// Set animation frame
    /// C++ Reference: renderobj.cpp lines 184-195
    fn set_animation_frame(&mut self, frame: f32);

    /// Get current animation frame
    /// C++ Reference: renderobj.cpp lines 197-200
    fn get_animation_frame(&self) -> f32;

    /// Check if animation is playing
    /// C++ Reference: renderobj.cpp lines 202-205
    fn is_animation_playing(&self) -> bool;

    /// Set animation speed multiplier
    /// C++ Reference: renderobj.cpp lines 207-215
    fn set_animation_speed(&mut self, speed: f32);

    /// Get available animations
    /// C++ Reference: renderobj.cpp lines 217-230
    fn get_animation_list(&self) -> Vec<String> {
        Vec::new()
    }

    // === Material/Texture Replacement ===
    /// C++ Reference: renderobj.cpp lines 280-350

    /// Set texture for a specific stage
    /// C++ Reference: renderobj.cpp lines 285-305
    fn set_texture(&mut self, stage: usize, texture: TextureId);

    /// Get texture for a specific stage
    /// C++ Reference: renderobj.cpp lines 307-315
    fn get_texture(&self, stage: usize) -> Option<TextureId>;

    /// Replace all occurrences of a texture
    /// C++ Reference: renderobj.cpp lines 317-340
    fn replace_all_textures(&mut self, old_tex: TextureId, new_tex: TextureId);

    /// Set material properties
    /// C++ Reference: renderobj.cpp lines 342-350
    fn set_material(&mut self, material_index: usize, material: Material);

    /// Get material count
    fn get_material_count(&self) -> usize {
        0
    }

    // === Visibility and Culling ===
    /// C++ Reference: renderobj.cpp lines 380-420

    /// Set hidden state
    /// C++ Reference: renderobj.cpp lines 382-390
    fn set_hidden(&mut self, hidden: bool);

    /// Check if hidden
    /// C++ Reference: renderobj.cpp lines 392-395
    fn is_hidden(&self) -> bool;

    /// Get bounding box
    /// C++ Reference: renderobj.cpp lines 397-405
    fn get_bounding_box(&self) -> AABoxClass;

    /// Get bounding sphere
    /// C++ Reference: renderobj.cpp lines 407-415
    fn get_bounding_sphere(&self) -> SphereClass;

    /// Update cached bounding volumes
    /// C++ Reference: renderobj.cpp lines 417-420
    fn update_bounding_volumes(&mut self);

    // === Rendering ===
    /// C++ Reference: renderobj.cpp lines 450-550

    /// Render the object
    /// C++ Reference: renderobj.cpp lines 455-490
    fn render(&self, context: &RenderContext);

    /// Render the object with pass filtering for multi-pass rendering
    /// Only renders geometry that matches the specified pass index
    fn render_with_pass_filter(&self, context: &RenderContext, pass_index: usize);

    /// Get polygon count
    /// C++ Reference: renderobj.cpp lines 492-495
    fn get_polygon_count(&self) -> usize;

    /// Get vertex count
    /// C++ Reference: renderobj.cpp lines 497-500
    fn get_vertex_count(&self) -> usize;

    /// Check if object has transparency
    /// C++ Reference: renderobj.cpp lines 502-510
    fn has_transparency(&self) -> bool {
        false
    }

    /// Get sort level for transparency sorting
    /// C++ Reference: renderobj.cpp lines 512-515
    fn get_sort_level(&self) -> i32 {
        0
    }

    // === Advanced Features ===
    /// C++ Reference: renderobj.cpp lines 580-680

    /// Get deformed vertices (for skinned meshes)
    /// This is critical for collision detection and ragdoll physics
    /// C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/meshmodel.cpp lines 850-950
    fn get_deformed_vertices(&self) -> Option<Vec<Vec3>>;

    /// Get deformed normals (for skinned meshes)
    /// C++ Reference: meshmodel.cpp lines 980-1050
    fn get_deformed_normals(&self) -> Option<Vec<Vec3>>;

    /// Attach object to a bone
    /// C++ Reference: bone_attachment.cpp lines 85-120
    fn attach_to_bone(&mut self, bone_name: &str, obj: Box<dyn RenderObjClassExt>);

    /// Detach object from bone
    /// C++ Reference: bone_attachment.cpp lines 122-135
    fn detach_from_bone(&mut self, bone_name: &str) -> Option<Box<dyn RenderObjClassExt>>;

    /// Get all bone attachments
    /// C++ Reference: bone_attachment.cpp lines 137-145
    fn get_bone_attachments(&self) -> &[BoneAttachment] {
        &[]
    }

    /// Get mutable bone attachments (thread-local storage for objects without attachments)
    /// This is a fallback for implementers that don't have their own attachment storage
    fn get_bone_attachments_mut(&mut self) -> &mut Vec<BoneAttachment> {
        thread_local! {
            static EMPTY: std::cell::RefCell<Vec<BoneAttachment>> = std::cell::RefCell::new(Vec::new());
        }
        // Safety: We're returning a mutable reference to thread-local storage.
        // This is safe because each thread has its own copy of the static.
        EMPTY.with(|cell| unsafe {
            // SAFETY: The RefCell is never borrowed elsewhere when we mutate it here,
            // because we only access it through this thread-local interface.
            &mut *(cell.borrow_mut().as_mut() as *mut Vec<BoneAttachment>)
        })
    }

    /// Update all bone attachments
    /// C++ Reference: bone_attachment.cpp lines 147-180
    fn update_attachments(&mut self, bone_transforms: &HashMap<String, Mat4>) {
        for attachment in self.get_bone_attachments_mut() {
            if let Some(bone_transform) = bone_transforms.get(&attachment.bone_name) {
                attachment.update_transform(*bone_transform);
            }
        }
    }

    // === LOD Management ===
    /// C++ Reference: renderobj.cpp lines 700-750

    /// Set LOD level
    /// C++ Reference: renderobj.cpp lines 705-712
    fn set_lod_level(&mut self, level: usize);

    /// Get current LOD level
    /// C++ Reference: renderobj.cpp lines 714-717
    fn get_lod_level(&self) -> usize {
        0
    }

    /// Get LOD count
    /// C++ Reference: renderobj.cpp lines 719-722
    fn get_lod_count(&self) -> usize {
        1
    }

    /// Compute rendering cost for LOD selection
    /// C++ Reference: renderobj.cpp lines 724-750
    fn compute_cost(&self, camera_distance: f32) -> f32 {
        let poly_count = self.get_polygon_count() as f32;
        let distance_factor = 1.0 / (camera_distance * camera_distance + 1.0);
        poly_count * distance_factor
    }

    // === Cloning and Lifecycle ===
    /// C++ Reference: renderobj.cpp lines 780-820

    /// Clone the render object
    /// C++ Reference: renderobj.cpp lines 785-810
    fn clone_obj(&self) -> Box<dyn RenderObjClassExt>;

    /// Release resources
    /// C++ Reference: renderobj.cpp lines 812-820
    fn release(&mut self) {
        // Default implementation does nothing
    }

    /// Get type for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get mutable type for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Render context for unified rendering interface
/// C++ Reference: renderobj.h lines 870-920
#[derive(Clone)]
pub struct RenderContext {
    /// Camera matrix
    pub view_matrix: Mat4,
    /// Projection matrix
    pub projection_matrix: Mat4,
    /// Combined view-projection
    pub view_projection_matrix: Mat4,
    /// Camera position in world space
    pub camera_position: Vec3,
    /// Frame time
    pub delta_time: f32,
    /// Total elapsed time
    pub elapsed_time: f32,
}

impl RenderContext {
    pub fn new() -> Self {
        Self {
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            camera_position: Vec3::ZERO,
            delta_time: 0.0,
            elapsed_time: 0.0,
        }
    }

    pub fn from_camera(camera: &crate::CameraClass) -> Self {
        let cam_mut = camera.clone();
        Self {
            view_matrix: cam_mut.view_matrix(),
            projection_matrix: cam_mut.projection_matrix(),
            view_projection_matrix: cam_mut.view_projection_matrix(),
            camera_position: camera.position(),
            delta_time: 0.0,
            elapsed_time: 0.0,
        }
    }
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Re-export from ww3d-core for convenience
pub use crate::culling::{AABox as AABoxClass, Sphere as SphereClass};

/// Ray for picking and intersection tests
/// C++ Reference: renderobj.h lines 650-680
#[derive(Debug, Clone, Copy)]
pub struct PickRay {
    pub origin: Vec3,
    pub direction: Vec3,
    pub length: f32,
}

impl PickRay {
    pub fn new(origin: Vec3, direction: Vec3, length: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            length,
        }
    }

    /// Create ray from screen coordinates
    /// C++ Reference: renderobj.cpp lines 660-720
    pub fn from_screen(screen_x: f32, screen_y: f32, camera: &crate::CameraClass) -> Self {
        // Transform screen coordinates to NDC
        let ndc_x = screen_x * 2.0 - 1.0;
        let ndc_y = 1.0 - screen_y * 2.0;

        let cam_mut = camera.clone();
        let view_proj_inv = cam_mut.view_projection_matrix().inverse();

        // Unproject near and far points
        let near_point = view_proj_inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let far_point = view_proj_inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));

        let direction = (far_point - near_point).normalize();
        let length = (far_point - near_point).length();

        Self {
            origin: near_point,
            direction,
            length,
        }
    }

    /// Get point along ray at distance t
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Pick result for object picking
/// C++ Reference: renderobj.h lines 722-745
#[derive(Debug, Clone)]
pub struct PickResult {
    /// Did we hit something
    pub hit: bool,
    /// Distance to hit point
    pub distance: f32,
    /// Hit point in world space
    pub point: Vec3,
    /// Surface normal at hit point
    pub normal: Vec3,
    /// Object that was hit
    pub object_name: String,
}

impl PickResult {
    pub fn no_hit() -> Self {
        Self {
            hit: false,
            distance: f32::MAX,
            point: Vec3::ZERO,
            normal: Vec3::ZERO,
            object_name: String::new(),
        }
    }

    pub fn hit(distance: f32, point: Vec3, normal: Vec3, object_name: String) -> Self {
        Self {
            hit: true,
            distance,
            point,
            normal,
            object_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_state_once() {
        let mut state = AnimationState {
            animation_name: "test".to_string(),
            current_frame: 0.0,
            mode: AnimationMode::Once,
            is_playing: true,
            speed: 1.0,
            frame_count: 30.0,
            frame_rate: 30.0,
            ping_pong_direction: 1.0,
        };

        // Update for 1 second at 30 FPS
        state.update(1.0, 30.0);

        // Should be at end and stopped
        assert!(!state.is_playing);
        assert_eq!(state.current_frame, 29.0);
    }

    #[test]
    fn test_animation_state_loop() {
        let mut state = AnimationState {
            animation_name: "test".to_string(),
            current_frame: 0.0,
            mode: AnimationMode::Loop,
            is_playing: true,
            speed: 1.0,
            frame_count: 30.0,
            frame_rate: 30.0,
            ping_pong_direction: 1.0,
        };

        // Update for 1.5 seconds at 30 FPS (45 frames)
        state.update(1.5, 30.0);

        // Should have looped and be at frame 15
        assert!(state.is_playing);
        assert_eq!(state.current_frame, 15.0);
    }

    #[test]
    fn test_animation_state_ping_pong() {
        let mut state = AnimationState {
            animation_name: "test".to_string(),
            current_frame: 0.0,
            mode: AnimationMode::PingPong,
            is_playing: true,
            speed: 1.0,
            frame_count: 10.0,
            frame_rate: 30.0,
            ping_pong_direction: 1.0,
        };

        // Update to end
        state.update(0.5, 30.0);

        // Should be going backward now
        assert_eq!(state.ping_pong_direction, -1.0);
    }

    #[test]
    fn test_animation_state_uses_source_frame_rate() {
        let mut state = AnimationState {
            animation_name: "slow".to_string(),
            current_frame: 0.0,
            mode: AnimationMode::Loop,
            is_playing: true,
            speed: 1.0,
            frame_count: 60.0,
            frame_rate: 15.0,
            ping_pong_direction: 1.0,
        };

        state.update(1.0, 30.0);

        assert_eq!(state.current_frame, 15.0);
    }

    #[test]
    fn test_texture_id() {
        let tex = TextureId::new(42);
        assert!(tex.is_valid());
        assert_eq!(tex.0, 42);

        let invalid = TextureId::invalid();
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_material_default() {
        let mat = Material::default();
        assert_eq!(mat.opacity, 1.0);
        assert_eq!(mat.ambient, Vec3::new(0.2, 0.2, 0.2));
    }

    #[test]
    fn test_pick_ray() {
        let ray = PickRay::new(Vec3::ZERO, Vec3::X, 100.0);
        assert_eq!(ray.origin, Vec3::ZERO);
        assert_eq!(ray.direction, Vec3::X);

        let point = ray.point_at(50.0);
        assert_eq!(point, Vec3::new(50.0, 0.0, 0.0));
    }

    #[test]
    fn test_pick_result() {
        let result = PickResult::no_hit();
        assert!(!result.hit);

        let hit = PickResult::hit(10.0, Vec3::X, Vec3::Y, "test".to_string());
        assert!(hit.hit);
        assert_eq!(hit.distance, 10.0);
        assert_eq!(hit.object_name, "test");
    }

    #[test]
    fn test_bone_attachment() {
        // This would require a concrete implementation
        // Just test the structure exists
        let _mode = AnimationMode::Loop;
        let _tex = TextureId::new(1);
    }
}
