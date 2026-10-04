//! C++ Object.cpp:384–462 installs the containment interface from the same
//! authored behavior before invoking any onObjectCreated callback.

use crate::common::{Coord3D, ObjectID, ObjectStatusMaskType, UpgradeMaskType};
use crate::helpers::TheThingFactory;
use crate::modules::ContainModuleInterface;
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};

const ID: ObjectID = 0x7B3D_0011;
const TRIGGER: &str = "Upgrade_AuthoredContainFiring";

fn child(name: &str) -> bool {
    let name = name.strip_prefix("gamelogic::").unwrap_or(name);
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(name, "GENERALS_CONTAIN_INSTALLATION_CHILD"),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

struct Installed {
    owner: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
}

impl Installed {
    fn new(name: &str, module: &str, fields: &str) -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        {
            let mut factory = get_thing_factory().unwrap();
            assert_eq!(
                factory.as_mut().unwrap().load_ini_text(&format!(
                    "Object {name}\n KindOf = INERT\n Behavior = {module} OwnedContainer\n {fields}\n End\n Behavior = PassengersFireUpgrade OwnedFiring\n TriggeredBy = {TRIGGER}\n End\nEnd\n"
                )),
                1,
                "actual authored container and upgrade"
            );
        }
        let template = TheThingFactory::find_template(name).unwrap();
        let owner = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        assert!(
            owner.read().unwrap().get_contain().is_none(),
            "constructor is inert"
        );
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .is_none()
        );
        Object::init_modules_for(&owner, template.as_ref()).unwrap();
        let (entry, contain) = {
            let guard = owner.read().unwrap();
            (
                guard.find_module_by_name(module).unwrap(),
                guard
                    .get_contain()
                    .expect("authored containment attached to driving owner"),
            )
        };
        Self {
            owner,
            entry,
            contain,
        }
    }

    fn fires(&self) -> bool {
        self.contain
            .lock()
            .unwrap()
            .is_passenger_allowed_to_fire(None)
    }

    fn save(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
        });
        bytes
    }
}

#[test]
fn authored_transport_same_id_owners_attach_distinct_runtime_and_upgrade() {
    if !child(concat!(
        module_path!(),
        "::authored_transport_same_id_owners_attach_distinct_runtime_and_upgrade"
    )) {
        return;
    }
    let first = Installed::new(
        "ContainInstallFirst",
        "TransportContain",
        "Slots = 4\n PassengersAllowedToFire = No",
    );
    let second = Installed::new(
        "ContainInstallSecond",
        "TransportContain",
        "Slots = 7\n PassengersAllowedToFire = Yes",
    );
    assert!(!Arc::ptr_eq(&first.contain, &second.contain));
    assert_eq!(first.contain.lock().unwrap().get_max_capacity(), 4);
    assert_eq!(second.contain.lock().unwrap().get_max_capacity(), 7);
    assert!(!first.fires());
    assert!(second.fires());
    let trigger =
        UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(TRIGGER).to_bits());
    first.owner.write().unwrap().apply_upgrade_modules(trigger);
    assert!(
        first.fires(),
        "installed authored upgrade reaches the exact authored transport"
    );
    second
        .contain
        .lock()
        .unwrap()
        .set_passenger_allowed_to_fire(false);
    assert!(first.fires());
    assert!(!second.fires());
    drop(first);
    assert_eq!(second.contain.lock().unwrap().get_max_capacity(), 7);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn authored_open_contain_callback_preserves_exact_installed_handle() {
    if !child(concat!(
        module_path!(),
        "::authored_open_contain_callback_preserves_exact_installed_handle"
    )) {
        return;
    }
    let first = Installed::new(
        "OpenContainInstallFirst",
        "OpenContain",
        "ContainMax = 2\n PassengersAllowedToFire = No",
    );
    let second = Installed::new(
        "OpenContainInstallSecond",
        "OpenContain",
        "ContainMax = 9\n PassengersAllowedToFire = Yes",
    );
    first.entry.with_module(|module| module.on_object_created());
    assert!(Arc::ptr_eq(
        &first.contain,
        &first.owner.read().unwrap().get_contain().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &second.contain,
        &second.owner.read().unwrap().get_contain().unwrap()
    ));
    assert_eq!(first.contain.lock().unwrap().get_max_capacity(), 2);
    assert_eq!(second.contain.lock().unwrap().get_max_capacity(), 9);
    assert!(!first.fires());
    assert!(second.fires());
}

