//! Actual admitted Physics / AI locomotion through the native match adapter.
use super::*;
use crate::common::{AsciiString, BodyDamageType, LocomotorSetType};
use crate::locomotor::{
    LOCOMOTOR_STORE, LocomotorAppearance, LocomotorBehaviorZ, LocomotorTemplate,
};
use crate::modules::AIUpdateInterface;
use crate::modules::PhysicsBehavior;
use crate::object::behavior::physics_update::PhysicsBehaviorModuleData;
use crate::object::body::active_body::ActiveBodyModuleData;
use crate::object::update::AIUpdateModuleData;
use game_engine::common::thing::module::ModuleInterfaceType;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_OWNED_LOCOMOTION_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

fn source_template(name: &str, acceleration: f32) -> Arc<dyn ThingTemplate> {
    source_template_with_appearance(name, acceleration, LocomotorAppearance::TwoLegs)
}

fn source_template_with_appearance(
    name: &str,
    acceleration: f32,
    appearance: LocomotorAppearance,
) -> Arc<dyn ThingTemplate> {
    source_template_with_motion(name, acceleration, appearance, 0.0, 0.0)
}
fn source_template_with_motion(
    name: &str,
    acceleration: f32,
    appearance: LocomotorAppearance,
    pivot: f32,
    wander: f32,
) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new(name.into());
    base.add_kind_of(KindOf::Infantry);
    let mut locomotor = if appearance == LocomotorAppearance::TwoLegs {
        LocomotorTemplate::new_infantry(format!("{name}Loco"))
    } else {
        LocomotorTemplate::new(format!("{name}Loco"))
    };
    locomotor.appearance = appearance;
    locomotor.turn_pivot_offset = pivot;
    locomotor.wander_width_factor = wander;
    locomotor.surfaces = crate::locomotor::SURFACE_GROUND;
    locomotor.behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    locomotor.acceleration = acceleration;
    if acceleration == 0.0 {
        locomotor.braking = 0.0;
    }
    locomotor.max_turn_rate = 0.1;
    locomotor.max_speed = 10.0;
    LOCOMOTOR_STORE.register_template(locomotor);
    let mut ai = AIUpdateModuleData::default();
    ai.set_locomotor_set_entries(
        LocomotorSetType::Normal,
        vec![AsciiString::from(format!("{name}Loco").as_str())],
    );
    let mut body = ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "OwnedMotionBody".into(),
                data: Arc::new(body),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "OwnedMotionAI".into(),
                data: Arc::new(ai),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: "PhysicsBehavior".into(),
                module_tag: "OwnedMotionPhysics".into(),
                data: Arc::new(PhysicsBehaviorModuleData::default()),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
        ],
    })
}

fn game(acceleration: f32) -> NativeMatch {
    game_with_appearance(acceleration, LocomotorAppearance::TwoLegs)
}

fn game_with_appearance(acceleration: f32, appearance: LocomotorAppearance) -> NativeMatch {
    let mut game = NativeMatch::with_source_template(
        150.0,
        container_template(),
        false,
        source_template_with_appearance("OwnedMotion", acceleration, appearance),
    );
    Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
    game
}

fn ai_proxy(
    owner: &Arc<RwLock<Object>>,
) -> game_engine::common::thing::update_module::UpdateModulePtr {
    owner
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .pop()
        .expect("admitted AI update proxy")
}

fn physics_handle(owner: &Arc<RwLock<Object>>) -> crate::object::PhysicsInterfaceHandle {
    owner
        .read()
        .unwrap()
        .get_physics()
        .expect("authored Physics")
}

fn prepare_ai_path(ai: &Arc<std::sync::Mutex<dyn AIUpdateInterface>>, target: Coord3D) {
    let mut guard = ai.lock().unwrap();
    guard
        .unit_ai_for_test()
        .expect("installed UnitAI")
        .install_native_path_for_test(&[Coord3D::new(50.0, 50.0, 0.0), target]);
    guard.set_locomotor_goal_position_on_path();
}

fn move_step(game: &mut NativeMatch, frame: u32, target: Coord3D) -> Result<(), String> {
    let owner = Arc::clone(&game.source);
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    let current = ai_proxy(&owner);
    prepare_ai_path(&ai, target);
    game.operate_with_current(frame, Some(current), |world| {
        ai.lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .map(|_| ())
    })
}

