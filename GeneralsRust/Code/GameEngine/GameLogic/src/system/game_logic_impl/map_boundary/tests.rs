use super::super::TerrainLogicSnapshotBridge;
use super::*;
use crate::common::ICoord2D;
use crate::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use crate::system::engine_stores::{self, EngineStores, with_active_stores};
use crate::system::map_loader::MapData;
use crate::system::shroud_manager::ShroudState;
use game_engine::{XferLoad, XferSave};

fn admitted_world() -> (Arc<EngineStores>, Arc<RwLock<Object>>) {
    admitted_world_with_actor(9191)
}

fn admitted_world_with_actor(id: ObjectID) -> (Arc<EngineStores>, Arc<RwLock<Object>>) {
    let stores = engine_stores::new_for_world();
    let mut map = MapData::new();
    map.width = 10;
    map.height = 10;
    map.heightmap = vec![0; 100];
    map.boundaries = vec![ICoord2D::new(10, 10), ICoord2D::new(5, 5)];
    stores
        .services()
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    stores
        .services()
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(100.0, 100.0);
    let actor = with_active_stores(&stores, || {
        let mut actor = Object::new_test(id, 100.0);
        actor.bind_owner_world_for_test(stores.services());
        actor.set_position(&Coord3D::new(75.0, 25.0, 0.0)).unwrap();
        let actor = Arc::new(RwLock::new(actor));
        stores
            .game_logic_mutex()
            .lock()
            .unwrap()
            .register_object(Arc::clone(&actor))
            .unwrap();
        actor
    });
    (stores, actor)
}

fn boundary(stores: &EngineStores) -> i32 {
    stores
        .services()
        .terrain()
        .read()
        .unwrap()
        .get_active_boundary()
}

#[test]
fn active_boundary_change_notifies_only_the_objects_on_its_map() {
    let _lock = crate::test_sync::lock();
    let (owner, actor) = admitted_world();
    let (peer, same_id) = admitted_world();
    let owner_radar = boundary_radar(owner.services());
    let peer_radar = boundary_radar(peer.services());
    let inside = Coord3D::new(20.0, 20.0, 0.0);
    let clipped = Coord3D::new(95.0, 20.0, 0.0);
    {
        let mut shroud = owner.services().shroud().lock().unwrap();
        shroud.do_shroud_reveal(&inside, 10.0, 1);
        shroud.queue_undo_shroud_reveal(&inside, 10.0, 1, 30, 0);
        shroud.do_shroud_reveal(&clipped, 10.0, 2);
        assert_eq!(shroud.get_shroud_state(0, &inside), ShroudState::Visible);
    }
    let owner_pathfinder = owner.ai().read().unwrap().pathfinder().unwrap();
    let original_ghost = {
        let mut logic = owner.game_logic_mutex().lock().unwrap();
        let blocked = owner_pathfinder.read().unwrap();
        assert!(logic.switch_active_map_boundary(1).is_err());
        assert_eq!(boundary(&owner), 0);
        assert!(!owner_radar.read().unwrap().has_map_extent());
        drop(blocked);
        let mut ghosts = owner.services().w3d_ghost_objects().write().unwrap();
        logic
            .partition_manager_mut()
            .attach_object_ghost(9191, true, &mut ghosts)
            .unwrap()
    };
    let peer_shroud = peer.services().shroud().lock().unwrap().snapshot_state();
    assert!(!actor.read().unwrap().is_off_map());
    assert!(!same_id.read().unwrap().is_off_map());
    with_active_stores(&peer, || {
        let mut logic = owner.game_logic_mutex().lock().unwrap();
        logic.switch_active_map_boundary(1).unwrap();
        assert!(logic.partition_manager().contains_object(9191));
        assert_eq!(
            logic.partition_manager().ghost_link_scene_id(9191),
            Some(original_ghost)
        );
    });
    assert_eq!(boundary(&owner), 1);
    assert_eq!(boundary(&peer), 0);
    assert!(actor.read().unwrap().is_off_map());
    assert!(!same_id.read().unwrap().is_off_map());
    assert_eq!(owner_radar.read().unwrap().map_extent().hi.x, 50.0);
    assert!(!peer_radar.read().unwrap().has_map_extent());
    assert_eq!(
        peer.services().shroud().lock().unwrap().snapshot_state(),
        peer_shroud
    );
    {
        let shroud = owner.services().shroud().lock().unwrap();
        assert_eq!(shroud.get_shroud_state(0, &inside), ShroudState::Explored);
        assert_eq!(shroud.get_shroud_state(1, &clipped), ShroudState::Hidden);
    }
    owner
        .game_logic_mutex()
        .lock()
        .unwrap()
        .switch_active_map_boundary(0)
        .unwrap();
    assert!(!actor.read().unwrap().is_off_map());
}

