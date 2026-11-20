use super::*;
fn visual(raw: u8) -> game_client::terrain::terrain_visual::TerrainVisualImpl {
    let mut map = game_client::terrain::height_map::HeightMap::new(8, 8, 159.375, 10.);
    map.heights.fill(f32::from(raw) / 255.);
    let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    visual.load_heightmap_from_data(map, None, None).unwrap();
    visual
}
#[test]
#[ignore = "requires native GPU for actual RenderPipeline construction"]
fn two_actual_pipeline_terrain_owners_accept_canonical_bytes_without_global_selection() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    // One guard for these reads: separate lock() temporaries in one call
    // keep the first guard until the statement ends (non-reentrant Mutex).
    let (device, queue, color_format, depth_format) = {
        let engine = engine.lock();
        (
            engine.device(),
            engine.queue(),
            engine.color_format(),
            engine.depth_format(),
        )
    };
    let graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
        .unwrap();
    let mut a = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    assert!(
        a.native_terrain().is_none(),
        "pipeline construction is inert"
    );
    a.native_terrain = Some(visual(40));
    let mut b = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    assert!(b.native_terrain().is_none());
    assert_eq!(a.native_terrain().unwrap().get_raw_map_height(1, 1), 40);
    b.native_terrain = Some(visual(80));
    let mut logical = gamelogic::terrain::TerrainLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 8;
    map.height = 8;
    map.heightmap = vec![40; 64];
    logical.try_load_map_geometry(map).unwrap();
    // Install a foreign compatibility visual and hold its guard across the
    // real height change. A callback that selected it would block.
    let compatibility = gamelogic::system::engine_stores::new_for_world();
    gamelogic::system::engine_stores::with_active_stores(&compatibility, || {
        use game_client::terrain::terrain_visual::{get_terrain_visual, init_terrain_visual};
        init_terrain_visual().unwrap();
        let mut foreign = get_terrain_visual().unwrap();
        *foreign = Some(visual(120));
        logical.set_raw_map_height_with_effects(1, 1, -1, &mut |change| {
            a.native_terrain_mut().unwrap().apply_accepted_height(
                change.sample_x,
                change.sample_y,
                change.raw_height,
            );
        });
        assert_eq!(foreign.as_ref().unwrap().get_raw_map_height(1, 1), 120);
    });
    assert_eq!(logical.logic_height_map_bytes()[9], 255);
    assert_eq!(
        a.native_terrain().unwrap().get_raw_map_height(1, 1),
        255,
        "accepted Int narrowing is not clamped/recompared by native presentation"
    );
    assert_eq!(b.native_terrain().unwrap().get_raw_map_height(1, 1), 80);
    b.clear_native_terrain();
    assert_eq!(a.native_terrain().unwrap().get_raw_map_height(1, 1), 255);
    assert!(
        a.native_terrain_mut()
            .unwrap()
            .add_scorch([1., 2., 3.], 5., 0)
    );
    a.hydrate_native_terrain_gpu(&graphics.device_arc(), &graphics.queue_arc())
        .unwrap();
    assert_eq!(
        a.native_terrain().unwrap().get_raw_map_height(1, 1),
        255,
        "GPU hydration must not replace the accepted CPU height map"
    );
    assert_eq!(
        a.native_terrain().unwrap().scorch_marks().len(),
        1,
        "GPU hydration must retain the saved/authored scorch owner"
    );
    a.clear_native_terrain();
    b.clear_native_terrain();
    let request =
        |name: &str, position| gamelogic::object::draw::w3d_prop_draw::TerrainPropAddRequest {
            drawable_id: 42,
            location: gamelogic::common::Coord3D::new(position, 2.0, 3.0),
            orientation: 0.5,
            scale: 1.5,
            model_name: name.into(),
        };
    a.receive_native_prop(1, request("stale", 1.0));
    a.clear_native_terrain();
    a.receive_native_prop(2, request("oak", 10.0));
    a.receive_native_prop(1, request("late", 2.0));
    b.receive_native_prop(3, request("rock", 20.0));
    a.install_native_terrain_cpu(Some(visual(40)));
    b.install_native_terrain_cpu(Some(visual(80)));
    assert_eq!(a.native_terrain().unwrap().terrain_props().len(), 1);
    assert_eq!(
        a.native_terrain().unwrap().terrain_props()[0].model_name,
        "oak"
    );
    assert_eq!(
        b.native_terrain().unwrap().terrain_props()[0].model_name,
        "rock"
    );
    assert_eq!(
        a.native_terrain().unwrap().terrain_props()[0].drawable_id,
        42
    );
    assert_eq!(
        b.native_terrain().unwrap().terrain_props()[0].drawable_id,
        42
    );
    drop(a);
    drop(b);
    drop(graphics);
    drop(engine);
}

