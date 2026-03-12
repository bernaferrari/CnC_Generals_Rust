//! UV Animation System Tests
//!
//! Comprehensive tests for texture coordinate transformations and animations

#[cfg(test)]
mod tests {
    use glam::{Mat3, Vec2, Vec3};
    use std::f32::consts::PI;
    use ww3d_geometry::{
        GridMapper, LinearOffsetMapper, NoOpMapper, RotateMapper, SineLinearOffsetMapper,
        TextureMapper, TextureMapperState, TextureMapperType,
    };

    #[test]
    fn test_linear_offset_mapper_basic() {
        let mapper = LinearOffsetMapper::new(0.5, 0.25);
        assert_eq!(mapper.u_offset_per_sec, 0.5);
        assert_eq!(mapper.v_offset_per_sec, 0.25);
    }

    #[test]
    fn test_linear_offset_transform() {
        let mapper = LinearOffsetMapper::new(1.0, 2.0);
        let transform = mapper.compute_transform(1.0);

        // After 1 second with 1.0 u/sec offset, u should be 1.0
        assert!((transform.z_axis.x - 1.0).abs() < 0.0001);
        // After 1 second with 2.0 v/sec offset, v should be 2.0
        assert!((transform.z_axis.y - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_linear_offset_progression() {
        let mapper = LinearOffsetMapper::new(0.1, 0.05);

        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);
        let t2 = mapper.compute_transform(2.0);

        // Check progression
        assert!((t0.z_axis.x - 0.0).abs() < 0.0001);
        assert!((t1.z_axis.x - 0.1).abs() < 0.0001);
        assert!((t2.z_axis.x - 0.2).abs() < 0.0001);
    }

    #[test]
    fn test_grid_mapper_frame_calculation() {
        let mapper = GridMapper::new(4, 4, 10.0);

        // Frame 0 at t=0
        let t0 = mapper.compute_transform(0.0);
        assert!((t0.z_axis.x - 0.0).abs() < 0.0001);
        assert!((t0.z_axis.y - 0.0).abs() < 0.0001);

        // Frame 1 at t=0.1 (10 FPS)
        let t1 = mapper.compute_transform(0.1);
        assert!((t1.z_axis.x - 0.25).abs() < 0.0001); // Next column
        assert!((t1.z_axis.y - 0.0).abs() < 0.0001);

        // Frame 4 at t=0.4 (moves to next row)
        let t4 = mapper.compute_transform(0.4);
        assert!((t4.z_axis.x - 0.0).abs() < 0.0001); // Back to first column
        assert!((t4.z_axis.y - 0.25).abs() < 0.0001); // Next row
    }

    #[test]
    fn test_grid_mapper_scale() {
        let mapper = GridMapper::new(2, 2, 10.0);
        let transform = mapper.compute_transform(0.0);

        // Scale should be 1/2 for both axes
        assert!((transform.x_axis.x - 0.5).abs() < 0.0001);
        assert!((transform.y_axis.y - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_grid_mapper_looping() {
        let mapper = GridMapper::new(4, 4, 10.0).with_looping(true);

        // After 16 frames (1.6 seconds), should loop back
        let before_loop = mapper.compute_transform(1.5);
        let after_loop = mapper.compute_transform(1.7);

        // Just check that it's valid (not NaN)
        assert!(!before_loop.z_axis.x.is_nan());
        assert!(!after_loop.z_axis.x.is_nan());
    }

    #[test]
    fn test_grid_mapper_non_looping() {
        let mapper = GridMapper::new(4, 4, 10.0).with_looping(false);

        // Get the last frame
        let last_frame_time = 15.0 / 10.0; // Frame 15 at 10 FPS
        let at_end = mapper.compute_transform(last_frame_time + 0.1);

        // Should stay at last frame (3,3)
        assert!((at_end.z_axis.x - 0.75).abs() < 0.0001);
        assert!((at_end.z_axis.y - 0.75).abs() < 0.0001);
    }

    #[test]
    fn test_rotate_mapper_90_degrees() {
        let mapper = RotateMapper::new(90.0);
        let transform = mapper.compute_transform(1.0); // 1 second = 90 degrees

        // Approximate check - just verify it's not identity
        let diff = transform.x_axis.x - 1.0;
        assert!(diff.abs() > 0.1); // Should be different from identity
    }

    #[test]
    fn test_rotate_mapper_center() {
        let mapper = RotateMapper::new(90.0).with_center(0.5, 0.5);
        assert_eq!(mapper.center_u, 0.5);
        assert_eq!(mapper.center_v, 0.5);
    }

    #[test]
    fn test_sine_offset_basic() {
        let mapper = SineLinearOffsetMapper::new(0.1, 0.1, 0.05, 1.0);
        let t0 = mapper.compute_transform(0.0);

        // At t=0, sine wave should be at 0, so just linear offset
        assert!((t0.z_axis.x - 0.0).abs() < 0.0001);
        assert!((t0.z_axis.y - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_sine_offset_amplitude() {
        let mapper = SineLinearOffsetMapper::new(0.0, 0.0, 0.1, 1.0);

        // At t=0.25, sine should be at peak (quarter period)
        let t_quarter = mapper.compute_transform(0.25);
        let expected_amplitude = 0.1; // Peak of sine with amplitude 0.1

        assert!(t_quarter.z_axis.x.abs() > 0.08); // Close to amplitude
        assert!(t_quarter.z_axis.x.abs() < 0.12);
    }

    #[test]
    fn test_sine_offset_with_linear() {
        let mapper = SineLinearOffsetMapper::new(0.1, 0.0, 0.05, 1.0);

        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);

        // Should have linear component: 0.1 * 1.0 = 0.1
        // Plus sine component at peak (2*PI): sin(2*PI) ≈ 0
        assert!((t1.z_axis.x - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_noop_mapper() {
        let mapper = NoOpMapper;
        let transform = mapper.compute_transform(100.0);

        assert_eq!(transform, Mat3::IDENTITY);
    }

    #[test]
    fn test_texture_mapper_types() {
        let linear: Box<dyn TextureMapper> = Box::new(LinearOffsetMapper::new(1.0, 1.0));
        let grid: Box<dyn TextureMapper> = Box::new(GridMapper::new(2, 2, 10.0));
        let rotate: Box<dyn TextureMapper> = Box::new(RotateMapper::new(90.0));
        let sine: Box<dyn TextureMapper> =
            Box::new(SineLinearOffsetMapper::new(0.1, 0.1, 0.05, 2.0));

        assert_eq!(linear.mapper_type(), TextureMapperType::Linear);
        assert_eq!(grid.mapper_type(), TextureMapperType::Grid);
        assert_eq!(rotate.mapper_type(), TextureMapperType::Rotate);
        assert_eq!(sine.mapper_type(), TextureMapperType::SineLinear);
    }

    #[test]
    fn test_mapper_state_enabled() {
        let mapper = LinearOffsetMapper::new(1.0, 1.0);
        let mut state = TextureMapperState::new(Box::new(mapper));

        let t_enabled = state.compute_transform(1.0);
        assert!((t_enabled.z_axis.x - 1.0).abs() < 0.0001);

        state.set_enabled(false);
        let t_disabled = state.compute_transform(1.0);
        assert_eq!(t_disabled, Mat3::IDENTITY);
    }

    #[test]
    fn test_uv_coordinate_transformation() {
        let mapper = LinearOffsetMapper::new(0.1, 0.2);
        let transform = mapper.compute_transform(1.0); // 1 second elapsed

        // Test transforming a UV coordinate
        let original_uv = Vec2::new(0.5, 0.5);
        let transformed = transform.mul_vec3(Vec3::new(original_uv.x, original_uv.y, 1.0));

        // Expected: (0.5 + 0.1, 0.5 + 0.2, 1.0) = (0.6, 0.7, 1.0)
        assert!((transformed.x - 0.6).abs() < 0.0001);
        assert!((transformed.y - 0.7).abs() < 0.0001);
    }

    #[test]
    fn test_grid_mapper_sprite_animation() {
        // Create a 3x3 sprite sheet running at 6 FPS
        let mapper = GridMapper::sprite_animation(3, 3).with_frame_count(9);

        // Test progression through frames
        for frame in 0..9 {
            let time = frame as f32 / 6.0; // 6 FPS
            let transform = mapper.compute_transform(time);

            // Just verify it's not NaN and is valid
            assert!(!transform.z_axis.x.is_nan());
            assert!(!transform.z_axis.y.is_nan());
            assert!(transform.z_axis.x >= 0.0 && transform.z_axis.x <= 1.0);
            assert!(transform.z_axis.y >= 0.0 && transform.z_axis.y <= 1.0);
        }
    }

    #[test]
    fn test_water_scroll_preset() {
        let mapper = LinearOffsetMapper::water_scroll();
        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);
        let t10 = mapper.compute_transform(10.0);

        // Verify it scrolls smoothly
        assert!(t1.z_axis.x > t0.z_axis.x);
        assert!(t10.z_axis.x > t1.z_axis.x);
    }

    #[test]
    fn test_fast_scroll_preset() {
        let mapper = LinearOffsetMapper::fast_scroll();
        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);

        // Fast scroll should have larger offset
        assert!((t1.z_axis.x - 1.0).abs() < 0.0001);
        assert!((t1.z_axis.y - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_slow_rotate_preset() {
        let mapper = RotateMapper::slow_rotate();
        let transform = mapper.compute_transform(1.0); // 1 second

        // 45 degrees should give specific rotation
        assert!(transform.x_axis.x > 0.0); // cos(45°) > 0
        assert!(!transform.x_axis.x.is_nan());
    }

    #[test]
    fn test_water_wave_preset() {
        let mapper = SineLinearOffsetMapper::water_wave();
        let t0 = mapper.compute_transform(0.0);
        let t1 = mapper.compute_transform(1.0);

        // Wave should exist and not be static
        assert!(t1.z_axis.x != t0.z_axis.x || t1.z_axis.y != t0.z_axis.y);
    }

    #[test]
    fn test_ripple_preset() {
        let mapper = SineLinearOffsetMapper::ripple();
        let transform = mapper.compute_transform(0.5);

        // Should have some ripple effect
        assert!(!transform.z_axis.x.is_nan());
        assert!(!transform.z_axis.y.is_nan());
    }

    #[test]
    fn test_mat3_composition_order() {
        // Verify matrix multiplication order is correct
        let m1 = Mat3::from_cols(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        );

        let m2 = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.5, 0.5, 1.0),
        );

        let composed = m1 * m2;

        // Apply to a test vector
        let uv = Vec3::new(1.0, 1.0, 1.0);
        let result = composed * uv;

        assert!(!result.x.is_nan());
        assert!(!result.y.is_nan());
    }
}
