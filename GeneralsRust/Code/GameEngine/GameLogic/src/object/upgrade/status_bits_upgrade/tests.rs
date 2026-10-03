//! Actual installed StatusBitsUpgrade contracts from StatusBitsUpgrade.cpp:84–126,
//! UpgradeModule.cpp:140–146,191–201 and Object.cpp:2410–2436,4491–4503.

use super::*;
use crate::common::{Coord3D, FXListId, FXListManagerInterface, ThingId};
use crate::helpers::TheThingFactory;
use crate::object::ModuleEntry;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A3C_0001;
const TRIGGER: &str = "Upgrade_OwnedStatusTrigger";
const UNRELATED: &str = "Upgrade_OwnedStatusUnrelated";

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(name).to_bits())
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entries: Vec<Arc<ModuleEntry>>,
}

impl Installed {
    fn new(name: &str, modules: &[(&str, &str)]) -> Self {
        Self::new_with_id(name, modules, ID)
    }

    fn new_with_id(name: &str, modules: &[(&str, &str)], id: ObjectID) -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let needs_factory = game_engine::common::thing::module_factory::get_module_factory()
            .unwrap()
            .is_none();
        if needs_factory {
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
        }
        // Registration precedes INI parsing, so the authored data uses the
        // canonical implementation rather than a placeholder module.
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let behaviors = modules
            .iter()
            .map(|(tag, fields)| {
                format!(
                    " Behavior = StatusBitsUpgrade {tag}\n TriggeredBy = {TRIGGER}\n {fields}\n End\n"
                )
            })
            .collect::<String>();
        let loaded = get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&format!("Object {name}\n KindOf = INERT\n{behaviors}End\n"));
        assert_eq!(loaded, 1);
        let template = TheThingFactory::find_template(name).expect("authored status definition");
        // Independent, unregistered owners deliberately share an ObjectID.
        // Factory creation and real Object module installation still run.
        let object = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            id,
            ObjectStatusMaskType::NONE,
            None,
        )));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entries = {
            let owner = object.read().unwrap();
            modules
                .iter()
                .map(|(tag, _)| {
                    owner
                        .modules
                        .iter()
                        .find(|entry| entry.tag().as_str() == *tag)
                        .expect("exact authored module tag")
                        .clone()
                })
                .collect()
        };
        Self { object, entries }
    }

    fn applied(&self, index: usize) -> bool {
        self.entries[index].with_module(|module| {
            module
                .as_any()
                .downcast_ref::<StatusBitsUpgrade>()
                .expect("canonical installed StatusBitsUpgrade")
                .applied
        })
    }

    fn apply(&self) {
        let upgrade = crate::upgrade::center::with_upgrade_center_mut(|center| {
            center.new_upgrade(AsciiString::from(TRIGGER))
        });
        self.object.write().unwrap().give_upgrade(&upgrade);
    }

    fn masked(&self) -> bool {
        self.object
            .read()
            .unwrap()
            .get_status_bits()
            .contains(ObjectStatusMaskType::MASKED)
    }

    fn set_masked(&self, value: bool) {
        self.object
            .write()
            .unwrap()
            .set_status(ObjectStatusMaskType::MASKED, value);
    }

    fn bytes(&self, crc: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entries[0].with_module(|module| {
            let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
            if crc {
                module.crc(&mut save).unwrap();
            } else {
                module.xfer(&mut save).unwrap();
            }
        });
        bytes
    }
}

#[derive(Debug)]
struct Effects {
    entry: Arc<ModuleEntry>,
    calls: AtomicUsize,
}

