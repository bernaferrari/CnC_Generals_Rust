//! Animation Evaluation Tests
//!
//! Comprehensive tests for the animation evaluator system.
//! These tests validate bone transform evaluation, hierarchy application,
//! and GPU skinning data generation.

use glam::{Mat4, Quat, Vec3};
use ww3d_renderer_3d::animation_evaluator::{
    AnimationEvaluator, AnimationEvaluatorError, BoneTransformData, GPUSkinningData,
};

#[test]
fn test_animation_evaluator_creation() {
    let evaluator = AnimationEvaluator::new(64);

    // Should have 64 bones
    let transforms = evaluator.get_all_transforms();
    assert_eq!(transforms.len(), 64);

    // All should be identity transforms
    for transform in transforms {
        assert_eq!(transform.translation, Vec3::ZERO);
        assert_eq!(transform.rotation, Quat::IDENTITY);
        assert_eq!(transform.scale, Vec3::ONE);
        assert!(transform.visible);
        assert_eq!(transform.world_transform, Mat4::IDENTITY);
    }
}

#[test]
fn test_animation_evaluator_default() {
    let evaluator = AnimationEvaluator::default();

    // Default should be 64 bones
    let transforms = evaluator.get_all_transforms();
    assert_eq!(transforms.len(), 64);
}

#[test]
fn test_bone_count_clamping() {
    // Max bones is 256
    let evaluator = AnimationEvaluator::new(512);

    // Should be clamped to 256
    let transforms = evaluator.get_all_transforms();
    assert_eq!(transforms.len(), 256);
}

#[test]
fn test_get_bone_transform_valid() {
    let evaluator = AnimationEvaluator::new(8);

    // Should get valid transform at index 0
    let transform = evaluator.get_bone_transform(0);
    assert!(transform.is_ok());

    let t = transform.unwrap();
    assert_eq!(t.translation, Vec3::ZERO);
    assert_eq!(t.rotation, Quat::IDENTITY);
}

#[test]
fn test_get_bone_transform_out_of_range() {
    let evaluator = AnimationEvaluator::new(8);

    // Should fail at index >= 8
    let transform = evaluator.get_bone_transform(8);
    assert!(transform.is_err());

    match transform {
        Err(AnimationEvaluatorError::BoneIndexOutOfRange(idx)) => {
            assert_eq!(idx, 8);
        }
        _ => panic!("Expected BoneIndexOutOfRange error"),
    }
}

#[test]
fn test_gpu_skinning_data_creation() {
    let skinning = GPUSkinningData::new(32, 256);

    // Should have 32 bones
    assert_eq!(skinning.num_bones, 32);
    assert_eq!(skinning.max_bones, 256);
    assert_eq!(skinning.bone_matrices.len(), 32);
    assert_eq!(skinning.inverse_bind_matrices.len(), 32);

    // All matrices should be identity
    for matrix in &skinning.bone_matrices {
        assert_eq!(*matrix, Mat4::IDENTITY);
    }
    for matrix in &skinning.inverse_bind_matrices {
        assert_eq!(*matrix, Mat4::IDENTITY);
    }
}

#[test]
fn test_gpu_skinning_max_bones_clamping() {
    // Request 300 bones but max is 256
    let skinning = GPUSkinningData::new(300, 256);

    // Should be clamped to 256
    assert_eq!(skinning.num_bones, 256);
    assert_eq!(skinning.bone_matrices.len(), 256);
}

#[test]
fn test_set_bone_matrix_valid() {
    let mut skinning = GPUSkinningData::new(8, 256);

    let test_matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let result = skinning.set_bone_matrix(0, test_matrix);

    assert!(result.is_ok());

    // Verify it was set
    let retrieved = skinning.get_bone_matrix(0).unwrap();
    assert_eq!(retrieved, test_matrix);
}

#[test]
fn test_set_bone_matrix_out_of_range() {
    let mut skinning = GPUSkinningData::new(8, 256);

    let test_matrix = Mat4::IDENTITY;
    let result = skinning.set_bone_matrix(8, test_matrix);

    assert!(result.is_err());
    match result {
        Err(AnimationEvaluatorError::BoneIndexOutOfRange(idx)) => {
            assert_eq!(idx, 8);
        }
        _ => panic!("Expected BoneIndexOutOfRange error"),
    }
}

#[test]
fn test_get_bone_matrix_valid() {
    let mut skinning = GPUSkinningData::new(4, 256);

    let test_matrix = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));
    skinning.set_bone_matrix(2, test_matrix).unwrap();

    let retrieved = skinning.get_bone_matrix(2).unwrap();
    assert_eq!(retrieved, test_matrix);
}

#[test]
fn test_get_bone_matrix_out_of_range() {
    let skinning = GPUSkinningData::new(4, 256);

    let result = skinning.get_bone_matrix(4);
    assert!(result.is_err());
}

