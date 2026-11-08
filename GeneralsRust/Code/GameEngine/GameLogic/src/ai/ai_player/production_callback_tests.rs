//! Original AIPlayer.cpp1024-1136 / AISkirmishPlayer.cpp295-297.
use super::*;
use crate::ai::ai_player::TeamInQueue;
use crate::ai::skirmish_player::AISkirmishPlayer;
use crate::common::DefaultThingTemplate;
use crate::object::Object;
use crate::object::registry::test_isolation_lock;
use crate::team::TeamPrototype;
use crate::team::{TeamFactory, get_team_factory};
use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

struct ProductionObjects {
    factory: Arc<RwLock<Object>>,
    unit: Arc<RwLock<Object>>,
    world: crate::system::game_logic::GameLogic,
    player: RwLock<Player>,
}

impl ProductionObjects {
    fn new(dozer: bool) -> Self {
        let factory = Arc::new(RwLock::new(Object::new_test(0xB0_8101, 100.0)));
        let mut template = DefaultThingTemplate::new("ProductionCallbackUnit".into());
        if dozer {
            template.add_kind_of(KindOf::Dozer);
        }
        let unit = Arc::new(RwLock::new(Object::new_test_from_template(
            0xB0_8102,
            100.0,
            Arc::new(template),
        )));
        let mut world = crate::system::game_logic::GameLogic::new();
        for object in [&factory, &unit] {
            OBJECT_REGISTRY.register_object(object.read().unwrap().get_id(), object);
            world.track_object_in_update_list(Arc::clone(object)).unwrap();
        }
        Self { factory, unit, world, player: RwLock::new(Player::new(0)) }
    }
}

impl Drop for ProductionObjects {
    fn drop(&mut self) {
        for object in [&self.factory, &self.unit] {
            OBJECT_REGISTRY.unregister_object(object.read().unwrap().get_id());
        }
    }
}

struct FactoryRestore(Option<TeamFactory>);

impl FactoryRestore {
    fn new() -> Self {
        Self(Some(std::mem::replace(
            &mut *get_team_factory().lock().unwrap(),
            TeamFactory::new(),
        )))
    }
}

impl Drop for FactoryRestore {
    fn drop(&mut self) {
        *get_team_factory().lock().unwrap() = self.0.take().unwrap();
    }
}

fn roundtrip(ai: &mut AIPlayer) -> AIPlayer {
    let mut bytes = Vec::new();
    ai.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1));
    let mut loaded = AIPlayer::new(ai.player_id);
    loaded.xfer(&mut XferLoad::new(Cursor::new(bytes), 1));
    loaded.load_post_process();
    loaded
}

#[test]
fn restored_skirmish_delivery_completes_first_order_and_assigns_team() {
    let _isolation = test_isolation_lock().lock().unwrap();
    let _factory = FactoryRestore::new();
    let objects = ProductionObjects::new(false);
    let factory_id = objects.factory.read().unwrap().get_id();
    let unit_id = objects.unit.read().unwrap().get_id();
    let prototype = TeamPrototype::new("ProductionCallbackTeam".into());
    let team = get_team_factory()
        .lock()
        .unwrap()
        .create_team_on_prototype_with_id(&prototype, 0xB0_8201)
        .unwrap();
    team.write().unwrap().set_controlling_player_id(None);
    let mut ai = AISkirmishPlayer::new(0);
    ai.base_for_test().set_team_delay_frames(99);
    let mut queued = TeamInQueue::new();
    queued.team = Some(team.clone());
    queued.team_name = Some("ProductionCallbackTeam".into());
    queued.reinforcement = true;
    let mut order = WorkOrder::new("ProductionCallbackUnit".into());
    order.factory_id = Some(factory_id);
    order.num_required = 2;
    let mut wrong_factory = order.clone();
    wrong_factory.factory_id = Some(factory_id + 10);
    let mut completed = order.clone();
    completed.num_completed = 2;
    queued.work_orders = vec![wrong_factory, completed, order.clone(), order];
    ai.base_for_test().team_build_queue.push_back(queued);
    // The real queue Xfer resolves the team ID from TeamFactory before delivery.
    let restored = roundtrip(ai.base_for_test());
    *ai.base_for_test() = restored;
    assert!(Arc::ptr_eq(
        ai.base_for_test().team_build_queue[0]
            .team
            .as_ref()
            .unwrap(),
        &team
    ));
    ai.on_unit_produced(&objects.world, &objects.player, &objects.factory, &objects.unit);
    let queued = &ai.base_for_test().team_build_queue[0];
    assert_eq!(queued.work_orders[0].num_completed, 0);
    assert_eq!(queued.work_orders[1].num_completed, 2);
    assert_eq!(queued.work_orders[2].num_completed, 1);
    assert_eq!(queued.work_orders[2].factory_id, None);
    assert_eq!(queued.work_orders[3].num_completed, 0);
    assert_eq!(queued.work_orders[3].factory_id, Some(factory_id));
    assert_eq!(queued.reinforcement_id, Some(unit_id));
    assert_eq!(ai.base_for_test().get_team_delay(), 0);
    assert!(Arc::ptr_eq(
        &objects.unit.read().unwrap().get_team().unwrap(),
        &team
    ));
    let loaded = roundtrip(ai.base_for_test());
    assert_eq!(loaded.team_build_queue[0].work_orders[2].num_completed, 1);
    assert_eq!(loaded.team_build_queue[0].work_orders[2].factory_id, None);
    assert_eq!(loaded.team_build_queue[0].reinforcement_id, Some(unit_id));
    assert_eq!(loaded.get_team_delay(), 0);
}

