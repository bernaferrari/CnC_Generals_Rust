//! Mounted EXEMPSphere material cache and frame submission witnesses.
use super::*;
use crate::presentation_frame::PresentationPrimaryGlobalLight;
use ww3d_renderer_3d::rendering::shader_system::shader::MaterialBlendMode;

#[test]
#[ignore = "requires mounted Zero Hour W3DZH.big/TexturesZH.big and native WGPU"]
fn mounted_emp_directional_tint_reaches_cached_additive_meshes_only_for_the_receiver() {
    let (ordinary, tinted, ordinary_again, scene_light) = render_emp_sphere(false);
    assert_eq!(
        ordinary, ordinary_again,
        "the untouched cached model must retain its pixels"
    );
    assert!(
        ordinary.iter().any(|p| p.iter().any(|&c| c > 8)),
        "the neutral authored model must draw"
    );
    assert!(
        greatest_cyan_increase(&ordinary, &tinted) > 8,
        "the exact cached additive meshes must receive the signed cyan directional light",
    );
    let scene_light = scene_light.expect("tint is carried as per-instance lighting");
    assert_eq!(scene_light.output_light_count(), 2);
    assert_eq!(
        scene_light.get_output_light_diffuse(0),
        Vec3::new(-0.75, 1.25, 1.25)
    );
    assert_eq!(scene_light.get_output_light_direction(0), Vec3::Y);
    assert_eq!(
        scene_light.get_output_light_diffuse(1),
        Vec3::new(-1., 1., 1.)
    );
    assert_eq!(scene_light.ambient, Vec3::new(0., 1., 1.));
}

#[test]
#[ignore = "requires mounted Zero Hour W3DZH.big/TexturesZH.big and native WGPU"]
fn mounted_emp_additive_mesh_uses_ambient_when_no_directional_lights_are_active() {
    let (ordinary, tinted, ordinary_again, scene_light) = render_emp_sphere(true);
    assert_eq!(ordinary, ordinary_again);
    let scene_light = scene_light.expect("tint is carried as per-instance lighting");
    assert_eq!(
        scene_light.output_light_count(),
        0,
        "this case isolates ambient contribution"
    );
    assert_eq!(scene_light.ambient, Vec3::new(0., 1., 1.));
    assert!(
        greatest_cyan_increase(&ordinary, &tinted) > 8,
        "the lit additive material must use the original Drawable-scene ambient",
    );
}

type FramePixels = Vec<[u8; 3]>;

fn greatest_cyan_increase(ordinary: &FramePixels, tinted: &FramePixels) -> i16 {
    ordinary
        .iter()
        .zip(tinted)
        .flat_map(|(base, changed)| {
            [
                i16::from(changed[1]) - i16::from(base[1]),
                i16::from(changed[2]) - i16::from(base[2]),
            ]
        })
        .max()
        .expect("native readback includes pixels")
}

