use super::*;
use gamelogic::common::Relationship;

#[test]
fn distinct_players_default_neutral_not_enemies() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "A", true));
    logic.add_player(Player::new(1, Team::GLA, "B", false));
    assert_eq!(logic.player_relationship(0, 1), Relationship::Neutral);
    assert_eq!(logic.player_relationship(1, 0), Relationship::Neutral);
    assert_eq!(logic.player_relationship(0, 0), Relationship::Allies);
}

#[test]
fn map_allies_and_enemies_override_default() {
    let mut logic = GameLogic::new();
    let mut a = Player::new(0, Team::USA, "PlyrAmerica", true);
    let mut b = Player::new(1, Team::China, "PlyrChina", false);
    let mut c = Player::new(2, Team::GLA, "PlyrGLA", false);
    a.set_map_relationship(1, Relationship::Allies);
    a.set_map_relationship(2, Relationship::Enemies);
    logic.add_player(a);
    logic.add_player(b);
    logic.add_player(c);
    assert_eq!(logic.player_relationship(0, 1), Relationship::Allies);
    assert_eq!(logic.player_relationship(0, 2), Relationship::Enemies);
}

#[test]
fn skirmish_alliance_teams_still_enemies_when_assigned() {
    let mut logic = GameLogic::new();
    let mut a = Player::new(0, Team::USA, "A", true);
    let mut b = Player::new(1, Team::USA, "B", false);
    a.alliance_team = 1;
    b.alliance_team = 2;
    logic.add_player(a);
    logic.add_player(b);
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
}

#[test]
fn team_override_allies_one_named_team() {
    // C++ Player.cpp:542-554 team-id override before player relations.
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
    let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
    usa.set_map_relationship(1, Relationship::Enemies);
    usa.set_team_relationship_override(42, Relationship::Allies);
    logic.add_player(usa);
    logic.add_player(gla);

    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut src = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    src.owner_player_id = Some(0);
    let mut convoy = Object::new(tmpl.clone(), ObjectId(2), Team::GLA);
    convoy.owner_player_id = Some(1);
    convoy.team_instance_name = "CivilianConvoy".into();
    convoy.team_instance_id = Some(42);
    let mut other = Object::new(tmpl, ObjectId(3), Team::GLA);
    other.owner_player_id = Some(1);

    assert_eq!(
        logic.object_relationship(&src, &convoy),
        Relationship::Allies
    );
    assert_eq!(
        logic.object_relationship(&src, &other),
        Relationship::Enemies
    );
}

#[test]
fn team_set_override_relation_to_team_beats_player_map() {
    // C++ Team::getRelationship team-id override before player relations.
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
    let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
    usa.set_map_relationship(1, Relationship::Enemies);
    logic.add_player(usa.with_script_identity("PlyrAmerica"));
    logic.add_player(gla.with_script_identity("PlyrGLA"));
    let (source_id, target_id) = {
        let mut factory = logic.team_factory.lock().unwrap();
        for name in ["RangerTeam", "CivilianConvoy"] {
            factory.replace_team_prototype(gamelogic::team::TeamPrototype::new(name.into()));
        }
        let source = factory.create_inactive_team("RangerTeam").unwrap();
        let target = factory.create_inactive_team("CivilianConvoy").unwrap();
        source.write().unwrap().set_controlling_player_id(Some(0));
        target.write().unwrap().set_controlling_player_id(Some(1));
        let source_id = source.read().unwrap().get_id();
        let target_id = target.read().unwrap().get_id();
        source
            .write()
            .unwrap()
            .set_override_team_relationship(target_id, Relationship::Allies);
        (source_id, target_id)
    };

    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut src = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    src.owner_player_id = Some(0);
    src.team_instance_name = "RangerTeam".into();
    src.team_instance_id = Some(source_id);
    let mut convoy = Object::new(tmpl.clone(), ObjectId(2), Team::GLA);
    convoy.owner_player_id = Some(1);
    convoy.team_instance_name = "CivilianConvoy".into();
    convoy.team_instance_id = Some(target_id);
    let mut other = Object::new(tmpl, ObjectId(3), Team::GLA);
    other.owner_player_id = Some(1);

    assert_eq!(
        logic.object_relationship(&src, &convoy),
        Relationship::Allies
    );
    assert_eq!(
        logic.object_relationship(&src, &other),
        Relationship::Enemies
    );
}

#[test]
fn undetected_defector_overrides_owner_map() {
    // C++ Object::getRelationship: self undetected → Neutral, that → Allies.
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
    let gla = Player::new(1, Team::GLA, "PlyrGLA", false);
    usa.set_map_relationship(1, Relationship::Enemies);
    logic.add_player(usa);
    logic.add_player(gla);

    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut src = Object::new(tmpl.clone(), ObjectId(10), Team::USA);
    src.owner_player_id = Some(0);
    let mut tgt = Object::new(tmpl, ObjectId(11), Team::GLA);
    tgt.owner_player_id = Some(1);

    assert_eq!(logic.object_relationship(&src, &tgt), Relationship::Enemies);

    src.begin_undetected_defection(0, 30, false);
    assert_eq!(
        logic.object_relationship(&src, &tgt),
        Relationship::Neutral,
        "undetected defector must not auto-acquire old enemies"
    );

    src.blow_defector_cover();
    tgt.begin_undetected_defection(0, 30, false);
    assert_eq!(
        logic.object_relationship(&src, &tgt),
        Relationship::Allies,
        "teammates must treat flashing defector as own"
    );
}
