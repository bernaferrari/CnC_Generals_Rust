//! Detached codec owners; no global terrain, water or scorch fixture.
use super::*;

struct NoTreeDraws;

impl TreeRandom for NoTreeDraws {
    fn int_range(&mut self, _: i32, _: i32) -> i32 {
        panic!("empty TREE slot consumed an integer draw");
    }

    fn real_range(&mut self, _: f32, _: f32) -> f32 {
        panic!("empty TREE slot consumed a real draw");
    }
}

fn terrain(raw: u8, enabled: bool) -> gamelogic::terrain::TerrainLogic {
    let mut owner = gamelogic::terrain::TerrainLogic::new();
    owner
        .try_load_map_geometry({
            let mut map = game_engine::map_data::MapData::new();
            map.width = 2;
            map.height = 2;
            map.heightmap = vec![raw; 4];
            if enabled {
                map.waypoints
                    .push(game_engine::map_data::MapWaypoint {
                        id: 1,
                        name: "WaveGuide1".into(),
                        location: glam::Vec3::new(0., 0., 0.),
                        path_label1: String::new(),
                        path_label2: String::new(),
                        path_label3: String::new(),
                        bi_directional: false,
                    });
            }
            map
        })
        .unwrap();
    let mut definitions = game_engine::common::ini::ini_game_data::GlobalData::new();
    definitions.vertex_water_available_maps[0] = "owned_water.map".into();
    definitions.vertex_water_x_grid_cells[0] = 1;
    definitions.vertex_water_y_grid_cells[0] = 1;
    owner.new_map_with_water_configuration(false, "owned_water.map", Some(&definitions), None);
    owner
}
#[test]
fn owned_codec_pairs_logical_and_optional_native_values_without_live_effects() {
    crate::test_support::ensure_ambient_host();
    let a = terrain(40, true);
    let b = terrain(90, false);
    let mark = ScorchMark {
        location: [10., 20., 3.],
        radius: 5.,
        scorch_type: 2,
    };
    let a_bytes = capture_owned_terrain_visual_for_terrain(&a, Some(&[mark])).unwrap();
    let b_bytes = capture_owned_terrain_visual_for_terrain(&b, None).unwrap();
    let a_prepared = prepare_terrain_visual_for_terrain(&a, &a_bytes)
        .unwrap()
        .unwrap();
    let b_prepared = prepare_terrain_visual_for_terrain(&b, &b_bytes)
        .unwrap()
        .unwrap();
    assert_eq!(a_prepared.heights(), &[40; 4]);
    assert_eq!(a_prepared.scorches(), Some([mark].as_slice()));
    assert!(a_prepared.grid().enabled);
    assert_eq!(
        b_prepared.scorches(),
        None,
        "logical-only save must not invent a native clear"
    );
    assert!(!b_prepared.grid().enabled);
    assert_eq!(a.logic_height_map_bytes(), &[40; 4]);
    assert_eq!(b.logic_height_map_bytes(), &[90; 4]);
    let mut receiving_a = terrain(1, true);
    let mut receiving_b = terrain(2, false);
    b_prepared.admit_logical(&mut receiving_b).unwrap();
    a_prepared.admit_logical(&mut receiving_a).unwrap();
    assert_eq!(receiving_a.logic_height_map_bytes(), &[40; 4]);
    assert!(receiving_a.water_grid_state().enabled);
    assert_eq!(receiving_b.logic_height_map_bytes(), &[90; 4]);
    assert!(!receiving_b.water_grid_state().enabled);
    let explicit_clear = capture_owned_terrain_visual_for_terrain(&b, Some(&[])).unwrap();
    assert_eq!(
        prepare_terrain_visual_for_terrain(&b, &explicit_clear)
            .unwrap()
            .unwrap()
            .scorches(),
        Some([].as_slice())
    );
    assert!(
        prepare_terrain_visual_for_terrain(&a, &[1])
            .unwrap()
            .is_none()
    );
}
#[test]
fn malformed_tail_and_trailing_bytes_fail_before_receiving_admission() {
    crate::test_support::ensure_ambient_host();
    let a = terrain(40, false);
    let mark = ScorchMark {
        location: [10., 20., 3.],
        radius: 5.,
        scorch_type: 2,
    };
    let bytes = capture_owned_terrain_visual_for_terrain(&a, Some(&[mark])).unwrap();
    let receiving = terrain(90, false);
    let before = receiving.height_revision();
    for invalid in [
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.as_slice(), &[0]].concat(),
    ] {
        assert!(prepare_terrain_visual_for_terrain(&receiving, &invalid).is_err());
        assert_eq!(receiving.logic_height_map_bytes(), &[90; 4]);
        assert!(!receiving.water_grid_state().enabled);
        assert_eq!(receiving.height_revision(), before);
    }
    let mut absent = capture_owned_terrain_visual_for_terrain(&a, None).unwrap();
    absent.push(0);
    assert!(
        prepare_terrain_visual_for_terrain(&receiving, &absent).is_err(),
        "partial count is not an absent tail"
    );
    assert!(
        prepare_terrain_visual_for_terrain(&gamelogic::terrain::TerrainLogic::new(), &bytes)
            .is_err()
    );
}

