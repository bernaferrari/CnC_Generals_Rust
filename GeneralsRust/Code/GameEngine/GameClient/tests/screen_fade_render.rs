use game_client_rust::gui::game_window::WindowEventEffects;
use game_client_rust::gui::ui_renderer::UIRenderer;
use game_client_rust::gui::window_draw_context::WindowDrawContext;
use game_client_rust::gui::window_manager::WindowManager;
use std::sync::{Arc, mpsc};

struct RestoreDirectory(std::path::PathBuf);
impl Drop for RestoreDirectory {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).unwrap();
    }
}

fn draw_fade(
    manager: &mut WindowManager,
    renderer: &mut UIRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> u8 {
    let size = wgpu::Extent3d {
        width: 64,
        height: 64,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("screen fade test"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screen fade readback"),
        size: 64 * 64 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    renderer.begin_frame();
    manager.draw_all(&mut WindowDrawContext::new(
        Some(renderer),
        Default::default(),
    ));
    let view = texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("screen fade pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
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
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        size,
    );
    queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap()
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let value = slice.get_mapped_range().unwrap()[0];
    buffer.unmap();
    value
}

#[test]
fn screen_fade_draws_intermediate_and_skipped_terminal_frames() {
    let temp = tempfile::tempdir().unwrap();
    let ini_dir = temp.path().join("Data/INI");
    std::fs::create_dir_all(&ini_dir).unwrap();
    std::fs::write(ini_dir.join("WindowTransitions.ini"), "WindowTransition Fade\n Window\n  Style = SCREENFADE\n  FrameDelay = 0\n END\nEND\nWindowTransition Sound\n Window\n  Style = REVERSESOUND\n  FrameDelay = 0\n END\nEND\n").unwrap();
    let _restore = RestoreDirectory(std::env::current_dir().unwrap());
    std::env::set_current_dir(temp.path()).unwrap();
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
            ..Default::default()
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("screen fade test"),
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
    let mut manager = WindowManager::new();
    manager.init();
    let mut messages = Vec::new();
    let mut effects = WindowEventEffects::new(&mut messages);
    manager.transition_set_group("Fade", true, None);
    manager.update(&mut effects);
    let first = draw_fade(&mut manager, &mut renderer, &device, &queue);
    for _ in 1..15 {
        manager.update(&mut effects);
    }
    let middle = draw_fade(&mut manager, &mut renderer, &device, &queue);
    for _ in 15..30 {
        manager.update(&mut effects);
    }
    let end = draw_fade(&mut manager, &mut renderer, &device, &queue);
    assert!(
        first > 230 && (90..=150).contains(&middle) && end < 5,
        "frames 1/15/30: {first}/{middle}/{end}"
    );
    manager.transition_set_group("Fade", true, None);
    manager.update(&mut effects);
    manager.transition_set_group("Sound", true, None);
    let skipped = draw_fade(&mut manager, &mut renderer, &device, &queue);
    assert!(
        skipped < 5,
        "skipping fade must draw its terminal frame, got {skipped}"
    );
}
