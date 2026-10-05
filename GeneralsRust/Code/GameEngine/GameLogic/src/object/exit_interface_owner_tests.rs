//! Canonical exit descriptors: Object.cpp2535–2558 selects a behavior before
//! containment;4383–4394 serializes only the ordered modules, not cached interfaces.
//! These are ownership-preservation controls, not evidence of a new CPP feature.

use super::*;
use crate::helpers::TheThingFactory;
use crate::modules::{ExitDoorType, ExitInterface};
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;

type TestExit = ExitInterfaceHandle;

fn rally(exit: &TestExit) -> Option<Coord3D> {
    exit.get_rally_point().unwrap()
}

fn reserve(exit: &mut TestExit) -> ExitDoorType {
    exit.reserve_door_for_exit(None, None)
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_EXIT_DESCRIPTOR_OWNER_CHILD",
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

const ID: ObjectID = 0x763E_0011;

fn installed(name: &str, production_exit: bool) -> Arc<RwLock<Object>> {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let exit = if production_exit {
        "Behavior = DefaultProductionExitUpdate ProductionExit\n UnitCreatePoint = X:2 Y:3 Z:4\n NaturalRallyPoint = X:5 Y:6 Z:7\n End\n"
    } else {
        ""
    };
    let ini = format!(
        "Object {name}\n KindOf = INERT\n {exit}Behavior = TransportContain Cargo\n Slots = 2\n NumberOfExitPaths = 0\n End\nEnd\n"
    );
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&ini),
        1
    );
    let template = TheThingFactory::find_template(name).expect("authored exact definition");
    let owner = Arc::new(RwLock::new(Object::new_raw(
        Arc::clone(&template),
        ID,
        crate::common::ObjectStatusMaskType::NONE,
        None,
    )));
    Object::init_modules_for(&owner, template.as_ref()).unwrap();
    assert!(
        owner
            .read()
            .unwrap()
            .find_module_by_name("TransportContain")
            .is_some()
    );
    assert!(owner.read().unwrap().get_contain().is_some());
    owner
}

fn set_contain_rally(owner: &Arc<RwLock<Object>>, point: Coord3D) {
    let contain = owner.read().unwrap().get_contain().unwrap();
    contain.lock().unwrap().set_rally_point(point);
}

fn ordered_module_bytes(owner: &Object) -> Vec<Vec<u8>> {
    owner
        .modules
        .iter()
        .map(|entry| {
            let mut bytes = Vec::new();
            entry.with_module(|module| {
                module
                    .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                    .unwrap()
            });
            bytes
        })
        .collect()
}

