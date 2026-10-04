//! Actual Common INI -> template adapter -> authored ModuleFactory -> Object
//! installation. C++ Object.cpp:388-403 caches m_body before callbacks:458-462.
//! The temporary callback fixture receives its exact owner explicitly; it does
//! not register a fake Unit, a fake body, or an Object in an ambient registry.

use crate::common::{ObjectID, ObjectStatusMaskType};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::object::body::body_module::{BodyModuleInterface, MaxHealthChangeType};
use crate::object::body::inactive_body::InactiveBody;
use crate::object::{ModuleEntry, Object};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module::{
    BaseModuleData, Module, ModuleData, ModuleInterfaceType, ModuleType, Thing,
};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::any::Any;
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};

const ID: ObjectID = 0x7B3D_0031;
const PROBE: &str = "AuthoredBodyCacheProbe";
const ACTIVE_KINDS: [&str; 6] = [
    "ActiveBody",
    "StructureBody",
    "HighlanderBody",
    "ImmortalBody",
    "HiveStructureBody",
    "UndeadBody",
];

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_BODY_INSTALLATION_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

// The factory signature has no borrowed owner callback context. This test-only
// module data carries an explicit, consumed callback loan; no retained owner
// cycle or ambient current-owner selection remains after onObjectCreated.
struct CacheProbeData {
    base: BaseModuleData,
    owner: Mutex<Option<Arc<RwLock<Object>>>>,
}

impl std::fmt::Debug for CacheProbeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheProbeData").finish_non_exhaustive()
    }
}

impl ModuleData for CacheProbeData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: u32) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> u32 {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for CacheProbeData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

struct CacheProbe {
    data: Arc<dyn ModuleData>,
    created_with: (usize, f32, bool),
    observed: Option<(usize, f32)>,
}

impl Module for CacheProbe {
    fn get_module_name_key(&self) -> u32 {
        NameKeyGenerator::name_to_key(PROBE)
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
    fn on_object_created(&mut self) {
        let data = self.data.as_any().downcast_ref::<CacheProbeData>().unwrap();
        let owner = data
            .owner
            .lock()
            .unwrap()
            .take()
            .expect("explicit callback owner");
        let owner = owner
            .try_write()
            .expect("Object loan released before callbacks");
        assert!(!owner.modules_ready, "C++ callbacks precede modulesReady");
        assert_eq!(owner.modules.len(), 2, "all authored modules exist");
        assert!(owner.smc_helper.is_some(), "SMC helper precedes callbacks");
        // Object.cpp:307-335 omits special-damage helpers for InactiveBody.
        let active_helpers = !owner
            .modules
            .iter()
            .any(|entry| entry.name().as_str() == "InactiveBody");
        assert_eq!(owner.status_damage_helper.is_some(), active_helpers);
        assert_eq!(owner.subdual_damage_helper.is_some(), active_helpers);
        let body = owner
            .get_body_module()
            .expect("m_body is cached before callback");
        let pointer = Arc::as_ptr(&body) as *const () as usize;
        let health = body.lock().unwrap().get_health();
        assert_eq!(
            self.created_with,
            (pointer, health, owner.is_effectively_dead()),
            "same cached body exists before next constructor and before callbacks"
        );
        self.observed = Some((pointer, health));
    }
}

impl Snapshotable for CacheProbe {
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

fn probe_factory(_: Arc<dyn Thing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    // This is the actual next authored factory constructor. C++ caches the
    // body's interface before calling us, not only before onObjectCreated.
    let owner = data
        .as_ref()
        .as_any()
        .downcast_ref::<CacheProbeData>()
        .unwrap()
        .owner
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    let owner = owner
        .try_read()
        .expect("body installation released exact owner");
    let body = owner
        .get_body()
        .expect("m_body cached before next authored factory");
    let created_with = (
        Arc::as_ptr(&body) as *const () as usize,
        body.lock().unwrap().get_health(),
        owner.is_effectively_dead(),
    );
    Box::new(CacheProbe {
        data,
        created_with,
        observed: None,
    })
}
fn probe_data_factory(_: Option<&mut game_engine::common::ini::INI>) -> Box<dyn ModuleData> {
    Box::new(CacheProbeData {
        base: BaseModuleData::new(),
        owner: Mutex::new(None),
    })
}

fn template(
    name: &str,
    module: &str,
    max: f32,
    initial: f32,
    probe: bool,
) -> Arc<dyn crate::common::ThingTemplate> {
    assert!(ensure_thing_factory_exists(), "real empty ThingFactory");
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    if probe {
        get_module_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .add_module_internal(
                Some(probe_factory),
                Some(probe_data_factory),
                ModuleType::Behavior,
                PROBE,
                ModuleInterfaceType::NONE,
            );
    }
    let kind = if matches!(module, "StructureBody" | "HiveStructureBody") {
        "STRUCTURE"
    } else {
        "INERT"
    };
    let fields = if module == "InactiveBody" {
        String::new()
    } else {
        format!(
            "MaxHealth = {max}\n InitialHealth = {initial}\n SubdualDamageCap = 50\n SubdualDamageHealRate = 1000\n SubdualDamageHealAmount = 7"
        )
    };
    let callback = if probe {
        format!("Behavior = {PROBE} OwnedBodyProbe\n End\n")
    } else {
        String::new()
    };
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
        "Object {name}\n KindOf = {kind}\n Body = {module} OwnedBody\n {fields}\n End\n {callback}End\n"
    )), 1, "real Common INI authored body");
    TheThingFactory::find_template(name).unwrap()
}