#[test]
fn authored_open_wrapper_xfer_uses_same_installed_state() {
    if !child(concat!(
        module_path!(),
        "::authored_open_wrapper_xfer_uses_same_installed_state"
    )) {
        return;
    }
    // OpenContain directly exposes its inherited rally-point operations. The
    // TransportContain trait delegation is a separate runtime contract.
    let first = Installed::new("OpenXferFirst", "OpenContain", "ContainMax = 4");
    let second = Installed::new("OpenXferSecond", "OpenContain", "ContainMax = 7");
    let rally = Coord3D::new(23.0, 71.0, 5.0);
    first.contain.lock().unwrap().set_rally_point(rally);
    let saved = first.save();
    let mut direct = Vec::new();
    first
        .contain
        .lock()
        .unwrap()
        .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut direct), 1))
        .unwrap();
    assert_eq!(saved, direct, "binding forwards to installed runtime");
    second.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap();
        module.load_post_process().unwrap();
    });
    assert_eq!(second.save(), saved);
    assert_eq!(
        second.contain.lock().unwrap().get_rally_point(),
        Some(rally),
        "interface mutation crosses binding Xfer into the exact destination interface"
    );
    assert_eq!(
        second.contain.lock().unwrap().get_max_capacity(),
        7,
        "load keeps destination authored configuration"
    );
    let mut crc = Vec::new();
    second.entry.with_module(|module| {
        module
            .crc(&mut XferSave::new(Cursor::new(&mut crc), 1))
            .unwrap()
    });
    assert!(crc.is_empty(), "C++ Module/Update/OpenContain CRC is empty");
    assert!(Arc::ptr_eq(
        &second.contain,
        &second.owner.read().unwrap().get_contain().unwrap()
    ));
}

#[test]
fn authored_transport_wrapper_restores_inherited_rally_to_installed_runtime() {
    if !child(concat!(
        module_path!(),
        "::authored_transport_wrapper_restores_inherited_rally_to_installed_runtime"
    )) {
        return;
    }
    // C++ TransportContain.cpp:655-684 extends OpenContain's Xfer and
    // post-load hooks; OpenContain.cpp:1678-1681 serializes inherited rally.
    let first = Installed::new("TransportXferFirst", "TransportContain", "Slots = 4");
    let second = Installed::new("TransportXferSecond", "TransportContain", "Slots = 7");
    assert!(!Arc::ptr_eq(&first.contain, &second.contain));
    let rally = Coord3D::new(37.0, 83.0, 6.0);
    first.contain.lock().unwrap().set_rally_point(rally);
    assert_eq!(second.contain.lock().unwrap().get_rally_point(), None);
    let saved = first.save();
    let mut direct = Vec::new();
    first
        .contain
        .lock()
        .unwrap()
        .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut direct), 1))
        .unwrap();
    assert_eq!(saved, direct, "binding and interface serialize one runtime");
    second.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap();
        module.load_post_process().unwrap();
    });
    assert_eq!(
        second.contain.lock().unwrap().get_rally_point(),
        Some(rally)
    );
    assert_eq!(second.save(), saved);
    assert_eq!(first.contain.lock().unwrap().get_max_capacity(), 4);
    assert_eq!(second.contain.lock().unwrap().get_max_capacity(), 7);
    assert!(Arc::ptr_eq(
        &second.contain,
        &second.owner.read().unwrap().get_contain().unwrap()
    ));
    first
        .contain
        .lock()
        .unwrap()
        .set_rally_point(Coord3D::new(1.0, 2.0, 3.0));
    assert_eq!(
        second.contain.lock().unwrap().get_rally_point(),
        Some(rally)
    );
}
