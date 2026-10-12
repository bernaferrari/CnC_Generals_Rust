//! Actual post-update destruction preserves the original notification boundary.
use super::*;
use crate::object::INVALID_ID;

#[test]
fn deletion_retires_actual_owner_and_leaves_same_id_peer_unchanged() {
    let mut game = lethal_damage::native_death_world();
    let peer = lethal_damage::native_death_world();
    let owner = game.target.clone();
    let team = owner.read().unwrap().get_team().unwrap();
    owner.write().unwrap().set_name("NativeVictim".into());
    game.disposal.names.register("NativeVictim".into(), 2);
    game.disposal.scripts.set_frame_object_count_changed(9);
    let peer_guard = peer.target.write().unwrap();
    game.operate(150, |world| {
        world
            .attempt_damage_owned(&owner, &mut lethal_damage::fatal(1))
            .unwrap();
    });
    assert!(game.factory.get_object(2).is_some());
    assert!(game.scheduler.find_object_by_id(2).is_some());
    assert!(team.read().unwrap().has_member(2));
    assert_eq!(game.disposal.names.find("NativeVictim"), Some(2));
    assert_eq!(game.disposal.scripts.get_frame_object_count_changed(), 9);
    assert_eq!(game.dispose_destroyed(150).unwrap(), 1);
    assert!(game.factory.get_object(2).is_none());
    assert!(game.scheduler.find_object_by_id(2).is_none());
    assert!(!team.read().unwrap().has_member(2));
    assert_eq!(game.disposal.names.find("NativeVictim"), None);
    assert_eq!(game.disposal.scripts.get_frame_object_count_changed(), 150);
    assert_eq!(game.source.read().unwrap().get_prev_object_id(), None);
    assert_eq!(game.source.read().unwrap().get_next_object_id(), None);
    assert!(owner.read().unwrap().native_destroy_is_finalized());
    assert_eq!(owner.read().unwrap().get_id(), INVALID_ID);
    assert!(owner.read().unwrap().get_team().is_none());
    assert_eq!(game.dispose_destroyed(151).unwrap(), 0);
    assert_eq!(game.disposal.scripts.get_frame_object_count_changed(), 150);
    assert_eq!(peer_guard.get_id(), 2);
    assert_eq!(peer_guard.get_health(), 100.0);
    assert!(!peer_guard.is_destroyed());
    assert!(peer.scheduler.find_object_by_id(2).is_some());
}

#[test]
fn held_retirement_participant_rejects_before_unlink_and_can_retry() {
    let mut game = lethal_damage::native_death_world();
    let owner = game.target.clone();
    let team = owner.read().unwrap().get_team().unwrap();
    game.operate(160, |world| {
        world
            .attempt_damage_owned(&owner, &mut lethal_damage::fatal(1))
            .unwrap();
    });
    let loan = team.read().unwrap();
    assert!(game.dispose_destroyed(160).is_err());
    assert!(game.factory.get_object(2).is_some());
    assert!(game.scheduler.find_object_by_id(2).is_some());
    assert!(loan.has_member(2));
    drop(loan);
    assert_eq!(game.dispose_destroyed(160).unwrap(), 1);
}
