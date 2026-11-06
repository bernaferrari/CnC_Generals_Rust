use super::*;
use gamelogic::common::Relationship;

fn world(relationship: Relationship) -> GameLogic {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "SameOwner", true);
    player.set_map_relationship(1, Relationship::Enemies);
    world.add_player(player);
    world.add_player(Player::new(1, Team::GLA, "SameTarget", false));
    {
        let mut factory = world.team_factory.lock().unwrap();
        factory.replace_team_prototype(gamelogic::team::TeamPrototype::new("SameTeam".into()));
        let team = factory.create_team("SameTeam").unwrap();
        let id = team.read().unwrap().get_id();
        world
            .players
            .get_mut(&0)
            .unwrap()
            .set_team_relationship_override(id, relationship);
    }
    world
}
fn relation(world: &GameLogic, team: &str) -> Relationship {
    GameLogic::object_relationship_from_owners(
        &world.team_factory,
        &world.players,
        Some(0),
        "",
        Some(1),
        team,
    )
}
#[test]
fn receiving_relationship_ignores_foreign_core_team_override() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_relationship_ignores_foreign_core_team_override",
        || {
            let source = world(Relationship::Allies);
            let peer = world(Relationship::Neutral);
            let team = source
                .team_factory
                .lock()
                .unwrap()
                .find_team_instances("SameTeam")
                .into_iter()
                .next()
                .unwrap();
            let mut foreign = gamelogic::player::Player::new(0);
            foreign.init_from_dict_defaults();
            foreign.set_team_relationship(&team.read().unwrap(), Relationship::Enemies);
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(foreign));
            {
                let __arc_bind = gamelogic::player::ThePlayerList();

                let mut list = __arc_bind.write().unwrap();
                list.clear();
                list.add_player(std::sync::Arc::clone(&foreign));
            }
            assert_eq!(relation(&source, "SameTeam"), Relationship::Allies);
            assert_eq!(relation(&peer, "SameTeam"), Relationship::Neutral);
            assert_eq!(
                foreign
                    .read()
                    .unwrap()
                    .override_relationship_for_team(&team.read().unwrap()),
                Some(Relationship::Enemies)
            );
        },
    );
}
#[test]
fn actual_relationship_owner_survives_encoded_restore_and_peer_reset() {
    let source = world(Relationship::Allies);
    let mut peer = world(Relationship::Neutral);
    assert_eq!(relation(&source, "SameTeam"), Relationship::Allies);
    assert_eq!(relation(&peer, "SameTeam"), Relationship::Neutral);
    assert_eq!(relation(&source, "OtherTeam"), Relationship::Enemies);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world(Relationship::Neutral);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(relation(&restored, "SameTeam"), Relationship::Allies);
    assert_eq!(relation(&restored, "OtherTeam"), Relationship::Enemies);
    peer.reset();
    assert_eq!(relation(&source, "SameTeam"), Relationship::Allies);
    assert_eq!(relation(&restored, "SameTeam"), Relationship::Allies);
}
