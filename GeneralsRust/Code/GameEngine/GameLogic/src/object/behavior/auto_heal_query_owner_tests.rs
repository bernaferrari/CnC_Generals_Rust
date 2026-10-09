//! Real Common Object INI -> registered AutoHeal module -> exact Object entry.
//! The local resolver failure injection runs the same private cache operation
//! on that installed runtime; normal eligibility/grants use Object dispatch.

use super::*;
use crate::helpers::TheThingFactory;
use crate::object::{ModuleEntry, Object};
use crate::upgrade::{UpgradeTemplate, UpgradeType};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

const ID: ObjectID = 0x7A3E_2001;
const A: &str = "Upgrade_HealQueryA";
const B: &str = "Upgrade_HealQueryB";
const C: &str = "Upgrade_HealQueryConflict";
const D: &str = "Upgrade_HealQueryOther";
const TAG: &str = "AuthoredHealQuery";

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_AUTOHEAL_QUERY_CHILD",
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

fn upgrade(name: &str) -> UpgradeTemplate {
    let allocated = crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(name))
    });
    let mut template = (*allocated).clone();
    template.set_upgrade_type(UpgradeType::Object);
    assert!(!template.mask().is_empty());
    template
}

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::test_upgrade_mask(name).bits())
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
}

impl Installed {
    fn new(name: &str, triggers: &str, conflict: &str, requires_all: bool) -> Self {
        for name in [A, B, C, D] {
            let _ = upgrade(name);
        }
        assert!(ensure_thing_factory_exists());
        if game_engine::common::thing::module_factory::get_module_factory()
            .unwrap()
            .is_none()
        {
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let ini = format!(
            "Object {name}\n KindOf = INERT\n Behavior = AutoHealBehavior {TAG}\n StartsActive = No\n HealingAmount = 7\n HealingDelay = 1000\n TriggeredBy = {triggers}\n ConflictsWith = {conflict}\n RequiresAllTriggers = {}\n End\nEnd\n",
            if requires_all { "Yes" } else { "No" },
        );
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(&ini),
            1,
        );
        let template = TheThingFactory::find_template(name).expect("authored Common template");
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
            .modules
            .iter()
            .find(|entry| entry.tag().as_str() == TAG)
            .expect("actual authored AutoHeal entry")
            .clone();
        entry.with_module(|module| {
            let module = module
                .as_any_mut()
                .downcast_mut::<AutoHealBehaviorModule>()
                .expect("registered concrete AutoHeal factory");
            assert_eq!(module.behavior().module_data.healing_amount, 7);
            assert_eq!(module.behavior().module_data.healing_delay, 30);
            assert_eq!(module.behavior().module_data.initially_active, false);
            assert_eq!(
                module.behavior().upgrade_mask_cache_for_test(),
                ("unresolved", None)
            );
            assert!(!module.behavior().upgrade_executed);
        });
        if crate::player::player_list()
            .read()
            .unwrap()
            .get_player(0)
            .is_none()
        {
            crate::player::player_list()
                .write()
                .unwrap()
                .add_player(Arc::new(RwLock::new(crate::player::Player::new(0))));
        }
        let team = Arc::new(RwLock::new(crate::team::Team::new(
            format!("{name}Team").into(),
            ID + 5,
        )));
        team.write().unwrap().set_controlling_player_id(Some(0));
        object.write().unwrap().set_team(Some(team)).unwrap();
        assert!(object.read().unwrap().get_controlling_player().is_some());
        assert!(
            OBJECT_REGISTRY.get_object(ID).is_none(),
            "no fake owner admission"
        );
        Self { object, entry }
    }

    fn with_heal<R>(&self, run: impl FnOnce(&mut AutoHealBehavior) -> R) -> R {
        self.entry.with_module(|module| {
            let module = module
                .as_any_mut()
                .downcast_mut::<AutoHealBehaviorModule>()
                .unwrap();
            run(module.behavior_mut())
        })
    }

    fn cache(&self) -> (&'static str, Option<(UpgradeMaskType, UpgradeMaskType)>) {
        self.with_heal(|heal| heal.upgrade_mask_cache_for_test())
    }

    fn executed(&self) -> bool {
        self.with_heal(|heal| heal.upgrade_executed)
    }

    fn affected(&self, name: &str) -> bool {
        self.object
            .read()
            .unwrap()
            .affected_by_upgrade(&upgrade(name))
    }

    fn give(&self, name: &str) {
        self.object.write().unwrap().give_upgrade(&upgrade(name));
    }

    fn bytes(&self, crc: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
            if crc {
                module.crc(&mut save).unwrap();
            } else {
                module.xfer(&mut save).unwrap();
            }
        });
        bytes
    }

    fn load(&self, bytes: &[u8]) {
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
                .unwrap();
            module.load_post_process().unwrap();
        });
    }
}

