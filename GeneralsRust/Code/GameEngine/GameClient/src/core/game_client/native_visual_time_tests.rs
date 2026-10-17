//! Receiving native prefix facts are observed at the original late UI boundary.
use super::*;

fn client(frame: u32) -> GameClient {
    let mut client = GameClient::new().unwrap();
    client.initialized = true;
    client.set_frame(frame);
    client
}
fn frames(frame: u32) -> NativeParticleFrames {
    NativeParticleFrames {
        logic_frame: frame,
        client_frame: frame,
    }
}
#[test]
fn native_visual_time_is_instance_scoped_and_repeated_frame_is_consumed_once() {
    let mut a = client(71);
    let mut b = client(71);
    assert_eq!(
        a.admit_visual_time(NativeVisualTime {
            frozen: false,
            visual_speed: 2.5
        })
        .unwrap(),
        (false, 2.5)
    );
    assert_eq!(
        b.admit_visual_time(NativeVisualTime {
            frozen: true,
            visual_speed: 0.5
        })
        .unwrap(),
        (true, 0.5)
    );
    assert_eq!(
        a.admit_visual_time(NativeVisualTime {
            frozen: false,
            visual_speed: 3.
        })
        .unwrap(),
        (true, 3.)
    );
    a.set_frame(72);
    assert_eq!(
        a.admit_visual_time(NativeVisualTime {
            frozen: false,
            visual_speed: 3.
        })
        .unwrap(),
        (false, 3.)
    );
    b.set_frame(72);
    assert_eq!(
        b.admit_visual_time(NativeVisualTime {
            frozen: false,
            visual_speed: -1.
        })
        .unwrap(),
        (false, -1.)
    );
}
#[test]
fn invalid_visual_speed_does_not_consume_receiving_frame() {
    let mut client = client(93);
    let before = client.last_visual_time_frame;
    for speed in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            client
                .admit_visual_time(NativeVisualTime {
                    frozen: false,
                    visual_speed: speed
                })
                .is_err()
        );
        assert_eq!(client.last_visual_time_frame, before);
    }
    assert_eq!(
        client
            .admit_visual_time(NativeVisualTime {
                frozen: false,
                visual_speed: 1.
            })
            .unwrap(),
        (false, 1.)
    );
}
#[test]
fn actual_native_prefix_uses_supplied_facts_despite_foreign_core_observation() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "actual_native_prefix_uses_supplied_facts_despite_foreign_core_observation",
    ) {
        return;
    }
    get_shell().init().unwrap();
    let mut a = client(121);
    let mut b = client(121);
    // Restore the existing standalone observation on this thread; neither
    // receiving prefix may sample it while processing its explicit facts.
    struct LegacyObservation {
        view: Option<crate::display::view::View>,
        speed: i32,
    }
    impl Drop for LegacyObservation {
        fn drop(&mut self) {
            let view = self.view.take().unwrap();
            crate::display::view::with_tactical_view(|current| *current = view);
            crate::core::script_action_handler::set_script_visual_speed_multiplier(self.speed);
        }
    }
    let _legacy = LegacyObservation {
        view: Some(crate::display::view::with_tactical_view(|view| {
            std::mem::replace(view, crate::display::view::View::new())
        })),
        speed: get_script_visual_speed_multiplier(),
    };
    crate::core::script_action_handler::set_script_visual_speed_multiplier(77);
    crate::display::view::with_tactical_view(|view| {
        view.zoom_camera(2., 1000, 0., 0.);
        view.camera_mod_freeze_time();
    });
    let mut observed = 0;
    let first = a
        .begin_presentation_shell_with_owned_visual_time(0.033, frames(121), &[], || {
            observed += 1;
            NativeVisualTime {
                frozen: false,
                visual_speed: 2.,
            }
        })
        .unwrap();
    assert!(!first.frozen);
    assert_eq!(observed, 1);
    let peer = b
        .begin_presentation_shell_with_owned_visual_time(0.033, frames(121), &[], || {
            NativeVisualTime {
                frozen: true,
                visual_speed: 0.5,
            }
        })
        .unwrap();
    assert!(peer.frozen);
    let repeated = a
        .begin_presentation_shell_with_owned_visual_time(0.033, frames(121), &[], || {
            NativeVisualTime {
                frozen: false,
                visual_speed: 2.,
            }
        })
        .unwrap();
    assert!(repeated.frozen);
    a.set_frame(122);
    let next = a
        .begin_presentation_shell_with_owned_visual_time(0.033, frames(122), &[], || {
            NativeVisualTime {
                frozen: false,
                visual_speed: 2.,
            }
        })
        .unwrap();
    assert!(!next.frozen);
    assert_eq!(b.last_visual_time_frame, 121);
}

#[path = "tests/native_visual_time_delta.rs"]
mod delta_drawable;

