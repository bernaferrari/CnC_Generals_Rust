//! CPP ActiveBody.cpp:414; neutral Academy belongs to the receiving match.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::game_logic::PlayerSideRole;
use crate::game_logic::object::DamageHitContext;

fn world(frame: u32) -> (GameLogic, [ObjectId; 2]) {
    let mut world = GameLogic::new();
    world.frame = frame;
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    let mut observer = Player::new(9, Team::Neutral, "ReplayObserver", true);
    observer.map_side.role = PlayerSideRole::ReplayObserver;
    observer.map_side.map_player_name = "ReplayObserver".into();
    observer.is_observer = true;
    world.add_player(observer);
    world.add_player(Player::new(1, Team::USA, "Victim", true));
    world.add_player(Player::new(2, Team::GLA, "Sniper", false));
    let mut ids = Vec::new();
    for (name, player, kind, position) in [
        ("OwnedSniperVictim", 1, KindOf::Vehicle, Vec3::ZERO),
        (
            "OwnedSniperSource",
            2,
            KindOf::Infantry,
            Vec3::new(20.0, 0.0, 0.0),
        ),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(100.0).add_kind_of(kind);
        world.templates.insert(name.into(), template);
        ids.push(
            world
                .create_object_for_player(name, player, position)
                .unwrap(),
        );
    }
    (world, ids.try_into().unwrap())
}

fn snipe(world: &mut GameLogic, victim: ObjectId, source: ObjectId) {
    let context = DamageHitContext::new(world.host_object(source), None, DamageType::KillPilot);
    world
        .apply_owned_damage(
            victim,
            1.0,
            Some(source),
            DamageType::KillPilot,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
}
fn count(world: &GameLogic, player: u32) -> u32 {
    world
        .get_player(player)
        .unwrap()
        .academy_stats
        .get_vehicles_sniped()
}

#[test]
fn all_three_kill_pilot_vehicle_branches_update_actual_neutral_only() {
    for (rider, moving) in [(false, false), (true, false), (true, true)] {
        let (mut owner, [victim, source]) = world(100);
        let (peer, peer_ids) = world(700);
        assert_eq!([victim, source], peer_ids);
        let object = owner.host_object_mut(victim).unwrap();
        object.is_combat_cycle_transport = rider;
        object.status.moving = moving;
        snipe(&mut owner, victim, source);
        assert_eq!(count(&owner, 0), 1, "rider={rider} moving={moving}");
        assert_eq!(
            count(&owner, 9),
            0,
            "neutral faction observer is not reserved neutral"
        );
        assert_eq!(count(&owner, 1), 0);
        assert_eq!(count(&owner, 2), 0);
        assert_eq!(count(&peer, 0), 0);
        assert_eq!(peer.host_object(victim).unwrap().health.current, 100.0);
        if !rider {
            let object = owner.host_object(victim).unwrap();
            assert!(object.status.disabled_unmanned);
            assert_eq!(object.owner_player_id, Some(0));
            assert_eq!(object.health.current, 100.0);
        }
    }
}

#[test]
fn normal_apro_save_preserves_sniper_counter_and_resumed_next_hit() {
    let (mut original, [victim, source]) = world(100);
    snipe(&mut original, victim, source);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, _) = world(900);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(count(&restored, 0), 1);
    assert_eq!(count(&restored, 9), 0);
    for world in [&mut original, &mut restored] {
        world.frame = 101;
        snipe(world, victim, source);
        assert_eq!(count(world, 0), 2);
        assert_eq!(count(world, 9), 0);
    }
}

#[test]
fn actual_neutron_unmanning_is_not_an_academy_sniper_event() {
    // CPP NeutonBlastBehavior.cpp112–143 neutralizes directly, without
    // ActiveBody DAMAGE_KILLPILOT or recordVehicleSniped.
    let (mut owner, [victim, caster]) = world(100);
    let (peer, _) = world(700);
    let (_, unmanned, _) = owner.apply_neutron_blast_at(Vec3::ZERO, Team::GLA, Some(caster), false);
    assert_eq!(unmanned, 1);
    assert!(owner.host_object(victim).unwrap().status.disabled_unmanned);
    assert_eq!(count(&owner, 0), 0);
    assert_eq!(count(&peer, 0), 0);
    assert_eq!(count(&owner, 9), 0);
}
