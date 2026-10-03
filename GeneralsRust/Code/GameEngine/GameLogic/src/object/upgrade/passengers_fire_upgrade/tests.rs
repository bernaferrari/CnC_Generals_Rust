//! Installed owner and UpgradeMux contracts from PassengersFireUpgrade.cpp:34–45,
//! UpgradeModule.cpp:140–201, and their versioned Xfer/CRC base chain.

use super::*;
use crate::common::{Coord3D, FXListId, FXListManagerInterface, ThingId};
use crate::helpers::TheThingFactory;
use crate::modules::ContainModuleInterface;
use crate::object::contain::transport_contain::{TransportContain, TransportContainModuleData};
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

const ID: ObjectID = 0x7A3C_0001;
const TRIGGER: &str = "Upgrade_OwnedPassengerFireTrigger";
const CONFLICT: &str = "Upgrade_OwnedPassengerFireConflict";

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(name).to_bits())
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
}

impl Installed {
    fn new(name: &str, fields: &str, with_transport: bool) -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        {
            let mut factory = get_thing_factory().unwrap();
            assert_eq!(factory.as_mut().unwrap().load_ini_text(&format!(
                "Object {name}\n KindOf = INERT\n Behavior = PassengersFireUpgrade OwnedFiring\n TriggeredBy = {TRIGGER}\n {fields}\n End\nEnd\n"
            )), 1);
        }
        let template = TheThingFactory::find_template(name).expect("authored upgrade definition");
        let object = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            crate::common::ObjectStatusMaskType::NONE,
            None,
        )));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entry = object
            .read()
            .unwrap()
            .find_module_by_name("PassengersFireUpgrade")
            .expect("installed authored upgrade");
        if with_transport {
            // ContainBindingModule still attaches through canonical ID discovery,
            // so its authored installation is a separate ownership contract.
            // Attach the real runtime through the existing Object operation;
            // the upgrade itself always takes the actual installed factory path.
            let mut data = TransportContainModuleData::default();
            data.slot_capacity = 4;
            assert!(!data.base.passengers_allowed_to_fire);
            let transport = TransportContain::new(Arc::downgrade(&object), &data).unwrap();
            let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(transport));
            object.write().unwrap().set_contain(Some(contain));
        }
        Self { object, entry }
    }

    fn applied(&self) -> bool {
        self.entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<PassengersFireUpgrade>()
                .unwrap()
                .applied
        })
    }

    fn fires(&self) -> bool {
        let contain = self
            .object
            .read()
            .unwrap()
            .get_contain()
            .expect("real transport");
        let fires = contain.lock().unwrap().is_passenger_allowed_to_fire(None);
        fires
    }

    fn apply(&self, key: UpgradeMaskType) {
        self.object.write().unwrap().apply_upgrade_modules(key);
    }

    fn xfer_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        bytes
    }

    fn crc_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .crc(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        bytes
    }
}

#[derive(Debug)]
struct Effects {
    entry: Arc<ModuleEntry>,
    calls: AtomicUsize,
    require_trigger: bool,
}

