use super::*;
use crate::terrain::water_tracks::{
    WaterTrackDrawRange, WaterTrackType, WaterTrackVertex, WaterTracksFlush,
    water_track_strip_indices,
};

#[test]
#[ignore = "requires a serialized native GPU"]
fn unchanged_water_and_shoreline_keep_their_gpu_buffers() {
    use game_engine::common::common_services::{CommonServices, with_common_services};

    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));
    let points = [[0.0, 0.0, 1.0], [4.0, 0.0, 1.0], [4.0, 4.0, 1.0], [0.0, 4.0, 1.0]];
    visual.overlay.shoreline_tiles.push(crate::terrain::ShoreLineTile {
        packed_xy: 0,
        verts: points,
        t: [0.0, 0.25, 0.5, 0.75],
    });
    visual.set_map_water_areas(vec![TerrainWaterArea {
        points: points.to_vec(),
        is_river: false,
        river_start: 0,
    }]);
    visual.rebuild_all_overlays();
    let buffers = |visual: &TerrainVisualImpl| {
        let water = &visual.polygon_water_meshes[0];
        let shore = &visual.shoreline_meshes[0];
        (water.vertex_buffer.clone(), shore.vertex_buffer.clone())
    };
    let (first_water, first_shore) = buffers(&visual);
    // Client-frame refresh and a separate dirty-overlay flush can run together.
    visual.sync_polygon_water_meshes(&device).unwrap();
    visual.rebuild_all_overlays();
    assert_eq!(buffers(&visual), (first_water.clone(), first_shore.clone()));

    visual.overlay.river_v_origin += 0.002;
    visual.sync_polygon_water_meshes(&device).unwrap();
    let (advanced_water, unchanged_shore) = buffers(&visual);
    assert_ne!(advanced_water, first_water);
    assert_eq!(unchanged_shore, first_shore);

    let elevated = points.map(|[x, y, z]| [x, y, z + 1.0]);
    visual.set_map_water_areas(vec![TerrainWaterArea {
        points: elevated.to_vec(),
        is_river: false,
        river_start: 0,
    }]);
    let (replaced_water, _) = buffers(&visual);
    assert_ne!(replaced_water, advanced_water);
    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || visual.init_gpu_resources(device, queue)).unwrap();
    visual.rebuild_all_overlays();
    let (recreated_water, recreated_shore) = buffers(&visual);
    assert_ne!(recreated_water, replaced_water);
    assert_ne!(recreated_shore, first_shore);

    visual.set_map_water_areas(Vec::new());
    assert!(visual.polygon_water_meshes.is_empty());
}

