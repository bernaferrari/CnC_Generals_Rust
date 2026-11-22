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
    // View x=[30,34], z=[1,2]; x=32 is the real 32-cell chunk seam.
    let transform = Mat4::from_cols_array_2d(&[
        [0.5, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0],
        [0.0, 2.0, 0.0, 0.0], [-16.0, -3.0, 0.5, 1.0],
    ]);
    render_live_terrain_visual_with_transform(device, queue, visual, transform)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_live_terrain_visual_with_transform(
    device: &Arc<wgpu::Device>,
    queue: &Arc<wgpu::Queue>,
    visual: &mut TerrainVisualImpl,
    transform: Mat4,
) -> Vec<u8> {
    use wgpu::util::DeviceExt;
    // Match the runtime's geometry update followed by its GPU upload.
    visual.chunk_manager.update_with_dynamic_lights(&visual.dynamic_lights).unwrap();
    visual.update_chunk_meshes().unwrap();
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
        visual.record_projected_unit_shadows(&mut pass);
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
fn live_terrain_more_than_sixteen_classes_preserve_both_blend_layers() {
    use crate::terrain::textures::BlendTileInfo;
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower, compatible_surface: None,
        force_fallback_adapter: false, apply_limit_buckets: false,
    })).expect("terrain material pixel test requires a GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("terrain material pixel test"), required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(), ..Default::default()
    })).expect("terrain material pixel test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let classes: Vec<_> = (0..18).map(|i| {
        let color = match i {
            0 => [255, 0, 0],
            1 => [0, 255, 0],
            17 => [0, 0, 255],
            _ => [20 + (i as u8 - 2) * 14, 60, 90],
        };
        (TerrainSourceTileClass {
            first_tile: i, num_tiles: 1, width: 1, name: format!("BatchClass{i:02}"),
        }, color)
    }).collect();
    let mut map = HeightMap::new(22, 4, 0.0, 1.0);
    for i in 0..18 {
        map.tile_ndxes[22 + i] = (i * 4) as i16;
    }
    let mut first = BlendTileInfo::new();
    first.blend_ndx = 17 * 4;
    first.horiz = 1;
    let mut third = BlendTileInfo::new();
    third.blend_ndx = 4;
    third.horiz = 1;
    map.blended_tiles = vec![BlendTileInfo::new(), first, third];
    map.blend_tile_ndxes[22 + 18] = 1;
    map.blend_tile_ndxes[22 + 19] = 1;
    map.extra_blend_tile_ndxes[22 + 19] = 2;
    let mut visual = live_terrain_visual(&device, &queue, &classes, map, 1.0);
    let transform = Mat4::from_cols_array_2d(&[
        [0.1, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0],
        [0.0, 2.0, 0.0, 0.0], [-1.0, -3.0, 0.5, 1.0],
    ]);
    visual.set_use_3way_terrain_blends(0);
    let off = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    visual.set_use_3way_terrain_blends(1);
    let on = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    let pixel = |pixels: &[u8], cell: usize| {
        let x = ((cell as f32 + 0.5) * 32.0 / 20.0) as usize;
        let start = 4 * 256 + x * 4;
        [pixels[start + 2], pixels[start + 1], pixels[start]]
    };
    for (i, (_, expected)) in classes.iter().enumerate() {
        let actual = pixel(&on, i);
        for channel in 0..3 {
            assert!(actual[channel].abs_diff(expected[channel]) <= 8,
                "class {i}: expected {expected:?}, got {actual:?}");
        }
    }
    let mixed = pixel(&off, 18);
    assert!(mixed[0] > 30 && mixed[2] > 30 && mixed[1] < 20,
        "first blend must mix red and the seventeenth class: {mixed:?}");
    let before = pixel(&off, 19);
    let after = pixel(&on, 19);
    assert!(after[1] > before[1] + 30,
        "third blend must add green: disabled {before:?}, enabled {after:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn three_way_blend_keeps_flat_terrain_directional_lighting() {
    use crate::terrain::chunk::TerrainStaticLighting;
    use crate::terrain::textures::BlendTileInfo;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("three-way blend test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("three-way blend test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let classes = [
        ("LightingSnowA", [140, 160, 200]),
        ("LightingSnowB", [170, 170, 200]),
        ("LightingSnowC", [220, 180, 200]),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, color))| {
        (TerrainSourceTileClass {
            first_tile: i as i32,
            num_tiles: 1,
            width: 1,
            name: name.into(),
        }, color)
    })
    .collect::<Vec<_>>();
    let mut map = HeightMap::new(35, 4, 0.0, 1.0);
    // Authored left-long diagonal as used by the Lone Eagle extra blend at
    // source grid (212, 126). The camera fixture draws the equivalent cell
    // at (32, 1), on the real chunk seam.
    let mut primary = BlendTileInfo::new();
    primary.blend_ndx = 4;
    primary.left_diagonal = 1;
    primary.inverted = 1;
    primary.long_diagonal = 1;
    let mut extra = primary.clone();
    extra.blend_ndx = 8;
    map.blended_tiles = vec![BlendTileInfo::new(), primary, extra];
    map.blend_tile_ndxes[35 + 32] = 1;
    map.extra_blend_tile_ndxes[35 + 32] = 2;
    let lighting = TerrainStaticLighting {
        lights: vec![(-Vec3::Y, [0.5, 0.45, 0.4])],
        ambient: [0.2, 0.25, 0.3],
        scorch_diffuse: [1.0; 3],
    };
    let mut visual = live_terrain_visual(&device, &queue, &classes, map, 1.0);
    visual.admit_static_lighting(lighting);
    visual.set_use_3way_terrain_blends(0);
    let off = render_live_terrain_visual(&device, &queue, &mut visual);
    visual.set_use_3way_terrain_blends(1);
    let on = render_live_terrain_visual(&device, &queue, &mut visual);

    let pixel = 4 * 256 + 19 * 4;
    assert_eq!(off[pixel], 140, "the flat base receives the overhead light");
    assert_eq!(
        on[pixel], off[pixel],
        "every authored texture uses blue=200, so the blend must preserve lit blue"
    );
    assert!(on[pixel + 2] > off[pixel + 2] + 25, "the extra layer must draw");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn lone_eagle_road_miter_fills_exterior_gap_pixels() {
    use game_engine::common::ini::ini_road::TerrainRoadCollection;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("road miter test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("road miter test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let texture_path = directory.path().join("RoadMiterWitness.tga");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([120u8, 100, 80, 255]))
        .save_with_format(&texture_path, image::ImageFormat::Tga)
        .unwrap();
    let mut definitions = TerrainRoadCollection::new();
    let road = definitions.new_road(AsciiString::from("RoadMiterWitness"));
    road.texture = AsciiString::from(texture_path.to_str().unwrap());
    let road_type_id = road.id;

    let mut roads = super::tests::lone_eagle_road_corner_segments();
    let origin = roads[0].start;
    for road in &mut roads {
        for endpoint in [&mut road.start, &mut road.end] {
            endpoint[0] = 40.0 + (endpoint[0] - origin[0]) * 0.1;
            endpoint[2] = 40.0 + (endpoint[2] - origin[2]) * 0.1;
        }
        road.width *= 0.1;
        road.road_type_id = road_type_id;
        road.template_name = "RoadMiterWitness".into();
    }
    let terrain = [(TerrainSourceTileClass {
        first_tile: 0, num_tiles: 1, width: 1, name: "RoadWhiteGround".into(),
    }, [255; 3])];
    let mut visual = live_terrain_visual(
        &device, &queue, &terrain, HeightMap::new(64, 64, 0.0, 1.0), 1.0,
    );
    visual.create_road_pipeline(&device).unwrap();
    visual.admit_road_presentation(definitions.presentation());
    visual.set_runtime_map_road_segments(&roads, &[]).unwrap();
    visual.update_road_meshes().unwrap();
    let transform = Mat4::from_cols_array_2d(&[
        [5.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0],
        [0.0, 5.0, 0.0, 0.0], [-186.5, -198.25, 0.5, 1.0],
    ]);
    let with_miter = render_live_terrain_visual_with_transform(
        &device, &queue, &mut visual, transform,
    );

    let mut ids = Vec::new();
    visual.road_system.for_each_visible_overlay_source(|_, road| ids.push(road.id));
    for id in ids {
        visual.road_system.get_segment_mut(id).unwrap().properties.w3d_miters = [None; 2];
    }
    visual.overlay_gpu_meshes_dirty = true;
    visual.update_road_meshes().unwrap();
    let without_miter = render_live_terrain_visual_with_transform(
        &device, &queue, &mut visual, transform,
    );

    // This sample is outside both butt-ended strips and inside the
    // shared footprint at the miter's exterior side.
    let pixel = 4 * 256 + 11 * 4;
    assert_eq!(&without_miter[pixel..pixel + 4], &[255, 255, 255, 255]);
    assert_eq!(&with_miter[pixel..pixel + 4], &[80, 100, 120, 255]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn lone_eagle_tight_road_curve_draws_the_authored_atlas_row() {
    use game_engine::common::ini::ini_road::TerrainRoadCollection;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("road curve test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("road curve test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let texture_path = directory.path().join("RoadCurveWitness.tga");
    let normal = [190, 45, 50, 255];
    let tight = [40, 185, 55, 255];
    image::RgbaImage::from_fn(128, 512, |_, y| {
        image::Rgba(match y {
            190..=315 => normal, // C++ normal curve row, 255/512
            355..=470 => tight,  // C++ tight curve row, 425/512
            _ => [110, 110, 110, 255],
        })
    })
    .save_with_format(&texture_path, image::ImageFormat::Tga)
    .unwrap();
    let mut definitions = TerrainRoadCollection::new();
    let road = definitions.new_road(AsciiString::from("RoadCurveWitness"));
    road.texture = AsciiString::from(texture_path.to_str().unwrap());
    let road_type_id = road.id;

    let mut roads = super::tests::lone_eagle_road_curve_segments();
    let origin = roads[0].start;
    for road in &mut roads {
        for endpoint in [&mut road.start, &mut road.end] {
            endpoint[0] = 40.0 + (endpoint[0] - origin[0]) * 0.1;
            endpoint[2] = 40.0 + (endpoint[2] - origin[2]) * 0.1;
        }
        road.width *= 0.1;
        road.road_type_id = road_type_id;
        road.template_name = "RoadCurveWitness".into();
    }
    let terrain = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "CurveWhiteGround".into(),
        },
        [255; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &terrain,
        HeightMap::new(64, 64, 0.0, 1.0),
        1.0,
    );
    visual.create_road_pipeline(&device).unwrap();
    visual.admit_road_presentation(definitions.presentation());
    visual.set_runtime_map_road_segments(&roads, &[]).unwrap();
    visual
        .update_native(
            TerrainVisualMotion {
                frame: 0,
                paused: false,
                gravity: 0.0,
                breeze: Default::default(),
            },
            None,
        )
        .unwrap();
    let transform = Mat4::from_cols_array_2d(&[
        [5.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [0.0, 5.0, 0.0, 0.0],
        [-200.0, -207.25, 0.5, 1.0],
    ]);
    let pixels = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    // Retail world (3179.58, 1940.74), sampled inside the generated curve
    // and beyond both adjoining straight strips.
    let pixel = 2 * 256 + 19 * 4;
    assert_eq!(
        &pixels[pixel..pixel + 4],
        &[tight[2], tight[1], tight[0], tight[3]]
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn tight_road_curve_preserves_max_cell_height_over_a_slope() {
    use game_engine::common::ini::ini_road::TerrainRoadCollection;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("sloped curve test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("sloped curve test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let texture_path = directory.path().join("SlopedCurveWitness.tga");
    let color = [35, 160, 210, 255];
    image::RgbaImage::from_pixel(128, 512, image::Rgba(color))
        .save_with_format(&texture_path, image::ImageFormat::Tga)
        .unwrap();
    let mut definitions = TerrainRoadCollection::new();
    let road = definitions.new_road(AsciiString::from("SlopedCurveWitness"));
    road.texture = AsciiString::from(texture_path.to_str().unwrap());
    let road_type_id = road.id;

    let mut roads = super::tests::lone_eagle_road_curve_segments();
    let offset = Vec3::new(300.0, 0.0, 300.0) - Vec3::from_array(roads[0].start);
    for road in &mut roads {
        road.start = (Vec3::from_array(road.start) + offset).to_array();
        road.end = (Vec3::from_array(road.end) + offset).to_array();
        road.road_type_id = road_type_id;
        road.template_name = "SlopedCurveWitness".into();
    }
    let terrain = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "CurveWhiteGround".into(),
        },
        [255; 3],
    )];
    let mut height_map = HeightMap::new(64, 64, 9.0, 10.0);
    height_map.set_raw_height(32, 32, u8::MAX);
    assert!((height_map.get_height_at(323.0, 318.0) - 4.5).abs() < 0.001);
    let mut visual = live_terrain_visual(&device, &queue, &terrain, height_map, 1.0);
    visual.create_road_pipeline(&device).unwrap();
    visual.admit_road_presentation(definitions.presentation());
    visual.set_runtime_map_road_segments(&roads, &[]).unwrap();
    let transform = Mat4::from_cols_array_2d(&[
        [0.5, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 0.5, 0.0, 0.0],
        [-161.46875, -158.875, 0.5, 1.0],
    ]);
    // World (323, 318) lies on the curve exclusively. In the source the two
    // surrounding columns both reach the 9.0 peak plus the road float bias.
    let pixel = 3 * 256 + 16 * 4;
    for frame in 0..2 {
        if frame != 0 {
            visual.static_lighting_changed();
        }
        visual
            .update_native(
                TerrainVisualMotion {
                    frame,
                    paused: false,
                    gravity: 0.0,
                    breeze: Default::default(),
                },
                None,
            )
            .unwrap();
        let pixels =
            render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
        assert_eq!(
            &pixels[pixel..pixel + 4],
            &[color[2], color[1], color[0], color[3]],
            "road bake at frame {frame}"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn render_named_road_pixel(texel: [u8; 4], ambient: f32) -> ([u8; 4], [u8; 4]) {
    use game_engine::common::ini::ini_road::TerrainRoadCollection;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("authored road test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("authored road test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let texture_path = directory.path().join("SnowRoadPalette.tga");
    image::RgbaImage::from_pixel(64, 64, image::Rgba(texel))
        .save_with_format(&texture_path, image::ImageFormat::Tga)
        .unwrap();

    let mut definitions = TerrainRoadCollection::new();
    let road_definition = definitions.new_road(AsciiString::from("SnowRoadPalette"));
    road_definition.texture = AsciiString::from(texture_path.to_str().unwrap());
    let roads = [RuntimeRoadVisualSegment {
        start: [26.0, 0.0, 2.0],
        end: [40.0, 0.0, 2.0],
        width: 2.0,
        template_name: "SnowRoadPalette".to_owned(),
        width_in_texture: 1.0,
        road_type_id: road_definition.id,
        start_is_angled: false,
        start_is_join: false,
        end_is_angled: false,
        end_is_join: false,
        curve_radius: 0.0,
    }];
    let terrain = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "SnowRoadReceiver".to_owned(),
        },
        [255; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &terrain,
        HeightMap::new(64, 64, 0.0, 1.0),
        ambient,
    );
    visual.create_road_pipeline(&device).unwrap();
    visual.admit_road_presentation(definitions.presentation());
    visual.set_runtime_map_road_segments(&roads, &[]).unwrap();
    visual.update_road_meshes().unwrap();
    assert!(
        visual
            .road_named_bind_groups
            .contains_key(&texture_path.to_str().unwrap().to_ascii_lowercase()),
        "the receiving road definition must bind its own authored palette"
    );

    // X=[30,34], Z=[0,4]. Column 16 crosses the road at row 4, with row 0
    // outside its coverage.
    let transform = Mat4::from_cols_array_2d(&[
        [0.5, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 0.5, 0.0, 0.0],
        [-16.0, -1.0, 0.5, 1.0],
    ]);
    let pixels = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    let pixel = |row: usize| pixels[row * 256 + 64..row * 256 + 68].try_into().unwrap();
    (pixel(4), pixel(0))
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_road_pixel(road: [u8; 4], expected: [u8; 3]) {
    assert_eq!(road[3], 255);
    // The native UNORM blend may round to an adjacent channel value when
    // the sample is near the center of the authored coverage ramp.
    assert!(
        road[..3]
            .iter()
            .zip(expected)
            .all(|(&actual, expected)| actual.abs_diff(expected) <= 1),
        "authored road pixel {road:?} differs from {expected:?}",
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn authored_road_keeps_dark_palette_rgb_and_coverage_alpha() {
    // A visible texel from retail TRGravelRoadS.dds. W3DShaderManager.cpp
    // :2377-2391 alpha-blends its unmodified RGB at full diffuse.
    let (road, terrain) = render_named_road_pixel([16, 8, 8, 253], 1.0);
    assert_eq!(terrain, [255; 4]);
    assert_road_pixel(road, [10, 10, 18]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn authored_road_uses_diffuse_below_the_old_lighting_floor() {
    let (road, terrain) = render_named_road_pixel([180, 140, 100, 255], 0.2);
    assert_road_pixel(road, [20, 28, 36]);
    // The adjacent terrain is still shaded by the separate terrain pass.
    assert_eq!(terrain, [89, 89, 89, 255]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn consecutive_road_textures_batch_with_identical_lit_and_shrouded_pixels() {
    use crate::fx_list::{DisplayLightPulse, DisplayLightScene};
    use game_engine::common::ini::ini_road::TerrainRoadCollection;

    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("road batch test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("road batch test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let primary = directory.path().join("Primary.tga");
    let alias = directory.path().join("PrimaryAlias.tga");
    let other = directory.path().join("Other.tga");
    image::RgbaImage::from_fn(128, 512, |x, _| {
        image::Rgba(if x < 64 {
            [200, 30, 60, 160]
        } else {
            [20, 180, 90, 96]
        })
    })
    .save_with_format(&primary, image::ImageFormat::Tga)
    .unwrap();
    std::fs::copy(&primary, &alias).unwrap();
    image::RgbaImage::from_pixel(128, 512, image::Rgba([40, 60, 210, 160]))
        .save_with_format(&other, image::ImageFormat::Tga)
        .unwrap();

    let segments = [
        ("PrimaryFirst", [26.0, 0.0, 1.25], [40.0, 0.0, 1.25]),
        ("PrimarySecond", [31.7, 0.0, 0.2], [31.7, 0.0, 3.8]),
        ("Crossing", [29.0, 0.0, 0.75], [35.0, 0.0, 2.5]),
        ("PrimaryLast", [26.5, 0.0, 1.57], [39.5, 0.0, 1.57]),
        ("Invisible", [28.0, 0.0, 1.8], [37.0, 0.0, 1.8]),
    ];
    let roads = segments.map(|(name, start, end)| RuntimeRoadVisualSegment {
        start,
        end,
        width: 1.4,
        template_name: name.into(),
        width_in_texture: 1.0,
        road_type_id: 1,
        start_is_angled: false,
        start_is_join: false,
        end_is_angled: false,
        end_is_join: false,
        curve_radius: 0.0,
    });
    let terrain = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "RoadGround".into(),
        },
        [255; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &terrain,
        HeightMap::new(64, 64, 0.0, 1.0),
        0.5,
    );
    visual.create_road_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.set_runtime_map_road_segments(&roads, &[]).unwrap();
    let mut invisible = None;
    visual
        .road_system
        .for_each_visible_overlay_source(|road, _| {
            if road.name == "Invisible" {
                invisible = Some(road.id);
            }
        });
    visual
        .road_system
        .get_road_mut(invisible.unwrap())
        .unwrap()
        .visible = false;

    let mut scene = DisplayLightScene::default();
    assert!(scene.create_pulse(DisplayLightPulse {
        pos: [32.0, 1.5, 4.0],
        color: [0.35, 0.0, 0.0],
        inner_radius: 20.0,
        outer_radius: 30.0,
        increase_frames: 0,
        decay_frames: 0,
    }));
    let transform = Mat4::from_cols_array_2d(&[
        [0.5, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 2.0, 0.0, 0.0],
        [-16.0, -3.0, 0.5, 1.0],
    ]);
    let mut render = |split: bool| {
        let mut definitions = TerrainRoadCollection::new();
        let second = if split { &alias } else { &primary };
        for ((name, _, _), path) in segments
            .iter()
            .zip([&primary, second, &other, &primary, &other])
        {
            definitions.new_road(AsciiString::from(*name)).texture =
                AsciiString::from(path.to_str().unwrap());
        }
        visual.admit_road_presentation(definitions.presentation());
        visual.clear_shroud_overlay();
        let capture = |visual: &mut TerrainVisualImpl, frame| {
            visual
                .update_native(
                    TerrainVisualMotion {
                        frame,
                        paused: false,
                        gravity: 0.0,
                        breeze: Default::default(),
                    },
                    None,
                )
                .unwrap();
            render_live_terrain_visual_with_transform(&device, &queue, visual, transform)
        };
        visual.admit_dynamic_lights(&[]);
        let unlit = capture(&mut visual, 0);
        visual.admit_dynamic_lights(scene.lights());
        let lit = capture(&mut visual, 1);
        visual.set_shroud_overlay_r8(64, 64, 1.0, [0.0; 2], &[132; 64 * 64]);
        let shrouded = capture(&mut visual, 2);
        let materials = visual
            .road_meshes
            .iter()
            .map(|mesh| mesh.texture_name.clone())
            .collect::<Vec<_>>();
        assert!(visual.road_texture_bind_group.is_some());
        assert!(materials.iter().all(|name| visual
            .road_named_bind_groups
            .contains_key(&name.to_ascii_lowercase())));
        let indices: u32 = visual.road_meshes.iter().map(|mesh| mesh.index_count).sum();
        ([unlit, lit, shrouded], materials, indices)
    };
    let (split_pixels, split_materials, split_indices) = render(true);
    let (batch_pixels, batch_materials, batch_indices) = render(false);
    let pixel = 3 * 256 + 14 * 4;
    let crossing = pixel..pixel + 4;
    assert_ne!(
        split_pixels[0][crossing.clone()],
        split_pixels[1][crossing.clone()],
        "active light must change the crossing"
    );
    assert_ne!(
        split_pixels[1][crossing.clone()],
        split_pixels[2][crossing],
        "projected shroud must change the crossing"
    );
    assert_eq!(batch_indices, split_indices);
    for (expected, actual) in split_pixels.iter().zip(batch_pixels.iter()) {
        assert!(
            expected == actual,
            "road batches must leave the captured pixels unchanged"
        );
    }
    assert_eq!(
        split_materials.len(),
        4,
        "one draw per distinct consecutive material"
    );
    assert_eq!(
        batch_materials,
        [&primary, &other, &primary].map(|path| path.to_str().unwrap().to_owned())
    );
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

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn projected_unit_shadow_uses_source_v_axis_and_offsets_in_native_pixels() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("projected shadow test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("projected shadow test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let classes = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "ShadowReceiver".to_owned(),
        },
        [200; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &classes,
        HeightMap::new(64, 64, 255.0, 1.0),
        1.0,
    );
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_fn(64, 64, |x, y| {
        let gray = match (x, y) {
            (4..=59, 8..=23) => 64,
            (4..=59, 40..=55) => 160,
            _ => 255,
        };
        image::Rgba([gray, gray, gray, 255])
    })
    .write_to(&mut encoded, image::ImageFormat::Tga)
    .unwrap();
    let caster = ProjectedUnitShadow {
        object_id: 7,
        transform: Mat4::from_translation(Vec3::new(14.0, 0.0, 18.0)),
        size: Vec2::new(16.0, 12.0),
        offset: Vec2::new(4.0, 2.0),
        layer_height: None,
        texture_name: "AxisWitness".to_owned(),
    };
    assert_eq!(
        visual.admit_projected_unit_shadows(vec![caster], |path| {
            (path == "Art/Textures/axiswitness.tga").then(|| encoded.get_ref().clone())
        }),
        HashSet::from([7])
    );
    visual
        .update_native(
            TerrainVisualMotion {
                frame: 0,
                paused: false,
                gravity: 0.0,
                breeze: Default::default(),
            },
            None,
        )
        .unwrap();
    // 32x8 top-down target covers X=[0,32] and Z=[8,24], with Z=23
    // at the first pixel row. The original midpoint is (18,16).
    let transform = Mat4::from_cols_array_2d(&[
        [0.0625, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 0.125, 0.0, 0.0],
        [-1.0, -2.0, 0.5, 1.0],
    ]);
    let pixels = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    let gray = |column: usize, row: usize| pixels[row * 256 + column * 4];
    assert!((49..=51).contains(&gray(13, 5)), "Z=13 samples V=0.25");
    assert!((124..=126).contains(&gray(13, 2)), "Z=19 samples V=0.75");
    assert!((49..=51).contains(&gray(22, 5)), "the X offset shifts the footprint right");
    assert_eq!(gray(7, 5), 200, "terrain outside the shifted footprint");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn projected_unit_shadow_multiplies_source_color_on_native_terrain() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("projected shadow test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("projected shadow test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let classes = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "ShadowReceiver".to_owned(),
        },
        [200; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &classes,
        HeightMap::new(64, 64, 255.0, 1.0),
        1.0,
    );
    let source = std::env::var_os("GENERALS_TEST_WORKER_SHADOW_DDS")
        .map(|path| std::fs::read(path).expect("read retail ShadowI DDS witness"));
    let mut encoded = std::io::Cursor::new(Vec::new());
    let synthetic = image::RgbaImage::from_fn(64, 64, |x, _| {
        image::Rgba(match x {
            3..=12 => [242, 236, 230, 255],
            24..=40 => [136, 128, 112, 0],
            _ => [255; 4],
        })
    });
    synthetic
        .write_to(&mut encoded, image::ImageFormat::Tga)
        .unwrap();
    let caster = ProjectedUnitShadow {
        object_id: 42,
        transform: Mat4::from_translation(Vec3::new(30.5, 0.0, 1.5)),
        size: Vec2::splat(14.0),
        offset: Vec2::ZERO,
        layer_height: None,
        texture_name: "ShadowI".into(),
    };
    let casters = visual.admit_projected_unit_shadows(
        vec![caster.clone()],
        |path| match path {
            "Art/Textures/shadowi.dds" => source.clone(),
            "Art/Textures/shadowi.tga" => Some(encoded.get_ref().clone()),
            _ => None,
        },
    );
    assert_eq!(casters, HashSet::from([42]));
    let motion = TerrainVisualMotion {
        frame: 0,
        paused: false,
        gravity: 0.0,
        breeze: Default::default(),
    };
    visual.update_native(motion, None).unwrap();
    let transform = Mat4::from_cols_array_2d(&[
        [0.125, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 2.0, 0.0, 0.0],
        [-3.8125, -3.0, 0.5, 1.0],
    ]);
    let pixels = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    let pixel = |x: usize| -> [u8; 4] {
        pixels[4 * 256 + x * 4..4 * 256 + x * 4 + 4]
            .try_into()
            .unwrap()
    };
    let (edge, center) = (pixel(4), pixel(16));
    assert_eq!(pixel(0), [200, 200, 200, 255]);
    if source.is_some() {
        assert!(
            (98..=112).contains(&center[0]) && (180..=199).contains(&edge[0]),
            "ShadowI edge {edge:?}, center {center:?}"
        );
    } else {
        assert_eq!(
            center,
            [88, 100, 107, 255],
            "zero texture alpha preserves multiplicative RGB"
        );
        assert_eq!(edge, [180, 185, 190, 255]);
    }

    let repeated_bytes = (0..20)
        .map(|_| {
            assert_eq!(
                visual.admit_projected_unit_shadows(vec![caster.clone()], |_| {
                    panic!("resident shadow texture should not be reloaded")
                }),
                casters
            );
            visual.retain_projected_unit_shadow_casters(|_| Some(caster.transform));
            visual.update_projected_unit_shadow_meshes()
        })
        .sum::<usize>();
    assert_eq!(
        repeated_bytes, 0,
        "stationary authored decal uploaded {repeated_bytes} duplicate geometry bytes"
    );
    assert_eq!(
        render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform),
        pixels
    );

    // Main admits a position before the Draw visit supplies the final
    // oriented model matrix. A stationary actor can repeat that handoff.
    let rotated = caster.transform * Mat4::from_rotation_y(0.55);
    visual.retain_projected_unit_shadow_casters(|_| Some(rotated));
    let first_rotated_bytes = visual.update_projected_unit_shadow_meshes();
    assert!(first_rotated_bytes > 0);
    let oriented_pixels = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    assert_ne!(oriented_pixels, pixels);
    let repeated_rotated_bytes = (0..20)
        .map(|_| {
            assert_eq!(
                visual.admit_projected_unit_shadows(vec![caster.clone()], |_| {
                    panic!("resident shadow texture should not be reloaded")
                }),
                casters
            );
            assert!(visual.has_projected_unit_shadow_inputs());
            visual.retain_projected_unit_shadow_casters(|_| Some(rotated));
            visual.update_projected_unit_shadow_meshes()
        })
        .sum::<usize>();
    eprintln!(
        "oriented authored decal CRC32 {:08x}, initial {first_rotated_bytes} bytes, 20 repeated Draw poses {repeated_rotated_bytes} bytes",
        crc32fast::hash(&oriented_pixels)
    );
    assert_eq!(repeated_rotated_bytes, 0);
    assert_eq!(
        render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform),
        oriented_pixels
    );
    visual.retain_projected_unit_shadow_casters(|_| Some(caster.transform));
    assert!(visual.update_projected_unit_shadow_meshes() > 0);

    let moved = Mat4::from_translation(Vec3::new(34.5, 0.0, 1.5));
    visual.retain_projected_unit_shadow_casters(|id| (id == caster.object_id).then_some(moved));
    let move_bytes = visual.update_projected_unit_shadow_meshes();
    assert!(move_bytes > 0);
    assert_ne!(
        render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform),
        pixels
    );
    visual.retain_projected_unit_shadow_casters(|_| Some(caster.transform));
    assert_eq!(visual.update_projected_unit_shadow_meshes(), move_bytes);

    let restore_draw_origin = visual.draw_origin_x;
    visual.draw_origin_x = 32;
    let clipped_bytes = visual.update_projected_unit_shadow_meshes();
    assert!(clipped_bytes > 0 && clipped_bytes < move_bytes);
    assert_ne!(
        render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform),
        pixels
    );
    visual.draw_origin_x = restore_draw_origin;
    assert_eq!(visual.update_projected_unit_shadow_meshes(), move_bytes);

    let center_height = |visual: &TerrainVisualImpl| {
        visual.projected_shadow_gpu.as_ref().unwrap().vertices.iter()
            .find(|vertex| vertex.position[0] == 31.0 && vertex.position[2] == 1.0)
            .unwrap().position[1]
    };
    let flat = center_height(&visual);
    visual.apply_accepted_height(31, 1, 18);
    assert_eq!(visual.update_projected_unit_shadow_meshes(), move_bytes);
    assert!(center_height(&visual) > flat);
    visual.apply_accepted_height(31, 1, 0);
    assert_eq!(visual.update_projected_unit_shadow_meshes(), move_bytes);
    assert_eq!(center_height(&visual), flat);
    assert_eq!(
        render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform),
        pixels
    );

    visual.add_seismic_simulation(SeismicSimulationNode::new(Vec3::new(31.0, 0.0, 1.0), 3.0, 0.01));
    visual.update_seismic_simulations();
    assert_eq!(visual.update_projected_unit_shadow_meshes(), move_bytes);
    assert!(center_height(&visual) > flat);
    eprintln!(
        "authored decal CRC32 {:08x}, 20 idle updates {repeated_bytes} bytes, moved {move_bytes}, clipped {clipped_bytes}",
        crc32fast::hash(&pixels)
    );

    visual.retain_projected_unit_shadow_casters(|_| None);
    visual.update_native(motion, None).unwrap();
    let hidden = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, transform);
    let sample = 4 * 256 + 16 * 4;
    assert_eq!(&hidden[sample..sample + 4], &[200, 200, 200, 255]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn authored_tank_tracks_draw_in_used_order_and_fade_on_the_receiving_clock() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("tank track test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("tank track test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let classes = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "TrackReceiver".to_owned(),
        },
        [200; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &classes,
        HeightMap::new(64, 64, 0.0, 1.0),
        1.0,
    );
    visual.admit_static_lighting(crate::terrain::chunk::TerrainStaticLighting {
        lights: Vec::new(),
        ambient: [1.0; 3],
        scorch_diffuse: [0.0; 3],
    });
    visual.create_road_pipeline(&device).unwrap();
    visual.ensure_road_texture_bind_group(&device);
    visual.set_terrain_tracks_detail_with_config(TerrainTracksConfig {
        max_tank_track_fade_delay: 100,
        ..TerrainTracksConfig::default()
    });
    for name in ["RedTrack.tga", "BlueTrack.tga"] {
        let handle = visual.terrain_tracks_mut().bind_track(0.5, 10.0, name).unwrap();
        for x in [0.5, 11.5, 22.5, 33.5, 44.5, 55.5] {
            visual.add_track_edge(handle, x, 1.5, 1000);
        }
    }
    let tga = |color| {
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(4, 4, image::Rgba(color))
            .write_to(&mut encoded, image::ImageFormat::Tga)
            .unwrap();
        encoded.into_inner()
    };
    let red = tga([255, 0, 0, 128]);
    let blue = tga([0, 0, 255, 128]);
    let read = |path: &str| match path {
        "Art/Textures/redtrack.tga" => Some(red.clone()),
        "Art/Textures/bluetrack.tga" => Some(blue.clone()),
        _ => None,
    };
    let view = Mat4::from_cols_array_2d(&[
        [0.125, 0.0, 0.0, 0.0],
        [0.0, 0.0, -0.02, 0.0],
        [0.0, 2.0, 0.0, 0.0],
        [-5.0625, -3.0, 0.5, 1.0],
    ]);
    let sample = 4 * 256 + 16 * 4;
    let assert_pixel = |pixels: &[u8], expected: [u8; 4]| {
        let actual = &pixels[sample..sample + 4];
        assert!(
            actual.iter().zip(expected).all(|(&a, b)| a.abs_diff(b) <= 1),
            "expected BGRA {expected:?}, got {actual:?}",
        );
        assert_eq!(&pixels[64..68], &[200, 200, 200, 255]);
    };
    visual.prepare_tank_track_draws(read);
    let initial = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, view);
    // C++ bind inserts at the head of the used list. Blue blends first.
    assert_pixel(&initial, [113, 50, 178, 255]);
    visual.age_tank_tracks(1050);
    visual.prepare_tank_track_draws(|_| panic!("resident track art was opened again"));
    let faded = render_live_terrain_visual_with_transform(&device, &queue, &mut visual, view);
    assert_pixel(&faded, [160, 113, 176, 255]);
}