#[test]
fn unqueued_dozer_delivery_preserves_cpp_repair_and_build_shortcuts() {
    let _isolation = test_isolation_lock().lock().unwrap();
    let objects = ProductionObjects::new(true);
    let unit_id = objects.unit.read().unwrap().get_id();
    for repair in [false, true] {
        let mut ai = AISkirmishPlayer::new(0);
        ai.base_for_test().dozer_queued_for_repair = repair;
        ai.base_for_test().team_delay = 99;
        ai.base_for_test().build_delay = 80;
        ai.base_for_test().structure_timer = 60;
        ai.on_unit_produced(&objects.world, &objects.player, &objects.factory, &objects.unit);
        assert_eq!(ai.base_for_test().team_delay, 0);
        assert_eq!(ai.base_for_test().build_delay, if repair { 80 } else { 0 });
        assert_eq!(
            ai.base_for_test().structure_timer,
            if repair { 60 } else { 1 }
        );
        assert_eq!(ai.base_for_test().repair_dozer, repair.then_some(unit_id));
        assert!(!ai.base_for_test().dozer_queued_for_repair);
    }
}

#[test]
fn no_object_production_notifications_keep_local_timer_and_null_factory_contracts() {
    let _isolation = test_isolation_lock().lock().unwrap();
    OBJECT_REGISTRY.clear();
    let world = crate::system::game_logic::GameLogic::new();
    let player = RwLock::new(Player::new(0));
    assert!(OBJECT_REGISTRY.is_empty());
    let mut ai = AIPlayer::new(0);
    let mut other = AIPlayer::new(1);
    ai.team_delay = 99;
    ai.build_delay = 80;
    ai.structure_timer = 60;
    other.team_delay = 77;
    other.build_delay = 66;
    // C++ null factory is the one early return before the wake-up.
    ai.on_unit_produced(&world, &player, INVALID_ID, INVALID_ID).unwrap();
    assert_eq!(ai.team_delay, 99);
    ai.on_unit_produced(&world, &player, 0xB0_8101, 0xB0_8102).unwrap();
    assert_eq!(ai.team_delay, 0);
    assert_eq!(ai.build_delay, 80);
    assert_eq!(ai.structure_timer, 60);
    ai.team_delay = 99;
    ai.on_structure_produced(INVALID_ID, 0xB0_8102).unwrap();
    assert_eq!(ai.team_delay, 0);
    assert_eq!(ai.build_delay, 0);
    assert_eq!(ai.structure_timer, 60);
    assert_eq!(other.team_delay, 77);
    assert_eq!(other.build_delay, 66);
    let loaded = roundtrip(&mut ai);
    assert_eq!(loaded.team_delay, 0);
    assert_eq!(loaded.build_delay, 0);
    assert_eq!(loaded.structure_timer, 60);
    let other_loaded = roundtrip(&mut other);
    assert_eq!(other_loaded.team_delay, 77);
    assert_eq!(other_loaded.build_delay, 66);
}

