//! Animation Evaluator - Bridge between playback controller and HCompressedAnimClass
//!
//! This module bridges AnimationPlayback with the ww3d-animation crate's skeletal
//! animation system (HCompressedAnimClass, HTreeClass). It handles:
//!
//! - Bone transform evaluation at current playback frame
//! - Skeleton hierarchy application
//! - GPU skinning data generation
//! - Animation blending and state management

use glam::{Mat4, Quat, Vec3};
use std::sync::{Arc, Mutex};
use ww3d_animation::{HAnimClass, HCompressedAnimClass, HTreeClass};

/// Result type for animation evaluation
pub type AnimationEvaluatorResult<T> = Result<T, AnimationEvaluatorError>;

/// Error types for animation evaluation
#[derive(Debug, Clone)]
pub enum AnimationEvaluatorError {
    /// Animation data not loaded
    AnimationNotLoaded,
    /// Skeleton hierarchy not available
    SkeletonNotAvailable,
    /// Bone index out of range
    BoneIndexOutOfRange(u32),
    /// Animation evaluation failed
    EvaluationError(String),
    /// GPU skinning data generation failed
    SkinningDataError(String),
}

impl std::fmt::Display for AnimationEvaluatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationEvaluatorError::AnimationNotLoaded => {
                write!(f, "Animation data not loaded")
            }
            AnimationEvaluatorError::SkeletonNotAvailable => {
                write!(f, "Skeleton hierarchy not available")
            }
            AnimationEvaluatorError::BoneIndexOutOfRange(idx) => {
                write!(f, "Bone index {} out of range", idx)
            }
            AnimationEvaluatorError::EvaluationError(msg) => {
                write!(f, "Animation evaluation error: {}", msg)
            }
            AnimationEvaluatorError::SkinningDataError(msg) => {
                write!(f, "GPU skinning data error: {}", msg)
            }
        }
    }
}

impl std::error::Error for AnimationEvaluatorError {}

/// Bone transform data at a specific frame
#[derive(Debug, Clone)]
pub struct BoneTransformData {
    /// Local translation
    pub translation: Vec3,
    /// Local rotation (quaternion)
    pub rotation: Quat,
    /// Local scale (often 1.0)
    pub scale: Vec3,
    /// Visibility flag
    pub visible: bool,
    /// Accumulated world transform (after hierarchy application)
    pub world_transform: Mat4,
}

impl BoneTransformData {
    /// Create identity transform
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visible: true,
            world_transform: Mat4::IDENTITY,
        }
    }

    /// Create local transform matrix
    pub fn local_transform(&self) -> Mat4 {
        Mat4::from_translation(self.translation)
            * Mat4::from_quat(self.rotation)
            * Mat4::from_scale(self.scale)
    }
}

/// GPU skinning data for vertex shader
#[derive(Debug, Clone)]
pub struct GPUSkinningData {
    /// Skinning matrices (bone world * inverse bind) (up to 256 bones)
    pub bone_matrices: Vec<Mat4>,
    /// Inverse bind pose matrices for skinning
    pub inverse_bind_matrices: Vec<Mat4>,
    /// Number of active bones
    pub num_bones: u32,
    /// Maximum bones for GPU (typically 64-256)
    pub max_bones: u32,
}

/// Source animation data for evaluation (compressed or uncompressed).
pub enum AnimationSource {
    Uncompressed(HAnimClass),
    Compressed(Arc<Mutex<HCompressedAnimClass>>),
}

impl GPUSkinningData {
    /// Create new GPU skinning data
    pub fn new(num_bones: u32, max_bones: u32) -> Self {
        let bone_count = std::cmp::min(num_bones, max_bones) as usize;
        Self {
            bone_matrices: vec![Mat4::IDENTITY; bone_count],
            inverse_bind_matrices: vec![Mat4::IDENTITY; bone_count],
            num_bones: bone_count as u32,
            max_bones,
        }
    }

    /// Set bone matrix at index
    pub fn set_bone_matrix(&mut self, index: u32, matrix: Mat4) -> AnimationEvaluatorResult<()> {
        let idx = index as usize;
        if idx >= self.bone_matrices.len() {
            return Err(AnimationEvaluatorError::BoneIndexOutOfRange(index));
        }
        self.bone_matrices[idx] = matrix;
        Ok(())
    }

