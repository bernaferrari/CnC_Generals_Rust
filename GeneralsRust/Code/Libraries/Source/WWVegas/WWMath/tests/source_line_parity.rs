//! Production WWMath calls compared with executed original colmathline/tri bodies.
//!
//! Fixture source: original colmathline.cpp and tri.cpp/tri.h, portable non-MSVC
//! normalization branch, FE_TONEAREST, contraction disabled. This is a component
//! comparison, not a retail MSVC x86 normalization or whole-game claim.
use math_utilities::{
    Vector3,
    aabox::AABox,
    collision::{CastResult, CollisionMath},
    line_segment::LineSegment,
    triangle::Triangle,
};

fn vector(values: &[f32], i: usize) -> Vector3 {
    Vector3::new(values[i], values[i + 1], values[i + 2])
}

fn seed(values: &[f32], i: usize) -> CastResult {
    CastResult {
        fraction: values[i],
        normal: vector(values, i + 1),
        contact_point: vector(values, i + 4),
        start_bad: values[i + 7] != 0.,
        compute_contact_point: values[i + 8] != 0.,
        ..Default::default()
    }
}

fn bits(value: Vector3) -> [u32; 3] {
    value.to_array().map(f32::to_bits)
}

fn result_words(hit: bool, result: CastResult) -> Vec<u32> {
    let mut words = vec![
        u32::from(hit),
        u32::from(result.start_bad),
        result.fraction.to_bits(),
    ];
    words.extend(bits(result.contact_point));
    words.extend(bits(result.normal));
    words.extend([u32::from(result.compute_contact_point), result.surface_type]);
    words
}

#[test]
fn original_portable_line_components_match_all_recorded_words() {
    let mut count = 0;
    for line in include_str!("fixtures/source_line_portable.tsv").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(fields.len(), 4);
        let [mode, name, input, expected] = fields[..] else {
            unreachable!()
        };
        let values: Vec<_> = input
            .split(',')
            .map(|word| f32::from_bits(u32::from_str_radix(word, 16).unwrap()))
            .collect();
        let expected: Vec<u32> = expected
            .split(',')
            .map(|word| word.parse().unwrap())
            .collect();
        let actual = match mode {
            "tri" => {
                assert_eq!(values.len(), 27);
                let line = LineSegment::new(vector(&values, 0), vector(&values, 3));
                let triangle = Triangle::with_normal(
                    vector(&values, 6),
                    vector(&values, 9),
                    vector(&values, 12),
                    vector(&values, 15),
                );
                let mut result = seed(&values, 18);
                let hit = CollisionMath::collide_line_triangle(&line, &triangle, &mut result);
                result_words(hit, result)
            }
            "aabox" => {
                assert_eq!(values.len(), 21);
                let line = LineSegment::new(vector(&values, 0), vector(&values, 3));
                let bounds = AABox::new(vector(&values, 6), vector(&values, 9));
                let mut result = seed(&values, 12);
                let hit = CollisionMath::collide_line_aabox(&line, &bounds, &mut result);
                result_words(hit, result)
            }
            "dominant" => {
                assert_eq!(values.len(), 3);
                let triangle = Triangle::with_normal(
                    Vector3::ZERO,
                    Vector3::X,
                    Vector3::Y,
                    vector(&values, 0),
                );
                let (a, b) = triangle.find_dominant_plane();
                vec![a as u32, b as u32, (3 - a - b) as u32]
            }
            "tri_normal" => {
                assert_eq!(values.len(), 9);
                bits(
                    Triangle::new(vector(&values, 0), vector(&values, 3), vector(&values, 6))
                        .normal,
                )
                .to_vec()
            }
            _ => panic!("unknown original component {mode}"),
        };
        assert_eq!(actual, expected, "original C++ case {name}");
        count += 1;
    }
    assert_eq!(count, 19, "the complete recorded corpus must execute");
}
