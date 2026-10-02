use super::*;
use crate::object::body::active_body::ActiveBodyModuleData;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::BaseModuleData;
use game_engine::common::thing::module_factory::register_module_override;
use game_engine::system::xfer_load::XferLoad;
use game_engine::system::xfer_save::XferSave;
use std::io::Cursor;

const PROBE_NAME: &str = "FactoryCreationBorrowProbe";

/// Assert the C++ callback boundary before the body callback can reacquire
/// its owner's write lock. A failure is immediate rather than a hung test.
struct CreationProbe {
    owner_id: ObjectID,
    data: Arc<dyn ModuleData>,
    created: bool,
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
    TheGameLogic::remove_object(saved_id);
    TheGameLogic::remove_object(loaded_id);
}
