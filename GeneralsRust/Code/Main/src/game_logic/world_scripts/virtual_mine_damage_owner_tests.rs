//! CPP detonateOnce's guarded Body drain and actual saved continuation.
use super::*;

fn mine_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let id = world
        .place_land_mine_named("ChinaStandardMine", Team::China, Vec3::ZERO, None)
        .unwrap();
    assert!(
        world.objects[&id].default_auto_heal.is_none(),
        "retail mines are not trainable and do not inherit the separate unit-veterancy healer"
    );
    (world, id)
}

#[test]
fn virtual_detonation_runs_guarded_body_callbacks_once_in_receiving_world() {
    let (mut world, id) = mine_world();
    let (peer, peer_id) = mine_world();
    assert_eq!(id, peer_id);
    world.frame = 77;
    let before = world.objects[&id]
        .mine_data
        .as_ref()
        .unwrap()
        .virtual_mines_remaining;
    assert!(world.trip_virtual_land_mine(id, Vec3::ZERO));
    let mine = &world.objects[&id];
    let data = mine.mine_data.as_ref().unwrap();
    assert_eq!(data.virtual_mines_remaining, before - 1);
    assert!(!data.ignore_damage);
    assert_eq!(
        data.auto_heal_wake_frame,
        77 + crate::game_logic::host_mines::MINE_AUTO_HEAL_START_DELAY_FRAMES
    );
    assert_eq!(data.last_synced_health, Some(mine.health.current));
    assert_eq!(mine.last_damage_source, Some(id));
    assert_eq!(mine.last_damage_timestamp, Some(77));
    assert_eq!(world.mine_residual_proximity_detonations, 1);
    assert_eq!(
        peer.objects[&id]
            .mine_data
            .as_ref()
            .unwrap()
            .virtual_mines_remaining,
        before
    );

    assert_eq!(peer.mine_residual_proximity_detonations, 0);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let (mut restored, restored_id) = mine_world();
    assert_eq!(id, restored_id);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.objects[&id]
            .mine_data
            .as_ref()
            .unwrap()
            .ignore_damage,
        false
    );
    let drained_health = world.objects[&id].health.current;
    for frame in [78, 526, 527, 676, 677] {
        world.frame = frame;
        restored.frame = frame;
        world.update_mines_and_demo_traps();
        restored.update_mines_and_demo_traps();
        let pulses = if frame < 527 {
            0.0
        } else if frame < 677 {
            1.0
        } else {
            2.0
        };
        assert_eq!(
            world.objects[&id].health.current,
            drained_health + pulses * crate::game_logic::host_mines::MINE_AUTO_HEAL_AMOUNT,
            "retail 450-frame damage delay then 150-frame healing interval"
        );
        assert_eq!(
            world.objects[&id].health.current,
            restored.objects[&id].health.current
        );
        assert_eq!(
            world.objects[&id]
                .mine_data
                .as_ref()
                .unwrap()
                .virtual_mines_remaining,
            restored.objects[&id]
                .mine_data
                .as_ref()
                .unwrap()
                .virtual_mines_remaining
        );
        assert_eq!(
            world.objects[&id]
                .mine_data
                .as_ref()
                .unwrap()
                .auto_heal_wake_frame,
            restored.objects[&id]
                .mine_data
                .as_ref()
                .unwrap()
                .auto_heal_wake_frame
        );
    }
    assert_eq!(peer.mine_residual_proximity_detonations, 0);
}

#[test]
fn regenerating_disarm_preserves_real_damage_callback_delay_without_blast() {
    let (mut world, id) = mine_world();
    let (peer, peer_id) = mine_world();
    assert_eq!(id, peer_id);
    world.frame = 170;
    assert!(world.clear_mine_internal(id, ObjectId(99001)));
    let mine = &world.objects[&id];
    let data = mine.mine_data.as_ref().unwrap();
    assert_eq!(data.virtual_mines_remaining, 0);
    assert!(!data.ignore_damage);
    assert!((mine.health.current - crate::game_logic::host_mines::MINE_MIN_HEALTH).abs() < 0.0001);
    assert_eq!(
        data.auto_heal_wake_frame,
        170 + crate::game_logic::host_mines::MINE_AUTO_HEAL_START_DELAY_FRAMES
    );
    assert_eq!(mine.last_damage_source, Some(id));
    assert_eq!(mine.last_damage_timestamp, Some(170));
    assert!(mine.status.masked);
    assert_eq!(world.mine_residual_clears, 1);
    assert_eq!(world.mine_residual_proximity_detonations, 0);
    assert!(peer.objects[&id].health.current > mine.health.current);

    assert_eq!(peer.mine_residual_clears, 0);
}

