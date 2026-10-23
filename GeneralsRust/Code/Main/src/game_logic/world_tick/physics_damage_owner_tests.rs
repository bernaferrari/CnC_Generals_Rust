//! Receiving Physics Body effects, CPP PhysicsUpdate overlap/landing order.
use super::*;
use crate::game_logic::{KindOf, Object, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn crush_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .players
        .insert(1, Player::new(1, Team::GLA, "Victim", false));
    world
        .players
        .insert(2, Player::new(2, Team::USA, "Crusher", true));
    let mut tank_template = ThingTemplate::new("OwnedCrushTank");
    tank_template.add_kind_of(KindOf::Vehicle);
    let tank_id = ObjectId(101);
    let mut tank = Object::new(tank_template, tank_id, Team::USA);
    tank.owner_player_id = Some(2);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(-20.0, 0.0, 0.0));
    world.objects.insert(tank_id, tank);
    let mut infantry = ThingTemplate::new("OwnedCrushInfantry");
    infantry.add_kind_of(KindOf::Infantry);
    infantry.set_health(100.0);
    let victim_id = ObjectId(102);
    let mut victim = Object::new(infantry, victim_id, Team::GLA);
    victim.owner_player_id = Some(1);
    victim.crushable_level = 0;
    victim.has_squish_collide = false;
    victim.last_damage_source = Some(tank_id);
    world.objects.insert(victim_id, victim);
    (world, tank_id, victim_id)
}

#[test]
fn resident_crush_zero_then_huge_uses_receiving_player_source_and_frame() {
    let (mut receiving, crusher, victim) = crush_world();
    let (peer, peer_crusher, peer_victim) = crush_world();
    assert_eq!((crusher, victim), (peer_crusher, peer_victim));
    receiving.frame = 31;
    assert!(receiving.apply_overlap_crush_check(crusher, victim, false));
    assert_eq!(receiving.host_object(victim).unwrap().health.current, 100.0);
    assert!(receiving.host_object(crusher).is_some());
    assert!(
        !receiving.players[&1].get_attacked_by(2),
        "CPP zero-amount crush FX cannot notify a retained previous attacker"
    );
    assert_eq!(receiving.players[&1].attacked_frame, 0);
    receiving
        .host_object_mut(crusher)
        .unwrap()
        .set_position(Vec3::new(6.0, 0.0, 0.0));
    receiving.frame = 32;
    assert!(receiving.apply_overlap_crush_check(crusher, victim, false));
    let actual = receiving.host_object(victim).unwrap();
    assert!(actual.status.destroyed);
    assert_eq!(actual.last_damage_source, Some(crusher));
    assert!(receiving.players[&1].get_attacked_by(2));
    assert_eq!(receiving.players[&1].attacked_frame, 32);
    assert_eq!(
        receiving.objects.registered_len(),
        2,
        "both canonical participants remain resident through the Body/Player operation"
    );
    assert!(!peer.players[&1].get_attacked_by(2));
    assert_eq!(peer.host_object(victim).unwrap().health.current, 100.0);
}

#[test]
fn landing_self_source_and_null_rest_kill_never_notify_another_world() {
    let (mut receiving, _, id) = crush_world();
    let (peer, _, peer_id) = crush_world();
    assert_eq!(id, peer_id);
    receiving.frame = 45;
    {
        let object = receiving.host_object_mut(id).unwrap();
        object.set_position(Vec3::new(0.0, -1.0, 0.0));
        object.was_airborne_last_frame = true;
        object.immune_to_falling_damage = false;
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.physics_mass = 1.0;
        object.fall_height_damage_factor = 1.0;
    }
    receiving.stamp_airborne_targets_from_locomotor(&[id]);
    assert!(receiving.host_object(id).unwrap().health.current < 100.0);
    assert!(receiving.players[&1].get_attacked_by(1));
    assert_eq!(receiving.players[&1].attacked_frame, 45);
    receiving.frame = 46;
    {
        let object = receiving.host_object_mut(id).unwrap();
        object.kill_when_resting_on_ground = true;
        object.movement.velocity = Vec3::ZERO;
    }
    receiving.stamp_airborne_targets_from_locomotor(&[id]);
    assert!(receiving.host_object(id).unwrap().status.destroyed);
    assert_eq!(
        receiving.players[&1].attacked_frame, 46,
        "CPP retains the valid prior SELF source on the next frame and notifies again"
    );
    assert_eq!(
        receiving.host_object(id).unwrap().last_damage_source,
        Some(id)
    );
    assert!(
        receiving.players[&1]
            .attacked_by
            .iter()
            .enumerate()
            .all(|(index, value)| *value == (index == 1)),
        "only the retained SELF attacker may be recorded"
    );
    assert!(!peer.players[&1].get_attacked_by(1));
    assert_eq!(peer.host_object(id).unwrap().health.current, 100.0);
}

