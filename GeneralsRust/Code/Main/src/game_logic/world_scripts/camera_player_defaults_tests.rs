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
    world.initialize_receiving_named_cache_for_fixture();
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
    camera
        .rotation_registers
        .begin_scalar(camera.rotation_registers.scalar_start().unwrap_or(0.), 2.);
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
    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        // This recipient owns no native Drawable; no mixer admission occurs.
        let _ = (change, audio);
        Ok(false)
    }

    #[cfg(feature = "game_client")]
    fn retire_physical_object_drawable(
        &mut self,
        _: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Err("camera-only fixture cannot physically retire native Drawables".into())
    }
    #[cfg(feature = "game_client")]
    fn host_epoch(&self) -> u64 {
        1
    }
    #[cfg(feature = "game_client")]
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        Err("camera-only fixture cannot construct native Drawables".into())
    }

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
            script_events: None,
            partition: None,
            audio: None,
            terrain: None,
            ui: None,
            view: Some(&mut view),
        };
        world.mmb_angle_pitch_defaults(&mut effects).unwrap();
        assert!(world.session_view.scripted.motion.active);
        assert!(world.session_view.scripted.path_arrived);
        assert_eq!(
            world.session_view.camera.rotation_registers.scalar_target(),
            Some(2.)
        );
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
            !camera.rotation_registers.active()
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
                script_events: None,
                partition: None,
                audio: None,
                terrain: None,
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
    let rules = world.session_view.camera.settings.current();
    let floor = world.session_view.camera.settings.bounds().0;
    assert!(floor > 0. && rules.max_camera_height > 0.);
    world
        .apply_script_camera_default(
            crate::game_logic::mission_scripts::CameraSetDefaultRequest {
                pitch: 0.,
                angle: 0.,
                max_height: floor / (rules.max_camera_height * 2.),
            },
        )
        .unwrap();
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
            script_events: None,
            partition: None,
            audio: None,
            terrain: None,
            ui: None,
            view: Some(&mut view),
        })
        .unwrap();
    assert_eq!(world.session_view.camera.native_camera.desired(), floor);
    let saved = world.capture_camera_continuation().unwrap();
    let mut restored = owner();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
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

fn camera_rules(min: f32, max: f32) -> game_engine::common::ini::ini_game_data::CameraContentRules {
    game_engine::common::ini::ini_game_data::CameraContentRules {
        min_camera_height: min,
        max_camera_height: max,
        ..Default::default()
    }
}
#[test]
fn actual_script_default_and_logic_v5_restore_preserve_captures_not_current_inputs() {
    use crate::game_logic::mission_scripts::CameraSetDefaultRequest;
    let mut world = crate::game_logic::GameLogic::new();
    world
        .admit_camera_application_rules(camera_rules(100., 300.))
        .unwrap();
    world
        .admit_camera_map_rules(camera_rules(200., 600.))
        .unwrap();
    world
        .apply_script_camera_default(CameraSetDefaultRequest {
            pitch: 37.,
            angle: 12.,
            max_height: 0.75,
        })
        .unwrap();
    assert_eq!(world.session_view.camera.settings.bounds(), (100., 450.));
    let saved = world.capture_camera_continuation().unwrap();
    let mut peer = crate::game_logic::GameLogic::new();
    peer.admit_camera_application_rules(camera_rules(20., 50.))
        .unwrap();
    let before = peer.capture_camera_continuation().unwrap();
    let mut receiver = crate::game_logic::GameLogic::new();
    receiver
        .admit_camera_continuation(
            crate::game_logic::GameLogic::decode_camera_continuation(&saved).unwrap(),
        )
        .unwrap();
    assert_eq!(
        receiver.session_view.camera.settings,
        world.session_view.camera.settings
    );
    assert_eq!(
        receiver.session_view.scripted.script_default_camera_pitch,
        37.
    );
    assert_eq!(before, peer.capture_camera_continuation().unwrap());
    let mut malformed = saved.clone();
    assert_eq!(&saved[4..8], &8u32.to_le_bytes());
    let codec = crate::save_load::camera_codec::CameraCodec::<{ 4 * 1024 * 1024 }>;
    // V5's final settings field precedes the appended V6-V8 owner payloads.
    // Identify it explicitly so a new suffix cannot redirect this corruption
    // into raw rotation words, which can legitimately contain any bit pattern.
    let suffix_len = codec
        .serialize(&world.session_view.camera.clip_policy)
        .unwrap()
        .len()
        + codec
            .serialize(&world.session_view.camera.shake_policy)
            .unwrap()
            .len()
        + codec
            .serialize(&world.session_view.camera.rotation)
            .unwrap()
            .len()
        + codec
            .serialize(&world.session_view.camera.rotation_registers.capture())
            .unwrap()
            .len();
    let settings_end = malformed.len() - suffix_len;
    assert_eq!(
        &saved[settings_end - 4..settings_end],
        &world.session_view.camera.settings.maximum().to_le_bytes()
    );
    malformed[settings_end - 4..settings_end].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(crate::game_logic::GameLogic::decode_camera_continuation(&malformed).is_err());
    assert_eq!(before, peer.capture_camera_continuation().unwrap());
}

#[test]
fn repeated_logic_reset_retains_the_once_captured_client_default() {
    let mut world = GameLogic::new();
    world
        .admit_camera_application_rules(camera_rules(100., 300.))
        .unwrap();
    world
        .admit_camera_map_rules(camera_rules(200., 600.))
        .unwrap();
    // Exact native Client/UI reset admission, before override removal.
    world.session_view.camera.settings = world.session_view.camera.settings.reset_seed();
    world.reset();
    world.reset();
    assert_eq!(world.session_view.camera.settings.bounds(), (100., 600.));
    assert_eq!(
        world.session_view.camera.settings.current(),
        camera_rules(100., 300.)
    );
}

#[test]
fn overflowing_live_default_rejects_before_any_owner_or_peer_mutation() {
    let mut world = owner();
    let peer = owner();
    world
        .admit_camera_application_rules(camera_rules(100., 300.))
        .unwrap();
    let before = world.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let error = world
        .apply_script_camera_default(
            crate::game_logic::mission_scripts::CameraSetDefaultRequest {
                pitch: 1.25,
                angle: 77.,
                max_height: f32::MAX,
            },
        )
        .unwrap_err();
    assert!(error.contains("nonfinite effective camera default maximum"));
    assert_eq!(before, world.capture_camera_continuation().unwrap());
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
}
