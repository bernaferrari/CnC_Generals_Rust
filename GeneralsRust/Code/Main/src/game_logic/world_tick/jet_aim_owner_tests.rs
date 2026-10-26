//! Actual headless Main AIM state observed through ordinary receiving owners.
use super::*;

fn world(frame: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    let mut attacker = ThingTemplate::new("AIMReceivingSource");
    attacker.add_kind_of(KindOf::Vehicle).set_health(100.0);
    attacker.set_primary_weapon(Weapon {
        range: 1000.0,
        damage: 1.0,
        can_target_air: true,
        can_target_ground: true,
        ..Default::default()
    });
    let mut victim = ThingTemplate::new("AmericaJetStealthFighter");
    victim.add_kind_of(KindOf::Aircraft).set_health(100.0);
    world.templates.insert(attacker.name.clone(), attacker);
    world.templates.insert(victim.name.clone(), victim);
    let source = world
        .create_object("AIMReceivingSource", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let target = world
        .create_object(
            "AmericaJetStealthFighter",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    world.objects.get_mut(&source).unwrap().target = Some(target);
    (world, source, target)
}

#[test]
fn aim_enter_admits_receiving_jet_before_turret_target_and_peer_is_independent() {
    let (mut a, source, target) = world(10);
    let (mut b, peer_source, peer_target) = world(100);
    assert_eq!((source, target), (peer_source, peer_target));
    assert!(
        a.attack_aim_at_target_enter_with_effects(source, &mut Default::default())
            .unwrap()
    );
    assert_eq!(
        a.host_object(target).unwrap().jet_ai.targeted_by,
        vec![source]
    );
    assert_eq!(
        a.host_object(target)
            .unwrap()
            .jet_ai
            .untargetable_expire_frame,
        55
    );
    assert!(
        b.attack_aim_at_target_enter_with_effects(peer_source, &mut Default::default())
            .unwrap()
    );
    assert_eq!(
        b.host_object(peer_target)
            .unwrap()
            .jet_ai
            .untargetable_expire_frame,
        145
    );
    drop(b);
    assert_eq!(
        a.host_object(target)
            .unwrap()
            .jet_ai
            .untargetable_expire_frame,
        55
    );
    assert!(
        a.host_object(target)
            .unwrap()
            .is_temporarily_preventing_aim_success(10)
    );
}

#[test]
fn turret_aim_re_admits_targeter_before_observing_prevention() {
    let (mut world, source, target) = world(30);
    let actor = world.objects.get_mut(&source).unwrap();
    actor.turret_enabled = true;
    actor.turret_turn_rate_rad = 0.2;
    world.set_turret_target_object(source, Some(target), false);
    assert!(
        world
            .host_object(target)
            .unwrap()
            .jet_ai
            .targeted_by
            .is_empty()
    );
    assert_eq!(
        world
            .tick_turret_aim_with_effects(source, 1.0, &mut Default::default())
            .unwrap(),
        AttackAimResult::Continue
    );
    assert_eq!(
        world.host_object(target).unwrap().jet_ai.targeted_by,
        vec![source]
    );
    assert_eq!(
        world
            .host_object(target)
            .unwrap()
            .jet_ai
            .untargetable_expire_frame,
        75
    );
}
