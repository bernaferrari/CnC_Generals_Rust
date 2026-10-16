use super::*;
#[test]
fn actual_follow_owner_replays_original_cpp_bit_records() {
    for row in include_str!("follow_cpp.csv").lines().skip(1) {
        let c: Vec<_> = row.split(',').collect();
        let f = |i: usize| c[i].parse::<f32>().unwrap();
        let b = |i: usize| c[i] == "1";
        let mut owner = SessionView::default();
        owner.target = Vec3::new(f(1), f(3), f(2));
        owner.camera.camera_yaw_radians = f(7);
        owner.camera.camera_follow_factor = f(9);
        owner
            .camera
            .native_camera
            .initialize_map(11., 300., 37.5, 13.);
        let before = owner.camera.native_camera.capture_continuation();
        owner.set_camera_follow(Some(crate::game_logic::ObjectId(42)));
        if b(10) {
            owner.set_camera_tether(crate::game_logic::ObjectId(42), b(12), f(11));
        }
        owner.camera.camera_follow_snap_immediate = b(12);
        #[cfg(feature = "game_client")]
        {
            owner.camera.native_filter_previous_target = Vec3::new(-91., -93., -92.);
        }
        let facts = FollowTargetFacts {
            id: crate::game_logic::ObjectId(42),
            position: Vec3::new(f(4), f(6), f(5)),
            airborne_orientation: (b(13) && b(14)).then(|| f(8)),
        };
        assert_eq!(
            owner.admit_camera_follow(
                Some(crate::game_logic::ObjectId(42)),
                Some(facts),
                10.,
                !b(15) && !b(16) && !b(17)
            ),
            Ok(FollowAdmission::Followed)
        );
        let actual = [
            owner.target.x,
            owner.target.z,
            owner.target.y,
            owner.camera.camera_yaw_radians,
            owner.camera.camera_follow_factor,
            owner.camera.native_camera.basis().ground_level(),
        ];
        for (i, v) in actual.into_iter().enumerate() {
            assert_eq!(
                v.to_bits(),
                c[18 + i].parse::<u32>().unwrap(),
                "case {} field {i}",
                c[0]
            );
        }
        assert_eq!(owner.camera.camera_follow_snap_immediate, b(24));
        #[cfg(feature = "game_client")]
        for (i, v) in [
            owner.camera.native_filter_previous_target.x,
            owner.camera.native_filter_previous_target.z,
            owner.camera.native_filter_previous_target.y,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                v.to_bits(),
                c[25 + i].parse::<u32>().unwrap(),
                "previous case {} field {i}",
                c[0]
            );
        }
        let after = owner.camera.native_camera.capture_continuation();
        assert_eq!(
            serde_json::to_value(&before).unwrap()["offset"],
            serde_json::to_value(&after).unwrap()["offset"],
            "follow never recalibrates map basis"
        );
    }
}
#[test]
fn follow_identity_validation_missing_target_and_peer_are_bounded() {
    let mut a = SessionView::default();
    let mut peer = SessionView::default();
    a.set_camera_follow(Some(crate::game_logic::ObjectId(42)));
    peer.set_camera_follow(Some(crate::game_logic::ObjectId(42)));
    a.camera.camera_follow_snap_immediate = true;
    a.scripted.motion.active = true;
    a.scripted.path_arrived = true;
    let facts = FollowTargetFacts {
        id: crate::game_logic::ObjectId(42),
        position: Vec3::new(80., 37., 90.),
        airborne_orientation: Some(2.),
    };
    assert_eq!(
        a.admit_camera_follow(
            Some(crate::game_logic::ObjectId(43)),
            Some(facts),
            10.,
            false
        ),
        Ok(FollowAdmission::AwaitingFrame)
    );
    assert!(
        a.scripted.motion.active
            && a.scripted.path_arrived
            && a.camera.camera_follow_snap_immediate
    );
    assert!(
        a.admit_camera_follow(
            Some(crate::game_logic::ObjectId(42)),
            Some(FollowTargetFacts {
                id: crate::game_logic::ObjectId(43),
                ..facts
            }),
            10.,
            false
        )
        .is_err()
    );
    assert!(a.scripted.motion.active);
    a.admit_camera_follow(
        Some(crate::game_logic::ObjectId(42)),
        Some(facts),
        10.,
        false,
    )
    .unwrap();
    assert_eq!(a.target, Vec3::new(80., 0., 90.));
    assert!(!a.scripted.motion.active);
    assert!(!a.scripted.path_arrived);
    assert_eq!(peer.target, Vec3::ZERO);
    assert_eq!(
        peer.scripted.camera_follow_target,
        Some(crate::game_logic::ObjectId(42))
    );
    a.set_camera_tether(crate::game_logic::ObjectId(42), true, 7.);
    a.set_camera_follow(Some(crate::game_logic::ObjectId(42)));
    assert_eq!(a.scripted.camera_tether_play, None);
    assert!(a.camera.camera_follow_snap_immediate);
    assert_eq!(
        a.admit_camera_follow(Some(crate::game_logic::ObjectId(42)), None, 10., false),
        Ok(FollowAdmission::TargetGone)
    );
    assert_eq!(a.scripted.camera_follow_target, None);
    assert!(a.camera.camera_follow_snap_immediate);
}