#[test]
fn supply_ledger_uses_receiving_world_and_unlocks_player_before_each_dock() {
    fn player() -> RwLock<Player> {
        let mut tail = crate::build_list_info::BuildListInfo::new();
        tail.set_object_id(0xB0_8302);
        tail.set_supply_building(true);
        tail.set_desired_gatherers(2);
        let mut missing = crate::build_list_info::BuildListInfo::new();
        missing.set_object_id(0xB0_8303);
        missing.set_supply_building(true);
        missing.set_desired_gatherers(2);
        missing.set_next_build_list(Some(tail));
        let mut first = crate::build_list_info::BuildListInfo::new();
        first.set_object_id(0xB0_8301);
        first.set_supply_building(true);
        first.set_desired_gatherers(2);
        first.set_next_build_list(Some(missing));
        let mut player = Player::new(0);
        player.set_build_list(Some(first));
        RwLock::new(player)
    }
    fn counts(player: &Player) -> Vec<i32> {
        let mut counts = Vec::new();
        let mut row = player.get_build_list();
        while let Some(info) = row {
            counts.push(info.get_current_gatherers());
            row = info.get_next();
        }
        counts
    }
    let mut world = crate::system::game_logic::GameLogic::new();
    let mut peer = crate::system::game_logic::GameLogic::new();
    for id in [0xB0_8301, 0xB0_8302] {
        world.track_object_in_update_list(
            Arc::new(RwLock::new(Object::new_test(id, 100.0)))
        ).unwrap();
    }
    peer.track_object_in_update_list(
        Arc::new(RwLock::new(Object::new_test(0xB0_8302, 100.0)))
    ).unwrap();
    let owner = player();
    let other = player();
    let mut visits = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&owner, &world, |id| {
        let guard = owner.try_read().expect("dock must not hold the player lock");
        visits.push((id, counts(&guard)));
    }).unwrap();
    assert_eq!(visits, [
        (0xB0_8301, vec![1, 0, 0]),
        (0xB0_8302, vec![1, 0, 1]),
    ]);
    assert_eq!(counts(&other.read().unwrap()), [0, 0, 0]);
    let mut peer_visits = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&other, &peer, |id| peer_visits.push(id)).unwrap();
    assert_eq!(peer_visits, [0xB0_8302]);
    assert_eq!(counts(&other.read().unwrap()), [0, 0, 1]);
    assert_eq!(counts(&owner.read().unwrap()), [1, 0, 1]);
}

