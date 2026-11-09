//! ModuleFactory.cpp:610-665 returns the actual constructed module. A failed
//! Rust constructor must not become a successful empty replacement behavior.
use super::*;
use crate::common::ThingTemplate;
use crate::object::behavior::flammable_update::{FlammableUpdate, FlammableUpdateModuleData};
use game_engine::common::thing::module::{BaseModuleData, ModuleInterfaceType};
use game_engine::common::thing::module_factory::ModuleFactory;

const FAILURE: &str = "authored constructor initialization control";
const NAME: &str = "InitializationFailureControl";
fn rejecting_constructor(
    _owner: Arc<RwLock<crate::object::Object>>,
    _data: Arc<dyn LegacyModuleData>,
) -> Result<FlammableUpdate, Box<dyn std::error::Error + Send + Sync>> {
    Err(FAILURE.into())
}
fn rejecting_factory(thing: Arc<dyn ModuleThing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    active_behavior_module::<FlammableUpdate, FlammableUpdateModuleData>(
        thing,
        data,
        NAME,
        rejecting_constructor,
    )
}
fn catalog() -> ModuleFactory {
    let mut factory = ModuleFactory::new();
    factory.add_module_internal(
        Some(rejecting_factory),
        None,
        ModuleType::Behavior,
        NAME,
        ModuleInterfaceType::UPDATE,
    );
    factory
}
fn supplied_owner(id: u32) -> (Arc<RwLock<crate::object::Object>>, Arc<dyn ModuleThing>) {
    let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
    let thing: Arc<dyn ModuleThing> = Arc::new(crate::object::ObjectThingHandle::new(&owner));
    (owner, thing)
}

#[test]
fn actual_factory_preserves_constructor_error_without_installing_replacement() {
    let (owner, thing) = supplied_owner(0xFACA_0001);
    let data: Arc<dyn ModuleData> = Arc::new(FlammableUpdateModuleData::default());
    let before = owner.read().unwrap().behavior_modules().len();
    let error = catalog()
        .new_module(thing, NAME, data, ModuleType::Behavior)
        .err()
        .expect("actual constructor failure must fail factory construction");
    assert!(error.contains(FAILURE), "{error}");
    assert!(error.contains(NAME), "{error}");
    assert_eq!(owner.read().unwrap().behavior_modules().len(), before);
}

#[test]
fn actual_factory_rejects_wrong_data_without_panicking_or_constructing() {
    let (owner, thing) = supplied_owner(0xFACA_0002);
    let data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
    let before = owner.read().unwrap().behavior_modules().len();
    let error = catalog()
        .new_module(thing, NAME, data, ModuleType::Behavior)
        .err()
        .expect("wrong data must fail the real factory");
    assert!(error.contains("expected module data"), "{error}");
    assert!(
        !error.contains(FAILURE),
        "constructor cannot run before data validation"
    );
    assert_eq!(owner.read().unwrap().behavior_modules().len(), before);
}