#[test]
fn last_virtual_charge_fires_once_before_direct_retirement_without_self_damage() {
    let (mut world, id) = mine_world();
    let (peer, _) = mine_world();
    let victim_id = ObjectId(99111);
    let mut template = ThingTemplate::new("MineBlastTarget");
    template.set_health(1000.0);
    let mut victim = Object::new(template, victim_id, Team::USA);
    victim.set_position(Vec3::X);
    world.objects.insert(victim_id, victim);
    let producer_id = ObjectId(99112);
    let mut producer_template = ThingTemplate::new("CapturedMineProducer");
    producer_template.set_health(1000.0);
    let mut producer = Object::new(producer_template, producer_id, Team::USA);
    producer.set_position(Vec3::X);
    world.objects.insert(producer_id, producer);
    {
        let mine = world.objects.get_mut(&id).unwrap();
        let data = mine.mine_data.as_mut().unwrap();
        data.regenerates = false;
        data.num_virtual_mines = 1;
        data.virtual_mines_remaining = 1;
        data.producer_id = Some(producer_id);
        data.detonation_damage = 7.0;
        data.detonation_radius = 20.0;
    }
    world.frame = 211;
    assert!(world.trip_virtual_land_mine(id, Vec3::ZERO));
    assert_eq!(world.mine_residual_proximity_detonations, 1);
    assert!(
        world.objects[&id].status.destroyed,
        "CPP deletion marks status while canonical lookup remains available"
    );
    assert_eq!(world.objects[&victim_id].health.current, 993.0);
    assert_eq!(
        world.objects[&producer_id].health.current, 1000.0,
        "CPP non-SELF splash excludes original producer even after its team changes"
    );
    let mine = &world.objects[&id];
    assert_eq!(mine.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);
    assert_eq!(
        mine.last_damage_timestamp, None,
        "weapon excludes SELF, direct delete cannot fabricate Body damage"
    );
    assert!(
        !mine.status.on_die_started,
        "CPP terminal detonateOnce requests deletion independently of onDie"
    );
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    assert_eq!(peer.mine_residual_proximity_detonations, 0);
    assert!(
        peer.objects[&id]
            .mine_data
            .as_ref()
            .unwrap()
            .virtual_mines_remaining
            > 0
    );
    world.process_destroy_list();
    assert!(
        world.host_object(id).is_none(),
        "physical removal follows the receiving deletion phase"
    );
}

#[test]
fn reciprocal_mine_callbacks_observe_source_charge_before_outer_decrement() {
    let (mut world, first) = mine_world();
    let (peer, _) = mine_world();
    let second = world
        .place_land_mine_named(
            "ChinaStandardMine",
            Team::USA,
            Vec3::new(10.0, 0.0, 0.0),
            None,
        )
        .unwrap();
    for id in [first, second] {
        let object = world.objects.get_mut(&id).unwrap();
        object.internal_change_health(10.0 - object.health.current);
        object.health.maximum = 10.0;
        object.max_health = 10.0;
        let data = object.mine_data.as_mut().unwrap();
        data.num_virtual_mines = 2;
        data.virtual_mines_remaining = 2;
        data.last_synced_health = Some(10.0);
        data.detonation_damage = 100.0;
    }
    world.frame = 250;
    assert!(world.trip_virtual_land_mine(first, Vec3::ZERO));
    // Outer A fires with TWO charges still visible. B's immediate onDamage
    // fires back; A completes both nested charges, then B completes its two.
    // The original outer A call finally decrements the already-empty count.
    assert_eq!(world.mine_residual_proximity_detonations, 5);
    for id in [first, second] {
        let object = &world.objects[&id];
        let data = object.mine_data.as_ref().unwrap();
        assert_eq!(data.virtual_mines_remaining, 0);
        assert!(!data.ignore_damage);
        assert!(
            (object.health.current - crate::game_logic::host_mines::MINE_MIN_HEALTH).abs() < 0.0001
        );
        assert!(object.status.masked);
        assert!(!object.status.on_die_started);
    }
    assert_eq!(peer.mine_residual_proximity_detonations, 0);
    assert_eq!(
        peer.objects[&first]
            .mine_data
            .as_ref()
            .unwrap()
            .virtual_mines_remaining,
        crate::game_logic::host_mines::STANDARD_MINE_NUM_VIRTUAL
    );
}

