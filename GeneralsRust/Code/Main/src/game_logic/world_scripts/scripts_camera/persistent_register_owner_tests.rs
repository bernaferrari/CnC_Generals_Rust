//! Actual authored receiving tracking and persistent source-register lifetime.
//! Original defined component inputs: 503/tracking-cache-cpp/case-contract.json.
use super::*;
use crate::presentation_frame::camera::movement_rules::CameraMovementRules;

const A: Vec3 = Vec3::new(100., 15., 50.);
const B: Vec3 = Vec3::new(-90., 11., 5.);
fn fixture() -> (GameLogic, ObjectId, ObjectId) {
    let (mut owner, a, _) = world(110.);
    owner.session_view.target = Vec3::ZERO;
    owner.set_fixture_object_position(a, A);
    let b = owner
        .create_object_for_player("CameraOwnerUnit", 0, B)
        .unwrap();
    owner.host_object_mut(b).unwrap().name = "ReplacementScout".into();
    assert_eq!(
        (a.0, b.0),
        (1, 3),
        "pin actual factory identities, never inject IDs"
    );
    (owner, a, b)
}
fn track(
    owner: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    name: &str,
    duration: f32,
    hold: f32,
) {
    let mut list = ScriptList::new();
    list.append_script(script(
        "PersistentTracking",
        vec![action(
            ScriptActionType::CameraLookTowardObject,
            vec![
                Parameter::with_string(ParameterType::Unit, name.into()),
                Parameter::with_real(ParameterType::Real, duration),
                Parameter::with_real(ParameterType::Real, hold),
                Parameter::with_real(ParameterType::Real, 0.),
                Parameter::with_real(ParameterType::Real, 0.),
            ],
        )],
        true,
    ));
    owner.frame += 1;
    let interpreter = run(owner, engine(Arc::clone(foreign), list));
    owner.install_owned_script_engine(interpreter);
}
fn actions(
    owner: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    values: Vec<Box<ScriptAction>>,
) {
    let mut list = ScriptList::new();
    list.append_script(script("PersistentScalar", values, true));
    owner.frame += 1;
    let interpreter = run(owner, engine(Arc::clone(foreign), list));
    owner.install_owned_script_engine(interpreter);
}
fn offer(owner: &mut GameLogic) {
    owner.headless_camera_view_offer(33).unwrap();
}
fn disable(owner: &mut GameLogic, value: bool) {
    owner.admit_camera_application_movement(CameraMovementRules {
        disable_camera_movement: value,
        ..Default::default()
    });
}
fn remove(owner: &mut GameLogic, id: ObjectId) {
    owner.destroy_object(id);
    assert!(owner.objects.contains_key(&id));
    owner.process_destroy_list();
    assert!(!owner.objects.contains_key(&id));
}
fn row(rows: &mut Vec<String>, scenario: &str, phase: &str, owner: &GameLogic) {
    let camera = &owner.session_view.camera;
    let registers = &camera.rotation_registers;
    let words = registers
        .initialized_source_words()
        .expect("actual enabled A offer initialized the whole source payload");
    rows.push(format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        scenario,
        phase,
        camera.camera_yaw_frame,
        camera.rotation.turn_frames,
        registers.hold_frames(),
        u8::from(registers.is_tracking()),
        u8::from(registers.active()),
        camera.rotation.time.start,
        camera.rotation.time.end,
        owner.visual_speed_multiplier as i32,
        camera.camera_yaw_radians.to_bits(),
        words[0],
        words[1],
        words[2],
        words[3]
    ));
}
fn receiving(saved: &[u8]) -> GameLogic {
    let (mut receiving, _, b) = fixture();
    remove(&mut receiving, b);
    receiving
        .admit_camera_continuation(GameLogic::decode_camera_continuation(saved).unwrap())
        .unwrap();
    let interpreter = engine(receiving.mission_scripts.clone(), ScriptList::new());
    receiving.install_owned_script_engine(interpreter);
    receiving
}
#[test]
fn actual_receiving_persistent_register_rows_for_original_cpp_comparison() {
    let mut rows = Vec::new();
    for scenario in [
        "tracking_replacement",
        "inactive_replacement",
        "disabled_pending",
        "disabled_expiry",
        "cancel_set_angle",
        "same_view_reset",
        "scalar_overlay",
        "coincident_prefix",
        "scalar_freeze",
        "tracking_freeze",
    ] {
        let (mut owner, _, b) = fixture();
        let (mut peer, peer_a, _) = fixture();
        peer.set_fixture_object_position(peer_a, Vec3::new(210., 25., 230.));
        track(&mut peer, &owner.mission_scripts, "FocusScout", 0.132, 0.);
        offer(&mut peer);
        let peer_before = peer.capture_camera_continuation().unwrap();
        track(
            &mut owner,
            &peer.mission_scripts,
            "FocusScout",
            if scenario == "inactive_replacement" {
                0.033
            } else {
                0.132
            },
            0.,
        );
        assert_eq!(
            owner.session_view.camera.rotation_registers.cached_target(),
            None,
            "admission is not an observation"
        );
        offer(&mut owner);
        assert_eq!(
            owner.session_view.camera.rotation_registers.cached_target(),
            Some(A)
        );
        match scenario {
            "tracking_replacement" => row(&mut rows, scenario, "observed_A", &owner),
            "inactive_replacement" => row(&mut rows, scenario, "A_finished", &owner),
            "cancel_set_angle" => {
                // The actual Core cancellation phase of the native setter;
                // finite 0.75 is the native pose input, not a cache initializer.
                owner.cancel_scripted_camera_from_player_set();
                owner.session_view.camera.camera_yaw_radians = 0.75;
                row(&mut rows, scenario, "cancelled", &owner);
            }
            "same_view_reset" => {
                // Same production rotation phase called by GameLogic.reset.
                // This component comparison does not claim whole View reset,
                // terrain, path or native transform equivalence.
                owner.session_view.camera.cancel_scalar_movements();
                row(&mut rows, scenario, "reset", &owner);
            }
            "scalar_overlay" | "scalar_freeze" => {
                actions(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![reals(
                        ScriptActionType::RotateCamera,
                        &[0.25, 0.132, 0., 0.],
                    )],
                );
                if scenario == "scalar_freeze" {
                    actions(
                        &mut owner,
                        &peer.mission_scripts,
                        vec![Box::new(ScriptAction::new(
                            ScriptActionType::CameraModFreezeAngle,
                        ))],
                    );
                }
                row(
                    &mut rows,
                    scenario,
                    if scenario == "scalar_overlay" {
                        "scalar_admitted"
                    } else {
                        "scalar_frozen"
                    },
                    &owner,
                );
            }
            "coincident_prefix" => {
                let target = owner.session_view.target;
                assert!(!owner.session_view.install_look_toward(
                    target,
                    target,
                    0.132,
                    0.,
                    0.,
                    false,
                    owner.visual_speed_multiplier
                ));
                row(&mut rows, scenario, "scalar_prefix_return", &owner);
            }
            "tracking_freeze" => {
                actions(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![Box::new(ScriptAction::new(
                        ScriptActionType::CameraModFreezeAngle,
                    ))],
                );
                row(&mut rows, scenario, "tracking_id_invalidated", &owner);
            }
            _ => {}
        }
        if scenario == "disabled_expiry" {
            track(
                &mut owner,
                &peer.mission_scripts,
                "ReplacementScout",
                0.033,
                0.,
            );
            disable(&mut owner, true);
            offer(&mut owner);
            disable(&mut owner, false);
            row(&mut rows, scenario, "disabled_B_finished", &owner);
        }
        track(
            &mut owner,
            &peer.mission_scripts,
            "ReplacementScout",
            0.132,
            0.066,
        );
        row(
            &mut rows,
            scenario,
            if scenario == "disabled_expiry" {
                "readmitted_B"
            } else {
                "admitted_B"
            },
            &owner,
        );
        if scenario == "disabled_pending" {
            disable(&mut owner, true);
            offer(&mut owner);
            disable(&mut owner, false);
            assert_eq!(
                owner.session_view.camera.rotation_registers.cached_target(),
                Some(A)
            );
            row(&mut rows, scenario, "disabled_B_offer", &owner);
        }
        remove(&mut owner, b);
        let saved = owner.capture_camera_continuation().unwrap();
        let mut restored = receiving(&saved);
        assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
        offer(&mut owner);
        offer(&mut restored);
        assert_eq!(
            restored.capture_camera_continuation().unwrap(),
            owner.capture_camera_continuation().unwrap()
        );
        row(&mut rows, scenario, "first_missing_B_offer", &owner);
        for _ in 0..2 {
            offer(&mut owner);
            offer(&mut restored);
            assert_eq!(
                restored.capture_camera_continuation().unwrap(),
                owner.capture_camera_continuation().unwrap()
            );
        }
        assert_eq!(
            peer.capture_camera_continuation().unwrap(),
            peer_before,
            "same-ID foreign camera cannot supply payload"
        );
    }
    assert_eq!(rows.len(), 30);
    println!("PERSISTENT_REGISTER_CPP_ROWS_BEGIN");
    for line in rows {
        println!("{line}");
    }
    println!("PERSISTENT_REGISTER_CPP_ROWS_END");
}

