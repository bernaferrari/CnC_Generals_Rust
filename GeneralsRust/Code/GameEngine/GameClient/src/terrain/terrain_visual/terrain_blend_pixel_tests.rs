use super::*;

/// Exercises the actual extra-blend upload and WGPU record path over a
/// two-material base. The third material is a solid red authored overlay.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a serialized native GPU"]
fn three_way_blend_setting_changes_actual_terrain_pixels() {
    use crate::terrain::textures::BlendTileInfo;
    use wgpu::util::DeviceExt;
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    })).expect("terrain blend pixel test requires a GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("terrain blend pixel test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    })).expect("terrain blend pixel test requires a GPU device");
    let device = Arc::new(device);

    fn render(device: &Arc<wgpu::Device>, queue: &wgpu::Queue, setting: i32) -> [u8; 4] {
        let mut map = HeightMap::new(3, 3, 0.0, 1.0);
        map.tile_ndxes[0] = 1;
        map.blend_tile_ndxes[0] = 1;
        map.extra_blend_tile_ndxes[0] = 2;
        let mut second = BlendTileInfo::new();
        second.blend_ndx = 2;
        second.horiz = 1;
        let mut third = BlendTileInfo::new();
        third.blend_ndx = 3;
        third.vert = 1;
        map.blended_tiles = vec![BlendTileInfo::new(), second, third];
        let mut visual = TerrainVisualImpl::new();
        visual.set_use_3way_terrain_blends(setting);
        visual.admit_static_lighting(crate::terrain::chunk::TerrainStaticLighting {
            lights: Vec::new(), ambient: [1.0; 3], scorch_diffuse: [1.0; 3],
        });
        visual.load_heightmap_from_data(map, None, None).unwrap();
        visual.create_terrain_pipeline(device).unwrap();
        visual.create_extra_blend_pipeline(device).unwrap();
        visual.device = Some(Arc::clone(device));
        visual.upload_extra_blend_overlay();

        let extent = wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: 1 };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain blend target"), size: extent, mip_level_count: 1,
            sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain blend depth"), size: extent, mip_level_count: 1,
            sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: TERRAIN_PIPELINES_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[],
        });
        let material_extent = wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 };
        let materials: Vec<_> = [[0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]]
            .into_iter().map(|rgba| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("authored terrain material"), size: material_extent,
                    mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(texture.as_image_copy(), &rgba, wgpu::TexelCopyBufferLayout {
                    offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1),
                }, material_extent);
                texture
            }).collect();
        let views: Vec<_> = materials.iter().map(|t| t.create_view(&Default::default())).collect();
        let sampler = device.create_sampler(&Default::default());
        let bind_materials = |overlay: bool| {
            let mut entries: Vec<_> = (0..MAX_TEXTURES_PER_CHUNK).map(|slot| wgpu::BindGroupEntry {
                binding: slot as u32,
                resource: wgpu::BindingResource::TextureView(&views[if overlay { 2 } else { slot.min(1) }]),
            }).collect();
            entries.push(wgpu::BindGroupEntry { binding: MAX_TEXTURES_PER_CHUNK as u32,
                resource: wgpu::BindingResource::Sampler(&sampler) });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terrain blend materials"),
                layout: visual.terrain_texture_bind_group_layout.as_ref().unwrap(), entries: &entries,
            })
        };
        let base_materials = bind_materials(false);
        let overlay_materials = bind_materials(true);
        visual.chunk_texture_bindings.insert(0, ChunkTextureBinding {
            bind_group: overlay_materials, slot_map: HashMap::new(),
            texture_ids: [0; MAX_TEXTURES_PER_CHUNK], diffuse_views: Vec::new(),
        });
        let transform = Mat4::from_cols_array_2d(&[
            [2.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0], [-1.0, -1.0, 0.5, 1.0],
        ]);
        let uniforms = TerrainUniforms {
            view_proj: transform.to_cols_array_2d(), view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            projection_matrix: Mat4::IDENTITY.to_cols_array_2d(), camera_position: [0.0; 4],
            time: 0.0, _time_padding: [0.0; 3], fog_color: [0.0; 4], fog_range: [0.0; 4],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain blend camera"), contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        visual.terrain_camera_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain blend camera binding"),
            layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        }));
        let vertex = |x: f32, z: f32| TerrainVertex::from_components(
            Vec3::new(x, 0.0, z), Vec3::Y, (x, z), [0, 1, 0, 0],
            [1.0 - x, x, 0.0, 0.0], [1.0; 4],
        );
        let vertices = [vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(1.0, 1.0),
            vertex(0.0, 0.0), vertex(1.0, 1.0), vertex(0.0, 1.0)];
        let base = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("authored two-material base"), contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrain blend pixels"), size: 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("real terrain blend pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view, resolve_target: None, depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
            });
            pass.set_pipeline(visual.terrain_pipeline.as_ref().unwrap());
            pass.set_bind_group(0, visual.terrain_camera_bind_group.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &base_materials, &[]);
            pass.set_vertex_buffer(0, base.slice(..));
            pass.draw(0..6, 0..1);
            visual.record_extra_blend_pass(&mut pass);
        }
        encoder.copy_texture_to_buffer(target.as_image_copy(), wgpu::TexelCopyBufferInfo {
            buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(4) },
        }, extent);
        queue.submit([encoder.finish()]);
        let (done, completed) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |result| { done.send(result).unwrap(); });
        device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(20)) }).unwrap();
        completed.recv_timeout(std::time::Duration::from_secs(1)).unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let pixel = mapped[256 + 2 * 4..256 + 2 * 4 + 4].try_into().unwrap();
        drop(mapped);
        readback.unmap();
        pixel
    }
    let off = render(&device, &queue, 0);
    let on = render(&device, &queue, 1);
    let debug = render(&device, &queue, 2);
    // BGRA: the base is a blue/green mix; only the overlay contributes red.
    assert_eq!(off[2], 0, "disabled world must retain the two-material base: {off:?}");
    assert!(off[0] > 20 && off[1] > 20, "base materials must both render: {off:?}");
    assert!(on[2] > 80, "enabled world must render the red third material: {on:?}");
    assert_eq!(on, debug, "nonzero debug mode must keep the extra pass");
}

