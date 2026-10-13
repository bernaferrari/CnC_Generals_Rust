//! Receiving shot clocks and one canonical FiringTracker across all lanes.
use super::*;

fn world(name: &str) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "FiringOwner", true));
    world.add_player(Player::new(2, Team::GLA, "FiringTarget", false));
    let mut source = ThingTemplate::new(name);
    source.set_health(1000.0).add_kind_of(KindOf::Vehicle);
    let mut target = ThingTemplate::new("ClockTarget");
    target.set_health(1000.0).add_kind_of(KindOf::Attackable);
    world.templates.insert(name.into(), source);
    world.templates.insert("ClockTarget".into(), target);
    let source = world.create_object_for_player(name, 1, Vec3::ZERO).unwrap();
    let first = world
        .create_object_for_player("ClockTarget", 2, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let second = world
        .create_object_for_player("ClockTarget", 2, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let actor = world.host_object_mut(source).unwrap();
    actor.weapon = Some(Weapon {
        damage: 1.0,
        range: 1000.0,
        ..Default::default()
    });
    actor.consecutive_shots_at_target = 4;
    actor.consecutive_shot_target = Some(first);
    actor.continuous_fire_level = 1;
    actor.continuous_fire_one_shots = 2;
    actor.continuous_fire_two_shots = 6;
    actor.continuous_fire_coast_frames = 30;
    actor.continuous_fire_coast_until_frame = 100;
    actor.pre_attack_ready_at = 5.0;
    (world, source, first, second)
}

fn commit(world: &mut GameLogic, source: ObjectId, target: ObjectId, frame: u32) {
    world.frame = frame;
    world.host_object_mut(source).unwrap().target = Some(target);
    world.commit_combat_shot(
        CombatAttackInput {
            attacker_id: source,
            current_time: frame as f32 / 30.0,
            attacker_team: Team::USA,
            target_id: Some(target),
            target_location: None,
            overcharge: false,
        },
        0,
    );
}

#[test]
fn actual_accepted_shots_use_receiving_clock_and_strict_pre_shot_coast_once() {
    for name in [
        "ClockOrdinary",
        "ChinaVehicleGattlingTank",
        "Infa_InfantryMiniGunner",
        "ChinaGattlingCannon",
    ] {
        let (mut a, source, _, target) = world(name);
        let (mut b, peer, _, peer_target) = world(name);
        assert_eq!((source, target), (peer, peer_target));
        // MiniGunner's authored Mean threshold is six (not the generic
        // fixture's two). A Mean tracker with four shots is an impossible
        // warm-up witness and correctly cools down on its next shot.
        let prior_count = if name == "Infa_InfantryMiniGunner" {
            7
        } else if name == "ChinaGattlingCannon" {
            6
        } else {
            4
        };
        // Cannon One=1 permits Mean to retain a newly switched count1;
        // use its authored Fast (>Two=5) lane to witness real demotion.
        let prior_level = if name == "ChinaGattlingCannon" { 2 } else { 1 };
        a.host_object_mut(source).unwrap().continuous_fire_level = prior_level;
        b.host_object_mut(peer).unwrap().continuous_fire_level = prior_level;
        a.host_object_mut(source)
            .unwrap()
            .consecutive_shots_at_target = prior_count;
        b.host_object_mut(peer).unwrap().consecutive_shots_at_target = prior_count;
        // B is at the exact old deadline; A is still inside its own coast.
        commit(&mut b, peer, peer_target, 100);
        commit(&mut a, source, target, 99);
        let actor = a.host_object(source).unwrap();
        assert_eq!(
            actor.consecutive_shots_at_target,
            prior_count + 1,
            "exactly one shot in {name}"
        );
        assert_eq!(actor.consecutive_shot_target, Some(target));
        assert_eq!(actor.continuous_fire_level, prior_level);
        assert_eq!(actor.pre_attack_ready_at, 0.0);
        let peer_actor = b.host_object(peer).unwrap();
        assert_eq!(
            peer_actor.consecutive_shots_at_target, 0,
            "CPP demotion resets tracker at strict deadline in {name}"
        );
        assert_eq!(peer_actor.consecutive_shot_target, None);
        assert_eq!(peer_actor.continuous_fire_level, 0);
        // A non-discharge may not advance the tracker.
        a.host_object_mut(source).unwrap().status.weapons_jammed = true;
        let before = a.host_object(source).unwrap().consecutive_shots_at_target;
        a.update_combat(&[source], 1.0 / 30.0);
        assert_eq!(
            a.host_object(source).unwrap().consecutive_shots_at_target,
            before
        );
    }
    // Cannon's One=1 is a distinct legitimate Mean boundary: retarget
    // restarts at one without demotion because CPP compares strictly <One.
    let (mut mean, source, _, target) = world("ChinaGattlingCannon");
    commit(&mut mean, source, target, 100);
    let actor = mean.host_object(source).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 1);
    assert_eq!(actor.consecutive_shot_target, Some(target));
    assert_eq!(actor.continuous_fire_level, 1);
}

