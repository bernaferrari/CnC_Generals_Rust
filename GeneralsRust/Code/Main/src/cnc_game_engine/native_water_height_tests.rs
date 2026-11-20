use crate::game_logic::GameLogic;
use game_client::terrain::terrain_visual::{TerrainVisualImpl, TerrainVisualMotion};
use game_client::terrain::{BreezeInfo, water_tracks::WaterTrackType};
use gamelogic::common::ICoord3D;
use gamelogic::polygon_trigger::PolygonTrigger;
use glam::{Mat4, Vec2, Vec3};

#[test]
fn native_water_tracks_sample_the_receiving_polygon_and_visual_grid() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_water_tracks_sample_the_receiving_polygon_and_visual_grid",
        || {
            let first = GameLogic::new();
            let peer = GameLogic::new();
            install_water_areas(&first, 25, 22);
            install_water_areas(&peer, 17, 42);
            let mut first_visual = receiving_visual(35.0);
            let mut peer_visual = receiving_visual(9.0);

            flush_native(&first, &mut first_visual);
            assert_eq!(wake_heights(&first_visual), [26.5, 36.5, 4.5]);
            flush_native(&peer, &mut peer_visual);
            assert_eq!(wake_heights(&peer_visual), [18.5, 43.5, 4.5]);
            assert_eq!(wake_heights(&first_visual), [26.5, 36.5, 4.5]);

            // The renderer samples both current surfaces at each flush.
            first
                .world_services
                .terrain()
                .write()
                .unwrap()
                .mutate_named_water_height(&"Water8".into(), 30.75, 0.0, false)
                .unwrap();
            first_visual.set_water_transform(std::f32::consts::FRAC_PI_2, 420.0, 210.0, 37.0);
            flush_native(&first, &mut first_visual);
            assert_eq!(wake_heights(&first_visual), [31.5, 38.5, 4.5]);
            assert_eq!(wake_heights(&peer_visual), [18.5, 43.5, 4.5]);
        },
    );
}

#[test]
fn native_water_tracks_keep_the_authored_grid_lookup_after_matrix_only_delivery() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_water_tracks_keep_the_authored_grid_lookup_after_matrix_only_delivery",
        || {
            let world = GameLogic::new();
            install_water_areas(&world, 25, 22);
            let mut visual = receiving_visual(35.0);
            let authored = visual.capture_water_state();
            visual.set_water_transform(0.0, -1000.0, 900.0, 35.0);
            visual.admit_map_water_configuration(&authored);
            flush_native(&world, &mut visual);
            assert_eq!(wake_heights(&visual), [26.5, 36.5, 4.5]);

            // Only the rendering transform changes. The original world-to-grid
            // basis still comes from the authored map angle setter.
            let mut moved = authored;
            moved.transform = Mat4::from_translation(Vec3::new(2200.0, 3100.0, 39.0))
                * Mat4::from_rotation_z(0.31);
            visual.admit_water_state(&moved);
            assert_eq!(visual.water_transform(), moved.transform);
            flush_native(&world, &mut visual);
            assert_eq!(wake_heights(&visual), [26.5, 40.5, 4.5]);

            visual.set_water_transform(0.31, 2200.0, 3100.0, 39.0);
            flush_native(&world, &mut visual);
            assert_eq!(wake_heights(&visual), [26.5, 23.5, 4.5]);
        },
    );
}

fn install_water_areas(world: &GameLogic, highest: i32, grid_polygon: i32) {
    let water_area = |id, min: [i32; 2], max: [i32; 2], height| {
        let mut trigger = PolygonTrigger::new(
            id,
            format!("Water{id}").as_str().into(),
            vec![
                ICoord3D::new(min[0], min[1], height),
                ICoord3D::new(max[0], min[1], height),
                ICoord3D::new(max[0], max[1], height),
                ICoord3D::new(min[0], max[1], height),
            ],
        );
        trigger.set_water_area(true);
        trigger
    };
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .replace_receiving_trigger_areas([
            water_area(7, [60, 45], [230, 210], 10),
            water_area(8, [70, 60], [220, 205], highest),
            water_area(9, [250, 215], [430, 440], grid_polygon),
        ]);
}

fn receiving_visual(grid_height: f32) -> TerrainVisualImpl {
    let mut visual = TerrainVisualImpl::new();
    let mut rules = visual.water_rules();
    rules.position[2] = 3.0;
    visual.admit_water_rules(rules);
    visual.enable_water_grid(true);
    visual.set_water_grid_resolution(7.0, 7.0, 30.0);
    visual.set_water_transform(std::f32::consts::FRAC_PI_2, 420.0, 210.0, grid_height);
    for start in [
        Vec2::new(140.0, 125.0),
        Vec2::new(350.0, 260.0),
        Vec2::new(700.0, 700.0),
    ] {
        visual.water_tracks_mut().add_wave_from_info(
            WaterTrackType::Pond,
            start,
            start + Vec2::Y * 28.0,
            false,
        );
    }
    visual
}

fn flush_native(world: &GameLogic, visual: &mut TerrainVisualImpl) {
    world.with_own_services(|| {
        let motion = TerrainVisualMotion {
            paused: true,
            frame: 90,
            breeze: BreezeInfo::default(),
            gravity: -1.0,
        };
        let trees = visual.tree_buffer_mut();
        let cursor = trees.begin_cpu_draw(true);
        let receipt = trees.finish_cpu_draw(cursor).unwrap();
        visual
            .update_native_after_tree_cpu_with_water_tracks(
                motion,
                Some(&receipt),
                Some(&mut |visual| super::flush_receiving_water_tracks(world, visual)),
            )
            .unwrap();
    });
}

fn wake_heights(visual: &TerrainVisualImpl) -> [f32; 3] {
    let mut heights = [f32::NAN; 3];
    let flush = visual.last_water_tracks_flush();
    assert_eq!(flush.ranges.len(), 3);
    for range in &flush.ranges {
        let vertices = &flush.vertices[range.first_vertex..range.first_vertex + range.vertex_count];
        heights[range.handle] = vertices[0].z;
        assert!(vertices.iter().all(|vertex| vertex.z == vertices[0].z));
    }
    heights
}
