//! CPP GameLogic.cpp3698-3736: a callback finishes before the next update.
//! No claim about original heap order: roles follow the existing arena encounter.
use super::*;
use crate::game_logic::game_logic::PlayerSideRole;
use crate::game_logic::weapon_bootstrap::{
    HostMissileFlight, HostProjectileFlight, HostProjectileLifecycle,
};

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = 100;
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let mut source = ThingTemplate::new("CallbackSource");
    source.set_health(100.0).add_kind_of(KindOf::Infantry);
    let mut blocker = ThingTemplate::new("CallbackBlocker");
    blocker
        .set_health(100.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Structure);
    world.templates.insert(source.name.clone(), source);
    world.templates.insert(blocker.name.clone(), blocker);
    let source = world
        .create_object_for_player("CallbackSource", 1, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    let blocker = world
        .create_object_for_player("CallbackBlocker", 1, Vec3::ZERO)
        .unwrap();
    (world, source, blocker)
}

fn shell(id: ObjectId, source: ObjectId) -> Projectile {
    let mut shell = Projectile::new(
        id,
        Vec3::ZERO,
        Vec3::new(1000.0, 0.0, 0.0),
        20.0,
        DamageType::Unresistable,
        source,
        None,
    );
    // Keep this controller-order witness independent of armor selection.
    // CPP UNRESISTABLE applies the specified amount without damage scalar.
    shell.speed = 30.0;
    shell.velocity = Vec3::new(30.0, 0.0, 0.0);
    shell.source_team = Team::USA;
    shell.source_owner_player_id = Some(1);
    shell.projectile_collides = crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_STRUCTURES;
    shell
}

#[test]
fn first_body_callback_neutralizes_controller_before_next_incidental_collision() {
    let (mut owner, source, blocker) = world();
    let (peer, peer_source, peer_blocker) = world();
    assert_eq!((source, blocker), (peer_source, peer_blocker));
    for id in [ObjectId(9001), ObjectId(9002)] {
        owner
            .combat_system
            .projectiles
            .insert(id, shell(id, source));
    }
    // Preserve HashMap's existing encounter, without pretending this is C++
    // equal-priority module order. Mutating values does not change that order.
    let order: Vec<_> = owner.combat_system.projectiles.keys().copied().collect();
    let first = owner.combat_system.projectiles.get_mut(&order[0]).unwrap();
    first.target_id = Some(blocker);
    first.target_position = Vec3::ZERO;
    first.damage = 1.0;
    first.damage_type = DamageType::KillPilot;
    let retired = owner.update_owned_projectile_impacts(0.0);
    assert_eq!(owner.host_object(blocker).unwrap().owner_player_id, Some(0));
    assert_eq!(
        owner.host_object(blocker).unwrap().health.current,
        80.0,
        "second STRUCTURES-only shell must observe first callback's neutral controller"
    );
    assert_eq!(retired, order);
    assert_eq!(owner.combat_system.projectile_count(), 0);
    assert_eq!(
        peer.host_object(peer_blocker).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        peer.host_object(peer_blocker).unwrap().health.current,
        100.0
    );
}

#[test]
fn registered_dead_target_is_not_missile_target_disappearance() {
    let (mut owner, source, target) = world();
    let target_object = owner.host_object_mut(target).unwrap();
    target_object.set_position(Vec3::new(1000.0, 0.0, 0.0));
    target_object.health.current = 0.0;
    assert!(!target_object.is_alive());
    let id = ObjectId(9001);
    let mut missile = shell(id, source);
    missile.target_id = Some(target);
    missile.target_position = Vec3::new(1234.0, 0.0, 0.0);
    missile.is_homing = true;
    missile.flight = Some(HostProjectileFlight::Missile(HostMissileFlight::default()));
    missile.projectile_lifecycle = Some(HostProjectileLifecycle::Missile {
        try_to_follow_target: true,
        fuel_lifetime_frames: 0,
        detonate_on_no_fuel: false,
        kill_self_delay_frames: 30,
    });
    owner.combat_system.projectiles.insert(id, missile);
    assert!(owner.update_owned_projectile_impacts(1.0 / 30.0).is_empty());
    let missile = &owner.combat_system.projectiles[&id];
    assert_eq!(
        missile.missile_kill_self_started_frame, None,
        "CPP getGoalObject returns a registered pointer, not an isAlive filter"
    );
    assert_eq!(missile.target_position.x, 1000.0);
    assert!(missile.position.x > 0.0);
    owner.objects.remove(&target);
    assert!(owner.update_owned_projectile_impacts(1.0 / 30.0).is_empty());
    assert!(
        owner.combat_system.projectiles[&id]
            .missile_kill_self_started_frame
            .is_some()
    );
}
