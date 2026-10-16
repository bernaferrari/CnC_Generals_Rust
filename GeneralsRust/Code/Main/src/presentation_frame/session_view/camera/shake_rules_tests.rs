use super::*;
use crate::game_logic::GameLogic;
use glam::Vec3;

fn rules(normal: f32, range: f32, maximum: f32) -> CameraShakeRules {
    CameraShakeRules {
        shake_normal_intensity: normal,
        max_shake_range: range,
        max_shake_intensity: maximum,
        ..Default::default()
    }
}
fn owner(rules: CameraShakeRules) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_camera_application_shake(rules).unwrap();
    world
}

#[test]
fn source_categories_negative_intensity_and_overflow_use_one_real_script_kernel() {
    // ScriptActions.cpp5030 sends its current view position into W3DView::shake.
    for (kind, expected) in [0.5, 1., 2.5, 5., 8., 12.].into_iter().enumerate() {
        let mut world = owner(rules(1., 150., 100.));
        world.enqueue_script_screen_shake_at(kind as i32, 7);
        assert_eq!(world.session_view.impulse.intensity, expected);
        assert_eq!(
            world.session_view.impulse.offset,
            Vec3::ZERO,
            "enqueue is not an update"
        );
    }
    let mut negative = owner(rules(-2., 150., 100.));
    negative.enqueue_script_screen_shake_at(1, 7);
    assert_eq!(
        negative.session_view.impulse.intensity, -2.,
        "source adds signed authored intensity"
    );
    let mut exact = owner(rules(10., 150., 10.));
    exact.enqueue_script_screen_shake_at(1, 7);
    assert_eq!(
        exact.session_view.impulse.intensity, 10.,
        "strict greater-than overflow"
    );
    let mut overflow = owner(rules(12., 150., 10.));
    overflow.enqueue_script_screen_shake_at(1, 7);
    assert_eq!(
        overflow.session_view.impulse.intensity, 3.,
        "source uses fixed three, not configured maximum"
    );
}

#[test]
fn source_zero_and_negative_ranges_are_not_replaced_with_a_floor() {
    let mut zero = owner(rules(2., 0., 10.));
    zero.enqueue_script_screen_shake_at(1, 7);
    assert!(
        zero.session_view.impulse.intensity.is_nan(),
        "source coincident zero-range divides zero by zero"
    );
    let mut receiving = crate::presentation_frame::SessionView::default();
    let before = receiving.capture();
    assert!(
        receiving.admit(zero.session_view.capture()).is_err(),
        "existing detached view-state contract rejects the resulting nonfinite impulse"
    );
    assert_eq!(receiving.capture(), before);
    assert_eq!(
        zero.session_view.advance_impulse(),
        Vec3::ZERO,
        "original quiet branch normalizes nonpositive/unordered intensity on update"
    );
    assert_eq!(zero.session_view.impulse.intensity, 0.);
    let mut negative = owner(rules(2., -1., 10.));
    negative.session_view.impulse.intensity = 4.;
    negative.enqueue_script_screen_shake_at(1, 7);
    assert_eq!(
        negative.session_view.impulse.intensity, 4.,
        "source range return occurs before adding intensity"
    );
    assert_ne!(
        negative.session_view.impulse.cosine, 0.,
        "direction is replaced before out-of-range return"
    );
}

#[test]
fn rule_admission_changes_only_future_enqueues_and_failed_admission_is_atomic() {
    let mut world = owner(rules(2., 100., 10.));
    let peer = owner(rules(9., 900., 90.));
    let peer_before = peer.capture_camera_continuation().unwrap();
    world.enqueue_script_screen_shake_at(1, 3);
    let active = world.session_view.capture();
    world.admit_camera_map_shake(rules(8., 200., 100.)).unwrap();
    assert_eq!(world.session_view.capture(), active);
    world.enqueue_script_screen_shake_at(1, 3);
    assert_eq!(world.session_view.impulse.intensity, 10.);
    let before = world.capture_camera_continuation().unwrap();
    assert!(
        world
            .admit_camera_map_shake(rules(1., f32::NAN, 10.))
            .is_err()
    );
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    world.session_view.camera.shake_policy.clear_map_rules();
    assert_eq!(
        world.session_view.impulse.intensity, 10.,
        "removing map overrides leaves active impulse untouched"
    );
    world.session_view.impulse.intensity = 0.;
    world.enqueue_script_screen_shake_at(1, 3);
    assert_eq!(
        world.session_view.impulse.intensity, 2.,
        "next admission uses application base again"
    );
}