/// Read pixels from the same admitted heightmap, class art, chunk upload and
/// draw path used by terrain rendering. The class list is deliberately an
/// input so larger material sets can reuse this fixture.
#[cfg(not(target_arch = "wasm32"))]
fn live_terrain_visual(
    device: &Arc<wgpu::Device>,
    queue: &Arc<wgpu::Queue>,
    classes: &[(TerrainSourceTileClass, [u8; 3])],
    map: HeightMap,
    ambient: f32,
) -> TerrainVisualImpl {
    let mut visual = TerrainVisualImpl::new();
    visual.admit_static_lighting(crate::terrain::chunk::TerrainStaticLighting {
        lights: Vec::new(), ambient: [ambient; 3], scorch_diffuse: [1.0; 3],
    });
    // Supply real encoded source images through the normal mounted-content
    // admission API, rather than manufacturing texture views or slot maps.
    let metadata: Vec<_> = classes.iter().map(|(class, _)| class.clone()).collect();
    visual.load_source_tiles_from_texture_classes_with_reader(&metadata, |path| {
        let (_, rgb) = classes.iter().find(|(class, _)| {
            path.to_ascii_lowercase().contains(&class.name.to_ascii_lowercase())
        })?;
        // Uncompressed 64x64, 24-bit TGA. Solid art isolates class selection
        // and blend weights from texture-coordinate filtering.
        let mut tga = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 64, 0, 24, 0x20];
        for _ in 0..64 * 64 {
            tga.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]);
        }
        Some(tga)
    }).unwrap();
    visual.load_heightmap_from_data(map, None, None).unwrap();
    visual.create_terrain_pipeline(device).unwrap();
    visual.create_extra_blend_pipeline(device).unwrap();
    visual.device = Some(Arc::clone(device));
    visual.queue = Some(Arc::clone(queue));
    visual
}

