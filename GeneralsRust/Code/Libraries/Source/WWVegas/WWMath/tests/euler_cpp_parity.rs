use math_utilities::wwmath::{EulerAngles, EulerOrder::*};
use math_utilities::{EulerAngles as LocalEulerAngles, EulerOrder as LocalEulerOrder};

const PORTABLE_MAC: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

const MATRICES: [[[f32; 3]; 3]; 4] = [
    [[0.36, -0.80, 0.48], [0.48, 0.60, 0.64], [-0.80, 0.0, 0.60]],
    [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
    [
        [-0.94, 0.12, 0.28],
        [-0.09, -0.99, 0.11],
        [0.29, 0.08, 0.953],
    ],
    [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
];

#[test]
fn all_axis_orders_match_original_euler_cpp() {
    // The rows are in C++ packed order (initial axis, parity, repeat, frame).
    // Captured from the unmodified euler.cpp with the original Matrix3D and
    // WWMath headers. The input translation was nonzero; To_Matrix clears it.
    let orders = [
        XYZs, ZYXr, XYXs, XYXr, XZYs, YZXr, XZXs, XZXr, YZXs, XZYr, YZYs, YZYr, YXZs, ZXYr, YXYs,
        YXYr, ZXYs, YXZr, ZXZs, ZXZr, ZYXs, XYZr, ZYZs, ZYZr,
    ];
    let fixture = include_str!("fixtures/euler_portable.tsv");
    assert_eq!(fixture.lines().count(), MATRICES.len() * orders.len());
    for (index, line) in fixture.lines().enumerate() {
        let mut words = line.split_whitespace();
        let matrix: usize = words.next().unwrap().parse().unwrap();
        let order: usize = words.next().unwrap().parse().unwrap();
        assert_eq!((matrix, order), (index / 24, index % 24));
        let actual = EulerAngles::from_matrix(&MATRICES[matrix], orders[order]);
        for axis in 0..3 {
            let expected = u64::from_str_radix(words.next().unwrap(), 16).unwrap();
            let angle = actual.get_angle(axis);
            if PORTABLE_MAC || f64::from_bits(expected) == 0.0 {
                assert_eq!(
                    angle.to_bits(),
                    expected,
                    "matrix {matrix}, order {order}, axis {axis}"
                );
            } else {
                assert!((angle - f64::from_bits(expected)).abs() <= 2.0e-6);
            }
        }
        for (row_index, row) in actual.to_matrix().into_iter().enumerate() {
            for (column, entry) in row.into_iter().enumerate() {
                let expected = u32::from_str_radix(words.next().unwrap(), 16).unwrap();
                if PORTABLE_MAC || f32::from_bits(expected) == 0.0 {
                    assert_eq!(
                        entry.to_bits(),
                        expected,
                        "matrix {matrix}, order {order}, at [{row_index}][{column}]"
                    );
                } else {
                    assert!((entry - f32::from_bits(expected)).abs() <= 2.0e-6);
                }
            }
            assert_eq!(words.next(), Some("00000000"));
        }
        assert!(words.next().is_none());
    }
}

#[test]
fn default_order_and_local_axis_adapter_match_original() {
    let default = EulerAngles::new();
    assert_eq!(default.to_matrix()[2][0].to_bits(), (-0.0f32).to_bits());
    for (local, original) in [
        (LocalEulerOrder::XYZ, XYZr),
        (LocalEulerOrder::XZY, XZYr),
        (LocalEulerOrder::YXZ, YXZr),
        (LocalEulerOrder::YZX, YZXr),
        (LocalEulerOrder::ZXY, ZXYr),
        (LocalEulerOrder::ZYX, ZYXr),
    ] {
        let [[a, b, c], [d, e, f], [g, h, i]] = MATRICES[0];
        let source = math_utilities::matrix3::Matrix3::from_values(a, b, c, d, e, f, g, h, i);
        let actual = LocalEulerAngles::from_matrix(&source, local);
        let expected = EulerAngles::from_matrix(&MATRICES[0], original);
        assert_eq!(
            actual.angles.map(f64::to_bits),
            std::array::from_fn(|axis| expected.get_angle(axis).to_bits())
        );
        let actual_matrix = actual.to_matrix();
        let expected_matrix = expected.to_matrix();
        for row in 0..3 {
            assert_eq!(
                actual_matrix[row].to_array().map(f32::to_bits),
                expected_matrix[row].map(f32::to_bits)
            );
        }
    }
}