#[test]
fn supply_delivery_follows_live_node_links_across_reentrant_edits() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
    use game_engine::common::system::snapshot::Snapshotable;

    let _isolation = test_isolation_lock().lock().unwrap();
    let _factory_restore = FactoryRestore::new();
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object SupplyDeliveryCursorChinook\n KindOf = AIRCRAFT\n Behavior = ChinookAIUpdate CursorFlight\n MaxBoxes = 2\n NumRopes = 1\n End\n End\n"
    ), 1);
    let mut factory = ObjectFactory::new();
    let unit_id = factory.create_object(
        "SupplyDeliveryCursorChinook", Coord3D::new(20.0, 30.0, 50.0),
        None, ObjectCreationFlags::empty()
    ).unwrap();
    let unit = factory.get_object(unit_id).unwrap().get_base_object().unwrap();
    assert!(unit.read().unwrap().get_ai_update_interface().unwrap()
        .lock().unwrap().get_supply_truck_ai_interface().is_some());
    fn row(id: u32) -> crate::build_list_info::BuildListInfo {
        let mut row = crate::build_list_info::BuildListInfo::new();
        row.set_object_id(id);
        row.set_supply_building(true);
        row.set_desired_gatherers(3);
        row
    }
    let (a, b, c, inserted) = (0xB0_8401, 0xB0_8402, 0xB0_8403, 0xB0_8404);
    let mut world = crate::system::game_logic::GameLogic::new();
    world.track_object_in_update_list(unit).unwrap();
    for id in [a, b, c, inserted] {
        world.track_object_in_update_list(Arc::new(RwLock::new(Object::new_test(id, 100.0)))).unwrap();
    }
    let mut first = row(a);
    let mut second = row(b);
    second.set_next_build_list(Some(row(c)));
    first.set_next_build_list(Some(second));
    let mut player = Player::new(0);
    player.set_build_list(Some(first));
    let player = RwLock::new(player);
    let factory_id = 0xB0_8490;
    let mut ai = AIPlayer::new(0);
    let mut order = WorkOrder::new("SupplyDeliveryCursorChinook".into());
    order.factory_id = Some(factory_id);
    order.num_required = 1;
    order.is_resource_gatherer = true;
    let mut team = TeamInQueue::new();
    team.work_orders.push(order);
    ai.team_build_queue.push_back(team);
    let mut visits = Vec::new();
    ai.on_unit_produced_with_dock(&world, &player, factory_id, unit_id, |unit_ai, id| {
        unit_ai.ai_dock(id, CommandSourceType::FromPlayer);
        let mut player = player.write().unwrap();
        let mut state = Vec::new();
        let mut node = player.get_build_list();
        while let Some(entry) = node {
            state.push((entry.get_object_id(), entry.get_current_gatherers()));
            node = entry.get_next();
        }
        visits.push((id, state));
        if id == a {
            // Native addToPriorityBuildList prepends without moving the current node.
            player.add_to_priority_build_list("new head".into(), Coord3D::origin(), 0.0);
            let current = player.get_build_list_mut().unwrap().get_next_mut().unwrap();
            let tail = current.take_next_build_list();
            let mut extra = row(inserted);
            extra.set_next_build_list_boxed(tail);
            current.set_next_build_list(Some(extra));
        } else if id == inserted {
            // Remove the prepended predecessor and the unvisited B; keep the
            // current node alive while its successor changes to C.
            let mut old_head = player.get_build_list_mut().unwrap().take_next_build_list().unwrap();
            let current = old_head.get_next_mut().unwrap();
            let mut removed = current.take_next_build_list().unwrap();
            current.set_next_build_list_boxed(removed.take_next_build_list());
            player.set_build_list(Some(*old_head));
        }
    }).unwrap();
    assert_eq!(visits, [
        (a, vec![(a, 1), (b, 0), (c, 0)]),
        (inserted, vec![(INVALID_ID, 0), (a, 1), (inserted, 1), (b, 0), (c, 0)]),
        (c, vec![(a, 1), (inserted, 1), (c, 1)]),
    ]);
    assert_eq!(ai.team_build_queue[0].work_orders[0].num_completed, 1);
    assert_eq!(ai.team_build_queue[0].work_orders[0].factory_id, None);
    let mut bytes = Vec::new();
    Snapshotable::xfer(&mut *player.write().unwrap(), &mut XferSave::new(Cursor::new(&mut bytes), 1)).unwrap();
    let mut restored = Player::new(0);
    Snapshotable::xfer(&mut restored, &mut XferLoad::new(Cursor::new(bytes), 1)).unwrap();
    let restored = RwLock::new(restored);
    let mut peer_world = crate::system::game_logic::GameLogic::new();
    peer_world.track_object_in_update_list(Arc::new(RwLock::new(Object::new_test(c, 100.0)))).unwrap();
    peer_world.track_object_in_update_list(factory.get_object(unit_id).unwrap().get_base_object().unwrap()).unwrap();
    let mut peer_player = Player::new(0);
    let mut peer_first = row(a);
    peer_first.set_next_build_list(Some(row(c)));
    peer_player.set_build_list(Some(peer_first));
    let peer_player = RwLock::new(peer_player);
    let mut peer_visits = Vec::new();
    let mut peer_ai = AIPlayer::new(0);
    let mut peer_order = WorkOrder::new("SupplyDeliveryCursorChinook".into());
    peer_order.factory_id = Some(factory_id);
    peer_order.num_required = 1;
    peer_order.is_resource_gatherer = true;
    let mut peer_team = TeamInQueue::new();
    peer_team.work_orders.push(peer_order);
    peer_ai.team_build_queue.push_back(peer_team);
    peer_ai.on_unit_produced_with_dock(&peer_world, &peer_player, factory_id, unit_id, |unit_ai, id| {
        unit_ai.ai_dock(id, CommandSourceType::FromPlayer);
        peer_visits.push(id);
    }).unwrap();
    assert_eq!(peer_visits, [c]);
    let mut continuation = Vec::new();
    ai.team_build_queue[0].work_orders[0].factory_id = Some(factory_id);
    ai.team_build_queue[0].work_orders[0].num_required = 2;
    ai.on_unit_produced_with_dock(&world, &restored, factory_id, unit_id, |unit_ai, id| {
        unit_ai.ai_dock(id, CommandSourceType::FromPlayer);
        continuation.push(id);
    }).unwrap();
    assert_eq!(continuation, [a, inserted, c]);
    let restored = restored.read().unwrap();
    let a_row = restored.get_build_list().unwrap();
    let inserted_row = a_row.get_next().unwrap();
    let c_row = inserted_row.get_next().unwrap();
    assert_eq!((a_row.get_current_gatherers(), inserted_row.get_current_gatherers(), c_row.get_current_gatherers()), (2, 2, 2));
}

