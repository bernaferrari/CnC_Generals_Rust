//! Executed original consumer rows, observed through the actual Rust operations.
//! Original wrapper enum labels are normalized explicitly, not serialized IDs.
use super::*;

fn inputs(mode: &str) -> Vec<Vec<&'static str>> {
    include_str!("fixtures/owned_ground_consumer_inputs.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|row| row[1] == mode)
        .collect()
}
fn number(row: &[&str], index: usize) -> f32 {
    row[index].parse().unwrap()
}
fn compare(tag: &str, observed: String) {
    let original = include_str!("fixtures/owned_ground_consumer_original.txt")
        .lines()
        .find(|line| line.split_whitespace().next() == Some(tag))
        .unwrap();
    assert_eq!(observed, original, "executed original consumer {tag}");
    eprintln!("PATHFINDING_CPP {observed}");
}
fn terrain(height: f32) -> gamelogic::terrain::TerrainLogic {
    let raw = height / 0.625;
    assert_eq!(
        raw.fract(),
        0.0,
        "input is representable in original byte heights"
    );
    assert!((0.0..=255.0).contains(&raw));
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 16;
    map.height = 16;
    map.border_size = 0;
    map.heightmap = vec![raw as u8; 256];
    let mut owner = gamelogic::terrain::TerrainLogic::new();
    assert!(owner.load_map_geometry(map).is_empty());
    owner
}
fn bridge(grid: &mut PathfindingGrid, height: f32) -> u8 {
    grid.reserve_bridge_layer(
        Vec3::new(20., height, 20.),
        Vec3::new(20., height, 80.),
        Vec3::new(90., height, 20.),
        Vec3::new(90., height, 80.),
    )
}
fn original_type(value: PathfindCellType) -> u8 {
    // The isolated C++ consumer adapter uses labels, not the engine's enum ABI.
    match value {
        PathfindCellType::Clear => 0,
        PathfindCellType::Cliff => 1,
        PathfindCellType::Water => 2,
        PathfindCellType::Obstacle => 3,
        PathfindCellType::Impassable => 4,
        PathfindCellType::BridgeImpassable => 5,
        other => panic!("unrepresented original adapter type: {other:?}"),
    }
}
fn original_layer(layer: PathfindLayerEnum) -> u8 {
    // The isolated original wrapper spells GROUND=0/WALL=1. Main's runtime
    // enum keeps its existing GROUND=1/WALL=15; bridge slot2 is unchanged.
    match layer {
        PathfindLayerEnum::Ground => 0,
        PathfindLayerEnum::Wall => 1,
        other => other as u8,
    }
}
#[test]
fn original_layer_selection_reads_receiving_ground_and_preserves_strict_tie() {
    for row in inputs("layer") {
        let owner = terrain(number(&row, 2));
        let ground = PathfindingGround::receiving(&owner);
        let mut grid = PathfindingGrid::new(160., 160., 10.);
        assert_eq!(bridge(&mut grid, number(&row, 4)), 2);
        let point = Vec3::new(50., number(&row, 3), 60.);
        compare(
            row[0],
            format!(
                "{} layer {} ground {}",
                row[0],
                original_layer(grid.layer_for_destination(point, &ground)),
                ground.height_at(point)
            ),
        );
    }
}
#[test]
fn original_bridge_clearance_reads_receiving_ground_with_entry_and_obstacle_guards() {
    assert_eq!(LAYER_Z_CLOSE_ENOUGH_F, 10.);
    for row in inputs("clearance") {
        let owner = terrain(number(&row, 2));
        let ground = PathfindingGround::receiving(&owner);
        let mut grid = PathfindingGrid::new(160., 160., 10.);
        let layer = bridge(&mut grid, number(&row, 3));
        // Real geometry gives the original wrapper's boolean entry predicate;
        // the interior sample is deliberately away from both entry lines.
        let point = if row[4] == "1" {
            GridPos::new(1, 6)
        } else {
            GridPos::new(5, 6)
        };
        if row[5] == "1" {
            grid.set_cell_type(point, PathfindCellType::Obstacle);
        }
        grid.stamp_reserved_bridge_layer(layer, false, &ground);
        let (ty, connection) = grid
            .bridge_layers
            .iter()
            .find(|bridge| bridge.id == layer)
            .unwrap()
            .cells[&(point.x, point.y)];
        // Original harness uses WALL as its no-connection sentinel. Rust's
        // existing stored sentinel is0; an actual ground connection is1.
        let original_connection = if connection == PathfindLayerEnum::Ground as u8 {
            0
        } else {
            assert_eq!(connection, 0);
            1
        };
        compare(
            row[0],
            format!(
                "{} ground_type {} bridge_type {} connect {}",
                row[0],
                original_type(grid.cell_type(point)),
                original_type(ty),
                original_connection
            ),
        );
    }
}
#[test]
fn original_adjusted_coordinates_sample_receiving_ground_and_clip_buried_bridge() {
    for row in inputs("adjust") {
        let owner = terrain(number(&row, 2));
        let ground = PathfindingGround::receiving(&owner);
        let mut grid = PathfindingGrid::new(160., 160., 10.);
        let layer = bridge(&mut grid, number(&row, 3));
        let selected = if row[7] == "0" {
            PathfindLayerEnum::Ground
        } else {
            assert_eq!(row[7], "2");
            PathfindLayerEnum::from_u32(layer as u32)
        };
        let actual = grid.adjust_coord_to_cell_on_layer(
            GridPos::new(row[4].parse().unwrap(), row[5].parse().unwrap()),
            row[6] == "1",
            selected,
            &ground,
        );
        compare(
            row[0],
            format!("{} xyz {} {} {}", row[0], actual.x, actual.z, actual.y),
        );
    }
}
#[test]
fn original_water_classification_observes_four_corners_and_real_structure_override() {
    for row in inputs("water") {
        let mut owner = terrain(50.);
        let mut raw = owner.logic_height_map_bytes().to_vec();
        for (index, input) in [(0, 3), (1, 4), (16, 5), (17, 6)] {
            let value = number(&row, input) / 0.625;
            assert_eq!(value.fract(), 0.);
            raw[index] = value as u8;
        }
        owner
            .try_apply_logic_height_map_bytes_with_effects(&raw, &mut || {})
            .unwrap();
        let mut water = gamelogic::terrain_water::WaterGridState::default();
        water.enabled = true;
        water.set_resolution(4., 4., 10.);
        water.set_height(number(&row, 2));
        owner.restore_water_grid_state(water);
        let ground = PathfindingGround::receiving(&owner);
        let wet = [(0., 0.), (0., 10.), (10., 10.), (10., 0.)]
            .into_iter()
            .any(|(x, y)| owner.is_underwater(x, y, None, None));
        let mut grid = PathfindingGrid::new(160., 160., 10.);
        let cell = GridPos::new(0, 0);
        grid.set_cell_type(cell, PathfindingGrid::classify_map_cell(row[8] == "1", wet));
        if row[7] == "1" {
            let mut template = ThingTemplate::new("OriginalWaterObstacle");
            template.add_kind_of(KindOf::Structure).set_health(100.);
            template.geometry_info = crate::game_logic::HostGeometryInfo {
                geom_type: crate::game_logic::HostGeometryType::Box,
                authored: true,
                major_radius: 1.,
                minor_radius: 1.,
                ..Default::default()
            };
            let mut object = Object::new(template, ObjectId(1), Team::USA);
            object.set_position(Vec3::new(5., 50., 5.));
            assert!(
                grid.classify_object_footprint(&object, false, &ground)
                    .is_some()
            );
        }
        compare(
            row[0],
            format!("{} type {}", row[0], original_type(grid.cell_type(cell))),
        );
    }
}