    /// Get bone matrix at index
    pub fn get_bone_matrix(&self, index: u32) -> AnimationEvaluatorResult<Mat4> {
        let idx = index as usize;
        if idx >= self.bone_matrices.len() {
            return Err(AnimationEvaluatorError::BoneIndexOutOfRange(index));
        }
        Ok(self.bone_matrices[idx])
    }
}

/// Animation evaluator - evaluates skeletal animations at specific frames
pub struct AnimationEvaluator {
    /// Current frame number
    current_frame: u32,
    /// Source animation data
    animation: Option<AnimationSource>,
    /// Skeleton hierarchy (HTree)
    hierarchy: Option<HTreeClass>,
    /// Cached bone transforms at current frame
    bone_transforms: Vec<BoneTransformData>,
    /// GPU skinning data
    skinning_data: GPUSkinningData,
    /// Root transform (world space anchor)
    root_transform: Mat4,
    /// Whether evaluation is dirty (needs update)
    is_dirty: bool,
}

impl AnimationEvaluator {
    /// Create new animation evaluator
    pub fn new(bone_count: u32) -> Self {
        let max_bones = 256;
        let bone_count = std::cmp::min(bone_count, max_bones);

        Self {
            current_frame: 0,
            animation: None,
            hierarchy: None,
            bone_transforms: vec![BoneTransformData::identity(); bone_count as usize],
            skinning_data: GPUSkinningData::new(bone_count, max_bones),
            root_transform: Mat4::IDENTITY,
            is_dirty: true,
        }
    }

    /// Attach an uncompressed animation source.
    pub fn set_uncompressed_animation(&mut self, animation: HAnimClass) {
        self.animation = Some(AnimationSource::Uncompressed(animation));
        self.is_dirty = true;
    }

    /// Attach a compressed animation source.
    pub fn set_compressed_animation(&mut self, animation: Arc<Mutex<HCompressedAnimClass>>) {
        self.animation = Some(AnimationSource::Compressed(animation));
        self.is_dirty = true;
    }

    /// Clear animation source.
    pub fn clear_animation(&mut self) {
        self.animation = None;
        self.is_dirty = true;
    }

    /// Attach skeleton hierarchy and rebuild bind pose data.
    pub fn set_hierarchy(&mut self, mut hierarchy: HTreeClass) {
        hierarchy.base_update(self.root_transform);

        let max_bones = self.skinning_data.max_bones;
        let bone_count = std::cmp::min(hierarchy.num_pivots() as u32, max_bones);

        self.bone_transforms = vec![BoneTransformData::identity(); bone_count as usize];
        self.skinning_data = GPUSkinningData::new(bone_count, max_bones);

        self.skinning_data.inverse_bind_matrices = hierarchy
            .pivots
            .iter()
            .take(bone_count as usize)
            .map(|pivot| pivot.transform.inverse())
            .collect();

        self.hierarchy = Some(hierarchy);
        self.is_dirty = true;
    }

    /// Evaluate animation at frame
    ///
    /// This is a placeholder that would integrate with HCompressedAnimClass.
    /// In the full implementation, this would:
    /// 1. Call HCompressedAnimClass::get_transform(bone_idx, frame) for each bone
    /// 2. Apply hierarchy transforms
    /// 3. Generate GPU skinning matrices
    pub fn evaluate_frame(&mut self, frame_number: u32) -> AnimationEvaluatorResult<()> {
        if self.current_frame == frame_number && !self.is_dirty {
            return Ok(()); // Already evaluated
        }

        let animation = self
            .animation
            .as_ref()
            .ok_or(AnimationEvaluatorError::AnimationNotLoaded)?;
        let hierarchy = self
            .hierarchy
            .as_mut()
            .ok_or(AnimationEvaluatorError::SkeletonNotAvailable)?;

        self.current_frame = frame_number;
        self.is_dirty = false;

        let bone_count = self.bone_transforms.len();
        if bone_count == 0 {
            return Ok(());
        }

        let frame = frame_number as f32;
        let mut translations = vec![Vec3::ZERO; bone_count];
        let mut rotations = vec![Quat::IDENTITY; bone_count];
        let mut visibility = vec![true; bone_count];

        match animation {
            AnimationSource::Uncompressed(hanim) => {
                for i in 0..bone_count {
                    translations[i] = hanim.get_translation(i, frame);
                    rotations[i] = hanim.get_orientation(i, frame);
                    visibility[i] = hanim.get_visibility(i, frame);
                }
            }
            AnimationSource::Compressed(anim) => {
                let mut anim = anim.lock().map_err(|_| {
                    AnimationEvaluatorError::EvaluationError("Animation lock poisoned".to_string())
                })?;
                for i in 0..bone_count {
                    translations[i] = anim.get_translation(i, frame);
                    rotations[i] = anim.get_orientation(i, frame);
                    visibility[i] = anim.get_visibility(i, frame);
                }
            }
        }

        hierarchy.anim_update(self.root_transform, &translations, &rotations);

        for i in 0..bone_count {
            let world_transform = hierarchy.get_transform(i).unwrap_or(Mat4::IDENTITY);
            self.bone_transforms[i] = BoneTransformData {
                translation: translations[i],
                rotation: rotations[i],
                scale: Vec3::ONE,
                visible: visibility[i],
                world_transform,
            };

            if i < self.skinning_data.bone_matrices.len()
                && i < self.skinning_data.inverse_bind_matrices.len()
            {
                self.skinning_data.bone_matrices[i] =
                    world_transform * self.skinning_data.inverse_bind_matrices[i];
            }
        }

        Ok(())
    }

