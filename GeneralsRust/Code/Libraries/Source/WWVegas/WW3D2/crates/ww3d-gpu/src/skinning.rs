//! GPU Skinning System
//!
//! This module implements skeletal animation skinning on the GPU using WGSL shaders.
//! Port of skinning.cpp (lines 80-150) shader generation logic.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;

/// Maximum bones supported in a single skinning operation
pub const MAX_SKINNING_BONES: usize = 128;

/// Maximum bone influences per vertex
pub const MAX_BONE_INFLUENCES: usize = 4;

/// Bone matrix for skinning
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoneMatrix {
    pub matrix: [[f32; 4]; 4],
}

impl From<Mat4> for BoneMatrix {
    fn from(mat: Mat4) -> Self {
        Self {
            matrix: mat.to_cols_array_2d(),
        }
    }
}

/// Bone palette for GPU skinning
/// Port of skinning.cpp BonePalette (lines 80-100)
#[repr(C)]
#[derive(Debug, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BonePalette {
    /// Array of bone matrices
    pub bones: [BoneMatrix; MAX_SKINNING_BONES],
    /// Number of active bones
    pub bone_count: u32,
    /// Padding for alignment
    pub _padding: [u32; 3],
}

impl BonePalette {
    /// Create new empty bone palette
    pub fn new() -> Self {
        Self {
            bones: [BoneMatrix {
                matrix: Mat4::IDENTITY.to_cols_array_2d(),
            }; MAX_SKINNING_BONES],
            bone_count: 0,
            _padding: [0; 3],
        }
    }

    /// Set bone matrix
    pub fn set_bone(&mut self, index: usize, matrix: Mat4) {
        if index < MAX_SKINNING_BONES {
            self.bones[index] = matrix.into();
            self.bone_count = self.bone_count.max((index + 1) as u32);
        }
    }

    /// Get bone matrix
    pub fn get_bone(&self, index: usize) -> Option<Mat4> {
        if index < self.bone_count as usize {
            let mat = self.bones[index].matrix;
            Some(Mat4::from_cols_array_2d(&mat))
        } else {
            None
        }
    }

    /// Clear all bones
    pub fn clear(&mut self) {
        self.bone_count = 0;
    }
}

impl Default for BonePalette {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex bone influence data
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexBoneInfluence {
    /// Bone indices (up to MAX_BONE_INFLUENCES)
    pub bone_indices: [u32; MAX_BONE_INFLUENCES],
    /// Bone weights (should sum to 1.0)
    pub bone_weights: [f32; MAX_BONE_INFLUENCES],
}

impl VertexBoneInfluence {
    /// Create new influence with no bones
    pub fn new() -> Self {
        Self {
            bone_indices: [0; MAX_BONE_INFLUENCES],
            bone_weights: [0.0; MAX_BONE_INFLUENCES],
        }
    }

