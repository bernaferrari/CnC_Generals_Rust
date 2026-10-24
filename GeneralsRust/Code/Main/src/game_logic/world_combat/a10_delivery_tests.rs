//! Actual receiving factory and callback boundaries; no Core registry or TLS fixtures.
use super::*;
use crate::game_logic::host_a10_strike_flight::{A10_DIVE_DIVING, A10_DIVE_PREDIVE, tick_a10_dive};
use crate::game_logic::special_power_strikes::{
    A10_PAYLOAD_TEMPLATE, A10_TRANSPORT, A10StrikeScienceTier,
};

fn fixture_world(cue: Option<&str>) -> GameLogic {
    let sound = cue
        .map(|cue| format!("UnitSpecificSounds\nStartDive = {cue}\nEnd\n"))
        .unwrap_or_default();
    let text = format!(
        "Object FlightSource\nKindOf = STRUCTURE\nBody = ActiveBody Body\nMaxHealth = 5000\nInitialHealth = 5000\nEnd\nEnd\nObject {A10_TRANSPORT}\nKindOf = VEHICLE AIRCRAFT\nBody = ActiveBody Body\nMaxHealth = 600\nInitialHealth = 600\nEnd\n{sound}End\nObject {A10_PAYLOAD_TEMPLATE}\nKindOf = PROJECTILE\nBody = ActiveBody Body\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "a10_delivery.ini".into(),
                text,
            )])
            .unwrap(),
        )
        .unwrap();
    let mut owner = Player::new(1, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    world.add_player(owner);
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    world.add_player(enemy);
    world
}

fn fixture(cue: Option<&str>) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = fixture_world(cue);
    let enemy_source = world
        .create_object_for_player("FlightSource", 2, Vec3::ZERO)
        .unwrap();
    let enemy_jet = world
        .spawn_a10_strike_flight(
            enemy_source,
            Vec3::new(180., 0., 0.),
            A10StrikeScienceTier::Level1,
        )
        .unwrap();
    let source = world
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_a10_strike_flight(
            source,
            Vec3::new(180., 0., 0.),
            A10StrikeScienceTier::Level1,
        )
        .unwrap();
    (world, jet, enemy_jet)
}

fn enter_dive(world: &mut GameLogic, jet: ObjectId) -> Vec3 {
    assert_eq!(
        world
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .dive_state,
        A10_DIVE_PREDIVE
    );
    for _ in 0..180 {
        let before = world.host_object(jet).unwrap().get_position();
        world.frame += 1;
        world.update_a10_strike_flights();
        if world
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .dive_state
            != A10_DIVE_PREDIVE
        {
            assert_eq!(
                world
                    .host_object(jet)
                    .unwrap()
                    .a10_strike_transport
                    .as_ref()
                    .unwrap()
                    .dive_state,
                A10_DIVE_DIVING
            );
            return before;
        }
    }
    panic!("actual receiving flight never entered dive");
}

#[test]
fn actual_flight_cue_uses_receiving_pre_movement_pose_once_with_same_id_peer_and_silence() {
    let (mut owner, jet, _) = fixture(Some("OwnerDive"));
    let (mut peer, peer_jet, _) = fixture(Some("PeerDive"));
    let (mut silent, silent_jet, _) = fixture(None);
    assert_eq!(jet, peer_jet);
    assert_eq!(jet, silent_jet);
    let peer_position = peer.host_object(peer_jet).unwrap().get_position();
    let before = enter_dive(&mut owner, jet);
    let own = owner
        .queued_audio_events
        .iter()
        .filter(|e| e.event_type == "OwnerDive")
        .collect::<Vec<_>>();
    assert_eq!(
        own.len(),
        2,
        "both actual same-template flights enter their own dive"
    );
    assert_eq!(own[0].position, Some(before));
    assert_ne!(
        owner.host_object(jet).unwrap().get_position(),
        before,
        "movement follows the cue phase"
    );
    assert!(
        own.iter()
            .all(|e| e.object_id.is_none() && e.player_index.is_none())
    );
    assert_eq!(
        peer.host_object(peer_jet).unwrap().get_position(),
        peer_position
    );
    assert!(peer.queued_audio_events.is_empty());
    for _ in 0..4 {
        owner.frame += 1;
        owner.update_a10_strike_flights();
    }
    assert_eq!(
        owner
            .queued_audio_events
            .iter()
            .filter(|e| e.event_type == "OwnerDive")
            .count(),
        2
    );
    enter_dive(&mut peer, peer_jet);
    assert_eq!(
        peer.queued_audio_events
            .iter()
            .filter(|e| e.event_type == "PeerDive")
            .count(),
        2
    );
    assert!(
        !peer
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "OwnerDive")
    );
    enter_dive(&mut silent, silent_jet);
    assert!(silent.queued_audio_events.is_empty());
}

