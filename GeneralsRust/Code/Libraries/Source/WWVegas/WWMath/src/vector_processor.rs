//! Vector processing utilities for bulk operations on vector arrays.
//!
//! This module provides optimized functions for common vector array operations
//! including transforms, copying, indexing, normalization, and mathematical operations.
//!
//! The original C++ implementation used SIMD optimizations for Intel SSE,
//! but this Rust version relies on the compiler's auto-vectorization and
//! explicit SIMD could be added in the future for performance-critical paths.

use crate::{Matrix3D, Matrix4, Vector3, Vector4};

/// Vector processor providing static methods for bulk vector operations.
pub struct VectorProcessor;

impl VectorProcessor {
    /// Transform `src` vectors by `matrix`, writing the results into `dst`.
    /// `dst` and `src` may alias.
    pub fn transform_vector3(dst: &mut [Vector3], src: &[Vector3], matrix: &Matrix3D) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = matrix.transform_vector(*s);
        }
    }

    /// Transform `src` vectors by `matrix`, writing `Vector4` results into `dst`.
    pub fn transform_vector3_to_vector4(dst: &mut [Vector4], src: &[Vector3], matrix: &Matrix4) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = *matrix * *s;
        }
    }

    /// Copy a slice of values into the destination slice.
    pub fn copy<T: Copy>(dst: &mut [T], src: &[T]) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        dst.copy_from_slice(src);
    }

    /// Copy `Vector3` values and companion alpha values into `Vector4` output.
    pub fn copy_vector3_to_vector4_with_alpha(dst: &mut [Vector4], src: &[Vector3], alpha: &[f32]) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        assert_eq!(
            src.len(),
            alpha.len(),
            "alpha array must match source length"
        );
        for ((d, s), &a) in dst.iter_mut().zip(src.iter()).zip(alpha.iter()) {
            *d = Vector4::new(s.x, s.y, s.z, a);
        }
    }

    /// Copy `Vector3` values into `Vector4` output using a uniform alpha.
    pub fn copy_vector3_to_vector4_single_alpha(dst: &mut [Vector4], src: &[Vector3], alpha: f32) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = Vector4::new(s.x, s.y, s.z, alpha);
        }
    }

    /// Copy a single `Vector3` into each element of `dst` with varying alpha values.
    pub fn copy_single_vector3_to_vector4_array(dst: &mut [Vector4], src: &Vector3, alpha: &[f32]) {
        assert_eq!(dst.len(), alpha.len(), "destination and alpha must match");
        for (d, &a) in dst.iter_mut().zip(alpha.iter()) {
            *d = Vector4::new(src.x, src.y, src.z, a);
        }
    }

    /// Copy indexed elements from `src` into `dst`.
    pub fn copy_indexed<T: Copy>(dst: &mut [T], src: &[T], indices: &[usize]) {
        assert!(
            dst.len() >= indices.len(),
            "destination must hold all indices"
        );
        for (d, &idx) in dst.iter_mut().zip(indices.iter()) {
            *d = src[idx];
        }
    }

    /// Clamp `src` values into `dst` while honoring potential aliasing.
    pub fn clamp_vector4(dst: &mut [Vector4], src: &[Vector4], min: f32, max: f32) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = Vector4::new(
                s.x.clamp(min, max),
                s.y.clamp(min, max),
                s.z.clamp(min, max),
                s.w.clamp(min, max),
            );
        }
    }

    /// Clamp `dst` in place.
    pub fn clamp_vector4_in_place(dst: &mut [Vector4], min: f32, max: f32) {
        for v in dst.iter_mut() {
            *v = Vector4::new(
                v.x.clamp(min, max),
                v.y.clamp(min, max),
                v.z.clamp(min, max),
                v.w.clamp(min, max),
            );
        }
    }

    /// Clear a Vector3 slice to zero.
    pub fn clear_vector3(dst: &mut [Vector3]) {
        for v in dst.iter_mut() {
            *v = Vector3::ZERO;
        }
    }

    /// Normalize a Vector3 slice in place.
    pub fn normalize_vector3(dst: &mut [Vector3]) {
        for v in dst.iter_mut() {
            *v = v.normalize_or_zero();
        }
    }

    /// Compute min/max vectors for `src` and write them to `min`/`max`.
    pub fn min_max_vector3(src: &[Vector3], min: &mut Vector3, max: &mut Vector3) {
        assert!(!src.is_empty(), "Cannot compute min/max of empty slice");
        *min = src[0];
        *max = src[0];
        for v in src.iter().skip(1) {
            if v.x < min.x {
                min.x = v.x;
            }
            if v.y < min.y {
                min.y = v.y;
            }
            if v.z < min.z {
                min.z = v.z;
            }
            if v.x > max.x {
                max.x = v.x;
            }
            if v.y > max.y {
                max.y = v.y;
            }
            if v.z > max.z {
                max.z = v.z;
            }
        }
    }

    /// Multiply-add operation performed in place on `dest`.
    pub fn mul_add_float_array(dest: &mut [f32], multiplier: f32, add: f32) {
        for value in dest.iter_mut() {
            *value = *value * multiplier + add;
        }
    }

    /// Compute dot products of `a` with each element of `b`, writing into `dst`.
    pub fn dot_product(dst: &mut [f32], a: &Vector3, b: &[Vector3]) {
        assert!(
            dst.len() >= b.len(),
            "destination must hold all dot products"
        );
        for (d, v) in dst.iter_mut().zip(b.iter()) {
            *d = a.dot(*v);
        }
    }

    /// Clamp floats to a minimum threshold, writing into `dst`.
    pub fn clamp_min_float_array(dst: &mut [f32], src: &[f32], min: f32) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, &v) in dst.iter_mut().zip(src.iter()) {
            *d = v.max(min);
        }
    }

    /// Raise floats to an exponent, writing into `dst`.
    pub fn power_float_array(dst: &mut [f32], src: &[f32], power: f32) {
        assert_eq!(dst.len(), src.len(), "destination and source must match");
        for (d, &v) in dst.iter_mut().zip(src.iter()) {
            *d = v.powf(power);
        }
    }

    /// Prefetch memory location (no-op in Rust, but provided for API compatibility).
    ///
    /// # Arguments
    /// * `_address` - Memory address to prefetch (ignored)
    pub fn prefetch(_address: *const u8) {
        // No-op in Rust - the compiler and CPU handle prefetching automatically
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Matrix3D, Matrix4, Vector3, Vector4};

    fn assert_vec3_approx_eq(actual: Vector3, expected: Vector3) {
        let diff = actual - expected;
        assert!(
            diff.length() <= 1e-5,
            "expected {:?}, got {:?}",
            expected,
            actual
        );
    }

    #[test]
    fn transform_vector3_writes_expected_results() {
        let src = vec![
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let mut dst = vec![Vector3::ZERO; src.len()];
        let matrix = Matrix3D::create_scale(Vector3::splat(2.0));
        VectorProcessor::transform_vector3(&mut dst, &src, &matrix);
        assert_vec3_approx_eq(dst[0], Vector3::new(2.0, 0.0, 0.0));
        assert_vec3_approx_eq(dst[1], Vector3::new(0.0, 2.0, 0.0));
        assert_vec3_approx_eq(dst[2], Vector3::new(0.0, 0.0, 2.0));
    }

    #[test]
    fn transform_vector3_to_vector4_writes_expected_results() {
        let src = vec![Vector3::new(1.0, 2.0, 3.0), Vector3::new(-1.0, 0.5, 4.0)];
        let mut dst = vec![Vector4::ZERO; src.len()];
        let matrix = Matrix4::IDENTITY;
        VectorProcessor::transform_vector3_to_vector4(&mut dst, &src, &matrix);
        assert_eq!(dst[0], Vector4::new(1.0, 2.0, 3.0, 1.0));
        assert_eq!(dst[1], Vector4::new(-1.0, 0.5, 4.0, 1.0));
    }

    #[test]
    fn copy_generic_slices() {
        let src = vec![1, 2, 3, 4];
        let mut dst = vec![0; src.len()];
        VectorProcessor::copy(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn copy_vector3_with_per_vertex_alpha() {
        let src = vec![Vector3::new(1.0, 2.0, 3.0), Vector3::new(4.0, 5.0, 6.0)];
        let alpha = vec![0.5, 0.8];
        let mut dst = vec![Vector4::ZERO; src.len()];
        VectorProcessor::copy_vector3_to_vector4_with_alpha(&mut dst, &src, &alpha);
        assert_eq!(dst[0], Vector4::new(1.0, 2.0, 3.0, 0.5));
        assert_eq!(dst[1], Vector4::new(4.0, 5.0, 6.0, 0.8));
    }

    #[test]
    fn copy_vector3_with_uniform_alpha() {
        let src = vec![Vector3::new(-1.0, 0.0, 2.0); 3];
        let mut dst = vec![Vector4::ZERO; src.len()];
        VectorProcessor::copy_vector3_to_vector4_single_alpha(&mut dst, &src, 1.0);
        assert!(dst.iter().all(|v| *v == Vector4::new(-1.0, 0.0, 2.0, 1.0)));
    }

    #[test]
    fn copy_single_vector3_to_vector4_array() {
        let src = Vector3::new(1.0, -1.0, 0.5);
        let alpha = vec![0.2, 0.4, 0.6];
        let mut dst = vec![Vector4::ZERO; alpha.len()];
        VectorProcessor::copy_single_vector3_to_vector4_array(&mut dst, &src, &alpha);
        assert_eq!(dst[0], Vector4::new(1.0, -1.0, 0.5, 0.2));
        assert_eq!(dst[1], Vector4::new(1.0, -1.0, 0.5, 0.4));
        assert_eq!(dst[2], Vector4::new(1.0, -1.0, 0.5, 0.6));
    }

    #[test]
    fn copy_indexed_writes_elements_in_order() {
        let src = vec![10, 20, 30, 40, 50];
        let indices = vec![0, 2, 4, 1];
        let mut dst = vec![0; indices.len()];
        VectorProcessor::copy_indexed(&mut dst, &src, &indices);
        assert_eq!(dst, vec![10, 30, 50, 20]);
    }

    #[test]
    fn clamp_vector4_from_separate_src() {
        let src = vec![
            Vector4::new(-2.0, 0.5, 1.5, 3.0),
            Vector4::new(0.2, -1.0, 2.5, 0.8),
        ];
        let mut dst = vec![Vector4::ZERO; src.len()];
        VectorProcessor::clamp_vector4(&mut dst, &src, 0.0, 2.0);
        assert_eq!(dst[0], Vector4::new(0.0, 0.5, 1.5, 2.0));
        assert_eq!(dst[1], Vector4::new(0.2, 0.0, 2.0, 0.8));
    }

    #[test]
    fn clamp_vector4_in_place_clamps_all_components() {
        let mut values = vec![
            Vector4::new(-2.0, 3.0, 0.5, 1.5),
            Vector4::new(1.2, -0.1, 2.5, -5.0),
        ];
        VectorProcessor::clamp_vector4_in_place(&mut values, 0.0, 2.0);
        assert_eq!(values[0], Vector4::new(0.0, 2.0, 0.5, 1.5));
        assert_eq!(values[1], Vector4::new(1.2, 0.0, 2.0, 0.0));
    }

    #[test]
    fn clear_vector3_sets_all_to_zero() {
        let mut values = vec![Vector3::new(1.0, -1.0, 2.0); 3];
        VectorProcessor::clear_vector3(&mut values);
        assert!(values.iter().all(|v| *v == Vector3::ZERO));
    }

    #[test]
    fn normalize_vector3_normalizes_each_entry() {
        let mut values = vec![
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, -3.0, 0.0),
            Vector3::new(0.0, 0.0, 4.0),
        ];
        VectorProcessor::normalize_vector3(&mut values);
        assert_vec3_approx_eq(values[0], Vector3::new(1.0, 0.0, 0.0));
        assert_vec3_approx_eq(values[1], Vector3::new(0.0, -1.0, 0.0));
        assert_vec3_approx_eq(values[2], Vector3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn normalize_vector3_leaves_zero_vector_unchanged() {
        let mut values = vec![Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0)];
        VectorProcessor::normalize_vector3(&mut values);
        assert_eq!(values[0], Vector3::ZERO);
        assert_eq!(values[1], Vector3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn min_max_vector3_matches_extrema() {
        let src = vec![
            Vector3::new(1.0, 5.0, -2.0),
            Vector3::new(-3.0, 2.0, 4.0),
            Vector3::new(2.0, -1.0, 3.0),
        ];
        let mut min = Vector3::ZERO;
        let mut max = Vector3::ZERO;
        VectorProcessor::min_max_vector3(&src, &mut min, &mut max);
        assert_eq!(min, Vector3::new(-3.0, -1.0, -2.0));
        assert_eq!(max, Vector3::new(2.0, 5.0, 4.0));
    }

    #[test]
    fn mul_add_float_array_updates_in_place() {
        let mut dest = vec![1.0, 2.0, 3.0, 4.0];
        VectorProcessor::mul_add_float_array(&mut dest, 2.0, 1.0);
        assert_eq!(dest, vec![3.0, 5.0, 7.0, 9.0]);
    }

    #[test]
    fn dot_product_streams_results_into_destination() {
        let a = Vector3::new(1.0, 0.5, -1.0);
        let b = vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 2.0, 1.0)];
        let mut dst = vec![0.0; b.len()];
        VectorProcessor::dot_product(&mut dst, &a, &b);
        assert_eq!(dst, vec![1.0, -1.5]);
    }

    #[test]
    fn clamp_min_float_array_writes_results() {
        let src = vec![-2.0, 0.5, 1.0, -1.5, 2.0];
        let mut dst = vec![0.0; src.len()];
        VectorProcessor::clamp_min_float_array(&mut dst, &src, 0.0);
        assert_eq!(dst, vec![0.0, 0.5, 1.0, 0.0, 2.0]);
    }

    #[test]
    fn power_float_array_writes_results() {
        let src = vec![1.0, 2.0, 3.0, 4.0];
        let mut dst = vec![0.0; src.len()];
        VectorProcessor::power_float_array(&mut dst, &src, 2.0);
        assert_eq!(dst, vec![1.0, 4.0, 9.0, 16.0]);
    }
}
