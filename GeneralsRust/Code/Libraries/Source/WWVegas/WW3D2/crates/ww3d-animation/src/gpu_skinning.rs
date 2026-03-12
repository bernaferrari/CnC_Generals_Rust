//! GPU Skinning Implementation
//!
//! This module provides GPU-accelerated skeletal animation by computing
//! bone transformation matrices on the GPU instead of the CPU.

use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;
use ww3d_assets::prototypes::HierarchyPrototype;

/// GPU skinning data structure for vertex skinning
/// Supports up to 4 bone influences per vertex as per WW3D standard
#[derive(Debug, Clone)]
pub struct GPUSkinningData {
    /// Bone transformation matrices (up to 64 bones for GPU)
    pub bone_matrices: [Mat4; 64],
    /// Inverse bind pose matrices for proper skinning
    pub inverse_bind_matrices: [Mat4; 64],
    /// Number of bones actually used
    pub num_bones: u32,
    /// Bone indices for each vertex (for vertex skinning) - up to 4 influences
    pub bone_indices: Vec<[u32; 4]>,
    /// Bone weights for each vertex (for vertex skinning) - up to 4 influences
    pub bone_weights: Vec<[f32; 4]>,
    /// Bone name to index mapping
    pub bone_name_to_index: HashMap<String, usize>,
}

impl Default for GPUSkinningData {
    fn default() -> Self {
        Self {
            bone_matrices: [Mat4::IDENTITY; 64],
            inverse_bind_matrices: [Mat4::IDENTITY; 64],
            num_bones: 0,
            bone_indices: Vec::new(),
            bone_weights: Vec::new(),
            bone_name_to_index: HashMap::new(),
        }
    }
}

impl GPUSkinningData {
    /// Create new GPU skinning data
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct skinning data directly from a hierarchy prototype, seeding the
    /// bind pose and inverse bind matrices with the baked WW3D pivot data.
    pub fn from_hierarchy(hierarchy: &HierarchyPrototype) -> Self {
        let mut data = Self::new();
        let bone_count = hierarchy
            .bind_transforms
            .len()
            .min(64)
            .min(hierarchy.inverse_bind_transforms.len());

        if bone_count == 0 {
            return data;
        }

        data.num_bones = bone_count as u32;

        for index in 0..bone_count {
            if let Some(pivot) = hierarchy.pivots.get(index) {
                data.bone_name_to_index
                    .insert(pivot.name_str(), index);
            }

            let bind = hierarchy
                .bind_transforms
                .get(index)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            data.bone_matrices[index] = bind;

            let inverse = hierarchy
                .inverse_bind_transforms
                .get(index)
                .copied()
                .filter(|matrix| matrix.is_finite())
                .unwrap_or_else(|| {
                    let computed = bind.inverse();
                    if computed.is_finite() {
                        computed
                    } else {
                        Mat4::IDENTITY
                    }
                });
            data.inverse_bind_matrices[index] = inverse;
        }

        data
    }

    /// Set bone transformation matrix
    pub fn set_bone_matrix(&mut self, bone_index: usize, matrix: Mat4) {
        if bone_index < 64 {
            self.bone_matrices[bone_index] = matrix;
            self.num_bones = self.num_bones.max((bone_index + 1) as u32);
        }
    }

    /// Set inverse bind pose matrix for a bone
    pub fn set_inverse_bind_matrix(&mut self, bone_index: usize, matrix: Mat4) {
        if bone_index < 64 {
            self.inverse_bind_matrices[bone_index] = matrix;
        }
    }

    /// Get inverse bind pose matrix for a bone
    pub fn get_inverse_bind_matrix(&self, bone_index: usize) -> Mat4 {
        if bone_index < 64 {
            self.inverse_bind_matrices[bone_index]
        } else {
            Mat4::IDENTITY
        }
    }

    /// Add bone with name mapping
    pub fn add_bone(&mut self, name: &str, bind_pose_matrix: Mat4) -> usize {
        let bone_index = self.num_bones as usize;
        if bone_index < 64 {
            self.bone_name_to_index.insert(name.to_string(), bone_index);
            self.inverse_bind_matrices[bone_index] = bind_pose_matrix.inverse();
            self.bone_matrices[bone_index] = Mat4::IDENTITY;
            self.num_bones += 1;
            bone_index
        } else {
            0 // Return 0 if we exceed the limit
        }
    }

