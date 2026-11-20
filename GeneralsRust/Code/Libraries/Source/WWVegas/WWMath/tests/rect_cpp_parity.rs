use math_utilities::{Rect, Vector2};
use std::mem::{align_of, offset_of, size_of};

fn bits(rect: Rect) -> [u32; 4] {
    [rect.left, rect.top, rect.right, rect.bottom].map(f32::to_bits)
}

#[test]
fn rectclass_layout_and_ordered_arithmetic_match_the_original_header() {
    assert_eq!(size_of::<Rect>(), 16);
    assert_eq!(align_of::<Rect>(), 4);
    assert_eq!(offset_of!(Rect, left), 0);
    assert_eq!(offset_of!(Rect, top), 4);
    assert_eq!(offset_of!(Rect, right), 8);
    assert_eq!(offset_of!(Rect, bottom), 12);

    let source = Rect::from_corners(Vector2::new(-3.5, -2.0), Vector2::new(7.25, 10.0));
    assert_eq!((source.width(), source.height()), (10.75, 12.0));
    assert_eq!(source.center(), Vector2::new(1.875, 4.0));
    assert_eq!(source.extent(), Vector2::new(5.375, 6.0));
    assert_eq!(source.upper_left(), Vector2::new(-3.5, -2.0));
    assert_eq!(source.upper_right(), Vector2::new(7.25, -2.0));
    assert_eq!(source.lower_left(), Vector2::new(-3.5, 10.0));
    assert_eq!(source.lower_right(), Vector2::new(7.25, 10.0));
    assert!(source.contains(source.upper_left()));
    assert!(source.contains(source.lower_right()));
    assert!(!source.contains(Vector2::new(7.25, 10.01)));
    assert!(!source.contains(Vector2::new(-3.5001, -2.0)));

    let mut rect = Rect::default();
    rect.set_from_rect(&source);
    assert_eq!(rect, source);
    rect.scale_relative_center(-2.25);
    assert_eq!(bits(rect), [0x415f8000, 0x418c0000, 0xc1238000, 0xc1180000]);

    rect.set_from_corners(Vector2::new(1.25, -0.125), Vector2::new(5.5, 3.75));
    rect *= 1.75;
    rect /= 3.5;
    rect.scale_vec(Vector2::new(-2.0, 3.0));
    rect.inverse_scale(Vector2::new(2.75, -8.0));
    rect += Vector2::new(-3.25, 17.0);
    rect -= Vector2::new(-0.125, 2.5);
    rect.inflate(Vector2::new(3.75, -1.5));
    assert_eq!(bits(rect), [0xc0ea8ba3, 0x41803000, 0xbfb00000, 0x4144c000]);
}

#[test]
fn rectclass_snapping_and_union_match_the_original_comparison_order() {
    let mut rect = Rect::new(-5.7, -3.2, -0.72, 6.8);
    rect.snap_to_units(Vector2::new(2.0, 1.25));
    assert_eq!(bits(rect), [0xc0800000, 0xc0200000, 0, 0x40c80000]);

    rect.set(-5.0, 0.0, 0.0, 1.0);
    let mut other = Rect::new(-2.0, -0.0, -0.0, 2.0);
    rect += other;
    assert_eq!(bits(rect), [0xc0a00000, 0x80000000, 0x80000000, 0x40000000]);

    other.set(f32::NAN, 3.0, -1.0, f32::NAN);
    rect += other;
    assert!(rect.left.is_nan());
    assert_eq!(rect.top, 0.0);
    assert_eq!(rect.right, 0.0);
    assert!(rect.bottom.is_nan());
}
