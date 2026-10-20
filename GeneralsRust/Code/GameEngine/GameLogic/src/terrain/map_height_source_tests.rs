//! Original BaseHeightMap sampler cells, independent of mutable presentation.
use super::*;

fn ramp() -> TerrainLogic {
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 8;
    map.height = 8;
    map.border_size = 2;
    map.heightmap = (0..12)
        .flat_map(|y| (0..12).map(move |x| 20 + x * 5 + y * 7))
        .collect();
    let mut owner = TerrainLogic::new();
    assert!(owner.load_map_geometry(map).is_empty());
    assert_eq!(owner.logic_height_map_extents(), (12, 12));
    owner
}

#[test]
fn negative_integer_ground_cell_uses_original_fast_floor_before_border() {
    let owner = ramp();
    // FAST_REAL_FLOOR(-1.0) = -2.0 after float almost-one subtraction.
    // Border two selects physical(0,2); clipping returns raw byte34*.625.
    assert_eq!(
        owner.get_ground_height(-10., 0., None).to_bits(),
        21.25f32.to_bits()
    );
    assert_eq!(
        owner.get_ground_height(0., -10., None).to_bits(),
        18.75f32.to_bits()
    );
    assert_eq!(
        owner.get_ground_height(0., 0., None).to_bits(),
        27.5f32.to_bits()
    );
    assert_eq!(
        owner.get_ground_height(-0., -0., None).to_bits(),
        27.5f32.to_bits()
    );
}

#[test]
fn original_border_disk_sample_sum_preserves_raw_rounding_and_peer() {
    let mut owner = ramp();
    let peer = ramp();
    let peer_before = peer.logic_height_map_bytes().to_vec();
    // Actual circle_border_with_padding source corpus samples in loop order.
    let points = [(-10., 0.), (0., 0.), (0., 10.), (10., 0.), (10., 10.)];
    let mut sum = 0.0f32;
    for ((x, y), expected) in points
        .into_iter()
        .zip([21.25f32, 27.5, 31.875, 30.625, 35.])
    {
        let observed = owner.get_ground_height(x, y, None);
        assert_eq!(observed.to_bits(), expected.to_bits());
        sum += observed;
    }
    assert_eq!(sum.to_bits(), 146.25f32.to_bits());
    let mut changes = Vec::new();
    owner
        .flatten_terrain_at_with_effects(1., 1., 13., &mut |e| changes.push(e))
        .unwrap();
    assert!(!changes.is_empty());
    assert!(
        changes
            .iter()
            .all(|e| e.requested_height == 47 && e.raw_height == 47)
    );
    assert_eq!(peer.logic_height_map_bytes(), peer_before);
    // Full ordered accepted edits/final bytes/snap remain independently asserted
    // by construction_flatten_cpp_tests::circle_border_with_padding.
}