    /// Get bone index by name
    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.bone_name_to_index.get(name).copied()
    }

    /// Get bone transformation matrix
    pub fn get_bone_matrix(&self, bone_index: usize) -> Mat4 {
        if bone_index < 64 {
            self.bone_matrices[bone_index]
        } else {
            Mat4::IDENTITY
        }
    }

    /// Add vertex skinning data with automatic weight normalization
    /// Ensures weights sum to 1.0 and sorts by weight (highest first)
    pub fn add_vertex_weights(&mut self, bone_indices: [u32; 4], bone_weights: [f32; 4]) {
        let normalized = Self::normalize_vertex_weights(bone_indices, bone_weights);
        self.bone_indices.push(normalized.0);
        self.bone_weights.push(normalized.1);
    }

    /// Add vertex skinning data without normalization (for pre-normalized data)
    pub fn add_vertex_weights_raw(&mut self, bone_indices: [u32; 4], bone_weights: [f32; 4]) {
        self.bone_indices.push(bone_indices);
        self.bone_weights.push(bone_weights);
    }

    /// Normalize vertex weights to sum to 1.0 and sort by weight
    /// This ensures proper skinning behavior matching WW3D standards
    fn normalize_vertex_weights(
        bone_indices: [u32; 4],
        bone_weights: [f32; 4],
    ) -> ([u32; 4], [f32; 4]) {
        // Create tuples of (bone_index, weight) and sort by weight descending
        let mut pairs: Vec<(u32, f32)> = bone_indices
            .iter()
            .zip(bone_weights.iter())
            .map(|(&idx, &weight)| (idx, weight.max(0.0))) // Ensure non-negative weights
            .collect();

        // Sort by weight descending
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate total weight
        let total_weight: f32 = pairs.iter().map(|(_, weight)| *weight).sum();

        let mut normalized_indices = [0u32; 4];
        let mut normalized_weights = [0.0f32; 4];

        if total_weight > 0.0 {
            // Normalize weights to sum to 1.0
            for (i, (bone_idx, weight)) in pairs.iter().enumerate() {
                normalized_indices[i] = *bone_idx;
                normalized_weights[i] = *weight / total_weight;
            }
        } else {
            // If no weights, use first bone with weight 1.0
            normalized_indices[0] = bone_indices[0];
            normalized_weights[0] = 1.0;
        }

        (normalized_indices, normalized_weights)
    }

    /// Get vertex bone indices
    pub fn vertex_bone_indices(&self, vertex_index: usize) -> Option<&[u32; 4]> {
        self.bone_indices.get(vertex_index)
    }

    /// Get vertex bone weights
    pub fn vertex_bone_weights(&self, vertex_index: usize) -> Option<&[f32; 4]> {
        self.bone_weights.get(vertex_index)
    }

    /// Compute blended transformation matrix for a vertex
    /// Uses proper skinning math: final_transform = sum(weight * bone_matrix * inverse_bind_matrix)
    pub fn compute_vertex_transform(&self, vertex_index: usize) -> Mat4 {
        if let (Some(indices), Some(weights)) = (
            self.vertex_bone_indices(vertex_index),
            self.vertex_bone_weights(vertex_index)
        ) {
            let mut result = Mat4::ZERO;

            for i in 0..4 {
                let bone_index = indices[i] as usize;
                let weight = weights[i];

                if weight > 0.0 && bone_index < self.num_bones as usize {
                    // Proper skinning: current_pose * inverse_bind_pose
                    let skinning_matrix = self.bone_matrices[bone_index] * self.inverse_bind_matrices[bone_index];
                    result += skinning_matrix * weight;
                }
            }

            result
        } else {
            Mat4::IDENTITY
        }
    }

    /// Transform a vertex position using GPU skinning
    pub fn transform_vertex(&self, vertex_index: usize, position: Vec3) -> Vec3 {
        let transform = self.compute_vertex_transform(vertex_index);
        transform.transform_point3(position)
    }

    /// Transform a vertex normal using GPU skinning
    pub fn transform_normal(&self, vertex_index: usize, normal: Vec3) -> Vec3 {
        let transform = self.compute_vertex_transform(vertex_index);
        // Use inverse transpose for normals
        let normal_transform = transform.inverse().transpose();
        normal_transform.transform_vector3(normal).normalize_or_zero()
    }

    /// Update all bone matrices from animation data
    pub fn update_from_animation(&mut self, bone_transforms: &[Mat4]) {
        for (i, &transform) in bone_transforms.iter().enumerate() {
            if i < 64 {
                self.bone_matrices[i] = transform;
            }
        }
        self.num_bones = bone_transforms.len().min(64) as u32;
    }

    /// Update bone matrices from hierarchy tree (integration with HTreeClass)
    pub fn update_from_htree(&mut self, htree: &crate::htree::HTreeClass) {
        let bone_count = htree.num_pivots().min(64);

        for i in 0..bone_count {
            if let Some(transform) = htree.get_transform(i) {
                self.bone_matrices[i] = transform;
            }
        }

        self.num_bones = bone_count as u32;
    }

    /// Set bone matrices and corresponding inverse bind matrices
    pub fn set_bone_data(&mut self, bone_index: usize, current_transform: Mat4, bind_pose_transform: Mat4) {
        if bone_index < 64 {
            self.bone_matrices[bone_index] = current_transform;
            self.inverse_bind_matrices[bone_index] = bind_pose_transform.inverse();
            self.num_bones = self.num_bones.max((bone_index + 1) as u32);
        }
    }

    /// Reset all bone matrices to identity
    pub fn reset_bones(&mut self) {
        for matrix in &mut self.bone_matrices {
            *matrix = Mat4::IDENTITY;
        }
        self.num_bones = 0;
    }

    /// Get bone matrices as flat array for GPU upload
    pub fn bone_matrices_flat(&self) -> [f32; 64 * 16] {
        let mut result = [0.0; 64 * 16];

        for (i, matrix) in self.bone_matrices.iter().enumerate() {
            let offset = i * 16;
            let cols = matrix.to_cols_array();

            for j in 0..16 {
                result[offset + j] = cols[j];
            }
        }

        result
    }

    /// Get inverse bind matrices as flat array for GPU upload
    pub fn inverse_bind_matrices_flat(&self) -> [f32; 64 * 16] {
        let mut result = [0.0; 64 * 16];

        for (i, matrix) in self.inverse_bind_matrices.iter().enumerate() {
            let offset = i * 16;
            let cols = matrix.to_cols_array();

            for j in 0..16 {
                result[offset + j] = cols[j];
            }
        }

        result
    }

    /// Get complete bone uniform data for GPU upload
    /// Returns (bone_matrices_flat, inverse_bind_matrices_flat)
    pub fn get_uniform_data(&self) -> ([f32; 64 * 16], [f32; 64 * 16]) {
        (self.bone_matrices_flat(), self.inverse_bind_matrices_flat())
    }

    /// Validate vertex weights (ensure they sum to approximately 1.0)
    pub fn validate_vertex_weights(&self, tolerance: f32) -> Vec<usize> {
        let mut invalid_vertices = Vec::new();

        for (i, weights) in self.bone_weights.iter().enumerate() {
            let total: f32 = weights.iter().sum();
            if (total - 1.0).abs() > tolerance {
                invalid_vertices.push(i);
            }
        }

        invalid_vertices
    }
}