#[test]
fn historical_v1_v2_without_native_tail_preserve_explicit_absence() {
    crate::test_support::ensure_ambient_host();
    let receiving = terrain(90, false);
    let v1 = prepare_terrain_visual_for_terrain(&receiving, &[1, 1, 0])
        .unwrap()
        .unwrap();
    assert_eq!(v1.heights(), &[90; 4]);
    assert_eq!(v1.scorches(), None);
    let v2 = prepare_terrain_visual_for_terrain(&receiving, &[2, 1, 0, 4, 0, 0, 0, 1, 2, 3, 4])
        .unwrap()
        .unwrap();
    assert_eq!(v2.heights(), &[1, 2, 3, 4]);
    assert_eq!(v2.scorches(), None);
}

#[test]
fn original_water_enabled_mismatch_is_rejected_against_receiving_map() {
    crate::test_support::ensure_ambient_host();
    let source = terrain(40, true);
    let receiving = terrain(90, false);
    let bytes = capture_owned_terrain_visual_for_terrain(&source, None).unwrap();
    assert!(
        prepare_terrain_visual_for_terrain(&receiving, &bytes)
            .unwrap_err()
            .contains("enabled flag")
    );
    assert_eq!(receiving.logic_height_map_bytes(), &[90; 4]);
    assert!(!receiving.water_grid_state().enabled);
}

#[test]
fn actual_waveguide_configuration_uses_explicit_definitions_and_optional_delivery() {
    crate::test_support::ensure_ambient_host();
    let mut definitions = game_engine::common::ini::ini_game_data::GlobalData::new();
    definitions.vertex_water_available_maps[0] = "owned_water.map".into();
    definitions.vertex_water_x_grid_cells[0] = 2;
    definitions.vertex_water_y_grid_cells[0] = 3;
    let mut receiver = terrain(40, true);
    let mut events = Vec::new();
    receiver.new_map_with_water_configuration(
        false,
        "owned_water.map",
        Some(&definitions),
        Some(&mut |grid| events.push((grid.enabled, grid.resolution))),
    );
    assert_eq!(events.len(), 1);
    assert!(events[0].0);
    assert_eq!((events[0].1.0, events[0].1.1), (2., 3.));
    let before = receiver.water_grid_state().clone();
    receiver.new_map_with_water_configuration(
        false,
        "missing_settings.map",
        Some(&definitions),
        Some(&mut |_| panic!("missing settings must not notify native receiver")),
    );
    assert!(receiver.is_water_grid_enabled());
    assert_eq!(receiver.water_grid_state(), &before);
    let mut no_waveguide = terrain(90, false);
    no_waveguide.restore_water_grid_state(before);
    let mut disabled = Vec::new();
    no_waveguide.new_map_with_water_configuration(
        false,
        "owned_water.map",
        Some(&definitions),
        Some(&mut |grid| disabled.push(grid.enabled)),
    );
    assert_eq!(disabled, [false]);
    assert!(!no_waveguide.is_water_grid_enabled());
}

