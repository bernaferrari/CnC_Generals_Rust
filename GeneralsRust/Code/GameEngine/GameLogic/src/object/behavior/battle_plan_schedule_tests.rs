//! Real factory/normal-dispatch control of the installed detector.
//! CPP BattlePlanUpdate.cpp:324–373,740–767,824–833 and
//! StealthDetectorUpdate.cpp:82–85; no fabricated countdown or wake sentinel.
use super::*;
use crate::object::ModuleUpdateProxy;
use crate::object::behavior::stealth_detector_update::{
    StealthDetectorUpdateModuleData, stealth_detector_update_tests::DetectorFixture,
};
use crate::object::object_thing::ObjectThingHandle;
use crate::player::{Player, player_list};
use crate::team::Team;
use game_engine::common::thing::module::{ModuleType, Thing as ModuleThing};
use game_engine::common::thing::module_factory::ModuleFactory;

#[test]
fn authored_normal_battle_plan_controls_registered_detector_before_next_callback() {
    let _guard = crate::test_sync::lock();
    let _players = super::delete_tests::PlayerListRestore::new();
    let player = Arc::new(RwLock::new(Player::new(0)));
    player_list().write().unwrap().add_player(player.clone());
    let mut detector_data = StealthDetectorUpdateModuleData::default();
    detector_data.initially_disabled = true;
    detector_data.update_rate = 10;
    let mut fixture = DetectorFixture::new(detector_data, 0x5D37_0020);
    let team = Arc::new(RwLock::new(Team::new(
        "DetectorBattlePlan".into(),
        0x5D37_0021,
    )));
    team.write().unwrap().set_controlling_player_id(Some(0));
    fixture
        .object
        .write()
        .unwrap()
        .set_team(Some(team))
        .unwrap();
    let base_vision = fixture.object.read().unwrap().get_vision_range();

    let data: Arc<dyn EngineModuleData> = Arc::new(BattlePlanUpdateModuleData {
        special_power_template: Some(1),
        strategy_center_search_and_destroy_sight_range_scalar: 2.0,
        ..Default::default()
    });
    let mut factory = ModuleFactory::new();
    crate::contain_module_overrides::register_module_overrides(&mut factory).unwrap();
    let thing: Arc<dyn ModuleThing> = Arc::new(ObjectThingHandle::new(&fixture.object));
    let module = factory
        .new_module(
            thing,
            "BattlePlanUpdate",
            data.clone(),
            ModuleType::Behavior,
        )
        .unwrap();
    fixture
        .object
        .write()
        .unwrap()
        .install_update_module("BattlePlanUpdate", module, data);
    let battle = fixture
        .object
        .read()
        .unwrap()
        .find_update_module("BattlePlanUpdate")
        .unwrap();
    battle.with_module(|module| module.on_object_created());
    battle
        .with_module_downcast::<BattlePlanUpdateModule, _, _>(|module| {
            module.behavior.desired_plan = BattlePlanStatus::SearchAndDestroy;
        })
        .expect("canonical authored BattlePlan factory");

    // Attach the actual installed proxies in the same module order used by
    // Object::init_modules_for, then register them on this exact owner.
    let (detector, battle_proxy) = {
        let object = fixture.object.read().unwrap();
        let detector: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(
            object.find_module_by_name("StealthDetectorUpdate").unwrap(),
            fixture.id,
        )));
        let battle: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(
            object.find_module_by_name("BattlePlanUpdate").unwrap(),
            fixture.id,
        )));
        (detector, battle)
    };
    {
        let mut object = fixture.object.write().unwrap();
        let detector_entry = object.find_module_by_name("StealthDetectorUpdate").unwrap();
        let battle_entry = object.find_module_by_name("BattlePlanUpdate").unwrap();
        object
            .attach_update_module_registration(detector.clone(), Some(&detector_entry))
            .unwrap();
        object
            .attach_update_module_registration(battle_proxy.clone(), Some(&battle_entry))
            .unwrap();
    }
    // Reproduce the callback's live proxy write guard with a bounded query.
    // The old name scan blocks on this unrelated BattlePlan proxy. Release
    // and join before asserting so RED diagnoses reentry instead of hanging.
    let (sender, receiver) = std::sync::mpsc::channel();
    let object = fixture.object.clone();
    let expected_detector = detector.clone();
    let executing = battle_proxy.write().unwrap();
    let worker = std::thread::spawn(move || {
        let result = object
            .read()
            .unwrap()
            .set_stealth_detector_enabled(false, 0)
            .map(|(wake, modules)| {
                (
                    wake,
                    modules.len(),
                    modules
                        .first()
                        .is_some_and(|module| Arc::ptr_eq(module, &expected_detector)),
                )
            });
        sender.send(result).unwrap();
    });
    let query = receiver.recv_timeout(std::time::Duration::from_secs(2));
    drop(executing);
    worker
        .join()
        .expect("registration lookup completes after proxy guard release");
    assert_eq!(
        query,
        Ok(Some((UpdateSleepTime::Forever.to_u32(), 1, true))),
        "owned registration identity must be readable while another proxy executes"
    );

    fixture
        .owner
        .register_sleepy_update_module(fixture.id, detector.clone(), 0x3fff_ffff);
    fixture
        .owner
        .register_normal_update_module(fixture.id, battle_proxy);

    fixture.owner.update(5).unwrap();
    assert!(!fixture.with_detector(|detector| detector.is_enabled()));
    assert_eq!(
        fixture.owner.sleepy_entry_for(&detector).unwrap().1,
        0x3fff_ffff
    );
    fixture.owner.update(6).unwrap();
    assert!(fixture.with_detector(|detector| detector.is_enabled()));
    assert_eq!(
        fixture.owner.sleepy_entry_for(&detector),
        Some((fixture.id, 7)),
        "the normal callback uses its driving frame"
    );
    assert_eq!(
        fixture.object.read().unwrap().get_vision_range(),
        base_vision * 2.0
    );
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_battle_plan_count(BattlePlanType::SearchAndDestroy),
        1
    );

    fixture.owner.update(7).unwrap();
    assert_eq!(
        fixture.owner.sleepy_entry_for(&detector).unwrap().1,
        17,
        "actual detector scan returns authored DetectionRate"
    );
    battle
        .with_module_downcast::<BattlePlanUpdateModule, _, _>(|module| {
            module.behavior.desired_plan = BattlePlanStatus::None;
        })
        .unwrap();
    fixture.owner.update(8).unwrap();
    assert!(!fixture.with_detector(|detector| detector.is_enabled()));
    assert_eq!(
        fixture.owner.sleepy_entry_for(&detector).unwrap().1,
        0x3fff_ffff
    );
    assert_eq!(fixture.owner.sleepy_update_count(), 1);
    assert_eq!(
        fixture.object.read().unwrap().get_vision_range(),
        base_vision
    );
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_battle_plan_count(BattlePlanType::SearchAndDestroy),
        0
    );
}