#[test]
fn saved_rule_owner_preserves_next_shake_and_new_impulse_future_without_peer_mutation() {
    let mut source = owner(rules(2., 100., 100.));
    source
        .admit_camera_map_shake(rules(8., 200., 100.))
        .unwrap();
    // Logic's envelope carries these definitions. Active impulse itself
    // remains in the unchanged Native85 wire, covered by native fixtures.
    let saved = source.capture_camera_continuation().unwrap();
    let mut restored = owner(rules(17., 1700., 100.));
    let peer = owner(rules(19., 1900., 100.));
    let peer_before = peer.capture_camera_continuation().unwrap();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
    for _ in 0..4 {
        assert_eq!(
            restored.session_view.advance_impulse(),
            source.session_view.advance_impulse()
        );
        source.enqueue_script_screen_shake_at(1, 13);
        restored.enqueue_script_screen_shake_at(1, 13);
        assert_eq!(
            restored.session_view.capture(),
            source.session_view.capture()
        );
        assert_eq!(
            restored.session_view.camera.shake_policy,
            source.session_view.camera.shake_policy
        );
    }
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

#[test]
#[cfg(feature = "game_client")]
fn real_fx_nugget_uses_receiving_rule_owner_despite_foreign_global_and_reused_ids() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "real_fx_nugget_uses_receiving_rule_owner_despite_foreign_global_and_reused_ids",
        || {
            use crate::game_logic::{Player, Team, ThingTemplate};
            use gamelogic::common::Coord3D;
            fn receiving(normal: f32) -> (GameLogic, crate::game_logic::ObjectId) {
                let mut world = owner(rules(normal, 100., 100.));
                world.add_player(Player::new(0, Team::USA, "Receiving", true));
                let template = ThingTemplate::new("ShakeOwner");
                world.templates.insert(template.name.clone(), template);
                let id = world
                    .create_object_for_player("ShakeOwner", 0, Vec3::ZERO)
                    .unwrap();
                let mut shroud = world.world_services.shroud().lock().unwrap();
                shroud.init_shroud_grid(256., 256.);
                shroud.reveal_map_for_player_permanently(0).unwrap();
                drop(shroud);
                (world, id)
            }
            let (mut source, id) = receiving(2.);
            let (mut peer, peer_id) = receiving(8.);
            assert_eq!(id, peer_id);
            {
                // Isolated existing child-process harness: no fixture Mutex/TLS or
                // save/restore guard. A native receiver must never poll this owner.
                let mut foreign = game_engine::common::global_data::write();
                foreign.shake_normal_intensity = 999.;
                foreign.max_shake_range = 1.;
                foreign.max_shake_intensity = 0.;
            }
            let mut definition =
                game_engine::common::ini::ini_fx_list::FXList::new("LocalShake".into());
            definition.add_nugget(game_engine::common::ini::ini_fx_list::FXNugget::ViewShake {
                shake_type: CameraShakeType::Normal,
            });
            let fx = game_client::fx_list::FXList::from_definition(&definition);
            source.with_host_fx_view(|view| {
                fx.do_fx_pos_with_view(
                    Some(&Coord3D::new(30., 40., 999.)),
                    None,
                    0.,
                    None,
                    0.,
                    view,
                )
            });
            assert_eq!(
                source.session_view.impulse.intensity, 1.,
                "source XY distance50 over receiving range100"
            );
            assert_eq!(source.session_view.impulse.offset, Vec3::ZERO);
            peer.with_host_fx_view(|view| {
                fx.do_fx_pos_with_view(Some(&Coord3D::new(0., 0., -999.)), None, 0., None, 0., view)
            });
            assert_eq!(
                peer.session_view.impulse.intensity, 8.,
                "same-ID peer keeps its own eight-point amplitude"
            );
            let before_peer = peer.capture_camera_continuation().unwrap();
            source.enqueue_script_screen_shake_at(1, 9);
            assert_eq!(
                source.session_view.impulse.intensity, 3.,
                "script and real FX admission share receiving definitions"
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), before_peer);
        },
    );
}