#[test]
fn earlier_actual_flight_lethal_damage_prevents_later_flight_movement_in_same_update() {
    let (mut owner, jet, later) = fixture(Some("OwnedDive"));
    let (peer, peer_jet, peer_later) = fixture(Some("PeerDive"));
    assert_eq!((jet, later), (peer_jet, peer_later));
    enter_dive(&mut owner, jet);
    let actor = owner.host_object(jet).unwrap();
    let data = actor.a10_strike_transport.as_ref().unwrap();
    let mut state = data.dive_state;
    let tick = tick_a10_dive(
        &mut state,
        actor.get_position(),
        data.target,
        actor.movement.velocity,
        22.,
    );
    assert!(tick.should_strafe);
    {
        let (actor, fixture_partition_actor) = owner.host_object_and_partition_mut(later).unwrap();
        actor.set_position_with_partition_owner(
            tick.strafe_point,
            0,
            &mut Default::default(),
            fixture_partition_actor,
        );
        actor.health.current = 1.;
    }
    let order = owner
        .host_objects()
        .registered()
        .filter(|(_, o)| o.a10_strike_transport.is_some())
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [jet, later],
        "test observes the actual receiving visitation order, not an ID sort"
    );
    let peer_pose = peer.host_object(peer_later).unwrap().get_position();
    let prior_drop_count = owner
        .host_object(later)
        .unwrap()
        .a10_strike_transport
        .as_ref()
        .unwrap()
        .missiles_remaining;
    owner.frame += 1;
    owner.update_a10_strike_flights();
    let dead = owner.host_object(later).unwrap();
    assert_eq!(
        dead.last_damage_source,
        Some(jet),
        "C++ fireCurrentWeapon attributes the strafe to this aircraft, not its command center"
    );
    assert!(
        dead.status.destroyed,
        "real earlier Vulcan damage completes the later source's death"
    );
    assert_eq!(
        dead.get_position(),
        tick.strafe_point,
        "a dead later source must not already have moved in a previous batch"
    );
    if let Some(data) = dead.a10_strike_transport.as_ref() {
        assert_eq!(data.missiles_remaining, prior_drop_count);
    }
    assert_eq!(
        peer.host_object(peer_later).unwrap().get_position(),
        peer_pose
    );
    assert!(peer.host_object(peer_later).unwrap().is_alive());
}

#[test]
fn actual_visible_payload_uses_pre_movement_bones_and_explicit_receiving_controller() {
    let (mut world, jet, _) = fixture(None);
    let before = world.host_object(jet).unwrap().get_position();
    let yaw = world.host_object(jet).unwrap().get_orientation();
    let expected = [1, 2].map(|slot| {
        crate::game_logic::host_a10_strike_drop_log::a10_weapon_a_world_pos(before, yaw, slot)
    });
    world.frame += 1;
    world.update_a10_strike_flights();
    let mut missiles = world
        .host_objects()
        .registered()
        .filter(|(_, o)| o.a10_strike_missile && o.producer_id == Some(jet))
        .map(|(id, o)| (*id, o))
        .collect::<Vec<_>>();
    missiles.sort_by_key(|(id, _)| *id);
    assert_eq!(missiles.len(), 2);
    for ((_, missile), position) in missiles.into_iter().zip(expected) {
        assert_eq!(missile.owner_player_id, Some(1));
        assert!(
            missile.get_position().distance(position) < 0.001,
            "same-frame missile integration returns to original pre-movement drop bone"
        );
    }
    assert_ne!(world.host_object(jet).unwrap().get_position(), before);
    assert_eq!(
        world
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        4
    );
}