#[test]
fn authored_boundary_script_uses_the_driving_core_loan() {
    let _lock = crate::test_sync::lock();
    let (owner, actor) = admitted_world();
    let (peer, same_id) = admitted_world();
    owner
        .services()
        .scripts()
        .initialize_engine(owner.services())
        .unwrap();
    let mut action = ScriptAction::new(ScriptActionType::MapSwitchBorder);
    action
        .add_parameter(Parameter::with_int(ParameterType::Boundary, 1))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    owner
        .services()
        .scripts()
        .engine()
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    with_active_stores(&peer, || {
        owner
            .game_logic_mutex()
            .lock()
            .unwrap()
            .evaluate_scripts()
            .unwrap();
    });
    assert_eq!((boundary(&owner), boundary(&peer)), (1, 0));
    assert!(actor.read().unwrap().is_off_map());
    assert!(!same_id.read().unwrap().is_off_map());
}

#[test]
fn authored_occlusion_script_sets_the_driving_core_while_a_peer_is_published() {
    let _lock = crate::test_sync::lock();
    let owner = engine_stores::new_for_world();
    let peer = engine_stores::new_for_world();
    owner.services().scripts().initialize_engine(owner.services()).unwrap();
    let mut action = ScriptAction::new(ScriptActionType::OptionsSetOcclusionMode);
    action.add_parameter(Parameter::with_int(ParameterType::Boolean, 0)).unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    owner.services().scripts().engine().write().unwrap().as_mut().unwrap()
        .set_script_list_for_player(0, Some(Box::new(list))).unwrap();

    with_active_stores(&peer, || {
        owner.game_logic_mutex().lock().unwrap().evaluate_scripts().unwrap();
    });
    assert!(!owner.game_logic_mutex().lock().unwrap().get_show_behind_building_markers());
    assert!(peer.game_logic_mutex().lock().unwrap().get_show_behind_building_markers());
}

#[test]
fn terrain_snapshot_rebuilds_the_registered_recipient_boundary() {
    let _lock = crate::test_sync::lock();
    let (source, _) = admitted_world();
    let (recipient, actor) = admitted_world();
    let mut source_bridge = TerrainLogicSnapshotBridge {
        logic: source.game_logic_mutex(),
        terrain: Arc::clone(source.services().terrain()),
    };
    let mut recipient_bridge = TerrainLogicSnapshotBridge {
        logic: recipient.game_logic_mutex(),
        terrain: Arc::clone(recipient.services().terrain()),
    };
    source
        .game_logic_mutex()
        .lock()
        .unwrap()
        .switch_active_map_boundary(1)
        .unwrap();
    let path = std::env::temp_dir().join(format!("core-boundary-xfer-{}.bin", std::process::id()));
    let path = path.to_str().unwrap();
    with_active_stores(&recipient, || {
        let mut save = XferSave::new();
        save.open(path.into()).unwrap();
        source_bridge.xfer(&mut save).unwrap();
        save.close().unwrap();
    });
    assert_eq!(std::fs::read(path).unwrap(), [2, 1, 0, 0, 0, 0, 0, 0, 0]);
    with_active_stores(&source, || {
        let mut load = XferLoad::new();
        load.open(path.into()).unwrap();
        recipient_bridge.xfer(&mut load).unwrap();
        load.close().unwrap();
    });
    std::fs::remove_file(path).unwrap();
    assert_eq!(boundary(&recipient), 1);
    assert!(actor.read().unwrap().is_off_map());
    let malformed = [2, 0, 0, 0, 0, 65, 0, 0, 0];
    std::fs::write(path, malformed).unwrap();
    let mut load = XferLoad::new();
    load.open(path.into()).unwrap();
    assert!(matches!(
        recipient_bridge.xfer(&mut load),
        Err(XferStatus::InvalidData)
    ));
    load.close().unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(boundary(&recipient), 1);
}

#[test]
fn terrain_snapshot_post_process_checks_bridge_objects_in_the_recipient_only() {
    use crate::path::PathfindLayerEnum;
    use crate::terrain::TerrainLogic;

    let _lock = crate::test_sync::lock();
    let (recipient, _) = admitted_world();
    let (peer, _) = admitted_world_with_actor(9192);
    let mut bridge = TerrainLogicSnapshotBridge {
        logic: recipient.game_logic_mutex(),
        terrain: Arc::clone(recipient.services().terrain()),
    };
    for (id, layer) in [
        (9191, PathfindLayerEnum::Bridge1),
        (9192, PathfindLayerEnum::Bridge2),
    ] {
        let info =
            TerrainLogic::bridge_info_from_parts(Coord3D::new(30.0, 30.0, 0.0), 0.0, 6.0, 2.0, id);
        bridge.terrain.write().unwrap().prepend_bridge_on_layer(
            info,
            AsciiString::from("TestBridge"),
            layer,
        );
    }
    with_active_stores(&peer, || bridge.load_post_process().unwrap());
    let terrain = bridge.terrain.read().unwrap();
    let mut ids = Vec::new();
    terrain.for_each_bridge(|bridge| ids.push(bridge.get_bridge_info().bridge_object_id));
    assert_eq!(ids, [9191]);
}
