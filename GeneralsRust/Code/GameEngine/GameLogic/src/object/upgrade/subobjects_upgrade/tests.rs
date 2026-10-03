//! Installed SubObjectsUpgrade ownership and CPP UpgradeMux ordering.
//! SubObjectsUpgrade.cpp:65–115,140–151; UpgradeModule.cpp:140–146,191–201.

use super::*;
use crate::common::{Coord3D, FXListId, FXListManagerInterface, ThingId};
use crate::helpers::TheThingFactory;
use crate::object::draw::{W3DModelDraw, W3DModelDrawModuleData};
use crate::object::drawable::{Drawable, DrawableExt, DrawableModuleHandle, DrawableType};
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module::ModuleInterfaceType;
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A3B_0001;
const TRIGGER: &str = "Upgrade_OwnedSubObjectsTrigger";
const CONFLICT: &str = "Upgrade_OwnedSubObjectsConflict";
const REMOVED: &str = "Upgrade_OwnedSubObjectsRemoved";

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(name).to_bits())
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    draw: DrawableModuleHandle,
}

impl Installed {
    fn new(name: &str, fields: &str) -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let needs_module_factory = game_engine::common::thing::module_factory::get_module_factory()
            .unwrap()
            .is_none();
        if needs_module_factory {
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let loaded = {
            let mut factory = get_thing_factory().unwrap();
            let factory = factory.as_mut().unwrap();
            factory.load_ini_text(&format!(
                "Object {name}\n KindOf = INERT\n Behavior = SubObjectsUpgrade OwnedVisibility\n TriggeredBy = {TRIGGER}\n {fields}\n End\nEnd\n"
            ))
        };
        assert_eq!(loaded, 1);
        let template = TheThingFactory::find_template(name).expect("authored object definition");
        // These independent objects deliberately remain unregistered. The
        // actual authored module factory and Object installation path run,
        // while identical IDs cannot select an ambient owner.
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
            .find_module_by_name("SubObjectsUpgrade")
            .expect("canonical authored SubObjectsUpgrade entry");
        let data = W3DModelDrawModuleData::new();
        let mut drawable = Drawable::new(ID + 1, ID, name.to_owned(), DrawableType::Static);
        let draw = drawable.add_module(
            ModuleInterfaceType::DRAW,
            AsciiString::from("W3DModelDraw"),
            AsciiString::from("ActualVisibility"),
            Arc::new(data.clone()),
            Box::new(W3DModelDraw::new(data)),
        );
        object
            .write()
            .unwrap()
            .set_drawable(Some(Arc::new(RwLock::new(drawable))));
        Self {
            object,
            entry,
            draw,
        }
    }

    fn applied(&self) -> bool {
        self.entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<SubObjectsUpgrade>()
                .unwrap()
                .applied
        })
    }

    fn apply(&self) {
        self.object
            .write()
            .unwrap()
            .apply_upgrade_modules(mask(TRIGGER));
    }

    fn visibility(&self) -> Vec<(String, bool)> {
        // Read the real model module's saved runtime overrides, rather than
        // adding a duplicate visibility map or a production-only test getter.
        let mut bytes = Vec::new();
        self.draw.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
        });
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        for max in [2, 1, 1, 1] {
            let mut version = 0;
            load.xfer_version(&mut version, max).unwrap();
        }
        for _ in 0..3 {
            let mut recoils = 0;
            load.xfer_unsigned_byte(&mut recoils).unwrap();
            assert_eq!(recoils, 0, "fixture never fires or applies recoil");
        }
        let mut count = 0;
        load.xfer_unsigned_byte(&mut count).unwrap();
        (0..count)
            .map(|_| {
                let mut name = String::new();
                let mut hidden = false;
                load.xfer_ascii_string(&mut name).unwrap();
                load.xfer_bool(&mut hidden).unwrap();
                (name, hidden)
            })
            .collect()
    }

    fn xfer_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
        });
        bytes
    }

    fn crc_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .crc(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
        });
        bytes
    }
}

#[derive(Debug)]
struct Effects {
    entries: Vec<Arc<ModuleEntry>>,
    calls: AtomicUsize,
}