/// GPU skinning shader utilities
pub struct GPUSkinningShaders;

impl GPUSkinningShaders {
    /// Vertex shader for GPU skinning
    pub fn vertex_shader_source() -> &'static str {
        r#"
        struct VertexInput {
            @location(0) position: vec3<f32>,
            @location(1) normal: vec3<f32>,
            @location(2) tex_coords: vec2<f32>,
            @location(3) bone_indices: vec4<u32>,
            @location(4) bone_weights: vec4<f32>,
        };

        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
            @location(0) tex_coords: vec2<f32>,
            @location(1) normal: vec3<f32>,
            @location(2) world_position: vec3<f32>,
        };

        struct CameraUniform {
            view_proj: mat4x4<f32>,
            position: vec3<f32>,
        };

        struct ModelUniform {
            model: mat4x4<f32>,
        };

        struct BoneUniform {
            bones: array<mat4x4<f32>, 64>,
            inverse_bind_matrices: array<mat4x4<f32>, 64>,
        };

        @group(0) @binding(0)
        var<uniform> camera: CameraUniform;

        @group(1) @binding(0)
        var<uniform> model: ModelUniform;

        @group(2) @binding(0)
        var<uniform> bones: BoneUniform;

        @vertex
        fn vs_main(input: VertexInput) -> VertexOutput {
            var output: VertexOutput;

            // Compute skinning transform using proper skinning math
            var skin_transform = mat4x4<f32>(0.0, 0.0, 0.0, 0.0,
                                            0.0, 0.0, 0.0, 0.0,
                                            0.0, 0.0, 0.0, 0.0,
                                            0.0, 0.0, 0.0, 0.0);

            var total_weight = 0.0;
            for (var i = 0u; i < 4u; i = i + 1u) {
                let bone_index = input.bone_indices[i];
                let bone_weight = input.bone_weights[i];

                if (bone_weight > 0.0) {
                    // Proper skinning: current_pose * inverse_bind_pose
                    let skinning_matrix = bones.bones[bone_index] * bones.inverse_bind_matrices[bone_index];
                    skin_transform = skin_transform + skinning_matrix * bone_weight;
                    total_weight = total_weight + bone_weight;
                }
            }

            // Normalize if weights don't sum to 1
            if (total_weight > 0.0 && total_weight != 1.0) {
                skin_transform = skin_transform / total_weight;
            }

            // Apply model transform
            let model_transform = model.model * skin_transform;

            // Transform position
            let world_position = model_transform * vec4<f32>(input.position, 1.0);
            output.clip_position = camera.view_proj * world_position;
            output.world_position = world_position.xyz;

            // Transform normal (use inverse transpose for correct normals)
            let normal_matrix = transpose(inverse(model_transform));
            output.normal = normalize((normal_matrix * vec4<f32>(input.normal, 0.0)).xyz);

            // Pass through texture coordinates
            output.tex_coords = input.tex_coords;

            return output;
        }
        "#
    }

    /// Fragment shader for GPU skinning
    pub fn fragment_shader_source() -> &'static str {
        r#"
        struct FragmentInput {
            @location(0) tex_coords: vec2<f32>,
            @location(1) normal: vec3<f32>,
            @location(2) world_position: vec3<f32>,
        };

        struct FragmentOutput {
            @location(0) color: vec4<f32>,
        };

        struct MaterialUniform {
            ambient: vec4<f32>,
            diffuse: vec4<f32>,
            specular: vec4<f32>,
            shininess: f32,
        };

        struct LightUniform {
            position: vec3<f32>,
            color: vec3<f32>,
            intensity: f32,
        };

        @group(3) @binding(0)
        var<uniform> material: MaterialUniform;

        @group(3) @binding(1)
        var<uniform> light: LightUniform;

        @group(3) @binding(2)
        var texture: texture_2d<f32>;

        @group(3) @binding(3)
        var sampler_: sampler;

        @fragment
        fn fs_main(input: FragmentInput) -> FragmentOutput {
            var output: FragmentOutput;

            // Sample texture
            let tex_color = textureSample(texture, sampler_, input.tex_coords);

            // Simple lighting calculation
            let light_dir = normalize(light.position - input.world_position);
            let diffuse_factor = max(dot(input.normal, light_dir), 0.0);
            let diffuse = material.diffuse.rgb * light.color * light.intensity * diffuse_factor;

            // Ambient lighting
            let ambient = material.ambient.rgb * 0.3;

            // Combine lighting
            let final_color = (ambient + diffuse) * tex_color.rgb;

            output.color = vec4<f32>(final_color, material.diffuse.a * tex_color.a);

            return output;
        }
        "#
    }
}

