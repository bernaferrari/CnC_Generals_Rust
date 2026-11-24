use super::*;

#[test]
#[ignore = "requires a native GPU adapter"]
fn snow_preserves_authored_dark_color_and_alpha() {
    crate::test_support::ensure_ambient_host();
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("snow pixel test requires a GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("snow pixel test requires a GPU device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let directory = tempfile::tempdir().unwrap();
    let texture_path = directory.path().join("SnowPixelFixture.tga");
    let texels = [[0, 0, 0, 255], [0, 0, 0, 128], [255, 255, 255, 0]];
    image::RgbaImage::from_fn(12, 1, |x, _| image::Rgba(texels[(x / 4) as usize]))
        .save_with_format(&texture_path, image::ImageFormat::Tga)
        .unwrap();

    let terrain = [(
        TerrainSourceTileClass {
            first_tile: 0,
            num_tiles: 1,
            width: 1,
            name: "SnowPixelReceiver".to_owned(),
        },
        [200; 3],
    )];
    let mut visual = live_terrain_visual(
        &device,
        &queue,
        &terrain,
        HeightMap::new(64, 64, 0.0, 1.0),
        1.0,
    );
    visual.create_road_pipeline(&device).unwrap();
    visual.ensure_snow_texture_bind_group_owned(&device, texture_path.to_str().unwrap());
    assert!(!visual.snow_texture_is_fallback);

    // Hold the flake geometry fixed while the normal snow draw samples the authored art.
    let vertices = [
        (30.5, 1.25, 0.0),
        (33.5, 1.25, 1.0),
        (33.5, 1.75, 1.0),
        (30.5, 1.75, 0.0),
    ]
    .map(|(x, z, u)| OverlayGpuVertex::from_cpp_xyzduv(x, z, 0.0, 0xffff_ffff, u, 0.5));
    visual.snow_mesh = TerrainVisualImpl::upload_overlay_mesh(
        &device,
        "Test Snow Flake",
        &vertices,
        &[0, 1, 2, 0, 2, 3],
    );
    let pixels = render_live_terrain_visual(&device, &queue, &mut visual);
    let sample = |column: usize| &pixels[4 * 256 + column * 4..4 * 256 + column * 4 + 4];
    assert_eq!(sample(2), [200, 200, 200, 255], "bare terrain");
    assert_eq!(sample(8), [0, 0, 0, 255], "opaque black snow texel");
    assert_eq!(
        sample(16),
        [100, 100, 100, 255],
        "half-alpha black snow texel"
    );
    assert_eq!(sample(24), sample(2), "transparent snow texel");
}
