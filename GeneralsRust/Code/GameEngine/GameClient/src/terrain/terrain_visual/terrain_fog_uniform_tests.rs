use super::*;

#[test]
fn terrain_fog_uniform_layout_and_receiving_inputs_match_wgsl() {
    assert_eq!(std::mem::size_of::<TerrainUniforms>(), 256);
    assert_eq!(std::mem::offset_of!(TerrainUniforms, time), 208);
    assert_eq!(std::mem::offset_of!(TerrainUniforms, fog_color), 224);
    assert_eq!(std::mem::offset_of!(TerrainUniforms, fog_range), 240);
    let mut receiving = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    assert_eq!(receiving.terrain_fog_uniforms().1, [0.0; 4]);
    receiving.set_lighting(None, None, None, Some([0.2, 0.3, 0.4]), Some((10.0, 100.0)));
    peer.set_lighting(None, None, None, Some([0.8, 0.7, 0.6]), None);
    assert_eq!(
        receiving.terrain_fog_uniforms(),
        ([0.2, 0.3, 0.4, 1.0], [10.0, 100.0, 0.0, 0.0])
    );
    assert_eq!(
        peer.terrain_fog_uniforms(),
        ([0.8, 0.7, 0.6, 1.0], [0.0; 4]),
        "fog color without an admitted range remains disabled in the actual GPU input"
    );
    drop(peer);
    assert_eq!(receiving.terrain_fog_uniforms().1, [10.0, 100.0, 0.0, 0.0]);
    let shader = include_str!("../../shaders/terrain.wgsl");
    assert!(shader.contains("fog_color: vec4<f32>"));
    assert!(shader.contains("fog_range: vec4<f32>"));
    assert!(shader.contains("if (fog_span > 0.0)"));
    assert!(shader.contains("mix(lit_color.rgb, camera.fog_color.rgb, fog_factor)"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a serialized native GPU"]
fn actual_terrain_shader_pixels_use_only_receiving_enabled_fog_range() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("terrain fog pixel witness requires an adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("receiving terrain fog pixel witness"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .expect("terrain fog pixel witness requires a device");
    fn pixel(
        visual: &mut TerrainVisualImpl,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> [u8; 4] {
        // The shipped pipeline compiles the shipped terrain.wgsl and uses its
        // real vertex format/bind layouts; no test shader or fog formula.
        visual.create_terrain_pipeline(device).unwrap();
        let extent = wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain fog pixel target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain fog depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TERRAIN_PIPELINES_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let white = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terrain fog white material"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            white.as_image_copy(),
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white.create_view(&Default::default());
        let sampler = device.create_sampler(&Default::default());
        let textures = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain fog actual material bindings"),
            layout: visual.terrain_texture_bind_group_layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let (fog_color, fog_range) = visual.terrain_fog_uniforms();
        let data = TerrainUniforms {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            projection_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            camera_position: [0.0, 0.0, 10.0, 0.0],
            time: 0.0,
            _time_padding: [0.0; 3],
            fog_color,
            fog_range,
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain fog actual uniform bytes"),
            contents: bytemuck::bytes_of(&data),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let camera = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain fog actual camera binding"),
            layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let vertices = [
            Vec3::new(-1.0, -1.0, 0.5),
            Vec3::new(3.0, -1.0, 0.5),
            Vec3::new(-1.0, 3.0, 0.5),
        ]
        .map(|position| {
            TerrainVertex::from_components(
                position,
                Vec3::Y,
                (0.0, 0.0),
                [0; 4],
                [1.0, 0.0, 0.0, 0.0],
                [1.0; 4],
            )
        });
        let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain fog actual vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terrain fog pixel readback"),
            size: 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain fog shipped pipeline witness"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(visual.terrain_pipeline.as_ref().unwrap());
            pass.set_bind_group(0, &camera, &[]);
            pass.set_bind_group(1, &textures, &[]);
            pass.set_vertex_buffer(0, vertex.slice(..));
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(4),
                },
            },
            extent,
        );
        queue.submit([encoder.finish()]);
        let (done, completed) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                done.send(result).unwrap();
            });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(20)),
            })
            .unwrap();
        completed
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        let mapped = readback
            .slice(..)
            .get_mapped_range()
            .expect("mapped terrain fog pixels");
        let pixel = mapped[256 * 2 + 2 * 4..256 * 2 + 2 * 4 + 4]
            .try_into()
            .unwrap();
        drop(mapped);
        readback.unmap();
        pixel
    }
    let mut receiving = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    let baseline = pixel(&mut receiving, &device, &queue);
    receiving.set_lighting(None, None, None, Some([1.0, 0.0, 0.0]), None);
    assert_eq!(
        pixel(&mut receiving, &device, &queue),
        baseline,
        "color-only input must not enable actual terrain shader fog"
    );
    receiving.set_lighting(None, None, None, None, Some((1.0, 2.0)));
    assert_eq!(
        pixel(&mut receiving, &device, &queue),
        [0, 0, 255, 255],
        "enabled receiving fog must reach pixels through shipped shader and actual uniform layout"
    );
    assert_eq!(
        pixel(&mut peer, &device, &queue),
        baseline,
        "changing receiving fog must not enable a separate visual's fog"
    );
    assert_eq!(
        baseline, [255; 4],
        "witness must draw actual terrain before comparing fog"
    );
}
