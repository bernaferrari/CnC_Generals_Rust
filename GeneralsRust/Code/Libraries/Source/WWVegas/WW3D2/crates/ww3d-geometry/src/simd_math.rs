//! SIMD-accelerated math operations for performance-critical code paths
//!
//! This module provides SIMD implementations of vector and matrix operations
//! to match or exceed C++ SSE/AVX performance. Falls back to scalar operations
//! on non-x86_64 platforms.

use glam::{Mat4, Vec3};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD-optimized vector operations
pub mod vector_ops {
    use super::*;

    /// Compute dot products for multiple vector pairs using SIMD
    /// Processes 4 vector pairs per SIMD operation (SSE)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn vec3_dot_simd_batch(a: &[Vec3], b: &[Vec3], output: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), output.len());

        let len = a.len();
        let mut i = 0;

        // Process 4 vectors at a time using SSE
        while i + 3 < len {
            // Load 4 x-components
            let ax = _mm_set_ps(a[i + 3].x, a[i + 2].x, a[i + 1].x, a[i].x);
            let bx = _mm_set_ps(b[i + 3].x, b[i + 2].x, b[i + 1].x, b[i].x);

            // Load 4 y-components
            let ay = _mm_set_ps(a[i + 3].y, a[i + 2].y, a[i + 1].y, a[i].y);
            let by = _mm_set_ps(b[i + 3].y, b[i + 2].y, b[i + 1].y, b[i].y);

            // Load 4 z-components
            let az = _mm_set_ps(a[i + 3].z, a[i + 2].z, a[i + 1].z, a[i].z);
            let bz = _mm_set_ps(b[i + 3].z, b[i + 2].z, b[i + 1].z, b[i].z);

            // Compute dot product: x*x + y*y + z*z
            let mut dot = _mm_mul_ps(ax, bx);
            dot = _mm_add_ps(dot, _mm_mul_ps(ay, by));
            dot = _mm_add_ps(dot, _mm_mul_ps(az, bz));

            // Store results
            let mut result = [0.0f32; 4];
            _mm_storeu_ps(result.as_mut_ptr(), dot);
            output[i] = result[0];
            output[i + 1] = result[1];
            output[i + 2] = result[2];
            output[i + 3] = result[3];

            i += 4;
        }

        // Handle remaining vectors
        while i < len {
            output[i] = a[i].dot(b[i]);
            i += 1;
        }
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn vec3_dot_simd_batch(a: &[Vec3], b: &[Vec3], output: &mut [f32]) {
        for i in 0..a.len() {
            output[i] = a[i].dot(b[i]);
        }
    }