#[test]
fn installed_same_id_exit_queries_keep_distinct_contain_state() {
    if !child(concat!(
        module_path!(),
        "::installed_same_id_exit_queries_keep_distinct_contain_state"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let first = installed("OwnedExitFirst", false);
    let second = installed("OwnedExitSecond", false);
    assert_eq!(
        first.read().unwrap().get_id(),
        second.read().unwrap().get_id()
    );
    let first_contain = first.read().unwrap().get_contain().unwrap();
    let second_contain = second.read().unwrap().get_contain().unwrap();
    assert!(!Arc::ptr_eq(&first_contain, &second_contain));
    let first_exit = first.read().unwrap().get_object_exit_interface().unwrap();
    let first_again = first.read().unwrap().get_object_exit_interface().unwrap();
    let second_exit = second.read().unwrap().get_object_exit_interface().unwrap();
    let a = Coord3D::new(11.0, 12.0, 13.0);
    let b = Coord3D::new(21.0, 22.0, 23.0);
    set_contain_rally(&first, a);
    set_contain_rally(&second, b);
    assert_eq!(rally(&first_exit), Some(a));
    assert_eq!(rally(&first_again), Some(a));
    assert_eq!(rally(&second_exit), Some(b));
    let changed = Coord3D::new(31.0, 32.0, 33.0);
    set_contain_rally(&first, changed);
    assert_eq!(rally(&first_exit), Some(changed));
    assert_eq!(rally(&first_again), Some(changed));
    assert_eq!(rally(&second_exit), Some(b));
}

#[test]
fn retained_exit_query_does_not_rebind_replaced_contain_cache() {
    if !child(concat!(
        module_path!(),
        "::retained_exit_query_does_not_rebind_replaced_contain_cache"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let first = installed("RetainedExitFirst", false);
    let replacement = installed("RetainedExitReplacement", false);
    let old_contain = first.read().unwrap().get_contain().unwrap();
    let new_contain = replacement.read().unwrap().get_contain().unwrap();
    let old = Coord3D::new(10.0, 20.0, 30.0);
    let new = Coord3D::new(40.0, 50.0, 60.0);
    old_contain.lock().unwrap().set_rally_point(old);
    new_contain.lock().unwrap().set_rally_point(new);
    let retained = first.read().unwrap().get_contain_exit_interface().unwrap();
    first
        .write()
        .unwrap()
        .set_contain(Some(Arc::clone(&new_contain)));
    let fresh = first.read().unwrap().get_contain_exit_interface().unwrap();
    assert_eq!(rally(&retained), Some(old));
    assert_eq!(rally(&fresh), Some(new));
    first.write().unwrap().set_contain(None);
    assert!(first.read().unwrap().get_contain_exit_interface().is_none());
    let changed = Coord3D::new(70.0, 80.0, 90.0);
    old_contain.lock().unwrap().set_rally_point(changed);
    assert_eq!(rally(&retained), Some(changed));
    assert_eq!(rally(&fresh), Some(new));
}

#[test]
fn authored_exit_precedes_contain_without_adding_snapshot_state() {
    if !child(concat!(
        module_path!(),
        "::authored_exit_precedes_contain_without_adding_snapshot_state"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let owner = installed("OrderedExitAndContain", true);
    let guard = owner.read().unwrap();
    let entries: Vec<_> = guard.modules.iter().cloned().collect();
    let before = ordered_module_bytes(&guard);
    let mut production_exit = guard.get_object_exit_interface().unwrap();
    let contain_exit = guard.get_contain_exit_interface().unwrap();
    // An unassigned DefaultProductionExit rally is None, while the authored
    // contain runtime has its own explicit rally. CPP must select production.
    let contain = guard.get_contain().unwrap();
    let point = Coord3D::new(4.0, 8.0, 12.0);
    contain.lock().unwrap().set_rally_point(point);
    assert_eq!(rally(&production_exit), None);
    assert_eq!(rally(&contain_exit), Some(point));
    assert_eq!(reserve(&mut production_exit), ExitDoorType::Primary);
    // Compare after the independent contain mutation, so only query and
    // reservation operations are under the serialization-invariance assertion.
    let stable = ordered_module_bytes(&guard);
    let second = guard.get_object_exit_interface().unwrap();
    assert_eq!(rally(&second), None);
    assert_eq!(ordered_module_bytes(&guard), stable);
    assert_eq!(before.len(), stable.len());
    assert!(
        entries
            .iter()
            .zip(&guard.modules)
            .all(|(a, b)| Arc::ptr_eq(a, b))
    );
}

#[test]
fn exit_callback_can_query_the_same_canonical_module_again() {
    if !child(concat!(
        module_path!(),
        "::exit_callback_can_query_the_same_canonical_module_again"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let owner = installed("NestedExitQuery", true);
    let guard = owner.read().unwrap();
    let entry = guard
        .find_module_by_name("DefaultProductionExitUpdate")
        .unwrap();
    let before = ordered_module_bytes(&guard);
    let door = guard.with_object_exit_interface(|exit| {
        assert!(exit.can_exit(ID));
        let door = exit.reserve_door_for_exit(None, None);
        // with_object_exit_interface must not retain the canonical module guard
        // across the user's callback. The original interface did not do so.
        assert!(entry.module.try_lock().is_ok());
        assert_eq!(
            guard.with_object_exit_interface(|same| same.can_exit(ID)),
            Some(true)
        );
        door
    });
    assert_eq!(door, Some(ExitDoorType::Primary));
    assert_eq!(ordered_module_bytes(&guard), before);
}
