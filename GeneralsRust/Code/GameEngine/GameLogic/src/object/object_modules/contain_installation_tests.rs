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
        // Authored fixtures reference defined Upgrade.ini upgrades only.
        for upgrade in [TRIGGER] {
            crate::upgrade::test_upgrade_mask(upgrade);
        }
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
        UpgradeMaskType::from_bits_retain(crate::upgrade::test_upgrade_mask(TRIGGER).to_bits());
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

#[test]
fn authored_transport_admission_uses_borrowed_same_id_owner_without_registry() {
    if !child(concat!(
        module_path!(),
        "::authored_transport_admission_uses_borrowed_same_id_owner_without_registry"
    )) {
        return;
    }
    let first = Installed::new(
        "BorrowedTransportFirst",
        "TransportContain",
        "Slots = 1\n AllowInsideKindOf = INFANTRY",
    );
    let second = Installed::new(
        "BorrowedTransportSecond",
        "TransportContain",
        "Slots = 0\n AllowInsideKindOf = INFANTRY",
    );
    {
        let mut factory = get_thing_factory().unwrap();
        assert_eq!(factory.as_mut().unwrap().load_ini_text("Object BorrowedPassenger\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject BorrowedWrongRider\n KindOf = VEHICLE\n TransportSlotCount = 1\nEnd\n"),2);
    }
    let passenger = Object::new_raw(
        TheThingFactory::find_template("BorrowedPassenger").unwrap(),
        ID + 1,
        ObjectStatusMaskType::NONE,
        None,
    );
    let wrong = Object::new_raw(
        TheThingFactory::find_template("BorrowedWrongRider").unwrap(),
        ID + 2,
        ObjectStatusMaskType::NONE,
        None,
    );
    let mut foreign_team = crate::team::Team::new("ForeignBorrowedRider".into(), 900);
    foreign_team.set_controlling_player_id(Some(42));
    let foreign_team = Arc::new(RwLock::new(foreign_team));
    let foreign = Object::new_raw(
        TheThingFactory::find_template("BorrowedPassenger").unwrap(),
        ID + 3,
        ObjectStatusMaskType::NONE,
        Some(Arc::clone(&foreign_team)),
    );
    assert_eq!(foreign.get_controlling_player_id(), Some(42));
    assert!(
        foreign_team.read().unwrap().get_members().is_empty(),
        "detached construction does not admit membership"
    );
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID + 3)
            .is_none()
    );
    let ambient_team = crate::team::get_team_factory()
        .lock()
        .unwrap()
        .create_team_on_prototype_with_id(
            &crate::team::TeamPrototype::new("AmbientSameId".into()),
            900,
        )
        .unwrap();
    ambient_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(99));
    assert!(Arc::ptr_eq(&foreign.get_team().unwrap(), &foreign_team));
    assert_eq!(
        foreign.get_controlling_player_id(),
        Some(42),
        "ambient same-ID team cannot replace admitted identity"
    );
    let before_first = first.save();
    let before_second = second.save();
    // Holding the actual owner used to force derived TransportContain back
    // through a missing/borrowed ambient object and reject this positive.
    let owner = first.owner.read().unwrap();
    let contain = first.contain.lock().unwrap();
    assert!(contain.is_valid_container_for_with_owner(&passenger, &owner, true));
    assert!(!contain.is_valid_container_for_with_owner(&wrong, &owner, true));
    assert!(!contain.is_valid_container_for_with_owner(&foreign, &owner, false));
    drop(contain);
    drop(owner);
    let owner = second.owner.read().unwrap();
    assert!(
        !second
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&passenger, &owner, true)
    );
    assert!(
        second
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&passenger, &owner, false)
    );
    drop(owner);
    assert_eq!(first.save(), before_first);
    assert_eq!(second.save(), before_second);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn authored_helix_and_rider_admission_use_exact_borrowed_owner() {
    if !child(concat!(
        module_path!(),
        "::authored_helix_and_rider_admission_use_exact_borrowed_owner"
    )) {
        return;
    }
    let helix = Installed::new(
        "OwnedHelixAdmission",
        "HelixContain",
        "Slots = 1\n AllowInsideKindOf = INFANTRY",
    );
    let helix_full = Installed::new(
        "OwnedHelixNoSlots",
        "HelixContain",
        "Slots = 0\n AllowInsideKindOf = INFANTRY",
    );
    let bike = Installed::new(
        "OwnedBikeAdmission",
        "RiderChangeContain",
        "Slots = 0\n AllowInsideKindOf = INFANTRY\n Rider1 = OwnedBikeRider RIDER1 WEAPON_RIDER1 STATUS_RIDER1 RiderCommands SET_NORMAL",
    );
    {
        let mut factory = get_thing_factory().unwrap();
        assert_eq!(factory.as_mut().unwrap().load_ini_text(
            "Object OwnedBikeRider\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject OwnedOtherRider\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject OwnedPortablePayload\n KindOf = PORTABLE_STRUCTURE\n TransportSlotCount = 0\nEnd\n"), 3);
    }
    let rider = Object::new_raw(
        TheThingFactory::find_template("OwnedBikeRider").unwrap(),
        ID + 1,
        ObjectStatusMaskType::NONE,
        None,
    );
    let other = Object::new_raw(
        TheThingFactory::find_template("OwnedOtherRider").unwrap(),
        ID + 2,
        ObjectStatusMaskType::NONE,
        None,
    );
    let portable = Object::new_raw(
        TheThingFactory::find_template("OwnedPortablePayload").unwrap(),
        ID + 3,
        ObjectStatusMaskType::NONE,
        None,
    );
    let mut foreign_team = crate::team::Team::new("ForeignRider".into(), 911);
    foreign_team.set_controlling_player_id(Some(42));
    let foreign = Object::new_raw(
        TheThingFactory::find_template("OwnedBikeRider").unwrap(),
        ID + 4,
        ObjectStatusMaskType::NONE,
        Some(Arc::new(RwLock::new(foreign_team))),
    );
    let before = [helix.save(), helix_full.save(), bike.save()];
    // The outer write borrow makes rediscovery impossible. Admission receives
    // this exact owner through the factory-installed interface instead.
    let owner = helix.owner.write().unwrap();
    assert!(
        helix
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&rider, &owner, true)
    );
    assert!(
        !helix
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&foreign, &owner, false)
    );
    drop(owner);
    let owner = helix_full.owner.write().unwrap();
    assert!(
        !helix_full
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&rider, &owner, true)
    );
    assert!(
        helix_full
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&rider, &owner, false)
    );
    assert!(
        helix_full
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&portable, &owner, true),
        "CPP portable first payload bypasses ordinary transport gates"
    );
    drop(owner);
    let owner = bike.owner.write().unwrap();
    assert!(
        bike.contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&rider, &owner, true),
        "CPP replacement deliberately ignores capacity"
    );
    assert!(
        !bike
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&other, &owner, false)
    );
    assert!(
        !bike
            .contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&foreign, &owner, false)
    );
    drop(owner);
    assert_eq!([helix.save(), helix_full.save(), bike.save()], before);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[test]
