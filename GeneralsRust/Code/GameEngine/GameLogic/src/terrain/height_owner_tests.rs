//! Receiving height mutation and effects without published visual state.
use super::*;

fn world(raw: u8) -> TerrainLogic {
    let mut terrain = TerrainLogic::new();
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 5;
    map.height = 5;
    map.border_size = 1;
    map.heightmap = vec![raw; 25];
    assert!(terrain.load_map_geometry(map).is_empty());
    terrain
}

#[test]
fn receiving_height_revision_tracks_accepted_changes_and_owner_lifecycle() {
    let mut a = world(30);
    let mut b = world(70);
    let before_b = b.logic_height_map_bytes().to_vec();
    let b_revision = b.height_revision();
    let revision = a.height_revision();
    assert_eq!(a.logic_height_map_border_size(), 1);
    let mut changes = Vec::new();
    a.set_raw_map_height_with_effects(1, 1, 12, &mut |change| changes.push(change));
    assert_eq!(a.height_revision(), revision + 1);
    assert_eq!(
        changes,
        [TerrainHeightChange {
            playable_x: 1,
            playable_y: 1,
            raw_height: 12,
            requested_height: 12
        }]
    );
    assert_eq!(a.logic_height_map_bytes()[2 * 5 + 2], 12);
    for height in [12, 31] {
        a.set_raw_map_height_with_effects(1, 1, height, &mut |change| changes.push(change));
    }
    assert_eq!(changes.len(), 1);
    assert_eq!(a.height_revision(), revision + 1);
    assert_eq!(b.logic_height_map_bytes(), before_b);
    assert_eq!(b.height_revision(), b_revision);
    b.reset();
    assert_eq!(b.height_revision(), b_revision + 1);
    assert_eq!(a.height_revision(), revision + 1);
    assert_eq!(a.logic_height_map_bytes()[12], 12);
}

#[test]
fn owned_raw_effect_runs_after_accepted_write_even_if_receiving_effect_panics() {
    let mut owner = world(30);
    let before = owner.height_revision();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.set_raw_map_height_with_effects(1, 1, 9, &mut |_| panic!("receiving effect witness"));
    }));
    assert!(result.is_err());
    assert_eq!(owner.logic_height_map_bytes()[12], 9);
    assert_eq!(owner.height_revision(), before + 1);
}

#[test]
fn owned_bulk_apply_and_restore_keep_prefix_zero_fill_and_single_lighting_phase() {
    let mut owner = world(30);
    let before = owner.height_revision();
    let mut lighting = 0;
    owner.apply_logic_height_map_bytes_with_effects(&[1, 2], &mut || lighting += 1);
    assert_eq!(&owner.logic_height_map_bytes()[..4], &[1, 2, 30, 30]);
    assert_eq!(lighting, 1);
    assert_eq!(owner.height_revision(), before + 1);
    owner.apply_logic_height_map_bytes_with_effects(&[1, 2], &mut || lighting += 1);
    assert_eq!(
        lighting, 2,
        "original Xfer notification still occurs for identical payload"
    );
    assert_eq!(
        owner.height_revision(),
        before + 1,
        "cache identity changes only with bytes"
    );
    owner.restore_logic_height_map_with_effects(2, 2, &[8, 9], &mut || lighting += 1);
    assert_eq!(owner.logic_height_map_extents(), (2, 2));
    assert_eq!(owner.logic_height_map_bytes(), &[8, 9, 0, 0]);
    assert_eq!(lighting, 3);
    assert_eq!(owner.height_revision(), before + 2);
    owner.restore_logic_height_map_with_effects(0, 2, &[1], &mut || lighting += 1);
    assert_eq!(lighting, 3);
    assert_eq!(owner.height_revision(), before + 2);
}