fn render_emp_sphere(
    ambient_only: bool,
) -> (
    FramePixels,
    FramePixels,
    FramePixels,
    Option<Arc<LightEnvironmentClass>>,
) {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("mounted Drawable scene-light fixture requires a native device");
    let _engine_scope = engine.clone();
    let device = engine.lock().device();
    let queue = engine.lock().queue();
    let format = engine.lock().color_format();
    let mut graphics =
        GraphicsSystem::new(device.clone(), queue, format, engine.lock().depth_format()).unwrap();
    let mut forward = ForwardPass::initialize(engine, &graphics).unwrap();
    forward
        .renderer
        .renderer_handle()
        .lock()
        .unwrap()
        .pause_animation();

    let retail = crate::fixture_content::retail_content_root()
        .join("Command & Conquer Generals Zero Hour/Data");
    let mut assets = crate::assets::AssetManager::new().unwrap();
    assets
        .archive_file_system_mut()
        .set_local_search_roots(Vec::new());
    for name in ["W3DZH.big", "TexturesZH.big"] {
        futures::executor::block_on(
            assets
                .archive_file_system_mut()
                .load_big_file(retail.join(name)),
        )
        .expect("retail EXEMPSphere needs both authored archives");
    }
    let model = assets
        .load_w3d_model_blocking("exempsphere")
        .expect("retail sphere mesh");
    assert_eq!(model.meshes.len(), 2);
    for mesh in &model.meshes {
        assert!(
            mesh.texture_library
                .iter()
                .any(|name| name.eq_ignore_ascii_case("EXLighteningTile2.tga")),
            "retail mesh '{}' must carry its authored texture: {:?}",
            mesh.name,
            mesh.texture_library,
        );
    }
    assets.prime_texture_raw_blocking("EXLighteningTile2.tga");
    forward.application_assets = Some(Arc::new(Mutex::new(assets)));

    // Keep the stock geometry and UV mapper. Normalize only the test camera fit.
    let extent = model.bounding_box_max - model.bounding_box_min;
    let scale = 0.9 / extent.max_element();
    assert!(scale.is_finite() && scale > 0.);
    let center = (model.bounding_box_min + model.bounding_box_max) * 0.5;
    let position = Vec3::new(0., 0., 0.5) - center * scale;
    let transform = Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(scale));
    let mut items: Vec<_> = model
        .meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| {
            assert!(mesh.name.to_ascii_lowercase().starts_with("exempsphere"));
            let mut item = RenderItem::new(
                ObjectID(200),
                "exempsphere".into(),
                index,
                position,
                transform,
                &mesh.material,
                RenderPass::ForwardTransparent,
            );
            item.set_mesh_local_transform(mesh.transform);
            item.fow_visibility = ObjectVisibility::FULLY_VISIBLE;
            item
        })
        .collect();
    graphics.cache_model("exempsphere".into(), model);

    let diffuse = if ambient_only { [0.; 3] } else { [0.25; 3] };
    forward.object_scene_lights[0] = Some(PresentationPrimaryGlobalLight {
        ambient: [0.25; 3],
        diffuse,
        light_pos: [0.0, 0.0, -1.0],
        object_light_active: !ambient_only,
    });
    forward.object_scene_lights[1] = Some(PresentationPrimaryGlobalLight {
        ambient: [0.75; 3],
        diffuse: [0.0; 3],
        light_pos: [0.0, 0.0, -1.0],
        object_light_active: !ambient_only,
    });
    let lighting = CachedLighting {
        ambient_color: Some([0.25; 3]),
        sun_direction: Some([0., 1., 0.]),
        sun_color: Some(diffuse),
        ..Default::default()
    };
    let ordinary_cache: Vec<_> = items
        .iter()
        .map(|item| {
            let instance = forward
                .prepare_mesh_instance(&graphics, item, Some(&lighting))
                .unwrap()
                .unwrap();
            assert!(instance.get_lighting_environment().is_none());
            let model = instance.model.as_ref().unwrap().clone();
            let pass = &model.material_passes[0];
            assert_eq!(pass.shader.blend_mode(), MaterialBlendMode::Additive);
            let vertex_material = pass.get_vertex_material().unwrap();
            assert!(vertex_material.use_lighting);
            assert_eq!(
                vertex_material.ambient_color_source,
                ww3d_renderer_3d::material_system::ColorSourceType::Material
            );
            assert_eq!(vertex_material.ambient, Vec3::ONE);
            assert!(
                pass.get_texture(0)
                    .unwrap()
                    .get_name()
                    .to_ascii_lowercase()
                    .contains("exlighteningtile2")
            );
            model
        })
        .collect();

    let draw = |forward: &mut ForwardPass, graphics: &mut GraphicsSystem, items: &[RenderItem]| {
        let readback = readback_buffer(&device, "retail-emp-sphere-scene-light");
        let mut capture = |frame: &mut ww3d_engine::RenderFrame| {
            copy_frame(frame, &readback);
            Ok(())
        };
        forward
            .render(
                graphics,
                items,
                &Mat4::IDENTITY,
                &Mat4::IDENTITY,
                Vec3::new(0., 0., -1.),
                Some(&lighting),
                None,
                false,
                None,
                None,
                Some(&mut capture),
                None,
                None,
                (0., 0., 32., 32.),
                wgpu::Color::BLACK,
            )
            .unwrap();
        let bytes = pixels(&device, &readback);
        let has_bgra = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .map(|(x, y)| {
                let [a, g, b] = pixel(&bytes, x, y);
                if has_bgra { [b, g, a] } else { [a, g, b] }
            })
            .collect::<FramePixels>()
    };

    let ordinary = draw(&mut forward, &mut graphics, &items);
    let mut scene_light = None;
    for (item, model) in items.iter_mut().zip(&ordinary_cache) {
        let material = item.material.clone();
        item.scene_light_tint = Some(DrawableSceneLightTint([-1., 1., 1.]));
        assert_eq!(item.material.diffuse_color, material.diffuse_color);
        assert_eq!(item.material.emissive_color, material.emissive_color);
        let instance = forward
            .prepare_mesh_instance(&graphics, item, Some(&lighting))
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(instance.model.as_ref().unwrap(), model));
        scene_light = instance.get_lighting_environment().cloned();
    }
    let tinted = draw(&mut forward, &mut graphics, &items);
    for item in &mut items {
        item.scene_light_tint = None;
    }
    let ordinary_again = draw(&mut forward, &mut graphics, &items);
    eprintln!(
        "retail EXEMPSphere ambient_only={ambient_only} neutral_peak={} tinted_peak={} cyan_increase={}",
        ordinary.iter().flatten().max().unwrap(),
        tinted.iter().flatten().max().unwrap(),
        greatest_cyan_increase(&ordinary, &tinted)
    );
    (ordinary, tinted, ordinary_again, scene_light)
}