#[test]
fn full_shipping_motion_landing_supplies_receiving_player_and_frame() {
    let (mut receiving, crusher, victim) = crush_world();
    let (peer, _, peer_victim) = crush_world();
    assert_eq!(victim, peer_victim);
    receiving
        .host_object_mut(crusher)
        .unwrap()
        .set_position(Vec3::new(1000.0, 0.0, 1000.0));
    receiving
        .host_object_mut(crusher)
        .unwrap()
        .movement
        .velocity = Vec3::ZERO;
    receiving.frame = 61;
    {
        let object = receiving.host_object_mut(victim).unwrap();
        object.set_position(Vec3::new(0.0, 1.0, 0.0));
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.was_airborne_last_frame = true;
        object.immune_to_falling_damage = false;
        object.physics_mass = 1.0;
        object.fall_height_damage_factor = 1.0;
        object.stick_to_ground = false;
        object.allow_to_fall = true;
    }
    receiving.tick_physics_collisions_all();
    let actual = receiving.host_object(victim).unwrap();
    assert!(actual.health.current < 100.0);
    assert_eq!(actual.last_damage_source, Some(victim));
    assert!(receiving.players[&1].get_attacked_by(1));
    assert_eq!(receiving.players[&1].attacked_frame, 61);
    assert!(!peer.players[&1].get_attacked_by(1));
    assert_eq!(peer.host_object(victim).unwrap().health.current, 100.0);
}

#[test]
fn full_shipping_stun_fall_and_inverted_death_keep_self_then_null_source_order() {
    let (mut receiving, _, victim) = crush_world();
    let (peer, _, peer_victim) = crush_world();
    assert_eq!(victim, peer_victim);
    receiving.frame = 71;
    {
        let object = receiving.host_object_mut(victim).unwrap();
        object.set_position(Vec3::new(0.0, 1.0, 0.0));
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.shock_stun_frames = 20;
        object.shock_was_airborne = true;
        object.shock_allow_bounce = false;
        object.immune_to_falling_damage = false;
        object.physics_mass = 1.0;
        object.fall_height_damage_factor = 1.0;
    }
    receiving.tick_shock_stun_all();
    assert!(receiving.host_object(victim).unwrap().health.current < 100.0);
    assert!(receiving.players[&1].get_attacked_by(1));
    assert_eq!(receiving.players[&1].attacked_frame, 71);
    receiving.frame = 72;
    {
        let object = receiving.host_object_mut(victim).unwrap();
        object.set_transform_matrix(glam::Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            glam::Quat::from_rotation_z(std::f32::consts::PI),
            Vec3::new(0.0, 1.0, 0.0),
        ));
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.shock_stun_frames = 20;
        object.shock_allow_bounce = true;
        object.shock_grounded_once = false;
    }
    receiving.tick_shock_stun_all();
    assert!(receiving.host_object(victim).unwrap().status.destroyed);
    assert_eq!(
        receiving.players[&1].attacked_frame, 72,
        "CPP retains the valid prior SELF source for next-frame null-source destruction"
    );
    assert_eq!(
        receiving.host_object(victim).unwrap().last_damage_source,
        Some(victim)
    );
    assert!(
        receiving.players[&1]
            .attacked_by
            .iter()
            .enumerate()
            .all(|(index, value)| *value == (index == 1)),
        "only the retained SELF attacker may be recorded"
    );
    assert!(!peer.players[&1].get_attacked_by(1));
    assert_eq!(peer.host_object(victim).unwrap().health.current, 100.0);
}

#[test]
fn nonlethal_physics_self_hit_prefers_actual_infantry_over_prior_vehicle() {
    let (mut receiving, crusher, victim) = crush_world();
    let (peer, _, _) = crush_world();
    receiving.frame = 90;
    {
        let object = receiving.host_object_mut(victim).unwrap();
        object.last_damage_timestamp = Some(90);
        object.last_damage_source = Some(crusher);
        object.set_position(Vec3::new(0.0, -1.0, 0.0));
        object.was_airborne_last_frame = true;
        object.immune_to_falling_damage = false;
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.physics_mass = 1.0;
        object.fall_height_damage_factor = 1.0;
    }
    receiving.stamp_airborne_targets_from_locomotor(&[victim]);
    let actual = receiving.host_object(victim).unwrap();
    assert!(actual.health.current > 0.0 && actual.health.current < 100.0);
    assert_eq!(
        actual.last_damage_source,
        Some(victim),
        "CPP prefers a real incoming infantry even when the hit is nonlethal"
    );
    assert!(receiving.players[&1].get_attacked_by(1));
    assert!(!receiving.players[&1].get_attacked_by(2));
    assert_eq!(receiving.players[&1].attacked_frame, 90);
    assert_eq!(
        peer.host_object(victim).unwrap().last_damage_source,
        Some(crusher)
    );
    assert_eq!(peer.host_object(victim).unwrap().health.current, 100.0);
    assert_eq!(peer.players[&1].attacked_frame, 0);
}
