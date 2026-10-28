//! Actual Main turret routing, isolated by ordinary session/object ownership.
use super::*;

fn receiving_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("AmericaVehicleHumvee");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.set_primary_weapon(Weapon {
        range: 1000.0,
        damage: 1.0,
        ..Default::default()
    });
    world
        .templates
        .insert("AmericaVehicleHumvee".into(), template);
    let owner = world
        .create_object("AmericaVehicleHumvee", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let object = world.host_object(owner).unwrap();
    assert!(
        object.turret_enabled,
        "actual Humvee turret definition is installed at birth"
    );
    assert!(
        object.turret_turn_rate_rad > 0.07,
        "authored turn rate admits one sweep step"
    );
    assert!(
        object.weapon.is_some(),
        "template authors the actual primary weapon"
    );
    (world, owner)
}

fn admit_sweep(world: &mut GameLogic, owner: ObjectId, frame: u32, sweep: f32) {
    world.frame = frame;
    world.set_turret_target_position(owner, Some(glam::Vec3::new(20.0, 0.0, 0.0)));
    world.set_turret_fire_angle_sweep(owner, 0, sweep);
    world.notify_turret_fired(owner);
}

#[test]
fn receiving_turret_sweep_and_target_do_not_observe_same_id_peer() {
    let (mut world, owner) = receiving_world();
    admit_sweep(&mut world, owner, 10, 0.035);
    let (mut peer, peer_owner) = receiving_world();
    assert_eq!(owner, peer_owner);
    admit_sweep(&mut peer, peer_owner, 100, 0.07);
    peer.set_turret_target_position(peer_owner, Some(glam::Vec3::new(0.0, 0.0, 20.0)));
    let _ = peer.tick_turret_aim(peer_owner, 1.0);
    let _ = world.tick_turret_aim(owner, 1.0);
    let actual = world.host_object(owner).unwrap();
    assert!((actual.turret_angle_deg.to_radians() - 0.035).abs() < 0.0001);
    assert_eq!(actual.turret_runtime.enable_sweep_until, 13);
    assert!(!actual.turret_runtime.positive_sweep);
    assert_eq!(
        actual.turret_runtime.target_pos,
        Some(glam::Vec3::new(20.0, 0.0, 0.0))
    );
    let clone = actual.clone();
    peer.set_turret_target_position(peer_owner, None);
    drop(peer);
    assert_eq!(clone.turret_runtime.enable_sweep_until, 13);
    world.frame = 11;
    let _ = world.tick_turret_aim(owner, 1.0);
    assert!(world.host_object(owner).unwrap().turret_angle_deg < 0.0);
    world
        .host_object_mut(owner)
        .unwrap()
        .set_turret_target_object(None, false);
    assert_eq!(
        world.host_object(owner).unwrap().turret_target_kind(),
        HostTurretTargetKind::None
    );
    assert!(
        world
            .host_object(owner)
            .unwrap()
            .turret_runtime
            .target_pos
            .is_none()
    );
}