/// CPU fallback for systems without GPU skinning support
pub struct CPUSkinningFallback {
    pub skinned_vertices: Vec<Vec3>,
    pub skinned_normals: Vec<Vec3>,
}

impl CPUSkinningFallback {
    pub fn new(vertex_count: usize) -> Self {
        Self {
            skinned_vertices: vec![Vec3::ZERO; vertex_count],
            skinned_normals: vec![Vec3::ZERO; vertex_count],
        }
    }

    /// Apply CPU skinning to vertices
    pub fn apply_skinning(
        &mut self,
        original_vertices: &[Vec3],
        original_normals: &[Vec3],
        bone_indices: &[[u32; 4]],
        bone_weights: &[[f32; 4]],
        bone_matrices: &[Mat4],
    ) {
        for (i, (&vertex, &normal)) in original_vertices.iter().zip(original_normals).enumerate() {
            if let (Some(indices), Some(weights)) = (
                bone_indices.get(i),
                bone_weights.get(i)
            ) {
                let mut skinned_vertex = Vec3::ZERO;
                let mut skinned_normal = Vec3::ZERO;

                for j in 0..4 {
                    let bone_index = indices[j] as usize;
                    let weight = weights[j];

                    if weight > 0.0 && bone_index < bone_matrices.len() {
                        let bone_matrix = bone_matrices[bone_index];

                        // Transform vertex
                        let transformed_vertex = bone_matrix.transform_point3(vertex);
                        skinned_vertex += transformed_vertex * weight;

                        // Transform normal (use inverse transpose)
                        let normal_matrix = bone_matrix.inverse().transpose();
                        let transformed_normal = normal_matrix.transform_vector3(normal);
                        skinned_normal += transformed_normal.normalize_or_zero() * weight;
                    }
                }

                self.skinned_vertices[i] = skinned_vertex;
                self.skinned_normals[i] = skinned_normal.normalize();
            } else {
                self.skinned_vertices[i] = vertex;
                self.skinned_normals[i] = normal;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_skinning_data() {
        let mut skinning_data = GPUSkinningData::new();

        let matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        skinning_data.set_bone_matrix(0, matrix);

        assert_eq!(skinning_data.num_bones, 1);
        assert_eq!(skinning_data.get_bone_matrix(0), matrix);
    }

    #[test]
    fn test_vertex_weights() {
        let mut skinning_data = GPUSkinningData::new();

        skinning_data.add_vertex_weights([0, 1, 2, 3], [0.5, 0.3, 0.2, 0.0]);

        assert_eq!(skinning_data.bone_indices.len(), 1);
        assert_eq!(skinning_data.bone_weights.len(), 1);
        assert_eq!(skinning_data.bone_indices[0], [0, 1, 2, 3]);
        assert_eq!(skinning_data.bone_weights[0], [0.5, 0.3, 0.2, 0.0]);
    }

    #[test]
    fn test_cpu_skinning_fallback() {
        let mut cpu_skinning = CPUSkinningFallback::new(3);

        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];

        let normals = vec![
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
        ];

        let bone_indices = vec![
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ];

        let bone_weights = vec![
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ];

        let bone_matrices = vec![Mat4::IDENTITY];

        cpu_skinning.apply_skinning(
            &vertices,
            &normals,
            &bone_indices,
            &bone_weights,
            &bone_matrices,
        );

        // Since we're using identity matrix, vertices should be unchanged
        assert_eq!(cpu_skinning.skinned_vertices[0], vertices[0]);
        assert_eq!(cpu_skinning.skinned_vertices[1], vertices[1]);
        assert_eq!(cpu_skinning.skinned_vertices[2], vertices[2]);
    }

    #[test]
    fn skinning_data_from_hierarchy_uses_baked_inverse() {
        use ww3d_core::w3d_format::{W3dPivotStruct, W3dVectorStruct};

        fn make_pivot(name: &str, parent_idx: i32, translation: [f32; 3]) -> W3dPivotStruct {
            let mut name_bytes = [0u8; 16];
            let raw = name.as_bytes();
            let len = raw.len().min(16);
            name_bytes[..len].copy_from_slice(&raw[..len]);
            W3dPivotStruct {
                name: name_bytes,
                parent_idx,
                translation: W3dVectorStruct {
                    x: translation[0],
                    y: translation[1],
                    z: translation[2],
                },
                euler_angles: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            }
        }

        let mut hierarchy = HierarchyPrototype::new("Skinned".into());
        hierarchy.pivots = vec![
            make_pivot("ROOT", -1, [0.0, 0.0, 0.0]),
            make_pivot("ARM", 0, [0.0, 1.0, 0.0]),
        ];
        hierarchy.num_pivots = hierarchy.pivots.len() as u32;
        hierarchy.recompute_bind_transforms();

        let data = GPUSkinningData::from_hierarchy(&hierarchy);
        assert_eq!(data.num_bones, 2);
        assert_eq!(data.bone_name_to_index.get("ARM"), Some(&1));
        assert_eq!(data.bone_matrices[0], Mat4::IDENTITY);
        assert_eq!(
            data.bone_matrices[1],
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0))
        );
        assert_eq!(
            data.inverse_bind_matrices[1],
            Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0))
        );
    }
}