    /// Create from single bone
    pub fn from_single_bone(bone_index: u32) -> Self {
        Self {
            bone_indices: [bone_index, 0, 0, 0],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Validate weights sum to 1.0
    pub fn validate(&self) -> Result<(), String> {
        let sum: f32 = self.bone_weights.iter().sum();
        if (sum - 1.0).abs() > 0.01 {
            Err(format!("Bone weights sum to {}, expected 1.0", sum))
        } else {
            Ok(())
        }
    }

    /// Normalize weights to sum to 1.0
    pub fn normalize(&mut self) {
        let sum: f32 = self.bone_weights.iter().sum();
        if sum > 0.0 {
            for weight in &mut self.bone_weights {
                *weight /= sum;
            }
        }
    }

    /// Get active influence count
    pub fn active_count(&self) -> usize {
        self.bone_weights
            .iter()
            .take_while(|&&w| w > 0.0)
            .count()
    }
}

impl Default for VertexBoneInfluence {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU Skinning Shader Generator
/// Port of skinning.cpp shader generation (lines 80-150)
pub struct SkinningShaderGenerator;

impl SkinningShaderGenerator {
    /// Generate complete WGSL skinning vertex shader
    /// Port of skinning.cpp DirectX shader generation (lines 80-150)
    pub fn generate_skinning_shader(max_bones: usize) -> String {
        format!(
            r#"// GPU Skinning Shader
// Generated for max {} bones

struct BoneMatrices {{
    bones: array<mat4x4<f32>, {max_bones}>,
}}

@group(1) @binding(0)
var<uniform> bone_matrices: BoneMatrices;

struct VertexInput {{
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) uv0: vec2<f32>,
    @location(4) bone_indices: vec4<u32>,
    @location(5) bone_weights: vec4<f32>,
}}

struct VertexOutput {{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) uv0: vec2<f32>,
}}

struct CameraUniforms {{
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_position: vec3<f32>,
}}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct ModelUniforms {{
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}}

@group(2) @binding(0)
var<uniform> model_uniforms: ModelUniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {{
    var out: VertexOutput;

    // Perform GPU skinning
    var skinned_pos = vec3<f32>(0.0, 0.0, 0.0);
    var skinned_normal = vec3<f32>(0.0, 0.0, 0.0);
    var skinned_tangent = vec3<f32>(0.0, 0.0, 0.0);

    // Apply bone influences (up to 4 bones per vertex)
    for (var i = 0u; i < 4u; i = i + 1u) {{
        let bone_index = in.bone_indices[i];
        let bone_weight = in.bone_weights[i];

        if (bone_weight > 0.0 && bone_index < {max_bones}u) {{
            let bone_matrix = bone_matrices.bones[bone_index];

            // Transform position
            let transformed_pos = (bone_matrix * vec4<f32>(in.position, 1.0)).xyz;
            skinned_pos = skinned_pos + transformed_pos * bone_weight;

            // Transform normal (w=0 for direction vectors)
            let transformed_normal = (bone_matrix * vec4<f32>(in.normal, 0.0)).xyz;
            skinned_normal = skinned_normal + transformed_normal * bone_weight;

            // Transform tangent
            let transformed_tangent = (bone_matrix * vec4<f32>(in.tangent, 0.0)).xyz;
            skinned_tangent = skinned_tangent + transformed_tangent * bone_weight;
        }}
    }}

    // Normalize the skinned normal and tangent
    skinned_normal = normalize(skinned_normal);
    skinned_tangent = normalize(skinned_tangent);

    // Transform to world space
    let world_pos = (model_uniforms.model * vec4<f32>(skinned_pos, 1.0)).xyz;
    let world_normal = (model_uniforms.normal_matrix * vec4<f32>(skinned_normal, 0.0)).xyz;
    let world_tangent = (model_uniforms.normal_matrix * vec4<f32>(skinned_tangent, 0.0)).xyz;

    // Transform to clip space
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.world_normal = normalize(world_normal);
    out.world_tangent = normalize(world_tangent);
    out.uv0 = in.uv0;

    return out;
}}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{
    // Simple lighting for visualization
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = max(dot(in.world_normal, light_dir), 0.0);
    let ambient = 0.2;
    let lighting = ambient + (1.0 - ambient) * ndotl;

    return vec4<f32>(lighting, lighting, lighting, 1.0);
}}
"#,
            max_bones = max_bones
        )
    }

    /// Generate skinning shader without tangents (simpler version)
    pub fn generate_simple_skinning_shader(max_bones: usize) -> String {
        format!(
            r#"// Simple GPU Skinning Shader
// Generated for max {} bones

struct BoneMatrices {{
    bones: array<mat4x4<f32>, {max_bones}>,
}}

@group(1) @binding(0)
var<uniform> bone_matrices: BoneMatrices;

struct VertexInput {{
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) bone_indices: vec4<u32>,
    @location(4) bone_weights: vec4<f32>,
}}

struct VertexOutput {{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
}}

struct Uniforms {{
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
}}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {{
    var out: VertexOutput;

    // Perform GPU skinning
    var skinned_pos = vec3<f32>(0.0);
    var skinned_normal = vec3<f32>(0.0);

    for (var i = 0u; i < 4u; i++) {{
        let bone_index = in.bone_indices[i];
        let bone_weight = in.bone_weights[i];

        if (bone_weight > 0.0) {{
            let bone_matrix = bone_matrices.bones[bone_index];
            skinned_pos += (bone_matrix * vec4<f32>(in.position, 1.0)).xyz * bone_weight;
            skinned_normal += (bone_matrix * vec4<f32>(in.normal, 0.0)).xyz * bone_weight;
        }}
    }}

    // Transform to clip space
    let world_pos = (uniforms.model * vec4<f32>(skinned_pos, 1.0)).xyz;
    out.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.world_normal = normalize(skinned_normal);
    out.uv0 = in.uv0;

    return out;
}}
"#,
            max_bones = max_bones
        )
    }
}

/// CPU fallback for skinning (when GPU not available)
/// Port of skinning.cpp CPU skinning (lines 200-250)
pub struct CpuSkinning;

impl CpuSkinning {
    /// Apply bone transforms on CPU
    pub fn apply_bone_transforms(
        position: Vec3,
        normal: Vec3,
        bone_palette: &BonePalette,
        influence: &VertexBoneInfluence,
    ) -> (Vec3, Vec3) {
        let mut skinned_pos = Vec3::ZERO;
        let mut skinned_normal = Vec3::ZERO;

        for i in 0..MAX_BONE_INFLUENCES {
            let weight = influence.bone_weights[i];
            if weight <= 0.0 {
                break;
            }

            let bone_idx = influence.bone_indices[i] as usize;
            if let Some(bone_matrix) = bone_palette.get_bone(bone_idx) {
                // Transform position
                let transformed_pos = bone_matrix.transform_point3(position);
                skinned_pos += transformed_pos * weight;

                // Transform normal
                let transformed_normal = bone_matrix.transform_vector3(normal);
                skinned_normal += transformed_normal * weight;
            }
        }

        (skinned_pos, skinned_normal.normalize())
    }