impl FXListManagerInterface for Effects {
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("SubObjectsUpgrade must use the actual object FX boundary");
    }

    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("SubObjectsUpgrade FX must retain the driving owner borrow");
    }

    fn do_fx_for_object(&self, _: FXListId, object: &Object) {
        assert_eq!(object.get_id(), ID);
        for entry in &self.entries {
            assert!(
                entry.module.try_lock().is_ok(),
                "FX must run outside module guards"
            );
        }
        let actual = object.find_module_by_name("SubObjectsUpgrade").unwrap();
        assert!(self.entries.iter().any(|entry| Arc::ptr_eq(entry, &actual)));
        assert!(
            object
                .get_template()
                .get_name()
                .as_str()
                .starts_with("OwnedSubObjects")
        );
        let removals = actual.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<SubObjectsUpgrade>()
                .unwrap()
                .data
                .upgrade_mux_data
                .removal_upgrade_names
                .clone()
        });
        for removal in removals {
            assert!(
                object
                    .completed_upgrades()
                    .intersects(mask(removal.as_str())),
                "CPP FX precedes removal of {removal}"
            );
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

fn effects(installed: &[&Installed]) -> Arc<Effects> {
    let recorder = Arc::new(Effects {
        entries: installed
            .iter()
            .map(|fixture| fixture.entry.clone())
            .collect(),
        calls: AtomicUsize::new(0),
    });
    assert!(crate::helpers::register_fx_list_manager(recorder.clone()));
    recorder
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    let name = name.strip_prefix("gamelogic::").unwrap_or(name);
    matches!(
        crate::test_process::run_bounded(name, "GENERALS_SUBOBJECTS_OWNED_CHILD",),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

#[test]
fn authored_same_id_objects_interleave_reset_and_query_alias() {
    if !child(concat!(
        module_path!(),
        "::authored_same_id_objects_interleave_reset_and_query_alias"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    let a = Installed::new(
        "OwnedSubObjectsA",
        "ShowSubObjects = A\n HideSubObjects = Shared",
    );
    let b = Installed::new(
        "OwnedSubObjectsB",
        "ShowSubObjects = B\n HideSubObjects = A",
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(!Arc::ptr_eq(&a.entry, &b.entry));
    assert!(!a.applied() && !b.applied());
    assert!(a.visibility().is_empty() && b.visibility().is_empty());
    let query = Arc::clone(&a.object);
    let query_entry = query
        .read()
        .unwrap()
        .find_module_by_name("SubObjectsUpgrade")
        .unwrap();
    assert!(Arc::ptr_eq(&query_entry, &a.entry));
    a.apply();
    assert!(a.applied() && !b.applied());
    assert_eq!(
        a.visibility(),
        vec![("a".into(), false), ("shared".into(), true)]
    );
    assert!(b.visibility().is_empty());
    b.apply();
    assert!(a.applied() && b.applied());
    assert_eq!(
        b.visibility(),
        vec![("b".into(), false), ("a".into(), true)]
    );
    let visible = a.visibility();
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied() && b.applied());
    assert_eq!(
        a.visibility(),
        visible,
        "CPP reset does not undo visibility"
    );
    a.apply();
    assert!(a.applied() && b.applied());
    assert_eq!(a.visibility(), visible);
}

#[test]
fn removal_of_current_trigger_runs_outside_installed_module_guard() {
    if !child(concat!(
        module_path!(),
        "::removal_of_current_trigger_runs_outside_installed_module_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedSubObjectsSelfRemoval",
        &format!(
            "ShowSubObjects = A\n RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedSubObjects"
        ),
    );
    let recorder = effects(&[&a]);
    a.object
        .write()
        .unwrap()
        .object_upgrades_completed
        .insert(mask(TRIGGER));
    a.apply();
    assert!(a.applied());
    assert!(
        !a.object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(TRIGGER))
    );
    assert_eq!(a.visibility(), vec![("a".into(), false)]);
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn removals_precede_visibility_conflicts_and_blocked_attempt_still_executes() {
    if !child(concat!(
        module_path!(),
        "::removals_precede_visibility_conflicts_and_blocked_attempt_still_executes"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedSubObjectsRemoveConflict",
        &format!(
            "ShowSubObjects = A\n ConflictsWith = {CONFLICT}\n RemovesUpgrades = {CONFLICT}\n FXListUpgrade = FX_OwnedSubObjects"
        ),
    );
    let b = Installed::new(
        "OwnedSubObjectsBlocked",
        &format!(
            "ShowSubObjects = B\n ConflictsWith = {CONFLICT}\n FXListUpgrade = FX_OwnedSubObjects"
        ),
    );
    let recorder = effects(&[&a, &b]);
    for fixture in [&a, &b] {
        fixture
            .object
            .write()
            .unwrap()
            .object_upgrades_completed
            .insert(mask(CONFLICT));
        fixture.apply();
        assert!(
            fixture.applied(),
            "CPP marks executed after implementation even if blocked"
        );
    }
    assert_eq!(a.visibility(), vec![("a".into(), false)]);
    assert!(b.visibility().is_empty());
    assert!(
        !a.object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(CONFLICT))
    );
    assert!(
        b.object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(CONFLICT))
    );
    b.apply();
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn xfer_restores_only_destination_and_refresh_has_no_fx_or_removals() {
    if !child(concat!(
        module_path!(),
        "::xfer_restores_only_destination_and_refresh_has_no_fx_or_removals"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedSubObjectsSave",
        "ShowSubObjects = A\n FXListUpgrade = FX_OwnedSubObjects",
    );
    let b = Installed::new(
        "OwnedSubObjectsRestore",
        &format!(
            "ShowSubObjects = B\n ConflictsWith = {CONFLICT}\n RemovesUpgrades = {REMOVED}\n FXListUpgrade = FX_OwnedSubObjects"
        ),
    );
    let recorder = effects(&[&a, &b]);
    assert_eq!(a.xfer_bytes(), vec![1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(a.crc_bytes(), vec![1, 0]);
    a.apply();
    let bytes = a.xfer_bytes();
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(a.crc_bytes(), vec![1, 1]);
    b.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
    });
    assert!(a.applied() && b.applied());
    assert!(
        b.visibility().is_empty(),
        "module Xfer does not execute visibility"
    );
    {
        let mut object = b.object.write().unwrap();
        object
            .object_upgrades_completed
            .insert(mask(CONFLICT) | mask(REMOVED));
        object.force_refresh_sub_object_upgrade_status();
    }
    assert!(
        b.visibility().is_empty(),
        "refresh honors current conflicts"
    );
    {
        let mut object = b.object.write().unwrap();
        object.remove_upgrade_mask(mask(CONFLICT));
        object.force_refresh_sub_object_upgrade_status();
    }
    assert_eq!(b.visibility(), vec![("b".into(), false)]);
    assert!(
        b.object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(REMOVED))
    );
    assert_eq!(
        recorder.calls.load(Ordering::SeqCst),
        1,
        "refresh must not replay FX"
    );
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(a.applied() && !b.applied());
    assert_eq!(a.visibility(), vec![("a".into(), false)]);
    assert_eq!(b.visibility(), vec![("b".into(), false)]);
}
