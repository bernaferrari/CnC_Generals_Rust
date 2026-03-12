///! Skeletal Animation System with Bone Transforms
///!
///! This module implements the complete skeletal animation system including:
///! - Bone transformation hierarchies
///! - Skinned mesh rendering with GPU bone matrices
///! - Animation playback with frame interpolation
///! - Multiple animation layers and blending
///!
///! Reference: C++ htree.cpp, hanim.cpp, mesh.cpp skinning code

use crate::hanim::{HAnimClass, AnimationMode};
use crate::htree::HTreeClass;
use glam::{Mat4, Quat, Vec3};

/// Maximum bones supported in a single model
/// This matches typical GPU uniform buffer limits (256 * 16 floats * 4 bytes = 16KB)
pub const MAX_BONES: usize = 256;

/// Skeleton state with current bone transforms
/// C++ Reference: htree.cpp::HTreeClass transform management
#[derive(Debug, Clone)]
pub struct SkeletonState {
    /// The hierarchy tree defining bone structure
    pub htree: HTreeClass,
    /// Current world-space bone transforms (one per bone)
    pub bone_transforms: Vec<Mat4>,
    /// Bind pose inverse matrices (for skinning)
    pub inverse_bind_matrices: Vec<Mat4>,
    /// Visibility flags per bone
    pub bone_visibility: Vec<bool>,
}

impl SkeletonState {
    /// Create a new skeleton state from a hierarchy
    /// C++ Reference: HTreeClass::Base_Update()
    pub fn new(htree: HTreeClass) -> Self {
        let num_bones = htree.num_pivots();
        let mut bone_transforms = vec![Mat4::IDENTITY; num_bones];
        let mut inverse_bind_matrices = vec![Mat4::IDENTITY; num_bones];
        let bone_visibility = vec![true; num_bones];

        // Initialize with base pose transforms
        for i in 0..num_bones {
            if let Some(pivot) = htree.get_pivot(i) {
                bone_transforms[i] = pivot.transform;
                // Inverse bind matrix is the inverse of the base pose transform
                inverse_bind_matrices[i] = pivot.transform.inverse();
            }
        }

        Self {
            htree,
            bone_transforms,
            inverse_bind_matrices,
            bone_visibility,
        }
    }

    /// Update skeleton to base pose (no animation)
    /// C++ Reference: HTreeClass::Base_Update()
    pub fn reset_to_base_pose(&mut self, root_transform: Mat4) {
        self.htree.base_update(root_transform);
        self.update_bone_transforms();
    }

    /// Apply animation at a specific frame
    /// C++ Reference: HTreeClass::Anim_Update()
    pub fn apply_animation(&mut self, root_transform: Mat4, animation: &HAnimClass, frame: f32) {
        let num_bones = self.htree.num_pivots();
        let mut translations = vec![Vec3::ZERO; num_bones];
        let mut rotations = vec![Quat::IDENTITY; num_bones];

        // Sample animation channels for each bone
        for i in 0..num_bones {
            translations[i] = animation.get_translation(i, frame);
            rotations[i] = animation.get_orientation(i, frame);
            self.bone_visibility[i] = animation.get_visibility(i, frame);
        }

        // Update hierarchy with animation data
        self.htree.anim_update(root_transform, &translations, &rotations);
        self.update_bone_transforms();
    }

    /// Blend between two animations
    /// C++ Reference: HTreeClass::Blend_Update()
    pub fn blend_animations(
        &mut self,
        root_transform: Mat4,
        anim0: &HAnimClass,
        frame0: f32,
        anim1: &HAnimClass,
        frame1: f32,
        blend_factor: f32,
    ) {
        let num_bones = self.htree.num_pivots();
        let mut translations = vec![Vec3::ZERO; num_bones];
        let mut rotations = vec![Quat::IDENTITY; num_bones];

        // Sample both animations
        for i in 0..num_bones {
            let trans0 = anim0.get_translation(i, frame0);
            let trans1 = anim1.get_translation(i, frame1);
            translations[i] = trans0.lerp(trans1, blend_factor);

            let rot0 = anim0.get_orientation(i, frame0);
            let rot1 = anim1.get_orientation(i, frame1);
            rotations[i] = rot0.slerp(rot1, blend_factor);

            // Use visibility from the animation with higher blend factor
            self.bone_visibility[i] = if blend_factor < 0.5 {
                anim0.get_visibility(i, frame0)
            } else {
                anim1.get_visibility(i, frame1)
            };
        }

        self.htree.anim_update(root_transform, &translations, &rotations);
        self.update_bone_transforms();
    }

    /// Update bone transforms from hierarchy state
    fn update_bone_transforms(&mut self) {
        for i in 0..self.htree.num_pivots() {
            if let Some(transform) = self.htree.get_transform(i) {
                self.bone_transforms[i] = transform;
            }
        }
    }