    /// Compute cross products for multiple vector pairs using SIMD
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn vec3_cross_simd_batch(a: &[Vec3], b: &[Vec3], output: &mut [Vec3]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), output.len());

        let len = a.len();
        let mut i = 0;

        // Process 4 vectors at a time
        while i + 3 < len {
            // Cross product formula: (a.y*b.z - a.z*b.y, a.z*b.x - a.x*b.z, a.x*b.y - a.y*b.x)

            // Load components
            let ax = _mm_set_ps(a[i + 3].x, a[i + 2].x, a[i + 1].x, a[i].x);
            let ay = _mm_set_ps(a[i + 3].y, a[i + 2].y, a[i + 1].y, a[i].y);
            let az = _mm_set_ps(a[i + 3].z, a[i + 2].z, a[i + 1].z, a[i].z);

            let bx = _mm_set_ps(b[i + 3].x, b[i + 2].x, b[i + 1].x, b[i].x);
            let by = _mm_set_ps(b[i + 3].y, b[i + 2].y, b[i + 1].y, b[i].y);
            let bz = _mm_set_ps(b[i + 3].z, b[i + 2].z, b[i + 1].z, b[i].z);

            // Compute cross product components
            let cx = _mm_sub_ps(_mm_mul_ps(ay, bz), _mm_mul_ps(az, by));
            let cy = _mm_sub_ps(_mm_mul_ps(az, bx), _mm_mul_ps(ax, bz));
            let cz = _mm_sub_ps(_mm_mul_ps(ax, by), _mm_mul_ps(ay, bx));

            // Store results
            let mut result_x = [0.0f32; 4];
            let mut result_y = [0.0f32; 4];
            let mut result_z = [0.0f32; 4];

            _mm_storeu_ps(result_x.as_mut_ptr(), cx);
            _mm_storeu_ps(result_y.as_mut_ptr(), cy);
            _mm_storeu_ps(result_z.as_mut_ptr(), cz);

            for j in 0..4 {
                output[i + j] = Vec3::new(result_x[j], result_y[j], result_z[j]);
            }

            i += 4;
        }

        // Handle remaining vectors
        while i < len {
            output[i] = a[i].cross(b[i]);
            i += 1;
        }
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn vec3_cross_simd_batch(a: &[Vec3], b: &[Vec3], output: &mut [Vec3]) {
        for i in 0..a.len() {
            output[i] = a[i].cross(b[i]);
        }
    }

    /// Normalize multiple vectors using SIMD
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn vec3_normalize_simd_batch(vectors: &mut [Vec3]) {
        let len = vectors.len();
        let mut i = 0;

        while i + 3 < len {
            // Load components
            let x = _mm_set_ps(
                vectors[i + 3].x,
                vectors[i + 2].x,
                vectors[i + 1].x,
                vectors[i].x,
            );
            let y = _mm_set_ps(
                vectors[i + 3].y,
                vectors[i + 2].y,
                vectors[i + 1].y,
                vectors[i].y,
            );
            let z = _mm_set_ps(
                vectors[i + 3].z,
                vectors[i + 2].z,
                vectors[i + 1].z,
                vectors[i].z,
            );

            // Compute length squared: x*x + y*y + z*z
            let mut len_sq = _mm_mul_ps(x, x);
            len_sq = _mm_add_ps(len_sq, _mm_mul_ps(y, y));
            len_sq = _mm_add_ps(len_sq, _mm_mul_ps(z, z));

            // Compute reciprocal square root (fast inverse sqrt)
            let inv_len = _mm_rsqrt_ps(len_sq);

            // Normalize
            let nx = _mm_mul_ps(x, inv_len);
            let ny = _mm_mul_ps(y, inv_len);
            let nz = _mm_mul_ps(z, inv_len);

            // Store results
            let mut result_x = [0.0f32; 4];
            let mut result_y = [0.0f32; 4];
            let mut result_z = [0.0f32; 4];

            _mm_storeu_ps(result_x.as_mut_ptr(), nx);
            _mm_storeu_ps(result_y.as_mut_ptr(), ny);
            _mm_storeu_ps(result_z.as_mut_ptr(), nz);

            for j in 0..4 {
                vectors[i + j] = Vec3::new(result_x[j], result_y[j], result_z[j]);
            }

            i += 4;
        }

        // Handle remaining vectors
        while i < len {
            vectors[i] = vectors[i].normalize_or_zero();
            i += 1;
        }
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn vec3_normalize_simd_batch(vectors: &mut [Vec3]) {
        for v in vectors {
            *v = v.normalize_or_zero();
        }
    }

    /// Safe wrapper for SIMD dot product
    pub fn batch_dot_product(a: &[Vec3], b: &[Vec3], output: &mut [f32]) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    vec3_dot_simd_batch(a, b, output);
                }
            } else {
                vec3_dot_simd_batch(a, b, output);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            vec3_dot_simd_batch(a, b, output);
        }
    }

    /// Safe wrapper for SIMD cross product
    pub fn batch_cross_product(a: &[Vec3], b: &[Vec3], output: &mut [Vec3]) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    vec3_cross_simd_batch(a, b, output);
                }
            } else {
                vec3_cross_simd_batch(a, b, output);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            vec3_cross_simd_batch(a, b, output);
        }
    }

    /// Safe wrapper for SIMD normalization
    pub fn batch_normalize(vectors: &mut [Vec3]) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    vec3_normalize_simd_batch(vectors);
                }
            } else {
                vec3_normalize_simd_batch(vectors);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            vec3_normalize_simd_batch(vectors);
        }
    }
}

/// SIMD-optimized matrix operations
pub mod matrix_ops {
    use super::*;