#[test]
fn test_bone_transform_data_identity() {
    let transform = BoneTransformData::identity();

    assert_eq!(transform.translation, Vec3::ZERO);
    assert_eq!(transform.rotation, Quat::IDENTITY);
    assert_eq!(transform.scale, Vec3::ONE);
    assert!(transform.visible);
    assert_eq!(transform.world_transform, Mat4::IDENTITY);
}

#[test]
fn test_bone_transform_local_matrix_identity() {
    let transform = BoneTransformData::identity();

    let local = transform.local_transform();
    assert_eq!(local, Mat4::IDENTITY);
}

#[test]
fn test_bone_transform_local_matrix_translation() {
    let transform = BoneTransformData {
        translation: Vec3::new(5.0, 10.0, 15.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
        visible: true,
        world_transform: Mat4::IDENTITY,
    };

    let local = transform.local_transform();
    let expected = Mat4::from_translation(Vec3::new(5.0, 10.0, 15.0));

    // Compare by converting to components
    assert!((local.w_axis.x - expected.w_axis.x).abs() < 0.001);
    assert!((local.w_axis.y - expected.w_axis.y).abs() < 0.001);
    assert!((local.w_axis.z - expected.w_axis.z).abs() < 0.001);
}

#[test]
fn test_bone_transform_local_matrix_rotation() {
    let angle = std::f32::consts::PI / 4.0;
    let rotation = Quat::from_axis_angle(Vec3::Y, angle);

    let transform = BoneTransformData {
        translation: Vec3::ZERO,
        rotation,
        scale: Vec3::ONE,
        visible: true,
        world_transform: Mat4::IDENTITY,
    };

    let local = transform.local_transform();

    // Verify rotation component is present
    assert_ne!(local, Mat4::IDENTITY);

    // Verify identity is preserved at origin
    let rotated = local.transform_point3(Vec3::ZERO);
    assert!((rotated - Vec3::ZERO).length() < 0.001);
}

#[test]
fn test_bone_transform_local_matrix_scale() {
    let transform = BoneTransformData {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::new(2.0, 3.0, 4.0),
        visible: true,
        world_transform: Mat4::IDENTITY,
    };

    let local = transform.local_transform();

    // Verify scale is applied
    let scaled_x = local.transform_vector3(Vec3::X);
    let scaled_y = local.transform_vector3(Vec3::Y);
    let scaled_z = local.transform_vector3(Vec3::Z);

    assert!((scaled_x.length() - 2.0).abs() < 0.001);
    assert!((scaled_y.length() - 3.0).abs() < 0.001);
    assert!((scaled_z.length() - 4.0).abs() < 0.001);
}

#[test]
fn test_bone_transform_local_matrix_combined() {
    // Create transform with translation, rotation, and scale
    let angle = std::f32::consts::PI / 6.0;
    let rotation = Quat::from_axis_angle(Vec3::Z, angle);

    let transform = BoneTransformData {
        translation: Vec3::new(10.0, 20.0, 30.0),
        rotation,
        scale: Vec3::new(2.0, 2.0, 2.0),
        visible: true,
        world_transform: Mat4::IDENTITY,
    };

    let local = transform.local_transform();

    // Verify translation component
    assert!((local.w_axis.x - 10.0).abs() < 0.001);
    assert!((local.w_axis.y - 20.0).abs() < 0.001);
    assert!((local.w_axis.z - 30.0).abs() < 0.001);

    // Verify scale is applied (unit X vector scaled by 2.0)
    let scaled_x = local.transform_vector3(Vec3::X);
    assert!((scaled_x.length() - 2.0).abs() < 0.001);
}

#[test]
fn test_animation_evaluator_frame_evaluation() {
    let mut evaluator = AnimationEvaluator::new(8);

    // Evaluate at frame 0
    let result = evaluator.evaluate_frame(0);
    assert!(result.is_ok());
    assert_eq!(evaluator.current_frame(), 0);
    assert!(!evaluator.is_dirty());
}

#[test]
fn test_animation_evaluator_frame_caching() {
    let mut evaluator = AnimationEvaluator::new(8);

    // First evaluation at frame 5
    let result1 = evaluator.evaluate_frame(5);
    assert!(result1.is_ok());

    // Second evaluation at same frame should return immediately (cached)
    let result2 = evaluator.evaluate_frame(5);
    assert!(result2.is_ok());

    // Current frame should be 5
    assert_eq!(evaluator.current_frame(), 5);
}

#[test]
fn test_animation_evaluator_different_frames() {
    let mut evaluator = AnimationEvaluator::new(8);

    // Evaluate at frame 3
    evaluator.evaluate_frame(3).unwrap();
    assert_eq!(evaluator.current_frame(), 3);

    // Evaluate at different frame 7
    evaluator.evaluate_frame(7).unwrap();
    assert_eq!(evaluator.current_frame(), 7);
}

#[test]
fn test_animation_evaluator_root_transform() {
    let mut evaluator = AnimationEvaluator::new(8);

    let transform = Mat4::from_translation(Vec3::new(100.0, 200.0, 300.0));
    evaluator.set_root_transform(transform);

    // Setting root should mark as dirty
    assert!(evaluator.is_dirty());
}

#[test]
fn test_animation_evaluator_mark_dirty() {
    let mut evaluator = AnimationEvaluator::new(8);

    // Not dirty initially after evaluation
    evaluator.evaluate_frame(0).unwrap();
    assert!(!evaluator.is_dirty());

    // Mark dirty
    evaluator.mark_dirty();
    assert!(evaluator.is_dirty());
}

#[test]
fn test_animation_evaluator_get_skinning_data() {
    let evaluator = AnimationEvaluator::new(32);

    let skinning = evaluator.get_skinning_data();
    assert_eq!(skinning.num_bones, 32);
    assert_eq!(skinning.max_bones, 256);
    assert_eq!(skinning.bone_matrices.len(), 32);
}

#[test]
fn test_multiple_evaluators() {
    let eval1 = AnimationEvaluator::new(32);
    let eval2 = AnimationEvaluator::new(64);
    let eval3 = AnimationEvaluator::new(128);

    // Each should have correct bone count
    assert_eq!(eval1.get_all_transforms().len(), 32);
    assert_eq!(eval2.get_all_transforms().len(), 64);
    assert_eq!(eval3.get_all_transforms().len(), 128);
}

#[test]
fn test_animation_evaluator_state_independence() {
    let mut eval1 = AnimationEvaluator::new(8);
    let mut eval2 = AnimationEvaluator::new(8);

    // Set different states
    eval1.evaluate_frame(5).unwrap();
    eval2.evaluate_frame(10).unwrap();

    // States should be independent
    assert_eq!(eval1.current_frame(), 5);
    assert_eq!(eval2.current_frame(), 10);

    // Modifying one shouldn't affect the other
    eval1.set_root_transform(Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0)));
    assert!(eval1.is_dirty());
    assert!(!eval2.is_dirty());
}

