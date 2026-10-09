//! CPP Object.cpp:2410–2436,4474–4503: grants update the complete mask,
//! preserve construction/destroyed/player guards, and snapshot the key mask
//! once before walking actual installed modules in authored order.
//! Proposed child of object so the existing private install boundary suffices.

use super::*;
use crate::helpers::TheThingFactory;
use crate::player::PlayerArcExt;
use crate::upgrade::{UpgradeStatus, UpgradeTemplate, UpgradeType};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};

const ID: ObjectID = 0x7A3D_1001;
const FIRST: &str = "Upgrade_OwnedAdmissionFirst";
const SECOND: &str = "Upgrade_OwnedAdmissionSecond";

fn upgrade(name: &str, kind: UpgradeType) -> UpgradeTemplate {
    let allocated = crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(name))
    });
    let mut upgrade = (*allocated).clone();
    // Retain the actual catalog's allocated name/key/mask, and declare the
    // intended OBJECT or PLAYER caller contract. Never a zero-mask template.
    upgrade.set_upgrade_type(kind);
    assert!(!upgrade.mask().is_empty());
    upgrade
}

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::test_upgrade_mask(name).to_bits())
}

fn player() -> Arc<RwLock<Player>> {
    if let Some(player) = crate::player::player_list()
        .read()
        .unwrap()
        .get_player(0)
        .cloned()
    {
        return player;
    }
    let player = Arc::new(RwLock::new(Player::new(0)));
    crate::player::player_list()
        .write()
        .unwrap()
        .add_player(player.clone());
    player
}

fn owner(name: &str, modules: &str, controlling_player: bool) -> Arc<RwLock<Object>> {
    assert!(ensure_thing_factory_exists());
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        for name in [FIRST, SECOND] {
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
    let loaded = get_thing_factory()
        .unwrap()
        .as_mut()
        .unwrap()
        .load_ini_text(&format!("Object {name}\n KindOf = INERT\n{modules}End\n"));
    assert_eq!(loaded, 1);
    let template = TheThingFactory::find_template(name)
        .expect("actual authored StatusBitsUpgrade definitions");
    let owner = Arc::new(RwLock::new(Object::new_raw(
        template.clone(),
        ID,
        ObjectStatusMaskType::NONE,
        None,
    )));
    Object::init_modules_for(&owner, template.as_ref()).unwrap();
    if controlling_player {
        player();
        let team = Arc::new(RwLock::new(Team::new(format!("{name}Team").into(), ID + 5)));
        team.write().unwrap().set_controlling_player_id(Some(0));
        owner.write().unwrap().set_team(Some(team)).unwrap();
        assert!(owner.read().unwrap().get_controlling_player().is_some());
    }
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    owner
}

fn status(owner: &Arc<RwLock<Object>>, flag: ObjectStatusMaskType) -> bool {
    owner.read().unwrap().get_status_bits().contains(flag)
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_UPGRADE_ADMISSION_CHILD"
        ),
        crate::test_process::TestProcess::Child
    )
}
#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

fn both_modules() -> String {
    format!(
        " Behavior = StatusBitsUpgrade RequiresBoth\n TriggeredBy = {FIRST} {SECOND}\n RequiresAllTriggers = Yes\n StatusToSet = MASKED\n End\n"
    )
}

#[test]
fn give_upgrade_combines_two_completed_object_bits() {
    if !child(concat!(
        module_path!(),
        "::give_upgrade_combines_two_completed_object_bits"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = owner("OwnedAdmissionBothObject", &both_modules(), true);
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(SECOND, UpgradeType::Object));
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "second grant must see both completed OBJECT bits"
    );
    assert!(
        owner
            .read()
            .unwrap()
            .completed_upgrades()
            .contains(mask(FIRST) | mask(SECOND))
    );
}

#[test]
fn give_upgrade_combines_completed_player_and_object_bits_without_mixing_authority() {
    if !child(concat!(
        module_path!(),
        "::give_upgrade_combines_completed_player_and_object_bits_without_mixing_authority"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let player = player();
    let first = upgrade(FIRST, UpgradeType::Player);
    player.add_upgrade(&first, UpgradeStatus::Complete, None);
    let owner = owner("OwnedAdmissionPlayerObject", &both_modules(), true);
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(SECOND, UpgradeType::Object));
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "grant combines completed player and object masks"
    );
    assert_eq!(
        owner.read().unwrap().completed_upgrades(),
        mask(SECOND),
        "PLAYER bit must not enter object completed ledger"
    );
}