struct Installed {
    owner: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    body: Arc<Mutex<dyn BodyModuleInterface>>,
}
impl Installed {
    fn new(name: &str, module: &str, max: f32, initial: f32) -> Self {
        let template = template(name, module, max, initial, true);
        let owner = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        assert!(
            owner.read().unwrap().get_body().is_none(),
            "raw constructor is inert"
        );
        assert!(!owner.read().unwrap().is_effectively_dead());
        let data = template
            .get_behavior_module_info()
            .iter()
            .find(|entry| entry.name.as_str() == PROBE)
            .unwrap();
        *data
            .data
            .as_any()
            .downcast_ref::<CacheProbeData>()
            .unwrap()
            .owner
            .lock()
            .unwrap() = Some(owner.clone());
        Object::init_modules_for(&owner, template.as_ref()).unwrap();
        let (entry, body) = {
            let object = owner.read().unwrap();
            (
                object.find_module_by_name(module).unwrap(),
                object.get_body().expect("exact authored owner cache"),
            )
        };
        let observed = owner
            .read()
            .unwrap()
            .find_module_by_name(PROBE)
            .unwrap()
            .with_module(|module| {
                module
                    .as_any()
                    .downcast_ref::<CacheProbe>()
                    .unwrap()
                    .observed
                    .unwrap()
            });
        assert_eq!(observed.0, Arc::as_ptr(&body) as *const () as usize);
        assert_eq!(
            observed.1,
            if module == "InactiveBody" {
                0.0
            } else {
                initial
            }
        );
        Self { owner, entry, body }
    }
    fn save(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        bytes
    }
    fn save_interface(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.body
            .lock()
            .unwrap()
            .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
        bytes
    }
}

#[test]
fn authored_body_kinds_same_id_install_distinct_state_before_callbacks() {
    if !child(concat!(
        module_path!(),
        "::authored_body_kinds_same_id_install_distinct_state_before_callbacks"
    )) {
        return;
    }
    for module in ACTIVE_KINDS.into_iter().chain(["InactiveBody"]) {
        let first = Installed::new(&format!("BodyFirst{module}"), module, 400.0, 300.0);
        let second = Installed::new(&format!("BodySecond{module}"), module, 800.0, 600.0);
        assert!(!Arc::ptr_eq(&first.body, &second.body));
        if module == "InactiveBody" {
            assert!(first.owner.read().unwrap().is_effectively_dead());
            assert!(second.owner.read().unwrap().is_effectively_dead());
            assert_eq!(first.body.lock().unwrap().get_health(), 0.0);
        } else {
            assert_eq!(first.body.lock().unwrap().get_health(), 300.0);
            assert_eq!(first.body.lock().unwrap().get_max_health(), 400.0);
            assert_eq!(second.body.lock().unwrap().get_health(), 600.0);
            assert_eq!(second.body.lock().unwrap().get_max_health(), 800.0);
            first.body.lock().unwrap().apply_damage_scalar(2.0).unwrap();
            assert_eq!(first.body.lock().unwrap().get_damage_scalar(), 2.0);
            assert_eq!(second.body.lock().unwrap().get_damage_scalar(), 1.0);
        }
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .is_none()
        );
        assert!(TheGameLogic::find_object_by_id(ID).is_none());
        let retained = first.body.clone();
        drop(first);
        assert!(!Arc::ptr_eq(&retained, &second.body));
    }
}

