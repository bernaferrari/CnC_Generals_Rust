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
#[test]
#[ignore = "requires native GPU for the receiving terrain shroud pass"]
fn terrain_shroud_draw_uses_frozen_border_levels_and_tint() {
    use crate::fow_rendering::{
        PresentationFowGrid, ProjectedShroudMetadata, ProjectedShroudSnapshot,
    };
    use crate::presentation_frame::PresentationFrame;

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 64,
        height: 64,
        ..Default::default()
    })
    .unwrap();
    let mut graphics = {
        let engine = engine.lock();
        GraphicsSystem::new(
            engine.device(),
            engine.queue(),
            engine.color_format(),
            engine.depth_format(),
        )
        .unwrap()
    };
    let mut render = RenderPipeline::initialize(engine, &graphics).unwrap();
    let mut terrain = visual(0);
    terrain.set_skybox_enabled(false);
    let mut water = terrain.water_rules();
    water.water_type = 0;
    terrain.admit_water_rules(water);
    render.install_native_terrain_cpu(Some(terrain));
    render
        .hydrate_native_terrain_gpu(&graphics.device_arc(), &graphics.queue_arc())
        .unwrap();
    let device = graphics.device_arc();
    let queue = graphics.queue_arc();
    let paint = |render: &mut RenderPipeline, frame: Option<PresentationFrame>| {
        render.set_presentation_frame(frame.map(Rc::new));
        render.update_minimap_fow_texture().unwrap();
        terrain_shroud_pixels(render.native_terrain_mut().unwrap(), &device, &queue)
    };
    let baseline = paint(&mut render, None);

    let mut logic = crate::game_logic::GameLogic::new();
    let mut frame = PresentationFrame::build_from_logic(&mut logic, 0);
    frame.fow_shell_bypass = false;
    frame.fow_grid = PresentationFowGrid {
        width: 2,
        height: 2,
        world_origin_xy: [19.5, 19.5],
        cell_size: 10.0,
        cells: vec![
            PresentationFowGrid::CELL_VISIBLE,
            PresentationFowGrid::CELL_EXPLORED,
            PresentationFowGrid::CELL_HIDDEN,
            PresentationFowGrid::CELL_VISIBLE,
        ].into(),
        active: true,
    };
    let mut metadata = ProjectedShroudMetadata {
        shroud_color_rgb: [128, 255, 64],
        clear_alpha: 255,
        fog_alpha: 96,
        shroud_alpha: 24,
        border_alpha: 80,
    };
    frame.projected_shroud = ProjectedShroudSnapshot::from_grid(&frame.fow_grid, metadata);
    let original_mask = std::sync::Arc::downgrade(&frame.projected_shroud.texels);
    assert_eq!(original_mask.strong_count(), 1);
    let paint_shroud = paint(&mut render, Some(frame.clone()));
    assert_eq!(original_mask.strong_count(), 3);

    let check = |pixels: &[[u8; 4]], x: usize, y: usize, fog: u8, fully_lit_weight: f32| {
        let index = x + y * 64;
        let source = baseline[index];
        let actual = pixels[index];
        let tint_bgr = [64.0, 255.0, 128.0];
        assert!(
            source[..3].iter().all(|&channel| channel > 10),
            "clear terrain at {x},{y}: {source:?}"
        );
        for channel in 0..3 {
            let dimmed = f32::from(fog) * tint_bgr[channel] / (255.0 * 255.0);
            let factor = fully_lit_weight + dimmed * (1.0 - fully_lit_weight);
            let expected = (f32::from(source[channel]) * factor).round() as u8;
            assert!(
                actual[channel].abs_diff(expected) <= 1,
                "terrain shroud pixel ({x},{y}) channel {channel}: {actual:?} from {source:?}, expected {expected}"
            );
        }
    };
    check(&paint_shroud, 8, 24, 80, 0.0); // padded border, outside the logical grid
    check(&paint_shroud, 24, 24, 255, 1.0); // full clear stays white
    check(&paint_shroud, 34, 24, 96, 0.0); // authored fog
    check(&paint_shroud, 24, 34, 24, 0.0); // authored hidden
    check(&paint_shroud, 29, 24, 96, 0.5); // center of clear/fog bilinear transition

    metadata.border_alpha = 196;
    frame.projected_shroud = ProjectedShroudSnapshot::from_grid(&frame.fow_grid, metadata);
    let changed_border = paint(&mut render, Some(frame.clone()));
    assert_eq!(original_mask.strong_count(), 0);
    let changed_mask = std::sync::Arc::downgrade(&frame.projected_shroud.texels);
    assert_eq!(changed_mask.strong_count(), 3);
    check(&changed_border, 8, 24, 196, 0.0);
    assert_eq!(changed_border[24 + 24 * 64], paint_shroud[24 + 24 * 64]);

    let snapshot = frame.terrain_projected_shroud().unwrap();
    let object_projection =
        draw_forward_shroud(&mut render, &mut graphics, Some(snapshot)).unwrap();
    assert_eq!(
        object_projection.content_fingerprint,
        snapshot.content_fingerprint()
    );
    assert_eq!(
        object_projection.shroud_color,
        [128.0 / 255.0, 1.0, 64.0 / 255.0]
    );
    assert_eq!(
        render.forward_pass.projected_shroud_uploader.upload_count(),
        0,
        "the exact receiving terrain texture must also serve the object pass"
    );
    let mut stale = snapshot.clone();
    std::sync::Arc::make_mut(&mut stale.texels)[0] ^= 1;
    let fallback = draw_forward_shroud(&mut render, &mut graphics, Some(&stale)).unwrap();
    assert_eq!(fallback.content_fingerprint, stale.content_fingerprint());
    assert_eq!(
        render.forward_pass.projected_shroud_uploader.upload_count(),
        1
    );
    assert!(
        render
            .forward_pass
            .projected_shroud_uploader
            .resident_state()
            .is_some()
    );
    let again = draw_forward_shroud(&mut render, &mut graphics, Some(snapshot)).unwrap();
    assert_eq!(again, object_projection);
    assert_eq!(
        render.forward_pass.projected_shroud_uploader.upload_count(),
        1
    );
    assert!(
        render
            .forward_pass
            .projected_shroud_uploader
            .resident_state()
            .is_none()
    );

    frame.fow_shell_bypass = true;
    assert_eq!(paint(&mut render, Some(frame.clone())), baseline);
    assert!(draw_forward_shroud(&mut render, &mut graphics, None).is_none());
    assert_eq!(changed_mask.strong_count(), 2);
    frame.fow_shell_bypass = false;
    assert_eq!(paint(&mut render, Some(frame)), changed_border);
    assert_eq!(changed_mask.strong_count(), 2);
    assert_eq!(paint(&mut render, None), baseline);
    assert_eq!(changed_mask.strong_count(), 0);
    assert!(draw_forward_shroud(&mut render, &mut graphics, None).is_none());
}

