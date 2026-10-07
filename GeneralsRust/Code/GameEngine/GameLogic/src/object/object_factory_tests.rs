use super::*;
use crate::object::body::active_body::ActiveBodyModuleData;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::BaseModuleData;
use game_engine::common::thing::module_factory::register_module_override;
use game_engine::system::xfer_load::XferLoad;
use game_engine::system::xfer_save::XferSave;
use std::any::Any;
use std::io::Cursor;

const PROBE_NAME: &str = "FactoryCreationBorrowProbe";

/// Assert the C++ callback boundary before the body callback can reacquire
/// its owner's write lock. A failure is immediate rather than a hung test.
struct CreationProbe {
    owner_id: ObjectID,
    data: Arc<dyn ModuleData>,
    created: bool,
    delete_calls: u32,
    delete_status: Vec<bool>,
}

impl Module for CreationProbe {
    fn get_module_name_key(&self) -> u32 {
        NameKeyGenerator::name_to_key(PROBE_NAME)
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        assert!(!self.created, "onObjectCreated must run once per module");
        let owner = OBJECT_REGISTRY
            .get_object(self.owner_id)
            .expect("published owner");
        let owner = owner
            .try_write()
            .expect("creation callback must borrow its owner");
        assert!(!owner.modules_ready, "callbacks precede modulesReady");
        assert_eq!(
            owner.modules.len(),
            2,
            "all authored modules must be installed"
        );
        assert!(owner.has_ctor_helpers(), "helpers precede callbacks");
        self.created = true;
    }

    fn on_delete(&mut self) {
        let owner = OBJECT_REGISTRY
            .get_object(self.owner_id)
            .expect("published owner");
        let mut owner = owner
            .try_write()
            .expect("detached callback must release the owner borrow");
        self.delete_status.push(owner.is_destroyed());
        self.delete_calls += 1;
        owner.construction_percent = self.delete_calls as f32;
    }
}

impl Snapshotable for CreationProbe {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn probe_factory(thing: Arc<dyn ModuleThing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    Box::new(CreationProbe {
        owner_id: thing.as_object().expect("object module").get_object_id(),
        data,
        created: false,
        delete_calls: 0,
        delete_status: Vec::new(),
    })
}

fn probe_data_factory(_: Option<&mut game_engine::common::ini::INI>) -> Box<dyn ModuleData> {
    Box::new(BaseModuleData::new())
}

struct AuthoredBodyTemplate {
    inner: DefaultThingTemplate,
    modules: Vec<crate::common::TemplateModuleInfo>,
}

impl std::fmt::Debug for AuthoredBodyTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthoredBodyTemplate")
            .field("name", &self.inner.get_name())
            .finish()
    }
}

impl AuthoredBodyTemplate {
    fn new(name: &str, with_probe: bool) -> Self {
        let mut modules = Vec::new();
        if with_probe {
            let mut data = BaseModuleData::new();
            data.set_module_tag_name_key(NameKeyGenerator::name_to_key("ModuleTag_CreationProbe"));
            modules.push(crate::common::TemplateModuleInfo {
                name: PROBE_NAME.into(),
                module_tag: "ModuleTag_CreationProbe".into(),
                data: Arc::new(data),
                interface_mask: ModuleInterfaceType::NONE,
            });
        }
        let mut data = ActiveBodyModuleData::default();
        data.max_health = 100.0;
        data.initial_health = 100.0;
        ModuleData::set_module_tag_name_key(
            &mut data,
            NameKeyGenerator::name_to_key("ModuleTag_AuthoredBody"),
        );
        modules.push(crate::common::TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "ModuleTag_AuthoredBody".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::BODY,
        });
        Self {
            inner: DefaultThingTemplate::new(name.into()),
            modules,
        }
    }
}

impl ThingTemplate for AuthoredBodyTemplate {
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.inner.is_kind_of(kind)
    }
    fn get_name(&self) -> &AsciiString {
        self.inner.get_name()
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.inner.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> Real {
        self.inner.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> Real {
        self.inner.calc_shroud_clearing_range()
    }
    fn get_behavior_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.modules
    }
}

fn register_creation_probe() {
    crate::contain_module_overrides::register_active_body_override_for_test()
        .expect("real ActiveBody descriptor");
    register_module_override(
        PROBE_NAME,
        ModuleType::Behavior,
        probe_factory,
        probe_data_factory,
    )
    .expect("probe descriptor");
    init_module_factory().expect("module factory");
    get_module_factory()
        .unwrap()
        .as_mut()
        .unwrap()
        .add_module_internal(
            Some(probe_factory),
            Some(probe_data_factory),
            ModuleType::Behavior,
            &AsciiString::from(PROBE_NAME),
            ModuleInterfaceType::NONE,
        );
}

