//! CPP Object::~Object unlinks members; onDie leaves them callback-visible.
use super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

fn fixture() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "RosterOwner", false));
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "teamRosterOwner".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    let mut template = ThingTemplate::new("RosterRemovalUnit");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("RosterRemovalUnit", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(members(&world), vec![id.0]);
    (world, id)
}

fn members(world: &GameLogic) -> Vec<u32> {
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    teams
        .into_iter()
        .flat_map(|team| team.read().unwrap().get_members().to_vec())
        .collect()
}

#[test]
fn direct_deletion_keeps_roster_until_physical_removal_and_isolates_peer() {
    let (mut owner, id) = fixture();
    let (peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    owner.destroy_object(id);
    assert!(owner.host_object(id).unwrap().status.destroyed);
    assert_eq!(
        members(&owner),
        vec![id.0],
        "onDelete is not physical unlink"
    );
    owner.process_destroy_list();
    assert!(owner.host_object(id).is_none());
    assert!(members(&owner).is_empty());
    assert_eq!(members(&peer), vec![id.0]);
    assert!(!peer.host_object(id).unwrap().status.destroyed);
}

#[test]
fn lethal_body_callback_keeps_dead_member_until_queued_physical_removal() {
    let (mut owner, id) = fixture();
    let (peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    owner
        .apply_owned_damage(
            id,
            100.0,
            None,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
    assert!(owner.host_object(id).unwrap().status.effectively_dead);
    assert_eq!(
        members(&owner),
        vec![id.0],
        "CPP onDie only notifies the team"
    );
    owner.destroy_object(id);
    assert_eq!(members(&owner), vec![id.0]);
    owner.process_destroy_list();
    assert!(owner.host_object(id).is_none());
    assert!(members(&owner).is_empty());
    assert_eq!(members(&peer), vec![id.0]);
    assert_eq!(peer.host_object(id).unwrap().health.current, 100.0);
}
