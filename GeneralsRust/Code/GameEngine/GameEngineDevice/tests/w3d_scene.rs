#![cfg(feature = "w3d")]

use game_engine_device::w3d::scene::{
    player_index_to_color_index, BoundingSphere, CameraInfo, CustomScenePassMode, DrawableState,
    LightEnvKind, ObjectShroudStatus, Ray, RenderEvent, RenderInfo, RenderObject,
    RenderObjectClass, RenderPassKind, SceneConfig, W3D2DScene, W3DInterfaceScene, W3DLight,
    W3DScene, KINDOF_INFANTRY, KINDOF_SCORE, KINDOF_STRUCTURE,
};
use glam::Vec3;

fn object(name: &str, class_id: RenderObjectClass, center: Vec3, radius: f32) -> RenderObject {
    RenderObject {
        name: name.to_string(),
        class_id,
        bounding_sphere: BoundingSphere::new(center, radius),
        ..RenderObject::default()
    }
}

fn drawable(id: u32, flags: u32) -> DrawableState {
    DrawableState {
        drawable_id: id,
        object_id: Some(id + 100),
        kindof_flags: flags,
        controlling_player_index: 0,
        player_color: 0xff80_4020,
        ..DrawableState::default()
    }
}

#[test]
fn constants_and_player_color_bit_flip_match_cpp() {
    assert_eq!(player_index_to_color_index(0), 0);
    assert_eq!(player_index_to_color_index(1), 8);
    assert_eq!(player_index_to_color_index(2), 4);
    assert_eq!(player_index_to_color_index(3), 12);
    assert_eq!(player_index_to_color_index(7), 14);
    assert_eq!(player_index_to_color_index(8), 1);
}