#[test]
fn actual_prefix_delivers_receiving_scaled_delta_repeat_zero_and_peer_reset_isolation() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "actual_prefix_delivers_receiving_scaled_delta_repeat_zero_and_peer_reset_isolation",
    ) {
        return;
    }
    use delta_drawable::DeltaDrawable;
    get_shell().init().unwrap();
    let mut a = client(151);
    let mut b = client(151);
    fn install(client: &mut GameClient) -> DrawableId {
        let id = client
            .create_drawable_from_template(&ThingTemplate::new())
            .unwrap();
        client
            .drawables
            .drawable_map
            .insert(id, Box::new(DeltaDrawable::new(id)));
        id
    }
    fn deltas(client: &GameClient, id: DrawableId) -> Vec<f32> {
        client.drawables.drawable_map[&id]
            .as_any()
            .downcast_ref::<DeltaDrawable>()
            .unwrap()
            .deltas
            .clone()
    }
    let aid = install(&mut a);
    let bid = install(&mut b);
    for (client, speed) in [(&mut a, 2.5), (&mut b, 0.5)] {
        let _ = client
            .begin_presentation_shell_with_owned_visual_time(0.04, frames(151), &[], || {
                NativeVisualTime {
                    frozen: false,
                    visual_speed: speed,
                }
            })
            .unwrap();
    }
    assert_eq!(deltas(&a, aid), [0.04_f32 * 2.5]);
    assert_eq!(deltas(&b, bid), [0.02]);
    let _ = a
        .begin_presentation_shell_with_owned_visual_time(0.04, frames(151), &[], || {
            NativeVisualTime {
                frozen: false,
                visual_speed: 2.5,
            }
        })
        .unwrap();
    assert_eq!(deltas(&a, aid), [0.04_f32 * 2.5, 0.]);
    // Dropping/reconstructing a peer cannot reset the receiving frame marker.
    drop(b);
    let peer = client(151);
    assert_ne!(peer.last_visual_time_frame, a.last_visual_time_frame);
    a.set_frame(152);
    let _ = a
        .begin_presentation_shell_with_owned_visual_time(0.04, frames(152), &[], || {
            NativeVisualTime {
                frozen: false,
                visual_speed: 2.5,
            }
        })
        .unwrap();
    assert_eq!(deltas(&a, aid), [0.04_f32 * 2.5, 0., 0.04_f32 * 2.5]);
    assert_eq!(peer.last_visual_time_frame, u32::MAX);
}

#[test]
fn receiving_freeze_admission_matches_extracted_cpp_truth_repeat_and_unsigned_wrap_trace() {
    let corpus = include_str!("native_visual_time_tests/fixtures/client_freeze_cpp.csv");
    let mut client = GameClient::new().unwrap();
    let mut rows = 0;
    for line in corpus.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        assert_eq!(fields.len(), 9);
        let frame = fields[2].parse::<u32>().unwrap();
        let flag = |index: usize| match fields[index] {
            "0" => false,
            "1" => true,
            value => panic!("invalid oracle boolean {value}"),
        };
        let frozen = (flag(3) && !flag(4)) || flag(5) || flag(6) || flag(7);
        client.set_frame(frame);
        let (actual, speed) = client
            .admit_visual_time(NativeVisualTime {
                frozen,
                visual_speed: 1.,
            })
            .unwrap();
        assert_eq!(
            actual,
            flag(8),
            "original CPP {} case{} frame{}",
            fields[0],
            fields[1],
            frame
        );
        assert_eq!(speed, 1.);
        assert_eq!(client.last_visual_time_frame, frame);
        rows += 1;
    }
    assert_eq!(
        rows, 75,
        "all original oracle offers must execute in one continuous trace"
    );
}

#[test]
fn native_shell_never_advances_foreign_core_impulse_but_standalone_effects_do() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "native_shell_never_advances_foreign_core_impulse_but_standalone_effects_do",
    ) {
        return;
    }
    get_shell().init().unwrap();
    use crate::display::view::{CameraShakeType, Point3};
    let mut rules = game_engine::common::global_data::read().clone();
    rules.shake_normal_intensity = 2.;
    rules.max_shake_intensity = 100.;
    crate::display::view::with_tactical_view(|view| {
        view.reset_camera_shake();
        view.set_position(&Point3::origin());
        view.shake_with_services(
            &Point3::origin(),
            CameraShakeType::Normal,
            &rules,
            &mut |_, _| 0.,
        );
        view.tick_impulse_shake();
    });
    let core = || {
        crate::display::view::with_tactical_view_ref(|view| {
            let offset = view.impulse_shake_offset();
            (view.camera_shake_intensity(), offset.x, offset.y)
        })
    };
    let before = core();
    assert!(before.0 > 0. && (before.1 != 0. || before.2 != 0.));
    let mut owner = client(161);
    let mut peer = client(161);
    for receiving in [&mut owner, &mut peer] {
        let prepared = receiving
            .begin_presentation_shell_with_owned_visual_time(1. / 30., frames(161), &[], || {
                NativeVisualTime {
                    frozen: false,
                    visual_speed: 1.,
                }
            })
            .unwrap();
        assert!(!prepared.frozen);
        assert_eq!(
            core(),
            before,
            "native prefix must not tick standalone camera"
        );
    }
    // Actual retained standalone effects entry still performs its original tick.
    owner.update_effects(1. / 30.).unwrap();
    assert!(core().0 < before.0);
}