#[test]
fn prepared_native_admission_changes_only_receiving_visual_and_preserves_absent_scorch_tail() {
    crate::test_support::ensure_ambient_host();
    use crate::terrain::{HeightMap, terrain_visual::TerrainVisualImpl};
    let source = terrain(40, true);
    let mark = ScorchMark {
        location: [10., 20., 3.],
        radius: 5.,
        scorch_type: 2,
    };
    let bytes = capture_owned_terrain_visual_for_terrain(&source, Some(&[mark])).unwrap();
    let prepared = prepare_terrain_visual_for_terrain(&source, &bytes)
        .unwrap()
        .unwrap();
    let mut a = TerrainVisualImpl::new();
    let mut b = TerrainVisualImpl::new();
    a.load_heightmap_from_data(HeightMap::new(2, 2, 0., 10.), None, None)
        .unwrap();
    b.load_heightmap_from_data(HeightMap::new(2, 2, 0., 10.), None, None)
        .unwrap();
    let foreign = ScorchMark {
        location: [30., 40., 5.],
        radius: 9.,
        scorch_type: 1,
    };
    b.add_scorch(foreign.location, foreign.radius, foreign.scorch_type);
    prepared.apply_native(&mut a, &mut NoTreeDraws);
    assert!(a.water_grid_enabled());
    assert!(!b.water_grid_enabled());
    assert_eq!(a.scorch_marks(), [mark]);
    assert_eq!(b.scorch_marks(), [foreign]);
    let absent = capture_owned_terrain_visual_for_terrain(&source, None).unwrap();
    prepare_terrain_visual_for_terrain(&source, &absent)
        .unwrap()
        .unwrap()
        .apply_native(&mut a, &mut NoTreeDraws);
    assert_eq!(
        a.scorch_marks(),
        [mark],
        "historical absent tail is not a native clear"
    );
    let clear = capture_owned_terrain_visual_for_terrain(&source, Some(&[])).unwrap();
    prepare_terrain_visual_for_terrain(&source, &clear)
        .unwrap()
        .unwrap()
        .apply_native(&mut a, &mut NoTreeDraws);
    assert!(a.scorch_marks().is_empty());
    assert_eq!(b.scorch_marks(), [foreign]);
}

