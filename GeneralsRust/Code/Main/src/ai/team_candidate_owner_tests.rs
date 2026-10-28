//! CPP AIPlayer1513/1638: player prototype order and live native reinforcement.
use super::*;

fn fixture(priority: i32) -> (GameLogic, ObjectId, u32) {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Owner", false).with_script_identity("Owner");
    player.resources.supplies = 50_000;
    world.add_player(player);
    world.add_player(Player::new(2, Team::USA, "Other", false).with_script_identity("Other"));
    let mut tank = ThingTemplate::new("AmericaTankCrusader");
    tank.add_kind_of(KindOf::Vehicle).set_cost(100, 0);
    world.templates.insert(tank.name.clone(), tank);
    let mut factory = ThingTemplate::new("AmericaWarFactory");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory);
    world.templates.insert(factory.name.clone(), factory);
    let unit = world
        .create_object_for_player("AmericaTankCrusader", 1, Vec3::ZERO)
        .unwrap();
    let foreign_unit = world
        .create_object_for_player("AmericaTankCrusader", 2, Vec3::new(8.0, 0.0, 0.0))
        .unwrap();
    world
        .create_object_for_player("AmericaWarFactory", 1, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let mut own_instance = 0;
    let mut memberships = Vec::new();
    {
        let mut factory = world.team_factory.lock().unwrap();
        for (name, owner, priority, member, auto) in [
            ("OwnedZulu", 1, priority, Some(unit), true),
            ("OwnedAlpha", 1, 10, None, false),
            ("ForeignAuto", 2, 999, Some(foreign_unit), true),
        ] {
            factory
                .init_team_owned(
                    name.into(),
                    if owner == 1 {
                        "Owner".into()
                    } else {
                        "Other".into()
                    },
                    false,
                    None,
                    Some(owner),
                )
                .unwrap();
            let mut prototype = (*factory.find_team_prototype(name).unwrap()).clone();
            prototype.set_max_instances(8);
            prototype.set_production_priority(priority);
            prototype.set_automatically_reinforce(auto);
            if !auto {
                prototype.set_production_condition("AlwaysBuild".into());
            }
            prototype.set_units_info(
                0,
                gamelogic::team::CreateUnitsInfo {
                    min_units: 1,
                    max_units: 3,
                    unit_thing_name: "AmericaTankCrusader".into(),
                },
            );
            factory.replace_team_prototype(prototype);
            if let Some(member) = member {
                let team = factory.create_team(name).unwrap();
                let team = team.read().unwrap();
                memberships.push((member, name.to_owned()));
                if owner == 1 {
                    own_instance = team.get_id();
                }
            }
        }
    }
    for (member, name) in memberships {
        world.host_object_mut(member).unwrap().team_instance_name = name;
        world
            .admit_team_for_host_object_with_effects_checked(
                member,
                None,
                true,
                &mut Default::default(),
            )
            .unwrap();
    }
    use gamelogic::scripting::core::{Condition, ConditionType, OrCondition, Script, ScriptList};
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.set_name("AlwaysBuild".into());
    script.condition = Some(Box::new(condition));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    world
        .owned_script_engine
        .as_ref()
        .unwrap()
        .set_script_list_for_player(1, Some(Box::new(scripts)))
        .unwrap();
    world.add_ai_opponent(1, Team::USA, AIDifficulty::Medium);
    (world, unit, own_instance)
}

#[test]
fn actual_native_selection_filters_controller_and_preserves_admission_order() {
    let (mut world, unit, instance) = fixture(50);
    let (mut peer, peer_unit, peer_instance) = fixture(80);
    assert_eq!(unit, peer_unit);
    assert_eq!(instance, peer_instance);
    let ai = world.ai_manager.ai_players.get(&1).unwrap();
    assert_eq!(
        ai.player_team_prototype_candidates()
            .into_iter()
            .filter(|(name, _)| name.starts_with("Owned"))
            .collect::<Vec<_>>(),
        [("OwnedZulu".into(), 50), ("OwnedAlpha".into(), 10)]
    );
    assert_eq!(
        ai.collect_auto_reinforce_candidates()
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>(),
        ["OwnedZulu"]
    );
    let data = world_view::AiDataView::from_world(&world);
    world.update_owned_ai_military(1, 1.0, &data);
    let queue = &world.ai_manager.ai_players.get(&1).unwrap().team_queue;
    assert!(queue.front().unwrap().reinforcement);
    assert_eq!(queue.front().unwrap().team_id, Some(instance));
    assert_eq!(queue.front().unwrap().name, "OwnedZulu");
    let data = world_view::AiDataView::from_world(&peer);
    peer.update_owned_ai_military(1, 1.0, &data);
    assert_eq!(
        peer.ai_manager
            .ai_players
            .get(&1)
            .unwrap()
            .team_queue
            .front()
            .unwrap()
            .team_id,
        Some(peer_instance)
    );
    assert_eq!(
        world
            .ai_manager
            .ai_players
            .get(&1)
            .unwrap()
            .team_queue
            .front()
            .unwrap()
            .team_id,
        Some(instance)
    );
}

#[test]
fn encoded_restore_and_foreign_reset_keep_candidate_owner() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
    let (mut world, _, instance) = fixture(50);
    let (mut peer, _, _) = fixture(80);
    let data = world_view::AiDataView::from_world(&world);
    world.update_owned_ai_military(1, 1.0, &data);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    // Typed restore receives the same prepared map definitions, as the real
    // native stage loads that saved map before applying the continuation.
    let (mut restored, _, _) = fixture(30);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    peer.reset();
    let ai = restored.ai_manager.ai_players.get(&1).unwrap();
    assert_eq!(ai.team_queue.front().unwrap().team_id, Some(instance));
    assert_eq!(
        ai.collect_auto_reinforce_candidates()
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>(),
        ["OwnedZulu"]
    );
}
