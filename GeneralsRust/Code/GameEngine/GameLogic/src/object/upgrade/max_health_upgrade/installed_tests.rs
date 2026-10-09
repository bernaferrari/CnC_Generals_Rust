//! Canonical installed MaxHealthUpgrade contracts (MaxHealthUpgrade.cpp:18-35,
//! 56-104; UpgradeModule.cpp:140-146,191-201).
use super::*;
use crate::common::{Coord3D, FXListId, FXListManagerInterface, ThingId};
use crate::helpers::TheThingFactory;
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A3C_1011;
const TRIGGER: &str = "Upgrade_OwnedMaxHealthTrigger";
fn mask() -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::test_upgrade_mask(TRIGGER).to_bits())
}
struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
}
impl Installed {
    fn new(name: &str, extra: &str) -> Self {
        Self::new_modules(name, &[("OwnedMaxHealth", extra)])
    }
    fn new_modules(name: &str, modules: &[(&str, &str)]) -> Self {
        // Authored fixtures reference defined Upgrade.ini upgrades only.
        for upgrade in [TRIGGER] {
            crate::upgrade::test_upgrade_mask(upgrade);
        }
        assert!(
            ensure_thing_factory_exists(),
            "empty authored fixture catalog"
        );
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let authored = modules.iter().map(|(tag,fields)| format!(
            " Behavior = MaxHealthUpgrade {tag}\n TriggeredBy = {TRIGGER}\n AddMaxHealth = 50\n {fields}\n End\n"
        )).collect::<String>();
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(&format!("Object {name}\n KindOf = INERT\n{authored}End\n")),
            1
        );
        let template = TheThingFactory::find_template(name).unwrap();
        // Actual authored upgrade factory + existing real ActiveBody fixture.
        // Authored BodyBindingModule cache installation remains an independent contract.
        let object = Arc::new(RwLock::new(Object::new_test_from_template(
            ID,
            100.0,
            template.clone(),
        )));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entry = object
            .read()
            .unwrap()
            .find_module_by_name("MaxHealthUpgrade")
            .unwrap();
        Self { object, entry }
    }
    fn bookkeeping(&self) -> (f32, f32, f32, f32) {
        let body = self.object.read().unwrap().get_body_module().unwrap();
        let body = body.lock().unwrap();
        (
            body.get_health(),
            body.get_max_health(),
            body.get_initial_health(),
            body.get_previous_health(),
        )
    }
    fn initial_percent(&self, percent: i32) {
        let body = self.object.read().unwrap().get_body_module().unwrap();
        body.lock().unwrap().set_initial_health(percent).unwrap();
    }
    fn applied(&self) -> bool {
        self.entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<MaxHealthUpgrade>()
                .unwrap()
                .applied
        })
    }
    fn health(&self) -> (f32, f32) {
        let body = self.object.read().unwrap().get_body_module().unwrap();
        let guard = body.lock().unwrap();
        (guard.get_health(), guard.get_max_health())
    }
    fn apply(&self) {
        self.object.write().unwrap().apply_upgrade_modules(mask());
    }
    fn bytes(&self, crc: bool) -> Vec<u8> {
        let mut data = Vec::new();
        self.entry.with_module(|module| {
            let mut xfer = XferSave::new(Cursor::new(&mut data), 1);
            if crc {
                module.crc(&mut xfer).unwrap();
            } else {
                module.xfer(&mut xfer).unwrap();
            }
        });
        data
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_MAX_HEALTH_OWNER_CHILD"
        ),
        crate::test_process::TestProcess::Child
    )
}
#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

#[test]
fn authored_same_id_owners_apply_reset_and_query_clone_independently() {
    if !child(concat!(
        module_path!(),
        "::authored_same_id_owners_apply_reset_and_query_clone_independently"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedMaxHealthA", "ChangeType = SAME_CURRENTHEALTH");
    let b = Installed::new("OwnedMaxHealthB", "ChangeType = SAME_CURRENTHEALTH");
    assert!(!Arc::ptr_eq(&a.entry, &b.entry));
    assert!(!a.applied() && !b.applied());
    assert_eq!(a.health(), (100.0, 100.0));
    assert_eq!(b.health(), (100.0, 100.0));
    let query = a.object.clone();
    assert!(Arc::ptr_eq(
        &a.entry,
        &query
            .read()
            .unwrap()
            .find_module_by_name("MaxHealthUpgrade")
            .unwrap()
    ));
    a.apply();
    assert_eq!(a.health(), (100.0, 150.0));
    assert_eq!(b.health(), (100.0, 100.0));
    assert!(a.applied() && !b.applied());
    a.apply();
    assert_eq!(a.health(), (100.0, 150.0));
    b.apply();
    assert_eq!(b.health(), (100.0, 150.0));
    query.write().unwrap().remove_upgrade_mask(mask());
    assert!(!a.applied() && b.applied());
    assert_eq!(
        a.health(),
        (100.0, 150.0),
        "CPP reset does not undo health addition"
    );
    a.apply();
    assert_eq!(a.health(), (100.0, 200.0));
    assert_eq!(b.health(), (100.0, 150.0));
}

#[test]
fn installed_crc_and_xfer_are_exact_cpp_v1_and_restore_only_receiver_execution() {
    if !child(concat!(
        module_path!(),
        "::installed_crc_and_xfer_are_exact_cpp_v1_and_restore_only_receiver_execution"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedMaxHealthWireA", "");
    let b = Installed::new("OwnedMaxHealthWireB", "");
    assert_eq!(a.bytes(true), [1, 0]);
    assert_eq!(a.bytes(false), [1, 1, 1, 1, 1, 1, 0]);
    a.apply();
    assert_eq!(a.bytes(true), [1, 1]);
    let saved = a.bytes(false);
    assert_eq!(saved, [1, 1, 1, 1, 1, 1, 1]);
    b.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved), 1))
            .unwrap()
    });
    assert!(a.applied() && b.applied());
    assert_eq!(
        b.health(),
        (100.0, 100.0),
        "module restore does not repeat upgrade implementation"
    );
    b.object.write().unwrap().remove_upgrade_mask(mask());
    assert!(a.applied() && !b.applied());
}

