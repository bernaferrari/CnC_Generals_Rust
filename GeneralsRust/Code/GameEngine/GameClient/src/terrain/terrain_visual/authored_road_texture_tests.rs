use super::*;
use game_engine::common::ini::ini_road::TerrainRoadCollection;

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a serialized native GPU"]
fn authored_road_dds_keeps_distinct_mip_pixels() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("road DDS test requires a GPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
        ..Default::default()
    }))
    .expect("road DDS test requires a GPU device");
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let road = directory.path().join("road_authored_mips.tga");
    let mut dds = vec![0u8; 128];
    dds[..4].copy_from_slice(b"DDS ");
    // DDSFileClass excludes the final two tiny levels. A complete 32×32
    // chain exposes four source levels before the road-specific cap.
    for (offset, value) in [
        (4, 124u32),
        (8, 0xA_1007),
        (12, 32),
        (16, 32),
        (20, 1024),
        (28, 6),
        (76, 32),
        (80, 4),
        (108, 0x40_1008),
    ] {
        dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    dds[84..88].copy_from_slice(b"DXT5");
    for (rgb565, blocks) in [
        (0xF800u16, 64),
        (0x07E0, 16),
        (0x001F, 4),
        (0xFFE0, 1),
        (0, 1),
        (0, 1),
    ] {
        for _ in 0..blocks {
            dds.extend_from_slice(&[255, 255, 0, 0, 0, 0, 0, 0]);
            dds.extend_from_slice(&rgb565.to_le_bytes());
            dds.extend_from_slice(&[0; 6]);
        }
    }
    std::fs::write(road.with_extension("dds"), &dds).unwrap();
    let mut roads = TerrainRoadCollection::new();
    roads.new_road(AsciiString::from("RoadDdsFixture")).texture =
        AsciiString::from(road.to_str().unwrap());
    let mut visual = TerrainVisualImpl::new();
    visual.queue = Some(Arc::clone(&queue));
    visual.admit_road_presentation(roads.presentation());
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_road_pipeline(&device).unwrap();
    visual.ensure_road_texture_bind_group(&device);
    assert_eq!(
        ww3d_assets::image::load_dds_from_memory(&dds)
            .unwrap()
            .mip_levels,
        4
    );
    assert!(!visual.road_texture_is_fallback, "test DDS must resolve");
    let texture = visual.road_texture.as_ref().unwrap();
    assert_eq!(texture.mip_level_count(), 3);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("road DDS mip readback"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var road: texture_2d<f32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let vertices = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(vertices[i], 0.0, 1.0);
}

@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(road, vec2<i32>(0, 0), i32(p.x));
}
"#
            .into(),
        ),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("road DDS mip readback"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mips = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture.create_view(&Default::default())),
        }],
    });
    let size = wgpu::Extent3d {
        width: 3,
        height: 1,
        depth_or_array_layers: 1,
    };
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let pixels = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 12,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output.create_view(&Default::default()),
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &mips, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &pixels,
            layout: Default::default(),
        },
        size,
    );
    let submission = queue.submit([encoder.finish()]);
    let (done, completed) = std::sync::mpsc::channel();
    pixels
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
    completed
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert_eq!(
        &*pixels.slice(..).get_mapped_range().unwrap(),
        &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a serialized native GPU"]
fn authored_road_textures_upload_three_mip_levels() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("road texture test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("road texture test requires a GPU device");

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("road_mip_fixture.tga");
    image::RgbaImage::from_pixel(16, 8, image::Rgba([88u8, 72, 40, 223]))
        .save_with_format(&path, image::ImageFormat::Tga)
        .unwrap();
    let name = path.to_str().unwrap();
    let mut roads = TerrainRoadCollection::new();
    roads.new_road(AsciiString::from("RoadMipFixture")).texture = AsciiString::from(name);
    let mut visual = TerrainVisualImpl::new();
    visual.admit_road_presentation(roads.presentation());
    visual.queue = Some(Arc::new(queue));
    visual.create_terrain_pipeline(&device).unwrap();
    visual.create_road_pipeline(&device).unwrap();
    visual.ensure_road_texture_bind_group(&device);
    assert!(!visual.road_texture_is_fallback, "test TGA must resolve");
    assert_eq!(visual.road_texture.as_ref().unwrap().mip_level_count(), 3);

    visual.ensure_named_road_texture(&device, name);
    let bound = &visual.road_named_bind_groups[&name.to_ascii_lowercase()];
    assert_eq!(bound._texture.mip_level_count(), 3);
}

#[test]
fn each_source_road_and_synthetic_join_uses_its_own_texture() {
    crate::test_support::ensure_ambient_host();
    let mut roads = TerrainRoadCollection::new();
    let cobble = roads.new_road(AsciiString::from("Cobblestone"));
    cobble.texture = AsciiString::from("TRCobbleStones.tga");
    let cobble_id = cobble.id;
    let dirt = roads.new_road(AsciiString::from("DirtRoad"));
    dirt.texture = AsciiString::from("TRDirtRoad.tga");
    let dirt_id = dirt.id;
    let bridge = roads.new_bridge(AsciiString::from("IronBridge"));
    bridge.texture = AsciiString::from("TBIronBridge.tga");
    let roads = roads.presentation();

    assert_eq!(
        authored_road_texture_name("Cobblestone", None, &roads),
        "TRCobbleStones.tga"
    );
    assert_eq!(
        authored_road_texture_name("DirtRoad", None, &roads),
        "TRDirtRoad.tga"
    );
    assert_eq!(
        authored_road_texture_name(
            "__SYNTH_TEE__",
            Some(&format!("RoadTypeId={dirt_id} Kind=TEE")),
            &roads
        ),
        "TRDirtRoad.tga"
    );
    assert_eq!(
        authored_road_texture_name(
            "Cobblestone",
            Some(&format!("RoadTypeId={dirt_id}")),
            &roads
        ),
        "TRCobbleStones.tga",
        "name wins over stale metadata"
    );
    assert_ne!(cobble_id, dirt_id);
    assert_eq!(
        authored_road_texture_name("IronBridge", None, &roads),
        "TBIronBridge.tga"
    );
    assert_eq!(authored_road_texture_name("Unknown", None, &roads), "");
}