#[test]
fn normal_world_codec_retains_specialized_next_shot_and_omits_idle_reload_timer() {
    let (mut original, source, _, target) = world("ChinaVehicleGattlingTank");
    commit(&mut original, source, target, 99);
    original
        .host_object_mut(source)
        .unwrap()
        .frame_to_force_reload = 101;
    original.host_object_mut(source).unwrap().weapon = Some(Weapon {
        damage: 1.0,
        range: 1000.0,
        clip_size: 6,
        ammo: Some(2),
        ..Default::default()
    });
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = original.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    assert_eq!(
        loaded
            .host_object(source)
            .unwrap()
            .consecutive_shots_at_target,
        5
    );
    assert_eq!(
        loaded.host_object(source).unwrap().consecutive_shot_target,
        Some(target)
    );
    assert_eq!(loaded.host_object(source).unwrap().continuous_fire_level, 1);
    assert_eq!(
        loaded
            .host_object(source)
            .unwrap()
            .continuous_fire_coast_until_frame,
        original
            .host_object(source)
            .unwrap()
            .continuous_fire_coast_until_frame
    );
    // CPP constructor zero plus xfer omission prevents idle reloading after
    // load. Exercise its actual update on a separate normally restored owner.
    let mut reload_owner = GameLogic::new();
    reload_owner.templates = original.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut reload_owner)
        .unwrap();
    reload_owner.frame = 150;
    reload_owner
        .host_object_mut(source)
        .unwrap()
        .tick_force_reload_when_idle(150);
    assert_eq!(
        reload_owner
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    for owner in [&mut original, &mut loaded] {
        commit(owner, source, target, 100);
        assert_eq!(
            owner
                .host_object(source)
                .unwrap()
                .consecutive_shots_at_target,
            6
        );
        assert_eq!(
            owner.host_object(source).unwrap().consecutive_shot_target,
            Some(target)
        );
        assert_eq!(owner.host_object(source).unwrap().continuous_fire_level, 1);
    }
}

#[test]
fn generic_tracker_arms_base_and_preserves_two_stage_strict_cooldown() {
    for delay in [0.0, 0.11] {
        for level in [0, 1, 2] {
            let (mut owner, source, target, _) = world("ClockOrdinary");
            {
                let actor = owner.host_object_mut(source).unwrap();
                actor.continuous_fire_level = level;
                actor.consecutive_shots_at_target = match level {
                    0 => 0,
                    1 => 3,
                    _ => 7,
                };
                actor.consecutive_shot_target = Some(target);
                actor.weapon.as_mut().unwrap().reload_time = delay;
            }
            let delay_frames = owner
                .host_object(source)
                .unwrap()
                .possible_next_shot_frame_for_slot(0, 100, false)
                - 100;
            commit(&mut owner, source, target, 100);
            let deadline = 100 + delay_frames + 30;
            assert_eq!(
                owner
                    .host_object(source)
                    .unwrap()
                    .continuous_fire_coast_until_frame,
                deadline
            );
            let mut sound =
                crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents::default();
            let (actor, health) = owner.host_object_and_health_events_mut(source).unwrap();
            let shot_level = actor.continuous_fire_level;
            actor.tick_continuous_fire_coast(deadline, &mut sound, health);
            assert_eq!(actor.continuous_fire_level, shot_level);
            actor.tick_continuous_fire_coast(deadline + 1, &mut sound, health);
            assert_eq!(actor.continuous_fire_level, 0);
            assert_eq!(actor.consecutive_shots_at_target, 0);
            assert_eq!(actor.consecutive_shot_target, None);
            if shot_level != 0 {
                assert_eq!(actor.continuous_fire_coast_until_frame, deadline + 31);
                actor.tick_continuous_fire_coast(deadline + 31, &mut sound, health);
                assert_eq!(actor.continuous_fire_coast_until_frame, deadline + 31);
                actor.tick_continuous_fire_coast(deadline + 32, &mut sound, health);
            }
            assert_eq!(actor.continuous_fire_coast_until_frame, 0);
        }
    }
    let (mut owner, source, target, _) = world("ClockOrdinary");
    owner
        .host_object_mut(source)
        .unwrap()
        .continuous_fire_coast_frames = 0;
    commit(&mut owner, source, target, 100);
    assert_eq!(
        owner
            .host_object(source)
            .unwrap()
            .continuous_fire_coast_until_frame,
        0
    );
}
