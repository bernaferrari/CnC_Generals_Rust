#![cfg(not(target_arch = "wasm32"))]

use game_client_rust::{
    core::game_client::{ClientInputAuthority, GameClient, ReceivingDrawablePolicy},
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
    ascii_string::AsciiString,
    ini::{
        INI, INILoadType,
        ini_animation::{OwnedAnim2DTemplates, ensure_anim2d_collection},
        ini_mapped_image::{
            Image as MappedImage, ImageCollection as MappedImages, OwnedMappedImages,
            Region2D as MappedRegion, ensure_mapped_image_collection,
        },
    },
    name_key_generator::NameKeyNamespace,
    system::{
        big_file_system::{BigArchiveBackend, BigFileSystem},
        file_system::get_file_system,
        subsystem_interface::SubsystemInterface,
    },
    thing::{
        module::StaticGameLodLevel, module_factory::ModuleFactory,
        receiving_module_content::ReceivingModuleContent,
    },
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

fn archive_entry(archive: PathBuf, member: &str) -> Vec<u8> {
    let mut big = BigFileSystem::new();
    big.open_archive_file(archive).unwrap();
    let mut raw = Vec::new();
    big.open_file(member, 1)
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    raw
}

fn stock_disabled_atlas() -> image::RgbaImage {
    let base = PathBuf::from(std::env::var_os("GENERALS_BASE_DIR").unwrap());
    let raw = archive_entry(
        base.join("Data/Textures.big"),
        "Art/Textures/scgameuserinterface512_002.tga",
    );
    let atlas = image::load_from_memory_with_format(&raw, image::ImageFormat::Tga)
        .unwrap()
        .into_rgba8();
    assert_eq!(atlas.dimensions(), (512, 512));
    atlas
}

/// Pin the archived Disabled animation's first visible frame for the icon eligibility check.
fn mount_stock_disabled_icon() {
    let atlas = stock_disabled_atlas();
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

const LIGHTNING_FRAMES: [(&str, [f32; 4]); 3] = [
    ("SCPLightning000", [171., 159., 203., 191.]),
    ("SCPLightning001", [205., 169., 237., 201.]),
    ("SCPLightning002", [239., 169., 271., 201.]),
];

fn stock_disabled_animation_ini() -> String {
    let game = PathBuf::from(std::env::var_os("GENERALS_ASSETS_DIR").unwrap());
    let animation = archive_entry(game.join("Data/INIZH.big"), "Data/INI/Animation2D.ini");
    let animation = String::from_utf8_lossy(&animation);
    let index = animation.find("Animation Disabled").unwrap();
    animation[index..]
        .lines()
        .take_while(|line| line.trim() != "End")
        .collect::<Vec<_>>()
        .join("\n")
        + "\nEnd\n"
}

/// Install the real template and its first three authored atlas rows.
fn mount_stock_disabled_animation() {
    let block = stock_disabled_animation_ini();
    let atlas = stock_disabled_atlas();
    for (name, [left, top, right, bottom]) in LIGHTNING_FRAMES {
        let mut metadata = MappedImage::new();
        metadata.set_name(name.to_owned());
        ensure_mapped_image_collection().write().add_image(metadata);

        let mut frame = Image::from_rgba_data(atlas.as_raw(), 512, 512, name).unwrap();
        frame.set_uv(Region2D::from_coords(
            left / 512.,
            top / 512.,
            right / 512.,
            bottom / 512.,
        ));
        get_mapped_image_collection().write().add_image(frame);
    }
    INI::new()
        .with_inline_source(&block, INI::parse_current_file)
        .unwrap();
    let template = ensure_anim2d_collection()
        .read()
        .find_template(&AsciiString::from("Disabled"))
        .unwrap();
    let template = template.read();
    assert_eq!(template.get_num_frames(), 16);
    assert_eq!(template.get_num_frames_between_updates(), 2);
    assert_eq!(template.get_frame_name(0), Some("SCPLightning000"));
    assert_eq!(template.get_frame_name(1), Some("SCPLightning001"));
    assert_eq!(template.get_frame_name(2), Some("SCPLightning002"));
}

fn admit_disabled_state(client: &mut GameClient, id: DrawableId, disabled: bool) {
    let drawable = client.find_drawable_by_id_mut(id).unwrap();
    let basic = drawable.downcast_mut::<BasicDrawable>().unwrap();
    basic.set_presentation_host_residual(
        Arc::default(),
        None,
        false,
        false,
        1.0,
        false,
        0,
        false,
        0.0,
        0,
        0,
        0,
        0,
        disabled,
        false,
        0,
        0,
        false,
        0.0,
        false,
        0,
        Vec::new(),
        String::new(),
        0,
        0,
        String::new(),
    );
    let region = IRegion2D::new(ICoord2D::new(32, 48), ICoord2D::new(152, 54));
    basic.overlay_data.visible = true;
    basic.overlay_data.health_region = Some(region);
    basic.draw_disabled(&region);
    assert_eq!(basic.overlay_data.show_disabled, disabled);
}

#[test]
#[ignore = "requires mounted retail INIZH.big and base Textures.big and native GPU"]
fn installed_disabled_animation_ticks_once_and_resumes_from_the_owned_snapshot() {
    let world = new_world_services();
    with_world_services(&world, || {
        mount_stock_disabled_animation();

        verify_disabled_playback(None);
    });
}

#[test]
#[ignore = "requires mounted retail INIZH.big and base Textures.big and native GPU"]
fn hosted_disabled_draw_and_load_use_boot_admission_without_legacy_assets() {
    let world = new_world_services();
    with_world_services(&world, || {
        let base = PathBuf::from(std::env::var_os("GENERALS_BASE_DIR").unwrap());
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(base.join("Data"));
        {
            let files = get_file_system();
            let mut files = files.lock().unwrap();
            files.register_backend(Box::new(backend));
            files.init().unwrap();
        }

        let mut mapped = MappedImages::new();
        for (name, [left, top, right, bottom]) in LIGHTNING_FRAMES {
            let mut frame = MappedImage::new();
            frame.set_name(name.to_owned());
            frame.set_filename("SCGameUserInterface512_002.tga".to_owned());
            frame.set_texture_width(512);
            frame.set_texture_height(512);
            frame.image_size.x = 32;
            frame.image_size.y = 32;
            frame.set_uv(MappedRegion::new(
                left / 512.,
                top / 512.,
                right / 512.,
                bottom / 512.,
            ));
            mapped.add_image(frame);
        }
        let images = OwnedMappedImages::from_collection(&mapped);
        let templates = OwnedAnim2DTemplates::default()
            .overlay_ini_text(&stock_disabled_animation_ini(), &|name| {
                images.find(name).is_some()
            })
            .unwrap();
        assert_eq!(
            templates.find("Disabled").unwrap().get_frame_name(1),
            Some("SCPLightning001")
        );
        assert!(
            ensure_anim2d_collection()
                .read()
                .find_template(&AsciiString::from("Disabled"))
                .is_none()
        );
        assert!(
            get_mapped_image_collection()
                .read()
                .find_image_by_name("SCPLightning001")
                .is_none()
        );
        verify_disabled_playback(Some((&templates, &images)));
        assert!(
            ensure_anim2d_collection()
                .read()
                .find_template(&AsciiString::from("Disabled"))
                .is_none()
        );
        assert!(
            get_mapped_image_collection()
                .read()
                .find_image_by_name("SCPLightning001")
                .is_none()
        );
    });
}

fn verify_disabled_playback(startup: Option<(&OwnedAnim2DTemplates, &OwnedMappedImages)>) {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let (device, queue) = (Arc::new(device), Arc::new(queue));
    let new_client = || {
        let mut renderer = UIRenderer::new(
            Arc::clone(&device),
            Arc::clone(&queue),
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        renderer.set_screen_size(WIDTH, HEIGHT);
        let authority = if startup.is_some() {
            ClientInputAuthority::NativeHosted
        } else {
            ClientInputAuthority::StandaloneCore
        };
        let mut client = GameClient::new_with_input_authority(authority).unwrap();
        client.set_ui_renderer(renderer);
        if let Some((templates, images)) = startup {
            client.admit_receiving_ui_images(images);
            client.admit_owned_world_animations(templates, images);
        }
        client
    };
    let draw = |client: &mut GameClient, logic_frame| {
        client.update_cpp_snow_and_anim2d(0.0, logic_frame);
        client.ui_renderer_mut().unwrap().begin_overlay_frame();
        assert_eq!(client.draw_live_ingame_hud(logic_frame).icon_overlays, 1);
        let renderer = client.ui_renderer_mut().unwrap();
        render_pixels(renderer, &device, &queue)
    };
    let lit = |frame: &[u8]| {
        frame
            .chunks_exact(4)
            .filter(|px| px[..3] != [0, 0, 0])
            .count()
    };

    let mut first = new_client();
    let id = first
        .register_drawable_with_template(
            Box::new(BasicDrawable::new(DrawableId::INVALID)),
            Some("DisabledPlaybackFixture".to_owned()),
        )
        .unwrap();
    admit_disabled_state(&mut first, id, true);
    let initial = draw(&mut first, 10);
    assert_eq!(lit(&initial), 0, "archived frame 000 is fully transparent");
    assert!(
        draw(&mut first, 10) == initial,
        "same logic frame repaints unchanged"
    );

    let frame_001 = draw(&mut first, 11);
    assert!(
        lit(&frame_001) > 0,
        "stock frame 001 draws on the next owning tick"
    );
    assert!(draw(&mut first, 11) == frame_001);
    assert!(
        draw(&mut first, 12) == frame_001,
        "stock 66 ms rounds up to two logic frames"
    );

    let save = first
        .capture_xfer_bytes_with_owned_auxiliaries(&[])
        .unwrap();
    let mut content =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    content
        .load_ini_text(
            "Object DisabledPlaybackFixture\nBuildCost = 1\nEnd",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut restored = new_client();
    restored
        .restore_from_xfer_bytes_with_receiving_content(
            &save,
            &mut content,
            ReceivingDrawablePolicy {
                time_of_day: Some(game_client_rust::system::TimeOfDay::Afternoon),
                use_draw_module_lod: true,
                static_game_lod: StaticGameLodLevel::High,
                use_tree_sway: true,
            },
        )
        .unwrap();
    admit_disabled_state(&mut restored, id, true);
    restored
        .find_drawable_by_id_mut(id)
        .unwrap()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .get_icon_info_mut()
        .update(13);
    let restored_frame_002 = draw(&mut restored, 13);
    assert!(lit(&restored_frame_002) > 0);
    assert!(
        restored_frame_002 != frame_001,
        "loaded clock advances before the first paint"
    );
    assert!(
        draw(&mut first, 12) == frame_001,
        "same-ID peer keeps its 001 frame"
    );
    first
        .find_drawable_by_id_mut(id)
        .unwrap()
        .downcast_mut::<BasicDrawable>()
        .unwrap()
        .overlay_data
        .visible = false;
    first.update_cpp_snow_and_anim2d(0.0, 13);
    first.ui_renderer_mut().unwrap().begin_overlay_frame();
    assert_eq!(first.draw_live_ingame_hud(13).icon_overlays, 0);
    admit_disabled_state(&mut first, id, true);
    assert!(
        draw(&mut first, 13) == restored_frame_002,
        "registration still advances while the HUD overlay is suppressed"
    );
    assert!(draw(&mut restored, 13) == restored_frame_002);

    admit_disabled_state(&mut first, id, false);
    assert_eq!(lit(&draw(&mut first, 14)), 0);
    admit_disabled_state(&mut first, id, true);
    assert_eq!(
        lit(&draw(&mut first, 15)),
        0,
        "next disable starts freshly at frame 000"
    );
    assert!(draw(&mut first, 16) == frame_001);
    assert!(draw(&mut restored, 13) == restored_frame_002);
    println!(
        "Stock Disabled 000→001→002: lit pixels {}, {}, {}. Save/load, paused owner and retired/reapplied playback verified.",
        lit(&initial),
        lit(&frame_001),
        lit(&restored_frame_002)
    );
}
