use game_client_rust::gui::ui_renderer::{UIRect, UIRenderer};
use std::sync::{Arc, mpsc};

fn render_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut UIRenderer,
    target_size: (u32, u32),
    scissor: Option<UIRect>,
) -> Vec<u8> {
    let extent = wgpu::Extent3d {
        width: target_size.0,
        height: target_size.1,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI scissor regression target"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("UI scissor readback"),
        size: u64::from(target_size.0 * target_size.1 * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    renderer.begin_frame();
    renderer
        .draw_rect_with_scissor(
            UIRect::new(0.0, 0.0, 800.0, 600.0),
            [0.0, 1.0, 0.0, 1.0],
            scissor,
        )
        .unwrap();
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI scissor regression pass"),
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
            .render_with_target_size(&mut pass, target_size)
            .unwrap();
    }
    renderer.end_frame();
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(target_size.0 * 4),
                rows_per_image: Some(target_size.1),
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

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    &pixels[(y * width + x) * 4..][..4]
}

#[test]
fn ui_scissors_follow_the_real_attachment_size_and_logical_coordinates() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("UI scissor regression requires a WGPU adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("UI scissor regression device"),
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
    renderer.set_screen_size(800, 600);

    let pixels = render_pixels(&device, &queue, &mut renderer, (640, 480), None);
    assert!(
        pixel(&pixels, 640, 639, 479)[1] > 200,
        "unclipped UI covers the smaller target"
    );

    let pixels = render_pixels(
        &device,
        &queue,
        &mut renderer,
        (640, 480),
        Some(UIRect::new(400.0, 300.0, 400.0, 300.0)),
    );
    assert_eq!(
        pixel(&pixels, 640, 319, 239)[1],
        0,
        "outside the scaled scissor"
    );
    assert!(
        pixel(&pixels, 640, 320, 240)[1] > 200,
        "inside the scaled scissor"
    );
    let pixels = render_pixels(
        &device,
        &queue,
        &mut renderer,
        (1280, 960),
        Some(UIRect::new(400.0, 300.0, 400.0, 300.0)),
    );
    assert_eq!(pixel(&pixels, 1280, 639, 479)[1], 0);
    assert!(pixel(&pixels, 1280, 640, 480)[1] > 200);
}