#[test]
fn admitted_two_legs_motion_wakes_exact_physics_proxy_and_keeps_single_turn_step() {
    const NAME: &str =
        "admitted_two_legs_motion_wakes_exact_physics_proxy_and_keeps_single_turn_step";
    if !child(&format!("{}::{}", module_path!(), NAME)) {
        return;
    }
    let mut game = game(0.1);
    let owner = Arc::clone(&game.source);
    let physics = physics_handle(&owner);
    let mut physics_proxies = owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior");
    assert_eq!(physics_proxies.len(), 1, "one installed Physics proxy");
    let proxy = physics_proxies.pop().unwrap();
    let registrations_before = game.scheduler.get_number_sleepy_updates();
    move_step(&mut game, 40, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert!((owner.read().unwrap().get_orientation() - 0.1).abs() < 1.0e-5);
    // A second operation in the same frame applies another motive force to the
    // same canonical Physics instance; the existing proxy is rescheduled in
    // place rather than entered twice. Its target follows the current heading,
    // so the second actual locomotor call does not request another turn.
    move_step(
        &mut game,
        40,
        Coord3D::new(
            50.0 + 100.0 * 0.1_f32.cos(),
            50.0 + 100.0 * 0.1_f32.sin(),
            0.0,
        ),
    )
    .unwrap();
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy), Some((1, 41)));
    assert_eq!(
        game.scheduler.get_number_sleepy_updates(),
        registrations_before,
        "force wake reschedules the existing proxy without registering a duplicate"
    );
    {
        let physics_state = physics.access().unwrap();
        assert!(physics_state.is_motive_at(49).unwrap());
        assert!(!physics_state.is_motive_at(50).unwrap());
        assert_eq!(
            physics_state.get_turning(),
            0.0,
            "the second requested rotation fits within the turn limit (CPP TURN_NONE)"
        );
    }
    assert!(
        physics.try_access().is_ok(),
        "every short lease was released"
    );
    assert!((owner.read().unwrap().get_orientation() - 0.1).abs() < 1.0e-5);
}

#[test]
fn zero_force_still_wakes_installed_physics_until_frame_deadline() {
    const NAME: &str = "zero_force_still_wakes_installed_physics_until_frame_deadline";
    if !child(&format!("{}::{}", module_path!(), NAME)) {
        return;
    }
    let mut game = game(0.0);
    let owner = Arc::clone(&game.source);
    let proxy = owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")[0]
        .clone();
    let registrations_before = game.scheduler.get_number_sleepy_updates();
    move_step(&mut game, 70, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy), Some((1, 71)));
    assert_eq!(
        game.scheduler.get_number_sleepy_updates(),
        registrations_before
    );
    let physics = physics_handle(&owner);
    let physics_state = physics.access().unwrap();
    assert!(physics_state.is_motive_at(79).unwrap());
    assert!(!physics_state.is_motive_at(80).unwrap());
}

