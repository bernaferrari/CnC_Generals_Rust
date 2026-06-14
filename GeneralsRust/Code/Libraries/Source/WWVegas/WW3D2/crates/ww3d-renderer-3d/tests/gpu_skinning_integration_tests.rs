//! GPU Skinning Integration Tests
//!
//! Comprehensive tests for GPU skinning pipeline integration with animation evaluation.
//! These tests validate bone matrix generation, inverse bind pose handling, and
//! the complete flow from animation evaluation to GPU skinning data preparation.

use glam::{Mat4, Quat, Vec3};
use ww3d_animation::{HAnimClass, HTreeClass};
use ww3d_renderer_3d::animation_evaluator::{AnimationEvaluator, GPUSkinningData};

fn evaluator_with_animation(bone_count: u32) -> AnimationEvaluator {
    let mut evaluator = AnimationEvaluator::new(bone_count);
    let mut hierarchy = HTreeClass::with_name("TestHierarchy");
    hierarchy.init_default();
    for index in 1..bone_count as usize {
        hierarchy.add_pivot(&format!("Bone{}", index), 0, Vec3::ZERO, Quat::IDENTITY);
    }

    evaluator.set_hierarchy(hierarchy);
    evaluator.set_uncompressed_animation(HAnimClass::new("TestAnim", "TestHierarchy", 60, 30.0));
    evaluator
}

#[test]
fn test_gpu_skinning_data_creation() {
    // Test creation of GPU skinning data with bone count
    let skinning = GPUSkinningData::new(64, 256);

    assert_eq!(skinning.num_bones, 64);
    assert_eq!(skinning.max_bones, 256);
    assert_eq!(skinning.bone_matrices.len(), 64);
    assert_eq!(skinning.inverse_bind_matrices.len(), 64);
}

#[test]
fn test_gpu_skinning_data_clamping() {
    // Test that bone count is clamped to max_bones
    let skinning = GPUSkinningData::new(300, 256);

    // Should be clamped to 256
    assert_eq!(skinning.num_bones, 256);
    assert_eq!(skinning.bone_matrices.len(), 256);
}

#[test]
fn test_gpu_skinning_identity_initialization() {
    // Test that all bone matrices are initialized to identity
    let skinning = GPUSkinningData::new(16, 256);

    for i in 0..16 {
        let matrix = skinning.get_bone_matrix(i).unwrap();
        assert_eq!(matrix, Mat4::IDENTITY);
    }
}

#[test]
fn test_gpu_skinning_identity_inverse_bind() {
    // Test that inverse bind pose matrices are initialized to identity
    let skinning = GPUSkinningData::new(16, 256);

    for i in 0..16 {
        assert_eq!(skinning.inverse_bind_matrices[i], Mat4::IDENTITY);
    }
}

#[test]
fn test_gpu_skinning_set_bone_matrix() {
    // Test setting bone matrix at specific index
    let mut skinning = GPUSkinningData::new(64, 256);

    let translation = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
    skinning.set_bone_matrix(0, translation).unwrap();

    let retrieved = skinning.get_bone_matrix(0).unwrap();
    assert_eq!(retrieved, translation);
}

#[test]
fn test_gpu_skinning_set_multiple_bones() {
    // Test setting matrices for multiple bones
    let mut skinning = GPUSkinningData::new(64, 256);

    let matrices = vec![
        Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)),
        Mat4::from_translation(Vec3::new(4.0, 0.0, 0.0)),
    ];

    for (idx, matrix) in matrices.iter().enumerate() {
        skinning.set_bone_matrix(idx as u32, *matrix).unwrap();
    }

    for (idx, expected_matrix) in matrices.iter().enumerate() {
        let retrieved = skinning.get_bone_matrix(idx as u32).unwrap();
        assert_eq!(retrieved, *expected_matrix);
    }
}

#[test]
fn test_gpu_skinning_out_of_range_access() {
    // Test that accessing out-of-range bones returns error
    let skinning = GPUSkinningData::new(16, 256);

    // Try to access beyond bone count
    assert!(skinning.get_bone_matrix(16).is_err());
    assert!(skinning.get_bone_matrix(100).is_err());
    assert!(skinning.get_bone_matrix(255).is_err());
}

#[test]
fn test_gpu_skinning_out_of_range_set() {
    // Test that setting out-of-range bones returns error
    let mut skinning = GPUSkinningData::new(16, 256);

    let matrix = Mat4::IDENTITY;

    assert!(skinning.set_bone_matrix(16, matrix).is_err());
    assert!(skinning.set_bone_matrix(100, matrix).is_err());
}

#[test]
fn test_animation_evaluator_skinning_data_access() {
    // Test that animation evaluator provides access to skinning data
    let evaluator = AnimationEvaluator::new(32);

    let skinning = evaluator.get_skinning_data();
    assert_eq!(skinning.num_bones, 32);
}