#[test]
fn native_prefix_preserves_foreign_core_tracking_and_standalone_still_follows() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "native_prefix_preserves_foreign_core_tracking_and_standalone_still_follows",
    ) {
        return;
    }
    use crate::display::view::Point3;
    use crate::drawable::Drawable;
    get_shell().init().unwrap();
    let mut owner = client(181);
    let mut peer = client(181);
    let mut empty = client(181);
    let id = DrawableId(7301);
    let mut selected = BasicDrawable::new(id);
    selected.set_position(Vector3::new(125., 225., 0.));
    selected.set_selected(true);
    owner
        .drawables
        .admit_native_drawable(id, Box::new(selected));
    let mut peer_selected = BasicDrawable::new(id);
    peer_selected.set_position(Vector3::new(325., 425., 0.));
    peer_selected.set_selected(true);
    peer.drawables
        .admit_native_drawable(id, Box::new(peer_selected));
    crate::display::view::with_tactical_view(|view| view.set_position(&Point3::new(10., 20., 0.)));
    crate::helpers::TheInGameUI::set_camera_tracking_drawable(true);
    assert!(crate::helpers::TheInGameUI::is_camera_tracking_drawable());
    let position = || {
        crate::display::view::with_tactical_view_ref(|view| {
            let p = view.position();
            (p.x, p.y, p.z)
        })
    };
    let before = position();
    for receiver in [&mut owner, &mut peer, &mut empty] {
        receiver
            .begin_presentation_shell_with_owned_visual_time(1. / 30., frames(181), &[], || {
                NativeVisualTime {
                    frozen: false,
                    visual_speed: 1.,
                }
            })
            .unwrap();
        assert_eq!(
            position(),
            before,
            "native prefix must not follow foreign Core drawable selection"
        );
        assert!(
            crate::helpers::TheInGameUI::is_camera_tracking_drawable(),
            "native empty peer must not clear foreign tracking"
        );
    }
    owner.update_camera_tracking_drawable();
    assert_ne!(
        position(),
        before,
        "retained standalone adapter still follows installed selection"
    );
    assert!(crate::helpers::TheInGameUI::is_camera_tracking_drawable());
    let owner_position = position();
    peer.update_camera_tracking_drawable();
    assert_ne!(
        position(),
        owner_position,
        "same ID resolves the standalone receiving Client's drawable"
    );
    assert!(crate::helpers::TheInGameUI::is_camera_tracking_drawable());
    empty.update_camera_tracking_drawable();
    assert!(
        !crate::helpers::TheInGameUI::is_camera_tracking_drawable(),
        "standalone empty selection retains original clear"
    );
}

#[test]
fn fallible_native_observation_rejects_at_late_boundary_without_consuming_visual_frame() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "fallible_native_observation_rejects_at_late_boundary_without_consuming_visual_frame",
    ) {
        return;
    }
    get_shell().init().unwrap();
    let mut receiver = client(191);
    let mut peer = client(191);
    let before = receiver.last_visual_time_frame;
    let peer_before = peer.last_visual_time_frame;
    let mut calls = 0;
    let result = receiver.try_begin_presentation_shell_with_owned_visual_time(
        1. / 30.,
        frames(191),
        &[],
        || {
            calls += 1;
            Err(GameClientError::InvalidOperation(
                "missing receiving debug owner".into(),
            ))
        },
    );
    assert!(
        matches!(result, Err(GameClientError::InvalidOperation(ref reason)) if reason == "missing receiving debug owner")
    );
    assert_eq!(calls, 1);
    assert_eq!(receiver.last_visual_time_frame, before);
    assert_eq!(peer.last_visual_time_frame, peer_before);
    let step = receiver
        .try_begin_presentation_shell_with_owned_visual_time(1. / 30., frames(191), &[], || {
            Ok(NativeVisualTime {
                frozen: false,
                visual_speed: 1.,
            })
        })
        .unwrap();
    assert!(!step.frozen);
    let step = peer
        .try_begin_presentation_shell_with_owned_visual_time(1. / 30., frames(191), &[], || {
            Ok(NativeVisualTime {
                frozen: true,
                visual_speed: 1.,
            })
        })
        .unwrap();
    assert!(step.frozen);
}