#[test]
fn installed_default_other_path_phase_preserves_ordinary_other_movement() {
    const NAME: &str = "installed_default_other_path_phase_preserves_ordinary_other_movement";
    if !child(&format!("{}::{}", module_path!(), NAME)) {
        return;
    }
    let mut game = game_with_appearance(0.1, LocomotorAppearance::Other);
    let owner = Arc::clone(&game.source);
    move_step(&mut game, 90, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert!((owner.read().unwrap().get_orientation() - 0.1).abs() < 1.0e-5);
    let physics = physics_handle(&owner);
    assert!(physics.access().unwrap().is_motive_at(99).unwrap());
}

#[test]
fn native_two_legs_rejects_foreign_owner_and_keeps_same_id_world_clocks_separate() {
    const NAME: &str =
        "native_two_legs_rejects_foreign_owner_and_keeps_same_id_world_clocks_separate";
    if !child(&format!("{}::{}", module_path!(), NAME)) {
        return;
    }
    let mut first = game(0.1);
    let mut peer = game(0.8);
    let first_owner = Arc::clone(&first.source);
    let peer_owner = Arc::clone(&peer.source);
    assert_eq!(
        first_owner.read().unwrap().get_id(),
        peer_owner.read().unwrap().get_id(),
        "separate factories may reuse the numeric ID"
    );
    let peer_ai = peer_owner
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    prepare_ai_path(&peer_ai, Coord3D::new(50.0, 150.0, 0.0));
    let first_ai = first_owner
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    prepare_ai_path(&first_ai, Coord3D::new(50.0, 150.0, 0.0));
    let peer_physics = physics_handle(&peer_owner);
    let first_current = ai_proxy(&first_owner);
    let result = first.operate_with_current(80, Some(first_current), |world| {
        peer_ai
            .lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
    });
    assert!(
        result.is_err(),
        "foreign factory owner rejects before mutation"
    );
    assert!(!peer_physics.access().unwrap().is_motive_at(80).unwrap());

    let wrong_current = first_owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")[0]
        .clone();
    let wrong_current_result = first.operate_with_current(81, Some(wrong_current), |world| {
        first_ai
            .lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
    });
    assert!(
        wrong_current_result.is_err(),
        "the Physics proxy cannot masquerade as the driving AI phase"
    );

    move_step(&mut first, 100, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    move_step(&mut peer, 200, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    let first_proxy = first_owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")[0]
        .clone();
    let peer_proxy = peer_owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")[0]
        .clone();
    assert_eq!(
        first.scheduler.sleepy_entry_for(&first_proxy),
        Some((1, 101))
    );
    assert_eq!(peer.scheduler.sleepy_entry_for(&peer_proxy), Some((1, 201)));
    assert!(
        physics_handle(&first_owner)
            .access()
            .unwrap()
            .is_motive_at(109)
            .unwrap()
    );
    assert!(
        physics_handle(&peer_owner)
            .access()
            .unwrap()
            .is_motive_at(209)
            .unwrap()
    );
    assert!(!physics_handle(&first_owner).same_instance(&physics_handle(&peer_owner)));
}

#[test]
fn installed_motive_force_wakes_before_stamping_its_cpp_deadline() {
    if !child(&format!(
        "{}::installed_motive_force_wakes_before_stamping_its_cpp_deadline",
        module_path!()
    )) {
        return;
    }
    let mut game = game(0.0);
    let owner = Arc::clone(&game.source);
    let physics = physics_handle(&owner);
    let current = ai_proxy(&owner);
    let object = owner.read().unwrap();
    let proxy = physics.installed_update_proxy(&object).unwrap();
    let mut woke = false;
    physics
        .apply_motive_force_with_wake(&Coord3D::ZERO, &object, 70, || {
            // C++ applyMotiveForce clears expiry before applyForce's synchronous
            // setWakeFrame, and stamps frame + MOTIVE_FRAMES only afterward.
            assert!(!physics.access().unwrap().is_motive_at(70).unwrap());
            game.scheduler
                .awaken_update_module_with_current(&proxy, Some(&current), 70, 71)?;
            assert!(!physics.access().unwrap().is_motive_at(70).unwrap());
            woke = true;
            Ok(())
        })
        .unwrap();
    assert!(woke, "even zero force requests a synchronous wake");
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy), Some((1, 71)));
    let state = physics.access().unwrap();
    assert!(state.is_motive_at(79).unwrap());
    assert!(!state.is_motive_at(80).unwrap());
}

#[test]
fn two_legs_right_angle_turn_with_zero_speed_delta_does_not_brake() {
    if !child(&format!(
        "{}::two_legs_right_angle_turn_with_zero_speed_delta_does_not_brake",
        module_path!()
    )) {
        return;
    }
    let mut game = game(2.0);
    let owner = Arc::clone(&game.source);
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    ai.lock().unwrap().set_desired_speed(10.0);
    let physics = physics_handle(&owner);
    assert_eq!(physics.accumulated_force_acceleration(), Coord3D::ZERO);

    // C++ Locomotor.cpp moveTowardsPositionLegs computes the turning speed
    // coefficient from the pre-turn angle. A right angle sets goal speed to
    // zero; with zero actual speed, its speedDelta != 0 branch must not brake.
    move_step(&mut game, 310, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert!((owner.read().unwrap().get_orientation() - 0.1).abs() < 1.0e-5);
    assert_eq!(physics.accumulated_force_acceleration(), Coord3D::ZERO);
}

#[test]
fn other_right_angle_turn_retains_speed_and_applies_force_along_pre_turn_heading() {
    if !child(&format!(
        "{}::other_right_angle_turn_retains_speed_and_applies_force_along_pre_turn_heading",
        module_path!()
    )) {
        return;
    }
    let mut game = game_with_appearance(2.0, LocomotorAppearance::Other);
    let owner = Arc::clone(&game.source);
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    ai.lock().unwrap().set_desired_speed(10.0);
    let physics = physics_handle(&owner);
    assert_eq!(physics.accumulated_force_acceleration(), Coord3D::ZERO);

    // C++ moveTowardsPositionOther keeps desiredSpeed as goalSpeed and copies
    // the unit direction before rotateTowardsPosition. This distinguishes it
    // from both the infantry turning slowdown and post-turn force direction.
    move_step(&mut game, 320, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert!((owner.read().unwrap().get_orientation() - 0.1).abs() < 1.0e-5);
    let acceleration = physics.accumulated_force_acceleration();
    assert!((acceleration.x - 2.0).abs() < 1.0e-5, "{acceleration:?}");
    assert!(acceleration.y.abs() < 1.0e-5, "{acceleration:?}");
    assert!(acceleration.z.abs() < 1.0e-5, "{acceleration:?}");
}

#[test]
fn small_angle_turn_reaches_heading_and_reports_turn_none() {
    if !child(&format!(
        "{}::small_angle_turn_reaches_heading_and_reports_turn_none",
        module_path!()
    )) {
        return;
    }
    let mut game = game(0.0);
    let owner = Arc::clone(&game.source);
    move_step(
        &mut game,
        330,
        Coord3D::new(
            50.0 + 100.0 * 0.05_f32.cos(),
            50.0 + 100.0 * 0.05_f32.sin(),
            0.0,
        ),
    )
    .unwrap();
    assert!((owner.read().unwrap().get_orientation() - 0.05).abs() < 1.0e-5);
    // C++ classifies TURN_NONE when the entire requested angle fits inside the
    // turn limit, even though the object actually changes its orientation.
    assert_eq!(physics_handle(&owner).access().unwrap().get_turning(), 0.0);
}

#[test]
fn installed_physics_stunned_flag_prevents_native_force_and_turn() {
    if !child(&format!(
        "{}::installed_physics_stunned_flag_prevents_native_force_and_turn",
        module_path!()
    )) {
        return;
    }
    let mut game = game(2.0);
    let owner = Arc::clone(&game.source);
    let physics = physics_handle(&owner);
    // No Object lease surrounds this actual module operation: set_stunned also
    // supports a synchronous object-model notification in compatibility mode.
    physics.access().unwrap().set_stunned(true);
    assert!(physics.installed_is_stunned().unwrap());
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    ai.lock().unwrap().set_desired_speed(10.0);
    let orientation = owner.read().unwrap().get_orientation();
    move_step(&mut game, 340, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
    assert_eq!(owner.read().unwrap().get_orientation(), orientation);
    assert_eq!(physics.accumulated_force_acceleration(), Coord3D::ZERO);
    assert_eq!(physics.access().unwrap().get_turning(), 0.0);
}

#[test]
fn other_authored_wandering_is_ignored_by_actual_owned_motion() {
    let mut configured = NativeMatch::with_source_template(
        150.0,
        container_template(),
        false,
        source_template_with_motion(
            "OwnedOtherWander",
            0.1,
            LocomotorAppearance::Other,
            0.5,
            0.3,
        ),
    );
    let mut ordinary = NativeMatch::with_source_template(
        150.0,
        container_template(),
        false,
        source_template_with_motion("OwnedOtherPlain", 0.1, LocomotorAppearance::Other, 0.5, 0.0),
    );
    let peer = NativeMatch::with_source_template(
        150.0,
        container_template(),
        false,
        source_template_with_motion("OwnedOtherPeer", 0.1, LocomotorAppearance::Other, 0.5, 0.3),
    );
    for game in [&mut configured, &mut ordinary] {
        Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
    }
    assert_eq!(
        configured.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let peer_pose = peer.source.read().unwrap().get_transform_matrix();
    // Run the real path phase first while turning, then along its post-turn
    // heading. OTHER must not consume the infantry wandering state or alter
    // either its pivot matrix or subsequent canonical motive force.
    for frame in [41, 42] {
        let target = if frame == 41 {
            Coord3D::new(50.0, 150.0, 0.0)
        } else {
            let owner = ordinary.source.read().unwrap();
            let position = owner.get_position();
            let angle = owner.get_orientation();
            Coord3D::new(
                position.x + 100.0 * angle.cos(),
                position.y + 100.0 * angle.sin(),
                position.z,
            )
        };
        move_step(&mut configured, frame, target).unwrap();
        move_step(&mut ordinary, frame, target).unwrap();
        assert_eq!(
            configured.source.read().unwrap().get_transform_matrix(),
            ordinary.source.read().unwrap().get_transform_matrix()
        );
        assert_eq!(
            physics_handle(&configured.source).accumulated_force_acceleration(),
            physics_handle(&ordinary.source).accumulated_force_acceleration()
        );
        assert_eq!(
            peer.source.read().unwrap().get_transform_matrix(),
            peer_pose
        );
        assert_eq!(
            physics_handle(&peer.source).accumulated_force_acceleration(),
            Coord3D::ZERO
        );
    }
    assert!(
        physics_handle(&configured.source)
            .accumulated_force_acceleration()
            .length()
            > 0.0
    );
}

#[test]
fn authored_pivot_and_wander_move_only_the_exact_factory_owner() {
    for (name, appearance, pivot, wander) in [
        ("OwnedPivot", LocomotorAppearance::Other, 0.5, 0.0),
        ("OwnedWander", LocomotorAppearance::TwoLegs, 0.0, 0.3),
    ] {
        let template = source_template_with_motion(name, 0.1, appearance, pivot, wander);
        let mut game = NativeMatch::with_source_template(
            150.0,
            container_template(),
            false,
            Arc::clone(&template),
        );
        let peer = NativeMatch::with_source_template(150.0, container_template(), false, template);
        let owner = Arc::clone(&game.source);
        Object::admit_update_schedule_from_handle(&owner, &mut game.scheduler).unwrap();
        let before = owner.read().unwrap().get_transform_matrix();
        let peer_before = peer.source.read().unwrap().get_transform_matrix();
        move_step(&mut game, 41, Coord3D::new(50.0, 150.0, 0.0)).unwrap();
        let after = owner.read().unwrap().get_transform_matrix();
        assert_ne!(after, before);
        assert!(after.is_finite());
        if appearance == LocomotorAppearance::Other {
            let radius = owner
                .read()
                .unwrap()
                .get_geometry_info()
                .get_bounding_circle_radius();
            let position = before.w_axis.truncate();
            let turn_point = glam::Vec3::new(position.x + pivot * radius, position.y, 0.0);
            // CPP Locomotor.cpp2113-2188 premultiplies the entire existing
            // transform around the geometry-derived pivot, with the authored
            // 0.1 turn limit. Checking only yaw would miss a lost translation.
            let expected = glam::Mat4::from_translation(turn_point)
                * glam::Mat4::from_rotation_z(0.1)
                * glam::Mat4::from_translation(-turn_point)
                * before;
            for (actual, expected) in after
                .to_cols_array()
                .into_iter()
                .zip(expected.to_cols_array())
            {
                assert!(
                    (actual - expected).abs() < 0.0001,
                    "pivot component {actual} != {expected}"
                );
            }
        }
        assert_eq!(
            peer.source.read().unwrap().get_transform_matrix(),
            peer_before
        );
        assert_eq!(
            physics_handle(&peer.source).accumulated_force_acceleration(),
            Coord3D::ZERO
        );
        // OTHER may turn in place before accepting motive force. Follow the
        // actual post-pivot heading on the next frame, as C++ does.
        let (position, angle) = {
            let object = owner.read().unwrap();
            (*object.get_position(), object.get_orientation())
        };
        move_step(
            &mut game,
            42,
            Coord3D::new(
                position.x + 100.0 * angle.cos(),
                position.y + 100.0 * angle.sin(),
                position.z,
            ),
        )
        .unwrap();
        assert!(
            physics_handle(&owner)
                .accumulated_force_acceleration()
                .length()
                > 0.0
        );
    }
}

/// The original explicit-goal fixture, exercised through actual admitted AI,
/// Body, Physics and driving terrain instead of the auxiliary registry model.
pub(crate) fn explicit_goal_moves_canonical_owner_without_path_or_foreign_writes() {
    let make = |name: &str| {
        let mut game = NativeMatch::with_source_template(
            150.0,
            container_template(),
            false,
            source_template_with_appearance(name, 0.1, LocomotorAppearance::Other),
        );
        Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
        game
    };
    let mut game = make("OwnedExplicitGoal");
    let peer = make("OwnedExplicitGoalPeer");
    let owner = Arc::clone(&game.source);
    assert_eq!(
        owner.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let peer_position = *peer.source.read().unwrap().get_position();
    let peer_velocity = physics_handle(&peer.source)
        .access()
        .unwrap()
        .get_velocity();
    let before = *owner.read().unwrap().get_position();
    let goal = Coord3D::new(before.x + 40.0, before.y, before.z);
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    ai.lock()
        .unwrap()
        .set_locomotor_goal_position_explicit(goal);
    game.operate_with_current(40, Some(ai_proxy(&owner)), |world| {
        ai.lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
    });
    let physics = physics_handle(&owner);
    let applied_acceleration = physics.accumulated_force_acceleration().x;
    assert!(applied_acceleration > 0.0);
    let proxy = owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")
        .pop()
        .unwrap();
    game.operate_with_current(40, Some(proxy), |world| {
        world.update_physics_phase(&owner).unwrap();
    });
    // Explicit zero on-path distance enters braking now (CPP Locomotor987–995).
    // Physics649–661 integrates velocity but suppresses XY pose this frame;
    // the direct locomotor correction begins when wasBraking was already true.
    assert!(
        owner
            .read()
            .unwrap()
            .test_status(crate::common::ObjectStatusTypes::Braking)
    );
    assert_eq!(*owner.read().unwrap().get_position(), before);
    let first_velocity = physics.access().unwrap().get_velocity().x;
    assert!(
        (first_velocity - applied_acceleration).abs() < 1.0e-6,
        "from rest, Physics must add the locomotor's per-frame acceleration without another time step: acceleration={applied_acceleration}, velocity={first_velocity}"
    );
    game.operate_with_current(41, Some(ai_proxy(&owner)), |world| {
        ai.lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
    });
    let after_locomotor = *owner.read().unwrap().get_position();
    assert!(
        after_locomotor.x > before.x,
        "CPP braking correction commits before Physics"
    );
    let physics_proxy = owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")
        .pop()
        .unwrap();
    game.operate_with_current(41, Some(physics_proxy), |world| {
        world.update_physics_phase(&owner).unwrap();
    });
    let position = *owner.read().unwrap().get_position();
    assert_eq!(
        position, after_locomotor,
        "Physics must not apply a second XY integration while braking"
    );
    assert!(position.x > before.x);
    assert!((position.y - before.y).abs() < 0.01);
    assert!(
        physics
            .access()
            .unwrap()
            .get_forward_speed_2d_with_object(&owner.read().unwrap())
            > 0.0
    );
    let observation = ai
        .lock()
        .unwrap()
        .unit_ai_for_test()
        .unwrap()
        .native_waypoint_observation_for_test();
    assert!(!observation.do_final_position);
    assert!(
        !observation.has_path,
        "explicit movement does not invent a path"
    );
    assert_eq!(*peer.source.read().unwrap().get_position(), peer_position);
    assert_eq!(
        physics_handle(&peer.source)
            .access()
            .unwrap()
            .get_velocity(),
        peer_velocity
    );
}

#[test]
fn installed_idle_scheduler_keeps_same_id_peer_wakes_and_pose_unchanged() {
    if !child(
        "modules::ai_native_world_tests::locomotion::installed_idle_scheduler_keeps_same_id_peer_wakes_and_pose_unchanged",
    ) {
        return;
    }
    let mut first = game(0.1);
    let peer = game(0.1);
    let first_proxy = ai_proxy(&first.source);
    let peer_proxy = ai_proxy(&peer.source);
    let interface = first
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    first.operate(0, |world| {
        interface
            .lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap()
    });
    assert_eq!(
        first.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let peer_wake = peer.scheduler.sleepy_entry_for(&peer_proxy);
    let peer_position = *peer.source.read().unwrap().get_position();
    let count = first
        .operate(1, |world| world.tick_owned_updates(1))
        .unwrap();
    assert!(count >= 1, "the real admitted heap must execute callbacks");
    assert!(
        first.scheduler.sleepy_entry_for(&first_proxy).unwrap().1 > 1,
        "actual Idle sleep must reschedule its proxy"
    );
    assert_eq!(peer.scheduler.sleepy_entry_for(&peer_proxy), peer_wake);
    assert_eq!(*peer.source.read().unwrap().get_position(), peer_position);
}

#[test]
fn unsupported_scheduled_goal_keeps_actual_wake_and_waypoint_metadata() {
    if !child(
        "modules::ai_native_world_tests::locomotion::unsupported_scheduled_goal_keeps_actual_wake_and_waypoint_metadata",
    ) {
        return;
    }
    let mut game = game(0.1);
    let owner = Arc::clone(&game.source);
    let interface = owner.read().unwrap().get_ai_update_interface().unwrap();
    game.operate(0, |world| {
        interface
            .lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap()
    });
    {
        let mut ai = interface.lock().unwrap();
        ai.set_locomotor_goal_orientation(1.0);
        ai.set_completed_waypoint_id(Some(23));
    }
    let proxy = ai_proxy(&owner);
    let wake_before = game.scheduler.sleepy_entry_for(&proxy);
    let position_before = *owner.read().unwrap().get_position();
    let failure = game
        .operate(1, |world| world.tick_owned_updates(1))
        .unwrap_err();
    assert!(failure.contains("goal requires"), "{failure}");
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy), wake_before);
    assert_eq!(
        interface.lock().unwrap().get_completed_waypoint_id(),
        Some(23)
    );
    assert_eq!(*owner.read().unwrap().get_position(), position_before);
}

#[test]
fn scheduled_dead_entry_notifies_receiving_scripts_then_preserves_full_velocity_magnitude() {
    if !child(
        "modules::ai_native_world_tests::locomotion::scheduled_dead_entry_notifies_receiving_scripts_then_preserves_full_velocity_magnitude",
    ) {
        return;
    }
    let peer = game(0.1);
    let mut game = game(0.1);
    let owner = Arc::clone(&game.source);
    let interface = owner.read().unwrap().get_ai_update_interface().unwrap();
    let proxy = ai_proxy(&owner);
    game.operate(9, |world| {
        interface
            .lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap()
    });
    interface.lock().unwrap().mark_dead_owned().unwrap();
    let entry_sleep = game
        .operate_with_current(10, Some(Arc::clone(&proxy)), |world| {
            interface
                .lock()
                .unwrap()
                .update_scheduled_tick_with_world(world)
        })
        .unwrap();
    assert_eq!(entry_sleep, crate::modules::UpdateSleepTime::None);
    assert_eq!(
        interface.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Dead as u32)
    );
    assert_eq!(game.disposal.scripts.get_frame_object_count_changed(), 10);
    let physics = physics_handle(&owner);
    physics
        .access()
        .unwrap()
        .set_velocity(&Coord3D::new(3.0, 4.0, 12.0));
    game.operate_with_current(11, Some(proxy), |world| {
        interface
            .lock()
            .unwrap()
            .update_scheduled_tick_with_world(world)
    })
    .unwrap();
    assert!(owner.read().unwrap().is_effectively_dead());
    let velocity = physics.access().unwrap().get_velocity();
    // CPP full magnitude is13 => desired10.4; scrub2D must not accelerate
    // current horizontal speed5. The old XY-only magnitude wrongly reduced it.
    assert_eq!((velocity.x, velocity.y, velocity.z), (3.0, 4.0, 12.0));
    assert!(!peer.source.read().unwrap().is_effectively_dead());
    assert_ne!(peer.disposal.scripts.get_frame_object_count_changed(), 10);
}