#[test]
fn authored_standard_mine_weapon_does_not_chain_allied_pad() {
    let (mut world, first) = mine_world();
    let second = world
        .place_land_mine_named(
            "ChinaStandardMine",
            Team::China,
            Vec3::new(10.0, 0.0, 0.0),
            None,
        )
        .unwrap();
    let before_health = world.objects[&second].health.current;
    let before_count = world.objects[&second]
        .mine_data
        .as_ref()
        .unwrap()
        .virtual_mines_remaining;
    world.frame = 260;
    assert!(world.trip_virtual_land_mine(first, Vec3::ZERO));
    assert_eq!(world.mine_residual_proximity_detonations, 1);
    assert_eq!(world.objects[&second].health.current, before_health);
    assert_eq!(
        world.objects[&second]
            .mine_data
            .as_ref()
            .unwrap()
            .virtual_mines_remaining,
        before_count,
        "retail StructureMineWeapon affects ENEMIES NEUTRALS, never allied pads"
    );
    assert_eq!(world.objects[&second].last_damage_timestamp, None);
}

#[test]
fn periodic_mine_healing_runs_body_poison_and_floor_callbacks_before_next_actor() {
    let (mut world, id) = mine_world();
    let (peer, _) = mine_world();
    {
        let mine = world.objects.get_mut(&id).unwrap();
        mine.internal_change_health(50.0 - mine.health.current);
        let data = mine.mine_data.as_mut().unwrap();
        data.virtual_mines_remaining = 4;
        data.last_synced_health = Some(50.0);
        data.auto_heal_wake_frame = 500;
        let mut poison =
            crate::game_logic::host_poisoned_behavior::HostPoisonedBehaviorData::default();
        poison.start_poisoned_effects(
            499,
            1.0,
            crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
        );
        mine.poisoned_behavior = Some(poison);
    }
    world.frame = 500;
    world.update_land_mine_regen_and_virtual();
    let mine = &world.objects[&id];
    assert_eq!(mine.health.current, 52.0);
    assert_eq!(mine.last_healing_timestamp, Some(500));
    assert_eq!(mine.last_damage_timestamp, Some(500));
    assert_eq!(mine.last_damage_source, Some(id));
    assert_eq!(
        mine.last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Healing)
    );
    assert!(!mine.poisoned_behavior.as_ref().unwrap().is_active());
    assert_eq!(
        mine.mine_data.as_ref().unwrap().virtual_mines_remaining,
        4,
        "onHealing uses FLOOR, not damage's CEIL"
    );
    assert_eq!(mine.mine_data.as_ref().unwrap().auto_heal_wake_frame, 650);
    assert_eq!(world.mine_residual_proximity_detonations, 0);
    assert_eq!(peer.objects[&id].last_healing_timestamp, None);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let (mut restored, _) = mine_world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    world.frame = 650;
    restored.frame = 650;
    world.update_land_mine_regen_and_virtual();
    restored.update_land_mine_regen_and_virtual();
    assert_eq!(world.objects[&id].health.current, 54.0);
    assert_eq!(restored.objects[&id].health.current, 54.0);
    assert_eq!(restored.objects[&id].last_healing_timestamp, Some(650));
}

