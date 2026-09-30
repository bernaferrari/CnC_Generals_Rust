use crate::game_logic::{GameLogic, Team as Faction, ThingTemplate};
use game_engine::common::dict::Dict;
use game_engine::common::well_known_keys::{
    key_team_is_singleton, key_team_name, key_team_owner, key_team_production_priority,
};
use gamelogic::team::{get_team_factory, TEAM_ID_INVALID};
use glam::Vec3;

fn add_world_prototype(logic: &GameLogic, name: &str, priority: i32) {
    let mut dict = Dict::new();
    dict.set_ascii_string(key_team_name(), name);
    dict.set_ascii_string(key_team_owner(), "");
    dict.set_bool(key_team_is_singleton(), false);
    dict.set_int(key_team_production_priority(), priority);
    let mut factory = logic.team_factory.lock().expect("world factory");
    factory.reset();
    factory
        .init_team(name.into(), "".into(), false, Some(&dict))
        .expect("prototype");
}

#[test]
fn host_team_activation_uses_the_driving_world_factory() {
    let mut world_a = GameLogic::new();
    let world_b = GameLogic::new();
    add_world_prototype(&world_a, "SharedTeam", 11);
    add_world_prototype(&world_b, "SharedTeam", 77);

    // Model the old Main consumer's ambient factory selection: world B is the
    // currently installed engine singleton while the host is ticking world A.
    {
        let mut factory = get_team_factory().lock().expect("ambient factory");
        factory.reset();
        factory
            .init_team("SharedTeam".into(), "".into(), false, None)
            .expect("ambient prototype");
    }

    world_a.templates.insert(
        "FactoryOwnerTestUnit".into(),
        ThingTemplate::new("FactoryOwnerTestUnit"),
    );
    let object_id = world_a
        .create_object("FactoryOwnerTestUnit", Faction::USA, Vec3::ZERO)
        .expect("host object");
    world_a
        .host_object_mut(object_id)
        .expect("host object lookup")
        .team_instance_name = "SharedTeam".into();

    world_a.activate_leftover_team_for_host_object(object_id);

    let world_a_members = world_a
        .team_factory
        .lock()
        .expect("world A factory")
        .find_team("SharedTeam")
        .and_then(|team| team.read().ok().map(|team| team.get_members().to_vec()))
        .unwrap_or_default();
    assert_eq!(world_a_members, [object_id.0]);
    assert_ne!(TEAM_ID_INVALID, object_id.0);
}