#[test]
fn construction_guard_preserves_grants_and_repeated_grant_rechecks_completion() {
    if !child(concat!(
        module_path!(),
        "::construction_guard_preserves_grants_and_repeated_grant_rechecks_completion"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = owner(
        "OwnedAdmissionConstruction",
        &format!(
            " Behavior = StatusBitsUpgrade Deferred\n TriggeredBy = {FIRST}\n StatusToSet = MASKED\n End\n"
        ),
        true,
    );
    owner
        .write()
        .unwrap()
        .set_status(ObjectStatusMaskType::UNDER_CONSTRUCTION, true);
    let first = upgrade(FIRST, UpgradeType::Object);
    owner.write().unwrap().give_upgrade(&first);
    assert!(
        owner
            .read()
            .unwrap()
            .completed_upgrades()
            .contains(mask(FIRST)),
        "guard preserves granted bit"
    );
    assert!(
        !status(&owner, ObjectStatusMaskType::MASKED),
        "no module runs under construction"
    );
    owner
        .write()
        .unwrap()
        .clear_status(ObjectStatusMaskType::UNDER_CONSTRUCTION);
    // CPP giveUpgrade always re-checks; it does not reject an already-set
    // non-stackable bit. This is a grant contract, not completeConstruction evidence.
    owner.write().unwrap().give_upgrade(&first);
    assert!(status(&owner, ObjectStatusMaskType::MASKED));
}

#[test]
fn destroyed_and_missing_player_skip_implementation_but_retain_object_bits() {
    if !child(concat!(
        module_path!(),
        "::destroyed_and_missing_player_skip_implementation_but_retain_object_bits"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let modules = format!(
        " Behavior = StatusBitsUpgrade Guarded\n TriggeredBy = {FIRST}\n StatusToSet = MASKED\n End\n"
    );
    let destroyed = owner("OwnedAdmissionDestroyed", &modules, true);
    destroyed
        .write()
        .unwrap()
        .set_status(ObjectStatusMaskType::DESTROYED, true);
    let no_player = owner("OwnedAdmissionNoPlayer", &modules, false);
    assert!(no_player.read().unwrap().get_controlling_player().is_none());
    for owner in [&destroyed, &no_player] {
        owner
            .write()
            .unwrap()
            .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
        assert!(
            owner
                .read()
                .unwrap()
                .completed_upgrades()
                .contains(mask(FIRST))
        );
        assert!(!status(owner, ObjectStatusMaskType::MASKED));
    }
}

#[test]
fn removals_preserve_original_combined_key_for_later_authored_modules() {
    if !child(concat!(
        module_path!(),
        "::removals_preserve_original_combined_key_for_later_authored_modules"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let modules = format!(
        " Behavior = StatusBitsUpgrade FirstRemoves\n TriggeredBy = {SECOND}\n RemovesUpgrades = {FIRST}\n StatusToClear = MASKED\n End\n Behavior = StatusBitsUpgrade SecondSeesOriginal\n TriggeredBy = {FIRST} {SECOND}\n RequiresAllTriggers = Yes\n StatusToSet = MASKED\n End\n"
    );
    let owner = owner("OwnedAdmissionOrderedRemoval", &modules, true);
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(SECOND, UpgradeType::Object));
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "later module receives original key even after earlier RemovesUpgrades"
    );
    assert_eq!(
        owner.read().unwrap().completed_upgrades(),
        mask(SECOND),
        "first module removed FIRST before the second implementation"
    );
}

// Append to object_upgrade_admission_tests. Uses its actual authored owner/player helpers.
// C++ Object.cpp:4474–4484 checks only a nonnull template before rechecking.
// UpgradeCenter normally allocates nonzero masks; this tests the public nonnull
// zero-mask constructor boundary, not an authored zero-mask catalog upgrade.
#[test]
fn nonnull_zero_mask_grant_rechecks_completed_player_upgrade() {
    if !child(concat!(
        module_path!(),
        "::nonnull_zero_mask_grant_rechecks_completed_player_upgrade"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let player = player();
    let first = upgrade(FIRST, UpgradeType::Player);
    player.add_upgrade(&first, UpgradeStatus::Complete, None);
    let owner = owner(
        "OwnedAdmissionZeroMaskRecheck",
        &format!(
            " Behavior = StatusBitsUpgrade ExistingPlayerUpgrade\n TriggeredBy = {FIRST}\n StatusToSet = MASKED\n End\n"
        ),
        true,
    );
    // Actual factory install occurred before set_team supplied this player;
    // no extra admission/recheck is injected into the fixture.
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    assert!(owner.read().unwrap().completed_upgrades().is_empty());
    assert!(
        player
            .read()
            .unwrap()
            .get_completed_upgrade_mask()
            .contains(mask(FIRST))
    );
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("StatusBitsUpgrade")
        .expect("actual canonical installed StatusBitsUpgrade");
    let mut crc_before = Vec::new();
    entry.with_module(|module| {
        module
            .crc(&mut game_engine::common::system::xfer_save::XferSave::new(
                std::io::Cursor::new(&mut crc_before),
                1,
            ))
            .unwrap();
    });
    assert_eq!(crc_before, vec![1, 0]);
    let zero = UpgradeTemplate::new(AsciiString::from("UnlinkedZeroMaskBoundary"));
    assert!(zero.mask().is_empty());
    // OLD with the admission packet's retained early return leaves MASKED off.
    // GREEN inserts no object bit then rechecks the real completed PLAYER mask.
    owner.write().unwrap().give_upgrade(&zero);
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "a nonnull empty grant still rechecks all completed masks like CPP"
    );
    assert!(
        owner.read().unwrap().completed_upgrades().is_empty(),
        "PLAYER state must not enter object completed ledger"
    );
    let mut crc_after = Vec::new();
    entry.with_module(|module| {
        module
            .crc(&mut game_engine::common::system::xfer_save::XferSave::new(
                std::io::Cursor::new(&mut crc_after),
                1,
            ))
            .unwrap();
    });
    assert_eq!(crc_after, vec![1, 1]);
}