#[test]
fn actual_payload_intervals_wait_through_n_plus_fifteen_and_fire_at_sixteen_across_wrap() {
    for first in [20, u32::MAX - 8] {
        let (mut world, jet, _) = fixture(None);
        world.frame = first;
        world.update_a10_strike_flights();
        assert_eq!(
            world
                .host_object(jet)
                .unwrap()
                .a10_strike_transport
                .as_ref()
                .unwrap()
                .missiles_remaining,
            4
        );
        for elapsed in 1..=15 {
            world.frame = first.wrapping_add(elapsed);
            world.update_a10_strike_flights();
            assert_eq!(
                world
                    .host_object(jet)
                    .unwrap()
                    .a10_strike_transport
                    .as_ref()
                    .unwrap()
                    .missiles_remaining,
                4,
                "no delivery through N+15 (N={first}, elapsed={elapsed})"
            );
        }
        world.frame = first.wrapping_add(16);
        world.update_a10_strike_flights();
        assert_eq!(
            world
                .host_object(jet)
                .unwrap()
                .a10_strike_transport
                .as_ref()
                .unwrap()
                .missiles_remaining,
            2
        );
    }
}

#[test]
fn two_actual_deliveries_credit_the_exact_newer_strike_and_leave_older_and_peer_untouched() {
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;
    let (mut world, standalone, _) = fixture(None);
    let (peer, peer_standalone, _) = fixture(None);
    assert_eq!(standalone, peer_standalone);
    let source = world.host_object(standalone).unwrap().producer_id.unwrap();
    world.override_world_size(4000., 4000.);
    let older_target = Vec3::ZERO;
    let newer_target = Vec3::new(-1800., 0., 0.);
    let older = world
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source,
            older_target,
        )
        .unwrap();
    let newer = world
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source,
            newer_target,
        )
        .unwrap();
    let flight_for = |target| {
        world
            .host_objects()
            .registered_values()
            .find(|object| {
                object.producer_id == Some(source)
                    && object
                        .a10_strike_transport
                        .as_ref()
                        .is_some_and(|data| data.target == target)
            })
            .map(|object| object.id)
            .unwrap()
    };
    let older_jet = flight_for(older_target);
    let newer_jet = flight_for(newer_target);
    let older_delivery = world
        .a10_strike_flight_reg
        .delivery_for_transport(older_jet)
        .unwrap();
    let newer_delivery = world
        .a10_strike_flight_reg
        .delivery_for_transport(newer_jet)
        .unwrap();
    assert_ne!(older_delivery, newer_delivery);
    assert_eq!(
        world
            .a10_strike_flight_reg
            .delivery_strike_id(older_delivery),
        Some(older)
    );
    assert_eq!(
        world
            .a10_strike_flight_reg
            .delivery_strike_id(newer_delivery),
        Some(newer)
    );
    let peer_position = peer.host_object(peer_standalone).unwrap().get_position();
    for _ in 0..2 {
        world.frame += 1;
        world.update_a10_strike_flights();
    }
    assert_eq!(
        world
            .special_power_strikes
            .get(older)
            .unwrap()
            .multi_strike_applied,
        0,
        "the older live delivery is still outside dive range; another flight cannot credit it"
    );
    assert!(
        world
            .special_power_strikes
            .get(newer)
            .unwrap()
            .multi_strike_applied
            > 0,
        "the actual newer delivery admitted its own real strafe callback"
    );
    assert_eq!(
        peer.host_object(peer_standalone).unwrap().get_position(),
        peer_position
    );
    assert!(
        peer.special_power_strikes
            .pending_of_kind(HostSuperweaponKind::A10Strike)
            .is_empty()
    );
}

