use super::*;
use game_engine::common::system::GeometryType;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn cpp_wire(kind: i32, values: [f32; 5]) -> Vec<u8> {
    let mut bytes = vec![1];
    bytes.extend(kind.to_le_bytes());
    bytes.push(1);
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
    bytes
}
#[test]
fn literal_cpp_geometry_preserves_dimensions_and_independent_saved_caches() {
    let bytes = cpp_wire(2, [8.0, 12.0, 4.0, 97.0, 101.0]);
    let mut geometry = GeometryInfo::default();
    let mut input = Cursor::new(bytes.clone());
    xfer_geometry_info(&mut XferLoad::new(&mut input, 1), &mut geometry).unwrap();
    assert_eq!(input.position(), 26);
    assert_eq!(geometry.get_geometry_type(), GeometryType::Box);
    assert!(geometry.get_is_small());
    assert_eq!(
        (
            geometry.get_max_height_above_position(),
            geometry.get_major_radius(),
            geometry.get_minor_radius()
        ),
        (8.0, 12.0, 4.0)
    );
    assert_eq!(geometry.get_bounding_circle_radius(), 97.0);
    assert_eq!(geometry.get_bounding_sphere_radius(), 101.0);
    let mut output = Cursor::new(Vec::new());
    xfer_geometry_info(&mut XferSave::new(&mut output, 1), &mut geometry).unwrap();
    assert_eq!(output.into_inner(), bytes);
}
#[test]
fn malformed_geometry_enum_and_truncated_wire_are_rejected() {
    for kind in [-1, 3, i32::MAX] {
        let mut geometry = GeometryInfo::default();
        assert!(
            xfer_geometry_info(
                &mut XferLoad::new(
                    Cursor::new(cpp_wire(kind, [8.0, 12.0, 4.0, 97.0, 101.0])),
                    1
                ),
                &mut geometry
            )
            .is_err()
        );
    }
    let bytes = cpp_wire(2, [8.0, 12.0, 4.0, 97.0, 101.0]);
    for end in 0..bytes.len() {
        let mut geometry = GeometryInfo::default();
        assert!(
            xfer_geometry_info(
                &mut XferLoad::new(Cursor::new(&bytes[..end]), 1),
                &mut geometry
            )
            .is_err(),
            "prefix {end}"
        );
    }
}
#[test]
fn decoded_invalid_dimensions_are_preserved_until_candidate_validation() {
    for index in 0..5 {
        for bad in [-1.0, f32::NAN, f32::INFINITY] {
            let mut values = [8.0, 12.0, 4.0, 97.0, 101.0];
            values[index] = bad;
            let mut geometry = GeometryInfo::default();
            xfer_geometry_info(
                &mut XferLoad::new(Cursor::new(cpp_wire(2, values)), 1),
                &mut geometry,
            )
            .unwrap();
            let actual = [
                geometry.shape.height,
                geometry.shape.width,
                geometry.shape.depth,
                geometry.shape.bounding_circle_radius,
                geometry.shape.bounding_sphere_radius,
            ];
            assert_eq!(actual[index].to_bits(), bad.to_bits());
            assert!(geometry.shape.validate_saved_shape().is_err());
        }
    }
}
#[test]
fn actual_shape_mutation_recalculates_caches_using_original_cpp_axes() {
    let mut geometry = GeometryInfo::default();
    geometry.shape.set(GeometryType::Box, true, 8.0, 12.0, 4.0);
    assert_eq!(geometry.get_bounding_circle_radius(), 160.0_f32.sqrt());
    geometry.tweak_extents(crate::common::GeometryExtentModType::Major, 2.0);
    assert_eq!(geometry.get_major_radius(), 14.0);
    assert_eq!(geometry.get_minor_radius(), 4.0);
    assert_eq!(geometry.get_bounding_sphere_radius(), 228.0_f32.sqrt());
    assert!(!geometry.get_is_small());
}
