//! ActiveBody.cpp:365-450 — KillPilot / KillGarrisoned mutate the borrowed
//! Body owner without reacquiring a target Object write lock (hq-cxq51).
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::game_logic::PlayerSideRole;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    world.add_player(Player::new(1, Team::USA, "Victim", true));
    world.add_player(Player::new(2, Team::GLA, "Sniper", false));

    let mut vt = ThingTemplate::new("ReentryKillPilotVehicle");
    vt.set_health(100.0).add_kind_of(KindOf::Vehicle);
    world.templates.insert("ReentryKillPilotVehicle".into(), vt);
    let vehicle = world
        .create_object_for_player("ReentryKillPilotVehicle", 1, Vec3::ZERO)
        .unwrap();

    let mut st = ThingTemplate::new("ReentryKillPilotSniper");
    st.set_health(50.0).add_kind_of(KindOf::Infantry);
    world.templates.insert("ReentryKillPilotSniper".into(), st);
    let sniper = world
        .create_object_for_player("ReentryKillPilotSniper", 2, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();

    let mut bt = ThingTemplate::new("ReentryKillGarrisonBuilding");
    bt.set_health(100.0)
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable);
    world
        .templates
        .insert("ReentryKillGarrisonBuilding".into(), bt);
    let building = world
        .create_object_for_player(
            "ReentryKillGarrisonBuilding",
            1,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .unwrap();
    (world, vehicle, sniper, building)
}

#[test]
fn kill_pilot_and_kill_garrisoned_complete_without_target_lock_reentry() {
    let (mut owner, vehicle, sniper, building) = world();
    let context = DamageHitContext::new(owner.host_object(sniper), None, DamageType::KillPilot);
    let result = owner
        .apply_owned_damage(
            vehicle,
            1.0,
            Some(sniper),
            DamageType::KillPilot,
            HostDeathType::Normal,
            None,
            &context,
        )
        .expect("KillPilot must complete without reentrant target lock");
    assert!(!result.destroyed);
    let object = owner.host_object(vehicle).unwrap();
    assert!(object.status.disabled_unmanned);
    assert_eq!(object.health.current, 100.0);

    let context =
        DamageHitContext::new(owner.host_object(sniper), None, DamageType::KillGarrisoned);
    let result = owner
        .apply_owned_damage(
            building,
            3.0,
            Some(sniper),
            DamageType::KillGarrisoned,
            HostDeathType::Normal,
            None,
            &context,
        )
        .expect("KillGarrisoned must complete without reentrant target lock");
    assert!(!result.destroyed);
    let building = owner.host_object(building).unwrap();
    assert_eq!(building.health.current, 100.0);
    assert_eq!(building.status.pending_kill_garrisoned, 3);
}