#[test]
fn actual_delivery_killed_before_any_payload_or_strafe_cancels_without_synthetic_impact() {
    use crate::game_logic::special_power_strikes::{HostStrikePhase, HostSuperweaponKind};
    let (mut world, standalone, _) = fixture(None);
    let source = world.host_object(standalone).unwrap().producer_id.unwrap();
    world.override_world_size(4000., 4000.);
    let strike = world
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source,
            Vec3::ZERO,
        )
        .unwrap();
    let jet = world
        .host_objects()
        .registered_values()
        .find(|object| {
            object.producer_id == Some(source)
                && object
                    .a10_strike_transport
                    .as_ref()
                    .is_some_and(|data| data.target == Vec3::ZERO)
        })
        .map(|object| object.id)
        .unwrap();
    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    let delivery = world
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    let killed = world
        .apply_owned_kill_with_effects(
            jet,
            crate::game_logic::combat::DamageType::Explosive,
            crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
            &mut effects,
        )
        .expect("real receiving Body accepts kill");
    assert!(killed.destroyed);
    assert!(world.host_object(jet).unwrap().status.on_die_started);
    // This minimal receiving Body has no authored DestroyDie. Complete the
    // distinct destructor phase explicitly, after proving real Body death.
    world.destroy_object_with_effects(jet, &mut effects);
    assert!(world.host_object(jet).unwrap().status.destroyed);
    world.process_destroy_list();
    assert!(
        world.host_object(jet).is_none(),
        "ordinary drain physically retires the killed producer"
    );
    world.frame += 1;
    world.update_a10_strike_flights();
    assert!(world.a10_strike_flight_reg.delivery_is_completed(delivery));
    let record = world.special_power_strikes.get(strike).unwrap();
    assert_eq!(record.phase, HostStrikePhase::Cancelled);
    assert_eq!(record.multi_strike_applied, 0);
    assert_eq!(record.objects_hit, 0);
    assert!(
        world
            .special_power_strikes
            .completed_of_kind(HostSuperweaponKind::A10Strike)
            .is_empty()
    );
}

#[test]
fn actual_same_faction_source_owns_its_jet_and_payload_instead_of_first_faction_slot() {
    let (mut world, _, _) = fixture(None);
    world.add_player(Player::new(3, Team::USA, "Second USA", false));
    let source = world
        .create_object_for_player("FlightSource", 3, Vec3::new(80., 0., 0.))
        .unwrap();
    let jet = world
        .spawn_a10_strike_flight(
            source,
            Vec3::new(180., 0., 0.),
            A10StrikeScienceTier::Level1,
        )
        .unwrap();
    assert_eq!(world.host_object(jet).unwrap().owner_player_id, Some(3));
    assert_eq!(
        world.host_object(jet).unwrap().team_instance_id,
        world.host_default_team_id(3)
    );
    world.frame += 1;
    world.update_a10_strike_flights();
    let payloads = world
        .host_objects()
        .registered_values()
        .filter(|o| o.a10_strike_missile && o.producer_id == Some(jet))
        .collect::<Vec<_>>();
    assert_eq!(payloads.len(), 2);
    for payload in payloads {
        assert_eq!(payload.owner_player_id, Some(3));
        assert_eq!(payload.team_instance_id, world.host_default_team_id(3));
    }
}

#[test]
fn actual_file_writer_preserves_a10_n_plus_fifteen_then_sixteen_future() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    let mut source = fixture_world(None);
    let producer = source
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let strike = source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            producer,
            Vec3::new(180., 0., 0.),
        )
        .unwrap();
    let jet = source
        .host_objects()
        .registered_values()
        .find(|object| {
            object.producer_id == Some(producer)
                && object.a10_strike_transport.is_some()
                && source
                    .a10_strike_flight_reg
                    .delivery_for_transport(object.id)
                    .and_then(|delivery| source.a10_strike_flight_reg.delivery_strike_id(delivery))
                    == Some(strike)
        })
        .map(|object| object.id)
        .unwrap();
    let first = 20_u32;
    for elapsed in 0..=15 {
        source.frame = first + elapsed;
        source.update_a10_strike_flights();
    }
    assert_eq!(
        source
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        4
    );
    let dir = tempfile::TempDir::new().unwrap();
    let mut saves = SaveFileManager::with_save_directory(dir.path());
    saves.init().unwrap();
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "a10_deadline".into(),
        display_name: "A10 deadline".into(),
        description: "Actual receiving delivery future".into(),
        map_name: "A10InlineNoMap".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    saves.save_game("a10_deadline", &source, &info).unwrap();
    let mut restored = fixture_world(None);
    saves.load_game("a10_deadline", &mut restored).unwrap();
    assert_eq!(restored.get_current_frame(), u64::from(first + 15));
    let saved_delivery = source
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    assert_eq!(
        restored.a10_strike_flight_reg.delivery_for_transport(jet),
        Some(saved_delivery)
    );
    assert_eq!(
        restored
            .a10_strike_flight_reg
            .delivery_strike_id(saved_delivery),
        Some(strike)
    );
    assert_eq!(
        restored
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        4
    );
    for world in [&mut source, &mut restored] {
        world.frame = first + 16;
        world.update_a10_strike_flights();
        assert_eq!(
            world
                .host_object(jet)
                .unwrap()
                .a10_strike_transport
                .as_ref()
                .unwrap()
                .missiles_remaining,
            2
        );
    }
    let actual = source.host_object(jet).unwrap();
    let loaded = restored.host_object(jet).unwrap();
    assert_eq!(actual.get_position(), loaded.get_position());
    assert_eq!(actual.movement.velocity, loaded.movement.velocity);
    assert_eq!(
        actual
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .next_drop_frame,
        loaded
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .next_drop_frame
    );
    assert_eq!(
        source
            .special_power_strikes
            .get(strike)
            .unwrap()
            .multi_strike_applied,
        restored
            .special_power_strikes
            .get(strike)
            .unwrap()
            .multi_strike_applied
    );
}

