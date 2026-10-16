//! Actual owned WGPU resource admission; no Engine/window or tactical adapter.
use super::*;
use crate::display::view::{Point3, Vector2};
use crate::display::view_filter::ViewFilterState;

#[test]
#[ignore = "requires a native WGPU adapter; run this explicit GPU acceptance test"]
fn owned_filter_gpu_admits_zero_fade_and_preserves_independent_cpu_phase_on_resize() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("owned filter GPU acceptance requires an adapter; never silently skipped");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("owned filter GPU acceptance"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))
    .expect("owned filter GPU acceptance requires a device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut a = ShaderFilterResources::default();
    let mut b = ShaderFilterResources::default();
    let mut fade = ViewFilterState::new();
    fade.set_mode(FilterMode::BWBlackAndWhite);
    fade.set_filter(FilterType::BlackAndWhite);
    fade.set_fade_parameters(4, 1);
    let before = fade.composite(Vector2::zero());
    assert_eq!(before.fade, 0.);
    assert!(!a.has_scene_capture_for(8, 8, format));
    assert!(a.prepare(&device, &queue, format, 8, 8, &before));
    assert!(!a.has_scene_capture_for(8, 8, format));
    fade.tick_fade();
    let composite = fade.composite(Vector2::zero());
    let post = fade.post_render(1, Vector2::zero());
    assert_eq!(post.fade, 0.25);
    let (texture, view) = make_color_target(&device, "owned filter destination", 8, 8, format);
    let red = [255u8, 0, 0, 255].repeat(8 * 8);
    queue.write_texture(
        texture.as_image_copy(),
        &red,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32),
            rows_per_image: Some(8),
        },
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(a.composite_live_view_filter(
        &device,
        &queue,
        &mut encoder,
        &view,
        &texture,
        format,
        8,
        8,
        &composite,
        &post
    ));
    queue.submit([encoder.finish()]);
    assert!(a.has_scene_capture_for(8, 8, format));
    let green = [0u8, 255, 0, 255].repeat(8 * 8);
    queue.write_texture(
        texture.as_image_copy(),
        &green,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32),
            rows_per_image: Some(8),
        },
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    let mut retained = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(a.composite_retained_view_filter(
        &device,
        &queue,
        &mut retained,
        &view,
        &texture,
        format,
        8,
        8,
        &composite,
        &post,
        false
    ));
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("retained filter scene readback"),
        size: 256 * 8,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    retained.copy_texture_to_buffer(
        a.gpu.as_ref().unwrap().scene_tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(8),
            },
        },
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([retained.finish()]);
    let (done, completion) = std::sync::mpsc::channel();
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
    completion
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert_eq!(
        &readback
            .slice(..)
            .get_mapped_range()
            .expect("retained scene readback range must be mapped after successful completion")
            [..4],
        &[255, 0, 0, 255],
        "retained scene must remain red after the destination was overwritten green"
    );
    readback.unmap();
    let mut phase = ViewFilterState::new();
    phase.set_mode(FilterMode::MBInAndOutAlpha);
    phase.set_filter(FilterType::MotionBlur);
    phase.set_pos(Point3::new(1., 2., 3.));
    for frame in 1..4 {
        phase.post_render(frame, Vector2::zero());
    }
    let count = phase.post_render(3, Vector2::zero()).motion_blur_count;
    assert!(b.prepare(
        &device,
        &queue,
        format,
        16,
        16,
        &phase.composite(Vector2::zero())
    ));
    assert!(a.prepare(&device, &queue, format, 32, 32, &composite));
    assert!(!a.has_scene_capture_for(8, 8, format));
    assert!(
        !a.has_scene_capture_for(32, 32, format),
        "resize invalidates capture until refreshed"
    );
    assert_eq!(a.gpu.as_ref().unwrap().width, 32);
    assert_eq!(b.gpu.as_ref().unwrap().width, 16);
    assert_eq!(
        phase.post_render(3, Vector2::zero()).motion_blur_count,
        count
    );
    assert_eq!(
        phase.post_render(4, Vector2::zero()).motion_blur_count,
        count + 5
    );
}
