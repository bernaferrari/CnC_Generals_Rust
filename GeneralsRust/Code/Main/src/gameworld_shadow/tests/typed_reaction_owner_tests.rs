//! Real released Body callbacks, independent role Weapons and saved continuation.
use super::*;
use crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDamagedWeaponRole as Role;

fn finish_three_states(logic: &mut GameLogic, id: ObjectId) {
    perform_owner_damage(logic, id, 19.0, None);
    perform_owner_health(logic, id, 90.0);
    perform_owner_damage(logic, id, 5.0, None);
}

#[test]
fn authored_same_frame_reaction_roles_fire_before_return_and_ai_does_not_replay() {
    let (mut owner, _shadow, id) = injured_world_with_reactions();
    let target = install_reaction_target(&mut owner);
    let (peer, _peer_shadow, peer_id) = injured_world_with_reactions();
    assert_eq!(id, peer_id);
    let peer_runtime = peer
        .host_object(peer_id)
        .unwrap()
        .temporary_weapon_runtime
        .clone();
    perform_owner_damage(&mut owner, id, 1.0, None);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.5);
    perform_owner_damage(&mut owner, id, 19.0, None);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.0);
    perform_owner_health(&mut owner, id, 90.0);
    perform_owner_damage(&mut owner, id, 5.0, None);
    assert_eq!(owner.host_object(target).unwrap().health.current, 98.375);
    assert_eq!(owner.host_object(target).unwrap().previous_health, 99.0);
    let runtime = owner
        .host_object(id)
        .unwrap()
        .temporary_weapon_runtime
        .clone();
    for role in [
        Role::ReactionDamaged,
        Role::ReactionReallyDamaged,
        Role::ReactionPristine,
    ] {
        assert_eq!(
            runtime.damaged[0].weapons[role.index()]
                .as_ref()
                .unwrap()
                .ammo_in_clip,
            7
        );
    }
    // The zero-delay weapons would fire again if ordinary AI replayed the
    // three still-pending health receipts using the final Pristine state.
    assert!(!owner.health_events.snapshot_damage().is_empty());
    let frame = owner.get_frame();
    owner.update();
    assert_eq!(owner.get_frame(), frame + 1);
    assert_eq!(owner.host_object(target).unwrap().health.current, 98.375);
    assert_eq!(
        owner.host_object(id).unwrap().temporary_weapon_runtime,
        runtime
    );
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 40.0);
    assert_eq!(
        peer.host_object(peer_id).unwrap().temporary_weapon_runtime,
        peer_runtime
    );
}

#[test]
fn authored_reaction_uses_actual_overkill_damage_for_the_module_threshold() {
    let (mut owner, _shadow, id) = injured_world_with_reaction_threshold(500.0);
    let target = install_reaction_target(&mut owner);
    // Only40 HP remain, but DamageInfo actual post-armor damage is1000.
    // Passing capped HP loss to the module would wrongly fail DamageAmount500.
    perform_owner_damage(&mut owner, id, 1000.0, None);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.375);
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .temporary_weapon_runtime
            .damaged[0]
            .weapons[Role::ReactionRubble.index()]
        .as_ref()
        .unwrap()
        .ammo_in_clip,
        7
    );
}

#[test]
fn authored_reaction_runtime_restores_and_continues_without_foreign_or_ai_replay() {
    let (mut source, _shadow, id) = injured_world_with_reactions();
    let target = install_reaction_target(&mut source);
    let (mut receiver, _receiver_shadow, receiver_id) = injured_world_with_reactions();
    let receiver_target = install_reaction_target(&mut receiver);
    let (peer, _peer_shadow, peer_id) = injured_world_with_reactions();
    assert_eq!((id, target), (receiver_id, receiver_target));
    perform_owner_damage(&mut source, id, 1.0, None);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    builder
        .restore_from_snapshot(&saved, &mut receiver)
        .unwrap();
    assert_eq!(
        source.host_object(id).unwrap().temporary_weapon_runtime,
        receiver.host_object(id).unwrap().temporary_weapon_runtime
    );
    for world in [&mut source, &mut receiver] {
        finish_three_states(world, id);
        assert_eq!(world.host_object(target).unwrap().health.current, 98.375);
        assert_eq!(world.host_object(target).unwrap().previous_health, 99.0);
        world.update();
        assert_eq!(world.host_object(target).unwrap().health.current, 98.375);
    }
    assert_eq!(
        source.host_object(id).unwrap().temporary_weapon_runtime,
        receiver.host_object(id).unwrap().temporary_weapon_runtime
    );
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 40.0);
}