#[test]
fn actual_retired_a10_producer_reuse_cannot_receive_visible_missile_damage_credit() {
    let (mut world, jet, _) = fixture(None);
    let (peer, peer_jet, _) = fixture(None);
    let peer_pose = peer.host_object(peer_jet).unwrap().get_position();
    world.frame = 20;
    world.update_a10_strike_flights();
    let missile = world
        .host_objects()
        .registered_values()
        .find(|o| o.a10_strike_missile && o.producer_id == Some(jet))
        .map(|o| o.id)
        .unwrap();
    let delivery = world
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    world.destroy_object(jet);
    world.process_destroy_list();
    assert!(world.host_object(jet).is_none());
    let next = world.next_object_id_for_snapshot();
    world.set_next_object_id_for_restore(jet);
    let replacement = world
        .create_object_for_player("FlightSource", 1, Vec3::new(800., 0., 800.))
        .unwrap();
    assert_eq!(replacement, jet);
    world.set_next_object_id_for_restore(next);
    assert!(!world.a10_strike_flight_reg.transport_lifetime_matches(
        delivery,
        jet,
        &world.host_object(jet).unwrap().visual_draw_identity_token
    ));
    let victim = world
        .create_object_for_player("FlightSource", 2, Vec3::new(400., 0., 400.))
        .unwrap();
    world.objects.get_mut(&victim).unwrap().health.current = 1.;
    let (missile_actor, fixture_partition_missile_actor) =
        world.host_object_and_partition_mut(missile).unwrap();
    missile_actor.set_position_with_partition_owner(
        Vec3::new(400., 1., 400.),
        0,
        &mut Default::default(),
        fixture_partition_missile_actor,
    );
    missile_actor.movement.velocity = Vec3::ZERO;
    // Freeze before the runtime reconciles the departed capability. The old
    // ID still exists in the ledger, but the replacement must not be hydrated
    // as the retired transport when this real encoded boundary is restored.
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = fixture_world(None);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(
        !restored.a10_strike_flight_reg.transport_lifetime_matches(
            delivery,
            jet,
            &restored
                .host_object(jet)
                .unwrap()
                .visual_draw_identity_token
        )
    );
    for actual in [&mut world, &mut restored] {
        actual.frame += 1;
        actual.update_a10_strike_flights();
        let damaged = actual.host_object(victim).unwrap();
        assert!(damaged.status.destroyed);
        assert_eq!(damaged.last_damage_source, None);
        assert_eq!(
            actual.host_object(replacement).unwrap().experience.current,
            0.
        );
    }
    let damaged = world.host_object(victim).unwrap();
    assert!(damaged.status.destroyed);
    assert_eq!(
        damaged.last_damage_source, None,
        "C++ visible DeliverPayload FireAt leaves launcher INVALID; ID reuse must not invent a damager"
    );
    assert_eq!(
        world.host_object(replacement).unwrap().get_position(),
        Vec3::new(800., 0., 800.)
    );
    assert_eq!(
        peer.host_object(peer_jet).unwrap().get_position(),
        peer_pose
    );
}

