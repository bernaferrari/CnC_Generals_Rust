//! Installed ExperienceScalarUpgrade contracts from ExperienceScalarUpgrade.cpp:17–102,
//! UpgradeModule.cpp:105–146,191–230, and Object.cpp:2410–2436,4491–4503.
//! These are authored Object module installs, including a registered owner;
//! they do not establish independent full GameLogic admission or retail gameplay.

use super::*;
use crate::common::{
    AsciiString, Coord3D, FXListId, FXListManagerInterface, ObjectStatusMaskType, ThingId,
};
use crate::helpers::TheThingFactory;
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A3E_0001;
const TRIGGER: &str = "Upgrade_OwnedExperienceTrigger";
const SECOND: &str = "Upgrade_OwnedExperienceSecond";
const CONFLICT: &str = "Upgrade_OwnedExperienceConflict";
const UNRELATED: &str = "Upgrade_OwnedExperienceUnrelated";

fn mask(name: &str) -> UpgradeMaskType {
    crate::upgrade::center::with_upgrade_center(|center| {
        let definition = center
            .find_upgrade(name)
            .expect("fixture declared the actual upgrade");
        UpgradeMaskType::from_bits_retain(definition.mask().bits())
    })
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entries: Vec<Arc<ModuleEntry>>,
}

impl Installed {
    fn new(name: &str, fields: &str) -> Self {
        Self::modules(name, &[("OwnedExperience", fields)])
    }

    fn modules(name: &str, modules: &[(&str, &str)]) -> Self {
        Self::modules_with_activation(name, modules, TRIGGER)
    }

    fn with_activation(name: &str, activation: &str, fields: &str) -> Self {
        Self::modules_with_activation(name, &[("OwnedExperience", fields)], activation)
    }

    fn modules_with_activation(name: &str, modules: &[(&str, &str)], activation: &str) -> Self {
        assert!(ensure_thing_factory_exists());
        // Real definitions precede INI parsing/eligibility. No fallback mask allocator.
        crate::upgrade::center::with_upgrade_center_mut(|center| {
            for name in [TRIGGER, SECOND, CONFLICT, UNRELATED] {
                center.new_upgrade(AsciiString::from(name));
            }
        });
        if game_engine::common::thing::module_factory::get_module_factory()
            .unwrap()
            .is_none()
        {
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let behaviors = modules.iter().map(|(tag, fields)| {
            format!(" Behavior = ExperienceScalarUpgrade {tag}\n TriggeredBy = {activation}\n {fields}\n End\n")
        }).collect::<String>();
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(&format!("Object {name}\n KindOf = INERT\n{behaviors}End\n")),
            1
        );
        let template =
            TheThingFactory::find_template(name).expect("authored experience definition");
        let object = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entries = {
            let owner = object.read().unwrap();
            modules
                .iter()
                .map(|(tag, _)| {
                    let entry = owner
                        .modules
                        .iter()
                        .find(|entry| entry.tag().as_str() == *tag)
                        .expect("exact authored experience tag")
                        .clone();
                    entry.with_module(|module| {
                        assert!(module.as_any().is::<ExperienceScalarUpgrade>())
                    });
                    entry
                })
                .collect()
        };
        Self { object, entries }
    }

    fn applied(&self, index: usize) -> bool {
        self.entries[index].with_module(|module| {
            module
                .as_any()
                .downcast_ref::<ExperienceScalarUpgrade>()
                .unwrap()
                .applied
        })
    }

    fn scalar(&self) -> Option<Real> {
        self.object
            .read()
            .unwrap()
            .with_experience_tracker(|tracker| tracker.get_experience_scalar())
    }

    fn set_scalar(&self, scalar: Real) {
        assert!(
            self.object
                .write()
                .unwrap()
                .with_experience_tracker_mut(|tracker| {
                    tracker.set_experience_scalar(scalar);
                })
                .is_some()
        );
    }