#[test]
fn test_bone_index_boundary() {
    let evaluator = AnimationEvaluator::new(16);

    // Index 15 should work
    assert!(evaluator.get_bone_transform(15).is_ok());

    // Index 16 should fail
    assert!(evaluator.get_bone_transform(16).is_err());
}

#[test]
fn test_gpu_skinning_all_bones_accessible() {
    let skinning = GPUSkinningData::new(64, 256);

    // All 64 bones should be accessible
    for i in 0..64 {
        let result = skinning.get_bone_matrix(i as u32);
        assert!(result.is_ok());
    }

    // 64 and beyond should fail
    assert!(skinning.get_bone_matrix(64).is_err());
}

#[test]
fn test_animation_evaluator_update_placeholder() {
    let mut evaluator = AnimationEvaluator::new(8);

    let result = evaluator.update_placeholder(42);
    assert!(result.is_ok());
    assert_eq!(evaluator.current_frame(), 42);
}

#[test]
fn test_bone_transform_visibility() {
    let mut transform = BoneTransformData::identity();
    assert!(transform.visible);

    transform.visible = false;
    assert!(!transform.visible);
}

#[test]
fn test_bone_transform_data_clone() {
    let transform1 = BoneTransformData {
        translation: Vec3::new(1.0, 2.0, 3.0),
        rotation: Quat::from_axis_angle(Vec3::Y, 1.0),
        scale: Vec3::new(2.0, 2.0, 2.0),
        visible: false,
        world_transform: Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0)),
    };

    let transform2 = transform1.clone();

    assert_eq!(transform2.translation, transform1.translation);
    assert_eq!(transform2.rotation, transform1.rotation);
    assert_eq!(transform2.scale, transform1.scale);
    assert_eq!(transform2.visible, transform1.visible);
    assert_eq!(transform2.world_transform, transform1.world_transform);
}

#[test]
fn test_animation_evaluator_current_frame_getter() {
    let mut evaluator = AnimationEvaluator::new(8);

    assert_eq!(evaluator.current_frame(), 0);

    evaluator.evaluate_frame(25).unwrap();
    assert_eq!(evaluator.current_frame(), 25);
}

#[test]
fn test_gpu_skinning_data_multiple_modifications() {
    let mut skinning = GPUSkinningData::new(8, 256);

    // Set multiple matrices
    for i in 0..8 {
        let matrix =
            Mat4::from_translation(Vec3::new(i as f32 * 10.0, i as f32 * 20.0, i as f32 * 30.0));
        skinning.set_bone_matrix(i, matrix).unwrap();
    }

    // Verify all were set correctly
    for i in 0..8 {
        let expected =
            Mat4::from_translation(Vec3::new(i as f32 * 10.0, i as f32 * 20.0, i as f32 * 30.0));
        let retrieved = skinning.get_bone_matrix(i).unwrap();
        assert_eq!(retrieved, expected);
    }
}

#[test]
fn test_bone_transform_visibility_toggle() {
    let mut transform = BoneTransformData::identity();

    // Toggle visibility
    assert!(transform.visible);
    transform.visible = false;
    assert!(!transform.visible);
    transform.visible = true;
    assert!(transform.visible);
}
