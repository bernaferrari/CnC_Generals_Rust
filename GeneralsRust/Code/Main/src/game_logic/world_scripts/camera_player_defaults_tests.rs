//! LookAtXlat short-click defaults: actual receiving setter phases, no window/GPU.
use super::*;
use crate::game_logic::mission_scripts::{
    CameraMoveToRequest, CameraResetRequest, CameraSlaveModeRequest,
};
use crate::game_logic::{NativeViewEffects, Object, ObjectId, Team, ThingTemplate};
use glam::Mat4;
fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
    map.heightmap = (0..4096).map(|i| ((i % 64) / 4) as u8).collect();
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.session_view.target = Vec3::new(180., 0., 200.);
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object MmbOwner\n Draw = W3DModelDraw DrawTag\n DefaultConditionState\n Model = CameraModel\n End\n End\nEnd\n", "mmb.ini").unwrap();
    let definition = parser.get_definition("MmbOwner").unwrap();
    let mut template = ThingTemplate::new("MmbOwner");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    let mut object = Object::new(template, ObjectId(42), Team::USA);
    object.name = "MmbSource".into();
    world.add_object(object);
    world.session_view.scripted.camera_follow_target = Some(ObjectId(42));
    world.session_view.scripted.camera_tether_play = Some(30.);
    world.session_view.scripted.script_default_camera_pitch = 0.37;
    world.session_view.scripted.script_default_camera_angle = 0.91;
    world.start_camera_move_to_oriented(
        CameraMoveToRequest {
            position: Vec3::new(250., 0., 280.),
            seconds: 2.,
            camera_stutter_seconds: 0.,
            ease_in_seconds: 0.,
            ease_out_seconds: 0.,
        },
        false,
    );
    world.session_view.scripted.motion.active = true;
    world.session_view.scripted.path_arrived = true;
    let camera = &mut world.session_view.camera;
    camera.camera_zoom = 0.7;
    camera.camera_yaw_radians = 0.6;
    camera.camera_pitch_radians = 0.2;
    camera.camera_fx_pitch = 0.4;
    camera.camera_yaw_target = Some(2.);
    camera.camera_zoom_target = Some(2.);
    camera.camera_pitch_target = Some(2.);
    camera.camera_slave_mode = Some(CameraSlaveModeRequest {
        thing_template_name: "MmbSource".into(),
        bone_name: "CAMERA".into(),
    });
    camera.rebuild_projection();
    camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 1000.), 20.)
        .unwrap();
    camera
        .script_camera_shakers
        .push(crate::presentation_frame::camera::ScriptCameraShaker::new(
            Vec3::ZERO,
            10000.,
            30.,
            0.,
        ));
    world.session_view.commit_orbit_pose();
    world
}
struct Effects {
    calls: usize,
    position: Vec3,
}
impl NativeViewEffects for Effects {
    fn local_player_id(&self) -> u32 {
        0
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        assert_eq!(source.identity.object_id(), 42);
        assert_eq!(bone, "CAMERA");
        self.calls += 1;
        SlaveCameraSample::Pose(Mat4::from_translation(self.position))
    }
}
#[test]
fn short_mmb_defaults_keep_first_setter_motion_then_cancel_second_and_sample_post_slave_position() {
    for invalid in [false, true] {
        let mut world = owner();
        let peer = owner();
        let peer_before = peer.capture_camera_continuation().unwrap();
        if invalid {
            world.session_view.camera.constraints.invalidate();
        }
        let hag_before = serde_json::to_value(
            world
                .session_view
                .camera
                .native_camera
                .capture_continuation(),
        )
        .unwrap();
        let mut view = Effects {
            calls: 0,
            position: Vec3::new(480., 70., 300.),
        };
        let mut effects = ScriptPresentationEffects {
            ui: None,
            view: Some(&mut view),
        };
        world.mmb_angle_pitch_defaults(&mut effects).unwrap();
        assert!(world.session_view.scripted.motion.active);
        assert!(world.session_view.scripted.path_arrived);
        assert_eq!(world.session_view.camera.camera_yaw_target, Some(2.));
        assert_eq!(world.session_view.camera.camera_zoom_target, Some(2.));
        assert_eq!(world.session_view.camera.camera_pitch_target, Some(2.));
        assert_eq!(world.session_view.camera.camera_yaw_radians, 0.);
        assert_eq!(world.session_view.camera.camera_pitch_radians, 0.37);
        assert_eq!(world.session_view.camera.camera_fx_pitch, 1.);
        assert_eq!(
            serde_json::to_value(
                world
                    .session_view
                    .camera
                    .native_camera
                    .capture_continuation()
            )
            .unwrap(),
            hag_before
        );
        assert_eq!(world.session_view.target, Vec3::new(480., 70., 300.));
        let terrain_owner = world.world_services.terrain();
        let terrain = terrain_owner.read().unwrap();
        let height = crate::presentation_frame::camera::height::height_around(
            world.session_view.target,
            |x, z| Some(terrain.get_ground_height(x, z, None)),
        )
        .unwrap();
        let old_height = crate::presentation_frame::camera::height::height_around(
            Vec3::new(180., 0., 200.),
            |x, z| Some(terrain.get_ground_height(x, z, None)),
        )
        .unwrap();
        assert_ne!(
            height, old_height,
            "fixture terrain distinguishes first-build slave position"
        );
        drop(terrain);
        let max_hag = world.receiving_camera_max_hag();
        let expected_zoom = world
            .session_view
            .camera
            .native_camera
            .default_zoom(height, max_hag);
        world.mmb_zoom_default(&mut effects).unwrap();
        drop(effects);
        assert_eq!(view.calls, if invalid { 4 } else { 3 });
        assert!(!world.session_view.scripted.motion.active);
        assert!(!world.session_view.scripted.path_arrived);
        let camera = &world.session_view.camera;
        assert_eq!(camera.camera_zoom, expected_zoom);
        assert_eq!(camera.native_camera.desired(), max_hag);
        let after = serde_json::to_value(camera.native_camera.capture_continuation()).unwrap();
        for key in ["offset", "ground", "enabled", "limited"] {
            assert_eq!(after[key], hag_before[key]);
        }
        assert!(
            camera.camera_yaw_target.is_none()
                && camera.camera_pitch_target.is_none()
                && camera.camera_zoom_target.is_none()
        );
        assert!(camera.camera_slave_mode.is_some());
        assert_eq!(
            world.session_view.scripted.camera_follow_target,
            Some(ObjectId(42))
        );
        assert_eq!(world.session_view.scripted.camera_tether_play, Some(30.));
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
        let mut combined = owner();
        if invalid {
            combined.session_view.camera.constraints.invalidate();
        }
        let mut second_view = Effects {
            calls: 0,
            position: view.position,
        };
        combined
            .admit_short_mmb_defaults(&mut ScriptPresentationEffects {
                ui: None,
                view: Some(&mut second_view),
            })
            .unwrap();
        assert_eq!(
            combined.capture_camera_continuation().unwrap(),
            world.capture_camera_continuation().unwrap()
        );
    }
}
#[test]
fn reset_and_mmb_default_share_minimum_hag_floor_without_mutating_peer() {
    let mut world = owner();
    let peer = owner();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let data = game_engine::common::global_data::read();
    assert!(data.min_camera_height > 0. && data.max_camera_height > 0.);
    let floor = data.min_camera_height;
    world.session_view.scripted.script_default_camera_max_height =
        floor / (data.max_camera_height * 2.);
    drop(data);
    assert_eq!(world.receiving_camera_max_hag(), floor);
    let target = world.session_view.target;
    let zoom = world.owned_script_camera_max_zoom_at(target).unwrap();
    world.admit_camera_reset_motion(CameraResetRequest {
        position: target,
        duration_seconds: 0.001,
        ease_in_seconds: 0.,
        ease_out_seconds: 0.,
    });
    assert_eq!(world.session_view.camera.camera_zoom_target, Some(zoom));
    let mut view = Effects {
        calls: 0,
        position: target,
    };
    world
        .admit_short_mmb_defaults(&mut ScriptPresentationEffects {
            ui: None,
            view: Some(&mut view),
        })
        .unwrap();
    assert_eq!(world.session_view.camera.native_camera.desired(), floor);
    let saved = world.capture_camera_continuation().unwrap();
    let mut restored = owner();
    restored.admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap());
    assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