    fn apply(&self, key: UpgradeMaskType) {
        self.object.write().unwrap().apply_upgrade_modules(key);
    }

    fn bytes(&self, index: usize, crc: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entries[index].with_module(|module| {
            let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
            if crc {
                module.crc(&mut xfer).unwrap();
            } else {
                module.xfer(&mut xfer).unwrap();
            }
        });
        bytes
    }
}

#[derive(Debug)]
struct Effects {
    entry: Arc<ModuleEntry>,
    calls: AtomicUsize,
    before_scalar: Real,
    require_trigger: bool,
}

impl FXListManagerInterface for Effects {
    fn do_fx_for_host_objects(
        &self,
        _: FXListId,
        _: &crate::helpers::HostFxObjectPose,
        _: Option<&crate::helpers::HostFxObjectPose>,
    ) {
        panic!("this upgrade fixture must observe its borrowed Core Object");
    }
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("experience FX must receive the driving Object");
    }
    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("experience FX must not rediscover the owner by ID");
    }
    fn do_fx_for_object(&self, _: FXListId, owner: &Object) {
        assert_eq!(owner.get_id(), ID);
        assert!(
            self.entry.module.try_lock().is_ok(),
            "FX must release the installed entry first"
        );
        assert!(Arc::ptr_eq(
            &self.entry,
            &owner
                .find_module_by_name("ExperienceScalarUpgrade")
                .unwrap()
        ));
        self.entry.with_module(|module| {
            assert!(
                !module
                    .as_any()
                    .downcast_ref::<ExperienceScalarUpgrade>()
                    .unwrap()
                    .applied,
                "CPP marks execution after FX/removals/implementation"
            );
        });
        assert_eq!(
            owner.with_experience_tracker(|tracker| tracker.get_experience_scalar()),
            Some(self.before_scalar),
            "CPP FX precedes additive tracker implementation"
        );
        if self.require_trigger {
            assert!(
                owner.completed_upgrades().intersects(mask(TRIGGER)),
                "CPP FX precedes self-removal"
            );
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

fn effects(fixture: &Installed, require_trigger: bool) -> Arc<Effects> {
    let recorder = Arc::new(Effects {
        entry: fixture.entries[0].clone(),
        calls: AtomicUsize::new(0),
        before_scalar: fixture.scalar().unwrap(),
        require_trigger,
    });
    assert!(crate::helpers::register_fx_list_manager(recorder.clone()));
    recorder
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_EXPERIENCE_OWNED_CHILD",
        ),
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
    let a = Installed::new("OwnedExperienceA", "AddXPScalar = 0.5");
    let b = Installed::new("OwnedExperienceB", "AddXPScalar = 2.0");
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(!Arc::ptr_eq(&a.entries[0], &b.entries[0]));
    assert!(!a.applied(0) && !b.applied(0));
    assert_eq!(a.scalar(), Some(1.0));
    assert_eq!(b.scalar(), Some(1.0));
    b.set_scalar(3.0);
    let query = Arc::clone(&a.object);
    assert!(Arc::ptr_eq(
        &query
            .read()
            .unwrap()
            .find_module_by_name("ExperienceScalarUpgrade")
            .unwrap(),
        &a.entries[0]
    ));
    a.apply(mask(TRIGGER));
    assert!(a.applied(0) && !b.applied(0));
    assert_eq!(a.scalar(), Some(1.5));
    assert_eq!(b.scalar(), Some(3.0));
    b.apply(mask(TRIGGER));
    assert_eq!(b.scalar(), Some(5.0));
    assert_eq!(a.scalar(), Some(1.5));
    a.apply(mask(TRIGGER));
    assert_eq!(a.scalar(), Some(1.5), "executed module must not add twice");
    query.write().unwrap().remove_upgrade_mask(mask(UNRELATED));
    assert!(a.applied(0) && b.applied(0));
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied(0) && b.applied(0));
    assert_eq!(
        a.scalar(),
        Some(1.5),
        "CPP reset does not undo the added scalar"
    );
    a.apply(mask(TRIGGER));
    assert!(a.applied(0) && b.applied(0));
    assert_eq!(a.scalar(), Some(2.0));
    assert_eq!(b.scalar(), Some(5.0));
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn registered_owner_grant_completes_under_existing_object_write_guard() {
    if !child(concat!(
        module_path!(),
        "::registered_owner_grant_completes_under_existing_object_write_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new("OwnedExperienceRegistered", "AddXPScalar = 0.75");
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
        AsciiString::from("OwnedExperienceRegisteredTeam"),
        ID + 1,
    )));
    team.write().unwrap().set_controlling_player_id(Some(0));
    fixture
        .object
        .write()
        .unwrap()
        .set_team(Some(team))
        .unwrap();
    // Only real registry registration is claimed here; full GameLogic admission is separate.
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &fixture.object);
    struct Unregister;
    impl Drop for Unregister {
        fn drop(&mut self) {
            crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
        }
    }
    let _registered = Unregister;
    assert!(Arc::ptr_eq(
        &crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .unwrap(),
        &fixture.object
    ));
    let definition =
        crate::upgrade::center::with_upgrade_center(|center| center.find_upgrade(TRIGGER).unwrap());
    let mut owner = fixture.object.write().unwrap();
    assert!(!owner.is_under_construction() && !owner.is_destroyed());
    assert!(owner.get_controlling_player().is_some());
    eprintln!("entered registered owner-held ExperienceScalarUpgrade grant");
    owner.give_upgrade(&definition);
    assert!(owner.completed_upgrades().intersects(mask(TRIGGER)));
    assert_eq!(
        owner.with_experience_tracker(|tracker| tracker.get_experience_scalar()),
        Some(1.75)
    );
    drop(owner);
    assert!(fixture.applied(0));
}

