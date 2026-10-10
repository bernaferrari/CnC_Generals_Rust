//! Actual Main team disband/destruction, with foreign Core runtime positive controls.
use super::*;
use gamelogic::scripting::engine::{ScriptEngine, ScriptEngineHandle, SequentialScript};
use std::sync::{Arc, RwLock};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn script_engine(id: u32) -> ScriptEngine {
    let engine = ScriptEngine::new().unwrap();
    let mut sequential = SequentialScript::new();
    sequential.team_to_exec_on = Some(id);
    engine.append_sequential_script(sequential);
    engine
}

fn owner_world(singleton: bool) -> (GameLogic, u32, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "TeamOwner", false));
    let mut template = ThingTemplate::new("OwnedTeamMember");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let unit = world
        .create_object_for_player("OwnedTeamMember", 1, Vec3::ZERO)
        .unwrap();
    let mut factory = world.team_factory.lock().unwrap();
    factory
        .init_team("teamTeamOwner".into(), "".into(), true, None)
        .unwrap();
    factory
        .init_team("DeleteTeam".into(), "".into(), singleton, None)
        .unwrap();
    let team = factory.create_inactive_team("DeleteTeam").unwrap();
    let id = team.read().unwrap().get_id();
    team.write().unwrap().add_member(unit.0);
    let mut prototype = (*factory.find_team_prototype("DeleteTeam").unwrap()).clone();
    prototype.set_initial_idle_frames(1);
    factory.replace_team_prototype(prototype);
    drop(factory);
    world.host_object_mut(unit).unwrap().team_instance_name = "DeleteTeam".into();
    (world, id, unit)
}

fn queued(id: u32, unit: ObjectId) -> AITeamQueue {
    let mut order = AIWorkOrder::new("OwnedTeamMember".into(), 2, 0);
    order.num_completed = 1;
    order.observed_unit_ids.push(unit);
    let mut queue = AITeamQueue::new("DeleteTeam".into(), vec![order], false, 0);
    queue.team_id = Some(id);
    queue
}

fn controller(world: &GameLogic, pid: u32, id: u32, unit: ObjectId, active: bool) -> AIPlayer {
    let mut ai = AIPlayer::new_with_team_factory(
        pid,
        Team::USA,
        AIDifficulty::Medium,
        world.team_factory.clone(),
    );
    ai.is_active = active;
    ai.skirmish_new_map_applied = true;
    ai.next_building_time = f32::MAX;
    ai.next_team_time = f32::MAX;
    ai.next_team_queue_time = f32::MAX;
    ai.enemy_check_time = f32::MAX;
    ai.team_queue.push_back(queued(id, unit));
    ai.team_ready_queue.push_back(queued(id, unit));
    ai
}

fn foreign_controller_bytes() -> Vec<u8> {
    use game_engine::common::system::Xfer;
    let mut bytes = Vec::new();
    let mut xfer =
        game_engine::system::xfer_save::XferSave::new(std::io::Cursor::new(&mut bytes), 1);
    xfer.open("foreign_team_ai").unwrap();
    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
        manager
            .with_ai_player_mut(1, |player| player.xfer(&mut xfer))
            .unwrap();
    })
    .unwrap();
    xfer.close().unwrap();
    drop(xfer);
    bytes
}

