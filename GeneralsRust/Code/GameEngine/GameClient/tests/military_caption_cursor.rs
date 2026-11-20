#![cfg(not(target_arch = "wasm32"))]
use game_client_rust::gui::{UIRenderer, WindowManager};
use game_client_rust::{
    core::{game_client::GameClient, subsystems::InGameUISubsystem},
    game_text::GameText,
};
use gamelogic::system::engine_stores::{new_world_services, with_world_services};
use std::sync::{Arc, mpsc};

const LABEL: &str = "MAP:MD_USA01CINELocation01";
const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;
const ROW_BYTES: u32 = WIDTH * 4;

fn capture(
    client: &mut GameClient,
    frame: u32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<u8> {
    client.ui_renderer_mut().unwrap().begin_overlay_frame();
    let counts = client.draw_live_ingame_hud(frame);
    assert_eq!(counts.military_subtitles, 1);
    let render = client.ui_renderer_mut().unwrap();
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("caption parity"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = color.create_view(&Default::default());
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(ROW_BYTES * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
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
            ..Default::default()
        });
        render.render(&mut pass).unwrap();
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW_BYTES),
                rows_per_image: Some(HEIGHT),
            },
        },
        color.size(),
    );
    queue.submit([encoder.finish()]);
    let (complete, read) = mpsc::channel();
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        complete.send(result).unwrap()
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    read.recv().unwrap().unwrap();
    slice.get_mapped_range().unwrap().to_vec()
}

#[test]
#[ignore = "requires mounted installed EnglishZH.big and native GPU"]
fn usa01_caption_cursor_uses_final_row_metrics_on_a_cropped_surface() {
    let world = new_world_services();
    with_world_services(&world, || {
        let mut text = GameText::default();
        assert!(text.load_runtime_strings().unwrap() > 5000);
        let (source, exists) = text.fetch_with_exists_owned(LABEL);
        assert!(exists);
        assert_eq!(source.lines().map(str::len).collect::<Vec<_>>(), [33, 21]);
        text.publish_legacy_language_strings();

        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let (device, queue) = (Arc::new(device), Arc::new(queue));
        let mut renderer = UIRenderer::new(
            Arc::clone(&device),
            Arc::clone(&queue),
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        renderer.set_screen_size(WIDTH, HEIGHT);
        let mut client = GameClient::new().unwrap();
        client.set_ui_renderer(renderer);
        let mut ui = InGameUISubsystem::default();
        let mut manager = WindowManager::new();
        ui.push_military_subtitle(LABEL, 10000, 1, &mut manager);
        client.set_in_game_ui_subsystem(ui);

        for frame in 1..190 {
            client.ui_renderer_mut().unwrap().begin_overlay_frame();
            client.draw_live_ingame_hud(frame);
        }
        let first = capture(&mut client, 190, &device, &queue);
        for frame in 191..200 {
            client.ui_renderer_mut().unwrap().begin_overlay_frame();
            client.draw_live_ingame_hud(frame);
        }
        let second = capture(&mut client, 200, &device, &queue);
        let ink = |pixel: &[u8]| pixel[0] > 10 || pixel[1] > 10 || pixel[2] > 10;
        let active = |pixels: &[u8]| pixels.chunks_exact(4).filter(|p| ink(p)).count();
        let (text, with_block) = if active(&first) < active(&second) {
            (&first, &second)
        } else {
            (&second, &first)
        };
        let mut changed = [WIDTH, HEIGHT, 0, 0];
        let mut overlap = 0;
        let mut line_ranges = Vec::new();
        let mut current_line: Option<[u32; 4]> = None;
        for y in 0..HEIGHT {
            let mut row = None;
            for x in 0..WIDTH {
                let id = (y * ROW_BYTES + x * 4) as usize;
                if text[id..id + 4] != with_block[id..id + 4] {
                    overlap += usize::from(ink(&text[id..id + 4]));
                    changed[0] = changed[0].min(x);
                    changed[1] = changed[1].min(y);
                    changed[2] = changed[2].max(x);
                    changed[3] = changed[3].max(y);
                }
                if ink(&text[id..id + 4]) {
                    let bounds = row.get_or_insert([x, x]);
                    bounds[0] = bounds[0].min(x);
                    bounds[1] = bounds[1].max(x);
                }
            }
            if let Some([left, right]) = row {
                let bounds = current_line.get_or_insert([left, y, right, y]);
                bounds[0] = bounds[0].min(left);
                bounds[2] = bounds[2].max(right);
                bounds[3] = y;
            } else if let Some(bounds) = current_line.take() {
                line_ranges.push(bounds);
            }
        }
        println!(
            "blinking block bounds: {changed:?}; caption ink bounds: {line_ranges:?}; overwritten text pixels: {overlap}"
        );
        assert_eq!(overlap, 0, "cursor overwrote caption pixels");
        assert_eq!(line_ranges.len(), 2, "the exact caption rendered");
        let final_line = line_ranges[1];
        assert!(
            changed[0] > final_line[2],
            "cursor overlaid the final caption row"
        );
        assert!(
            changed[1] > line_ranges[0][3],
            "cursor sits on title instead of final row"
        );
        assert!(
            changed[1] <= final_line[1] && changed[3] >= final_line[3],
            "cursor spans final row's ink height"
        );
    });
}