fn draw_forward_shroud(
    render: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    snapshot: Option<&crate::fow_rendering::ProjectedShroudSnapshot>,
) -> Option<ww3d_renderer_3d::rendering::projected_shroud::ProjectedShroudProjection> {
    let eye = Vec3::new(32.0, 100.0, 32.0);
    let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::new(32.0, 0.0, 32.0), Vec3::NEG_Z);
    let projection =
        glam::camera::rh::proj::directx::orthographic(-32.0, 32.0, -32.0, 32.0, 1.0, 200.0);
    render
        .forward_pass
        .render(
            graphics,
            &[],
            &view,
            &projection,
            eye,
            None,
            snapshot,
            false,
            None,
            None,
            None,
            None,
            render.native_terrain.as_ref(),
            (0.0, 0.0, 64.0, 64.0),
            wgpu::Color::BLACK,
        )
        .unwrap();
    render
        .forward_pass
        .renderer
        .renderer_handle()
        .lock()
        .unwrap()
        .projected_shroud_projection()
}

fn terrain_shroud_pixels(
    visual: &mut game_client::terrain::terrain_visual::TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<[u8; 4]> {
    use game_client::{
        snow::{SnowManager, SnowRenderState, WeatherSetting},
        terrain::{BreezeInfo, terrain_visual::TerrainVisualMotion},
    };

    let snow = SnowRenderState {
        manager: SnowManager::default(),
        definition: WeatherSetting::default(),
    };
    let view = glam::camera::rh::view::look_at_mat4(
        Vec3::new(32.0, 100.0, 32.0),
        Vec3::new(32.0, 0.0, 32.0),
        Vec3::NEG_Z,
    );
    let projection =
        glam::camera::rh::proj::directx::orthographic(-32.0, 32.0, -32.0, 32.0, 1.0, 200.0);
    visual
        .render_native_y_up(&view, &projection, 0.0, &snow, None)
        .unwrap();
    visual
        .update_native(
            TerrainVisualMotion {
                paused: false,
                frame: 31,
                breeze: BreezeInfo::default(),
                gravity: -1.0,
            },
            None,
        )
        .unwrap();

    const ROW: u32 = 64 * 4;
    let size = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 1,
    };
    let attachment = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain projected shroud fixture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = attachment(
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = attachment(
        wgpu::TextureFormat::Depth24PlusStencil8,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrain projected shroud pixels"),
        size: u64::from(ROW * size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color.create_view(&Default::default()),
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.create_view(&Default::default()),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        visual.record_chunk_draws(&mut pass);
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    let submitted = queue.submit([encoder.finish()]);
    let (done, received) = std::sync::mpsc::channel();
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        done.send(result).unwrap()
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submitted),
            timeout: Some(std::time::Duration::from_secs(20)),
        })
        .unwrap();
    received
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap();
    let pixels = slice
        .get_mapped_range()
        .unwrap()
        .chunks_exact(4)
        .map(|pixel| pixel.try_into().unwrap())
        .collect();
    readback.unmap();
    pixels
}