#[test]
#[ignore = "requires a serialized native GPU"]
fn adjacent_water_tracks_preserve_ordered_color_and_shroud_pixels() {
    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_water_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));

    set_water_test_camera(&mut visual, &device, Vec2::ZERO, Vec2::new(4.0, 2.0));
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    for (name, rgba) in [
        ("wave256.tga", [255, 255, 255, 255]),
        ("blue-wave.tga", [0, 0, 255, 255]),
    ] {
        let texture = TerrainVisualImpl::create_solid_rgba_texture(&device, &queue, rgba, name);
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: visual.water_texture_bind_group_layout.as_ref().unwrap(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        if name == "wave256.tga" {
            visual.water_texture_bind_group = Some(bind_group.clone());
        }
        visual.water_named_bind_groups.insert(
            name.to_owned(),
            NamedWaterBind {
                _texture: texture,
                bind_group,
            },
        );
    }
    visual.set_shroud_overlay_r8(
        4,
        2,
        1.0,
        [0.0, 0.0],
        &[255, 217, 191, 159, 255, 217, 191, 159],
    );
    visual.sync_shroud_dest_texture();

    let mut flush = WaterTracksFlush::default();
    let strip = water_track_strip_indices(2, 2);
    for (handle, (name, diffuse, left, right)) in [
        ("wave256.tga", 0x80ff_0000, 0.2, 2.7),
        ("", 0x8000_ff00, 0.8, 3.2),
        ("blue-wave.tga", 0x80ff_ffff, 1.0, 3.7),
        ("wave256.tga", 0x80ff_ff00, 1.5, 3.8),
    ]
    .into_iter()
    .enumerate()
    {
        let first_vertex = flush.vertices.len();
        let first_index = flush.indices.len();
        flush.ranges.push(WaterTrackDrawRange {
            handle,
            first_vertex,
            vertex_count: 4,
            first_index,
            index_count: strip.len(),
            texture_name: name.to_owned(),
            wave_type: WaterTrackType::Pond,
        });
        flush.vertices.extend(
            [(left, 0.25), (right, 0.25), (left, 1.75), (right, 1.75)].map(|(x, y)| {
                WaterTrackVertex {
                    x,
                    y,
                    z: 1.5,
                    diffuse,
                    u1: 0.5,
                    v1: 0.5,
                }
            }),
        );
        flush
            .indices
            .extend(strip.iter().map(|&index| index + first_vertex as u16));
    }

    let mut separate_meshes = Vec::new();
    for range in &flush.ranges {
        let single = WaterTracksFlush {
            vertices: flush.vertices.clone(),
            indices: flush.indices.clone(),
            ranges: vec![range.clone()],
        };
        visual.upload_water_track_meshes(&single);
        assert_eq!(visual.water_track_meshes.len(), 1);
        separate_meshes.append(&mut visual.water_track_meshes);
    }
    visual.water_track_meshes = separate_meshes;
    let separate_color = read_water_pixels(&visual, &device, &queue, false);
    let separate_shroud = read_water_pixels(&visual, &device, &queue, true);
    assert_ne!(
        separate_color, separate_shroud,
        "the shroud must affect the readback"
    );
    visual.water_track_meshes.swap(0, 1);
    assert_ne!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color,
        "the overlapping alpha colors must detect a change in primitive order",
    );

    visual.upload_water_track_meshes(&flush);
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, true),
        separate_shroud
    );
    let meshes: Vec<_> = visual
        .water_track_meshes
        .iter()
        .map(|mesh| (mesh.texture_name.as_str(), mesh.index_count))
        .collect();
    assert_eq!(
        meshes,
        [
            ("wave256.tga", 12),
            ("blue-wave.tga", 6),
            ("wave256.tga", 6)
        ]
    );

    let first_index_buffer = &visual.water_track_meshes[0].index_buffer;
    assert!(
        visual.water_track_meshes[1..]
            .iter()
            .all(|mesh| &mesh.index_buffer == first_index_buffer)
    );

    let original = std::mem::take(&mut visual.water_track_meshes);
    let mut changed = flush.clone();
    changed.indices[..4].copy_from_slice(&[0, 3, 1, 2]);
    visual.upload_water_track_meshes(&changed);
    assert_eq!(
        visual.water_track_meshes[0].index_count,
        original[0].index_count
    );
    assert_ne!(
        visual.water_track_meshes[0].index_buffer,
        original[0].index_buffer
    );
    assert_ne!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );

    // A previously recorded frame can retain its old immutable topology.
    visual.water_track_meshes = original;
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, false),
        separate_color
    );
    assert_eq!(
        read_water_pixels(&visual, &device, &queue, true),
        separate_shroud
    );
    check_projected_shroud_uploads(&mut visual, &device, &queue);
}