#[cfg(not(target_arch = "wasm32"))]
fn render_live_terrain_visual(
    device: &Arc<wgpu::Device>,
    queue: &Arc<wgpu::Queue>,
    visual: &mut TerrainVisualImpl,
) -> Vec<u8> {
    use wgpu::util::DeviceExt;
    // Match the runtime's geometry update followed by its GPU upload.
    visual.chunk_manager.update_with_dynamic_lights(&visual.dynamic_lights).unwrap();
    visual.update_chunk_meshes().unwrap();
    // View x=[30,34], z=[1,2]; x=32 is the real 32-cell chunk seam.
    let transform = Mat4::from_cols_array_2d(&[
        [0.5, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0],
        [0.0, 2.0, 0.0, 0.0], [-16.0, -3.0, 0.5, 1.0],
    ]);
    let uniforms = TerrainUniforms {
        view_proj: transform.to_cols_array_2d(), view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        projection_matrix: Mat4::IDENTITY.to_cols_array_2d(), camera_position: [0.0; 4],
        time: 0.0, _time_padding: [0.0; 3], fog_color: [0.0; 4], fog_range: [0.0; 4],
    };
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("live terrain test camera"), contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    visual.terrain_camera_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("live terrain test camera binding"),
        layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
    }));
    let extent = wgpu::Extent3d { width: 32, height: 8, depth_or_array_layers: 1 };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("live terrain test target"), size: extent, mip_level_count: 1,
        sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("live terrain test depth"), size: extent, mip_level_count: 1,
        sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: TERRAIN_PIPELINES_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("live terrain test pixels"), size: 256 * 8,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let target_view = target.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("live terrain test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view, resolve_target: None, depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
        });
        visual.record_chunk_draws(&mut pass);
    }
    encoder.copy_texture_to_buffer(target.as_image_copy(), wgpu::TexelCopyBufferInfo {
        buffer: &readback, layout: wgpu::TexelCopyBufferLayout {
            offset: 0, bytes_per_row: Some(256), rows_per_image: Some(8),
        },
    }, extent);
    queue.submit([encoder.finish()]);
    let (done, completed) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |result| { done.send(result).unwrap(); });
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(20)) }).unwrap();
    completed.recv_timeout(std::time::Duration::from_secs(1)).unwrap().unwrap();
    let pixels = readback.slice(..).get_mapped_range().unwrap().to_vec();
    readback.unmap();
    pixels
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn live_terrain_classes_blend_across_chunk_seam() {
    use crate::terrain::textures::BlendTileInfo;
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower, compatible_surface: None,
        force_fallback_adapter: false, apply_limit_buckets: false,
    })).expect("live terrain pixel test requires a GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("live terrain pixel test"), required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(), ..Default::default()
    })).expect("live terrain pixel test requires a GPU device");
    let classes: Vec<_> = [("LiveBlue", [0, 0, 255]), ("LiveGreen", [0, 255, 0]), ("LiveRed", [255, 0, 0])]
        .into_iter().enumerate().map(|(i, (name, color))| (TerrainSourceTileClass {
            first_tile: i as i32, num_tiles: 1, width: 1, name: name.to_string(),
        }, color)).collect();
    let mut map = HeightMap::new(35, 4, 0.0, 1.0);
    // The two seam-adjacent cells have the same authored blue/red blend;
    // the next cell selects green without blending.
    let mut blend = BlendTileInfo::new();
    blend.blend_ndx = 8;
    blend.horiz = 1;
    map.blended_tiles = vec![BlendTileInfo::new(), blend];
    for x in [31, 32] {
        map.blend_tile_ndxes[35 + x] = 1;
    }
    map.tile_ndxes[35 + 33] = 4;
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let mut visual = live_terrain_visual(&device, &queue, &classes, map, 1.0);
    let pixels = render_live_terrain_visual(&device, &queue, &mut visual);
    let pixel = |x: usize, y: usize| &pixels[y * 256 + x * 4..y * 256 + x * 4 + 4];
    let blue = pixel(4, 4);
    let left = pixel(12, 4);
    let right = pixel(20, 4);
    let green = pixel(28, 4);
    assert!(blue[0] > 100 && blue[1] < 20 && blue[2] < 20, "blue class: {blue:?}");
    assert!(green[1] > 100 && green[0] < 20 && green[2] < 20, "green class: {green:?}");
    for mixed in [left, right] {
        assert!(mixed[0] > 35 && mixed[2] > 35 && mixed[1] < 20, "blue/red blend: {mixed:?}");
    }
    for channel in 0..3 {
        assert!(left[channel].abs_diff(right[channel]) <= 8, "chunk seam: {left:?} vs {right:?}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn receiving_terrain_owners_render_their_own_live_lights_and_restore_static_color() {
    use crate::fx_list::{DisplayLightPulse, FxClientState};
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower, compatible_surface: None,
        force_fallback_adapter: false, apply_limit_buckets: false,
    })).expect("terrain light pixel test requires a GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("terrain light pixel test"), required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(), ..Default::default()
    })).expect("terrain light pixel test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let classes = [(TerrainSourceTileClass {
        first_tile: 0, num_tiles: 1, width: 1, name: "LightWhite".to_string(),
    }, [255, 255, 255])];
    let make_visual = || live_terrain_visual(
        &device, &queue, &classes, HeightMap::new(35, 4, 0.0, 1.0), 0.1,
    );
    let mut a = make_visual();
    let mut b = make_visual();
    let sample = |pixels: Vec<u8>| -> [u8; 4] {
        pixels[4 * 256 + 16 * 4..4 * 256 + 16 * 4 + 4].try_into().unwrap()
    };
    let baseline = sample(render_live_terrain_visual(&device, &queue, &mut a));
    assert_eq!(baseline, sample(render_live_terrain_visual(&device, &queue, &mut b)));
    let pulse = |color| DisplayLightPulse {
        pos: [32.0, 1.5, 10.0], color, inner_radius: 50.0, outer_radius: 30.0,
        increase_frames: 2, decay_frames: 2,
    };
    let mut first = FxClientState::default();
    let mut second = FxClientState::default();
    // Both clients use their first light slot; pulse lights have no explicit ID.
    assert!(first.create_display_light_pulse(pulse([0.3, 0.0, 0.0])));
    assert!(second.create_display_light_pulse(pulse([0.0, 0.0, 0.3])));
    a.admit_dynamic_lights(first.scene.lights());
    b.admit_dynamic_lights(second.scene.lights());
    let red = sample(render_live_terrain_visual(&device, &queue, &mut a));
    let blue = sample(render_live_terrain_visual(&device, &queue, &mut b));
    assert!(red[2] > baseline[2] + 20 && red[0] == baseline[0], "{red:?} vs {baseline:?}");
    assert!(blue[0] > baseline[0] + 20 && blue[2] == baseline[2], "{blue:?} vs {baseline:?}");

    first.tick_scene_dynamic_lights(); // First increase frame changes the color to half.
    a.admit_dynamic_lights(first.scene.lights());
    let faded = sample(render_live_terrain_visual(&device, &queue, &mut a));
    assert!(faded[2] > baseline[2] && faded[2] < red[2], "{faded:?} vs {red:?}");
    assert_eq!(blue, sample(render_live_terrain_visual(&device, &queue, &mut b)));
    second.clear_scene_dynamic_lights();
    b.admit_dynamic_lights(second.scene.lights());
    assert_eq!(baseline, sample(render_live_terrain_visual(&device, &queue, &mut b)));
    assert_eq!(faded, sample(render_live_terrain_visual(&device, &queue, &mut a)));

    first.tick_scene_dynamic_lights();
    first.tick_scene_dynamic_lights();
    first.tick_scene_dynamic_lights(); // Decay expires the first client's light.
    assert!(first.scene.lights().is_empty());
    a.admit_dynamic_lights(first.scene.lights());
    assert_eq!(baseline, sample(render_live_terrain_visual(&device, &queue, &mut a)));
    assert_eq!(baseline, sample(render_live_terrain_visual(&device, &queue, &mut b)));
}
