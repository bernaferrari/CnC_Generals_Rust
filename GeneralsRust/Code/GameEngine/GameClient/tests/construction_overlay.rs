#![cfg(not(target_arch = "wasm32"))]

use game_client_rust::{
    core::game_client::GameClient,
    display::image::{Image, Region2D, get_mapped_image_collection},
    drawable::{
        DrawableExt,
        drawable::{
            BasicDrawable, DrawableId, ICoord2D, IRegion2D, format_under_construction_desc,
        },
    },
    game_text::GameText,
    gui::UIRenderer,
};
use game_engine::common::{
    ascii_string::AsciiString, ini::ini_animation::ensure_anim2d_collection,
    system::big_file_system::BigFileSystem,
};
use gamelogic::system::engine_stores::{new_world_services, with_world_services};
use glam::Vec2;
use std::{
    io::Read,
    path::PathBuf,
    sync::{Arc, mpsc},
};

const WIDTH: u32 = 192;
const HEIGHT: u32 = 96;
const ROW_BYTES: u32 = WIDTH * 4;

fn render_pixels(render: &mut UIRenderer, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("construction overlay parity"),
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

/// Pin the archived Disabled animation's first visible frame for the icon eligibility check.
fn mount_stock_disabled_icon() {
    let base = PathBuf::from(std::env::var_os("GENERALS_BASE_DIR").unwrap());
    let mut archive = BigFileSystem::new();
    archive
        .open_archive_file(base.join("Data/Textures.big"))
        .unwrap();
    let mut raw = Vec::new();
    archive
        .open_file("Art/Textures/scgameuserinterface512_002.tga", 1)
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let atlas = image::load_from_memory_with_format(&raw, image::ImageFormat::Tga)
        .unwrap()
        .into_rgba8();
    assert_eq!(atlas.dimensions(), (512, 512));
    let mut frame = Image::from_rgba_data(atlas.as_raw(), 512, 512, "SCPLightning001").unwrap();
    frame.set_uv(Region2D::from_coords(
        205.0 / 512.0,
        169.0 / 512.0,
        237.0 / 512.0,
        201.0 / 512.0,
    ));
    get_mapped_image_collection().write().add_image(frame);

    let collection = ensure_anim2d_collection();
    let template = collection
        .write()
        .new_template(AsciiString::from("Disabled"));
    let mut template = template.write();
    template.allocate_images(1);
    template
        .store_image_name(Some("SCPLightning001".to_owned()))
        .unwrap();
}

#[test]
#[ignore = "requires mounted installed EnglishZH.big and native GPU"]
fn installed_construction_label_renders_without_missing_disabled_icon() {
    let world = new_world_services();
    with_world_services(&world, || {
        let mut game_text = GameText::default();
        assert!(game_text.load_runtime_strings().unwrap() > 5000);
        assert_eq!(
            game_text.fetch_with_exists_owned("CONTROLBAR:UnderConstructionDesc"),
            ("Building: %.0f%%".to_owned(), true),
        );
        game_text.publish_legacy_language_strings();
        assert_eq!(format_under_construction_desc(0.42), "Building: 42%");

        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
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

        let mut structure = BasicDrawable::new(DrawableId::INVALID);
        structure.overlay_data.visible = true;
        structure.overlay_data.health_region = Some(IRegion2D::new(
            ICoord2D::new(32, 48),
            ICoord2D::new(152, 54),
        ));
        structure.overlay_data.is_under_construction = true;
        structure.overlay_data.construction_percent = 0.42;
        structure.overlay_data.show_disabled = true;
        let structure = client.register_drawable(Box::new(structure)).unwrap();
        let draw = |client: &mut GameClient| {
            client.ui_renderer_mut().unwrap().begin_overlay_frame();
            assert_eq!(client.draw_live_ingame_hud(0).icon_overlays, 1);
            let renderer = client.ui_renderer_mut().unwrap();
            let commands = renderer.queued_draw_command_count();
            (commands, render_pixels(renderer, &device, &queue))
        };

        let under_construction_disabled = draw(&mut client);
        let drawable = client.find_drawable_by_id_mut(structure).unwrap();
        let overlay = &mut drawable
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .overlay_data;
        overlay.show_disabled = false;
        let under_construction_enabled = draw(&mut client);
        assert_eq!(under_construction_disabled.0, 2);
        assert!(under_construction_disabled == under_construction_enabled);

        let reference = client.ui_renderer_mut().unwrap();
        reference.begin_overlay_frame();
        reference
            .draw_text_simple(
                "Building: 42%",
                Vec2::new(54.0, 37.0),
                10.0,
                [0., 0., 0., 1.],
            )
            .unwrap();
        reference
            .draw_text_simple(
                "Building: 42%",
                Vec2::new(53.0, 36.0),
                10.0,
                [1., 1., 1., 1.],
            )
            .unwrap();
        let reference_pixels = render_pixels(reference, &device, &queue);
        assert!(under_construction_disabled.1 == reference_pixels);
        let lit = reference_pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 0)
            .count();
        assert!(lit > 100, "construction glyphs must occupy real pixels");

        let drawable = client.find_drawable_by_id_mut(structure).unwrap();
        let overlay = &mut drawable
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .overlay_data;
        overlay.is_under_construction = false;
        overlay.show_disabled = true;
        let finished_disabled = draw(&mut client);
        assert_eq!(finished_disabled.0, 0);
        assert!(
            finished_disabled
                .1
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 255])
        );

        mount_stock_disabled_icon();
        let authored_disabled = draw(&mut client);
        assert_eq!(authored_disabled.0, 1, "the mounted valid icon draws");
        let authored_pixels = authored_disabled
            .1
            .chunks_exact(4)
            .filter(|px| px[..3] != [0, 0, 0])
            .count();
        assert!(
            authored_pixels > 0,
            "the archived frame must paint real pixels"
        );

        let drawable = client.find_drawable_by_id_mut(structure).unwrap();
        let overlay = &mut drawable
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .overlay_data;
        overlay.is_under_construction = true;
        let text_with_authored_icon = draw(&mut client);
        assert_eq!(
            text_with_authored_icon.0, 3,
            "a real Disabled icon can accompany construction"
        );
        assert!(text_with_authored_icon.1 != reference_pixels);

        println!(
            "Stock construction: {lit} glyph pixels and two label quads. Missing Disabled: zero quads. Archived Disabled: one quad with {authored_pixels} lit pixels."
        );
    });
}