#[test]
fn native_water_motion_is_captured_from_receiving_visual_without_changing_logical_only_codec() {
    crate::test_support::ensure_ambient_host();
    use crate::terrain::terrain_visual::TerrainVisualImpl;
    let logical = terrain(40, true);
    let mut native = TerrainVisualImpl::new();
    native.admit_water_state(logical.water_grid_state());
    native.set_water_attenuation_factors(1.0, 0.0, 0.0, 1.0);
    native.add_water_velocity(0.0, 0.0, 3.0, 17.0);
    let observed = native.capture_water_state();
    assert!(
        !observed.mesh_motion.is_empty(),
        "actual native velocity producer"
    );
    assert!(logical.water_grid_state().mesh_motion.is_empty());
    let bytes = capture_terrain_visual_with_water(&logical, Some(&observed), Some(&[])).unwrap();
    let prepared = prepare_terrain_visual_for_terrain(&logical, &bytes)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.grid().mesh_motion, observed.mesh_motion);
    assert_eq!(prepared.heights(), &[40; 4]);
    let typed = capture_typed_terrain_visual_with_tree_records(
        &logical,
        Some(&observed),
        Some(&[]),
        Vec::new(),
    )
    .unwrap();
    assert!(!typed.water_cells.is_empty());
    let typed_prepared = prepare_typed_terrain_visual_for_terrain(&logical, &typed)
        .unwrap()
        .unwrap();
    assert_eq!(typed_prepared.grid().mesh_motion, observed.mesh_motion);
    assert_eq!(typed_prepared.heights(), &[40; 4]);
    let logical_bytes = capture_owned_terrain_visual_for_terrain(&logical, None).unwrap();
    let logical_prepared = prepare_terrain_visual_for_terrain(&logical, &logical_bytes)
        .unwrap()
        .unwrap();
    assert!(logical_prepared.grid().mesh_motion.is_empty());
    let mut receiver = TerrainVisualImpl::new();
    receiver.admit_water_state(logical.water_grid_state());
    let mut peer = TerrainVisualImpl::new();
    peer.admit_water_state(logical.water_grid_state());
    let peer_before = peer.capture_water_state();
    let before = receiver.capture_water_state();
    let corrupt = &bytes[..bytes.len() - 1];
    assert!(prepare_terrain_visual_for_terrain(&logical, corrupt).is_err());
    assert_eq!(
        receiver.capture_water_state(),
        before,
        "failed parse does not apply native motion"
    );
    prepared.apply_native(&mut receiver, &mut NoTreeDraws);
    assert_eq!(
        receiver.capture_water_state().mesh_motion,
        observed.mesh_motion
    );
    assert_eq!(peer.capture_water_state(), peer_before);
}

#[test]
fn native_water_admission_and_recapture_preserves_complete_source_status_byte() {
    crate::test_support::ensure_ambient_host();
    use crate::terrain::terrain_visual::TerrainVisualImpl;
    let mut logical = terrain(40, true);
    logical.water_grid_state_mut().mesh_motion.insert(
        (0, 0),
        gamelogic::terrain_water::WaterGridMeshMotion {
            velocity: 3.0,
            status: 0x82,
            preferred_height: 17,
        },
    );
    let mut visual = TerrainVisualImpl::new();
    visual.admit_water_state(logical.water_grid_state());
    assert_eq!(
        visual.capture_water_state().mesh_motion[&(0, 0)].status,
        0x82
    );
    visual.set_water_attenuation_factors(1.0, 0.0, 0.0, 1.0);
    visual.add_water_velocity(0.0, 0.0, 2.0, 19.0);
    assert_eq!(
        visual.capture_water_state().mesh_motion[&(0, 0)].status,
        0x83,
        "C++ BitSet preserves accepted upper status bits"
    );
}

