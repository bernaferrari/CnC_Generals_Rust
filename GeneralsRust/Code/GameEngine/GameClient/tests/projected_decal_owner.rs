use game_client_rust::core::game_client::GameClient;
use game_client_rust::radius_decal::LiveTerrainDecalSpec;

fn spec(x: f32) -> LiveTerrainDecalSpec {
    LiveTerrainDecalSpec {
        texture: "same-name.tga".into(),
        radius: 2.0,
        position: [x, 3.0, 4.0],
        color: [255, 255, 255],
        opacity: 1.0,
    }
}

#[test]
fn live_terrain_decals_remain_on_their_receiving_client() {
    use gamelogic::system::engine_stores::{new_world_services, with_world_services};

    let first_world = new_world_services();
    let second_world = new_world_services();
    let mut first = with_world_services(&first_world, || GameClient::new().unwrap());
    let mut second = with_world_services(&second_world, || GameClient::new().unwrap());
    let first_items = first.collect_native_radius_render_items([spec(10.0)]);
    let second_items = second.collect_native_radius_render_items([spec(20.0)]);

    assert_eq!(first_items.len(), 1);
    assert_eq!(second_items.len(), 1);
    assert_eq!(first_items[0].texture_name, "same-name.tga");
    assert_eq!(first_items[0].position.x, 10.0);
    assert_eq!(second_items[0].position.x, 20.0);
    assert!(first.collect_native_radius_render_items([]).is_empty());
    assert_eq!(second_items[0].position.x, 20.0);
    let next_second = second.collect_native_radius_render_items([spec(21.0)]);
    assert_eq!(next_second.len(), 1);
    assert_eq!(next_second[0].position.x, 21.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires serialized native GPU and mounted retail map and texture archives"]
fn receiving_live_decal_rows_keep_pixels_and_remove_absent_casters() {
    use game_client_rust::display::image::load_image_from_engine_filesystem;
    use game_client_rust::effects::particle_renderer::{ParticleRenderer, ParticleUniforms};
    use game_client_rust::radius_decal::{
        ProjectedShadowManager, enqueue_delivery_decal_argb_with_owner,
    };
    use game_engine::common::system::big_file_system::BigArchiveBackend;
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::subsystem_interface::SubsystemInterface;
    use gamelogic::system::engine_stores::{new_world_services, with_world_services};
    use std::path::Path;
    use std::sync::Arc;

    let base = std::env::var_os("GENERALS_BASE_DIR").expect("mounted retail base required");
    let source = tempfile::tempdir().unwrap();
    std::fs::hard_link(
        Path::new(&base).join("Data/Textures.big"),
        source.path().join("Textures.big"),
    )
    .unwrap();
    let world = new_world_services();
    with_world_services(&world, || {
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(source.path());
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }
        for name in ["EXHorde.tga", "EXJunkCrate.tga"] {
            let image = load_image_from_engine_filesystem(name).unwrap();
            assert_eq!((image.width(), image.height()), (128, 128));
        }

        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let (device, queue) = (Arc::new(device), Arc::new(queue));
        let mut renderer = ParticleRenderer::new(
            Arc::clone(&device),
            Arc::clone(&queue),
            wgpu::TextureFormat::Bgra8Unorm,
        )
        .unwrap();
        let uniforms = ParticleUniforms {
            screen_size: [32.0, 32.0],
            ..Default::default()
        };

        let mut client = GameClient::new().unwrap();
        let persistent = client
            .borrow_native_drawable_owner()
            .allocate_projected_radius("EXHordeB", 0.7, [-0.16, 0.1, 0.0], 0x0050_D077, 0.87)
            .unwrap();
        let live = [
            LiveTerrainDecalSpec {
                texture: "EXHorde".into(),
                radius: 0.58,
                position: [0.1, -0.1, 0.0],
                color: [185, 90, 40],
                opacity: 0.63,
            },
            LiveTerrainDecalSpec {
                texture: "EXJunkCrate".into(),
                radius: 0.5,
                position: [0.24, 0.05, 0.0],
                color: [60, 110, 210],
                opacity: 0.71,
            },
        ];

        let legacy_rows = |fixed: bool, dynamic: &[LiveTerrainDecalSpec]| {
            let mut manager = ProjectedShadowManager::new();
            if fixed {
                enqueue_delivery_decal_argb_with_owner(
                    &mut manager,
                    "EXHordeB",
                    0.7,
                    -0.16,
                    0.1,
                    0.0,
                    0x0050_D077,
                    0.87,
                )
                .unwrap();
            }
            for spec in dynamic {
                let color = u32::from_be_bytes([0, spec.color[0], spec.color[1], spec.color[2]]);
                enqueue_delivery_decal_argb_with_owner(
                    &mut manager,
                    &spec.texture,
                    spec.radius,
                    spec.position[0],
                    spec.position[1],
                    spec.position[2],
                    color,
                    spec.opacity,
                )
                .unwrap();
            }
            manager.collect_render_items_with_policy(false)
        };
        let mut draw = |items: &[game_client_rust::effects::decals::DecalRenderItem]| {
            read_projected_decal_pixels(&mut renderer, &device, &queue, items, &uniforms)
        };

        let mut specs = live.to_vec();
        specs.extend([
            LiveTerrainDecalSpec {
                opacity: 0.001,
                ..live[0].clone()
            },
            LiveTerrainDecalSpec {
                radius: f32::NAN,
                ..live[0].clone()
            },
            LiveTerrainDecalSpec {
                texture: String::new(),
                ..live[0].clone()
            },
        ]);
        let texture_storage = specs[0].texture.as_ptr();
        let full = client.collect_native_radius_render_items(specs);
        assert_eq!(full.len(), 3);
        assert_eq!(
            full.iter()
                .map(|item| item.texture_name.as_str())
                .collect::<Vec<_>>(),
            ["EXHordeB", "EXHorde", "EXJunkCrate"]
        );
        let full_pixels = draw(&full);
        assert_eq!(full_pixels, draw(&legacy_rows(true, &live)));
        let mut missing_texture = full.clone();
        missing_texture[1].texture_name = "missing-owner-decal-probe.tga".into();
        assert_ne!(full_pixels, draw(&missing_texture));
        assert_eq!(
            full[1].texture_name.as_ptr(),
            texture_storage,
            "the receiving draw row should retain the frame's texture allocation"
        );
        assert_eq!(
            client
                .borrow_native_drawable_owner()
                .collect_projected_decal_items(false)
                .len(),
            1
        );

        let updated = client.collect_native_radius_render_items(vec![live[0].clone()]);
        assert_eq!(updated.len(), 2);
        let updated_pixels = draw(&updated);
        assert_eq!(updated_pixels, draw(&legacy_rows(true, &live[..1])));
        assert_ne!(full_pixels, updated_pixels);

        persistent.release();
        let only_live = client.collect_native_radius_render_items(vec![live[0].clone()]);
        assert_eq!(only_live.len(), 1);
        let live_pixels = draw(&only_live);
        assert_eq!(live_pixels, draw(&legacy_rows(false, &live[..1])));
        let cleared = client.collect_native_radius_render_items(Vec::new());
        assert!(cleared.is_empty());
        assert_ne!(live_pixels, draw(&cleared));
        eprintln!(
            "mounted decal row handoffs 3→2→1→0, RGBA CRCs {:08x} {:08x} {:08x}",
            crc32fast::hash(&full_pixels),
            crc32fast::hash(&updated_pixels),
            crc32fast::hash(&live_pixels)
        );
        check_sloped_decal(&mut renderer, &device, &queue, only_live[0].clone());
        check_decal_batch_order(&mut renderer, &device, &queue, &only_live[0], &uniforms);
        check_retail_additive_grid_style(&mut renderer, &device, &queue, &uniforms);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn check_sloped_decal(
    renderer: &mut game_client_rust::effects::particle_renderer::ParticleRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut decal: game_client_rust::effects::decals::DecalRenderItem,
) {
    use game_client_rust::effects::particle_renderer::ParticleUniforms;
    use game_engine::common::system::big_file_system::BigFileSystem;
    use gamelogic::system::map_loader::MapLoader;
    use gamelogic::terrain::{TerrainLogic, get_terrain_logic};
    use glam::{Mat4, Vec3, Vec4};
    use std::io::Read;
    use std::path::Path;

    let retail = std::env::var_os("GENERALS_ASSETS_DIR").expect("mounted Zero Hour required");
    let mut maps = BigFileSystem::new();
    maps.open_archive_file(Path::new(&retail).join("Data/MapsZH.big"))
        .unwrap();
    let mut bytes = Vec::new();
    maps.open_file("Maps/Lone Eagle/Lone Eagle.map", 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut map = MapLoader::new();
    map.load_map_from_bytes(&bytes).unwrap();
    let terrain = get_terrain_logic();
    let center = {
        let mut terrain = terrain.write().unwrap();
        terrain.try_load_map_geometry(map.to_map_data()).unwrap();
        let (x, y) = (280.0, 200.0);
        let lower = terrain.get_ground_height(x - 15.0, y - 10.0, None);
        let upper = terrain.get_ground_height(x + 15.0, y + 10.0, None);
        assert!((upper - lower).abs() > 8.0, "authored Lone Eagle slope");
        Vec3::new(x, y, terrain.get_ground_height(x, y, None))
    };

    decal.position = center;
    decal.size_x = 43.0;
    decal.size_y = 33.0;
    decal.rotation = 0.57;
    decal.uv_offset = [0.017, -0.024];
    let mut uniforms = ParticleUniforms {
        view_matrix: Mat4::from_translation(-center).to_cols_array_2d(),
        projection_matrix: Mat4::from_cols(
            Vec4::new(1.0 / 35.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / 35.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / 75.0, 1.0 / 100.0, 0.0),
            Vec4::new(0.0, 0.0, 0.5, 1.0),
        )
        .to_cols_array_2d(),
        screen_size: [32.0, 32.0],
        ..Default::default()
    };
    let sloped = read_projected_decal_pixels(
        renderer,
        device,
        queue,
        std::slice::from_ref(&decal),
        &uniforms,
    );
    assert_ne!(
        sloped,
        read_projected_decal_pixels(renderer, device, queue, &[], &uniforms)
    );

    *terrain.write().unwrap() = TerrainLogic::new();
    uniforms.view_matrix =
        Mat4::from_translation(Vec3::new(-center.x, -center.y, 0.0)).to_cols_array_2d();
    let flat = read_projected_decal_pixels(renderer, device, queue, &[decal], &uniforms);
    assert_ne!(
        flat,
        read_projected_decal_pixels(renderer, device, queue, &[], &uniforms)
    );
    assert_ne!(
        sloped, flat,
        "the mounted height samples affect native output"
    );
    eprintln!(
        "Lone Eagle slope at {center:?}, sloped and flat decal CRCs {:08x} {:08x}",
        crc32fast::hash(&sloped),
        crc32fast::hash(&flat)
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn check_decal_batch_order(
    renderer: &mut game_client_rust::effects::particle_renderer::ParticleRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    base: &game_client_rust::effects::decals::DecalRenderItem,
    uniforms: &game_client_rust::effects::particle_renderer::ParticleUniforms,
) {
    use game_client_rust::effects::decals::DecalRenderItem;
    use glam::Vec3;

    let row = |x, y, color| DecalRenderItem {
        color,
        position: Vec3::new(x, y, 0.0),
        ..base.clone()
    };
    let mut missing = row(0.15, -0.13, [0.13, 0.46, 0.95, 0.36]);
    missing.texture_name = "missing-adjacent-radius-decal.tga".into();

    let rows = [
        row(-0.17, 0.15, [0.68, 0.15, 0.14, 0.65]),
        row(-0.15, 0.16, [0.19, 0.77, 0.32, 0.75]),
        DecalRenderItem {
            shadow_type: 0x40,
            ..row(-0.13, 0.14, [0.33, 0.19, 0.86, 0.3])
        },
        row(-0.12, 0.12, [0.86, 0.68, 0.14, 0.58]),
        missing.clone(),
        DecalRenderItem {
            position: Vec3::new(0.11, -0.13, 0.0),
            color: [0.64, 0.26, 0.2, 0.52],
            ..missing
        },
        row(0.15, -0.11, [0.18, 0.51, 0.24, 0.68]),
    ];

    let before = renderer.stats.draw_calls;
    let ordered = read_projected_decal_pixels(renderer, device, queue, &rows, uniforms);
    assert_eq!(renderer.stats.draw_calls - before, 5);
    let mut changed_order = rows.clone();
    changed_order.swap(2, 3);
    assert_ne!(
        ordered,
        read_projected_decal_pixels(renderer, device, queue, &changed_order, uniforms),
        "an intervening additive row with the same texture must retain its order"
    );
    let mut changed_missing_order = rows.clone();
    changed_missing_order.swap(5, 6);
    assert_ne!(
        ordered,
        read_projected_decal_pixels(renderer, device, queue, &changed_missing_order, uniforms),
        "a fallback-textured row must retain its order relative to loaded art"
    );
    let mut loaded_only = rows;
    loaded_only[4].texture_name.clone_from(&base.texture_name);
    loaded_only[5].texture_name.clone_from(&base.texture_name);
    assert_ne!(
        ordered,
        read_projected_decal_pixels(renderer, device, queue, &loaded_only, uniforms)
    );
    eprintln!(
        "7 ordered decals in 5 receiving batches, native BGRA CRC {:08x}",
        crc32fast::hash(&ordered)
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn read_projected_decal_pixels(
    renderer: &mut game_client_rust::effects::particle_renderer::ParticleRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    items: &[game_client_rust::effects::decals::DecalRenderItem],
    uniforms: &game_client_rust::effects::particle_renderer::ParticleUniforms,
) -> Vec<u8> {
    let size = wgpu::Extent3d {
        width: 32,
        height: 32,
        depth_or_array_layers: 1,
    };
    let attachment = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
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
        ww3d_gpu::SCENE_DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.begin_effects_frame();
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.06,
                        b: 0.07,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                stencil_ops: None,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
            }),
            ..Default::default()
        });
        renderer.record_decals(&mut pass, items, uniforms);
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    let submission = queue.submit([encoder.finish()]);
    let (mapped, wait) = std::sync::mpsc::channel();
    output
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            mapped.send(result).unwrap()
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    wait.recv().unwrap().unwrap();
    let pixels = output.slice(..).get_mapped_range().unwrap();
    pixels
        .chunks_exact(256)
        .flat_map(|row| row[..(4 * size.width) as usize].iter().copied())
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn check_retail_additive_grid_style(
    renderer: &mut game_client_rust::effects::particle_renderer::ParticleRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    uniforms: &game_client_rust::effects::particle_renderer::ParticleUniforms,
) {
    use game_client_rust::display::image::load_image_from_engine_filesystem;
    use game_client_rust::effects::decals::DecalRenderItem;
    use game_client_rust::radius_decal::{ProjectedShadowManager, ShadowTypeInfo};

    let grid_image = load_image_from_engine_filesystem("EXGrid.tga").expect("retail EXGrid atlas");
    assert!(grid_image.to_rgba8().pixels().any(|pixel| pixel[2] > 0));

    let ordinary = DecalRenderItem {
        texture_name: "EXGrid.tga".into(),
        size: 1.5,
        color: [0.74, 0.11, 0.17, 0.62],
        shadow_type: gamelogic::common::SHADOW_ALPHA_DECAL,
        ..Default::default()
    };
    let projection = DecalRenderItem {
        shadow_type: gamelogic::common::SHADOW_PROJECTION,
        ..ordinary.clone()
    };
    let alpha_pixels = read_projected_decal_pixels(renderer, device, queue, &[ordinary], uniforms);
    assert_eq!(
        alpha_pixels,
        read_projected_decal_pixels(renderer, device, queue, &[projection], uniforms)
    );
    assert_ne!(
        alpha_pixels,
        read_projected_decal_pixels(renderer, device, queue, &[], uniforms)
    );
    eprintln!(
        "EXGrid exact alpha/projection BGRA CRC {:08x}",
        crc32fast::hash(&alpha_pixels)
    );

    let mut manager = ProjectedShadowManager::new();
    let grid = manager
        .add_decal(&ShadowTypeInfo {
            allow_updates: false,
            allow_world_align: true,
            shadow_type: gamelogic::common::SHADOW_ADDITIVE_DECAL,
            shadow_name: "EXGrid".into(),
            size_x: 1.5,
            size_y: 1.5,
            offset_x: 0.0,
            offset_y: 0.0,
        })
        .unwrap();
    assert_eq!(
        manager.collect_render_items_with_policy(false)[0].color,
        [1.0; 4]
    );
    grid.set_color(0x0020_4080);
    let expected = [0.0, 0.0, 224.0 / 255.0, 0.0];
    assert_eq!(
        manager.collect_render_items_with_policy(false)[0].color,
        expected
    );
    for (opacity, blue) in [(246, 127u8), (161, 124), (8, 7)] {
        grid.set_opacity(opacity);
        assert_eq!(
            manager.collect_render_items_with_policy(false)[0].color,
            [0.0, 0.0, f32::from(blue) / 255.0, 0.0],
        );
    }
    grid.set_opacity(255);
    let actual = manager.collect_render_items_with_policy(false);
    assert_eq!(actual.len(), 1);

    // Original Shadow.h packs opacity-scaled channels with an unshifted OR.
    // W3D flushDecals then uses ONE/ONE, including this zero-alpha input.
    let original_style = DecalRenderItem {
        color: expected,
        ..actual[0].clone()
    };
    let cleared = read_projected_decal_pixels(renderer, device, queue, &[], uniforms);
    let blended = read_projected_decal_pixels(renderer, device, queue, &[original_style], uniforms);
    let blue_changes = blended
        .chunks_exact(4)
        .zip(cleared.chunks_exact(4))
        .filter(|(before, after)| before[0] > after[0])
        .count();
    assert!(
        blended
            .chunks_exact(4)
            .zip(cleared.chunks_exact(4))
            .all(|(rgb, base)| rgb[1..] == base[1..])
    );
    eprintln!(
        "EXGrid collected diffuse {:?} / original {:?}. Original packed row native blue deltas: {}. BGRA CRC {:08x} / clear {:08x}",
        actual[0].color,
        expected,
        blue_changes,
        crc32fast::hash(&blended),
        crc32fast::hash(&cleared)
    );
    assert_eq!(
        (actual[0].color, blue_changes > 0),
        (expected, true),
        "authored additive 0-alpha RGB+destination must preserve the raster output"
    );
    assert_eq!(
        blended,
        read_projected_decal_pixels(renderer, device, queue, &actual, uniforms)
    );
}