#[test]
fn cave_factory_uses_the_supplied_owner_when_a_foreign_world_is_active() {
    use crate::contain_module_overrides::contain::{
        ContainModuleDataAdapter, cave_contain_module_factory,
    };
    use crate::object::contain::CaveContainModuleData;
    use crate::system::engine_stores::{EngineStores, with_active_stores};

    let receiving = Arc::new(EngineStores::new_for_world());
    let foreign = Arc::new(EngineStores::new_for_world());
    let receiving_cave = Arc::clone(&receiving.services().logic_managers().cave);
    let foreign_cave = Arc::clone(&foreign.services().logic_managers().cave);
    with_active_stores(&foreign, || {
        let owner = Arc::new(RwLock::new(crate::object::Object::new_test(0xFACA_0003, 100.0)));
        let thing: Arc<dyn ModuleThing> = Arc::new(
            crate::object::ObjectThingHandle::with_cave_system(&owner, Some(Arc::clone(&receiving_cave))),
        );
        let mut data = CaveContainModuleData::default();
        data.cave_index_data = 4;
        let data: Arc<dyn ModuleData> = Arc::new(ContainModuleDataAdapter::new(data));
        let mut module = cave_contain_module_factory(thing, data);
        module.on_object_created();
        assert_eq!(receiving_cave.lock().unwrap().registered_indices(), vec![4]);
        assert!(foreign_cave.lock().unwrap().registered_indices().is_empty());

        // Exercise the same owner propagation through actual native object
        // assembly, not only through the module constructor.
        let cave_mask = ModuleInterfaceType(ModuleInterfaceType::CONTAIN.0 | ModuleInterfaceType::CREATE.0);
        let mut catalog = ModuleFactory::new();
        catalog.add_module_internal(
            Some(cave_contain_module_factory), None, ModuleType::Behavior,
            "CaveContain", cave_mask,
        );
        let mut native_data = CaveContainModuleData::default();
        native_data.cave_index_data = 5;
        let template: Arc<dyn ThingTemplate> = Arc::new(FailingTemplate {
            base: crate::common::DefaultThingTemplate::new("OwnedCaveFactory".into()),
            modules: vec![crate::common::TemplateModuleInfo {
                name: "CaveContain".into(),
                module_tag: "OwnedCave".into(),
                data: Arc::new(ContainModuleDataAdapter::new(native_data)),
                interface_mask: cave_mask,
            }],
        });
        let bodies = Arc::new(crate::object::body::active_body::BodyDefinitionCatalog::new(
            Some(&game_engine::common::global_data::GlobalData::default()),
            std::collections::HashMap::new(), std::collections::HashMap::new(),
        ).unwrap());
        let mut scheduler = crate::system::game_logic::GameLogic::new_with_engine_stores(Arc::clone(&receiving));
        let mut objects = crate::object::object_factory::ObjectFactory::new();
        let id = objects.prepare_object_owned(
            &mut scheduler, template, Coord3D::ZERO, None,
            crate::object::object_factory::ObjectCreationFlags::NO_AI,
            &mut |_name: &str| Ok(None), bodies, &catalog,
        ).expect("owned cave assembly");
        let object = objects.get_object(id).unwrap().get_base_object().unwrap();
        object.write().unwrap().on_build_complete();
        let tracker = receiving_cave.lock().unwrap().get_tunnel_tracker_for_cave_index(5).unwrap();
        assert_eq!(tracker.read().unwrap().get_tunnel_count(), 1);
        assert_eq!(receiving_cave.lock().unwrap().registered_indices(), vec![4, 5]);
        assert!(foreign_cave.lock().unwrap().registered_indices().is_empty());
    });
}