    /// Get skinning matrices for GPU upload
    /// Returns matrix that transforms from mesh space to bone space
    /// C++ Reference: mesh.cpp skinning matrix computation
    pub fn get_skinning_matrices(&self) -> Vec<Mat4> {
        self.bone_transforms
            .iter()
            .zip(self.inverse_bind_matrices.iter())
            .map(|(bone_transform, inverse_bind)| {
                // Skinning matrix = bone_transform * inverse_bind_pose
                *bone_transform * *inverse_bind
            })
            .collect()
    }

    /// Get skinning matrices as flat f32 array for GPU
    pub fn get_skinning_matrices_flat(&self) -> Vec<f32> {
        let matrices = self.get_skinning_matrices();
        let mut flat = Vec::with_capacity(matrices.len() * 16);
        for matrix in matrices {
            flat.extend_from_slice(&matrix.to_cols_array());
        }
        flat
    }

    /// Get number of bones
    pub fn bone_count(&self) -> usize {
        self.htree.num_pivots()
    }

    /// Get bone world transform
    pub fn get_bone_transform(&self, bone_idx: usize) -> Option<Mat4> {
        self.bone_transforms.get(bone_idx).copied()
    }

    /// Get bone visibility
    pub fn is_bone_visible(&self, bone_idx: usize) -> bool {
        self.bone_visibility.get(bone_idx).copied().unwrap_or(true)
    }
}

/// Animated model instance with playback state
/// Manages animation playback, blending, and skeleton updates
#[derive(Debug)]
pub struct AnimatedModel {
    /// Skeleton state
    pub skeleton: SkeletonState,
    /// Current animation
    current_animation: Option<HAnimClass>,
    /// Target animation for blending
    target_animation: Option<HAnimClass>,
    /// Current frame
    current_frame: f32,
    /// Target frame for blending
    target_frame: f32,
    /// Blend factor (0.0 = current, 1.0 = target)
    blend_factor: f32,
    /// Blend speed (factor increase per second)
    blend_speed: f32,
    /// Animation playback mode
    mode: AnimationMode,
    /// Playback speed multiplier
    speed: f32,
    /// Is animation paused
    paused: bool,
    /// Ping-pong direction for current animation (true = forward)
    current_ping_pong_forward: bool,
    /// Ping-pong direction for target animation (true = forward)
    target_ping_pong_forward: bool,
}

impl AnimatedModel {
    /// Create a new animated model from a hierarchy
    pub fn new(htree: HTreeClass) -> Self {
        Self {
            skeleton: SkeletonState::new(htree),
            current_animation: None,
            target_animation: None,
            current_frame: 0.0,
            target_frame: 0.0,
            blend_factor: 1.0,
            blend_speed: 5.0, // Default: blend completes in 0.2 seconds
            mode: AnimationMode::Loop,
            speed: 1.0,
            paused: false,
            current_ping_pong_forward: true,
            target_ping_pong_forward: true,
        }
    }

    /// Set current animation immediately (no blending)
    pub fn set_animation(&mut self, animation: HAnimClass) {
        self.current_animation = Some(animation);
        self.target_animation = None;
        self.current_frame = 0.0;
        self.blend_factor = 1.0;
        self.current_ping_pong_forward = true;
    }

    /// Transition to a new animation with blending
    pub fn transition_to(&mut self, animation: HAnimClass, blend_duration: f32) {
        if let Some(current) = self.current_animation.take() {
            self.target_animation = Some(animation);
            self.current_animation = Some(current);
            self.target_frame = 0.0;
            self.blend_factor = 0.0;
            self.blend_speed = if blend_duration > 0.0 {
                1.0 / blend_duration
            } else {
                f32::INFINITY
            };
            self.target_ping_pong_forward = true;
        } else {
            // No current animation, just set it
            self.set_animation(animation);
        }
    }

    /// Update animation state
    pub fn update(&mut self, delta_time: f32, root_transform: Mat4) {
        if self.paused {
            return;
        }

        // Handle blend transition
        if self.target_animation.is_some() {
            self.blend_factor += self.blend_speed * delta_time;
            if self.blend_factor >= 1.0 {
                // Blend complete, swap animations
                self.current_animation = self.target_animation.take();
                self.current_frame = self.target_frame;
                self.blend_factor = 1.0;
            }
        }

        // Advance animation frames
        let mode = self.mode;
        let speed = self.speed;
        if let Some(ref anim) = self.current_animation {
            self.current_frame = Self::advance_frame(
                self.current_frame,
                anim,
                delta_time,
                speed,
                mode,
                &mut self.current_ping_pong_forward,
            );
        }

        if let Some(ref target_anim) = self.target_animation {
            self.target_frame = Self::advance_frame(
                self.target_frame,
                target_anim,
                delta_time,
                speed,
                mode,
                &mut self.target_ping_pong_forward,
            );
        }

        // Update skeleton
        self.update_skeleton(root_transform);
    }

