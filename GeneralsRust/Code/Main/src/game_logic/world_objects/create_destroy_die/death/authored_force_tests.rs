//! Source-backed OCL execution at the installed object's authored death callback.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

fn force_world(magnitude: u32, physics: bool, death: &str) -> GameLogic {
    let physics = if physics {
        "Behavior = PhysicsBehavior PhysicsTag\nMass = 5\nEnd\n"
    } else {
        ""
    };
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "ocl-random-force.ini".to_owned(),
        format!(
            "Object AuthoredWreck\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{physics}{death}\nEnd\n"
        ),
    )]).unwrap();
    let mut world = GameLogic::new();
    world
        .admit_ocl_application_sources(&[format!(
            "ObjectCreationList WreckToss\nApplyRandomForce\nMinForceMagnitude = {magnitude}\nMaxForceMagnitude = {magnitude}\nMinForcePitch = 90\nMaxForcePitch = 90\nSpinRate = 90\nEnd\nEnd"
        )])
        .unwrap();
    world.admit_receiving_module_content(content).unwrap();
    world
}

fn kill(world: &mut GameLogic, id: ObjectId) {
    world
        .apply_owned_damage(
            id,
            1_000.0,
            None,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}

#[test]
fn authored_object_death_consumes_force_draws_on_the_receiving_world() {
    let death = "Behavior = CreateObjectDie DeathTag\nCreationList = WreckToss\nEnd";
    let mut first = force_world(50, true, death);
    let mut peer = force_world(100, true, death);
    let mut no_physics = force_world(150, false, death);
    let spawn = |world: &mut GameLogic| {
        world
            .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
            .unwrap()
    };
    let id = spawn(&mut first);
    assert_eq!(id, spawn(&mut peer));
    assert_eq!(id, spawn(&mut no_physics));

    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(first.logic_random.seed_words());
    let original_peer_seed = peer.logic_random.seed_words();
    let original_pose = first.objects[&id].get_transform_matrix();
    let original_velocity = first.objects[&id].movement.velocity;
    expected.next_real(0.0, std::f32::consts::TAU);
    expected.next_real(std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    expected.next_real(50.0, 50.0);
    let spin = 90.0_f32.to_radians() * (1.0 / 30.0);
    let rates = std::array::from_fn::<_, 3, _>(|_| expected.next_real(-spin, spin));

    peer.with_own_services(|| kill(&mut first, id));
    let object = &first.objects[&id];
    assert!((object.physics_accel.y - 50.0 / object.physics_get_mass()).abs() < 1e-5);
    assert_eq!(object.get_transform_matrix(), original_pose);
    assert_eq!(object.movement.velocity, original_velocity);
    assert!((object.shock_yaw_rate - rates[0]).abs() < 1e-7);
    assert!((object.shock_roll_rate - rates[1]).abs() < 1e-7);
    assert!((object.shock_pitch_rate - rates[2]).abs() < 1e-7);
    assert_eq!(first.logic_random.seed_words(), expected.seed_words());
    assert_eq!(peer.logic_random.seed_words(), original_peer_seed);
    assert_eq!(peer.objects[&id].physics_accel, Vec3::ZERO);

    kill(&mut peer, id);
    let peer_object = &peer.objects[&id];
    assert!((peer_object.physics_accel.y - 100.0 / peer_object.physics_get_mass()).abs() < 1e-5);
    let seed = no_physics.logic_random.seed_words();
    kill(&mut no_physics, id);
    assert_eq!(no_physics.objects[&id].physics_accel, Vec3::ZERO);
    assert_eq!(no_physics.logic_random.seed_words(), seed);
}

#[test]
fn ordinary_slow_death_runs_initial_force_once_at_admission() {
    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 500\nOCL = INITIAL WreckToss\nEnd";
    let mut world = force_world(75, true, death);
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    let seed = world.logic_random.seed_words();
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(seed);
    expected.next_int(0, 9); // One applicable SlowDeath with the default weight.
    expected.next_int(0, 0); // Sink delay variance.
    expected.next_int(0, 0); // Destruction delay variance.
    expected.next_int(5, 9); // C++ midpoint bounds for a 15-frame lifetime.
    expected.next_int(0, 0); // One authored INITIAL entry.
    expected.next_real(0.0, std::f32::consts::TAU);
    let spin = 90.0_f32.to_radians() * (1.0 / 30.0);
    for _ in 0..3 {
        expected.next_real(-spin, spin);
    }

    kill(&mut world, id);
    let wreck = &world.objects[&id];
    assert!(
        wreck
            .slow_death
            .as_ref()
            .is_some_and(|death| death.is_active())
    );
    let accel = wreck.physics_accel;
    assert!((accel.y - 75.0 / wreck.physics_get_mass()).abs() < 1e-5);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    kill(&mut world, id);
    assert_eq!(world.objects[&id].physics_accel, accel);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn location_ocl_with_owner_id_never_draws_object_force() {
    let mut world = force_world(75, true, "");
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    let seed = world.logic_random.seed_words();
    world
        .execute_parsed_weapon_ocl_at(
            "WreckToss",
            Some(id),
            Team::GLA,
            crate::game_logic::VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::X,
        )
        .unwrap();
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
    assert_eq!(world.logic_random.seed_words(), seed);
}

#[test]
fn legacy_air_death_name_does_not_supply_an_unauthored_force() {
    let mut world = GameLogic::new();
    let name = "GLAVehicleTechnicalAirDeath";
    let mut template = crate::game_logic::ThingTemplate::new(name);
    template
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(100.0);
    world.templates.insert(name.into(), template);
    let id = world.create_object(name, Team::GLA, Vec3::Y).unwrap();
    let velocity = world.objects[&id].movement.velocity;
    kill(&mut world, id);
    assert_eq!(world.objects[&id].movement.velocity, velocity);
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
}

#[test]
fn same_frame_slow_death_runs_midpoint_then_final_before_retirement() {
    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 0\nOCL = MIDPOINT WreckToss\nOCL = FINAL FinalToss\nEnd";
    let mut world = force_world(50, true, death);
    world.apply_receiving_ocl_overrides(&[
        "ObjectCreationList FinalToss\nApplyRandomForce\nMinForceMagnitude = 50\nMaxForceMagnitude = 50\nMinForcePitch = 90\nMaxForcePitch = 90\nSpinRate = 0\nEnd\nEnd".into(),
    ]).unwrap();
    world.frame = 41;
    let id = world
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut world, id);
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
    let mut expected = game_engine::common::random_value::RandomState::default();
    expected.set_seed_words(world.logic_random.seed_words());
    let mut expected_force = 0.0;
    let mut expected_spin = [0.0; 3];
    let mut expected_retirement = false;
    for action in include_str!("slow_death_same_frame.trace").lines() {
        match action {
            "draw" => {
                expected.next_int(0, 0);
            }
            "ocl WreckToss" | "ocl FinalToss" => {
                expected_force += 50.0;
                expected.next_real(0.0, std::f32::consts::TAU);
                let spin = if action == "ocl WreckToss" {
                    90.0_f32.to_radians() * (1.0 / 30.0)
                } else {
                    0.0
                };
                expected_spin = std::array::from_fn(|_| expected.next_real(-spin, spin));
            }
            "retire" => expected_retirement = true,
            _ => panic!("unexpected original SlowDeath action: {action}"),
        }
    }
    world.update_ai(&[id], 1.0 / 30.0).unwrap();
    let wreck = &world.objects[&id];
    assert!((wreck.physics_accel.y - expected_force / wreck.physics_get_mass()).abs() < 1e-5);
    assert_eq!(
        [wreck.shock_yaw_rate, wreck.shock_roll_rate, wreck.shock_pitch_rate],
        expected_spin
    );
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    assert_eq!(
        world
            .pending_object_removal_snapshot()
            .iter()
            .any(|record| record.id == id),
        expected_retirement,
    );
}

#[test]
fn saved_slow_death_resumes_selected_clock_without_replaying_midpoint() {
    use crate::save_load::snapshot::SnapshotBuilder;

    let death = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 1000\nOCL = MIDPOINT WreckToss\nOCL = FINAL WreckToss\nEnd";
    let mut source = force_world(50, true, death);
    source.frame = 41;
    let id = source
        .create_object("AuthoredWreck", Team::GLA, Vec3::ZERO)
        .unwrap();
    kill(&mut source, id);
    let receipt = source.objects[&id].native_die_runtime.slow_death.unwrap();
    assert!((51..=60).contains(&receipt.midpoint_frame));
    source.frame = receipt.midpoint_frame;
    source.update_ai(&[id], 1.0 / 30.0).unwrap();
    let timer = source.objects[&id].slow_death.as_ref().unwrap();
    assert!(timer.midpoint_played);
    let final_frame = timer.destroy_at_frame;
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = force_world(50, true, death);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.objects[&id].native_die_runtime.slow_death,
        Some(receipt)
    );
    let post_midpoint = restored.logic_random.seed_words();
    let saved_accel = restored.objects[&id].physics_accel;
    restored.update_ai(&[id], 1.0 / 30.0).unwrap();
    assert_eq!(restored.logic_random.seed_words(), post_midpoint);
    assert_eq!(restored.objects[&id].physics_accel, saved_accel);
    for world in [&mut restored, &mut source] {
        world.frame = final_frame;
        world.update_ai(&[id], 1.0 / 30.0).unwrap();
        assert!(
            world
                .pending_object_removal_snapshot()
                .iter()
                .any(|record| record.id == id)
        );
    }
    assert_eq!(
        restored.logic_random.seed_words(),
        source.logic_random.seed_words()
    );
}