    /// Get bone transform at current frame
    pub fn get_bone_transform(
        &self,
        bone_index: u32,
    ) -> AnimationEvaluatorResult<&BoneTransformData> {
        let idx = bone_index as usize;
        if idx >= self.bone_transforms.len() {
            return Err(AnimationEvaluatorError::BoneIndexOutOfRange(bone_index));
        }
        Ok(&self.bone_transforms[idx])
    }

    /// Get all bone transforms
    pub fn get_all_transforms(&self) -> &[BoneTransformData] {
        &self.bone_transforms
    }

    /// Get GPU skinning data
    pub fn get_skinning_data(&self) -> &GPUSkinningData {
        &self.skinning_data
    }

    /// Set root transform
    pub fn set_root_transform(&mut self, transform: Mat4) {
        self.root_transform = transform;
        self.is_dirty = true;
    }

    /// Mark evaluator as needing update
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Get current frame number
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Update from animation (placeholder)
    ///
    /// This would be called with HCompressedAnimClass reference:
    /// pub fn update_from_animation(
    ///     &mut self,
    ///     animation: &mut HCompressedAnimClass,
    ///     frame: f32,
    /// ) -> AnimationEvaluatorResult<()>
    pub fn update_placeholder(&mut self, frame: u32) -> AnimationEvaluatorResult<()> {
        self.evaluate_frame(frame)
    }
}

impl Default for AnimationEvaluator {
    fn default() -> Self {
        Self::new(64) // Default 64 bones
    }
}

/// Animation blend state manager
pub struct AnimationBlender {
    /// Primary animation
    primary_animation: Option<AnimationEvaluator>,
    /// Secondary animation for blending
    secondary_animation: Option<AnimationEvaluator>,
    /// Blend factor (0.0 = primary only, 1.0 = secondary only)
    blend_factor: f32,
    /// Blend mode
    blend_mode: BlendMode,
}

/// Blend modes for combining animations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Replace: Use only primary or secondary based on factor
    Replace,
    /// Additive: Add primary + (secondary * factor)
    Additive,
    /// Multiplicative: primary * (secondary * factor)
    Multiplicative,
    /// Lerp: Linear interpolation
    Lerp,
}

impl AnimationBlender {
    /// Create new animation blender
    pub fn new(bone_count: u32) -> Self {
        Self {
            primary_animation: Some(AnimationEvaluator::new(bone_count)),
            secondary_animation: None,
            blend_factor: 0.0,
            blend_mode: BlendMode::Lerp,
        }
    }

    /// Set primary animation
    pub fn set_primary(&mut self, evaluator: AnimationEvaluator) {
        self.primary_animation = Some(evaluator);
    }

    /// Set secondary animation for blending
    pub fn set_secondary(&mut self, evaluator: AnimationEvaluator) {
        self.secondary_animation = Some(evaluator);
    }

    /// Set blend factor (0.0 - 1.0)
    pub fn set_blend_factor(&mut self, factor: f32) {
        self.blend_factor = factor.clamp(0.0, 1.0);
    }