fn authored_railed_and_internet_admission_use_borrowed_container() {
    if !child(concat!(
        module_path!(),
        "::authored_railed_and_internet_admission_use_borrowed_container"
    )) {
        return;
    }
    for (index, module) in ["RailedTransportContain", "InternetHackContain"]
        .into_iter()
        .enumerate()
    {
        let owner = Installed::new(
            &format!("BorrowedDerived{index}"),
            module,
            "Slots = 1\n AllowInsideKindOf = INFANTRY",
        );
        let full = Installed::new(
            &format!("BorrowedDerivedFull{index}"),
            module,
            "Slots = 0\n AllowInsideKindOf = INFANTRY",
        );
        let passenger_name = format!("BorrowedDerivedPassenger{index}");
        let wrong_name = format!("BorrowedDerivedWrong{index}");
        {
            let mut factory = get_thing_factory().unwrap();
            assert_eq!(factory.as_mut().unwrap().load_ini_text(&format!("Object {passenger_name}\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject {wrong_name}\n KindOf = VEHICLE\n TransportSlotCount = 1\nEnd\n")), 2);
        }
        let passenger = Object::new_raw(
            TheThingFactory::find_template(&passenger_name).unwrap(),
            ID + 1,
            ObjectStatusMaskType::NONE,
            None,
        );
        let wrong = Object::new_raw(
            TheThingFactory::find_template(&wrong_name).unwrap(),
            ID + 2,
            ObjectStatusMaskType::NONE,
            None,
        );
        let mut foreign_team = crate::team::Team::new("DerivedForeign".into(), 1200);
        foreign_team.set_controlling_player_id_for_owned_members(Some(42));
        let foreign = Object::new_raw(
            TheThingFactory::find_template(&passenger_name).unwrap(),
            ID + 3,
            ObjectStatusMaskType::NONE,
            Some(Arc::new(RwLock::new(foreign_team))),
        );
        let before = [owner.save(), full.save()];
        let guard = owner.owner.write().unwrap();
        assert!(
            owner
                .contain
                .lock()
                .unwrap()
                .is_valid_container_for_with_owner(&passenger, &guard, true)
        );
        assert!(
            !owner
                .contain
                .lock()
                .unwrap()
                .is_valid_container_for_with_owner(&wrong, &guard, false)
        );
        assert!(
            !owner
                .contain
                .lock()
                .unwrap()
                .is_valid_container_for_with_owner(&foreign, &guard, false)
        );
        drop(guard);
        let guard = full.owner.write().unwrap();
        assert!(
            !full
                .contain
                .lock()
                .unwrap()
                .is_valid_container_for_with_owner(&passenger, &guard, true)
        );
        assert!(
            full.contain
                .lock()
                .unwrap()
                .is_valid_container_for_with_owner(&passenger, &guard, false)
        );
        drop(guard);
        assert_eq!([owner.save(), full.save()], before);
    }
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}