#[derive(Debug)]
struct Effects {
    entry: Arc<ModuleEntry>,
    calls: AtomicUsize,
}
impl FXListManagerInterface for Effects {
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("must borrow actual owner");
    }
    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("must not rediscover owner by ID");
    }
    fn do_fx_for_object(&self, _: FXListId, owner: &Object) {
        assert_eq!(owner.get_id(), ID);
        assert!(
            self.entry.module.try_lock().is_ok(),
            "FX must run outside ModuleEntry guard"
        );
        assert!(
            owner.completed_upgrades().intersects(mask()),
            "FX precedes removal"
        );
        let body = owner.get_body_module().unwrap();
        assert_eq!(
            body.lock().unwrap().get_max_health(),
            100.0,
            "FX precedes implementation"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn installed_self_removal_fx_runs_before_body_and_commits_execution() {
    if !child(concat!(
        module_path!(),
        "::installed_self_removal_fx_runs_before_body_and_commits_execution"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedMaxHealthSelfRemoval",
        &format!("RemovesUpgrades = {TRIGGER}\n FXListUpgrade = OwnedMaxHealthFx"),
    );
    // Actual giveUpgrade invokes CPP updateUpgradeModules, which requires
    // the driving object's real controlling player; the isolated child owns
    // this fixture roster and never resets an unrelated world.
    let has_player = crate::player::player_list()
        .read()
        .unwrap()
        .get_player(0)
        .is_some();
    if !has_player {
        crate::player::player_list()
            .write()
            .unwrap()
            .add_player(Arc::new(RwLock::new(crate::player::Player::new(0))));
    }
    let team = Arc::new(RwLock::new(crate::team::Team::new(
        "OwnedMaxHealthSelfRemovalTeam".into(),
        ID + 5,
    )));
    team.write().unwrap().set_controlling_player_id(Some(0));
    a.object.write().unwrap().set_team(Some(team)).unwrap();
    assert!(a.object.read().unwrap().get_controlling_player().is_some());
    let recorder = Arc::new(Effects {
        entry: a.entry.clone(),
        calls: AtomicUsize::new(0),
    });
    assert!(crate::helpers::register_fx_list_manager(recorder.clone()));
    let allocated = crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER))
    });
    let mut upgrade = (*allocated).clone();
    upgrade.set_upgrade_type(crate::upgrade::UpgradeType::Object);
    assert!(
        !upgrade.mask().is_empty(),
        "actual catalog-allocated upgrade mask"
    );
    a.object.write().unwrap().give_upgrade(&upgrade);
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    assert!(a.applied());
    assert!(
        !a.object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask())
    );
    assert_eq!(a.health(), (100.0, 150.0));
}