#[test]
fn actual_file_writer_accepts_second_payload_interval_and_preserves_next_future() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    let mut source = fixture_world(None);
    // A real extent admits the whole approach/exit domain. The default tiny
    // fixture extent would make x280 an immediate off-map destructor request.
    source.override_world_size(4000., 4000.);
    let producer = source
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let target = Vec3::new(500.0, 0.0, 500.0);
    // Target is on the ground; the aircraft remains genuinely airborne and
    // outside the target's blast footprint throughout this deadline fixture.
    let airborne = target + Vec3::new(100.0, 160.0, 0.0);
    let strike = source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            producer,
            target,
        )
        .unwrap();
    let jet = source
        .host_objects()
        .registered_values()
        .find(|object| {
            object.producer_id == Some(producer)
                && object.a10_strike_transport.is_some()
                && source
                    .a10_strike_flight_reg
                    .delivery_for_transport(object.id)
                    .and_then(|delivery| source.a10_strike_flight_reg.delivery_strike_id(delivery))
                    == Some(strike)
        })
        .map(|object| object.id)
        .unwrap();
    let flight = source
        .host_object(jet)
        .unwrap()
        .a10_strike_transport
        .as_ref()
        .unwrap();
    assert_eq!(flight.map_min, Vec3::new(-2000., 0., -2000.));
    assert_eq!(flight.map_max, Vec3::new(2000., 0., 2000.));
    assert!(
        airborne.x > flight.map_min.x
            && airborne.x < flight.map_max.x
            && airborne.z > flight.map_min.z
            && airborne.z < flight.map_max.z
    );
    let first = 20_u32;
    for elapsed in 0..=16 {
        source.frame = first + elapsed;
        source.set_fixture_object_position(jet, airborne);
        source.update_a10_strike_flights();
        assert!(
            source.host_object(jet).unwrap().is_alive(),
            "source aircraft unexpectedly died at frame {}: {:?}",
            source.frame,
            source.host_object(jet).unwrap().last_damage_source
        );
    }
    let delivery = source
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    assert_eq!(
        source
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        2
    );
    assert_eq!(
        source.a10_strike_flight_reg.deliveries[&delivery].payload_attempts_consumed,
        4
    );

    let dir = tempfile::TempDir::new().unwrap();
    let mut saves = SaveFileManager::with_save_directory(dir.path());
    saves.init().unwrap();
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "a10_second_interval".into(),
        display_name: "A10 second interval".into(),
        description: "Actual second payload interval continuation".into(),
        map_name: "A10InlineNoMap".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    saves
        .save_game("a10_second_interval", &source, &info)
        .unwrap();
    let mut restored = fixture_world(None);
    saves
        .load_game("a10_second_interval", &mut restored)
        .unwrap();
    assert_eq!(restored.get_current_frame(), u64::from(first + 16));
    assert_eq!(
        restored.a10_strike_flight_reg.deliveries[&delivery].payload_attempts_consumed,
        4
    );
    assert_eq!(
        restored
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        2
    );

    for (owner, world) in [("source", &mut source), ("restored", &mut restored)] {
        assert!(
            world.host_object(jet).unwrap().is_alive(),
            "{owner}: live saved aircraft required"
        );
        assert_eq!(
            world
                .host_object(jet)
                .unwrap()
                .a10_strike_transport
                .as_ref()
                .unwrap()
                .next_drop_frame,
            first + 32,
            "{owner}: absolute next-attempt deadline"
        );
        assert!(
            world.a10_strike_flight_reg.transport_lifetime_matches(
                delivery,
                jet,
                &world.host_object(jet).unwrap().visual_draw_identity_token
            ),
            "{owner}: original admitted lifetime"
        );
        world.frame = first + 32;
        world.set_fixture_object_position(jet, airborne);
        world.update_a10_strike_flights();
        assert!(
            world.host_object(jet).unwrap().is_alive(),
            "{owner}: third-attempt aircraft survived"
        );
        assert_eq!(
            world
                .host_object(jet)
                .unwrap()
                .a10_strike_transport
                .as_ref()
                .unwrap()
                .missiles_remaining,
            0,
            "{owner}: third actual payload interval"
        );
    }
    assert_eq!(
        source.a10_strike_flight_reg.deliveries[&delivery].payload_attempts_consumed,
        6
    );
    assert_eq!(
        restored.a10_strike_flight_reg.deliveries[&delivery].payload_attempts_consumed,
        6
    );
    assert_eq!(
        source
            .special_power_strikes
            .get(strike)
            .unwrap()
            .multi_strike_applied,
        restored
            .special_power_strikes
            .get(strike)
            .unwrap()
            .multi_strike_applied
    );
    assert_eq!(
        source.a10_strike_flight_reg.deliveries[&delivery].impacts,
        restored.a10_strike_flight_reg.deliveries[&delivery].impacts
    );
}

