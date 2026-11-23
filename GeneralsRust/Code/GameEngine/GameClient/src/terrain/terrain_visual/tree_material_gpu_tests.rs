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
fn tree_shader_applies_cutout_shroud_darken_and_sway() {
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
        width: 16,
        height: 1,
        depth_or_array_layers: 1,
    };
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("alpha threshold and mip selection controls"),
        size: atlas_size,
        mip_level_count: 3,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for mip in 0..3 {
        let width = atlas_size.width >> mip;
        let pixels: Vec<u8> = (0..width)
            .flat_map(|x| {
                let alpha = match mip {
                    0 => 95 + u8::from(x >= width / 2),
                    1 => 112,
                    _ => 0,
                };
                [255, 255, 255, alpha]
            })
            .collect();
        let mut copy = atlas.as_image_copy();
        copy.mip_level = mip;
        queue.write_texture(
            copy,
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width, ..atlas_size },
        );
    }
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
                tree_effects: [
                    if coord > 0.5 { 1.0 } else { 0.0 },
                    if coord > 0.5 { 0.5 } else { 1.0 },
                    -1.0,
                ],
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
    let render = |visual: &TerrainVisualImpl| {
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
        readback.slice(..).get_mapped_range().unwrap().to_vec()
    };
    let image = render(&visual);
    assert_eq!(
        &image[0..4],
        &[255, 0, 0, 255],
        "95 discards without darkening the background"
    );
    assert_eq!(
        &image[12..16],
        &[64, 64, 64, 96],
        "96 writes one fragment with shroud and C++ push-aside darkening"
    );
    let mut sway = TreeSwayGpuUniform {
        factors: [[0.0; 4]; crate::terrain::tree_buffer::MAX_SWAY_TYPES],
    };
    sway.factors[0][0] = -0.5;
    queue.write_buffer(
        visual.tree_sway_uniform_buffer.as_ref().unwrap(),
        0,
        bytemuck::bytes_of(&sway),
    );
    let image = render(&visual);
    assert_eq!(&image[4..8], &[64, 64, 64, 96], "the upper tree moves left");
    assert_eq!(&image[12..16], &[255, 0, 0, 255], "the old pixel is clear");

    // 0.65 U across four screen pixels on a 16-texel atlas gives
    // LOD log2(16 * 0.65 / 4) = 1.38. Point mip selection keeps level 1
    // (alpha 112). Blending it with level 2 gives alpha 70 and discards.
    let minified = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
        .map(|[x, y]| TreeGpuVertex {
            position: [x, y, 0.5],
            color: [1.0; 3],
            tex_coords: [if x < 0.0 { 0.175 } else { 0.825 }, 0.5],
            diffuse: u32::MAX,
            tree_effects: [0.0, 1.0, -1.0],
        });
    visual.tree_meshes = vec![
        TerrainVisualImpl::upload_tree_mesh(&device, &minified, &[0, 1, 2, 0, 2, 3])
            .unwrap(),
    ];
    let image = render(&visual);
    for x in 0..4 {
        assert_eq!(image[4 * x + 3], 112, "one mip supplies pixel {x}");
    }
}


#[test]
#[ignore = "requires native WGPU validation of reused tree geometry buffers"]
fn tree_mesh_refresh_reuses_buffers_and_accepts_odd_index_counts() {
    use crate::terrain::{TreeModuleData, TreeRandom, TreeTypeMesh};
    use game_engine::common::common_services::CommonServicesScope;

    struct MinTreeRandom;
    impl TreeRandom for MinTreeRandom {
        fn int_range(&mut self, min: i32, _: i32) -> i32 {
            min
        }
        fn real_range(&mut self, min: f32, _: f32) -> f32 {
            min
        }
    }

    let _common = CommonServicesScope::new();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut visual = TerrainVisualImpl::new();
    visual.device = Some(Arc::new(device));
    visual.queue = Some(Arc::new(queue));
    let data = TreeModuleData {
        model_name: "ResidentTree".into(),
        texture_name: "ResidentTree.tga".into(),
        ..Default::default()
    };
    let place = |visual: &mut TerrainVisualImpl, id| {
        visual
            .tree_buffer
            .add_tree_from_model(
                id,
                Vec3::new(10.0, 20.0, 2.0),
                1.0,
                0.0,
                data.clone(),
                |_| {
                    Some((
                        TreeTypeMesh {
                            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 3.0]],
                            normals: None,
                            uvs: vec![[0.0, 0.0]; 3],
                            colors: None,
                            polygons: vec![[0, 1, 2]],
                            emissive: [0.0; 3],
                        },
                        Vec3::ONE,
                        None,
                    ))
                },
                &mut MinTreeRandom,
            )
            .unwrap();
    };
    place(&mut visual, 1);
    visual.finish_tree_meshes(None, true, None);
    let mesh = &visual.tree_meshes[0];
    let resident = (
        mesh.vertex_buffer.clone(),
        mesh.dynamic_vertex_buffer.clone(),
        mesh.index_buffer.clone(),
    );
    assert_eq!(mesh.index_count, 3);

    visual.tree_buffer.remove_tree(1);
    visual.finish_tree_meshes(None, true, None);
    assert!(!visual.has_visible_tree_mesh());

    place(&mut visual, 2);
    visual.finish_tree_meshes(None, true, None);
    let mesh = &visual.tree_meshes[0];
    assert_eq!(mesh.index_count, 3);
    assert_eq!(mesh.vertex_buffer, resident.0);
    assert_eq!(mesh.dynamic_vertex_buffer, resident.1);
    assert_eq!(mesh.index_buffer, resident.2);

    visual.queue.as_ref().unwrap().submit([]);
    visual.device.as_ref().unwrap().poll(wgpu::PollType::Poll).unwrap();
}
