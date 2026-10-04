//! Installed WeaponBonusUpgrade contracts from WeaponBonusUpgrade.cpp:49–110,
//! UpgradeModule.cpp:105–146,191–230, Object.cpp:4650–4659, and Weapon.cpp:1935–1976.
//! Upgrade modules use actual authored factory installs. The optional weapon
//! timing fixture attaches canonical weapon definitions explicitly; it does not
//! prove retail Weapon INI, world scheduling, inherited container bonuses, or firing.

use super::*;
use crate::common::{
    AsciiString, Coord3D, FXListId, FXListManagerInterface, ObjectStatusMaskType, ThingId,
    WeaponBonusConditionFlags as CommonBonusFlags,
};
use crate::helpers::TheThingFactory;
use crate::object::{ModuleEntry, Object};
use crate::weapon::{WeaponSlotType, WeaponStatus, WeaponTemplate, WeaponTemplateSet};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A4B_0001;
const TRIGGER: &str = "Upgrade_OwnedWeaponBonusTrigger";
const SECOND: &str = "Upgrade_OwnedWeaponBonusSecond";
const CONFLICT: &str = "Upgrade_OwnedWeaponBonusConflict";
const UNRELATED: &str = "Upgrade_OwnedWeaponBonusUnrelated";

fn mask(name: &str) -> UpgradeMaskType {
    crate::upgrade::center::with_upgrade_center(|center| {
        let definition = center
            .find_upgrade(name)
            .expect("fixture declared actual upgrade");
        UpgradeMaskType::from_bits_retain(definition.mask().bits())
    })
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
}

impl Installed {
    fn new(name: &str, fields: &str) -> Self {
        Self::with_activation(name, TRIGGER, fields)
    }

    fn with_activation(name: &str, activation: &str, fields: &str) -> Self {
        assert!(ensure_thing_factory_exists());
        // Register real masks before eligibility parsing; no fallback allocation.
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
        assert_eq!(
            get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
                "Object {name}\n KindOf = INERT\n Behavior = WeaponBonusUpgrade OwnedWeaponBonus\n TriggeredBy = {activation}\n {fields}\n End\nEnd\n"
            )),
            1
        );
        let template = TheThingFactory::find_template(name).expect("authored bonus definition");
        let object = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entry = {
            let owner = object.read().unwrap();
            let entry = owner
                .modules
                .iter()
                .find(|entry| entry.tag().as_str() == "OwnedWeaponBonus")
                .expect("exact authored bonus tag")
                .clone();
            entry.with_module(|module| assert!(module.as_any().is::<WeaponBonusUpgrade>()));
            entry
        };
        Self { object, entry }
    }

    fn applied(&self) -> bool {
        self.entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<WeaponBonusUpgrade>()
                .unwrap()
                .applied
        })
    }

    fn flags(&self) -> CommonBonusFlags {
        self.object.read().unwrap().get_weapon_bonus_condition()
    }

    fn apply(&self, key: UpgradeMaskType) {
        self.object.write().unwrap().apply_upgrade_modules(key);
    }

    fn bytes(&self, crc: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
            if crc {
                module.crc(&mut xfer).unwrap();
            } else {
                module.xfer(&mut xfer).unwrap();
            }
        });
        bytes
    }

    fn player(&self) {
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
            AsciiString::from("OwnedWeaponBonusTeam"),
            ID + 1,
        )));
        team.write().unwrap().set_controlling_player_id(Some(0));
        self.object.write().unwrap().set_team(Some(team)).unwrap();
    }

    fn give(&self) {
        let definition = crate::upgrade::center::with_upgrade_center(|center| {
            center.find_upgrade(TRIGGER).unwrap()
        });
        self.object.write().unwrap().give_upgrade(&definition);
    }

    fn weapons(&self, shared: bool) {
        // Explicit canonical weapon fixtures. Upgrade install above remains authored.
        // Per-template extra bonuses avoid adding a mutable GameLogic/global driver.
        // The existing accessor initializes a default GameLogic and returns
        // its bonus set. Verify its actual contribution is neutral rather
        // than assuming the optional accessor returns None.
        let mut global_bonus = crate::weapon::WeaponBonus::new();
        let mut flags = crate::weapon::WeaponBonusConditionFlags::new();
        flags.set(crate::weapon::WeaponBonusConditionType::PlayerUpgrade);
        crate::helpers::TheGameLogic::get_global_weapon_bonus_set()
            .expect("default global bonus set")
            .append_bonuses(flags, &mut global_bonus);
        assert_eq!(
            global_bonus.get_field(crate::weapon::WeaponBonusField::RateOfFire),
            1.0
        );
        let mut extra = crate::weapon::WeaponBonusSet::new();
        let mut bonus = crate::weapon::WeaponBonus::new();
        bonus.set_field(crate::weapon::WeaponBonusField::RateOfFire, 2.0);
        extra.set_bonus(
            crate::weapon::WeaponBonusConditionType::PlayerUpgrade,
            bonus,
        );
        let mut set = WeaponTemplateSet::new();
        set.is_reload_time_shared = shared;
        for (slot, name, shot_delay, clip_delay) in [
            (WeaponSlotType::Primary, "OwnedBonusPrimary", 20, 80),
            (WeaponSlotType::Secondary, "OwnedBonusSecondary", 30, 120),
        ] {
            let mut template = WeaponTemplate::new(name.into());
            template.clip_size = 5;
            template.min_delay_between_shots = shot_delay;
            template.max_delay_between_shots = shot_delay;
            template.clip_reload_time = clip_delay;
            template.extra_bonus = Some(extra.clone());
            set.set_weapon_template(slot, Arc::new(template));
        }
        let mut owner = self.object.write().unwrap();
        owner.weapon_set.add_weapon_template_set(set);
        owner.refresh_weapon_set().unwrap();
        owner
            .get_weapon_in_slot_mut(WeaponSlotType::Primary)
            .unwrap()
            .set_status(WeaponStatus::BetweenFiringShots);
        owner
            .get_weapon_in_slot_mut(WeaponSlotType::Secondary)
            .unwrap()
            .set_status(WeaponStatus::ReloadingClip);
        for slot in [WeaponSlotType::Primary, WeaponSlotType::Secondary] {
            owner
                .get_weapon_in_slot_mut(slot)
                .unwrap()
                .set_possible_next_shot_frame(900);
        }
    }

    fn next(&self, slot: WeaponSlotType) -> (WeaponStatus, u32) {
        let owner = self.object.read().unwrap();
        let weapon = owner.get_weapon_in_slot(slot).unwrap();
        (weapon.get_status(), weapon.get_possible_next_shot_frame())
    }
}