#[test]
fn encoded_world_snapshot_continues_live_missiles_after_transport_death() {
    let mut source = fixture_world(None);
    let producer = source
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let target = Vec3::new(180.0, 0.0, 0.0);
    let strike = source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            producer,
            target,
        )
        .unwrap();
    let jet = source
        .host_objects()
        .registered_values()
        .find(|object| {
            object.producer_id == Some(producer)
                && source
                    .a10_strike_flight_reg
                    .delivery_for_transport(object.id)
                    .and_then(|delivery| source.a10_strike_flight_reg.delivery_strike_id(delivery))
                    == Some(strike)
        })
        .map(|object| object.id)
        .unwrap();
    let delivery = source
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    source.set_fixture_object_position(jet, target + Vec3::new(100., 160., 0.));
    source.frame = 1;
    source.update_a10_strike_flights();
    assert!(
        !source
            .a10_strike_flight_reg
            .delivery_missile_ids(delivery)
            .is_empty()
    );

    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    assert!(
        source.a10_strike_flight_reg.deliveries[&delivery]
            .active_missiles
            .iter()
            .all(|id| source
                .host_object(*id)
                .is_some_and(|actor| actor.is_alive() && actor.get_position().y > 5.)),
        "this is an actual in-flight boundary, not already detonated ground payload"
    );
    let killed = source
        .apply_owned_kill_with_effects(
            jet,
            crate::game_logic::combat::DamageType::Explosive,
            crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
            &mut effects,
        )
        .unwrap();
    assert!(killed.destroyed);
    assert!(source.host_object(jet).unwrap().status.on_die_started);
    // This minimal receiving Body has no authored DestroyDie. Complete the
    // distinct destructor phase explicitly, after proving real Body death.
    source.destroy_object_with_effects(jet, &mut effects);
    assert!(source.host_object(jet).unwrap().status.destroyed);
    source.process_destroy_list();
    assert!(source.host_object(jet).is_none());
    source.frame = 2;
    source.update_a10_strike_flights();
    assert!(
        source.a10_strike_flight_reg.deliveries[&delivery]
            .active_transports
            .is_empty()
    );
    assert!(
        !source.a10_strike_flight_reg.deliveries[&delivery]
            .active_missiles
            .is_empty()
    );

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = fixture_world(None);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(
        restored.a10_strike_flight_reg.deliveries[&delivery]
            .active_transports
            .is_empty()
    );
    assert_eq!(
        restored.a10_strike_flight_reg.deliveries[&delivery].active_missiles,
        source.a10_strike_flight_reg.deliveries[&delivery].active_missiles
    );

    for frame in 3..=40 {
        source.frame = frame;
        restored.frame = frame;
        source.update_a10_strike_flights();
        restored.update_a10_strike_flights();
        assert_eq!(
            source
                .special_power_strikes
                .get(strike)
                .unwrap()
                .multi_strike_applied,
            restored
                .special_power_strikes
                .get(strike)
                .unwrap()
                .multi_strike_applied
        );
        assert_eq!(
            source.a10_strike_flight_reg.deliveries[&delivery].impacts,
            restored.a10_strike_flight_reg.deliveries[&delivery].impacts
        );
    }
}

