//! W3D Bone Hierarchy — delegates to ww3d-animation's HTreeClass.
//!
//! This module provides the GameClient-facing skeleton type used by the
//! rendering pipeline. It wraps `HTreeClass` from the `ww3d-animation`
//! crate and converts bone transforms to the `cgmath` matrices expected
//! by the rest of the GameClient W3D layer.
//!
//! C++ Reference: HTreeClass in htree.h / htree.cpp

use cgmath::Matrix4;
use glam::{Mat4, Quat, Vec3};
use ww3d_animation::HTreeClass;

/// Maximum bones supported by the GPU shader (BoneUniform array size).
pub const MAX_GPU_BONES: usize = 64;

/// Bone hierarchy populated from W3D pivot data, delegating to `HTreeClass`.
///
/// This replaces the previous hardcoded 8-bone skeleton with a proper
/// hierarchy that can be populated from loaded W3D files.
#[derive(Debug, Clone)]
pub struct W3DHTree {
    inner: HTreeClass,
}

impl W3DHTree {
    /// Create an empty hierarchy (no bones).
    pub fn new() -> Self {
        Self {
            inner: HTreeClass::new(),
        }
    }

    /// Create a hierarchy with just a root transform.
    pub fn with_root() -> Self {
        let mut inner = HTreeClass::new();
        inner.init_default();
        Self { inner }
    }

    /// Build a hierarchy from an existing `HTreeClass` (e.g. loaded from W3D).
    pub fn from_htree_class(htree: HTreeClass) -> Self {
        Self { inner: htree }
    }

    /// Access the underlying `HTreeClass` for animation evaluation.
    pub fn inner(&self) -> &HTreeClass {
        &self.inner
    }

    /// Access the underlying `HTreeClass` mutably for animation updates.
    pub fn inner_mut(&mut self) -> &mut HTreeClass {
        &mut self.inner
    }

    /// Add a bone from translation + rotation.
    pub fn add_bone(&mut self, name: &str, parent_idx: i32, translation: Vec3, rotation: Quat) {
        self.inner.add_pivot(name, parent_idx, translation, rotation);
    }

    /// Add a bone providing the base transform directly.
    pub fn add_bone_from_base(&mut self, name: &str, parent_idx: i32, base_transform: Mat4) {
        self.inner.add_pivot_from_base(name, parent_idx, base_transform);
    }

    /// Get number of bones in the hierarchy.
    pub fn bone_count(&self) -> usize {
        self.inner.num_pivots()
    }

    /// Look up a bone index by name (case-insensitive).
    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.inner.find_pivot_index(name)
    }

    /// Get the bone name at the given index.
    pub fn get_bone_name(&self, index: usize) -> Option<&str> {
        self.inner.get_bone_name(index)
    }

    /// Compute the base pose transforms (no animation).
    /// C++ Reference: HTreeClass::Base_Update
    pub fn base_update(&mut self) {
        self.inner.base_update(Mat4::IDENTITY);
    }

    /// Capture a bone for external control (turret, recoil, etc.).
    pub fn capture_bone(&mut self, index: usize) {
        self.inner.capture_bone(index);
    }

    /// Release a captured bone.
    pub fn release_bone(&mut self, index: usize) {
        self.inner.release_bone(index);
    }

    /// Control a captured bone with a custom transform.
    pub fn control_bone(&mut self, index: usize, relative_tm: Mat4) {
        self.inner.control_bone(index, relative_tm);
    }

    /// Check whether a bone is captured.
    pub fn is_bone_captured(&self, index: usize) -> bool {
        self.inner.is_bone_captured(index)
    }

    /// Retrieve the final bone transform as a `glam::Mat4`.
    pub fn get_bone_transform_glam(&self, index: usize) -> Mat4 {
        self.inner.get_transform(index).unwrap_or(Mat4::IDENTITY)
    }

    /// Retrieve the final bone transform as a `cgmath::Matrix4<f32>`.
    /// For compatibility with the rest of the GameClient W3D layer.
    pub fn get_bone_transform(&self, index: usize) -> Matrix4<f32> {
        glam_to_cgmath(self.get_bone_transform_glam(index))
    }

    /// Convert all bone transforms to a flat `f32` array suitable for
    /// WGPU uniform buffer upload. Each bone produces 16 f32 values
    /// (4x4 column-major matrix). Pads to `MAX_GPU_BONES`.
    pub fn to_uniform_data(&self) -> Vec<f32> {
        let num_bones = self.inner.num_pivots();
        let mut data = Vec::with_capacity(MAX_GPU_BONES * 16);

        for i in 0..num_bones.min(MAX_GPU_BONES) {
            let m = self.inner.get_transform(i).unwrap_or(Mat4::IDENTITY);
            data.extend_from_slice(&m.to_cols_array());
        }

        // Pad remaining slots with identity matrices
        let identity_cols = Mat4::IDENTITY.to_cols_array();
        while data.len() < MAX_GPU_BONES * 16 {
            data.extend_from_slice(&identity_cols);
        }

        data
    }

    /// Build skinning matrices (bone_transform * inverse_bind_pose)
    /// using the provided inverse bind matrices.
    /// Returns flat f32 array padded to MAX_GPU_BONES.
    pub fn to_skinning_uniform(&self, inverse_bind_matrices: &[Mat4]) -> Vec<f32> {
        let num_bones = self.inner.num_pivots().min(MAX_GPU_BONES);
        let mut data = Vec::with_capacity(MAX_GPU_BONES * 16);

        for i in 0..num_bones {
            let bone_tm = self.inner.get_transform(i).unwrap_or(Mat4::IDENTITY);
            let inv_bind = inverse_bind_matrices.get(i).copied().unwrap_or(Mat4::IDENTITY);
            let skinning = bone_tm * inv_bind;
            data.extend_from_slice(&skinning.to_cols_array());
        }

        let identity_cols = Mat4::IDENTITY.to_cols_array();
        while data.len() < MAX_GPU_BONES * 16 {
            data.extend_from_slice(&identity_cols);
        }

        data
    }

    /// Expand the bone array to hold at least `count` bones.
    pub fn ensure_bone_count(&mut self, count: usize) {
        while self.inner.num_pivots() < count {
            let idx = self.inner.num_pivots();
            self.inner.add_pivot_from_base(
                &format!("bone{idx}"),
                if idx == 0 { -1 } else { 0 },
                Mat4::IDENTITY,
            );
        }
    }
}

impl Default for W3DHTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a `glam::Mat4` to a `cgmath::Matrix4<f32>`.
fn glam_to_cgmath(m: Mat4) -> Matrix4<f32> {
    let cols = m.to_cols_array();
    // cgmath Matrix4 uses column-major storage in `x`, `y`, `z`, `w` fields
    // where each field is a Vector4 (column).
    Matrix4::new(
        cols[0], cols[1], cols[2], cols[3],   // col 0
        cols[4], cols[5], cols[6], cols[7],   // col 1
        cols[8], cols[9], cols[10], cols[11], // col 2
        cols[12], cols[13], cols[14], cols[15], // col 3
    )
}