#[test]
fn owned_flatten_box_and_circle_deliver_each_accepted_lower_write_in_order() {
    for is_box in [false, true] {
        let mut owner = world(30);
        owner.set_raw_map_height_with_effects(1, 1, 4, &mut |_| {});
        let before = owner.logic_height_map_bytes().to_vec();
        let revision = owner.height_revision();
        let mut changes = Vec::new();
        if is_box {
            owner
                .flatten_terrain_box_at_with_effects(10., 10., 0., 8., 8., &mut |change| {
                    changes.push(change)
                })
                .unwrap();
        } else {
            owner
                .flatten_terrain_at_with_effects(10., 10., 12., &mut |change| changes.push(change))
                .unwrap();
        }
        assert!(!changes.is_empty());
        assert_eq!(owner.height_revision() - revision, changes.len() as u64);
        for change in &changes {
            let index = (change.playable_y + 1) as usize * 5 + (change.playable_x + 1) as usize;
            assert!(before[index] > change.raw_height);
            assert_eq!(owner.logic_height_map_bytes()[index], change.raw_height);
        }
        let mut repeated = Vec::new();
        if is_box {
            owner
                .flatten_terrain_box_at_with_effects(10., 10., 0., 8., 8., &mut |change| {
                    repeated.push(change)
                })
                .unwrap();
        } else {
            owner
                .flatten_terrain_at_with_effects(10., 10., 12., &mut |change| repeated.push(change))
                .unwrap();
        }
        // Original circle can lower again because it has no box center cap.
        // Each accepted callback still means one actual byte change here.
        assert_eq!(
            owner.height_revision() - revision,
            (changes.len() + repeated.len()) as u64
        );
    }
}

#[test]
fn original_raw_index_alias_and_int_before_byte_narrowing_are_retained() {
    let mut owner = world(30);
    let mut events = Vec::new();
    // Border1: playablex4 means fullx5, alias of next row's x0.
    owner.set_raw_map_height_with_effects(4, 0, 7, &mut |e| events.push(e));
    assert_eq!(owner.logic_height_map_bytes()[10], 7);
    assert_eq!(owner.get_raw_map_height(4, 0), 7);
    assert_eq!(events[0].playable_x, 4);
    let revision = owner.height_revision();
    owner.set_raw_map_height_with_effects(4, 0, -249, &mut |e| events.push(e));
    assert_eq!(owner.logic_height_map_bytes()[10], 7);
    assert_eq!(
        events.len(),
        2,
        "accepted Int comparison still notifies even if byte narrows unchanged"
    );
    assert_eq!(events[1].requested_height, -249);
    assert_eq!(owner.height_revision(), revision);
}

#[test]
fn original_box_neighbour_callbacks_keep_explicit_cpp_order_and_preflight_rejects() {
    let mut owner = world(30);
    owner.set_raw_map_height_with_effects(1, 1, 4, &mut |_| {});
    let mut events = Vec::new();
    owner
        .flatten_terrain_box_at_with_effects(10., 10., 0., 8., 8., &mut |e| events.push(e))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.playable_x, e.playable_y))
            .collect::<Vec<_>>(),
        [
            (0, 1),
            (2, 1),
            (1, 0),
            (1, 2),
            (0, 0),
            (2, 2),
            (2, 0),
            (0, 2)
        ]
    );
    assert!(events.iter().all(|e| e.raw_height == 4));
    let before = owner.logic_height_map_bytes().to_vec();
    let revision = owner.height_revision();
    assert!(
        owner
            .flatten_terrain_box_at_with_effects(f32::NAN, 10., 0., 8., 8., &mut |_| panic!(
                "no effects before preflight"
            ))
            .is_err()
    );
    assert_eq!(owner.logic_height_map_bytes(), before);
    assert_eq!(owner.height_revision(), revision);
}

#[test]
fn typed_height_admission_rejects_before_bytes_revision_metadata_or_effect_changes() {
    let mut owner = world(30);
    let bytes = owner.logic_height_map_bytes().to_vec();
    let revision = owner.height_revision();
    let border = owner.logic_height_map_border_size();
    let extents = owner.logic_height_map_extents();
    let mut bad = crate::system::map_loader::MapData::new();
    bad.width = 8;
    bad.height = 6;
    bad.heightmap = vec![1; 8];
    assert!(owner.try_load_map_geometry(bad).is_err());
    assert!(
        owner
            .try_restore_logic_height_map_with_effects(i32::MAX, 2, &[1], &mut || panic!(
                "effect after invalid admission"
            ))
            .is_err()
    );
    assert_eq!(owner.logic_height_map_bytes(), bytes);
    assert_eq!(owner.height_revision(), revision);
    assert_eq!(owner.logic_height_map_border_size(), border);
    assert_eq!(owner.logic_height_map_extents(), extents);
    owner.validate_height_map().unwrap();
    let mut empty = TerrainLogic::new();
    assert!(
        empty
            .try_apply_logic_height_map_bytes_with_effects(&[1], &mut || panic!("orphan effect"))
            .is_err()
    );
    assert!(empty.logic_height_map_bytes().is_empty());
    assert_eq!(empty.logic_height_map_extents(), (0, 0));
    assert_eq!(empty.height_revision(), 0);
    empty.validate_height_map().unwrap();
}