#[test]
fn encoded_world_snapshot_keeps_two_live_a10_deliveries_independent() {
    let mut source = fixture_world(None);
    let producer = source
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let first_target = Vec3::new(100.0, 0.0, 0.0);
    let second_target = Vec3::new(-100.0, 0.0, 0.0);
    let first = source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            producer,
            first_target,
        )
        .unwrap();
    let second = source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            producer,
            second_target,
        )
        .unwrap();
    let find_jet = |world: &GameLogic, strike| {
        world
            .host_objects()
            .registered_values()
            .find(|object| {
                object.producer_id == Some(producer)
                    && world
                        .a10_strike_flight_reg
                        .delivery_for_transport(object.id)
                        .and_then(|delivery| {
                            world.a10_strike_flight_reg.delivery_strike_id(delivery)
                        })
                        == Some(strike)
            })
            .map(|object| object.id)
            .unwrap()
    };
    let first_jet = find_jet(&source, first);
    let second_jet = find_jet(&source, second);
    let first_delivery = source
        .a10_strike_flight_reg
        .delivery_for_transport(first_jet)
        .unwrap();
    let second_delivery = source
        .a10_strike_flight_reg
        .delivery_for_transport(second_jet)
        .unwrap();
    assert_ne!(first_delivery, second_delivery);
    source.set_fixture_object_position(first_jet, first_target);
    source.set_fixture_object_position(second_jet, second_target);
    source.frame = 1;
    source.update_a10_strike_flights();

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = fixture_world(None);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored
            .a10_strike_flight_reg
            .delivery_for_transport(first_jet),
        Some(first_delivery)
    );
    assert_eq!(
        restored
            .a10_strike_flight_reg
            .delivery_for_transport(second_jet),
        Some(second_delivery)
    );

    for frame in 2..=8 {
        for world in [&mut source, &mut restored] {
            world.frame = frame;
            if world
                .host_object(first_jet)
                .is_some_and(|object| object.is_alive())
            {
                world.set_fixture_object_position(first_jet, first_target);
            }
            if world
                .host_object(second_jet)
                .is_some_and(|object| object.is_alive())
            {
                world.set_fixture_object_position(second_jet, second_target);
            }
            world.update_a10_strike_flights();
        }
        assert_eq!(
            source
                .special_power_strikes
                .get(first)
                .unwrap()
                .multi_strike_applied,
            restored
                .special_power_strikes
                .get(first)
                .unwrap()
                .multi_strike_applied
        );
        assert_eq!(
            source
                .special_power_strikes
                .get(second)
                .unwrap()
                .multi_strike_applied,
            restored
                .special_power_strikes
                .get(second)
                .unwrap()
                .multi_strike_applied
        );
        assert_eq!(
            source.a10_strike_flight_reg.deliveries[&first_delivery].impacts,
            restored.a10_strike_flight_reg.deliveries[&first_delivery].impacts
        );
        assert_eq!(
            source.a10_strike_flight_reg.deliveries[&second_delivery].impacts,
            restored.a10_strike_flight_reg.deliveries[&second_delivery].impacts
        );
    }
}

#[test]
fn actual_same_id_same_role_replacement_is_not_a_historical_delivery_transport() {
    let mut world = fixture_world(None);
    let source = world
        .create_object_for_player("FlightSource", 1, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_a10_strike_flight(
            source,
            Vec3::new(180., 0., 0.),
            A10StrikeScienceTier::Level1,
        )
        .unwrap();
    let old_data = world
        .host_object(jet)
        .unwrap()
        .a10_strike_transport
        .clone()
        .unwrap();
    let delivery = world
        .a10_strike_flight_reg
        .delivery_for_transport(jet)
        .unwrap();
    world.destroy_object(jet);
    world.process_destroy_list();
    let next = world.next_object_id_for_snapshot();
    world.set_next_object_id_for_restore(jet);
    let replacement = world
        .create_object_for_player(A10_TRANSPORT, 1, Vec3::new(180., 160., 0.))
        .unwrap();
    world.set_next_object_id_for_restore(next);
    assert_eq!(replacement, jet);
    // A real factory-born same-template actor can carry the same residual role
    // without becoming the departed lifetime's delivery capability.
    world.objects.get_mut(&jet).unwrap().a10_strike_transport = Some(old_data);
    let pose = world.host_object(jet).unwrap().get_position();
    let missiles = world.a10_strike_flight_reg.missiles_dropped;
    let before_remaining = world
        .host_object(jet)
        .unwrap()
        .a10_strike_transport
        .as_ref()
        .unwrap()
        .missiles_remaining;
    world.frame = 20;
    world.update_a10_strike_flights();
    assert_eq!(world.host_object(jet).unwrap().get_position(), pose);
    assert_eq!(
        world
            .host_object(jet)
            .unwrap()
            .a10_strike_transport
            .as_ref()
            .unwrap()
            .missiles_remaining,
        before_remaining
    );
    assert_eq!(world.a10_strike_flight_reg.missiles_dropped, missiles);
    assert!(!world.a10_strike_flight_reg.transport_lifetime_matches(
        delivery,
        jet,
        &world.host_object(jet).unwrap().visual_draw_identity_token
    ));
}