#[test]
fn actual_mmb_event_routes_defaults_while_hotkey_retains_one_millisecond_motion() {
    let mouse = include_str!("../../cnc_game_engine/mouse/camera.rs");
    let event = mouse
        .split("fn end_mmb_lookat_rotate")
        .nth(1)
        .unwrap()
        .split("fn reset_camera_pose_in_place")
        .next()
        .unwrap();
    assert!(event.contains("lookat_mmb_is_short_click"));
    assert!(event.contains("restore_short_mmb_camera_defaults"));
    assert!(!event.contains("self.reset_camera_pose_in_place()"));
    let hotkey = include_str!("../../cnc_game_engine/hotkeys.rs");
    assert!(hotkey.contains("self.reset_camera_pose_in_place()"));
}

#[test]
fn current_terrain_boundary_drives_recalculation_not_cached_host_world_bounds() {
    let mut world = owner();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 4096];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(51, 47)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.session_view.camera.camera_slave_mode = None;
    world.session_view.camera.constraints.invalidate();
    let terrain = world.world_services.terrain().read().unwrap();
    assert_eq!(terrain.get_active_boundary(), 0);
    let extent = terrain.get_extent();
    assert_eq!((extent.hi.x, extent.hi.y), (510., 470.));
    let bounds = GameLogic::camera_bounds_for_terrain(&terrain);
    assert_ne!(bounds, world.world_bounds());
    drop(terrain);
    world
        .commit_native_camera_with_effects(&mut ScriptPresentationEffects::default())
        .unwrap();
    let cache = world.session_view.camera.constraints.bounds(None).unwrap();
    // Source uses the same inset on each opposite edge. Their sums identify
    // the actual current boundary independently of the provisional ray inset.
    assert!((cache.x_min + cache.x_max - 510.).abs() < 0.001);
    assert!((cache.z_min + cache.z_max - 470.).abs() < 0.001);
}