#[test]
#[ignore = "requires native GPU for the receiving RenderPipeline"]
fn track_lod_clears_the_live_ring_before_committing_its_receiving_level() {
    use game_client::terrain::terrain_tracks::TerrainTrackHeightProvider;
    use game_engine::common::ini::{
        ini_game_data::TerrainTrackRules,
        ini_game_lod::StaticGameLODLevel::{Custom, High, Low, Unknown},
    };

    struct FlatTrackHeight;
    impl TerrainTrackHeightProvider for FlatTrackHeight {
        fn ground_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
            (0.0, Vec3::Z)
        }
    }

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 16,
        height: 16,
        ..Default::default()
    })
    .unwrap();
    let graphics = {
        let engine = engine.lock();
        GraphicsSystem::new(
            engine.device(),
            engine.queue(),
            engine.color_format(),
            engine.depth_format(),
        )
        .unwrap()
    };
    let mut render = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer = RenderPipeline::initialize(engine, &graphics).unwrap();
    render.install_native_terrain_cpu(Some(visual(40)));
    peer.install_native_terrain_cpu(Some(visual(80)));

    let lay = |renderer: &mut RenderPipeline, slot| {
        let tracks = renderer.native_terrain_mut().unwrap().terrain_tracks_mut();
        for x in [10.0, 25.0, 40.0] {
            tracks.add_edge_to_track(slot, &FlatTrackHeight, x, 20.0, 1000);
        }
    };
    let bind = |renderer: &mut RenderPipeline| {
        renderer
            .native_terrain_mut()
            .unwrap()
            .terrain_tracks_mut()
            .bind_track(14.0, 10.0, "EXTnkTrack.tga")
            .unwrap()
    };
    let count = |renderer: &RenderPipeline, slot| {
        renderer
            .native_terrain()
            .unwrap()
            .terrain_tracks()
            .track(slot)
            .unwrap()
            .active_edge_count()
    };
    let slot = bind(&mut render);
    let peer_slot = bind(&mut peer);
    assert_eq!(slot, peer_slot);
    lay(&mut render, slot);
    lay(&mut peer, peer_slot);
    assert_eq!((count(&render, slot), count(&peer, peer_slot)), (2, 2));

    let mut world = crate::game_logic::GameLogic::new();
    let mut peer_world = crate::game_logic::GameLogic::new();
    world.admit_static_lod_level(Some(Low));
    peer_world.admit_static_lod_level(Some(Low));
    let base = world.effective_terrain_track_rules();
    let peer_base = peer_world.effective_terrain_track_rules();
    let mut preset = base;
    preset.max_tank_track_edges = 12;
    preset.max_tank_track_opaque_edges = 7;
    preset.max_tank_track_fade_delay = 321;
    preset.max_terrain_tracks = 1;
    preset.make_track_marks = false;
    let before = render.native_terrain().unwrap().terrain_tracks().config();

    assert!(
        world
            .lighting_definitions
            .apply_static_lod(High, preset, |owner| {
                assert_eq!(owner.static_lod_level, Some(Low));
                render.set_native_terrain_track_detail(&owner.effective.terrain_tracks);
                assert_eq!(owner.static_lod_level, Some(Low));
                assert_eq!(count(&render, slot), 0);
                assert_eq!(count(&peer, peer_slot), 2);
            })
    );
    assert_eq!(world.current_static_lod_level(), Some(High));
    assert_eq!(peer_world.current_static_lod_level(), Some(Low));
    assert_eq!(peer_world.effective_terrain_track_rules(), peer_base);
    let tracks = render.native_terrain().unwrap().terrain_tracks();
    let after = tracks.config();
    assert_eq!(after.max_tank_track_edges, 12);
    assert_eq!(after.max_tank_track_opaque_edges, 7);
    assert_eq!(after.max_tank_track_fade_delay, 321);
    assert_eq!(after.max_terrain_tracks, before.max_terrain_tracks);
    assert_eq!(after.make_track_marks, before.make_track_marks);
    assert!(tracks.track(slot).unwrap().bound());

    lay(&mut render, slot);
    for unchanged in [High, Unknown] {
        assert!(
            !world
                .lighting_definitions
                .apply_static_lod(unchanged, base, |_| {
                    panic!("unapplied LOD must leave the receiving track ring intact");
                })
        );
        assert_eq!(world.current_static_lod_level(), Some(High));
        assert_eq!(count(&render, slot), 2);
    }

    for previous in [High, Custom] {
        assert!(
            world
                .lighting_definitions
                .apply_static_lod(Custom, base, |owner| {
                    assert_eq!(owner.static_lod_level, Some(previous));
                    render.set_native_terrain_track_detail(&owner.effective.terrain_tracks);
                })
        );
        assert_eq!(count(&render, slot), 0);
        assert_eq!(
            render.native_terrain().unwrap().terrain_tracks().config(),
            after
        );
        lay(&mut render, slot);
    }
    assert_eq!(count(&peer, peer_slot), 2);

    render.clear_native_terrain();
    assert!(world.lighting_definitions.apply_static_lod(
        Low,
        TerrainTrackRules::default(),
        |owner| {
            render.set_native_terrain_track_detail(&owner.effective.terrain_tracks);
        }
    ));
    assert_eq!(world.current_static_lod_level(), Some(Low));
    assert!(render.native_terrain().is_none());
    assert_eq!(count(&peer, peer_slot), 2);
}