#[test]
fn supply_delivery_preserves_independent_world_ledgers_through_player_xfer() {
    use game_engine::common::system::snapshot::Snapshotable;

    fn player() -> Player {
        let mut a = crate::build_list_info::BuildListInfo::new();
        a.set_object_id(0xB0_8501);
        a.set_supply_building(true);
        a.set_desired_gatherers(4);
        let mut b = crate::build_list_info::BuildListInfo::new();
        b.set_object_id(0xB0_8502);
        b.set_supply_building(true);
        b.set_desired_gatherers(4);
        a.set_next_build_list(Some(b));
        let mut player = Player::new(0);
        player.set_build_list(Some(a));
        player
    }
    let mut world = crate::system::game_logic::GameLogic::new();
    let mut peer_world = crate::system::game_logic::GameLogic::new();
    for id in [0xB0_8501, 0xB0_8502] {
        world.track_object_in_update_list(Arc::new(RwLock::new(Object::new_test(id, 100.0)))).unwrap();
    }
    peer_world.track_object_in_update_list(
        Arc::new(RwLock::new(Object::new_test(0xB0_8502, 100.0)))
    ).unwrap();
    let owner = RwLock::new(player());
    let peer = RwLock::new(player());
    let mut visits = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&owner, &world, |id| visits.push(id)).unwrap();
    assert_eq!(visits, [0xB0_8501, 0xB0_8502]);
    let mut bytes = Vec::new();
    Snapshotable::xfer(&mut *owner.write().unwrap(), &mut XferSave::new(Cursor::new(&mut bytes), 1)).unwrap();
    let mut loaded = Player::new(0);
    Snapshotable::xfer(&mut loaded, &mut XferLoad::new(Cursor::new(bytes), 1)).unwrap();
    let loaded = RwLock::new(loaded);
    let mut loaded_visits = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&loaded, &world, |id| loaded_visits.push(id)).unwrap();
    assert_eq!(loaded_visits, [0xB0_8501, 0xB0_8502]);
    let mut peer_visits = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&peer, &peer_world, |id| peer_visits.push(id)).unwrap();
    assert_eq!(peer_visits, [0xB0_8502]);
    for (player, expected) in [(&owner, (1, 1)), (&loaded, (2, 2)), (&peer, (0, 1))] {
        let player = player.read().unwrap();
        let first = player.get_build_list().unwrap();
        assert_eq!((first.get_current_gatherers(), first.get_next().unwrap().get_current_gatherers()), expected);
    }
}

#[test]
fn nested_supply_delivery_keeps_the_outer_current_node() {
    let mut world = crate::system::game_logic::GameLogic::new();
    for id in [0xB0_8601, 0xB0_8602] {
        world.track_object_in_update_list(Arc::new(RwLock::new(Object::new_test(id, 100.0)))).unwrap();
    }
    let mut first = crate::build_list_info::BuildListInfo::new();
    first.set_object_id(0xB0_8601);
    first.set_supply_building(true);
    first.set_desired_gatherers(4);
    let mut second = crate::build_list_info::BuildListInfo::new();
    second.set_object_id(0xB0_8602);
    second.set_supply_building(true);
    second.set_desired_gatherers(4);
    first.set_next_build_list(Some(second));
    let mut owner = Player::new(0);
    owner.set_build_list(Some(first));
    let owner = RwLock::new(owner);
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    AIPlayer::visit_supply_gatherer_slots(&owner, &world, |id| {
        outer.push(id);
        if id == 0xB0_8601 {
            AIPlayer::visit_supply_gatherer_slots(&owner, &world, |id| inner.push(id)).unwrap();
        }
    }).unwrap();
    assert_eq!(outer, [0xB0_8601, 0xB0_8602]);
    assert_eq!(inner, [0xB0_8601, 0xB0_8602]);
    let owner = owner.read().unwrap();
    let first = owner.get_build_list().unwrap();
    assert_eq!(first.get_current_gatherers(), 2);
    assert_eq!(first.get_next().unwrap().get_current_gatherers(), 2);
}

#[test]
fn removing_current_supply_node_is_an_explicit_error() {
    let id = 0xB0_8701;
    let mut world = crate::system::game_logic::GameLogic::new();
    world.track_object_in_update_list(Arc::new(RwLock::new(Object::new_test(id, 100.0)))).unwrap();
    let mut row = crate::build_list_info::BuildListInfo::new();
    row.set_object_id(id);
    row.set_supply_building(true);
    row.set_desired_gatherers(1);
    let mut player = Player::new(0);
    player.set_build_list(Some(row));
    let player = RwLock::new(player);
    let result = AIPlayer::visit_supply_gatherer_slots(&player, &world, |_| {
        player.write().unwrap().set_build_list(None);
    });
    assert_eq!(result, Err(AiError::InvalidObject));
}