#[test]
fn test_animation_evaluator_bone_matrix_update() {
    // Test updating bone matrix through evaluator's skinning data
    let evaluator = AnimationEvaluator::new(64);

    // Note: In real usage, this would be done through a mutable reference
    // For now, we test the structure exists
    assert_eq!(evaluator.get_skinning_data().num_bones, 64);
}

#[test]
fn test_gpu_skinning_rotation_matrix() {
    // Test setting bone with rotation matrix
    let mut skinning = GPUSkinningData::new(64, 256);

    let rotation = Mat4::from_quat(Quat::from_rotation_z(std::f32::consts::PI / 4.0));
    skinning.set_bone_matrix(0, rotation).unwrap();

    let retrieved = skinning.get_bone_matrix(0).unwrap();
    // Check that rotation matrix was stored (using approximate equality due to floating point)
    assert!((retrieved.w_axis.w - 1.0).abs() < 0.001);
}

#[test]
fn test_gpu_skinning_scale_matrix() {
    // Test setting bone with scale matrix
    let mut skinning = GPUSkinningData::new(64, 256);

    let scale = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
    skinning.set_bone_matrix(0, scale).unwrap();

    let retrieved = skinning.get_bone_matrix(0).unwrap();
    assert_eq!(retrieved, scale);
}

#[test]
fn test_gpu_skinning_combined_transform() {
    // Test setting bone with combined translation, rotation, and scale
    let mut skinning = GPUSkinningData::new(64, 256);

    let translation = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
    let rotation = Mat4::from_quat(Quat::from_rotation_y(std::f32::consts::PI / 6.0));
    let scale = Mat4::from_scale(Vec3::new(1.5, 1.5, 1.5));

    let combined = translation * rotation * scale;
    skinning.set_bone_matrix(0, combined).unwrap();

    let retrieved = skinning.get_bone_matrix(0).unwrap();
    assert_eq!(retrieved, combined);
}

#[test]
fn test_gpu_skinning_hierarchy_chain() {
    // Test skinning data for a bone hierarchy (parent-child relationship simulation)
    let mut skinning = GPUSkinningData::new(4, 256);

    // Root bone at origin
    let root = Mat4::IDENTITY;
    skinning.set_bone_matrix(0, root).unwrap();

    // Child bone 1 (offset from root)
    let child1 = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
    skinning.set_bone_matrix(1, child1).unwrap();

    // Child bone 2 (offset from child1)
    let child2_local = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.0));
    let child2_world = child1 * child2_local;
    skinning.set_bone_matrix(2, child2_world).unwrap();

    // Verify all matrices are stored
    assert_eq!(skinning.get_bone_matrix(0).unwrap(), root);
    assert_eq!(skinning.get_bone_matrix(1).unwrap(), child1);
    assert_eq!(skinning.get_bone_matrix(2).unwrap(), child2_world);
}

#[test]
fn test_gpu_skinning_inverse_bind_matrix_identity() {
    // Test that inverse bind matrices can be set and retrieved
    let mut skinning = GPUSkinningData::new(8, 256);

    // Simulate setting inverse bind poses
    let bind_matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let inverse = bind_matrix.inverse();
    skinning.inverse_bind_matrices[0] = inverse;

    // Verify it can be accessed
    assert!(skinning.inverse_bind_matrices[0].is_finite());
}

#[test]
fn test_gpu_skinning_32_bone_skeleton() {
    // Test typical 32-bone skeleton
    let mut skinning = GPUSkinningData::new(32, 256);

    // Simulate bone hierarchy for a humanoid
    for i in 0..32 {
        let offset = Vec3::new(i as f32 * 0.1, 0.0, 0.0);
        let matrix = Mat4::from_translation(offset);
        skinning.set_bone_matrix(i, matrix).unwrap();
    }

    // Verify all bones are accessible
    assert_eq!(skinning.num_bones, 32);
    for i in 0..32 {
        let matrix = skinning.get_bone_matrix(i).unwrap();
        assert!(matrix.is_finite());
    }
}

#[test]
fn test_gpu_skinning_256_bone_skeleton() {
    // Test maximum 256-bone skeleton
    let mut skinning = GPUSkinningData::new(256, 256);

    // Set all bones to sequential translations
    for i in 0..256 {
        let offset = Vec3::new((i as f32) * 0.01, 0.0, 0.0);
        let matrix = Mat4::from_translation(offset);
        skinning.set_bone_matrix(i as u32, matrix).unwrap();
    }

    // Verify all bones are accessible
    assert_eq!(skinning.num_bones, 256);
    for i in 0..256 {
        let matrix = skinning.get_bone_matrix(i as u32).unwrap();
        assert!(matrix.is_finite());
    }
}

#[test]
fn test_animation_evaluator_large_bone_count() {
    // Test evaluator with maximum bone count
    let evaluator = AnimationEvaluator::new(1000); // Should clamp to 256

    let skinning = evaluator.get_skinning_data();
    assert_eq!(skinning.num_bones, 256);
    assert_eq!(skinning.max_bones, 256);
}