fn install_foreign(id: u32, unit: ObjectId) -> Arc<RwLock<gamelogic::object::Object>> {
    gamelogic::ai::integration::initialize_ai_integration().unwrap();
    let mut player = gamelogic::player::Player::new(1);
    player.set_can_build_units(true);
    gamelogic::player::player_list()
        .write()
        .unwrap()
        .add_player(Arc::new(RwLock::new(player)));
    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
        manager.ensure_ai_player(1, false);
        manager
            .with_ai_player_mut(1, |ai| {
                ai.select_skillset(9);
                ai.set_team_delay_seconds(19.0);
            })
            .unwrap();
    })
    .unwrap();
    let team = {
        let mut factory = gamelogic::team::get_team_factory().lock().unwrap();
        factory
            .init_team("DeleteTeam".into(), "".into(), false, None)
            .unwrap();
        let prototype = factory.find_team_prototype("DeleteTeam").unwrap();
        factory
            .create_team_on_prototype_with_id(&prototype, id)
            .unwrap()
    };
    let object = Arc::new(RwLock::new(gamelogic::object::Object::new_for_xfer_load(
        unit.0, 100.0,
    )));
    object.write().unwrap().set_team(Some(team)).unwrap();
    gamelogic::object::registry::OBJECT_REGISTRY.register_object(unit.0, &object);
    object
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn deletion_notifies_scripts_and_owned_players_before_members_and_metadata() {
    isolated(
        "deletion_notifies_scripts_and_owned_players_before_members_and_metadata",
        || {
            let (mut world, id, unit) = owner_world(false);
            let (second, other_id, other_unit) = owner_world(false);
            assert_eq!((id, unit), (other_id, other_unit));
            let foreign = install_foreign(id, unit);
            let foreign_before = foreign_controller_bytes();
            *gamelogic::scripting::engine::get_script_engine()
                .write()
                .unwrap() = Some(script_engine(id));
            let scripts = ScriptEngineHandle::from_engine(script_engine(id));
            let mut actor = controller(&world, 1, id, unit, false);
            world
                .ai_manager
                .ai_players
                .insert(0, controller(&world, 0, id, unit, false));
            world
                .ai_manager
                .ai_players
                .insert(2, controller(&world, 2, id, unit, false));
            world
                .get_player_mut(1)
                .unwrap()
                .set_team_relationship_override(
                    "DeleteTeam",
                    gamelogic::common::Relationship::Enemies,
                );
            let mut called = false;
            let mut notify = |current: &mut AIPlayer,
                              owner: &mut GameLogic,
                              deleted,
                              name: &str| {
                assert!(
                    !scripts
                        .read()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .has_active_sequential_script_for_team(deleted)
                );
                assert!(
                    owner
                        .team_factory
                        .lock()
                        .unwrap()
                        .find_team_by_id(deleted)
                        .is_some()
                );
                assert_eq!(
                    owner.host_object(unit).unwrap().team_instance_name,
                    "DeleteTeam"
                );
                assert_eq!(
                    owner
                        .get_player(1)
                        .unwrap()
                        .team_relationship_override(name),
                    None
                );
                super::team_lifecycle::notify_installed_team_destroy(current, owner, deleted, name);
                assert!(current.team_queue.is_empty() && current.team_ready_queue.is_empty());
                assert!(
                    owner
                        .ai_manager
                        .ai_players
                        .values()
                        .all(|ai| ai.team_queue.is_empty() && ai.team_ready_queue.is_empty())
                );
                called = true;
            };
            assert!(world.delete_ai_team_owned(&mut actor, id, &scripts, &mut notify));
            assert_eq!(
                foreign.read().unwrap().get_team_id(),
                Some(id),
                "foreign Core member must retain its team"
            );
            assert!(
                gamelogic::scripting::engine::get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .has_active_sequential_script_for_team(id),
                "deletion must use the explicitly borrowed script owner"
            );
            assert!(called);
            assert!(
                world
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(id)
                    .is_none()
            );
            assert!(
                world
                    .host_object(unit)
                    .unwrap()
                    .team_instance_name
                    .is_empty()
            );
            assert_eq!(world.host_object(unit).unwrap().owner_player_id, None);
            assert_eq!(foreign.read().unwrap().get_team_id(), Some(id));
            assert_eq!(foreign_controller_bytes(), foreign_before);
            assert!(
                second
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(id)
                    .is_some()
            );
            assert_eq!(
                second.host_object(other_unit).unwrap().team_instance_name,
                "DeleteTeam"
            );
            assert!(
                world.take_ai_team_destroy_notifications().is_empty(),
                "owned callbacks must not be queued for a later frame"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_ai_disband_purges_preceding_current_and_following_borrowed_players() {
    isolated(
        "actual_ai_disband_purges_preceding_current_and_following_borrowed_players",
        || {
            let (mut world, id, unit) = owner_world(false);
            let (second, other_id, other_unit) = owner_world(false);
            let foreign = install_foreign(id, unit);
            let before = foreign_controller_bytes();
            *gamelogic::scripting::engine::get_script_engine()
                .write()
                .unwrap() = Some(script_engine(id));
            let unrelated = world
                .team_factory
                .lock()
                .unwrap()
                .create_inactive_team("DeleteTeam")
                .unwrap();
            let unrelated_id = unrelated.read().unwrap().get_id();
            assert_ne!(unrelated_id, id);
            for (pid, active) in [(0, false), (1, true), (2, false)] {
                let mut ai = controller(&world, pid, id, unit, active);
                // Current's build queue expires. Its ready queue is observed by
                // inactive peer callbacks, not activated before the disband.
                if active {
                    ai.team_ready_queue.clear();
                }
                if pid == 0 {
                    // C++ pointer identity: both a null handle and another
                    // real instance of the same prototype survive deletion.
                    for identity in [None, Some(unrelated_id)] {
                        let mut survivor = queued(id, unit);
                        survivor.team_id = identity;
                        ai.team_queue.push_back(survivor.clone());
                        ai.team_ready_queue.push_back(survivor);
                    }
                }
                world.ai_manager.ai_players.insert(pid, ai);
            }
            AIManager::update_owned(&mut world, 1.0);
            for (&pid, ai) in &world.ai_manager.ai_players {
                assert!(
                    ai.team_queue
                        .iter()
                        .chain(ai.team_ready_queue.iter())
                        .all(|q| q.team_id != Some(id)),
                    "deleted instance must be purged synchronously from every borrowed controller"
                );
                if pid == 0 {
                    for queue in [&ai.team_queue, &ai.team_ready_queue] {
                        assert_eq!(
                            queue.iter().map(|q| q.team_id).collect::<Vec<_>>(),
                            vec![None, Some(unrelated_id)],
                            "same-name null and distinct instance handles must survive"
                        );
                    }
                } else {
                    assert!(ai.team_queue.is_empty() && ai.team_ready_queue.is_empty());
                }
            }
            assert!(
                world
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(unrelated_id)
                    .is_some()
            );
            // Preserve the separate legacy no-identity notification adapter.
            let peer = world.ai_manager.ai_players.get_mut(&0).unwrap();
            peer.ai_pre_team_destroy(None, "DeleteTeam");
            for queue in [&peer.team_queue, &peer.team_ready_queue] {
                assert_eq!(
                    queue.iter().map(|q| q.team_id).collect::<Vec<_>>(),
                    vec![Some(unrelated_id)]
                );
            }

            assert!(
                world
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(id)
                    .is_none()
            );
            assert_eq!(
                world.host_object(unit).unwrap().team_instance_name,
                "teamTeamOwner"
            );
            let factory = world.team_factory.lock().unwrap();
            let default = factory
                .find_team_instances("teamTeamOwner")
                .into_iter()
                .next()
                .unwrap();
            assert!(default.read().unwrap().get_members().contains(&unit.0));
            drop(factory);
            assert!(
                !gamelogic::scripting::engine::get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .has_active_sequential_script_for_team(id)
            );
            assert_eq!(foreign.read().unwrap().get_team_id(), Some(id));
            assert_eq!(foreign_controller_bytes(), before);
            assert_eq!((id, unit), (other_id, other_unit));
            assert!(
                second
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(id)
                    .is_some()
            );
            assert_eq!(
                second.host_object(other_unit).unwrap().team_instance_name,
                "DeleteTeam"
            );
            assert!(world.take_ai_team_destroy_notifications().is_empty());
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn singleton_disband_transfers_members_without_destruction_callbacks() {
    isolated(
        "singleton_disband_transfers_members_without_destruction_callbacks",
        || {
            let (mut world, id, unit) = owner_world(true);
            let scripts = ScriptEngineHandle::from_engine(script_engine(id));
            let mut actor = controller(&world, 1, id, unit, false);
            let queue = actor.team_queue.front().unwrap().clone();
            actor.disband_queued_team_with_owner(
                &mut world,
                &queue,
                false,
                &scripts,
                &mut |_, _, _, _| panic!("singleton was not deleted"),
            );
            assert!(
                world
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(id)
                    .is_some()
            );
            assert_eq!(
                world.host_object(unit).unwrap().team_instance_name,
                "teamTeamOwner"
            );
            assert!(
                scripts
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .has_active_sequential_script_for_team(id)
            );
            assert_eq!(actor.team_queue.len(), 1);
        },
    );
}