    /// Batch skin multiple vertices
    pub fn skin_vertices(
        positions: &[Vec3],
        normals: &[Vec3],
        influences: &[VertexBoneInfluence],
        bone_palette: &BonePalette,
    ) -> Vec<(Vec3, Vec3)> {
        assert_eq!(positions.len(), normals.len());
        assert_eq!(positions.len(), influences.len());

        positions
            .iter()
            .zip(normals.iter())
            .zip(influences.iter())
            .map(|((pos, normal), influence)| {
                Self::apply_bone_transforms(*pos, *normal, bone_palette, influence)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bone_palette_creation() {
        let palette = BonePalette::new();
        assert_eq!(palette.bone_count, 0);
    }

    #[test]
    fn test_bone_palette_set_get() {
        let mut palette = BonePalette::new();
        let matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));

        palette.set_bone(0, matrix);
        assert_eq!(palette.bone_count, 1);

        let retrieved = palette.get_bone(0).unwrap();
        assert_eq!(retrieved, matrix);
    }

    #[test]
    fn test_vertex_bone_influence_single() {
        let influence = VertexBoneInfluence::from_single_bone(5);
        assert_eq!(influence.bone_indices[0], 5);
        assert_eq!(influence.bone_weights[0], 1.0);
        assert!(influence.validate().is_ok());
    }

    #[test]
    fn test_vertex_bone_influence_validation() {
        let mut influence = VertexBoneInfluence::new();
        influence.bone_indices = [0, 1, 2, 3];
        influence.bone_weights = [0.4, 0.3, 0.2, 0.1];

        assert!(influence.validate().is_ok());
    }

    #[test]
    fn test_vertex_bone_influence_normalize() {
        let mut influence = VertexBoneInfluence::new();
        influence.bone_indices = [0, 1, 0, 0];
        influence.bone_weights = [0.6, 0.6, 0.0, 0.0];

        influence.normalize();
        assert!((influence.bone_weights[0] - 0.5).abs() < 0.001);
        assert!((influence.bone_weights[1] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_vertex_bone_influence_active_count() {
        let mut influence = VertexBoneInfluence::new();
        influence.bone_weights = [0.5, 0.3, 0.2, 0.0];

        assert_eq!(influence.active_count(), 3);
    }

    #[test]
    fn test_shader_generation() {
        let shader = SkinningShaderGenerator::generate_skinning_shader(64);
        assert!(shader.contains("bones: array<mat4x4<f32>, 64>"));
        assert!(shader.contains("bone_indices"));
        assert!(shader.contains("bone_weights"));
        assert!(shader.contains("@vertex"));
        assert!(shader.contains("@fragment"));
    }

    #[test]
    fn test_simple_shader_generation() {
        let shader = SkinningShaderGenerator::generate_simple_skinning_shader(32);
        assert!(shader.contains("bones: array<mat4x4<f32>, 32>"));
        assert!(shader.contains("skinned_pos"));
        assert!(shader.contains("skinned_normal"));
    }

    #[test]
    fn test_cpu_skinning_single_bone() {
        let mut palette = BonePalette::new();
        let transform = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        palette.set_bone(0, transform);

        let influence = VertexBoneInfluence::from_single_bone(0);
        let position = Vec3::new(5.0, 5.0, 5.0);
        let normal = Vec3::new(0.0, 1.0, 0.0);

        let (skinned_pos, skinned_normal) =
            CpuSkinning::apply_bone_transforms(position, normal, &palette, &influence);

        assert_eq!(skinned_pos, Vec3::new(15.0, 5.0, 5.0));
        assert_eq!(skinned_normal, normal); // Translation doesn't affect normals
    }

    #[test]
    fn test_cpu_skinning_multiple_bones() {
        let mut palette = BonePalette::new();

        // Bone 0: translate right
        palette.set_bone(0, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));

        // Bone 1: translate up
        palette.set_bone(1, Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0)));

        let mut influence = VertexBoneInfluence::new();
        influence.bone_indices = [0, 1, 0, 0];
        influence.bone_weights = [0.5, 0.5, 0.0, 0.0];

        let position = Vec3::ZERO;
        let normal = Vec3::Z;

        let (skinned_pos, _) =
            CpuSkinning::apply_bone_transforms(position, normal, &palette, &influence);

        // Should be average of (10, 0, 0) and (0, 10, 0)
        assert!((skinned_pos.x - 5.0).abs() < 0.001);
        assert!((skinned_pos.y - 5.0).abs() < 0.001);
        assert!((skinned_pos.z - 0.0).abs() < 0.001);
    }
}