#[test]
fn authored_same_id_first_query_and_repeat_use_one_runtime_cache() {
    if !child(concat!(
        module_path!(),
        "::authored_same_id_first_query_and_repeat_use_one_runtime_cache"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let first = Installed::new("HealQueryFirst", &format!("{A} {B}"), C, true);
    let first_bytes = first.bytes(false);
    let second = Installed::new("HealQuerySecond", D, C, false);
    assert!(!Arc::ptr_eq(&first.entry, &second.entry));
    assert_eq!(
        first.cache(),
        ("unresolved", None),
        "another ctor does not resolve existing cache"
    );
    assert!(!first.affected(A), "all triggers are required");
    let expected = (mask(A) | mask(B), mask(C));
    assert_eq!(first.cache(), ("ready", Some(expected)));
    assert_eq!(
        first.bytes(false),
        first_bytes,
        "eligibility cache is not save state"
    );
    assert!(
        !first.executed(),
        "eligibility query never applies the upgrade"
    );
    assert_eq!(second.cache(), ("unresolved", None));
    assert!(second.affected(D));
    assert_eq!(second.cache(), ("ready", Some((mask(D), mask(C)))));
    first.with_heal(|heal| {
        let reused =
            heal.compute_upgrade_masks_with(|_| panic!("ready cache must not resolve again"));
        assert_eq!(reused, expected);
    });
    first.give(A);
    assert!(!first.executed());
    first.give(B);
    assert!(
        first.executed(),
        "actual completed OBJECT mask dispatch activates the installed runtime"
    );
    assert!(
        !second.executed(),
        "same-ID sibling runtime remains independent"
    );
    second.give(D);
    assert!(second.executed());
    assert_eq!(first.cache(), ("ready", Some(expected)));
}

#[test]
fn authored_snapshot_transfers_execution_and_timers_without_cache_state() {
    if !child(concat!(
        module_path!(),
        "::authored_snapshot_transfers_execution_and_timers_without_cache_state"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let saved = Installed::new("HealQuerySaved", A, C, false);
    let before_query = saved.bytes(false);
    let crc_before_query = saved.bytes(true);
    assert!(saved.affected(A));
    assert_eq!(saved.bytes(false), before_query);
    assert_eq!(saved.bytes(true), crc_before_query);
    saved.give(A);
    saved.with_heal(|heal| {
        heal.soonest_heal_frame = 81;
    });
    let running_bytes = saved.bytes(false);
    let running_crc = saved.bytes(true);
    let loaded = Installed::new("HealQueryLoaded", A, C, false);
    loaded.load(&running_bytes);
    loaded.with_heal(|heal| {
        assert!(heal.upgrade_executed);
        assert!(!heal.stopped);
        assert_eq!(heal.soonest_heal_frame, 81);
    });
    assert_eq!(loaded.bytes(false), running_bytes);
    assert_eq!(loaded.bytes(true), running_crc);
    assert_eq!(loaded.cache(), ("unresolved", None));
    saved.with_heal(|heal| {
        heal.stop_healing();
    });
    let bytes = saved.bytes(false);
    let crc = saved.bytes(true);
    loaded.load(&bytes);
    loaded.with_heal(|heal| {
        assert!(heal.upgrade_executed);
        assert!(heal.stopped);
        assert_eq!(heal.soonest_heal_frame, NEVER);
        assert_eq!(heal.last_wake_sleep(), Some(UPDATE_SLEEP_FOREVER));
    });
    assert_eq!(
        loaded.cache(),
        ("unresolved", None),
        "cache is not loaded with runtime state"
    );
    assert_eq!(loaded.bytes(false), bytes);
    assert_eq!(loaded.bytes(true), crc);
    loaded.with_heal(|heal| {
        assert_eq!(heal.compute_upgrade_masks(), (mask(A), mask(C)));
    });
    assert_eq!(loaded.cache(), saved.cache());
    assert_eq!(loaded.bytes(false), bytes);
}

#[test]
fn authored_lookup_unwind_freezes_zero_pair_and_is_not_serialized() {
    if !child(concat!(
        module_path!(),
        "::authored_lookup_unwind_freezes_zero_pair_and_is_not_serialized"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let installed = Installed::new("HealQueryUnwind", &format!("{A} {B}"), C, true);
    let bytes = installed.bytes(false);
    let crc = installed.bytes(true);
    let mut resolved_names = Vec::new();
    installed.with_heal(|heal| {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            heal.compute_upgrade_masks_with(|name| {
                resolved_names.push(name.as_str().to_owned());
                if name.as_str() == C {
                    panic!("injected conflict resolver unwind");
                }
                upgrade_mask_for_ascii(name)
            });
        }));
        assert!(outcome.is_err());
        assert_eq!(
            heal.upgrade_mask_cache_for_test(),
            (
                "failed",
                Some((UpgradeMaskType::none(), UpgradeMaskType::none())),
            )
        );
        assert_eq!(
            heal.compute_upgrade_masks_with(|_| panic!("failed cache must not retry")),
            (UpgradeMaskType::none(), UpgradeMaskType::none())
        );
        assert!(!heal.upgrade_executed);
    });
    assert_eq!(
        resolved_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [A, B, C]
    );
    assert!(
        installed.affected(D),
        "preserve the existing zero-activation eligibility policy after unwind"
    );
    assert_eq!(installed.cache().0, "failed");
    assert_eq!(installed.bytes(false), bytes);
    assert_eq!(installed.bytes(true), crc);
    installed.load(&bytes);
    assert_eq!(
        installed.cache().0,
        "failed",
        "load does not clear a runtime-only failure cache"
    );
    let restored = Installed::new("HealQueryUnwindRestore", &format!("{A} {B}"), C, true);
    restored.load(&bytes);
    assert_eq!(
        restored.cache(),
        ("unresolved", None),
        "failure state is not transferred to a fresh runtime"
    );
    assert_eq!(restored.bytes(false), bytes);
}
