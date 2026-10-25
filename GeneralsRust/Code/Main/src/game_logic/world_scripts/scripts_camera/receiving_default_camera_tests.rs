//! Receiving ScriptCameraState defaults and the real HAG/save kernels.
//! The source assertions bind these CPU operations to native wheel/ease/reset;
//! these tests do not construct a windowed engine or certify a rendered frame.
#![cfg(not(target_arch = "wasm32"))]
use super::*;
use crate::cnc_game_engine::live_view_height_clamp;
use crate::presentation_frame::PresentationFrame;

fn set_defaults(world: &mut GameLogic, pitch: f32, max_height: f32) {
    let mut list = ScriptList::new();
    list.append_script(script(
        "ReceivingCameraDefaults",
        vec![reals(
            ScriptActionType::CameraSetDefault,
            &[pitch, 1.25, max_height],
        )],
        true,
    ));
    let hooks = MissionScriptHooks::new();
    // run returns the exact interpreter after its synchronous owner loan.
    // This fixture has no detached continuation: retain it for frame/save.
    let interpreter = run(world, engine(hooks.clone(), list));
    world.install_owned_script_engine(interpreter);
    assert!(hooks.drain_camera_set_default_requests().is_empty());
}

#[test]
fn authored_defaults_reach_hag_before_the_next_completed_presentation() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_defaults_reach_hag_before_the_next_completed_presentation",
        || {
            let mut owner = GameLogic::new();
            let stale = PresentationFrame::build_from_logic(&mut owner, 0);
            assert_eq!(stale.script_default_camera_max_height, 1.0);
            assert_eq!(stale.script_default_camera_pitch, 0.0);
            set_defaults(&mut owner, 0.375, 1.5);
            assert_eq!(owner.script_default_camera_max_height(), 1.5);
            assert_eq!(owner.script_default_camera_pitch(), 0.375);
            assert_eq!(owner.session_view.scripted.script_default_camera_angle, 0.0);
            // CPP W3DView1896–1898: max=600*1.5=900, floored to min100.
            // Both live native clamp consumers use this same production helper.
            let (min, max) =
                live_view_height_clamp(100.0, 600.0, owner.script_default_camera_max_height());
            assert_eq!((min, max), (100.0, 900.0));
            let height = &mut owner.session_view.camera.native_camera;
            height.initialize_map(40.0, 300.0, 37.5, 0.0);
            height.set_height(10_000.0, min, max);
            assert_eq!(height.desired(), 900.0);
            // CPP setZoomToDefault: (terrain40+max900)/(ground40+base300).
            assert_eq!(height.set_zoom_to_default(40.0, max), 940.0 / 340.0);
            assert_eq!(height.desired(), 900.0);
            // The earlier completed output remains immutable and hostile to
            // this operation: sampling it would incorrectly clamp at600.
            assert_eq!(stale.script_default_camera_max_height, 1.0);
            assert_eq!(stale.script_default_camera_pitch, 0.0);
        },
    );
}

#[test]
fn independent_owner_defaults_reset_to_one_without_rebinding_the_peer() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "independent_owner_defaults_reset_to_one_without_rebinding_the_peer",
        || {
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            // This domain has no object IDs: each operation uses its session,
            // even with identical frame numbers and interleaved script actions.
            assert_eq!(owner.frame, peer.frame);
            set_defaults(&mut owner, -0.25, 2.0);
            set_defaults(&mut peer, 0.5, 0.25);
            let peer_before = peer.capture_camera_continuation().unwrap();
            assert_eq!(
                live_view_height_clamp(100.0, 600.0, owner.script_default_camera_max_height()),
                (100.0, 1200.0)
            );
            assert_eq!(
                live_view_height_clamp(100.0, 600.0, peer.script_default_camera_max_height()),
                (100.0, 150.0)
            );
            owner.reset();
            assert_eq!(owner.script_default_camera_max_height(), 1.0);
            assert_eq!(owner.script_default_camera_pitch(), 0.0);
            assert_eq!(
                live_view_height_clamp(100.0, 600.0, owner.script_default_camera_max_height()),
                (100.0, 600.0)
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            // Source max-height floor is retained, including negative scales.
            set_defaults(&mut owner, 0.125, -2.0);
            assert_eq!(
                live_view_height_clamp(100.0, 600.0, owner.script_default_camera_max_height()),
                (100.0, 100.0)
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
        },
    );
}

#[test]
fn receiving_world_save_admits_defaults_and_failed_decode_is_inert() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "receiving_world_save_admits_defaults_and_failed_decode_is_inert",
        || {
            let mut source = GameLogic::new();
            set_defaults(&mut source, 0.625, 1.75);
            source
                .session_view
                .camera
                .native_camera
                .set_height(420.0, 100.0, 1050.0);
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let snapshot: crate::save_load::WorldSnapshot =
                bincode_legacy::deserialize(&bytes).unwrap();
            let mut receiving = GameLogic::new();
            set_defaults(&mut receiving, -0.5, 0.25);
            let stale = PresentationFrame::build_from_logic(&mut receiving, 0);
            let mut peer = GameLogic::new();
            set_defaults(&mut peer, -0.25, 3.0);
            let peer_before = peer.capture_camera_continuation().unwrap();
            builder
                .restore_from_snapshot(&snapshot, &mut receiving)
                .unwrap();
            assert_eq!(receiving.script_default_camera_pitch(), 0.625);
            assert_eq!(receiving.script_default_camera_max_height(), 1.75);
            assert_eq!(receiving.session_view.camera.native_camera.desired(), 420.0);
            assert_eq!(
                live_view_height_clamp(100.0, 600.0, receiving.script_default_camera_max_height()),
                (100.0, 1050.0)
            );
            assert_eq!(stale.script_default_camera_pitch, -0.5);
            assert_eq!(stale.script_default_camera_max_height, 0.25);
            let before = receiving.capture_camera_continuation().unwrap();
            let mut malformed = before.clone();
            malformed.truncate(malformed.len() / 2);
            assert!(GameLogic::decode_camera_continuation(&malformed).is_err());
            assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
            source.reset();
            peer.reset();
            assert_eq!(receiving.capture_camera_continuation().unwrap(), before);
            assert_ne!(peer.capture_camera_continuation().unwrap(), peer_before);
        },
    );
}
