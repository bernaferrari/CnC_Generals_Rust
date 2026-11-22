use super::*;
use crate::presentation_frame::waypoint_overlay::WaypointPathStyle;
use game_client::snow::{SnowManager, SnowRenderState, WeatherSetting};
use game_client::terrain::{
    BreezeInfo,
    height_map::HeightMap,
    terrain_visual::{TerrainBibOwnerKind, TerrainVisualImpl, TerrainVisualMotion},
};
use ww3d_renderer_3d::rendering::wgpu_main_renderer::{FramePhase, ScopedNativePass};

#[test]
fn waypoint_ribbon_uv_uses_goal_ordinals_and_preserves_miter_vertices() {
    let path = WaypointOverlayPath {
        origin: Vec3::new(4.0, 0.0, 8.0),
        goals: vec![Vec3::new(9.0, 0.0, 8.0), Vec3::new(9.0, 0.0, 32.0)],
        style: WaypointPathStyle::Selected,
    };
    let mut packed = Vec::new();
    append_ribbon(&path, Vec3::new(9.0, 60.0, 8.0), &mut packed);
    assert_eq!(packed.len(), 12);
    assert_eq!(packed[0].uv, [0.0, 0.0]);
    assert_eq!(packed[2].uv, [0.0, 1.0]);
    assert_eq!(packed[8].uv, [0.0, 2.0]);
    assert_eq!(packed[2].position, packed[6].position);
    assert_eq!(packed[5].position, packed[7].position);
}

#[test]
#[ignore = "requires a native WGPU device and one installed frame receiver"]
fn native_waypoint_ribbon_is_textured_always_depth_and_precedes_terrain_bibs() {
    const SIZE: u32 = 64;
    const ROW: u32 = SIZE * 4;
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: SIZE,
        height: SIZE,
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
    assert_eq!(graphics.color_format(), wgpu::TextureFormat::Bgra8Unorm);
    let device = graphics.device_arc();
    let queue = graphics.queue_arc();

    let mut render = RenderPipeline::initialize(engine, &graphics).unwrap();
    let mut terrain = TerrainVisualImpl::new();
    terrain
        .load_heightmap_from_data(HeightMap::new(8, 8, 159.375, 10.0), None, None)
        .unwrap();
    terrain.set_skybox_enabled(false);
    let mut water = terrain.water_rules();
    water.water_type = 0;
    terrain.admit_water_rules(water);
    render.install_native_terrain_cpu(Some(terrain));
    render.hydrate_native_terrain_gpu(&device, &queue).unwrap();

    let extent = wgpu::Extent3d {
        width: 2,
        height: 2,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Deterministic EXLaser witness"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Different color channels prove the source texture is actually sampled.
    queue.write_texture(
        texture.as_image_copy(),
        &[80, 24, 32, 255].repeat(4),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        extent,
    );
    render.forward_pass.waypoint_ribbon_gpu = Some(WaypointRibbonGpu::new(
        &device,
        &queue,
        graphics.color_format(),
        Some(texture.create_view(&Default::default())),
    ));

    let eye = Vec3::new(32.0, 100.0, 32.0);
    let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::new(32.0, 0.0, 32.0), Vec3::NEG_Z);
    let projection =
        glam::camera::rh::proj::directx::orthographic(-32.0, 32.0, -32.0, 32.0, 1.0, 200.0);
    let snow = SnowRenderState {
        manager: SnowManager::default(),
        definition: WeatherSetting::default(),
    };
    let mut paint = |render: &mut RenderPipeline, paths: Vec<WaypointOverlayPath>| {
        graphics.begin_frame();
        render.stage_native_waypoint_overlay(&mut graphics, paths);
        let terrain = render.native_terrain.as_mut().unwrap();
        terrain
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
        terrain
            .render_native_y_up(&view, &projection, 0.0, &snow, None)
            .unwrap();
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Native waypoint pixels"),
            size: u64::from(ROW * SIZE),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut capture = |frame: &mut ww3d_engine::RenderFrame, _: &mut GraphicsSystem| {
            let (texture, encoder) = frame.color_texture_and_encoder();
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(ROW),
                        rows_per_image: Some(SIZE),
                    },
                },
                wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
            );
            Ok(None)
        };
        render
            .forward_pass
            .renderer
            .enqueue_scoped_native_pass(FramePhase::Terrain, ScopedNativePass::TerrainGround);
        render
            .forward_pass
            .render(
                &mut graphics,
                &[],
                &view,
                &projection,
                eye,
                None,
                None,
                false,
                Some(&mut capture),
                None,
                None,
                None,
                render.native_terrain.as_ref(),
                (0.0, 0.0, SIZE as f32, SIZE as f32),
                wgpu::Color::BLACK,
            )
            .unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let mapped = readback.slice(..);
        mapped.map_async(wgpu::MapMode::Read, move |result| {
            sent.send(result).unwrap()
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .unwrap();
        received
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        let pixels: Vec<[u8; 4]> = mapped
            .get_mapped_range()
            .unwrap()
            .chunks_exact(4)
            .map(|p| p.try_into().unwrap())
            .collect();
        readback.unmap();
        pixels
    };

    let paths = || {
        [
            WaypointPathStyle::Selected,
            WaypointPathStyle::RevealedEnemy,
        ]
        .into_iter()
        .zip([20.5, 40.5])
        .map(|(style, z)| WaypointOverlayPath {
            // Ground is at zero. A normal LessEqual shader rejects both ribbons.
            origin: Vec3::new(8.0, -1.0, z),
            goals: vec![Vec3::new(54.0, -1.0, z)],
            style,
        })
        .collect()
    };
    let bare = paint(&mut render, Vec::new());
    let with_lines = paint(&mut render, paths());
    let pixel = |x: usize, z: usize| z * SIZE as usize + x;
    let delta = |after: [u8; 4], before: [u8; 4], channel: usize| {
        i16::from(after[channel]) - i16::from(before[channel])
    };
    let selected = pixel(16, 20);
    let enemy = pixel(16, 40);
    // Bgra target. Mesh shader preserves EXLaser's independent channel values.
    assert!((29..=34).contains(&delta(with_lines[selected], bare[selected], 0)));
    assert!((17..=23).contains(&delta(with_lines[selected], bare[selected], 2)));
    assert!((72..=79).contains(&delta(with_lines[enemy], bare[enemy], 2)));
    assert_eq!(with_lines[pixel(16, 19)], bare[pixel(16, 19)]);
    assert!(delta(with_lines[pixel(16, 39)], bare[pixel(16, 39)], 2) > 50);
    assert_eq!(with_lines[pixel(16, 38)], bare[pixel(16, 38)]);

    assert!(render.native_terrain_mut().unwrap().add_faction_bib(
        7,
        TerrainBibOwnerKind::Object,
        Mat4::from_translation(Vec3::new(33.0, 0.0, 20.5))
            * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
        8.0,
        4.0,
        true,
        0.0,
        0.0,
        true,
        0.0,
    ));
    let with_bib = paint(&mut render, Vec::new());
    let with_both = paint(&mut render, paths());
    let overlap = pixel(33, 20);
    assert_ne!(
        with_bib[overlap], bare[overlap],
        "fixture bib overlaps the selected path"
    );
    let clear_line = delta(with_lines[overlap], bare[overlap], 0);
    let bib_line = delta(with_both[overlap], with_bib[overlap], 0);
    assert!(
        bib_line >= 0 && bib_line < clear_line,
        "the bib attenuates the already drawn ribbon"
    );
}