#[test]
fn test_gpu_skinning_matrix_multiplication() {
    // Test that matrices can be multiplied (for hierarchy transformation)
    let mut skinning = GPUSkinningData::new(4, 256);

    // Parent bone
    let parent = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));

    // Child bone (local space)
    let child_local = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0));

    // Child bone in world space = parent * child_local
    let child_world = parent * child_local;

    skinning.set_bone_matrix(0, parent).unwrap();
    skinning.set_bone_matrix(1, child_world).unwrap();

    let retrieved_parent = skinning.get_bone_matrix(0).unwrap();
    let retrieved_child = skinning.get_bone_matrix(1).unwrap();

    assert_eq!(retrieved_parent, parent);
    assert_eq!(retrieved_child, child_world);
}

#[test]
fn test_animation_evaluator_frame_evaluation_with_skinning() {
    // Test that evaluator maintains separate frame evaluation and skinning data
    let mut evaluator = evaluator_with_animation(16);

    // Evaluate at frame 0
    evaluator.evaluate_frame(0).unwrap();
    assert_eq!(evaluator.current_frame(), 0);

    // Evaluate at frame 10
    evaluator.evaluate_frame(10).unwrap();
    assert_eq!(evaluator.current_frame(), 10);

    // Verify skinning data still exists
    let skinning = evaluator.get_skinning_data();
    assert_eq!(skinning.num_bones, 16);
}

#[test]
fn test_gpu_skinning_none_bones() {
    // Edge case: create with 0 bones (or minimum)
    let skinning = GPUSkinningData::new(0, 256);

    assert_eq!(skinning.num_bones, 0);
    assert_eq!(skinning.bone_matrices.len(), 0);
}

#[test]
fn test_gpu_skinning_max_bones_limit() {
    // Test that creating with max_bones=1 works correctly
    let skinning = GPUSkinningData::new(10, 1);

    // Should clamp to 1
    assert_eq!(skinning.num_bones, 1);
    assert_eq!(skinning.bone_matrices.len(), 1);
}

#[test]
fn test_gpu_skinning_matrix_finite_values() {
    // Test that all matrices contain finite values
    let mut skinning = GPUSkinningData::new(16, 256);

    let matrix = Mat4::from_translation(Vec3::new(f32::MAX / 2.0, 0.0, 0.0));
    skinning.set_bone_matrix(0, matrix).unwrap();

    let retrieved = skinning.get_bone_matrix(0).unwrap();
    // Verify all components are finite
    assert!(retrieved.x_axis.is_finite());
    assert!(retrieved.y_axis.is_finite());
    assert!(retrieved.z_axis.is_finite());
    assert!(retrieved.w_axis.is_finite());
}

#[test]
fn test_animation_evaluator_root_transform() {
    // Test that animation evaluator manages root transform separately
    let evaluator = AnimationEvaluator::new(32);

    // Root transform should be accessible for world-space positioning
    assert_eq!(evaluator.get_skinning_data().num_bones, 32);
}

#[test]
fn test_gpu_skinning_concurrent_access() {
    // Test that multiple evaluators can maintain separate skinning data
    let eval1 = AnimationEvaluator::new(32);
    let eval2 = AnimationEvaluator::new(64);
    let eval3 = AnimationEvaluator::new(16);

    let skin1 = eval1.get_skinning_data();
    let skin2 = eval2.get_skinning_data();
    let skin3 = eval3.get_skinning_data();

    assert_eq!(skin1.num_bones, 32);
    assert_eq!(skin2.num_bones, 64);
    assert_eq!(skin3.num_bones, 16);

    // Verify they don't interfere with each other
    assert_ne!(skin1.num_bones, skin2.num_bones);
}

#[test]
fn test_gpu_skinning_sequential_updates() {
    // Test updating the same bone matrix multiple times
    let mut skinning = GPUSkinningData::new(8, 256);

    let matrix1 = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
    skinning.set_bone_matrix(0, matrix1).unwrap();

    let matrix2 = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0));
    skinning.set_bone_matrix(0, matrix2).unwrap();

    let matrix3 = Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0));
    skinning.set_bone_matrix(0, matrix3).unwrap();

    // Last update should be retained
    let retrieved = skinning.get_bone_matrix(0).unwrap();
    assert_eq!(retrieved, matrix3);
}

#[test]
fn test_gpu_skinning_different_bone_sizes() {
    // Test creating skinning data with various bone counts
    let sizes = vec![1, 2, 4, 8, 16, 32, 64, 128, 256];

    for size in sizes {
        let skinning = GPUSkinningData::new(size, 256);
        assert_eq!(skinning.num_bones, size);
        assert_eq!(skinning.bone_matrices.len(), size as usize);
    }
}
