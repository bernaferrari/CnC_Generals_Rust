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

#[test]
fn factory_authored_body_creates_once_and_restores_live_health() {
    let _lock = crate::test_sync::lock();
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