fn saved_owner_with_active_follow_path() -> crate::game_logic::GameLogic {
    use super::super::waypoint::{Point, WaypointMotion};
    let mut world = crate::game_logic::GameLogic::new();
    let view = &mut world.session_view;
    view.target = Vec3::new(3., 2., -7.);
    view.camera.camera_yaw_radians = 0.75;
    view.camera.camera_follow_factor = 0.25;
    view.camera
        .native_camera
        .initialize_map(11., 300., 37.5, 13.);
    view.scripted.motion = WaypointMotion::move_to(
        Point {
            x: 300.,
            y: 150.,
            z: 20.,
        },
        Point::from_world(view.target),
        0.75,
        11.,
        20.,
        1,
        3000,
        true,
        0.,
        0.,
    );
    view.scripted.path_arrived = true;
    view.set_camera_tether(crate::game_logic::ObjectId(42), false, 14.);
    #[cfg(feature = "game_client")]
    {
        view.camera.native_filter_previous_target = Vec3::new(-91., -93., -92.);
    }
    world
}

#[test]
fn invalid_tether_threshold_rejects_before_mutating_saved_owner() {
    let peer = saved_owner_with_active_follow_path();
    let peer_before = peer.capture_camera_continuation().unwrap();
    for cell in [0., f32::from_bits(1), f32::MAX, f32::INFINITY, f32::NAN] {
        let mut world = saved_owner_with_active_follow_path();
        let before = world.capture_camera_continuation().unwrap();
        let facts = FollowTargetFacts {
            id: crate::game_logic::ObjectId(42),
            position: world.session_view.target,
            airborne_orientation: None,
        };
        assert_eq!(
            world
                .session_view
                .admit_camera_follow(Some(facts.id), Some(facts), cell, true),
            Err("invalid tether partition threshold"),
            "cell bits {:08x}",
            cell.to_bits(),
        );
        assert_eq!(before, world.capture_camera_continuation().unwrap());
        assert!(world.session_view.scripted.motion.active);
        assert!(world.session_view.scripted.path_arrived);
        assert!(!world.session_view.camera.camera_follow_snap_immediate);
        assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
    }
}

#[test]
fn snap_and_ordinary_follow_do_not_observe_unused_tether_threshold() {
    for cell in [0., f32::from_bits(1), f32::MAX, f32::INFINITY, f32::NAN] {
        let mut snap = saved_owner_with_active_follow_path();
        snap.session_view.camera.camera_follow_snap_immediate = true;
        let facts = FollowTargetFacts {
            id: crate::game_logic::ObjectId(42),
            position: Vec3::new(83., 37., 73.),
            airborne_orientation: None,
        };
        assert_eq!(
            snap.session_view
                .admit_camera_follow(Some(facts.id), Some(facts), cell, true),
            Ok(FollowAdmission::Followed),
        );
        assert_eq!(snap.session_view.target, Vec3::new(83., 2., 73.));
        assert!(!snap.session_view.camera.camera_follow_snap_immediate);
        let mut follow = saved_owner_with_active_follow_path();
        follow.session_view.set_camera_follow(Some(facts.id));
        assert_eq!(
            follow
                .session_view
                .admit_camera_follow(Some(facts.id), Some(facts), cell, true),
            Ok(FollowAdmission::Followed),
        );
        assert_eq!(follow.session_view.target, Vec3::new(27., 2., 17.));
    }
}

#[test]
fn finite_follow_inputs_with_overflow_reject_before_mutating_saved_owner() {
    for tether in [false, true] {
        let mut world = saved_owner_with_active_follow_path();
        world.session_view.target.x = f32::MAX;
        if !tether {
            world
                .session_view
                .set_camera_follow(Some(crate::game_logic::ObjectId(42)));
        }
        let before = world.capture_camera_continuation().unwrap();
        let facts = FollowTargetFacts {
            id: crate::game_logic::ObjectId(42),
            position: Vec3::new(-f32::MAX, 37., 67.),
            airborne_orientation: None,
        };
        assert_eq!(
            world
                .session_view
                .admit_camera_follow(Some(facts.id), Some(facts), 10., true),
            Err("nonfinite camera follow result"),
        );
        assert_eq!(before, world.capture_camera_continuation().unwrap());
    }
}