fn check_projected_shroud_uploads(
    visual: &mut TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let (width, height) = (67, 41);
    let origin = [-17.0, -13.0];
    let mut cells: Vec<_> = (0..width * height)
        .map(|index| (20 + (index * 17 + index / width * 9) % 235) as u8)
        .collect();
    visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells);
    let mask_bytes = visual.sync_shroud_dest_texture();
    let r8_pixels = read_water_pixels(visual, device, queue, true);
    let unshrouded = read_water_pixels(visual, device, queue, false);
    assert_ne!(r8_pixels, unshrouded);
    let texture = visual.shroud_gpu.dest_texture.as_ref().unwrap();
    assert_eq!((texture.width(), texture.height()), (67, 41));
    assert_eq!(texture.format(), wgpu::TextureFormat::R8Unorm);
    assert_eq!(mask_bytes, cells.len());

    // Compare all pixels with the old replicated RGBA format through the same
    // sampler, projection parameters, mesh, and native terrain shroud shader.
    let rgba = legacy_shroud_binding(visual, device, queue, &cells, 67, 41);
    let r8 = visual.shroud_gpu.bind_group.replace(rgba).unwrap();
    assert_eq!(read_water_pixels(visual, device, queue, true), r8_pixels);
    visual.shroud_gpu.bind_group = Some(r8);
    for _ in 0..90 {
        visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells);
        assert_eq!(visual.sync_shroud_dest_texture(), 0);
    }
    assert_eq!(read_water_pixels(visual, device, queue, true), r8_pixels);
    visual.set_shroud_overlay_r8(width, height, 1.0, origin, &cells[..cells.len() - 1]);
    assert_eq!(visual.sync_shroud_dest_texture(), 0);
    eprintln!(
        "shroud native frame CRC32 {:08x}, projected mask {mask_bytes} bytes, 90 identical updates 0 bytes",
        crc32fast::hash(&r8_pixels),
    );

    visual.set_shroud_level(18, 13, 255);
    cells[13 * width as usize + 18] = 255;
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    let after_cell = read_water_pixels(visual, device, queue, true);
    assert_ne!(after_cell, r8_pixels);
    visual.set_shroud_level(18, 13, 255);
    assert_eq!(visual.sync_shroud_dest_texture(), 0);

    visual.set_shroud_overlay_r8(width, height, 1.0, [-18.0, -13.0], &cells);
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    let after_origin = read_water_pixels(visual, device, queue, true);
    assert_ne!(after_origin, after_cell);
    visual.create_shroud_overlay_pipelines(device).unwrap();
    assert_eq!(visual.sync_shroud_dest_texture(), mask_bytes);
    assert_eq!(read_water_pixels(visual, device, queue, true), after_origin);

    visual.clear_shroud_overlay();
    assert_eq!(visual.sync_shroud_dest_texture(), 1);
    assert_eq!(read_water_pixels(visual, device, queue, true), unshrouded);
    visual.clear_shroud_overlay();
    assert_eq!(visual.sync_shroud_dest_texture(), 0);
}

fn legacy_shroud_binding(
    visual: &TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    cells: &[u8],
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("prior RGBA terrain shroud test control"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let pixels: Vec<_> = cells
        .iter()
        .flat_map(|&cell| [cell, cell, cell, 255])
        .collect();
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("prior RGBA terrain shroud test control"),
        layout: visual.shroud_gpu.bind_layout.as_ref().unwrap(),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(
                    visual.shroud_gpu.sampler.as_ref().unwrap(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: visual
                    .shroud_gpu
                    .params
                    .as_ref()
                    .unwrap()
                    .as_entire_binding(),
            },
        ],
    })
}

#[test]
#[ignore = "requires native GPU and the retail Zero Hour MapsZH.big"]
fn tournament_b_water_tracks_upload_one_mesh() {
    use game_engine::common::system::big_file_system::BigFileSystem;
    use std::io::Read;

    let assets_dir =
        std::env::var_os("GENERALS_ASSETS_DIR").expect("retail Zero Hour assets required");
    let mut files = BigFileSystem::new();
    files
        .open_archive_file(Path::new(&assets_dir).join("Data").join("MapsZH.big"))
        .unwrap();
    let mut bytes = Vec::new();
    files
        .open_file("Maps/TournamentB/TournamentB.wak", 0)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let records = crate::terrain::water_tracks::decode_wak_records(&bytes).unwrap();
    let mut visual = TerrainVisualImpl::new();
    assert_eq!(visual.water_tracks_mut().load_records(&records).len(), 921);
    let (device, queue) = native_water_test_device();
    visual.device = Some(device);
    visual.queue = Some(queue);
    visual.flush_water_tracks();

    assert_eq!(visual.last_water_tracks_flush().ranges.len(), 921);
    assert_eq!(visual.water_track_meshes.len(), 1);
    assert_eq!(visual.water_track_meshes[0].index_count, 921 * 6);
    assert_eq!(visual.water_track_meshes[0].texture_name, "wave256.tga");

    let indices = visual.water_track_meshes[0].index_buffer.clone();
    let vertices = visual.water_track_meshes[0].vertex_buffer.clone();
    let initial_vertices = visual.last_water_tracks_flush().vertices.clone();
    for _ in 0..30 {
        visual.flush_water_tracks();
        assert_eq!(visual.water_track_meshes.len(), 1);
        assert_eq!(visual.water_track_meshes[0].index_buffer, indices);
        assert_ne!(visual.water_track_meshes[0].vertex_buffer, vertices);
    }
    assert_ne!(visual.last_water_tracks_flush().vertices, initial_vertices);
}