#[test]
fn authored_full_heal_change_type_is_accepted_and_preserved() {
    if !child(concat!(
        module_path!(),
        "::authored_full_heal_change_type_is_accepted_and_preserved"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedMaxHealthFullyHeal", "ChangeType = FULLY_HEAL");
    a.entry.with_module(|module| {
        assert_eq!(
            module
                .as_any()
                .downcast_ref::<MaxHealthUpgrade>()
                .unwrap()
                .data
                .change_type(),
            MaxHealthChangeType::FullyHeal
        )
    });
    a.apply();
    assert_eq!(a.health(), (150.0, 150.0));
}

struct Registered(Arc<RwLock<Object>>);
impl Registered {
    fn new(owner: &Arc<RwLock<Object>>) -> Self {
        let id = owner.read().unwrap().get_id();
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(id)
                .is_none()
        );
        crate::object::registry::OBJECT_REGISTRY.register_object(id, owner);
        Self(owner.clone())
    }
}
impl Drop for Registered {
    fn drop(&mut self) {
        let id = self.0.read().unwrap().get_id();
        let actual = crate::object::registry::OBJECT_REGISTRY
            .get_object(id)
            .unwrap();
        assert!(
            Arc::ptr_eq(&actual, &self.0),
            "fixture must retire only its exact registration"
        );
        crate::object::registry::OBJECT_REGISTRY.unregister_object(id);
    }
}
#[test]
fn registered_installed_upgrade_does_not_reborrow_held_owner() {
    if !child(concat!(
        module_path!(),
        "::registered_installed_upgrade_does_not_reborrow_held_owner"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new(
        "OwnedMaxHealthRegistered",
        "ChangeType = SAME_CURRENTHEALTH",
    );
    let _registration = Registered::new(&a.object);
    a.apply();
    assert!(a.applied());
    assert_eq!(a.health(), (100.0, 150.0));
}

#[test]
fn installed_no_body_still_commits_execution_and_reset_only_clears_execution() {
    if !child(concat!(
        module_path!(),
        "::installed_no_body_still_commits_execution_and_reset_only_clears_execution"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedMaxHealthWithoutBody", "");
    a.object.write().unwrap().set_body_module(None);
    assert!(a.object.read().unwrap().get_body_module().is_none());
    assert!(!a.applied());
    a.apply();
    assert!(
        a.applied(),
        "CPP giveSelfUpgrade commits execution after body-null implementation"
    );
    a.object.write().unwrap().remove_upgrade_mask(mask());
    assert!(!a.applied());
}

#[test]
fn authored_change_types_preserve_cpp_current_initial_and_previous_health() {
    if !child(concat!(
        module_path!(),
        "::authored_change_types_preserve_cpp_current_initial_and_previous_health"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    // C++ ActiveBody.cpp:855-865 establishes current=40 and previous=100.
    // CPP873-922 assigns max+initial before each case; internalChangeHealth1188
    // records previous=current only when that virtual method is actually called.
    let cases = [
        ("Same", "SAME_CURRENTHEALTH", (40.0, 150.0, 150.0, 100.0)),
        ("Ratio", "PRESERVE_RATIO", (60.0, 150.0, 150.0, 40.0)),
        ("Add", "ADD_CURRENT_HEALTH_TOO", (90.0, 150.0, 150.0, 40.0)),
        ("Full", "FULLY_HEAL", (150.0, 150.0, 150.0, 40.0)),
    ];
    for (name, change, expected) in cases {
        let a = Installed::new(
            &format!("OwnedMaxHealthType{name}"),
            &format!("ChangeType = {change}"),
        );
        a.initial_percent(40);
        assert_eq!(a.bookkeeping(), (40.0, 100.0, 100.0, 100.0));
        a.apply();
        assert_eq!(a.bookkeeping(), expected, "CPP ChangeType {change}");
        assert!(a.applied());
    }
    let clipped = Installed::new(
        "OwnedMaxHealthTypeClipped",
        "AddMaxHealth = -50\n ChangeType = SAME_CURRENTHEALTH",
    );
    clipped.initial_percent(80);
    assert_eq!(clipped.bookkeeping(), (80.0, 100.0, 100.0, 100.0));
    clipped.apply();
    assert_eq!(
        clipped.bookkeeping(),
        (50.0, 50.0, 50.0, 80.0),
        "CPP final high cap calls internalChangeHealth even SAME_CURRENTHEALTH"
    );
    assert!(clipped.applied());
}

#[test]
fn authored_modules_apply_in_order_to_the_same_canonical_body() {
    if !child(concat!(
        module_path!(),
        "::authored_modules_apply_in_order_to_the_same_canonical_body"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new_modules(
        "OwnedMaxHealthOrdered",
        &[
            ("OwnedHealthFirst", "ChangeType = PRESERVE_RATIO"),
            ("OwnedHealthSecond", "ChangeType = ADD_CURRENT_HEALTH_TOO"),
        ],
    );
    a.initial_percent(40);
    let body = a.object.read().unwrap().get_body_module().unwrap();
    a.apply();
    assert!(Arc::ptr_eq(
        &body,
        &a.object.read().unwrap().get_body_module().unwrap()
    ));
    // Authored first: 40/100 -> 60/150, then +50 current -> 110/200.
    // Reversing the two effects would yield 120/200.
    assert_eq!(a.bookkeeping(), (110.0, 200.0, 200.0, 60.0));
    let entries = a
        .object
        .read()
        .unwrap()
        .modules
        .iter()
        .filter(|entry| {
            entry.tag().as_str() == "OwnedHealthFirst"
                || entry.tag().as_str() == "OwnedHealthSecond"
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(!Arc::ptr_eq(&entries[0], &entries[1]));
    for entry in entries {
        entry.with_module(|module| {
            assert!(
                module
                    .as_any()
                    .downcast_ref::<MaxHealthUpgrade>()
                    .unwrap()
                    .applied
            )
        });
    }
}

#[path = "owner_callback_tests.rs"]
mod owner_callback_tests;
