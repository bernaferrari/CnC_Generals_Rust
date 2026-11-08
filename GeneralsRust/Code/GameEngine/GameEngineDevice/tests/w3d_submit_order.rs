#![cfg(feature = "w3d")]

use game_engine_device::W3DRenderer;

#[tokio::test]
async fn renderer_overlay_runs_after_its_owning_engine_frame() {
    let engine = ww3d_engine::init_headless(ww3d_engine::EngineConfig::default())
        .await
        .expect("headless engine");
    let (device, queue, submit) = {
        let engine = engine.lock();
        (engine.device(), engine.queue(), engine.frame_submit_state())
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("overlay ordering test"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = W3DRenderer::new_with_wgpu(&device, &queue, &format)
        .await
        .expect("renderer");
    let mut frame = engine.lock().begin_render().expect("begin frame");
    {
        let _pass = frame.encoder().begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("engine red clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    renderer.end_frame_with_view(Some(&view), &submit.lock()).await.expect("overlay");
    assert_eq!(submit.lock().last_out_of_frame_submit_count(), 0);

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("overlay ordering readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    submit.lock().submit_recorded(
        &queue,
        ww3d_engine::FrameCommandPhase::Post,
        copy.finish(),
        ww3d_engine::OutOfFrameReason::BlockingGpuReadback,
    );
    engine.lock().end_render(frame).expect("end frame");
    assert_eq!(submit.lock().last_frame_submit_count(), 1);

    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    rx.recv().expect("map callback").expect("map readback");
    let bytes = readback.slice(..).get_mapped_range().expect("mapped range");
    assert_eq!(&bytes[..4], &[0, 0, 0, 255]);
    drop(bytes);
    readback.unmap();
}