struct Registered;
impl Registered {
    fn new(object: &Arc<RwLock<Object>>) -> Self {
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .is_none()
        );
        crate::object::registry::OBJECT_REGISTRY.register_object(ID, object);
        assert!(Arc::ptr_eq(
            &crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .unwrap(),
            object
        ));
        Self
    }
}
impl Drop for Registered {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
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
        panic!("bonus FX must receive borrowed owner");
    }
    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("bonus FX must not rediscover owner by ID");
    }
    fn do_fx_for_object(&self, _: FXListId, owner: &Object) {
        assert_eq!(owner.get_id(), ID);
        assert!(
            self.entry.module.try_lock().is_ok(),
            "release exact installed entry before FX"
        );
        assert!(Arc::ptr_eq(
            &self.entry,
            &owner.find_module_by_name("WeaponBonusUpgrade").unwrap()
        ));
        self.entry.with_module(|module| {
            assert!(
                !module
                    .as_any()
                    .downcast_ref::<WeaponBonusUpgrade>()
                    .unwrap()
                    .applied,
                "CPP commits execution after implementation"
            );
        });
        assert!(
            !owner
                .get_weapon_bonus_condition()
                .contains(CommonBonusFlags::PLAYER_UPGRADE),
            "CPP FX precedes weapon bonus implementation"
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
        entry: fixture.entry.clone(),
        calls: AtomicUsize::new(0),
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
            "GENERALS_WEAPON_BONUS_OWNED_CHILD",
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
    let a = Installed::new("OwnedWeaponBonusA", "");
    let b = Installed::new("OwnedWeaponBonusB", "");
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(!Arc::ptr_eq(&a.entry, &b.entry));
    assert!(!a.applied() && !b.applied());
    assert!(a.flags().is_empty() && b.flags().is_empty());
    b.object
        .write()
        .unwrap()
        .set_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
    let query = Arc::clone(&a.object);
    assert!(Arc::ptr_eq(
        &query
            .read()
            .unwrap()
            .find_module_by_name("WeaponBonusUpgrade")
            .unwrap(),
        &a.entry
    ));
    a.apply(mask(TRIGGER));
    assert!(a.applied() && !b.applied());
    assert_eq!(a.flags(), CommonBonusFlags::PLAYER_UPGRADE);
    assert_eq!(b.flags(), CommonBonusFlags::VETERAN);
    b.apply(mask(TRIGGER));
    assert!(a.applied() && b.applied());
    assert_eq!(
        b.flags(),
        CommonBonusFlags::VETERAN | CommonBonusFlags::PLAYER_UPGRADE
    );
    query.write().unwrap().remove_upgrade_mask(mask(UNRELATED));
    assert!(a.applied() && b.applied());
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied() && b.applied());
    assert_eq!(
        a.flags(),
        CommonBonusFlags::PLAYER_UPGRADE,
        "CPP reset has no inverse flag effect"
    );
    a.apply(mask(TRIGGER));
    assert!(a.applied() && b.applied());
    assert_eq!(a.flags(), CommonBonusFlags::PLAYER_UPGRADE);
    assert_eq!(
        b.flags(),
        CommonBonusFlags::VETERAN | CommonBonusFlags::PLAYER_UPGRADE
    );
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
    let fixture = Installed::new("OwnedWeaponBonusRegistered", "");
    fixture.player();
    let _registered = Registered::new(&fixture.object);
    let definition =
        crate::upgrade::center::with_upgrade_center(|center| center.find_upgrade(TRIGGER).unwrap());
    let mut owner = fixture.object.write().unwrap();
    assert!(!owner.is_under_construction() && !owner.is_destroyed());
    assert!(owner.get_controlling_player().is_some());
    // Real registry admission only. This is not full GameLogic world admission.
    eprintln!("entered registered owner-held WeaponBonusUpgrade grant");
    owner.give_upgrade(&definition);
    assert!(owner.completed_upgrades().intersects(mask(TRIGGER)));
    assert!(
        owner
            .get_weapon_bonus_condition()
            .contains(CommonBonusFlags::PLAYER_UPGRADE)
    );
    drop(owner);
    assert!(fixture.applied());
}

