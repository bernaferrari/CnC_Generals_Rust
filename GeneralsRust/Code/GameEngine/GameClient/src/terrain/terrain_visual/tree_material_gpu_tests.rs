use super::*;

#[test]
#[ignore = "requires a native WGPU device for atlas resource identity"]
fn tree_atlas_gpu_upload_survives_vertex_refresh_and_reloads_after_lod_change() {
    use crate::terrain::{TreeModuleData, TreeSphere, TreeTgaHeader, TreeTileImageSpec};
    use game_engine::common::common_services::CommonServicesScope;

    let _common = CommonServicesScope::new();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut visual = TerrainVisualImpl::new();
    visual.device = Some(Arc::new(device));
    visual.queue = Some(Arc::new(queue));
    visual.tree_buffer.add_tree_type(
        TreeModuleData {
            model_name: "GpuAtlasTest".to_owned(),
            texture_name: "GpuAtlasTest.tga".to_owned(),
            ..Default::default()
        },
        TreeSphere::default(),
    );
    visual.tree_buffer.update_texture(&[TreeTileImageSpec {
        texture_name: "GpuAtlasTest.tga".to_owned(),
        header: TreeTgaHeader::truecolor(64, 64),
    }]);
    visual.finish_tree_meshes(None, true, None);
    let atlas = visual.tree_atlas_texture.as_ref().unwrap().clone();
    assert_eq!(atlas.width(), 512);

    visual.tree_buffer.force_vertex_rebuild();
    visual.finish_tree_meshes(None, true, None);
    assert_eq!(visual.tree_atlas_texture.as_ref(), Some(&atlas));

    visual.tree_buffer.set_texture_lod(1);
    visual.finish_tree_meshes(None, true, None);
    let reduced = visual.tree_atlas_texture.as_ref().unwrap();
    assert_ne!(reduced, &atlas);
    assert_eq!(reduced.width(), 256);
}

#[test]
#[ignore = "requires native WGPU readback for the tree material"]
fn tree_alpha_test_applies_shroud_only_to_covered_pixels() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let (device, queue) = (Arc::new(device), Arc::new(queue));
    let mut visual = TerrainVisualImpl::new();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.create_tree_pipeline(&device).unwrap();
    visual.overlay.shroud_width = 1;
    visual.overlay.shroud_height = 1;
    visual.overlay.shroud_cell_size = 1.0;
    visual.overlay.shroud_cells = Arc::from([128]);
    visual.overlay.shroud_tint = Some([255; 3]);
    visual.shroud_gpu.dirty = true;
    visual.sync_shroud_dest_texture();

    let uniforms = TerrainUniforms {
        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        projection_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        camera_position: [0.0; 4],
        time: 0.0,
        _time_padding: [0.0; 3],
        fog_color: [0.0; 4],
        fog_range: [0.0; 4],
    };
    let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tree readback camera"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    visual.terrain_camera_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tree readback camera"),
        layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera.as_entire_binding(),
        }],
    }));
    let atlas_size = wgpu::Extent3d {
        width: 2,
        height: 1,
        depth_or_array_layers: 1,
    };
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("alpha threshold controls"),
        size: atlas_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        atlas.as_image_copy(),
        &[255, 255, 255, 95, 255, 255, 255, 96],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        atlas_size,
    );
    visual.tree_atlas_texture = Some(atlas);
    visual.bind_tree_atlas_group();
    let vertices: Vec<_> = [(-1.0, 0.0, 0.25), (0.0, 1.0, 0.75)]
        .into_iter()
        .flat_map(|(left, right, coord)| {
            [[left, -1.0], [right, -1.0], [right, 1.0], [left, 1.0]].map(|[x, y]| TreeGpuVertex {
                position: [x, y, 0.5],
                color: [1.0; 3],
                tex_coords: [coord, 0.5],
                diffuse: u32::MAX,
            })
        })
        .collect();
    visual.tree_meshes.push(
        TerrainVisualImpl::upload_tree_mesh(
            &device,
            &vertices,
            &[0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        )
        .unwrap(),
    );

    let extent = wgpu::Extent3d {
        width: 4,
        height: 2,
        depth_or_array_layers: 1,
    };
    let output = |label, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = output(
        "tree color",
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = output(
        "tree depth",
        TERRAIN_PIPELINES_DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let mut command = device.create_command_encoder(&Default::default());
    {
        let mut pass = command.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
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
            ..Default::default()
        });
        visual.record_tree_draws(&mut pass);
    }
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tree alpha readback"),
        size: 512,
        mapped_at_creation: false,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    });
    command.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(2),
            },
        },
        extent,
    );
    queue.submit([command.finish()]);
    let (done, completed) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            done.send(result).unwrap()
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
    let image = readback.slice(..).get_mapped_range().unwrap();
    assert_eq!(
        &image[0..4],
        &[255, 0, 0, 255],
        "95 discards without darkening the background"
    );
    assert_eq!(
        &image[12..16],
        &[128, 128, 128, 96],
        "96 writes one shroud-modulated fragment"
    );
}