#[test]
fn cave_index_script_uses_the_interpreters_world_with_foreign_world_active() {
    use crate::object::contain::{CaveContain, CaveContainModuleData};
    use crate::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
    use crate::scripting::engine::ScriptEngine;
    use crate::scripting::executor::{ScriptActionDispatcher, ScriptContext};
    use crate::system::engine_stores::{EngineStores, with_active_stores};
    use std::cell::RefCell;

    let receiving = Arc::new(EngineStores::new_for_world());
    let foreign = Arc::new(EngineStores::new_for_world());
    let id = 0xFACA_0004;
    let mut owners = Vec::new();
    for world in [&receiving, &foreign] {
        let owner = Arc::new(RwLock::new(crate::object::Object::new_test(id, 100.0)));
        let caves = Arc::clone(&world.services().logic_managers().cave);
        let mut cave = CaveContain::new(Arc::downgrade(&owner), &CaveContainModuleData::default(), Some(caves)).unwrap();
        cave.on_owner_created().unwrap();
        owner.write().unwrap().set_contain(Some(Arc::new(Mutex::new(cave))));
        world.services().object_registry().register_object(id, &owner);
        world.services().scripts().named_objects().register_named_object("Cave".into(), id).unwrap();
        owners.push(owner);
    }
    let receiving_caves = &receiving.services().logic_managers().cave;
    let foreign_caves = &foreign.services().logic_managers().cave;
    let old = receiving_caves.lock().unwrap().get_tunnel_tracker_for_cave_index(0).unwrap();
    old.write().unwrap().add_to_contain_list(123).unwrap();
    let engine = ScriptEngine::new_with_services(receiving.services()).unwrap();
    let state = RefCell::new(ScriptContext::new());
    let mut action = ScriptAction::new(ScriptActionType::SetCaveIndex);
    action.add_parameter(Parameter::with_string(ParameterType::Unit, "Cave".into())).unwrap();
    action.add_parameter(Parameter::with_int(ParameterType::Int, 2)).unwrap();
    with_active_stores(&foreign, || {
        let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);
        dispatcher.execute_action(&action).unwrap();
        assert_eq!(receiving_caves.lock().unwrap().registered_indices(), vec![0]);
        old.write().unwrap().remove_from_contain(123, false).unwrap();
        dispatcher.execute_action(&action).unwrap();
        assert_eq!(receiving_caves.lock().unwrap().registered_indices(), vec![0, 2]);
        assert_eq!(old.read().unwrap().get_tunnel_count(), 0);
        let moved = receiving_caves.lock().unwrap().get_tunnel_tracker_for_cave_index(2).unwrap();
        assert_eq!(moved.read().unwrap().get_tunnel_count(), 1);
        let foreign_old = foreign_caves.lock().unwrap().get_tunnel_tracker_for_cave_index(0).unwrap();
        assert_eq!(foreign_old.read().unwrap().get_tunnel_count(), 1);
        assert_eq!(foreign_caves.lock().unwrap().registered_indices(), vec![0]);
    });
    drop(owners);
}

struct FailingTemplate {
    base: crate::common::DefaultThingTemplate,
    modules: Vec<crate::common::TemplateModuleInfo>,
}
impl std::fmt::Debug for FailingTemplate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FailingTemplate")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
impl crate::common::ThingTemplate for FailingTemplate {
    fn get_name(&self) -> &AsciiString {
        self.base.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.base.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: crate::common::KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_behavior_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.modules
    }
}

#[test]
fn failed_native_factory_assembly_does_not_admit_object_or_schedule() {
    use crate::common::ThingTemplate;
    use crate::object::body::active_body::BodyDefinitionCatalog;
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::collections::HashMap;
    let template: Arc<dyn ThingTemplate> = Arc::new(FailingTemplate {
        base: crate::common::DefaultThingTemplate::new("FailedFactoryOwner".into()),
        modules: vec![crate::common::TemplateModuleInfo {
            name: NAME.into(),
            module_tag: "InvalidConstructor".into(),
            data: Arc::new(FlammableUpdateModuleData::default()),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    });
    let bodies = Arc::new(
        BodyDefinitionCatalog::new(
            Some(&game_engine::common::global_data::GlobalData::default()),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap(),
    );
    let mut factory = ObjectFactory::new();
    let mut scheduler = crate::system::game_logic::GameLogic::new();
    let mut lookup = |_name: &str| Ok(None);
    let error = factory
        .prepare_object_owned(
            &mut scheduler,
            template,
            Coord3D::ZERO,
            None,
            ObjectCreationFlags::NO_AI,
            &mut lookup,
            bodies,
            &catalog(),
        )
        .err()
        .expect("failed module must abort unpublished object assembly");
    assert!(error.to_string().contains(FAILURE), "{error}");
    assert_eq!(factory.get_statistics().total_objects, 0);
    assert_eq!(factory.get_statistics().total_created, 0);
    assert!(factory.get_object(1).is_none());
    assert!(scheduler.get_all_object_ids().is_empty());
    assert_eq!(scheduler.iter_all_objects().count(), 0);
    assert_eq!(scheduler.sleepy_update_count(), 0);
}