#[test]
fn registered_same_id_decoy_never_receives_driving_bonus() {
    if !child(concat!(
        module_path!(),
        "::registered_same_id_decoy_never_receives_driving_bonus"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let driving = Installed::new("OwnedWeaponBonusDriving", "");
    let decoy = Installed::new("OwnedWeaponBonusDecoy", "");
    driving
        .object
        .write()
        .unwrap()
        .set_weapon_bonus_condition(WeaponBonusConditionType::Veteran);
    decoy
        .object
        .write()
        .unwrap()
        .set_weapon_bonus_condition(WeaponBonusConditionType::Elite);
    let _registered = Registered::new(&decoy.object);
    driving.apply(mask(TRIGGER));
    assert!(driving.applied() && !decoy.applied());
    assert_eq!(
        driving.flags(),
        CommonBonusFlags::VETERAN | CommonBonusFlags::PLAYER_UPGRADE
    );
    assert_eq!(
        decoy.flags(),
        CommonBonusFlags::ELITE,
        "same numeric ID cannot select sibling owner"
    );
    let query = Arc::clone(&driving.object);
    query.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!driving.applied() && !decoy.applied());
    assert!(driving.flags().contains(CommonBonusFlags::PLAYER_UPGRADE));
    driving.apply(mask(TRIGGER));
    assert!(driving.applied() && !decoy.applied());
    assert_eq!(decoy.flags(), CommonBonusFlags::ELITE);
}

#[test]
fn self_removal_fx_and_bonus_preserve_cpp_order_outside_module_guard() {
    if !child(concat!(
        module_path!(),
        "::self_removal_fx_and_bonus_preserve_cpp_order_outside_module_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new(
        "OwnedWeaponBonusSelfRemoval",
        &format!("RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedWeaponBonus"),
    );
    fixture.player();
    let recorder = effects(&fixture, true);
    fixture.give();
    assert!(fixture.applied());
    assert_eq!(fixture.flags(), CommonBonusFlags::PLAYER_UPGRADE);
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
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.flags(), CommonBonusFlags::PLAYER_UPGRADE);
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
        "OwnedWeaponBonusSave",
        &format!("{TRIGGER} {SECOND}"),
        &format!(
            "RequiresAllTriggers = Yes\n ConflictsWith = {CONFLICT}\n FXListUpgrade = FX_OwnedWeaponBonus"
        ),
    );
    let b = Installed::new("OwnedWeaponBonusRestore", "");
    let recorder = effects(&a, false);
    assert_eq!(a.bytes(true), vec![1, 0]);
    assert_eq!(a.bytes(false), vec![1, 1, 1, 1, 1, 1, 0]);
    a.apply(mask(TRIGGER));
    assert!(!a.applied());
    a.apply(mask(TRIGGER) | mask(SECOND) | mask(CONFLICT));
    assert!(!a.applied());
    assert!(a.flags().is_empty());
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 0);
    a.apply(mask(TRIGGER) | mask(SECOND));
    assert!(a.applied());
    assert_eq!(a.flags(), CommonBonusFlags::PLAYER_UPGRADE);
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
    assert_eq!(a.bytes(true), vec![1, 1]);
    let bytes = a.bytes(false);
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    b.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
        module.load_post_process().unwrap();
    });
    assert!(a.applied() && b.applied());
    assert!(
        b.flags().is_empty(),
        "module restore does not replay owner flag implementation"
    );
    assert_eq!(b.bytes(true), vec![1, 1]);
    b.apply(mask(TRIGGER));
    assert!(b.flags().is_empty());
    a.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied() && b.applied());
    assert_eq!(a.flags(), CommonBonusFlags::PLAYER_UPGRADE);
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!b.applied());
    b.apply(mask(TRIGGER));
    assert!(b.applied());
    assert_eq!(b.flags(), CommonBonusFlags::PLAYER_UPGRADE);
    assert_eq!(recorder.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn bonus_change_notifies_exact_owner_weapons_once_and_reset_keeps_flag() {
    if !child(concat!(
        module_path!(),
        "::bonus_change_notifies_exact_owner_weapons_once_and_reset_keeps_flag"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedWeaponBonusWeaponA", "");
    let b = Installed::new("OwnedWeaponBonusWeaponB", "");
    a.weapons(false);
    b.weapons(false);
    let frame = crate::helpers::TheGameLogic::get_frame();
    a.apply(mask(TRIGGER));
    assert_eq!(
        a.next(WeaponSlotType::Primary),
        (WeaponStatus::BetweenFiringShots, frame + 10)
    );
    assert_eq!(
        a.next(WeaponSlotType::Secondary),
        (WeaponStatus::ReloadingClip, frame + 60)
    );
    assert_eq!(
        b.next(WeaponSlotType::Primary),
        (WeaponStatus::BetweenFiringShots, 900)
    );
    assert_eq!(
        b.next(WeaponSlotType::Secondary),
        (WeaponStatus::ReloadingClip, 900)
    );
    {
        let mut owner = a.object.write().unwrap();
        owner
            .get_weapon_in_slot_mut(WeaponSlotType::Primary)
            .unwrap()
            .set_possible_next_shot_frame(777);
    }
    a.apply(mask(TRIGGER));
    assert_eq!(
        a.next(WeaponSlotType::Primary).1,
        777,
        "executed module cannot notify again"
    );
    a.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!a.applied());
    assert!(a.flags().contains(CommonBonusFlags::PLAYER_UPGRADE));
    a.apply(mask(TRIGGER));
    assert!(a.applied());
    assert_eq!(
        a.next(WeaponSlotType::Primary).1,
        777,
        "same already-set bonus has no change notification"
    );
}

#[test]
fn shared_reload_notifications_keep_cpp_slot_order() {
    if !child(concat!(
        module_path!(),
        "::shared_reload_notifications_keep_cpp_slot_order"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let fixture = Installed::new("OwnedWeaponBonusShared", "");
    fixture.weapons(true);
    let frame = crate::helpers::TheGameLogic::get_frame();
    fixture.apply(mask(TRIGGER));
    // Primary BETWEEN uses20/2 then copies RELOADING onto secondary. Secondary
    // now uses120/2 and copies that frame/status to both, exactly CPP1961–1972.
    assert_eq!(
        fixture.next(WeaponSlotType::Primary),
        (WeaponStatus::ReloadingClip, frame + 60)
    );
    assert_eq!(
        fixture.next(WeaponSlotType::Secondary),
        (WeaponStatus::ReloadingClip, frame + 60)
    );
    assert!(fixture.applied());
}
