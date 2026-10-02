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
        for object in [&factory, &unit] {
            OBJECT_REGISTRY.register_object(object.read().unwrap().get_id(), object);
        }
        Self { factory, unit }
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
    ai.on_unit_produced(&objects.factory, &objects.unit);
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
        ai.on_unit_produced(&objects.factory, &objects.unit);
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
    assert!(OBJECT_REGISTRY.is_empty());
    let mut ai = AIPlayer::new(0);
    let mut other = AIPlayer::new(1);
    ai.team_delay = 99;
    ai.build_delay = 80;
    ai.structure_timer = 60;
    other.team_delay = 77;
    other.build_delay = 66;
    // C++ null factory is the one early return before the wake-up.
    ai.on_unit_produced(INVALID_ID, INVALID_ID).unwrap();
    assert_eq!(ai.team_delay, 99);
    ai.on_unit_produced(0xB0_8101, 0xB0_8102).unwrap();
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
