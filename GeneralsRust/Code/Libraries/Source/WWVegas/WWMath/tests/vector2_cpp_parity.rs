//! Original portable `vector2.h` results with floating-point contraction disabled.

use math_utilities::{
    Vector2,
    vec2_extensions::{Vec2Extensions, perp_dot_product},
    vector2::{Vector2Ext, normalize},
};

fn bits(vector: Vector2) -> [u32; 2] {
    vector.to_array().map(f32::to_bits)
}

#[test]
fn original_turns_use_the_clockwise_perpendicular_and_preserve_opposite_ties() {
    assert_eq!(perp_dot_product(Vector2::X, Vector2::Y), -1.0);
    assert_eq!(Vector2::Y.perp_dot_product(Vector2::X), 1.0);
    for (target, reached, positive, expected) in [
        (Vector2::Y, false, true, [0x3f5d_b3d7, 0x3f00_0000]),
        (Vector2::NEG_Y, false, false, [0x3f5d_b3d7, 0xbf00_0000]),
        (Vector2::X, true, false, [0x3f80_0000, 0]),
        (Vector2::NEG_X, false, false, [0x3f5d_b3d7, 0xbf00_0000]),
    ] {
        let mut heading = Vector2::X;
        let mut positive_turn = !positive;
        assert_eq!(
            heading.rotate_towards_vector_with_sin_cos(
                &target,
                0.5,
                0.866_025_4,
                &mut positive_turn,
            ),
            reached
        );
        assert_eq!(positive_turn, positive);
        assert_eq!(bits(heading), expected);
    }

    let mut heading = Vector2::X;
    let mut positive = false;
    assert!(
        heading.rotate_towards_vector(&Vector2::Y, std::f32::consts::FRAC_PI_2, &mut positive,)
    );
    assert!(positive);
    assert_eq!(heading, Vector2::Y);

    let mut vector = Vector2::new(1.125, -2.75);
    Vector2Ext::rotate(&mut vector, 0.3725);
    assert_eq!(bits(vector), [0x4003_1dda, 0xc009_b9d8]);
    let mut zero = Vector2::splat(-0.0);
    zero.rotate_with_sin_cos(0.0, 1.0);
    assert_eq!(bits(zero), [0, 0x8000_0000]);
}

#[test]
fn portable_normalization_preserves_the_two_original_overloads() {
    for (mut vector, free, in_place) in [
        (
            Vector2::new(3.0, 4.0),
            [0x4170_0000, 0x41a0_0000],
            [0x3f19_999a, 0x3f4c_cccd],
        ),
        (Vector2::new(-0.0, 0.0), [0; 2], [0x8000_0000, 0]),
        (
            Vector2::new(-1e-30, 2e-30),
            [0; 2],
            [0x8da2_4260, 0x0e22_4260],
        ),
        (
            Vector2::new(0.1234, 0.00315),
            [0x3c79_91d3, 0x39cb_dcc5],
            [0x3f7f_eaa8, 0x3cd1_0c06],
        ),
    ] {
        assert_eq!(bits(normalize(vector)), free);
        vector.normalize_in_place();
        assert_eq!(bits(vector), in_place);
    }
}

#[test]
fn interpolation_distance_and_component_operations_keep_original_order() {
    let a = Vector2::new(-5.0625, 3.0);
    let b = Vector2::new(2.9375, -16.5);
    let mut result = Vector2::NAN;
    Vector2::lerp_into(&mut result, a, b, 0.3725);
    assert_eq!(bits(result), [0xc005_47ae, 0xc088_70a4]);
    Vector2::lerp_into(&mut result, a, b, -1.25);
    assert_eq!(bits(result), [0xc171_0000, 0x41db_0000]);

    assert_eq!(Vector2::X.distance(Vector2::Y).to_bits(), 0x3fb5_04f3);
    assert_eq!(Vector2::X.quick_distance(Vector2::Y), 0.5);
    let target = Vector2::new(2.0, 4.0);
    assert_eq!(Vector2::ZERO.quick_distance(target), -4.0);
    assert_eq!(target.quick_distance(Vector2::ZERO), 5.0);
    assert!(!Vector2::X.equal_within_epsilon(Vector2::Y, 1.0));
    assert!(Vector2::X.equal_within_epsilon(Vector2::Y, 1.01));
    assert!(!Vector2::X.equal_within_epsilon(Vector2::X, 0.0));

    result.set(-1.25, 2.5);
    result.scale(4.0, -2.0);
    assert_eq!(bits(result), [0xc0a0_0000, 0xc0a0_0000]);
    result.scale_by_vector(&Vector2::new(0.0, 0.5));
    assert_eq!(bits(result), [0x8000_0000, 0xc020_0000]);
    let mut bounds = Vector2::ZERO;
    bounds.set_u(0.0);
    bounds.set_v(-0.0);
    assert_eq!(
        [bounds.u().to_bits(), bounds.v().to_bits()],
        [0, 0x8000_0000]
    );
    bounds.update_min(&Vector2::new(-0.0, 0.0));
    bounds.update_max(&Vector2::new(-0.0, 0.0));
    assert_eq!(bits(bounds), [0, 0x8000_0000]);

    Vector2::swap(&mut result, &mut bounds);
    assert_eq!(bits(bounds), [0x8000_0000, 0xc020_0000]);
    result.set_from(bounds);
    assert_eq!(bits(result), bits(bounds));
    assert!(Vector2Ext::is_valid(&result));
    assert!(!Vector2Ext::is_valid(&Vector2::NAN));
}
