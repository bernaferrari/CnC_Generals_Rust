//! AIUpdate.cpp privateGuardPosition/Object reject immobile or projectile actors.
use generals_main::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
use glam::Vec3;

fn make_world(kind: KindOf) -> GameLogic {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("GuardAdmission");
    template.add_kind_of(KindOf::Vehicle).add_kind_of(kind);
    world.templates.insert("GuardAdmission".into(), template);
    world
}

#[test]
fn ordinary_guard_commands_reject_ineligible_actor_without_changing_peer() {
    for kind in [KindOf::Immobile, KindOf::Projectile] {
        let mut world = make_world(kind);
        let mut peer = make_world(KindOf::Selectable);
        let id = world
            .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
            .unwrap();
        let peer_id = peer
            .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
            .unwrap();
        assert_eq!(id, peer_id);
        let before = world.host_object(id).unwrap().ai_state();
        let old_source = world.host_object(id).unwrap().last_command_source;
        assert!(!world.unit_command_guard_position(id, Vec3::new(10., 0., 20.)));
        assert!(!world.unit_command_guard_object(id, id));
        let actor = world.host_object(id).unwrap();
        assert_eq!(actor.ai_state(), before);
        assert_eq!(actor.last_command_source, old_source);
        assert_eq!(actor.guard_position(), None);
        assert_eq!(actor.guard_target(), None);
        assert!(peer.unit_command_guard_position(peer_id, Vec3::new(30., 0., 40.)));
        assert_eq!(
            peer.host_object(peer_id).unwrap().ai_state(),
            AIState::GuardingArea
        );
        assert_eq!(
            peer.host_object(peer_id).unwrap().guard_position(),
            Some(Vec3::new(30., 0., 40.))
        );
        assert!(peer.unit_command_guard_object(peer_id, peer_id));
        assert_eq!(
            peer.host_object(peer_id).unwrap().guard_target(),
            Some(peer_id)
        );
    }
}

#[test]
fn disabled_actor_cannot_enter_either_guard_order() {
    let mut world = make_world(KindOf::Selectable);
    let id = world
        .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().status.disabled_emp = true;
    let before = world.host_object(id).unwrap().ai_state();
    assert!(!world.unit_command_guard_position(id, Vec3::new(10., 0., 20.)));
    assert!(!world.unit_command_guard_object(id, id));
    let actor = world.host_object(id).unwrap();
    assert_eq!(actor.ai_state(), before);
    assert_eq!(actor.guard_position(), None);
    assert_eq!(actor.guard_target(), None);
}

#[test]
fn guard_retaliation_exits_before_ineligible_guard_order_is_rejected() {
    use generals_main::game_logic::{ObjectAudio, SessionAudio};
    let mut world = make_world(KindOf::Selectable);
    let id = world
        .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut audio = SessionAudio::default();
    let actor = world.host_object_mut(id).unwrap();
    actor.set_ai_state(
        AIState::GuardRetaliating,
        &mut ObjectAudio::queue(&mut audio),
    );
    actor.status.disabled_emp = true;
    assert!(!world.unit_command_guard_position(id, Vec3::new(10., 0., 20.)));
    let actor = world.host_object(id).unwrap();
    assert_ne!(actor.ai_state(), AIState::GuardRetaliating);
    assert_eq!(actor.guard_position(), None);
}

#[test]
fn guard_orders_exit_the_receiving_attack_state() {
    use generals_main::game_logic::{ObjectAudio, SessionAudio};
    for object_guard in [false, true] {
        let mut world = make_world(KindOf::Selectable);
        let id = world
            .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
            .unwrap();
        let mut peer = make_world(KindOf::Selectable);
        let peer_id = peer
            .create_object("GuardAdmission", Team::USA, Vec3::ZERO)
            .unwrap();
        assert_eq!(id, peer_id);
        let mut audio = SessionAudio::default();
        for session in [&mut world, &mut peer] {
            let actor = session.host_object_mut(id).unwrap();
            actor.set_ai_state(AIState::Attacking, &mut ObjectAudio::queue(&mut audio));
            actor.target = Some(id);
            actor.set_status_attacking(true);
            actor.set_status_firing_weapon(true);
            actor.set_status_aiming_weapon(true);
        }
        if object_guard {
            assert!(world.unit_command_guard_object(id, id));
        } else {
            assert!(world.unit_command_guard_position(id, Vec3::new(10., 0., 20.)));
        }
        let actor = world.host_object(id).unwrap();
        assert!(
            !actor.status.attacking
                && !actor.status.is_firing_weapon
                && !actor.status.is_aiming_weapon
        );
        assert_eq!(actor.target, None);
        let foreign = peer.host_object(id).unwrap();
        assert_eq!(foreign.ai_state(), AIState::Attacking);
        assert!(
            foreign.status.attacking
                && foreign.status.is_firing_weapon
                && foreign.status.is_aiming_weapon
        );
        assert_eq!(foreign.target, Some(id));
    }
}
