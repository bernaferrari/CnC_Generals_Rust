//! Detached codec owners; no global terrain, water or scorch fixture.
use super::*;
fn terrain(raw: u8, enabled: bool) -> gamelogic::terrain::TerrainLogic {
    let mut owner = gamelogic::terrain::TerrainLogic::new();
    owner
        .try_load_map_geometry({
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 2;
            map.height = 2;
            map.heightmap = vec![raw; 4];
            if enabled {
                map.waypoints
                    .push(gamelogic::system::map_loader::MapWaypoint {
                        id: 1,
                        name: "WaveGuide1".into(),
                        location: gamelogic::system::map_loader::Coord3D::new(0., 0., 0.),
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