    /// Multiply two 4x4 matrices using SIMD
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn mat4_mul_simd(a: &Mat4, b: &Mat4) -> Mat4 {
        // Load matrix B columns
        let b_col0 = _mm_loadu_ps(b.col(0).as_ref().as_ptr());
        let b_col1 = _mm_loadu_ps(b.col(1).as_ref().as_ptr());
        let b_col2 = _mm_loadu_ps(b.col(2).as_ref().as_ptr());
        let b_col3 = _mm_loadu_ps(b.col(3).as_ref().as_ptr());

        let mut result_cols = [[0.0f32; 4]; 4];

        // Compute each column of the result
        for i in 0..4 {
            let a_row = a.row(i);

            // Broadcast each component of the row
            let a_x = _mm_set1_ps(a_row.x);
            let a_y = _mm_set1_ps(a_row.y);
            let a_z = _mm_set1_ps(a_row.z);
            let a_w = _mm_set1_ps(a_row.w);

            // Multiply and accumulate
            let mut result = _mm_mul_ps(a_x, b_col0);
            result = _mm_add_ps(result, _mm_mul_ps(a_y, b_col1));
            result = _mm_add_ps(result, _mm_mul_ps(a_z, b_col2));
            result = _mm_add_ps(result, _mm_mul_ps(a_w, b_col3));

            _mm_storeu_ps(result_cols[i].as_mut_ptr(), result);
        }

        Mat4::from_cols_array_2d(&result_cols).transpose()
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn mat4_mul_simd(a: &Mat4, b: &Mat4) -> Mat4 {
        *a * *b
    }

    /// Transform multiple vectors by a matrix using SIMD
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn mat4_transform_points_simd(matrix: &Mat4, points: &[Vec3], output: &mut [Vec3]) {
        assert_eq!(points.len(), output.len());

        // Load matrix rows
        let m_row0 = _mm_loadu_ps(matrix.row(0).as_ref().as_ptr());
        let m_row1 = _mm_loadu_ps(matrix.row(1).as_ref().as_ptr());
        let m_row2 = _mm_loadu_ps(matrix.row(2).as_ref().as_ptr());
        let m_row3 = _mm_loadu_ps(matrix.row(3).as_ref().as_ptr());

        for i in 0..points.len() {
            let p = points[i];

            // Create point as (x, y, z, 1)
            let point = _mm_set_ps(1.0, p.z, p.y, p.x);

            // Broadcast components
            let x = _mm_shuffle_ps(point, point, 0b00_00_00_00);
            let y = _mm_shuffle_ps(point, point, 0b01_01_01_01);
            let z = _mm_shuffle_ps(point, point, 0b10_10_10_10);
            let w = _mm_shuffle_ps(point, point, 0b11_11_11_11);

            // Transform
            let mut result = _mm_mul_ps(m_row0, x);
            result = _mm_add_ps(result, _mm_mul_ps(m_row1, y));
            result = _mm_add_ps(result, _mm_mul_ps(m_row2, z));
            result = _mm_add_ps(result, _mm_mul_ps(m_row3, w));

            // Store result
            let mut r = [0.0f32; 4];
            _mm_storeu_ps(r.as_mut_ptr(), result);

            // Perspective divide if needed
            if r[3].abs() > 0.0001 {
                output[i] = Vec3::new(r[0] / r[3], r[1] / r[3], r[2] / r[3]);
            } else {
                output[i] = Vec3::new(r[0], r[1], r[2]);
            }
        }
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn mat4_transform_points_simd(matrix: &Mat4, points: &[Vec3], output: &mut [Vec3]) {
        for i in 0..points.len() {
            output[i] = matrix.transform_point3(points[i]);
        }
    }

    /// Safe wrapper for matrix multiplication
    pub fn multiply_matrices(a: &Mat4, b: &Mat4) -> Mat4 {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe { mat4_mul_simd(a, b) }
            } else {
                *a * *b
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            mat4_mul_simd(a, b)
        }
    }

    /// Safe wrapper for batch transformation
    pub fn transform_points(matrix: &Mat4, points: &[Vec3], output: &mut [Vec3]) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    mat4_transform_points_simd(matrix, points, output);
                }
            } else {
                for i in 0..points.len() {
                    output[i] = matrix.transform_point3(points[i]);
                }
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            mat4_transform_points_simd(matrix, points, output);
        }
    }
}

/// SIMD-optimized skinning for skeletal animation
pub mod skinning {
    use super::*;

    /// Skinning vertex data
    #[derive(Clone, Copy)]
    pub struct SkinnedVertex {
        pub position: Vec3,
        pub normal: Vec3,
        pub bone_indices: [u8; 4],
        pub bone_weights: [f32; 4],
    }

