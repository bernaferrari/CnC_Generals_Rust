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

#[test]
#[ignore = "requires native WGPU to record and retain a real scene"]
fn motion_blur_retained_scene_does_not_advance_the_receiving_wake() {
    use crate::assets::AssetManager;
    use crate::graphics::{GraphicsSystem, render_pipeline::RenderPipeline};
    use crate::presentation_frame::{PresentationFrame, camera::viewport::TacticalViewport};
    use game_client::core::game_client::GameClient;
    use game_client::display::view::{FilterMode, FilterType, Vector2};
    use game_client::render_bridge::RenderBridge;
    use game_client::snow::SnowRenderState;
    use game_client::terrain::height_map::HeightMap;
    use std::rc::Rc;

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("native retained water fixture needs a headless renderer");
    let (device, queue, color_format, depth_format) = {
        let engine = engine.lock();
        (engine.device(), engine.queue(), engine.color_format(), engine.depth_format())
    };
    let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format).unwrap();
    let make_visual = || {
        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(HeightMap::new(8, 8, 159.375, 10.), None, None)
            .unwrap();
        visual.water_tracks_mut().add_wave_from_info(
            WaterTrackType::Pond,
            Vec2::new(33.0, 40.0),
            Vec2::new(57.0, 40.0),
            false,
        );
        visual
    };
    let mut first = GameLogic::new();
    let mut peer = GameLogic::new();
    for world in [&mut first, &mut peer] {
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 8;
        map.height = 8;
        map.heightmap = vec![0; 64];
        world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
        world.session_view.filter.set_filter(FilterType::MotionBlur);
        world.session_view.filter.set_mode(FilterMode::MBInAndOutAlpha);
    }
    let mut first_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let viewport = TacticalViewport::for_display(32, 32).unwrap();
    first_pipeline.set_tactical_3d_viewport(viewport);
    peer_pipeline.set_tactical_3d_viewport(viewport);
    first_pipeline.install_native_terrain_cpu(Some(first.with_own_services(make_visual)));
    peer_pipeline.install_native_terrain_cpu(Some(peer.with_own_services(make_visual)));
    let mut first_client = first.with_own_services(|| GameClient::new().unwrap());
    let mut peer_client = peer.with_own_services(|| GameClient::new().unwrap());
    let mut bridge = RenderBridge::new();
    let snow = SnowRenderState { manager: Default::default(), definition: Default::default() };
    let assets = first.with_own_services(|| AssetManager::new().unwrap());
    let elapsed = |pipeline: &mut RenderPipeline| {
        let wakes = pipeline.native_terrain_mut().unwrap().water_tracks_mut();
        assert_eq!(wakes.used_handles().len(), 1);
        wakes.track(wakes.used_handles()[0]).unwrap().elapsed_ms()
    };
    let mut offer = |world: &mut GameLogic,
                     pipeline: &mut RenderPipeline,
                     client: &mut GameClient,
                     frame: u32| {
        world.with_own_services_mut(|world| {
            world.set_current_frame(u64::from(frame));
            let presentation = Rc::new(PresentationFrame::try_build_from_logic(world, 0).unwrap());
            pipeline.set_presentation_frame(Some(presentation));
            let mut modes = crate::cnc_game_engine::mouse::LookAtHostModes::default();
            let mut receiver = super::NativeFrameReceiver {
                audio: None,
                world,
                drawables: client.borrow_native_drawable_owner(),
                look_at_modes: &mut modes,
                world_bounds: (Vec3::ZERO, Vec3::splat(70.0)),
                slave_source: None,
                slave_binding_key: None,
                host_epoch: 19,
                local_player_id: 0,
                assets: &assets,
                logic_frame: frame,
                use_shadow_decals: true,
            };
            pipeline
                .execute(
                    &mut graphics,
                    &mut bridge,
                    &Mat4::IDENTITY,
                    &Mat4::IDENTITY,
                    Vec3::ZERO,
                    0.0,
                    false,
                    0,
                    false,
                    &mut receiver,
                    Vec::new(),
                    None,
                    &snow,
                    Vec2::ZERO,
                    frame,
                    None,
                )
                .unwrap();
        });
        (elapsed(pipeline), engine.lock().last_frame_stats().render_passes)
    };

    let fresh = offer(&mut first, &mut first_pipeline, &mut first_client, 1);
    assert_eq!(fresh.0, 33.0);
    assert!(first.session_view.filter.skip_render());
    let retained = offer(&mut first, &mut first_pipeline, &mut first_client, 2);
    assert!(retained.1 < fresh.1, "retained frame must suppress the actual scene pass");

    // The same filter skip state cannot reuse a capture from another layout owner.
    peer.session_view.filter.post_render(1, Vector2::zero());
    assert!(peer.session_view.filter.skip_render());
    let missing = offer(&mut peer, &mut peer_pipeline, &mut peer_client, 2);
    assert_eq!(missing, fresh);
    assert_eq!(elapsed(&mut first_pipeline), retained.0);

    first.session_view.filter.set_mode(FilterMode::MBInAndOutAlpha);
    assert!(!first.session_view.filter.skip_render());
    let resumed = offer(&mut first, &mut first_pipeline, &mut first_client, 3);
    eprintln!("native wakes (elapsed_ms, render_passes) fresh={fresh:?} retained={retained:?} peer={missing:?} resumed={resumed:?}");
    assert_eq!(retained.0, fresh.0, "a retained scene offers no water draw");
    assert_eq!(resumed, (66.0, fresh.1));
    assert_eq!(elapsed(&mut peer_pipeline), 33.0);
}
