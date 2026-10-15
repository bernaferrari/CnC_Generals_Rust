use super::*;

#[test]
fn receiving_team_observation_keeps_live_idle_counts_without_unrequested_sighting() {
    let (mut owner, unit, owner_target) = super::super::named_command_test_support::world();
    let (mut peer, peer_unit, target) = super::super::named_command_test_support::world();
    // Sighting uses authored player relations, not faction labels, and an
    // explicit positive vision radius. Missing map relations are Neutral.
    for (world, viewer, enemy) in [
        (&mut owner, unit, owner_target),
        (&mut peer, peer_unit, target),
    ] {
        world
            .get_player_mut(1)
            .unwrap()
            .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
        world.host_object_mut(viewer).unwrap().vision_range = 100.0;
        assert_eq!(
            world.script_sighting_relationship(
                world.host_object(viewer).unwrap(),
                world.host_object(enemy).unwrap(),
            ),
            gamelogic::common::Relationship::Enemies
        );
    }
    peer.host_object_mut(target)
        .unwrap()
        .set_position(Vec3::new(900., 0., 900.));
    let members = [unit.0];
    let disabled = owner.observe_owned_team_state(&members, false);
    let enabled = owner.observe_owned_team_state(&members, true);
    assert!(!disabled.enemy_sighted);
    assert!(enabled.enemy_sighted);
    assert_eq!(disabled.member_count, enabled.member_count);
    assert_eq!(disabled.live_count, enabled.live_count);
    assert_eq!(disabled.any_live_member, enabled.any_live_member);
    assert_eq!(disabled.all_ai_idle, enabled.all_ai_idle);
    assert_eq!(disabled.any_live_ai, enabled.any_live_ai);
    assert!(
        !peer
            .observe_owned_team_state(&[peer_unit.0], true)
            .enemy_sighted
    );
    assert!(owner.observe_owned_team_state(&members, true).enemy_sighted);
}

#[test]
fn actual_core_team_requests_sighting_only_after_create_with_authored_hooks() {
    use gamelogic::team::{Team, TeamPrototype, TeamStateStep};
    let mut team = Team::new("ReceivingSighted".into(), 17);
    let peer = Team::new("ReceivingSighted".into(), 17);
    assert!(!team.needs_enemy_sighting_observation(&TeamStateStep::Sighted));
    let mut prototype = TeamPrototype::new("ReceivingSighted".into());
    prototype.set_script_on_create("Create".into());
    prototype.set_script_on_enemy_sighted("Enemy".into());
    team.apply_template_script_hooks(&prototype);
    team.set_active();
    let mut step = TeamStateStep::Start;
    assert!(!team.needs_enemy_sighting_observation(&step));
    assert_eq!(
        team.advance_owned_state(&mut step, Default::default()),
        Some("Create".into())
    );
    assert_eq!(step, TeamStateStep::AfterCreate);
    assert!(!team.needs_enemy_sighting_observation(&step));
    // The immediate OnCreate callback may remove the hook before Sighted.
    prototype.set_script_on_enemy_sighted("".into());
    team.apply_template_script_hooks(&prototype);
    team.advance_owned_state(&mut step, Default::default());
    assert_eq!(step, TeamStateStep::Sighted);
    assert!(!team.needs_enemy_sighting_observation(&step));
    prototype.set_script_on_enemy_sighted("Enemy".into());
    team.apply_template_script_hooks(&prototype);
    assert!(team.needs_enemy_sighting_observation(&step));
    assert!(!peer.needs_enemy_sighting_observation(&step));
    for step in [
        TeamStateStep::Destroyed,
        TeamStateStep::AfterDestroyed,
        TeamStateStep::Idle,
        TeamStateStep::AfterIdle(false),
        TeamStateStep::Done,
    ] {
        assert!(!team.needs_enemy_sighting_observation(&step));
    }
}