impl FXListManagerInterface for Effects {
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("passenger upgrade FX must receive the driving owner");
    }

    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("passenger upgrade FX must not rediscover an object by ID");
    }

    fn do_fx_for_object(&self, _: FXListId, owner: &Object) {
        assert_eq!(owner.get_id(), ID);
        assert!(
            self.entry.module.try_lock().is_ok(),
            "FX runs outside installed module guard"
        );
        assert!(Arc::ptr_eq(
            &self.entry,
            &owner.find_module_by_name("PassengersFireUpgrade").unwrap()
        ));
        if self.require_trigger {
            assert!(
                owner.completed_upgrades().intersects(mask(TRIGGER)),
                "CPP FX precedes self-removal"
            );
        }
        let contain = owner.get_contain().expect("actual transport owner");
        assert!(
            !contain.lock().unwrap().is_passenger_allowed_to_fire(None),
            "CPP FX precedes passenger firing implementation"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

fn effects(fixture: &Installed, require_trigger: bool) -> Arc<Effects> {
    let recorder = Arc::new(Effects {
        entry: fixture.entry.clone(),
        calls: AtomicUsize::new(0),
        require_trigger,
    });
    assert!(crate::helpers::register_fx_list_manager(recorder.clone()));
    recorder
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    let name = name.strip_prefix("gamelogic::").unwrap_or(name);
    matches!(
        crate::test_process::run_bounded(name, "GENERALS_PASSENGER_FIRE_OWNED_CHILD"),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

#[test]
fn authored_same_id_owners_interleave_reset_and_query_clone() {
    if !child(concat!(
        module_path!(),
        "::authored_same_id_owners_interleave_reset_and_query_clone"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    let a = Installed::new("OwnedPassengerFireA", "", true);
    let b = Installed::new("OwnedPassengerFireB", "", true);
    assert!(!Arc::ptr_eq(&a.entry, &b.entry));
    assert!(
        !a.applied() && !b.applied() && !a.fires() && !b.fires(),
        "constructors must not execute the upgrade"
    );
    let query = a.object.clone();
    let query_entry = query
        .read()
        .unwrap()
        .find_module_by_name("PassengersFireUpgrade")
        .unwrap();
    assert!(Arc::ptr_eq(&query_entry, &a.entry));
    a.apply(mask(TRIGGER));
    assert!(a.applied() && a.fires() && !b.applied() && !b.fires());
    b.apply(mask(TRIGGER));
    assert!(a.applied() && b.applied() && a.fires() && b.fires());
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied() && b.applied());
    assert!(
        a.fires() && b.fires(),
        "CPP reset never disables passenger firing"
    );
    a.apply(mask(TRIGGER));
    assert!(a.applied() && b.applied());
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn missing_contain_still_commits_executed_and_reset() {
    if !child(concat!(
        module_path!(),
        "::missing_contain_still_commits_executed_and_reset"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new("OwnedPassengerFireNoContain", "", false);
    assert!(fixture.object.read().unwrap().get_contain().is_none());
    assert!(!fixture.applied());
    fixture.apply(mask(TRIGGER));
    assert!(
        fixture.applied(),
        "CPP base commits after optional contain implementation"
    );
    assert_eq!(fixture.xfer_bytes(), vec![1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(fixture.crc_bytes(), vec![1, 1]);
    fixture
        .object
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(TRIGGER));
    assert!(!fixture.applied());
}

#[test]
fn self_removal_and_fx_retain_driving_owner_outside_module_guard() {
    if !child(concat!(
        module_path!(),
        "::self_removal_and_fx_retain_driving_owner_outside_module_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new(
        "OwnedPassengerFireSelfRemoval",
        &format!("RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedPassengerFire"),
        true,
    );
    let recorder = effects(&fixture, true);
    fixture
        .object
        .write()
        .unwrap()
        .object_upgrades_completed
        .insert(mask(TRIGGER));
    fixture.apply(mask(TRIGGER));
    assert!(fixture.applied() && fixture.fires());
    assert!(
        !fixture
            .object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(TRIGGER))
    );
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    fixture.apply(mask(TRIGGER));
    assert_eq!(
        recorder.calls.load(Ordering::SeqCst),
        1,
        "executed upgrade skips FX"
    );
}

#[test]
fn conflicting_key_skips_fx_and_xfer_restores_only_destination_state() {
    if !child(concat!(
        module_path!(),
        "::conflicting_key_skips_fx_and_xfer_restores_only_destination_state"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedPassengerFireSave",
        &format!("ConflictsWith = {CONFLICT}\n FXListUpgrade = FX_OwnedPassengerFire"),
        true,
    );
    let b = Installed::new("OwnedPassengerFireRestore", "", true);
    let recorder = effects(&a, false);
    assert_eq!(a.xfer_bytes(), vec![1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(a.crc_bytes(), vec![1, 0]);
    a.apply(mask(TRIGGER) | mask(CONFLICT));
    assert!(!a.applied() && !a.fires());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 0);
    a.apply(mask(TRIGGER));
    assert!(a.applied() && a.fires());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    let bytes = a.xfer_bytes();
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(a.crc_bytes(), vec![1, 1]);
    b.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap()
    });
    assert!(a.applied() && b.applied());
    assert!(!b.fires(), "upgrade Xfer never replays the implementation");
    b.apply(mask(TRIGGER));
    assert!(
        !b.fires(),
        "restored execution flag prevents duplicate implementation"
    );
    a.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied() && b.applied() && a.fires());
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!b.applied() && !b.fires());
    b.apply(mask(TRIGGER));
    assert!(b.applied() && b.fires());
    assert!(!a.applied() && a.fires());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}
