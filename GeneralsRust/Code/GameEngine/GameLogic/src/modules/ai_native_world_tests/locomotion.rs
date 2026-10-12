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
    let mut base = DefaultThingTemplate::new(name.into());
    base.add_kind_of(KindOf::Infantry);
    let mut locomotor = if appearance == LocomotorAppearance::TwoLegs {
        LocomotorTemplate::new_infantry(format!("{name}Loco"))
    } else {
        LocomotorTemplate::new(format!("{name}Loco"))
    };
    locomotor.appearance = appearance;
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
            .update_locomotor_path_phase_with_world(world)
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
            .update_locomotor_path_phase_with_world(world)
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
            .update_locomotor_path_phase_with_world(world)
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