#[test]
fn factory_authored_body_creates_once_and_restores_live_health() {
    let _lock = crate::test_sync::lock();
    register_creation_probe();
    let mut factory = ObjectFactory::new();
    // Avoid retail database startup while still taking the complete production
    // factory -> constructor -> module override -> Object Xfer route.
    factory.next_object_id = 91_301;
    factory.template_cache.insert(
        "AuthoredBodyProbe".into(),
        Arc::new(AuthoredBodyTemplate::new("AuthoredBodyProbe", true)),
    );
    let saved_id = factory
        .create_object(
            "AuthoredBodyProbe",
            Coord3D::new(20.0, 3.0, 45.0),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .expect("factory object");
    let saved = factory
        .get_object(saved_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let mut bytes = Vec::new();
    {
        let mut object = saved.write().unwrap();
        assert_eq!(object.modules.len(), 2);
        assert_eq!(
            object.body_module_handles.len(),
            1,
            "one authored body interface"
        );
        object.set_health(55.0).expect("damage");
        object
            .status_damage_helper()
            .unwrap()
            .set_frame_to_heal_for_test(77);
        object
            .status_damage_helper()
            .unwrap()
            .set_status_to_heal_for_test(ObjectStatusTypes::Stealthed);
        object.xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1));
    }
    assert_eq!(bytes[0], 9, "Object Xfer framing stays version9");
    let loaded_id = factory
        .create_object(
            "AuthoredBodyProbe",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .expect("restore object");
    let loaded = factory
        .get_object(loaded_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    {
        let mut object = loaded.write().unwrap();
        object.xfer(&mut XferLoad::new(Cursor::new(&bytes), 1));
        object.load_post_process();
        let drawable = object.get_drawable().expect("created drawable");
        let drawable = drawable.read().unwrap();
        assert_eq!(drawable.get_object_id(), object.get_id());
        assert_eq!(
            drawable.get_transform_matrix(),
            object.get_transform_matrix()
        );
        drop(drawable);
        assert_eq!(object.get_health(), 55.0, "Xfer must restore the live body");
        assert_eq!(
            object.status_damage_helper().unwrap().get_frame_to_heal(),
            77
        );
        assert_eq!(
            object.status_damage_helper().unwrap().get_status_to_heal(),
            ObjectStatusTypes::Stealthed
        );
        object.set_health(40.0).expect("damage after restore");
        assert_eq!(object.get_health(), 40.0);
    }
    // Registry identities were allocated before load; Object Xfer restores the
    // saved id in the payload. Remove both original registrations explicitly.
    retire_factory_fixture_objects(&[saved_id, loaded_id]);
}

#[test]
fn factory_behavior_views_keep_identity_and_release_owner_before_callbacks() {
    let _lock = crate::test_sync::lock();
    register_creation_probe();
    let mut factory = ObjectFactory::new();
    factory.next_object_id = 91_411;
    let mut template = AuthoredBodyTemplate::new("DetachedBehaviorProbe", true);
    template.modules[0].interface_mask = ModuleInterfaceType::DESTROY;
    factory
        .template_cache
        .insert("DetachedBehaviorProbe".into(), Arc::new(template));
    let first_id = factory
        .create_object(
            "DetachedBehaviorProbe",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .expect("first factory object");
    let second_id = factory
        .create_object(
            "DetachedBehaviorProbe",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .expect("second factory object");
    let first = factory
        .get_object(first_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let second = factory
        .get_object(second_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let (mut detached, entry, names) = {
        let object = first.read().unwrap();
        let detached = object
            .find_update_behavior(PROBE_NAME)
            .expect("probe behavior");
        let names: Vec<String> = object
            .get_behavior_modules()
            .into_iter()
            .map(|mut view| view.access().unwrap().get_module_name().to_string())
            .collect();
        (detached, object.modules[0].clone(), names)
    };
    assert_eq!(
        &names[..3],
        [
            "ObjectSMCHelper",
            "StatusDamageHelper",
            "SubdualDamageHelper"
        ]
    );
    assert_eq!(&names[names.len() - 2..], [PROBE_NAME, "ActiveBody"]);
    assert!(Arc::ptr_eq(
        detached.template_entry_for_test().unwrap(),
        &entry
    ));
    let mut cloned = detached.clone();
    assert!(Arc::ptr_eq(
        cloned.template_entry_for_test().unwrap(),
        &entry
    ));
    {
        let mut lease = detached.access().unwrap();
        assert!(
            lease.get_damage().is_none(),
            "mask controls damage visibility"
        );
        assert!(lease.get_production_update_interface().is_none());
        assert!(
            lease.get_body().is_none(),
            "view must not expose extra interfaces"
        );
        assert!(
            !lease.as_any().is::<CreationProbe>(),
            "view does not downcast to the module"
        );
        lease.get_destroy().unwrap().on_destroy(first_id);
    }
    assert_eq!(first.read().unwrap().construction_percent, 1.0);
    assert_eq!(
        second.read().unwrap().construction_percent,
        CONSTRUCTION_COMPLETE
    );
    cloned
        .access()
        .unwrap()
        .get_destroy()
        .unwrap()
        .on_destroy(first_id);
    assert_eq!(first.read().unwrap().construction_percent, 2.0);
    entry.with_module(|module| {
        let probe = (module as &mut dyn Any)
            .downcast_mut::<CreationProbe>()
            .unwrap();
        assert!(probe.created);
        assert_eq!(
            probe.delete_calls, 2,
            "both views mutate the same factory module"
        );
    });
    // Removing the owner's list does not retarget detached views to another
    // object or copy module state. The retained Entry is the same authority.
    first.write().unwrap().modules.clear();
    first.write().unwrap().behaviors.clear();
    cloned
        .access()
        .unwrap()
        .get_destroy()
        .unwrap()
        .on_destroy(first_id);
    assert_eq!(first.read().unwrap().construction_percent, 3.0);
    assert_eq!(
        second.read().unwrap().construction_percent,
        CONSTRUCTION_COMPLETE
    );
    let second_entry = second.read().unwrap().modules[0].clone();
    retire_factory_fixture_objects(&[first_id, second_id]);
    second_entry.with_module(|module| {
        let probe = (module as &mut dyn Any)
            .downcast_mut::<CreationProbe>()
            .unwrap();
        // The existing template adapter forwards both callbacks to this test
        // module. Verify the two C++ boundaries without claiming real Destroy
        // family dispatch (tracked separately in hq-owgwt).
        assert_eq!(probe.delete_status, [false, true]);
    });
}

fn retire_factory_fixture_objects(admitted_ids: &[ObjectID]) {
    let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
    for &id in admitted_ids {
        logic.destroy_object(id);
    }
    logic
        .cleanup_dead_objects()
        .expect("canonical fixture retirement");
    for &id in admitted_ids {
        assert!(
            logic.find_object_by_id(id).is_none(),
            "owner admission retired"
        );
        assert!(
            OBJECT_REGISTRY.get_object(id).is_none(),
            "lookup retired after callbacks"
        );
    }
}

#[test]
fn factory_fixture_retirement_leaves_no_admitted_objects_for_later_reset() {
    factory_authored_body_creates_once_and_restores_live_health();
    factory_behavior_views_keep_identity_and_release_owner_before_callbacks();
    let still_admitted = {
        let logic = crate::system::game_logic::get_game_logic().lock().unwrap();
        [91_301, 91_302, 91_411, 91_412]
            .into_iter()
            .filter(|id| logic.find_object_by_id(*id).is_some())
            .collect::<Vec<_>>()
    };
    assert!(
        still_admitted.is_empty(),
        "fixture objects remain admitted after their lookup keys were removed: {still_admitted:?}"
    );
    crate::system::game_logic::get_game_logic()
        .lock()
        .unwrap()
        .clear_all_objects();
}

#[test]
fn factory_destruction_preserves_canonical_callback_and_retirement_order() {
    let _lock = crate::test_sync::lock();
    register_creation_probe();
    let mut factory = ObjectFactory::new();
    factory.next_object_id = 91_501;
    let mut template = AuthoredBodyTemplate::new("FactoryDestroyProbe", true);
    template.modules[0].interface_mask = ModuleInterfaceType::DESTROY;
    factory
        .template_cache
        .insert("FactoryDestroyProbe".into(), Arc::new(template));
    let id = factory
        .create_object(
            "FactoryDestroyProbe",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let retained = factory.get_object(id).unwrap().get_base_object().unwrap();
    let entry = retained.read().unwrap().modules[0].clone();
    let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
    factory.destroy_object(&mut logic, id);
    entry.with_module(|module| {
        let probe = module.as_any_mut().downcast_mut::<CreationProbe>().unwrap();
        // Existing Destroy adapter dispatch is tracked separately in hq-owgwt.
        assert_eq!(probe.delete_status, [false, true]);
    });
    assert!(retained.read().unwrap().is_destroyed());
    assert!(
        OBJECT_REGISTRY.get_object(id).is_some(),
        "onDelete precedes lookup retirement"
    );
    assert!(logic.find_object_by_id(id).is_some());
    factory.destroy_object(&mut logic, id);
    assert_eq!(
        factory.destruction_queue,
        [id],
        "duplicate requests do not duplicate bookkeeping"
    );
    factory.process_destruction_queue(&logic);
    assert!(
        factory.get_object(id).is_some(),
        "pending canonical object retains its factory wrapper"
    );
    assert_eq!(factory.get_statistics().total_destroyed, 0);
    logic.cleanup_dead_objects().unwrap();
    assert!(logic.find_object_by_id(id).is_none());
    assert!(OBJECT_REGISTRY.get_object(id).is_none());
    factory.process_destruction_queue(&logic);
    assert!(factory.get_object(id).is_none());
    assert_eq!(factory.get_statistics().total_destroyed, 1);
}

#[test]
fn projectile_bookkeeping_survives_canonical_retirement_without_object_borrows() {
    let _lock = crate::test_sync::lock();
    let id = 91_551;
    let retained = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let mut logic = crate::system::game_logic::GameLogic::new();
    logic.register_object(Arc::clone(&retained)).unwrap();
    let mut factory = ObjectFactory::new();
    factory
        .object_registry
        .insert(id, GameObjectInstance::Projectile(id));
    factory.update_pool_stats(&ObjectType::Projectile);
    factory.total_objects_created = 1;
    {
        let _exclusive_owner = retained.write().unwrap();
        let stats = factory.get_statistics();
        assert_eq!(stats.projectiles, 1);
        assert_eq!(stats.pool_stats["Projectile"].in_use, 1);
        assert_eq!(factory.get_all_projectiles(), [id]);
    }
    factory.destroy_object(&mut logic, id);
    factory.process_destruction_queue(&logic);
    assert_eq!(factory.get_statistics().pool_stats["Projectile"].in_use, 1);
    logic.cleanup_dead_objects().unwrap();
    factory.process_destruction_queue(&logic);
    let stats = factory.get_statistics();
    assert_eq!(stats.total_destroyed, 1);
    assert_eq!(stats.projectiles, 0);
    assert_eq!(stats.pool_stats["Projectile"].in_use, 0);
    assert_eq!(stats.pool_stats["Projectile"].allocated, 1);
    assert!(!stats.pool_stats.contains_key("BaseObject"));
}

#[test]
fn factory_reset_retires_owned_objects_and_preserves_a_live_id_namespace() {
    let _lock = crate::test_sync::lock();
    register_creation_probe();
    let mut factory = ObjectFactory::new();
    factory.next_object_id = 91_601;
    let mut template = AuthoredBodyTemplate::new("FactoryResetProbe", true);
    template.modules[0].interface_mask = ModuleInterfaceType::DESTROY;
    factory
        .template_cache
        .insert("FactoryResetProbe".into(), Arc::new(template));
    let id = factory
        .create_object(
            "FactoryResetProbe",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let retained = factory.get_object(id).unwrap().get_base_object().unwrap();
    let entry = retained.read().unwrap().modules[0].clone();
    let next_id = factory.next_object_id;
    let unrelated_id = 91_610;
    let unrelated = Arc::new(RwLock::new(Object::new_test(unrelated_id, 100.0)));
    let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
    logic.register_object(Arc::clone(&unrelated)).unwrap();
    factory.clear_all_objects(&mut logic).unwrap();
    assert!(factory.get_object(id).is_none());
    assert!(logic.find_object_by_id(id).is_none());
    assert!(logic.find_object_by_id(unrelated_id).is_some());
    assert!(!unrelated.read().unwrap().is_destroyed());
    assert_eq!(
        factory.next_object_id, next_id,
        "factory reset must not reuse a live world namespace"
    );
    entry.with_module(|module| {
        let probe = module.as_any_mut().downcast_mut::<CreationProbe>().unwrap();
        assert_eq!(probe.delete_status, [false, true]);
    });
    logic.destroy_object(unrelated_id);
    logic.cleanup_dead_objects().unwrap();
}

#[test]
fn factory_retirement_queries_the_driving_world_with_identical_ids() {
    let _lock = crate::test_sync::lock();
    let id = 91_650;
    let first = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let second = Arc::new(RwLock::new(Object::new_test(id, 200.0)));
    let mut first_world = crate::system::game_logic::GameLogic::new();
    let mut second_world = crate::system::game_logic::GameLogic::new();
    first_world.register_object(Arc::clone(&first)).unwrap();
    second_world.register_object(Arc::clone(&second)).unwrap();
    let mut factory = ObjectFactory::new();
    factory
        .object_registry
        .insert(id, GameObjectInstance::BaseObject(id));
    factory.destroy_object(&mut first_world, id);
    assert!(first.read().unwrap().is_destroyed());
    assert!(!second.read().unwrap().is_destroyed());
    first_world.cleanup_dead_objects().unwrap();
    factory.process_destruction_queue(&second_world);
    assert!(
        factory.get_object(id).is_some(),
        "other driving world's admission retains bookkeeping"
    );
    factory.process_destruction_queue(&first_world);
    assert!(factory.get_object(id).is_none());
    // Global lookup publication remains a separate migration; this fixture
    // proves only that destruction/retirement use the passed canonical owner.
    second_world.destroy_object(id);
    second_world.cleanup_dead_objects().unwrap();
}

#[path = "factory_ai_creation_tests.rs"]
mod ai_creation_tests;