impl FXListManagerInterface for Effects {
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("upgrade FX must use its real object");
    }

    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("upgrade FX must retain the driving object borrow");
    }

    fn do_fx_for_object(&self, _: FXListId, object: &Object) {
        assert_eq!(object.get_id(), ID);
        assert_eq!(
            object.get_template().get_name().as_str(),
            "OwnedStatusSelfRemoval"
        );
        assert!(
            object
                .modules
                .iter()
                .any(|entry| Arc::ptr_eq(entry, &self.entry)),
            "FX sees the exact installed owner"
        );
        assert!(
            self.entry.module.try_lock().is_ok(),
            "FX runs outside the entry guard"
        );
        assert!(
            object.completed_upgrades().intersects(mask(TRIGGER)),
            "FX precedes removal"
        );
        assert!(
            !object
                .get_status_bits()
                .contains(ObjectStatusMaskType::MASKED),
            "FX precedes implementation"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    let name = name.strip_prefix("gamelogic::").unwrap_or(name);
    matches!(
        crate::test_process::run_bounded(name, "GENERALS_STATUSBITS_OWNED_CHILD"),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

#[test]
fn authored_order_and_each_modules_clear_precedence() {
    if !child(concat!(
        module_path!(),
        "::authored_order_and_each_modules_clear_precedence"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let ordered = Installed::new(
        "OwnedStatusOrdered",
        &[
            ("ClearFirst", "StatusToClear = MASKED"),
            ("SetSecond", "StatusToSet = MASKED"),
        ],
    );
    ordered.apply();
    assert!(ordered.applied(0) && ordered.applied(1));
    assert!(
        ordered.masked(),
        "later authored set must win over earlier clear"
    );
    let overlap = Installed::new(
        "OwnedStatusOverlap",
        &[(
            "SetThenClear",
            "StatusToSet = MASKED\n StatusToClear = MASKED",
        )],
    );
    overlap.apply();
    assert!(overlap.applied(0));
    assert!(
        !overlap.masked(),
        "one module sets then clears, so its own clear wins"
    );
}

#[test]
fn unrelated_removal_and_reset_preserve_external_status_writes() {
    if !child(concat!(
        module_path!(),
        "::unrelated_removal_and_reset_preserve_external_status_writes"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let set = Installed::new_with_id(
        "OwnedStatusExternalClear",
        &[("Set", "StatusToSet = MASKED")],
        ID + 1,
    );
    let clear = Installed::new_with_id(
        "OwnedStatusExternalSet",
        &[("Clear", "StatusToClear = MASKED")],
        ID + 2,
    );
    set.apply();
    clear.set_masked(true);
    clear.apply();
    assert!(set.masked() && !clear.masked());
    set.set_masked(false);
    clear.set_masked(true);
    for fixture in [&set, &clear] {
        fixture
            .object
            .write()
            .unwrap()
            .remove_upgrade_mask(mask(UNRELATED));
        assert!(fixture.applied(0), "unrelated reset must retain executed");
    }
    assert!(
        !set.masked(),
        "unrelated removal must not replay a prior set"
    );
    assert!(
        clear.masked(),
        "unrelated removal must not replay a prior clear"
    );
    for fixture in [&set, &clear] {
        fixture
            .object
            .write()
            .unwrap()
            .remove_upgrade_mask(mask(TRIGGER));
        assert!(!fixture.applied(0));
    }
    assert!(!set.masked(), "reset has no inverse status implementation");
    assert!(clear.masked(), "reset has no inverse status implementation");
    set.apply();
    clear.apply();
    assert!(
        set.masked() && !clear.masked(),
        "a new eligible application executes again"
    );
}

#[test]
fn inert_same_id_owners_interleave_and_reset_independently() {
    if !child(concat!(
        module_path!(),
        "::inert_same_id_owners_interleave_and_reset_independently"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedStatusA", &[("Set", "StatusToSet = MASKED")]);
    let b = Installed::new("OwnedStatusB", &[("Clear", "StatusToClear = MASKED")]);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(!a.masked() && !b.masked() && !a.applied(0) && !b.applied(0));
    assert!(!Arc::ptr_eq(&a.entries[0], &b.entries[0]));
    b.set_masked(true);
    a.apply();
    assert!(a.applied(0) && !b.applied(0));
    assert!(
        a.masked() && b.masked(),
        "A must not apply B's same-ID clear"
    );
    b.apply();
    assert!(a.masked() && !b.masked());
    let alias = Arc::clone(&a.object);
    assert!(Arc::ptr_eq(
        &alias.read().unwrap().modules[0],
        &a.entries[0]
    ));
    alias.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied(0) && b.applied(0));
    assert!(
        a.masked() && !b.masked(),
        "reset affects only the driving executed flag"
    );
    a.apply();
    assert!(a.applied(0) && b.applied(0));
    assert!(a.masked() && !b.masked());
}

#[test]
fn self_removal_runs_fx_and_status_outside_the_installed_entry_guard() {
    if !child(concat!(
        module_path!(),
        "::self_removal_runs_fx_and_status_outside_the_installed_entry_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = Installed::new(
        "OwnedStatusSelfRemoval",
        &[(
            "SelfRemoval",
            &format!(
                "StatusToSet = MASKED\n RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedStatus"
            ),
        )],
    );
    let effects = Arc::new(Effects {
        entry: owner.entries[0].clone(),
        calls: AtomicUsize::new(0),
    });
    assert!(crate::helpers::register_fx_list_manager(effects.clone()));
    owner.apply();
    assert!(owner.applied(0) && owner.masked());
    assert!(
        !owner
            .object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(TRIGGER))
    );
    assert_eq!(effects.calls.load(Ordering::SeqCst), 1);
    owner.apply();
    assert_eq!(
        effects.calls.load(Ordering::SeqCst),
        1,
        "executed prevents duplicate FX"
    );
}

#[test]
fn crc_and_xfer_restore_only_execution_without_replaying_status() {
    if !child(concat!(
        module_path!(),
        "::crc_and_xfer_restore_only_execution_without_replaying_status"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedStatusSave", &[("Set", "StatusToSet = MASKED")]);
    let b = Installed::new("OwnedStatusLoad", &[("Clear", "StatusToClear = MASKED")]);
    assert_eq!(a.bytes(true), vec![1, 0]);
    assert_eq!(a.bytes(false), vec![1, 1, 1, 1, 1, 1, 0]);
    a.apply();
    assert_eq!(a.bytes(true), vec![1, 1]);
    let bytes = a.bytes(false);
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    b.set_masked(true);
    b.entries[0].with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
        module.load_post_process().unwrap();
    });
    assert!(a.applied(0) && b.applied(0));
    assert!(
        a.masked() && b.masked(),
        "Xfer/load must not execute destination clear"
    );
    b.object
        .write()
        .unwrap()
        .force_refresh_sub_object_upgrade_status();
    assert!(
        b.masked(),
        "subobject refresh must not replay StatusBitsUpgrade"
    );
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(a.applied(0) && !b.applied(0));
    assert!(a.masked() && b.masked(), "reset has no status undo");
    b.apply();
    assert!(a.masked() && !b.masked());
}

/// Retire only this fixture's admission, including an assertion unwind. The
/// child owns its PlayerList/template catalog until process exit; object and
/// roster lifetimes end here without resetting another world's state.
struct RegisteredOwner {
    object: Arc<RwLock<Object>>,
    player: Arc<RwLock<crate::player::Player>>,
}

impl Drop for RegisteredOwner {
    fn drop(&mut self) {
        let registry = &crate::object::registry::OBJECT_REGISTRY;
        let exact = registry
            .get_object(ID)
            .is_some_and(|admitted| Arc::ptr_eq(&admitted, &self.object));
        if exact {
            // No object or player guard spans registry destruction callbacks.
            if let (Ok(owner), Ok(mut player)) = (self.object.read(), self.player.write()) {
                player.remove_owned_object_for_object(&owner);
            }
            registry.unregister_object(ID);
        }
    }
}

#[test]
fn player_completion_rechecks_the_existing_authored_module_immediately() {
    if !child(concat!(
        module_path!(),
        "::player_completion_rechecks_the_existing_authored_module_immediately"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    use crate::player::PlayerArcExt;

    // CPP Player.cpp:3034–3039,3054–3081 completes the player bit, then
    // immediately asks existing owned objects to check their actual modules.
    let fixture = Installed::new(
        "OwnedStatusPlayerFanout",
        &[("PlayerStatus", "StatusToSet = MASKED")],
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(
        crate::player::player_list()
            .read()
            .unwrap()
            .get_player(0)
            .is_none()
    );
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    crate::player::player_list()
        .write()
        .unwrap()
        .add_player(player.clone());
    let team = Arc::new(RwLock::new(crate::team::Team::new(
        "OwnedStatusPlayerTeam".into(),
        0x00A0_9005,
    )));
    team.write().unwrap().set_controlling_player_id(Some(0));
    // Attach before admission: set_team must not discover and re-lock its
    // already borrowed owner through the registry.
    fixture
        .object
        .write()
        .unwrap()
        .set_team(Some(team))
        .unwrap();
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &fixture.object);
    let admission = RegisteredOwner {
        object: fixture.object.clone(),
        player: player.clone(),
    };
    player.write().unwrap().add_owned_object(ID);
    assert!(player.read().unwrap().get_all_objects().contains(&ID));
    assert!(!fixture.applied(0) && !fixture.masked());
    let upgrade = crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER))
    });
    player.add_upgrade(&upgrade, crate::upgrade::UpgradeStatus::InProduction, None);
    assert!(
        !fixture.applied(0) && !fixture.masked(),
        "in-progress upgrades do not activate modules"
    );
    player.add_upgrade(&upgrade, crate::upgrade::UpgradeStatus::Complete, None);
    assert!(
        fixture.applied(0) && fixture.masked(),
        "completion must fan out before add_upgrade returns"
    );
    assert!(
        !fixture
            .object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(TRIGGER)),
        "player bits stay player-owned"
    );
    drop(admission);
    assert!(!player.read().unwrap().get_all_objects().contains(&ID));
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &fixture.object);
    player.write().unwrap().add_owned_object(ID);
    let unwind_admission = RegisteredOwner {
        object: fixture.object.clone(),
        player: player.clone(),
    };
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _admission = unwind_admission;
        panic!("intentional exact-owner fixture unwind");
    }));
    assert!(unwind.is_err());
    assert!(!player.read().unwrap().get_all_objects().contains(&ID));
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}