#[test]
#[ignore = "requires native GPU and the retail Generals Textures.big"]
fn stock_wake_texture_keeps_the_original_seven_dds_levels() {
    use game_engine::common::common_services::{CommonServices, with_common_services};
    use game_engine::common::system::big_file_system::BigArchiveBackend;
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::subsystem_interface::SubsystemInterface;

    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail base Generals assets required");
    let archive = Path::new(&base).join("Data").join("Textures.big");
    let source = tempfile::tempdir().unwrap();
    std::fs::hard_link(archive, source.path().join("Textures.big")).unwrap();
    let common = Arc::new(CommonServices::new());
    with_common_services(&common, || {
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(source.path());
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }
        let (device, queue) = native_water_test_device();
        let mut visual = TerrainVisualImpl::new();
        visual.create_terrain_pipeline(&device).unwrap();
        visual.create_water_pipeline(&device).unwrap();
        visual.device = Some(Arc::clone(&device));
        visual.queue = Some(queue);
        visual.ensure_named_water_bind_group(&device, "wave256.tga");
        let texture = &visual.water_named_bind_groups["wave256.tga"]._texture;
        assert_eq!((texture.width(), texture.height()), (256, 128));
        assert_eq!(texture.mip_level_count(), 7);
    });
}

#[test]
#[ignore = "requires native GPU and the retail Generals Textures.big"]
fn mounted_wake_dds_is_bound_before_first_native_water_draw() {
    use game_engine::common::system::big_file_system::BigFileSystem;
    use std::io::Read;

    let base = std::env::var_os("GENERALS_BASE_DIR").expect("retail base Generals assets required");
    let archive = Path::new(&base).join("Data").join("Textures.big");
    let mut files = BigFileSystem::new();
    files.open_archive_file(archive).unwrap();
    let mut dds = Vec::new();
    files
        .open_file("Art/Textures/wave256.dds", 0)
        .unwrap()
        .read_to_end(&mut dds)
        .unwrap();

    let (device, queue) = native_water_test_device();
    let mut visual = TerrainVisualImpl::new();
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_water_pipeline(&device).unwrap();
    visual.create_shroud_overlay_pipelines(&device).unwrap();
    visual.device = Some(Arc::clone(&device));
    visual.queue = Some(Arc::clone(&queue));
    let wake = visual.water_tracks_mut().add_wave_from_info(
        WaterTrackType::Pond,
        Vec2::new(100.0, 100.0),
        Vec2::new(100.0, 128.0),
        false,
    )[0];
    visual
        .water_tracks_mut()
        .track_mut(wake)
        .unwrap()
        .update(1000);

    let mut requests = Vec::new();
    visual.hydrate_mounted_water_track_textures(|filename| {
        requests.push(filename.to_owned());
        (filename == "wave256.dds").then(|| dds.clone())
    });
    assert_eq!(requests, ["wave256.dds"]);
    assert!(visual.last_water_tracks_flush().ranges.is_empty());
    let authored = &visual.water_named_bind_groups["wave256.tga"];
    assert_eq!(
        (authored._texture.width(), authored._texture.height()),
        (256, 128)
    );
    assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 9);
    // DDSFileClass drops the two lowest levels after applying reduction.
    assert_eq!(authored._texture.mip_level_count(), 7);
    visual.water_texture_bind_group = Some(authored.bind_group.clone());
    visual.hydrate_mounted_water_track_textures(|_| panic!("resident wake art reopened"));
    visual.flush_water_tracks();
    assert_eq!(visual.water_track_meshes.len(), 1);

    let bounds = visual.last_water_tracks_flush().vertices.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(min, max), vertex| {
            let point = Vec2::new(vertex.x, vertex.y);
            (min.min(point), max.max(point))
        },
    );
    set_water_test_camera(&mut visual, &device, bounds.0, bounds.1);
    let pixels_with_mips = read_water_pixels(&visual, &device, &queue, false);
    assert!(
        pixels_with_mips
            .chunks_exact(4)
            .any(|p| p[0..3] != [0, 0, 0])
    );

    // Use the same BC decoder and uploader for level zero in both captures.
    // The sampler's available mip levels are the only difference.
    let dds = ww3d_assets::image::load_dds_from_memory(&dds).unwrap();
    let single_level = ww3d_renderer_3d::texture_system::upload_dds_levels(
        &device,
        &queue,
        "single-level wake control",
        dds.width,
        dds.height,
        1,
        dds.compression.unwrap(),
        |level| dds.get_level_data(level),
    )
    .unwrap();
    let layout = Arc::clone(visual.water_texture_bind_group_layout.as_ref().unwrap());
    visual.bind_water_track_texture(&device, &layout, "wave256.tga", single_level);
    let pixels_without_mips = read_water_pixels(&visual, &device, &queue, false);
    assert_ne!(pixels_with_mips, pixels_without_mips);

    // The actual mounted wake's material still samples the tinted shroud once
    // its R8 texture is available to both native and Draw consumers.
    let cell_size = (bounds.1 - bounds.0).max_element() + 1.0;
    let origin = (bounds.0 - Vec2::splat(cell_size)).to_array();
    let cells = [160; 4];
    visual.set_projected_shroud_overlay_r8(2, 2, cell_size, origin, [128, 255, 0], &cells);
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &cells)
            .is_none()
    );
    assert_eq!(visual.sync_shroud_dest_texture(), cells.len());
    let (view, sampler) = visual
        .matching_shroud_sampling_resources(&device, (2, 2), &cells)
        .unwrap();
    assert!(Arc::ptr_eq(
        view,
        visual.shroud_gpu.dest_view.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        sampler,
        visual.shroud_gpu.sampler.as_ref().unwrap()
    ));
    assert!(
        visual
            .matching_shroud_sampling_resources(&device, (2, 2), &[159, 160, 160, 160])
            .is_none()
    );
    let (foreign, _) = native_water_test_device();
    assert!(
        visual
            .matching_shroud_sampling_resources(&foreign, (2, 2), &cells)
            .is_none()
    );

    let tinted = read_water_pixels(&visual, &device, &queue, true);
    assert_ne!(tinted, pixels_without_mips);
    for (base, shaded) in pixels_without_mips
        .chunks_exact(4)
        .zip(tinted.chunks_exact(4))
    {
        for (channel, tint) in [0.0, 1.0, 128.0 / 255.0].into_iter().enumerate() {
            let expected = (f32::from(base[channel]) * (160.0 / 255.0) * tint).round() as u8;
            assert!(
                shaded[channel].abs_diff(expected) <= 1,
                "mounted colored wake pixel {base:?} -> {shaded:?}"
            );
        }
    }
}

