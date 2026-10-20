//! Original colmathline/TriClass finite component contracts.
use super::*;
fn triangle(normal: Vector3) -> Triangle {
    Triangle::with_normal(Vector3::ZERO, Vector3::X, Vector3::Y, normal)
}
#[test]
fn original_line_triangle_uses_supplied_normal_and_exact_nonzero_denominator() {
    let tri = triangle(Vector3::new(0., 0., 2.));
    let line = LineSegment::new(
        Vector3::new(0.25, 0.25, 1e-8),
        Vector3::new(0.25, 0.25, -1e-8),
    );
    let mut result = CastResult {
        compute_contact_point: true,
        ..Default::default()
    };
    assert!(CollisionMath::collide_line_triangle(
        &line,
        &tri,
        &mut result
    ));
    assert_eq!(result.fraction, 0.5);
    assert_eq!(result.normal, Vector3::new(0., 0., 2.));
    assert_eq!(result.contact_point, Vector3::new(0.25, 0.25, 0.));
}
#[test]
fn original_line_triangle_is_two_sided_and_strict_at_existing_fraction() {
    let tri = triangle(Vector3::Z);
    for (a, b) in [(1., -1.), (-1., 1.)] {
        let line = LineSegment::new(Vector3::new(0.25, 0.25, a), Vector3::new(0.25, 0.25, b));
        let mut result = CastResult::default();
        assert!(CollisionMath::collide_line_triangle(
            &line,
            &tri,
            &mut result
        ));
        assert_eq!(result.fraction, 0.5);
    }
    let endpoint = LineSegment::new(Vector3::new(0.25, 0.25, 1.), Vector3::new(0.25, 0.25, 0.));
    let mut result = CastResult::default();
    assert!(!CollisionMath::collide_line_triangle(
        &endpoint,
        &tri,
        &mut result
    ));
    assert_eq!(result.fraction, 1.);
    let start = LineSegment::new(Vector3::new(0.25, 0.25, 0.), Vector3::new(0.25, 0.25, 1.));
    assert!(CollisionMath::collide_line_triangle(
        &start,
        &tri,
        &mut result
    ));
    assert_eq!(result.fraction, 0.);
    assert!(!result.start_bad, "triangle routine never writes StartBad");
}
#[test]
fn original_inside_aabox_changes_start_bad_only() {
    let line = LineSegment::new(Vector3::ZERO, Vector3::new(3., 0., 0.));
    let box_ref = AABox::new(Vector3::ZERO, Vector3::ONE);
    let mut result = CastResult {
        fraction: 0.75,
        normal: Vector3::Y,
        contact_point: Vector3::new(7., 8., 9.),
        compute_contact_point: true,
        surface_type: 42,
        ..Default::default()
    };
    assert!(CollisionMath::collide_line_aabox(
        &line,
        &box_ref,
        &mut result
    ));
    assert!(result.start_bad);
    assert_eq!(result.fraction, 0.75);
    assert_eq!(result.normal, Vector3::Y);
    assert_eq!(result.contact_point, Vector3::new(7., 8., 9.));
    assert_eq!(result.surface_type, 42);
}
#[test]
fn original_dominant_plane_ties_and_zero_normal_are_preserved() {
    for (normal, axes) in [
        (Vector3::new(1., 1., 0.), (1, 2)),
        (Vector3::new(1., 0., 1.), (1, 2)),
        (Vector3::new(0., 1., 1.), (0, 2)),
    ] {
        assert_eq!(triangle(normal).find_dominant_plane(), axes);
    }
    let degenerate = Triangle::new(Vector3::ZERO, Vector3::X, Vector3::X * 2.);
    assert_eq!(degenerate.normal, Vector3::ZERO);
}

#[test]
fn original_inside_identity_and_rotated_obbox_change_start_bad_only() {
    let (sin, cos) = std::f32::consts::FRAC_PI_4.sin_cos();
    for basis in [
        crate::Matrix3::IDENTITY,
        crate::Matrix3 {
            row: [
                Vector3::new(cos, -sin, 0.),
                Vector3::new(sin, cos, 0.),
                Vector3::Z,
            ],
        },
    ] {
        let center = Vector3::new(7., 8., 9.);
        let obbox = OBBox::from_center_extent_basis(center, Vector3::new(2., 1., 0.5), basis);
        let inside = center + basis * Vector3::new(1.5, 0.5, 0.25);
        let line = LineSegment::new(inside, center + Vector3::new(4., 3., 2.));
        let mut result = CastResult {
            fraction: 0.375,
            normal: Vector3::X,
            contact_point: Vector3::new(-7., -8., -9.),
            compute_contact_point: true,
            surface_type: 99,
            ..Default::default()
        };
        assert!(CollisionMath::collide_line_obbox(
            &line,
            &obbox,
            &mut result
        ));
        assert!(result.start_bad);
        assert_eq!(result.fraction, 0.375);
        assert_eq!(result.normal, Vector3::X);
        assert_eq!(result.contact_point, Vector3::new(-7., -8., -9.));
        assert_eq!(result.surface_type, 99);
    }
}
