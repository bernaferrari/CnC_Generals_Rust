use game_client_rust::display::image::{Image as MappedImage, ImageCollection};
use game_client_rust::gui::{
    game_window::{Image, WIN_COLOR_UNDEFINED},
    ui_renderer::UIRenderer,
    window_draw_context::WindowDrawContext,
    window_manager::WindowManager,
};
use std::sync::{Arc, mpsc};

// The old renderer has no admission API; its control run still reads global art.
trait ImageAdmission<'a> {
    fn set_receiving_images(&mut self, images: &'a mut ImageCollection);
}
impl<'a> ImageAdmission<'a> for WindowDrawContext<'a> {
    fn set_receiving_images(&mut self, _: &'a mut ImageCollection) {}
}

fn image(name: &str, rgba: [u8; 4]) -> MappedImage {
    MappedImage::from_rgba_data(&rgba.repeat(4), 2, 2, name).unwrap()
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut UIRenderer,
    images: &mut ImageCollection,
    batch: bool,
) -> Vec<u8> {
    let extent = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("receiving WND image"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("receiving WND readback"),
        size: 64 * 64 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    renderer.begin_frame();
    let mut manager = WindowManager::new();
    let mut messages = Vec::new();
    let mut effects = game_client_rust::gui::game_window::WindowEventEffects::new(&mut messages);
    let window = manager
        .create_window(None, 0, 0, 64, 64, &mut effects)
        .unwrap();
    window.borrow_mut().set_draw_callback(move |_, _, context| {
        if let Some(manager) = context.manager {
            let image = Image {
                name: "SharedWindowArt".into(),
                width: 2,
                height: 2,
            };
            if batch {
                manager.win_draw_image_batch(&image, &[(0, 0, 32, 64), (32, 0, 64, 64)], context);
            } else {
                manager.win_draw_image(&image, 0, 0, 64, 64, WIN_COLOR_UNDEFINED, context);
            }
        }
    });
    {
        let mut context = WindowDrawContext::new(Some(renderer), Default::default());
        context.set_receiving_images(images);
        manager.draw_all(&mut context);
    }
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("receiving WND image pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer
            .render_with_target_size(&mut pass, (64, 64))
            .unwrap();
    }
    renderer.end_frame();
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        extent,
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let pixels = slice.get_mapped_range().unwrap().to_vec();
    buffer.unmap();
    pixels
}

#[test]
fn receiving_window_images_use_their_own_gpu_textures() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("receiving image regression requires a WGPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("receiving WND test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .unwrap();
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let mut renderer = UIRenderer::new(
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
    )
    .unwrap();
    renderer.set_screen_size(64, 64);
    game_client_rust::display::image::get_mapped_image_collection()
        .write()
        .add_image(image("SharedWindowArt", [255, 0, 0, 255]));
    let mut first = ImageCollection::new();
    first.add_image(image("SharedWindowArt", [0, 255, 0, 255]));
    let mut peer = ImageCollection::new();
    peer.add_image(image("SharedWindowArt", [0, 0, 255, 255]));
    let mut unavailable = ImageCollection::new();
    unavailable.add_image(MappedImage::with_name("SharedWindowArt"));
    for (catalog, channel) in [(&mut first, 1), (&mut peer, 2)] {
        let pixels = render(&device, &queue, &mut renderer, catalog, false);
        let pixel = &pixels[(32 * 64 + 32) * 4..][..4];
        assert!(
            pixel[channel] > 200 && pixel[0] < 20,
            "actual pixel: {pixel:?}"
        );
    }
    let pixels = render(&device, &queue, &mut renderer, &mut first, false);
    assert!(pixels[(32 * 64 + 32) * 4 + 1] > 200);
    let pixels = render(&device, &queue, &mut renderer, &mut unavailable, false);
    let pixel = &pixels[(32 * 64 + 32) * 4..][..4];
    assert!(
        pixel[0] < 150,
        "must not fall back to peer/global red: {pixel:?}"
    );
    for (catalog, channel) in [(&mut first, 1), (&mut peer, 2)] {
        let pixels = render(&device, &queue, &mut renderer, catalog, true);
        let pixel = &pixels[(32 * 64 + 32) * 4..][..4];
        assert!(
            pixel[channel] > 200 && pixel[0] < 20,
            "batch pixel: {pixel:?}"
        );
    }
    let pixels = render(&device, &queue, &mut renderer, &mut unavailable, true);
    assert_eq!(&pixels[(32 * 64 + 32) * 4..][..3], &[0, 0, 0]);
    let mut not_admitted = ImageCollection::new();
    let pixels = render(&device, &queue, &mut renderer, &mut not_admitted, true);
    assert!(pixels[(32 * 64 + 32) * 4] > 200);
    first
        .find_image_by_name_mut("SharedWindowArt")
        .unwrap()
        .set_status(game_client_rust::display::image::ImageStatus::ROTATED_90_CLOCKWISE);
    let pixels = render(&device, &queue, &mut renderer, &mut first, true);
    assert!(pixels[(32 * 64 + 32) * 4 + 1] > 200);
}

#[test]
fn admitted_image_cache_preserves_source_atlas_metadata() {
    use game_engine::common::ini::ini_mapped_image::{
        ICoord2D, Image, ImageCollection as SourceImages, ImageStatus, OwnedMappedImages, Region2D,
    };
    let mut image = Image::new();
    image.set_name("AtlasEntry".into());
    image.set_filename("own/atlas.tga".into());
    image.set_texture_width(128);
    image.set_texture_height(256);
    image.set_image_size(ICoord2D::new(24, 40));
    image.set_uv(Region2D::new(0.125, 0.25, 0.5, 0.75));
    image.status.set_flag(ImageStatus::ROTATED_90_CLOCKWISE);
    let mut source = SourceImages::new();
    source.add_image(image);
    let snapshot = OwnedMappedImages::from_collection(&source);
    let cache = ImageCollection::from_owned_mapped_images(&snapshot);
    let result = cache.find_image_by_name("AtlasEntry").unwrap();
    assert_eq!(result.get_filename(), "own/atlas.tga");
    assert_eq!(
        (result.get_texture_size().x, result.get_texture_size().y),
        (128, 256)
    );
    assert_eq!(
        (result.get_image_size().x, result.get_image_size().y),
        (24, 40)
    );
    assert_eq!(
        (
            result.get_uv().min.x,
            result.get_uv().min.y,
            result.get_uv().max.x,
            result.get_uv().max.y
        ),
        (0.125, 0.25, 0.5, 0.75)
    );
    assert!(
        result
            .get_status()
            .contains(game_client_rust::display::image::ImageStatus::ROTATED_90_CLOCKWISE)
    );
}

#[test]
fn native_clients_draw_their_own_same_named_image() {
    use game_engine::common::ini::ini_mapped_image::{
        ICoord2D, Image as SourceImage, ImageCollection as SourceImages, OwnedMappedImages,
    };
    let temp = tempfile::tempdir().unwrap();
    let red_path = temp.path().join("receiver-red.png");
    let green_path = temp.path().join("receiver-green.png");
    image::save_buffer(
        &red_path,
        &[255, 0, 0, 255].repeat(4),
        2,
        2,
        image::ColorType::Rgba8,
    )
    .unwrap();
    image::save_buffer(
        &green_path,
        &[0, 255, 0, 255].repeat(4),
        2,
        2,
        image::ColorType::Rgba8,
    )
    .unwrap();
    fn definitions(path: &std::path::Path) -> OwnedMappedImages {
        let mut image = SourceImage::new();
        image.set_name("NativeSharedImage".into());
        image.set_filename(path.to_string_lossy().into_owned());
        image.set_texture_width(2);
        image.set_texture_height(2);
        image.set_image_size(ICoord2D::new(2, 2));
        let mut source = SourceImages::new();
        source.add_image(image);
        OwnedMappedImages::from_collection(&source)
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("native UI ownership test requires a WGPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("native UI ownership test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .unwrap();
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    fn client(
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        defs: &OwnedMappedImages,
    ) -> game_client_rust::core::GameClient {
        let mut client = game_client_rust::core::GameClient::new().unwrap();
        let mut renderer = UIRenderer::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        renderer.set_screen_size(64, 64);
        client.set_ui_renderer(renderer);
        client.admit_receiving_ui_images(defs);
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let window = manager.create_window(None, 0, 0, 64, 64, effects).unwrap();
                window.borrow_mut().set_draw_callback(|_, _, context| {
                    if let Some(manager) = context.manager {
                        manager.win_draw_image(
                            &Image {
                                name: "NativeSharedImage".into(),
                                width: 2,
                                height: 2,
                            },
                            0,
                            0,
                            64,
                            64,
                            WIN_COLOR_UNDEFINED,
                            context,
                        );
                    }
                });
            })
        });
        client
    }
    let mut first = client(&device, &queue, &definitions(&red_path));
    let mut peer = client(&device, &queue, &definitions(&green_path));
    fn pixel(
        client: &mut game_client_rust::core::GameClient,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<u8> {
        let extent = wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64 * 64 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        client.ui_renderer_mut().unwrap().begin_frame();
        client.draw_receiving_ui(Default::default(), None);
        let renderer = client.ui_renderer_mut().unwrap();
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer
                .render_with_target_size(&mut pass, (64, 64))
                .unwrap();
        }
        renderer.end_frame();
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            extent,
        );
        queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let pixels = slice.get_mapped_range().unwrap();
        let value = pixels[(32 * 64 + 32) * 4..][..4].to_vec();
        drop(pixels);
        buffer.unmap();
        value
    }
    let a = pixel(&mut first, &device, &queue);
    let b = pixel(&mut peer, &device, &queue);
    let a_again = pixel(&mut first, &device, &queue);
    assert!(a[0] > 200 && a[1] < 20, "first: {a:?}");
    assert!(b[1] > 200 && b[0] < 20, "peer: {b:?}");
    assert_eq!(a, a_again);
    let (replacement_device, replacement_queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("replacement UI device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .unwrap();
    let replacement_device = Arc::new(replacement_device);
    let replacement_queue = Arc::new(replacement_queue);
    let mut replacement = UIRenderer::new(
        replacement_device.clone(),
        replacement_queue.clone(),
        wgpu::TextureFormat::Rgba8Unorm,
    )
    .unwrap();
    replacement.set_screen_size(64, 64);
    first.set_ui_renderer(replacement);
    let after_replacement = pixel(&mut first, &replacement_device, &replacement_queue);
    assert!(after_replacement[0] > 200 && after_replacement[1] < 20);
    assert_eq!(pixel(&mut peer, &device, &queue), b);
}