fn set_water_test_camera(
    visual: &mut TerrainVisualImpl,
    device: &wgpu::Device,
    min: Vec2,
    max: Vec2,
) {
    let scale = Vec2::splat(2.0) / (max - min);
    let projection = Mat4::from_cols_array_2d(&[
        [scale.x, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0],
        [0.0, scale.y, 0.0, 0.0],
        [-1.0 - min.x * scale.x, -1.0 - min.y * scale.y, 0.5, 1.0],
    ]);
    let uniforms = TerrainUniforms {
        view_proj: projection.to_cols_array_2d(),
        view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        projection_matrix: projection.to_cols_array_2d(),
        camera_position: [0.0; 4],
        time: 0.0,
        _time_padding: [0.0; 3],
        fog_color: [0.0; 4],
        fog_range: [0.0; 4],
    };
    let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("water track test camera"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    visual.terrain_camera_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: visual.terrain_camera_bind_group_layout.as_ref().unwrap(),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera.as_entire_binding(),
        }],
    }));
}

fn native_water_test_device() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    (Arc::new(device), Arc::new(queue))
}

fn read_water_pixels(
    visual: &TerrainVisualImpl,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shroud: bool,
) -> Vec<u8> {
    let size = wgpu::Extent3d {
        width: 32,
        height: 16,
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
        TERRAIN_PIPELINES_DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let color_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ordered water track readback"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
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
            ..Default::default()
        });
        if shroud {
            visual.record_water_scene_draws(&mut pass);
        } else {
            visual.record_water_draws(&mut pass);
        }
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    let submission = queue.submit([encoder.finish()]);
    let (done, received) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            done.send(result).unwrap()
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(20)),
        })
        .unwrap();
    received
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap();
    let pixels = readback
        .slice(..)
        .get_mapped_range()
        .unwrap()
        .chunks_exact(256)
        .flat_map(|row| row[..size.width as usize * 4].iter().copied())
        .collect();
    readback.unmap();
    pixels
}