#[test]
#[ignore = "requires a native WGPU device and mounted base Generals archives"]
fn mounted_waypoint_overlay_resolves_and_prepares_base_game_assets() {
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
    let base = crate::fixture_content::retail_content_root().join("Command & Conquer Generals");
    let mut assets = crate::assets::AssetManager::new().unwrap();
    futures::executor::block_on(assets.init(&graphics.device_arc(), &graphics.queue_arc()))
        .expect("initialize the receiving asset manager");
    assets.archive_file_system_mut().set_local_search_roots([]);
    for name in ["W3D.big", "Textures.big"] {
        futures::executor::block_on(assets.archive_file_system_mut().load_big_file(base.join(name)))
            .expect("waypoint assets require the mounted base archive");
    }
    let assets = Arc::new(Mutex::new(assets));
    let mut render = RenderPipeline::initialize(engine, &graphics).unwrap();
    render.bind_application_assets(Arc::clone(&assets));
    render.install_native_terrain_cpu(Some(TerrainVisualImpl::new()));
    render.stage_native_waypoint_overlay(
        &mut graphics,
        vec![WaypointOverlayPath {
            origin: Vec3::new(10.0, 0.0, 10.0),
            goals: vec![Vec3::new(30.0, 0.0, 10.0)],
            style: WaypointPathStyle::Selected,
        }],
    );
    let node = graphics.get_model(NODE_MODEL).expect("retail SCMNode");
    assert_eq!(node.meshes.len(), 1);
    assert!(node.meshes[0].texture_library.iter().any(|name| {
        name.eq_ignore_ascii_case("marker.tga") || name.eq_ignore_ascii_case("marker.dds")
    }));
    let plans = std::mem::take(&mut render.forward_pass.pending_waypoint_overlay);
    let prepared = render
        .forward_pass
        .prepare_native_waypoint_overlay(&graphics, plans, Vec3::Y * 60.0, Mat4::IDENTITY)
        .unwrap();
    assert_eq!(prepared.paths.len(), 1);
    assert_eq!(prepared.paths[0].nodes.len(), 1);
    assert_eq!(prepared.paths[0].ribbon.len(), 6);
    assets
        .lock()
        .unwrap()
        .gpu_texture_for_cached_raw(&graphics.device_arc(), &graphics.queue_arc(), LINE_TEXTURE)
        .expect("the production waypoint stage primed and uploaded retail EXLaser");
}