    /// Set blend mode
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    /// Get blended result
    pub fn get_blended_transforms(&self) -> AnimationEvaluatorResult<Vec<BoneTransformData>> {
        let primary = self
            .primary_animation
            .as_ref()
            .ok_or(AnimationEvaluatorError::AnimationNotLoaded)?;

        let primary_transforms = primary.get_all_transforms();

        match (&self.secondary_animation, self.blend_factor) {
            (None, _) | (_, 0.0) => {
                // No blending, use primary only
                Ok(primary_transforms.to_vec())
            }
            (Some(secondary), factor) => {
                let secondary_transforms = secondary.get_all_transforms();

                if primary_transforms.len() != secondary_transforms.len() {
                    return Err(AnimationEvaluatorError::EvaluationError(
                        "Primary and secondary animations have different bone counts".to_string(),
                    ));
                }

                let blended: Vec<BoneTransformData> = primary_transforms
                    .iter()
                    .zip(secondary_transforms.iter())
                    .map(|(p, s)| Self::blend_transforms(p, s, factor, self.blend_mode))
                    .collect();

                Ok(blended)
            }
        }
    }

    /// Blend two transforms
    fn blend_transforms(
        primary: &BoneTransformData,
        secondary: &BoneTransformData,
        factor: f32,
        mode: BlendMode,
    ) -> BoneTransformData {
        match mode {
            BlendMode::Replace => {
                if factor > 0.5 {
                    secondary.clone()
                } else {
                    primary.clone()
                }
            }
            BlendMode::Lerp => {
                BoneTransformData {
                    translation: primary.translation.lerp(secondary.translation, factor),
                    rotation: primary.rotation.slerp(secondary.rotation, factor),
                    scale: primary.scale.lerp(secondary.scale, factor),
                    visible: if factor > 0.5 {
                        secondary.visible
                    } else {
                        primary.visible
                    },
                    // For matrix, linearly interpolate components
                    world_transform: Mat4::from_translation(
                        Vec3::new(
                            primary.world_transform.w_axis.x,
                            primary.world_transform.w_axis.y,
                            primary.world_transform.w_axis.z,
                        )
                        .lerp(
                            Vec3::new(
                                secondary.world_transform.w_axis.x,
                                secondary.world_transform.w_axis.y,
                                secondary.world_transform.w_axis.z,
                            ),
                            factor,
                        ),
                    ),
                }
            }
            BlendMode::Additive => {
                BoneTransformData {
                    translation: primary.translation + (secondary.translation * factor),
                    rotation: primary.rotation * secondary.rotation, // Multiplicative for rotations
                    scale: Vec3::ONE,
                    visible: primary.visible && secondary.visible,
                    world_transform: primary.world_transform,
                }
            }
            BlendMode::Multiplicative => BoneTransformData {
                translation: primary.translation * (1.0 + (secondary.translation * factor)),
                rotation: primary.rotation * secondary.rotation,
                scale: primary.scale * secondary.scale.lerp(Vec3::ONE, 1.0 - factor),
                visible: primary.visible && secondary.visible,
                world_transform: primary.world_transform,
            },
        }
    }
}

impl Default for AnimationBlender {
    fn default() -> Self {
        Self::new(64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_evaluator_creation() {
        let evaluator = AnimationEvaluator::new(64);
        assert_eq!(evaluator.get_all_transforms().len(), 64);
    }

    #[test]
    fn test_bone_transform_data_identity() {
        let transform = BoneTransformData::identity();
        assert_eq!(transform.translation, Vec3::ZERO);
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(transform.scale, Vec3::ONE);
        assert!(transform.visible);
    }

    #[test]
    fn test_gpu_skinning_data() {
        let mut skinning = GPUSkinningData::new(64, 256);
        assert_eq!(skinning.num_bones, 64);

        let matrix = Mat4::IDENTITY;
        assert!(skinning.set_bone_matrix(0, matrix).is_ok());
        assert!(skinning.get_bone_matrix(0).is_ok());
        assert!(skinning.get_bone_matrix(999).is_err());
    }

    #[test]
    fn test_animation_blender() {
        let blender = AnimationBlender::new(64);
        assert!(blender.primary_animation.is_some());
        assert!(blender.secondary_animation.is_none());
    }

    #[test]
    fn test_blend_factor_clamping() {
        let mut blender = AnimationBlender::new(64);
        blender.set_blend_factor(1.5);
        assert_eq!(blender.blend_factor, 1.0);

        blender.set_blend_factor(-0.5);
        assert_eq!(blender.blend_factor, 0.0);
    }

    #[test]
    fn test_error_display() {
        let err = AnimationEvaluatorError::BoneIndexOutOfRange(999);
        assert_eq!(format!("{}", err), "Bone index 999 out of range");
    }
}