#[test]
fn visibility_check_classifies_translucent_occluder_occludee_and_regular_queues() {
    let mut scene = W3DScene::default();
    scene.set_current_frame(10);

    let mut transparent_structure = drawable(1, KINDOF_STRUCTURE);
    transparent_structure.effective_opacity = 0.5;
    let building = drawable(2, KINDOF_STRUCTURE);
    let score = drawable(3, KINDOF_SCORE);
    let regular = drawable(4, 0);

    let transparent_id = scene.add_render_object(
        object("transparent", RenderObjectClass::Model, Vec3::ZERO, 2.0)
            .with_drawable(transparent_structure),
    );
    let building_id = scene.add_render_object(
        object(
            "building",
            RenderObjectClass::Model,
            Vec3::new(1.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(building),
    );
    let score_id = scene.add_render_object(
        object(
            "score",
            RenderObjectClass::Model,
            Vec3::new(2.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(score),
    );
    let regular_id = scene.add_render_object(
        object(
            "regular",
            RenderObjectClass::Model,
            Vec3::new(3.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(regular),
    );

    scene.visibility_check(&CameraInfo::default());

    assert_eq!(scene.queue_counts(), (1, 1, 1, 1));
    assert!(scene
        .get_render_object(transparent_id)
        .unwrap()
        .drawable_info
        .as_ref()
        .unwrap()
        .flags
        .contains(1 << 0));
    assert!(scene
        .get_render_object(building_id)
        .unwrap()
        .drawable_info
        .as_ref()
        .unwrap()
        .flags
        .contains(1 << 2));
    assert!(scene
        .get_render_object(score_id)
        .unwrap()
        .drawable_info
        .as_ref()
        .unwrap()
        .flags
        .contains(1 << 3));
    assert!(scene
        .get_render_object(regular_id)
        .unwrap()
        .drawable_info
        .as_ref()
        .unwrap()
        .flags
        .contains(1 << 4));
}

#[test]
fn render_orders_terrain_first_delays_occluders_and_flushes_translucent_with_alpha_reset() {
    let mut scene = W3DScene::default();
    let terrain_id = scene.add_render_object(object(
        "terrain",
        RenderObjectClass::TileMap,
        Vec3::ZERO,
        100.0,
    ));
    let building_id = scene.add_render_object(
        object(
            "building",
            RenderObjectClass::Model,
            Vec3::new(0.0, 0.0, 0.0),
            4.0,
        )
        .with_drawable(drawable(10, KINDOF_STRUCTURE)),
    );
    let mut translucent = drawable(11, 0);
    translucent.effective_opacity = 0.25;
    let translucent_id = scene.add_render_object(
        object(
            "translucent",
            RenderObjectClass::Model,
            Vec3::new(4.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(translucent),
    );

    let mut rinfo = RenderInfo::default();
    scene.render(&mut rinfo);

    let events = scene.render_events();
    assert!(events
        .iter()
        .any(|event| *event == RenderEvent::Terrain(terrain_id, RenderPassKind::Shroud)));
    assert!(events.iter().any(|event| *event
        == RenderEvent::Object(building_id, RenderPassKind::Normal, LightEnvKind::Default)));
    assert!(events.iter().any(|event| *event
        == RenderEvent::Object(
            translucent_id,
            RenderPassKind::Normal,
            LightEnvKind::Default
        )));
    assert_eq!(rinfo.alpha_override, 1.0);
}

#[test]
fn shroud_mask_heatvision_and_fogged_paths_match_render_one_object_decisions() {
    let mut scene = W3DScene::default();

    let mut shrouded = drawable(1, 0);
    shrouded.shroud_status = ObjectShroudStatus::Shrouded;
    let shrouded_id = scene.add_render_object(
        object("shrouded", RenderObjectClass::Model, Vec3::ZERO, 2.0).with_drawable(shrouded),
    );

    let mut heat = drawable(2, KINDOF_INFANTRY);
    heat.second_material_pass_opacity = 0.8;
    heat.stealth_visible_detected = true;
    let heat_id = scene.add_render_object(
        object(
            "heat",
            RenderObjectClass::Model,
            Vec3::new(2.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(heat),
    );

    let ghost_id = scene.add_render_object(RenderObject {
        name: "ghost".to_string(),
        bounding_sphere: BoundingSphere::new(Vec3::new(3.0, 0.0, 0.0), 2.0),
        drawable_info: Some(game_engine_device::w3d::scene::DrawableInfo::new(None)),
        ..RenderObject::default()
    });

    let mut rinfo = RenderInfo::default();
    scene.render(&mut rinfo);

    assert!(scene.render_events().iter().any(|event| {
        *event == RenderEvent::Object(shrouded_id, RenderPassKind::Shroud, LightEnvKind::Default)
    }));
    assert!(scene.render_events().iter().any(|event| {
        *event
            == RenderEvent::Object(
                heat_id,
                RenderPassKind::HeatVisionOnly,
                LightEnvKind::Infantry,
            )
    }));
    assert!(scene.render_events().iter().any(|event| {
        *event == RenderEvent::Object(ghost_id, RenderPassKind::Fogged, LightEnvKind::Fogged)
    }));

    scene.clear_render_events();
    scene.set_custom_pass_mode(CustomScenePassMode::AlphaMask);
    scene.render(&mut rinfo);
    assert!(scene
        .render_events()
        .iter()
        .any(|event| matches!(event, RenderEvent::Object(_, RenderPassKind::Mask, _))));
}

#[test]
fn occluded_player_stencil_mask_uses_cpp_color_index_layout() {
    let mut scene = W3DScene::default();
    scene.add_render_object(
        object("building", RenderObjectClass::Model, Vec3::ZERO, 5.0)
            .with_drawable(drawable(1, KINDOF_STRUCTURE)),
    );

    let mut score = drawable(2, KINDOF_SCORE);
    score.controlling_player_index = 2;
    score.player_color = 0xff40_8040;
    scene.add_render_object(
        object(
            "score",
            RenderObjectClass::Model,
            Vec3::new(10.0, 0.0, 0.0),
            2.0,
        )
        .with_drawable(score),
    );

    scene.render(&mut RenderInfo::default());

    assert!(scene.render_events().iter().any(|event| {
        *event
            == RenderEvent::OccludedPlayerColor {
                player_index: 2,
                color_index: 8,
                stencil_ref: (8 << 3) | 0x80,
                color: 0xff20_4020,
            }
    }));
    assert_eq!(scene.stencil_shadow_mask(), ((8 << 3) | 0x80) as i32);
}

#[test]
fn reflection_visibility_uses_draws_in_mirror_and_skips_frame_update() {
    let mut scene = W3DScene::default();
    let mut mirror_hidden = drawable(1, 0);
    mirror_hidden.draws_in_mirror = false;
    let id = scene.add_render_object(
        object("no-mirror", RenderObjectClass::Model, Vec3::ZERO, 2.0).with_drawable(mirror_hidden),
    );

    scene.set_backface_culling_inverted(true);
    scene.render(&mut RenderInfo::default());

    assert!(!scene.get_render_object(id).unwrap().visible);
    assert_eq!(scene.get_render_object(id).unwrap().frame_update_count, 0);
}

#[test]
fn ray_cast_clips_to_nearest_visible_collision() {
    let mut scene = W3DScene::default();
    let far = scene.add_render_object(object(
        "far",
        RenderObjectClass::Model,
        Vec3::new(10.0, 0.0, 0.0),
        1.0,
    ));
    let near = scene.add_render_object(object(
        "near",
        RenderObjectClass::Model,
        Vec3::new(4.0, 0.0, 0.0),
        1.0,
    ));
    scene.get_render_object_mut(far).unwrap().collision_type = 0b01;
    scene.get_render_object_mut(near).unwrap().collision_type = 0b01;

    let hit = scene
        .cast_ray(Ray::new(Vec3::ZERO, Vec3::X, 100.0), false, 0b01)
        .unwrap();

    assert_eq!(hit.object_id, near);
    assert!((hit.distance - 3.0).abs() < 0.001);
    assert!((hit.clipped_end.x - 3.0).abs() < 0.001);
}

#[test]
fn fixed_light_environments_scale_fog_and_infantry_lights() {
    let mut scene = W3DScene::new(SceneConfig {
        fog_alpha: 0.25,
        clear_alpha: 1.0,
        infantry_light_scale: 2.0,
        ..SceneConfig::default()
    });
    scene.set_global_light(
        W3DLight::directional(Vec3::Z, Vec3::splat(0.75), Vec3::splat(0.2)),
        0,
    );

    scene.render(&mut RenderInfo::default());

    assert_eq!(
        scene.default_light_env().lights[0].diffuse,
        Vec3::splat(0.75)
    );
    assert_eq!(
        scene.fogged_light_env().lights[0].diffuse,
        Vec3::splat(0.1875)
    );
}

#[test]
fn overlay_scenes_keep_object_membership_order() {
    let mut overlay = W3D2DScene::new();
    overlay.add_object(3);
    overlay.add_object(7);
    overlay.remove_object(3);
    assert_eq!(overlay.name(), "RTS2DScene");
    assert_eq!(overlay.iter_objects().collect::<Vec<_>>(), vec![7]);

    let mut interface = W3DInterfaceScene::default();
    interface.add_object(2);
    interface.add_object(4);
    interface.remove_object(2);
    assert_eq!(interface.iter_objects().collect::<Vec<_>>(), vec![4]);
}