#[test]
fn no_tracker_still_commits_execution_and_reset() {
    if !child(concat!(
        module_path!(),
        "::no_tracker_still_commits_execution_and_reset"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new("OwnedExperienceNoTracker", "AddXPScalar = 9.0");
    // Exercise CPP's optional tracker branch after the actual factory install.
    fixture.object.write().unwrap().experience_tracker = None;
    fixture.apply(mask(TRIGGER));
    assert!(
        fixture.applied(0),
        "CPP mux commits even if leaf has no tracker"
    );
    assert_eq!(fixture.scalar(), None);
    assert_eq!(fixture.bytes(0, false), vec![1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(fixture.bytes(0, true), vec![1, 1]);
    fixture
        .object
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(TRIGGER));
    assert!(!fixture.applied(0));
    assert_eq!(fixture.scalar(), None);
}

#[test]
fn self_removal_fx_and_scalar_preserve_cpp_order_outside_module_guard() {
    if !child(concat!(
        module_path!(),
        "::self_removal_fx_and_scalar_preserve_cpp_order_outside_module_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new(
        "OwnedExperienceSelfRemoval",
        &format!(
            "AddXPScalar = 0.25\n RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedExperience"
        ),
    );
    let recorder = effects(&fixture, true);
    fixture
        .object
        .write()
        .unwrap()
        .object_upgrades_completed
        .insert(mask(TRIGGER));
    fixture.apply(mask(TRIGGER));
    assert!(fixture.applied(0));
    assert_eq!(fixture.scalar(), Some(1.25));
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
    assert_eq!(fixture.scalar(), Some(1.25));
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn authored_all_triggers_conflicts_and_xfer_restore_only_installed_state() {
    if !child(concat!(
        module_path!(),
        "::authored_all_triggers_conflicts_and_xfer_restore_only_installed_state"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::with_activation(
        "OwnedExperienceSave",
        &format!("{TRIGGER} {SECOND}"),
        &format!(
            "AddXPScalar = 0.5\n RequiresAllTriggers = Yes\n ConflictsWith = {CONFLICT}\n FXListUpgrade = FX_OwnedExperience"
        ),
    );
    let b = Installed::new("OwnedExperienceRestore", "AddXPScalar = 2.0");
    let recorder = effects(&a, false);
    assert_eq!(a.bytes(0, false), vec![1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(a.bytes(0, true), vec![1, 0]);
    a.apply(mask(TRIGGER));
    assert!(!a.applied(0));
    a.apply(mask(TRIGGER) | mask(SECOND) | mask(CONFLICT));
    assert!(!a.applied(0));
    assert_eq!(a.scalar(), Some(1.0));
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 0);
    a.apply(mask(TRIGGER) | mask(SECOND));
    assert!(a.applied(0));
    assert_eq!(a.scalar(), Some(1.5));
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    assert_eq!(a.bytes(0, true), vec![1, 1]);
    let bytes = a.bytes(0, false);
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    b.entries[0].with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
    });
    assert!(a.applied(0) && b.applied(0));
    assert_eq!(
        b.scalar(),
        Some(1.0),
        "module Xfer never replays tracker implementation"
    );
    b.apply(mask(TRIGGER));
    assert_eq!(b.scalar(), Some(1.0));
    a.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied(0) && b.applied(0));
    assert_eq!(a.scalar(), Some(1.5));
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!b.applied(0));
    b.apply(mask(TRIGGER));
    assert!(b.applied(0));
    assert_eq!(b.scalar(), Some(3.0));
    assert_eq!(a.scalar(), Some(1.5));
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn authored_default_negative_and_ordered_additions_do_not_clamp_or_multiply() {
    if !child(concat!(
        module_path!(),
        "::authored_default_negative_and_ordered_additions_do_not_clamp_or_multiply"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let default = Installed::new("OwnedExperienceDefault", "");
    default.apply(mask(TRIGGER));
    assert!(default.applied(0));
    assert_eq!(
        default.scalar(),
        Some(1.0),
        "CPP AddXPScalar defaults to zero"
    );
    let ordered = Installed::modules(
        "OwnedExperienceOrdered",
        &[
            ("AddFirst", "AddXPScalar = 0.5"),
            ("SubtractSecond", "AddXPScalar = -2.0"),
        ],
    );
    ordered.apply(mask(TRIGGER));
    assert!(ordered.applied(0) && ordered.applied(1));
    assert_eq!(
        ordered.scalar(),
        Some(-0.5),
        "CPP adds each scalar without clamping"
    );
}

#[test]
fn registered_same_id_decoy_never_receives_driving_scalar() {
    if !child(concat!(
        module_path!(),
        "::registered_same_id_decoy_never_receives_driving_scalar"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let driving = Installed::new("OwnedExperienceDriving", "AddXPScalar = 0.5");
    let decoy = Installed::new("OwnedExperienceDecoy", "AddXPScalar = 2.0");
    driving.set_scalar(2.0);
    decoy.set_scalar(9.0);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &decoy.object);
    struct Unregister;
    impl Drop for Unregister {
        fn drop(&mut self) {
            crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
        }
    }
    let _registered = Unregister;
    assert!(Arc::ptr_eq(
        &crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .unwrap(),
        &decoy.object,
    ));
    driving.apply(mask(TRIGGER));
    assert!(driving.applied(0) && !decoy.applied(0));
    assert_eq!(
        driving.scalar(),
        Some(2.5),
        "use the caller's tracker despite a registered equal ID"
    );
    assert_eq!(
        decoy.scalar(),
        Some(9.0),
        "never write the registered sibling's tracker"
    );
    let query = Arc::clone(&driving.object);
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!driving.applied(0) && !decoy.applied(0));
    assert_eq!(driving.scalar(), Some(2.5));
    driving.apply(mask(TRIGGER));
    assert_eq!(driving.scalar(), Some(3.0));
    assert_eq!(decoy.scalar(), Some(9.0));
}