#[test]
fn unavailable_tracking_cache_stays_unavailable_through_v8_restore_and_missing_expiry() {
    let (mut source, a, _) = fixture();
    let (peer, _, _) = fixture();
    source.visual_speed_multiplier = 5.;
    track(&mut source, &peer.mission_scripts, "FocusScout", 0.132, 0.);
    actions(
        &mut source,
        &peer.mission_scripts,
        vec![ints(
            ScriptActionType::CameraModSetFinalSpeedMultiplier,
            &[2],
        )],
    );
    remove(&mut source, a);
    assert_eq!(
        source
            .session_view
            .camera
            .rotation_registers
            .cached_target(),
        None
    );
    let saved = source.capture_camera_continuation().unwrap();
    let (mut restored, restored_a, _) = fixture();
    remove(&mut restored, restored_a);
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    restored
        .install_owned_script_engine(engine(restored.mission_scripts.clone(), ScriptList::new()));
    assert_eq!(
        restored
            .session_view
            .camera
            .rotation_registers
            .cached_target(),
        None,
        "detached V7 zero projection must never initialize V8 cache"
    );
    for frame in 1..=4 {
        offer(&mut source);
        offer(&mut restored);
        assert_eq!(source.session_view.camera.camera_yaw_frame, frame);
        assert_eq!(
            source
                .session_view
                .camera
                .rotation_registers
                .cached_target(),
            None
        );
        assert_eq!(source.session_view.camera.camera_yaw_radians, 0.);
        assert_eq!(
            source.visual_speed_multiplier, 5.,
            "safe unavailable policy does not fabricate angle/time samples"
        );
        assert_eq!(
            source.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
    }
    assert!(!source.session_view.camera.rotation_registers.active());
}

#[test]
fn rejected_and_disabled_offers_do_not_observe_first_tracking_target() {
    let (mut owner, _, _) = fixture();
    let (peer, _, _) = fixture();
    track(&mut owner, &peer.mission_scripts, "FocusScout", 0.132, 0.);
    owner.is_paused = true;
    offer(&mut owner);
    owner.is_paused = false;
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 0);
    assert_eq!(
        owner.session_view.camera.rotation_registers.cached_target(),
        None
    );
    disable(&mut owner, true);
    offer(&mut owner);
    disable(&mut owner, false);
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 1);
    assert_eq!(
        owner.session_view.camera.rotation_registers.cached_target(),
        None
    );
    offer(&mut owner);
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 2);
    assert_eq!(
        owner.session_view.camera.rotation_registers.cached_target(),
        Some(A)
    );
}

#[path = "persistent_register_continuation_tests.rs"]
mod persistent_register_continuation_tests;
