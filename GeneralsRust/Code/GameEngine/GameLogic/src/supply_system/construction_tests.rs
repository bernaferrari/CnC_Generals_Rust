//! C++ WorkerAIUpdate.cpp163-177 shares DozerAIUpdate.cpp540-596 phases.
use super::*;
use crate::object::create::PreorderCreate;
use crate::object::drawable::{Drawable, DrawableExt, DrawableType};
use crate::object::update::ai_update::dozer_ai_update::construction_callback_tests::CompletionFixture;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, Module, ModuleData, ModuleInterfaceType, Thing,
};

#[derive(Debug)]
struct TestThing;
impl Thing for TestThing {}

#[derive(Debug, PartialEq)]
struct BuildCompleteView {
    money_spent: i32,
    buildings_built: i32,
    production: i32,
    build_list_pending: bool,
    name: String,
    health: f32,
    cached_id: Option<u32>,
    supply_plan: Option<(bool, i32, i32)>,
    supply_center_registered: bool,
}

// The observer wraps a real create module, rather than replacing the real
// callback with a direct state assignment. All observations come from the
// live owner and controlling player at the actual onBuildComplete boundary.
struct ObservedCreate<T> {
    create: T,
    observations: Arc<Mutex<Vec<Option<BuildCompleteView>>>>,
    data: Arc<BaseModuleData>,
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> CreateInterface
    for ObservedCreate<T>
{
    fn on_create(&self) {
        self.create.on_create();
    }
    fn on_build_complete(&self) {
        self.create.on_build_complete();
    }
    fn should_do_on_build_complete(&self) -> bool {
        self.create.should_do_on_build_complete()
    }
    fn on_build_complete_with_owner(&self, owner: &mut dyn std::any::Any) {
        self.create.on_build_complete_with_owner(owner);
        let owner = owner.downcast_mut::<crate::object::Object>().unwrap();
        let view = owner.with_controlling_player(|player| BuildCompleteView {
            money_spent: player.get_score_keeper().get_total_money_spent(),
            buildings_built: player.get_score_keeper().get_total_buildings_built(),
            production: player.get_energy().production(),
            build_list_pending: player
                .get_build_list()
                .is_some_and(|info| info.is_under_construction()),
            name: owner.get_name().to_string(),
            health: owner.get_health(),
            cached_id: crate::scripting::engine::get_named_object_tracker()
                .get_object_id(owner.get_name().as_str())
                .unwrap(),
            supply_plan: player.get_build_list().map(|info| {
                (
                    info.is_supply_building(),
                    info.get_desired_gatherers(),
                    info.get_current_gatherers(),
                )
            }),
            supply_center_registered: player
                .get_resource_manager()
                .is_some_and(|manager| manager.get_supply_centers().contains(&owner.get_id())),
        });
        self.observations.lock().unwrap().push(view);
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Module for ObservedCreate<T> {
    fn get_module_name_key(&self) -> u32 {
        crate::common::name_key_generate("ObservedPreorderCreate")
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(self)
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Snapshotable for ObservedCreate<T> {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.create.load_post_process()
    }
}

#[test]
fn worker_create_modules_observe_completed_player_and_ai_phases() {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    for ai in [false, true] {
        for rebuild in [false, true] {
            let fixture = CompletionFixture::new(10);
            fixture.player.write().unwrap().set_is_preorder(true);
            let drawable = Arc::new(RwLock::new(Drawable::new(
                0xBC_0011,
                fixture.structure.read().unwrap().get_id(),
                "ConstructionTest".to_owned(),
                DrawableType::Static,
            )));
            fixture
                .structure
                .write()
                .unwrap()
                .set_drawable(Some(drawable.clone()));
            if ai {
                let mut info = crate::build_list_info::BuildListInfo::new();
                info.set_object_id(fixture.structure.read().unwrap().get_id());
                info.set_building_name("WorkerCompletedAi".into());
                info.set_under_construction(true);
                info.set_health(75);
                fixture.player.write().unwrap().set_build_list(Some(info));
                crate::ai::integration::with_ai_integration_mut(|manager| {
                    manager.ensure_ai_player(0, false)
                })
                .unwrap();
            }
            if ai {
                use crate::object::production::dock_update::{
                    SupplyCenterDockUpdate, SupplyCenterDockUpdateData,
                    SupplyCenterDockUpdateModule,
                };
                let data = Arc::new(SupplyCenterDockUpdateData::default());
                let (id, position) = {
                    let structure = fixture.structure.read().unwrap();
                    (structure.get_id(), *structure.get_position())
                };
                let behavior = SupplyCenterDockUpdate::new((*data).clone(), id, &position);
                let module = SupplyCenterDockUpdateModule::new(
                    behavior,
                    &"SupplyCenterDockUpdate".into(),
                    data.clone(),
                );
                fixture.structure.write().unwrap().install_update_module(
                    "SupplyCenterDockUpdate",
                    Box::new(module),
                    data,
                );
            }
            let observations = Arc::new(Mutex::new(Vec::new()));
            let data = Arc::new(BaseModuleData::new());
            fixture.structure.write().unwrap().install_module_for_test(
                "ObservedPreorderCreate",
                Box::new(ObservedCreate {
                    create: PreorderCreate::new(Arc::new(TestThing)),
                    observations: observations.clone(),
                    data: data.clone(),
                }),
                data,
                ModuleInterfaceType::CREATE,
            );
            let mut worker = WorkerAIUpdate::new(
                WorkerAIUpdateData::default(),
                fixture.builder.read().unwrap().get_id(),
                0,
            );
            let production_before = fixture.player.read().unwrap().get_energy().production();
            worker.handle_build_completion(&fixture.builder, &fixture.structure, rebuild);
            assert_eq!(
                *observations.lock().unwrap(),
                vec![Some(BuildCompleteView {
                    money_spent: if rebuild { 0 } else { 700 },
                    buildings_built: if rebuild { 0 } else { 1 },
                    production: production_before + 10,
                    build_list_pending: false,
                    name: if ai {
                        "WorkerCompletedAi"
                    } else {
                        "CompletionScriptName"
                    }
                    .to_owned(),
                    health: if ai { 75.0 } else { 100.0 },
                    // C++ AI explicitly adds the completed build-list object
                    // to cache; human notification only records the frame.
                    cached_id: ai.then(|| fixture.structure.read().unwrap().get_id()),
                    supply_plan: ai.then_some((true, 1, -1)),
                    supply_center_registered: false,
                })]
            );
            assert!(
                drawable
                    .read()
                    .unwrap()
                    .get_model_conditions()
                    .contains(ModelConditionFlags::PREORDER)
            );
        }
    }
}

fn assert_unowned_completion_skips_create(worker: bool) {
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    {
        let fixture = CompletionFixture::new(0);
        fixture.structure.write().unwrap().set_team(None);
        let observations = Arc::new(Mutex::new(Vec::new()));
        let data = Arc::new(BaseModuleData::new());
        fixture.structure.write().unwrap().install_module_for_test(
            "ObservedPreorderCreate",
            Box::new(ObservedCreate {
                create: PreorderCreate::new(Arc::new(TestThing)),
                observations: observations.clone(),
                data: data.clone(),
            }),
            data,
            ModuleInterfaceType::CREATE,
        );
        if worker {
            let mut worker = WorkerAIUpdate::new(
                WorkerAIUpdateData::default(),
                fixture.builder.read().unwrap().get_id(),
                0,
            );
            worker.handle_build_completion(&fixture.builder, &fixture.structure, false);
        } else {
            fixture.complete(false);
        }
        assert!(
            observations.lock().unwrap().is_empty(),
            "C++ only invokes onBuildComplete when Player exists"
        );
        assert!(!fixture.structure.read().unwrap().is_under_construction());
    }
}

#[test]
fn worker_skips_create_hooks_without_a_controlling_player() {
    assert_unowned_completion_skips_create(true);
}
#[test]
fn dozer_skips_create_hooks_without_a_controlling_player() {
    assert_unowned_completion_skips_create(false);
}

#[test]
fn real_supply_center_create_completes_without_relocking_its_object() {
    use crate::object::create::SupplyCenterCreate;
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.player.write().unwrap().init_from_dict_defaults();
    let observations = Arc::new(Mutex::new(Vec::new()));
    let data = Arc::new(BaseModuleData::new());
    let thing = Arc::new(crate::object::ObjectThingHandle::new(&fixture.structure));
    fixture.structure.write().unwrap().install_module_for_test(
        "SupplyCenterCreate",
        Box::new(ObservedCreate {
            create: SupplyCenterCreate::new(thing),
            observations: observations.clone(),
            data: data.clone(),
        }),
        data,
        ModuleInterfaceType::CREATE,
    );
    let mut worker = WorkerAIUpdate::new(
        WorkerAIUpdateData::default(),
        fixture.builder.read().unwrap().get_id(),
        0,
    );
    worker.handle_build_completion(&fixture.builder, &fixture.structure, false);
    assert_eq!(
        *observations.lock().unwrap(),
        vec![Some(BuildCompleteView {
            money_spent: 700,
            buildings_built: 1,
            production: 0,
            build_list_pending: false,
            name: "CompletionScriptName".into(),
            health: 100.0,
            cached_id: None,
            supply_plan: None,
            supply_center_registered: true,
        })]
    );
}

#[test]
fn supply_center_completion_flag_survives_xfer() {
    use crate::object::create::SupplyCenterCreate;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let fixture = CompletionFixture::new(0);
    fixture.player.write().unwrap().init_from_dict_defaults();
    let thing = Arc::new(crate::object::ObjectThingHandle::new(&fixture.structure));
    let mut create = SupplyCenterCreate::new(thing.clone());
    assert!(create.should_do_on_build_complete());
    create.on_build_complete_with_owner(&mut *fixture.structure.write().unwrap());
    assert!(!create.should_do_on_build_complete());
    let id = fixture.structure.read().unwrap().get_id();
    assert_eq!(
        fixture
            .player
            .read()
            .unwrap()
            .get_resource_manager()
            .unwrap()
            .get_supply_centers(),
        &[id]
    );
    let mut bytes = Vec::new();
    create
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    let mut restored = SupplyCenterCreate::new(thing);
    restored
        .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    restored.load_post_process().unwrap();
    assert!(!restored.should_do_on_build_complete());
    // An already completed module must not re-register a subsequently removed
    // center, including after restoring its one-time state.
    fixture
        .player
        .write()
        .unwrap()
        .get_resource_manager_mut()
        .unwrap()
        .remove_supply_center(id);
    restored.on_build_complete_with_owner(&mut *fixture.structure.write().unwrap());
    assert!(
        fixture
            .player
            .read()
            .unwrap()
            .get_resource_manager()
            .unwrap()
            .get_supply_centers()
            .is_empty()
    );
}

#[test]
fn worker_runs_real_player_upgrade_grant_after_notification() {
    use crate::object::create::{GrantUpgradeCreate, GrantUpgradeCreateModuleData};
    let _isolation = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap();
    let stores = crate::system::engine_stores::new_for_world();
    // These test threads have distinct name-key registries. Create definitions
    // on this thread instead of copying keys initialized by an earlier test.
    *stores.upgrade_center().write().unwrap() = crate::upgrade::center::UpgradeCenter::new();
    crate::system::engine_stores::with_active_stores(&stores, || {
        let fixture = CompletionFixture::new(0);
        let name = "Upgrade_WorkerCompletionTest";
        let upgrade = crate::upgrade::center::with_upgrade_center_mut(|center| {
            center.new_upgrade(name.into())
        });
        assert_eq!(
            upgrade.get_name().as_str(),
            name,
            "fixture resolved an unrelated engine template"
        );
        assert!(
            !fixture
                .player
                .read()
                .unwrap()
                .has_upgrade_complete(&upgrade)
        );
        let observations = Arc::new(Mutex::new(Vec::new()));
        let data = Arc::new(BaseModuleData::new());
        let thing = Arc::new(crate::object::ObjectThingHandle::new(&fixture.structure));
        fixture.structure.write().unwrap().install_module_for_test(
            "GrantUpgradeCreate",
            Box::new(ObservedCreate {
                create: GrantUpgradeCreate::new(
                    thing,
                    Arc::new(GrantUpgradeCreateModuleData {
                        upgrade_name: name.into(),
                        ..Default::default()
                    }),
                ),
                observations: observations.clone(),
                data: data.clone(),
            }),
            data,
            ModuleInterfaceType::CREATE,
        );
        let mut worker = WorkerAIUpdate::new(
            WorkerAIUpdateData::default(),
            fixture.builder.read().unwrap().get_id(),
            0,
        );
        worker.handle_build_completion(&fixture.builder, &fixture.structure, false);
        assert!(
            fixture
                .player
                .read()
                .unwrap()
                .has_upgrade_complete(&upgrade),
            "template_mask={:?}, completed_mask={:?}, views={:?}",
            upgrade.get_mask(),
            fixture.player.read().unwrap().get_completed_upgrade_mask(),
            observations.lock().unwrap()
        );
        let views = observations.lock().unwrap();
        let view = views[0].as_ref().unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(view.money_spent, 700);
        assert_eq!(view.buildings_built, 1);
        assert_eq!(view.cached_id, None);
        drop(views);
        // Object dispatch itself respects the module's completed flag.
        fixture.structure.write().unwrap().on_build_complete();
        assert_eq!(observations.lock().unwrap().len(), 1);
    });
}