#[test]
fn creator_disappearance_drains_through_body_without_detonating_virtual_charges() {
    let (mut world, id) = mine_world();
    let (peer, _) = mine_world();
    {
        let mine = world.objects.get_mut(&id).unwrap();
        let data = mine.mine_data.as_mut().unwrap();
        data.producer_id = Some(ObjectId(99991));
        data.next_death_check_frame = 300;
        data.last_synced_health = Some(mine.health.current);
    }
    world.frame = 300;
    world.update_land_mine_regen_and_virtual();
    let expected =
        100.0 - (100.0 * crate::game_logic::host_mines::MINE_DEGEN_PERCENT_PER_SECOND / 30.0);
    let mine = &world.objects[&id];
    assert!((mine.health.current - expected).abs() < 0.0001);
    assert_eq!(mine.last_damage_timestamp, Some(300));
    assert_eq!(mine.last_damage_source, Some(id));
    assert_eq!(
        mine.last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Unresistable)
    );
    assert!(mine.mine_data.as_ref().unwrap().draining);
    assert!(mine.mine_data.as_ref().unwrap().auto_heal_stopped);
    assert_eq!(world.mine_residual_proximity_detonations, 0);
    assert_eq!(peer.objects[&id].health.current, 100.0);
    assert!(!peer.objects[&id].mine_data.as_ref().unwrap().draining);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let (mut restored, _) = mine_world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for frame in [301, 302, 303, 304] {
        let previous_health = world.objects[&id].health.current;
        world.frame = frame;
        restored.frame = frame;
        world.update_land_mine_regen_and_virtual();
        restored.update_land_mine_regen_and_virtual();
        assert_eq!(
            world.objects[&id].health.current,
            restored.objects[&id].health.current
        );
        assert!(
            world.objects[&id].health.current < previous_health,
            "real Body drain occurs even when CPP retains previous damage history"
        );
        // CPP ActiveBody556 retains a valid prior source on the next frame
        // when incoming MINE is not Vehicle/Infantry/FactionStructure. The
        // next nonadjacent observation replaces the entire history record.
        let recorded_frame = if frame % 2 == 1 { frame - 1 } else { frame };
        assert_eq!(
            world.objects[&id].last_damage_timestamp,
            Some(recorded_frame)
        );
        assert_eq!(
            restored.objects[&id].last_damage_timestamp,
            Some(recorded_frame)
        );
        assert_eq!(world.objects[&id].last_damage_source, Some(id));
        assert_eq!(restored.objects[&id].last_damage_source, Some(id));
        assert_eq!(world.mine_residual_proximity_detonations, 0);
        assert_eq!(peer.objects[&id].health.current, 100.0);
        assert!(!peer.objects[&id].mine_data.as_ref().unwrap().draining);
    }
}

#[test]
fn normal_fixed_step_advances_only_the_authored_mine_healer_once() {
    let (mut world, id) = mine_world();
    {
        let mine = world.objects.get_mut(&id).unwrap();
        mine.internal_change_health(50.0 - mine.health.current);
        let data = mine.mine_data.as_mut().unwrap();
        data.virtual_mines_remaining = 4;
        data.last_synced_health = Some(50.0);
        data.auto_heal_wake_frame = 500;
    }
    world.frame = 500;
    world.tick_logic_frame(1.0 / 30.0, None, Some(1));
    assert_eq!(world.frame, 501, "actual fixed-step owner advanced once");
    let mine = &world.objects[&id];
    assert_eq!(
        mine.health.current, 52.0,
        "mine phase then generic auto-heal phase cannot duplicate the authored pulse"
    );
    assert_eq!(mine.last_healing_timestamp, Some(500));
    assert!(mine.default_auto_heal.is_none());
    assert_eq!(mine.mine_data.as_ref().unwrap().auto_heal_wake_frame, 650);
    assert_eq!(world.mine_residual_proximity_detonations, 0);
}

#[test]
fn authored_mine_deadlines_wrap_without_treating_zero_as_uninitialized() {
    let mut data =
        crate::game_logic::host_mines::HostMineData::land_mine_for_template("ChinaStandardMine");
    data.on_damage_at(u32::MAX - 449, 1.0);
    assert_eq!(
        data.auto_heal_wake_frame, 0,
        "CPP unsigned frame plus450 wraps to zero"
    );
    assert_eq!(
        data.tick_regen_auto_heal(u32::MAX - 448, 50.0, 100.0),
        52.0,
        "zero is a real due deadline, not a missing schedule sentinel"
    );
    assert_eq!(
        data.auto_heal_wake_frame,
        (u32::MAX - 448).wrapping_add(150)
    );
    data.auto_heal_wake_frame = u32::MAX - 10;
    assert_eq!(data.tick_regen_auto_heal(u32::MAX - 10, 50.0, 100.0), 52.0);
    assert_eq!(
        data.auto_heal_wake_frame, 139,
        "next authored pulse uses unsigned arithmetic too"
    );
}
