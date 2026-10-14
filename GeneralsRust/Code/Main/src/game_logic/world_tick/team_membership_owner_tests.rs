use super::super::*;

fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "RosterOwner", true));
    let mut template = ThingTemplate::new("RosterUnit");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    world
}
fn members(world: &GameLogic) -> Vec<u32> {
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    teams
        .iter()
        .flat_map(|team| team.read().unwrap().get_members().to_vec())
        .collect()
}
#[test]
fn actual_factory_admits_default_members_in_cpp_prepend_order_and_isolates_peer() {
    let mut world = owner();
    let peer = owner();
    let first = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(members(&world), [second.0, first.0]);
    world.activate_leftover_team_for_host_object(first);
    assert_eq!(
        members(&world),
        [second.0, first.0],
        "re-admission does not reorder"
    );
    assert!(members(&peer).is_empty());
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    assert_eq!(teams.len(), 1);
    assert!(teams[0].read().unwrap().is_active());
}
#[test]
fn existing_authored_default_identity_survives_player_admission() {
    let mut world = GameLogic::new();
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
    let before = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1)[0]
        .read()
        .unwrap()
        .get_id();
    world.add_player(Player::new(1, Team::USA, "RosterOwner", true));
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].read().unwrap().get_id(), before);
}

#[test]
fn explicit_named_admission_moves_member_out_of_default_before_observation() {
    let mut world = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "NamedRoster".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    world.host_object_mut(unit).unwrap().team_instance_name = "NamedRoster".into();
    world.activate_leftover_team_for_host_object(unit);
    let mut factory = world.team_factory.lock().unwrap();
    assert!(
        !factory
            .find_team("teamRosterOwner")
            .unwrap()
            .read()
            .unwrap()
            .has_member(unit.0)
    );
    assert_eq!(
        factory
            .find_team("NamedRoster")
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        &[unit.0]
    );
}

#[test]
fn missing_exact_identity_does_not_retarget_an_existing_same_name_team() {
    let mut world = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    let before = members(&world);
    world.host_object_mut(unit).unwrap().team_instance_id = Some(u32::MAX);
    world.activate_leftover_team_for_host_object(unit);
    assert_eq!(
        world.host_object(unit).unwrap().team_instance_id,
        Some(u32::MAX)
    );
    assert_eq!(members(&world), before);
}