#[test]
fn authored_body_cached_snapshot_and_binding_xfer_restore_one_runtime() {
    if !child(concat!(
        module_path!(),
        "::authored_body_cached_snapshot_and_binding_xfer_restore_one_runtime"
    )) {
        return;
    }
    for module in ACTIVE_KINDS {
        let first = Installed::new(&format!("BodyXferFirst{module}"), module, 400.0, 300.0);
        let second = Installed::new(&format!("BodyXferSecond{module}"), module, 800.0, 600.0);
        {
            let mut body = first.body.lock().unwrap();
            body.set_max_health(600.0, MaxHealthChangeType::SameCurrentHealth)
                .unwrap();
            body.set_initial_health(25).unwrap();
            body.set_front_crushed(true).unwrap();
            body.set_back_crushed(true).unwrap();
            body.apply_damage_scalar(2.5).unwrap();
        }
        let saved = first.save();
        assert_eq!(
            first.save_interface(),
            saved,
            "cached interface preserves derived C++ Snapshot chain: {module}"
        );
        second.entry.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
                .unwrap();
            module.load_post_process().unwrap();
        });
        assert_eq!(second.save(), saved);
        assert_eq!(second.save_interface(), saved);
        {
            let body = second.body.lock().unwrap();
            assert_eq!(body.get_health(), 150.0);
            assert_eq!(body.get_previous_health(), 300.0);
            assert_eq!(body.get_max_health(), 600.0);
            assert_eq!(body.get_initial_health(), 600.0);
            assert!(body.get_front_crushed());
            assert!(body.get_back_crushed());
            assert_eq!(body.get_damage_scalar(), 2.5);
        }
        assert!(Arc::ptr_eq(
            &second.body,
            &second.owner.read().unwrap().get_body().unwrap()
        ));
        first.body.lock().unwrap().apply_damage_scalar(3.0).unwrap();
        assert_eq!(second.body.lock().unwrap().get_damage_scalar(), 2.5);
    }
    let inactive = Installed::new("BodyXferInactive", "InactiveBody", 0.0, 0.0);
    assert_eq!(
        inactive.save_interface(),
        inactive.save(),
        "inactive concrete Snapshot prefix"
    );
}

#[test]
fn authored_body_object_module_list_xfer_reaches_exact_cached_body() {
    if !child(concat!(
        module_path!(),
        "::authored_body_object_module_list_xfer_reaches_exact_cached_body"
    )) {
        return;
    }
    let first = Installed::new("BodyListXferFirst", "StructureBody", 400.0, 300.0);
    let second = Installed::new("BodyListXferSecond", "StructureBody", 800.0, 600.0);
    first.body.lock().unwrap().set_initial_health(20).unwrap();
    let mut saved = Vec::new();
    first
        .owner
        .write()
        .unwrap()
        .xfer_behavior_module_list(&mut XferSave::new(Cursor::new(&mut saved), 1), true);
    second
        .owner
        .write()
        .unwrap()
        .xfer_behavior_module_list(&mut XferLoad::new(Cursor::new(saved), 1), false);
    assert_eq!(second.body.lock().unwrap().get_health(), 60.0);
    assert_eq!(second.body.lock().unwrap().get_max_health(), 400.0);
    assert!(Arc::ptr_eq(
        &second.body,
        &second.owner.read().unwrap().get_body().unwrap()
    ));
}

#[test]
fn inactive_construction_is_inert_and_installation_mutates_exact_same_id_owner() {
    if !child(concat!(
        module_path!(),
        "::inactive_construction_is_inert_and_installation_mutates_exact_same_id_owner"
    )) {
        return;
    }
    let _authored = template("BodyPublishedAlive", "ActiveBody", 400.0, 300.0, false);
    let mut factory = crate::object::object_factory::ObjectFactory::new();
    let id = factory
        .create_object(
            "BodyPublishedAlive",
            crate::common::Coord3D::default(),
            None,
            crate::object::object_factory::ObjectCreationFlags::NO_DRAWABLE
                | crate::object::object_factory::ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let published = factory.get_object(id).unwrap().get_base_object().unwrap();
    assert!(!published.read().unwrap().is_effectively_dead());
    let _uninstalled = InactiveBody::new_with_owner(Default::default(), id);
    assert!(
        !published.read().unwrap().is_effectively_dead(),
        "body constructor cannot mutate a previously published same-ID Object"
    );
    let authored = template("BodyUnadmittedInactive", "InactiveBody", 0.0, 0.0, false);
    let owner = Arc::new(RwLock::new(Object::new_raw(
        authored.clone(),
        id,
        ObjectStatusMaskType::NONE,
        None,
    )));
    Object::init_modules_for(&owner, authored.as_ref()).unwrap();
    assert!(owner.read().unwrap().is_effectively_dead());
    assert!(
        !published.read().unwrap().is_effectively_dead(),
        "installation selects its exact owner"
    );
    assert_eq!(
        published
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        300.0
    );
    assert!(!Arc::ptr_eq(
        &published.read().unwrap().get_body().unwrap(),
        &owner.read().unwrap().get_body().unwrap()
    ));
}