#[test]
fn idle_scan_random_draws_belong_to_each_receiving_world() {
    let (mut world, owner) = receiving_world();
    let (mut peer, peer_owner) = receiving_world();
    assert_eq!(owner, peer_owner);

    const SEED: u32 = 0x51A7_CAFE;
    for logic in [&mut world, &mut peer] {
        logic.logic_random.seed_random(SEED);
        logic.set_turret_idle_scan_params(owner, 0.2, 0.8, 0, 0);
    }

    // Idle entry consumes the interval draw, then its due transition enters
    // IdleScan on the following update. Interleave the same-ID peer between
    // those callbacks to detect process-global RNG contamination.
    assert_eq!(
        world.tick_turret_state_machine(owner, 0.0, 10),
        AttackAimResult::Continue
    );
    assert_eq!(
        peer.tick_turret_state_machine(peer_owner, 0.0, 10),
        AttackAimResult::Continue
    );
    assert_eq!(
        world.tick_turret_state_machine(owner, 0.0, 10),
        AttackAimResult::Continue
    );
    assert_eq!(
        peer.tick_turret_state_machine(peer_owner, 0.0, 10),
        AttackAimResult::Continue
    );

    let world_extra = &world.host_object(owner).unwrap().turret_runtime;
    let peer_extra = &peer.host_object(peer_owner).unwrap().turret_runtime;
    assert!(world_extra.idle_scan_entered && peer_extra.idle_scan_entered);
    assert_eq!(
        world_extra.idle_scan_desired_rad,
        peer_extra.idle_scan_desired_rad
    );

    let expected = game_engine::common::random_value::SessionRandom::default();
    expected.seed_random(SEED);
    assert_eq!(expected.next_int(0, 0), 0);
    let mut expected_angle = 0.2 + expected.next_real(0.0, 0.6);
    if expected.next_int(0, 1) == 0 {
        expected_angle = -expected_angle;
    }
    assert!((world_extra.idle_scan_desired_rad - expected_angle).abs() < 1e-6);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    assert_eq!(peer.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn idle_scan_keeps_disabled_and_zero_angle_rng_draws_suppressed() {
    let (mut world, owner) = receiving_world();
    world.logic_random.seed_random(0x1D1E_5CA1);
    world.set_turret_idle_scan_params(owner, 0.0, 0.0, 0, 0);
    let before = world.logic_random.seed_words();

    // Disabled TurretAI returns before the idle state machine.
    world.host_object_mut(owner).unwrap().turret_enabled = false;
    assert_eq!(
        world.tick_turret_state_machine(owner, 0.0, 10),
        AttackAimResult::Failure
    );
    assert_eq!(world.logic_random.seed_words(), before);

    // The classic zero-angle IdleScan entry transitions directly to HOLD
    // without drawing either the random offset or the random sign.
    {
        let unit = world.host_object_mut(owner).unwrap();
        unit.turret_enabled = true;
        unit.turret_substate = crate::game_logic::object::TurretSubState::IdleScan;
        unit.turret_runtime.idle_scan_entered = false;
    }
    assert_eq!(
        world.tick_turret_state_machine(owner, 0.0, 10),
        AttackAimResult::Continue
    );
    assert_eq!(world.logic_random.seed_words(), before);
}

#[test]
fn normal_encoded_save_continues_existing_sweep_before_another_shot() {
    let (mut original, owner) = receiving_world();
    admit_sweep(&mut original, owner, 10, 0.035);
    let _ = original.tick_turret_aim(owner, 1.0);
    assert!(
        !original
            .host_object(owner)
            .unwrap()
            .turret_runtime
            .positive_sweep
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = original.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut loaded)
        .unwrap();
    assert_eq!(
        loaded
            .host_object(owner)
            .unwrap()
            .turret_runtime
            .enable_sweep_until,
        13
    );
    assert!(
        !loaded
            .host_object(owner)
            .unwrap()
            .turret_runtime
            .positive_sweep
    );
    for frame in [11, 12, 13, 14] {
        original.frame = frame;
        loaded.frame = frame;
        let expected = original.tick_turret_aim(owner, 1.0);
        let actual = loaded.tick_turret_aim(owner, 1.0);
        assert_eq!(actual, expected, "frame {frame}");
        let original_owner = original.host_object(owner).unwrap();
        let loaded_owner = loaded.host_object(owner).unwrap();
        assert!(
            (original_owner.turret_angle_deg - loaded_owner.turret_angle_deg).abs() < 0.0001,
            "frame {frame}"
        );
        assert_eq!(
            original_owner.turret_runtime.positive_sweep,
            loaded_owner.turret_runtime.positive_sweep
        );
        assert_eq!(
            loaded_owner.turret_runtime.target_pos,
            original_owner.turret_runtime.target_pos
        );
        if frame == 11 {
            assert!(
                loaded_owner.turret_angle_deg < 0.0,
                "saved negative sweep applies without notifyFired"
            );
        }
        if frame >= 13 {
            assert!(
                loaded_owner.turret_angle_deg.abs() < 0.0001,
                "original three-frame sweep expiry"
            );
        }
    }
}

#[test]
fn receiving_object_target_captures_team_immediately_and_load_resolves_actual_victim() {
    let (mut world, owner) = receiving_world();
    let mut victim_template = ThingTemplate::new("OwnedTurretVictim");
    victim_template
        .add_kind_of(KindOf::Vehicle)
        .set_health(100.0);
    world
        .templates
        .insert("OwnedTurretVictim".into(), victim_template);
    let victim = world
        .create_object(
            "OwnedTurretVictim",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    world.set_turret_target_object(owner, Some(victim), false);
    assert_eq!(
        world.host_object(owner).unwrap().turret_runtime.victim_team,
        Some(Team::China)
    );
    assert_eq!(
        world.host_object(owner).unwrap().turret_target_kind(),
        HostTurretTargetKind::Object
    );
    world.set_turret_target_object(owner, Some(owner), true);
    assert_eq!(
        world.host_object(owner).unwrap().turret_runtime.victim_team,
        Some(Team::USA)
    );
    world.set_turret_target_object(owner, Some(victim), false);
    // Original loadPostProcess rereads current loaded team; it does not save
    // the pre-change surrogate alongside a target ID.
    world.host_object_mut(victim).unwrap().team = Team::GLA;
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = world.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    let actual_team = loaded.host_object(victim).unwrap().team;
    assert_eq!(actual_team, Team::GLA);
    assert_eq!(
        loaded
            .host_object(owner)
            .unwrap()
            .turret_runtime
            .victim_team,
        Some(actual_team)
    );
    assert_eq!(
        loaded.host_object(owner).unwrap().turret_target_id,
        Some(victim)
    );
}