#[test]
fn native_tree_slot_round_trips_source_fields_and_deleted_rows_in_admission_order() {
    use crate::terrain::{TreeModuleData, TreeSphere, W3DToppleState, terrain_visual::TerrainVisualImpl};
    use glam::{Vec3, Vec4};

    let logical = terrain(40, false);
    let mut visual = TerrainVisualImpl::new();
    let module = TreeModuleData {
        model_name: "Oak".into(),
        texture_name: "Oak.tga".into(),
        ..Default::default()
    };
    for (id, position) in [(42, Vec3::new(1., 2., 3.)), (7, Vec3::new(4., 5., 6.))] {
        visual.add_tree(id, position, 2., 0., 0., module.clone(), TreeSphere::default());
    }
    let tree = visual.tree_buffer_mut().tree_mut(0).unwrap();
    tree.sin = 0.6;
    tree.cos = 0.8;
    tree.angular_velocity = 0.125;
    tree.angular_acceleration = 0.25;
    tree.topple_direction = Vec3::new(-0.5, 0.75, 0.);
    tree.topple_state = W3DToppleState::Fogged;
    tree.angular_accumulation = 0.5;
    tree.options = 0x03;
    tree.matrix = Mat4::from_cols(
        Vec4::new(1., 2., 3., 0.),
        Vec4::new(4., 5., 6., 0.),
        Vec4::new(7., 8., 9., 0.),
        Vec4::new(10., 11., 12., 1.),
    );
    tree.sink_frames_left = 31;
    visual.tree_buffer_mut().remove_tree(7);

    let authored = visual.capture_tree_records();
    assert_eq!(authored.len(), 2);
    assert_eq!(authored[0].drawable_id, 42);
    assert_eq!(authored[1].drawable_id, 7);
    assert!(authored[1].model_name.is_empty() && authored[1].model_texture.is_empty());
    let mark = ScorchMark {
        location: [10., 20., 3.],
        radius: 5.,
        scorch_type: 2,
    };
    let bytes =
        capture_terrain_visual_with_tree_records(&logical, None, Some(&[mark]), authored.clone())
            .unwrap();
    let prepared = prepare_terrain_visual_for_terrain(&logical, &bytes)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.tree_records(), Some(authored.as_slice()));
    assert_eq!(prepared.scorches(), Some([mark].as_slice()));

    let mut expected = b"\x03\x01\x00\x04\x00\x00\x00\x28\x28\x28\x28\x01\x01".to_vec();
    expected.extend(2_i32.to_le_bytes());
    expected.extend(b"\x03Oak\x07Oak.tga");
    for value in [1.0_f32, 2., 3., 2., 0.6, 0.8] {
        expected.extend(value.to_le_bytes());
    }
    expected.extend(42_u32.to_le_bytes());
    for value in [0.125_f32, 0.25, -0.5, 0.75, 0.] {
        expected.extend(value.to_le_bytes());
    }
    expected.extend(2_i32.to_le_bytes()); // TOPPLE_FOGGED
    expected.extend(0.5_f32.to_le_bytes());
    expected.extend(3_u32.to_le_bytes());
    expected.push(1); // Matrix3D version
    for value in [1.0_f32, 4., 7., 10., 2., 5., 8., 11., 3., 6., 9., 12.] {
        expected.extend(value.to_le_bytes());
    }
    expected.extend(31_u32.to_le_bytes());
    expected.extend([0, 0]); // empty model and texture of the next record
    assert_eq!(&bytes[..expected.len()], &expected);
}

#[test]
fn native_tree_slot_rejects_invalid_counts_and_truncated_records_before_admission() {
    let source = terrain(40, false);
    let receiver = terrain(90, false);
    let bytes = capture_owned_terrain_visual_for_terrain(&source, None).unwrap();
    let count_offset = 9 + source.logic_height_map_bytes().len();
    let tree_version_offset = count_offset - 1;
    assert_eq!(bytes[tree_version_offset], 1);
    assert_eq!(bytes[count_offset..count_offset + 4], [0; 4]);
    let prepared = prepare_terrain_visual_for_terrain(&receiver, &bytes)
        .unwrap()
        .unwrap();
    assert_eq!(prepared.tree_records(), Some([].as_slice()));

    let before_height_revision = receiver.height_revision();
    for count in [-1, MAX_TREES as i32 + 1] {
        let mut invalid = bytes.clone();
        invalid[count_offset..count_offset + 4].copy_from_slice(&count.to_le_bytes());
        let err = prepare_terrain_visual_for_terrain(&receiver, &invalid).unwrap_err();
        assert!(err.contains("TREE count"), "{err}");
    }

    let mut truncated = bytes.clone();
    truncated[count_offset..count_offset + 4].copy_from_slice(&1_i32.to_le_bytes());
    assert!(prepare_terrain_visual_for_terrain(&receiver, &truncated).is_err());
    assert_eq!(receiver.logic_height_map_bytes(), &[90; 4]);
    assert_eq!(receiver.height_revision(), before_height_revision);

    // Terrain v2 ends before the BaseHeightMap/Tree/Prop snapshot.
    let mut v2 = bytes[..tree_version_offset - 1].to_vec();
    v2[0] = 2;
    assert!(
        prepare_terrain_visual_for_terrain(&receiver, &v2)
            .unwrap()
            .unwrap()
            .tree_records()
            .is_none()
    );
}