    /// Advance animation frame based on mode
    /// C++ Reference: RenderObjClass animation frame update
    fn advance_frame(
        frame: f32,
        animation: &HAnimClass,
        delta_time: f32,
        speed: f32,
        mode: AnimationMode,
        ping_pong_forward: &mut bool,
    ) -> f32 {
        let num_frames = animation.get_num_frames() as f32;
        let frame_rate = animation.get_frame_rate();
        let delta_frames = delta_time * frame_rate * speed;

        match mode {
            AnimationMode::Manual => frame, // Don't auto-advance
            AnimationMode::Loop => {
                let new_frame = frame + delta_frames;
                if new_frame >= num_frames {
                    new_frame % num_frames
                } else {
                    new_frame
                }
            }
            AnimationMode::Once => {
                let new_frame = frame + delta_frames;
                new_frame.min(num_frames - 1.0)
            }
            AnimationMode::PingPong => {
                if num_frames <= 1.0 {
                    return 0.0;
                }

                let max_frame = num_frames - 1.0;
                let period = 2.0 * max_frame;
                let direction_delta = if *ping_pong_forward {
                    delta_frames
                } else {
                    -delta_frames
                };
                let mut pos = frame + direction_delta;
                pos = pos % period;
                if pos < 0.0 {
                    pos += period;
                }

                if pos <= max_frame {
                    *ping_pong_forward = true;
                    pos
                } else {
                    *ping_pong_forward = false;
                    period - pos
                }
            }
            AnimationMode::LoopBackwards => {
                let new_frame = frame - delta_frames;
                if new_frame < 0.0 {
                    num_frames + (new_frame % num_frames)
                } else {
                    new_frame
                }
            }
            AnimationMode::OnceBackwards => {
                let new_frame = frame - delta_frames;
                new_frame.max(0.0)
            }
        }
    }

    /// Update skeleton with current animation state
    fn update_skeleton(&mut self, root_transform: Mat4) {
        if let Some(ref target_anim) = self.target_animation {
            // Blending between animations
            if let Some(ref current_anim) = self.current_animation {
                self.skeleton.blend_animations(
                    root_transform,
                    current_anim,
                    self.current_frame,
                    target_anim,
                    self.target_frame,
                    self.blend_factor,
                );
            }
        } else if let Some(ref anim) = self.current_animation {
            // Single animation
            self.skeleton
                .apply_animation(root_transform, anim, self.current_frame);
        } else {
            // No animation, use base pose
            self.skeleton.reset_to_base_pose(root_transform);
        }
    }

    /// Get skinning matrices for rendering
    pub fn get_skinning_matrices(&self) -> Vec<Mat4> {
        self.skeleton.get_skinning_matrices()
    }

    /// Get skinning matrices as flat array for GPU
    pub fn get_skinning_matrices_flat(&self) -> Vec<f32> {
        self.skeleton.get_skinning_matrices_flat()
    }

    /// Set animation mode
    pub fn set_animation_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
    }

    /// Set playback speed
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.0);
    }

    /// Pause animation
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume animation
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if animation is playing
    pub fn is_playing(&self) -> bool {
        !self.paused && self.current_animation.is_some()
    }

    /// Get current frame
    pub fn get_current_frame(&self) -> f32 {
        self.current_frame
    }

    /// Set current frame manually
    pub fn set_frame(&mut self, frame: f32) {
        self.current_frame = frame;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeleton_creation() {
        let htree = HTreeClass::new();
        let skeleton = SkeletonState::new(htree);
        assert_eq!(skeleton.bone_count(), 0);
    }

    #[test]
    fn test_animated_model() {
        let mut htree = HTreeClass::new();
        htree.init_default();
        let model = AnimatedModel::new(htree);
        assert!(!model.is_playing());
        assert_eq!(model.skeleton.bone_count(), 1); // Root bone
    }

    #[test]
    fn test_skinning_matrices() {
        let mut htree = HTreeClass::new();
        htree.init_default();
        let skeleton = SkeletonState::new(htree);
        let matrices = skeleton.get_skinning_matrices();
        assert_eq!(matrices.len(), 1); // One bone (root)
    }

    #[test]
    fn test_bone_visibility() {
        let mut htree = HTreeClass::new();
        htree.init_default();
        let skeleton = SkeletonState::new(htree);
        assert!(skeleton.is_bone_visible(0));
    }
}