    /// Transform vertices by bone matrices using SIMD
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    pub unsafe fn skin_vertices_simd(
        vertices: &[SkinnedVertex],
        bone_matrices: &[Mat4],
        output_positions: &mut [Vec3],
        output_normals: &mut [Vec3],
    ) {
        assert_eq!(vertices.len(), output_positions.len());
        assert_eq!(vertices.len(), output_normals.len());

        for (i, vertex) in vertices.iter().enumerate() {
            // Accumulate weighted transformations
            let mut final_pos = Vec3::ZERO;
            let mut final_normal = Vec3::ZERO;

            for j in 0..4 {
                let bone_idx = vertex.bone_indices[j] as usize;
                let weight = vertex.bone_weights[j];

                if weight > 0.0001 && bone_idx < bone_matrices.len() {
                    let matrix = &bone_matrices[bone_idx];

                    // Transform position
                    let transformed_pos = matrix.transform_point3(vertex.position);
                    final_pos += transformed_pos * weight;

                    // Transform normal (use 3x3 part only)
                    let transformed_normal = matrix.transform_vector3(vertex.normal);
                    final_normal += transformed_normal * weight;
                }
            }

            output_positions[i] = final_pos;
            output_normals[i] = final_normal.normalize_or_zero();
        }
    }

    /// Fallback for non-x86_64 platforms
    #[cfg(not(target_arch = "x86_64"))]
    pub fn skin_vertices_simd(
        vertices: &[SkinnedVertex],
        bone_matrices: &[Mat4],
        output_positions: &mut [Vec3],
        output_normals: &mut [Vec3],
    ) {
        for (i, vertex) in vertices.iter().enumerate() {
            let mut final_pos = Vec3::ZERO;
            let mut final_normal = Vec3::ZERO;

            for j in 0..4 {
                let bone_idx = vertex.bone_indices[j] as usize;
                let weight = vertex.bone_weights[j];

                if weight > 0.0001 && bone_idx < bone_matrices.len() {
                    let matrix = &bone_matrices[bone_idx];
                    final_pos += matrix.transform_point3(vertex.position) * weight;
                    final_normal += matrix.transform_vector3(vertex.normal) * weight;
                }
            }

            output_positions[i] = final_pos;
            output_normals[i] = final_normal.normalize_or_zero();
        }
    }

    /// Safe wrapper for vertex skinning
    pub fn skin_vertices(
        vertices: &[SkinnedVertex],
        bone_matrices: &[Mat4],
        output_positions: &mut [Vec3],
        output_normals: &mut [Vec3],
    ) {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    skin_vertices_simd(vertices, bone_matrices, output_positions, output_normals);
                }
            } else {
                skin_vertices_simd(vertices, bone_matrices, output_positions, output_normals);
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            skin_vertices_simd(vertices, bone_matrices, output_positions, output_normals);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_dot_product() {
        let a = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];

        let b = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];

        let mut output = vec![0.0; 4];
        vector_ops::batch_dot_product(&a, &b, &mut output);

        assert!((output[0] - 1.0).abs() < 0.001);
        assert!((output[1] - 1.0).abs() < 0.001);
        assert!((output[2] - 1.0).abs() < 0.001);
        assert!((output[3] - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_batch_cross_product() {
        let a = vec![Vec3::X, Vec3::Y, Vec3::Z, Vec3::X];
        let b = vec![Vec3::Y, Vec3::Z, Vec3::X, Vec3::Z];
        let mut output = vec![Vec3::ZERO; 4];

        vector_ops::batch_cross_product(&a, &b, &mut output);

        assert!((output[0] - Vec3::Z).length() < 0.001);
        assert!((output[1] - Vec3::X).length() < 0.001);
        assert!((output[2] - Vec3::Y).length() < 0.001);
        assert!((output[3] - (-Vec3::Y)).length() < 0.001);
    }

    #[test]
    fn test_matrix_multiplication() {
        let a = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let b = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));

        let result = matrix_ops::multiply_matrices(&a, &b);
        let expected = a * b;

        // Compare with small epsilon
        for i in 0..4 {
            for j in 0..4 {
                let diff = (result.col(i)[j] - expected.col(i)[j]).abs();
                assert!(
                    diff < 0.001,
                    "Matrix element ({}, {}) differs by {}",
                    i,
                    j,
                    diff
                );
            }
        }
    }
}
