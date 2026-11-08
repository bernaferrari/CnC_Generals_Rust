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
