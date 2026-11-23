//! CPP Object.cpp:2410–2436,4474–4503: grants update the complete mask,
//! preserve construction/destroyed/player guards, and snapshot the key mask
//! once before walking actual installed modules in authored order.
//! Proposed child of object so the existing private install boundary suffices.

use super::*;
use crate::helpers::TheThingFactory;
use crate::player::PlayerArcExt;
use crate::upgrade::{UpgradeStatus, UpgradeTemplate, UpgradeType};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use game_engine::common::system::Snapshotable;

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

fn roster_with_players(indices: &[crate::player::PlayerIndex]) -> Arc<RwLock<crate::player::PlayerList>> {
    let mut roster = crate::player::PlayerList::new();
    for &index in indices {
        roster.add_player(Arc::new(RwLock::new(Player::new(index))));
    }
    Arc::new(RwLock::new(roster))
}

fn roster_player(
    roster: &Arc<RwLock<crate::player::PlayerList>>,
    index: crate::player::PlayerIndex,
) -> Arc<RwLock<Player>> {
    roster
        .read()
        .unwrap()
        .get_player(index)
        .cloned()
        .expect("fixture roster player")
}

fn owner_for_roster(
    name: &str,
    modules: &str,
    roster: &Arc<RwLock<crate::player::PlayerList>>,
    player_index: u32,
) -> Arc<RwLock<Object>> {
    let owner = owner(name, modules, false);
    owner
        .write()
        .unwrap()
        .bind_rank_roster(roster)
        .expect("bind receiving player roster");
    let team = Arc::new(RwLock::new(Team::new(
        format!("{name}Team").into(),
        ID + 100 + player_index,
    )));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player_index));
    owner.write().unwrap().set_team(Some(team)).unwrap();
    owner
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
fn cost_modifier_completion_uses_the_receiving_owner_and_cleans_up_on_capture_delete() {
    if !child(concat!(
        module_path!(),
        "::cost_modifier_completion_uses_the_receiving_owner_and_cleans_up_on_capture_delete"
    )) {
        return;
    }
    const TRIGGER: &str = "Upgrade_CostModifierOwnerIsolation";
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER));
    });
    let module_a = format!(
        " Behavior = CostModifierUpgrade CostModifierOwnerIsolation\n TriggeredBy = {TRIGGER}\n EffectKindOf = STRUCTURE\n Percentage = -10%\n End\n"
    );
    let module_b = format!(
        " Behavior = CostModifierUpgrade CostModifierOwnerIsolation\n TriggeredBy = {TRIGGER}\n EffectKindOf = STRUCTURE\n Percentage = -25%\n End\n"
    );
    let roster_a = roster_with_players(&[0, 1]);
    let roster_b = roster_with_players(&[0]);
    let source_a = owner_for_roster("CostModifierOwnerA", &module_a, &roster_a, 0);
    let source_b = owner_for_roster("CostModifierOwnerB", &module_b, &roster_b, 0);
    assert_eq!(source_a.read().unwrap().id, source_b.read().unwrap().id);
    assert!(crate::object::registry::OBJECT_REGISTRY
        .get_object(ID)
        .is_none());

    source_a
        .write()
        .unwrap()
        .give_upgrade(&upgrade(TRIGGER, UpgradeType::Object));
    source_b
        .write()
        .unwrap()
        .give_upgrade(&upgrade(TRIGGER, UpgradeType::Object));

    let player_a0 = roster_player(&roster_a, 0);
    let player_a1 = roster_player(&roster_a, 1);
    let player_b0 = roster_player(&roster_b, 0);
    assert_eq!(
        player_a0.read().unwrap().get_player_index(),
        player_b0.read().unwrap().get_player_index(),
        "the colliding Objects resolve slot zero from distinct receiving rosters"
    );
    let structure = game_engine::common::system::kind_of::KindOfMask::STRUCTURE.bits();
    assert_eq!(
        player_a0
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        0.9,
        "A's completed module must affect only A's player"
    );
    assert_eq!(
        player_b0
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        0.75,
        "same numeric ObjectID in B must still affect B's player"
    );
    let quote = crate::object::production::build_cost_calculator::BuildCostCalculator::new()
        .calc_cost_to_build(
            1_000,
            &crate::object::production::build_cost_calculator::PlayerBuildModifiers {
                production_cost_change_by_kind: player_a0
                    .read()
                    .unwrap()
                    .get_production_cost_change_based_on_kind_of(structure),
                ..Default::default()
            },
        );
    assert_eq!(quote, 900, "the authored kind modifier is a live purchase quote");
    let quote_b = crate::object::production::build_cost_calculator::BuildCostCalculator::new()
        .calc_cost_to_build(
            1_000,
            &crate::object::production::build_cost_calculator::PlayerBuildModifiers {
                production_cost_change_by_kind: player_b0
                    .read()
                    .unwrap()
                    .get_production_cost_change_based_on_kind_of(structure),
                ..Default::default()
            },
        );
    assert_eq!(quote_b, 750, "B's authored value is read from B's roster");

    // Object.cpp calls onCapture after changing the object's controlling team.
    let captured_team = Arc::new(RwLock::new(Team::new("CapturedInA".into(), ID + 200)));
    captured_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(1));
    source_a
        .write()
        .unwrap()
        .set_team(Some(captured_team))
        .unwrap();
    source_a
        .write()
        .unwrap()
        .on_capture(Some(player_a0.clone()), Some(player_a1.clone()));
    assert_eq!(
        player_a0
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        1.0
    );
    assert_eq!(
        player_a1
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        0.9
    );
    assert_eq!(
        player_b0
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        0.75,
        "capture in A's receiving roster must not cross into B"
    );

    source_a.write().unwrap().on_destroy();
    assert_eq!(
        player_a1
            .read()
            .unwrap()
            .get_production_cost_change_based_on_kind_of(structure),
        1.0,
        "destroying captured A removes only its retained reference"
    );
    assert_eq!(
        player_b0.read().unwrap().kind_of_production_change_entries(),
        vec![(structure, -0.25, 1)]
    );

    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut xfer = game_engine::common::system::xfer_save::XferSave::new(&mut bytes, 1);
        player_b0.write().unwrap().xfer(&mut xfer).unwrap();
    }
    bytes.set_position(0);
    let mut restored = Player::new(0);
    {
        let mut xfer = game_engine::common::system::xfer_load::XferLoad::new(&mut bytes, 1);
        restored.xfer(&mut xfer).unwrap();
    }
    assert_eq!(
        restored.kind_of_production_change_entries(),
        vec![(structure, -0.25, 1)],
        "the owning Player save keeps the remaining module reference"
    );
}

#[test]
fn give_upgrade_combines_two_completed_object_bits() {
    if !child(concat!(
        module_path!(),
        "::give_upgrade_combines_two_completed_object_bits"
    )) {
        return;
    }
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

// hq-2qedt.67 — C++ UpgradeModule.cpp:105-147,191-201 + Object.cpp:4491-4503:
// ConflictsWith blocks attemptUpgrade without latching; a later module's
// RemovesUpgrades resets an earlier, already-executed module's latch so the
// next updateUpgradeModules re-runs it in authored order.
#[test]
fn conflicts_block_without_latch_and_later_removal_resets_earlier_latch() {
    if !child(concat!(
        module_path!(),
        "::conflicts_block_without_latch_and_later_removal_resets_earlier_latch"
    )) {
        return;
    }
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    game_engine::common::common_services::with_common_services(&common, || {
        let stores = crate::system::engine_stores::new_for_world();
        crate::system::engine_stores::with_active_stores(&stores, conflict_reset_body);
    });
}

fn conflict_reset_body() {
    let modules = format!(
        " Behavior = StatusBitsUpgrade EarlySet\n TriggeredBy = {FIRST}\n ConflictsWith = {SECOND}\n StatusToSet = MASKED\n End\n Behavior = StatusBitsUpgrade LateRemoves\n TriggeredBy = {SECOND}\n RemovesUpgrades = {FIRST}\n StatusToClear = MASKED\n End\n"
    );
    let owner = owner("OwnedAdmissionConflictReset", &modules, true);
    // SECOND alone: LateRemoves runs (removing absent FIRST is a no-op).
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(SECOND, UpgradeType::Object));
    // FIRST with SECOND owned: EarlySet conflicts -> not run, not latched.
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
    assert!(
        !status(&owner, ObjectStatusMaskType::MASKED),
        "ConflictsWith in key mask blocks attemptUpgrade"
    );
    // Drop SECOND (resets LateRemoves), then regrant FIRST: EarlySet now runs.
    owner
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(SECOND));
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "conflict-blocked module never latched, so it runs once the conflict clears"
    );
    // SECOND again: EarlySet already latched (skip); LateRemoves removes FIRST
    // (resetting EarlySet) and clears MASKED.
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(SECOND, UpgradeType::Object));
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    assert_eq!(owner.read().unwrap().completed_upgrades(), mask(SECOND));
    // Remove SECOND, regrant FIRST: EarlySet latch was reset by the later
    // module's removal, so it re-runs.
    owner
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(SECOND));
    owner
        .write()
        .unwrap()
        .give_upgrade(&upgrade(FIRST, UpgradeType::Object));
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "later RemovesUpgrades reset the earlier executed module (C++ resetUpgrade)"
    );
}

// hq-2qedt.18 — C++ Object.cpp onVeterancyLevelChanged: updateUpgradeModules()
// runs *before* findVeterancyUpgrade/giveUpgrade and before the Body callback.
// A module triggered by a researched PLAYER upgrade that ConflictsWith the rank
// upgrade only runs if the pre-rank scan happens first; dropping or moving that
// scan after giveUpgrade(Upgrade_Veterancy_VETERAN) leaves it conflict-blocked.
#[test]
fn veterancy_change_rescans_upgrade_modules_before_rank_upgrade_and_body() {
    if !child(concat!(
        module_path!(),
        "::veterancy_change_rescans_upgrade_modules_before_rank_upgrade_and_body"
    )) {
        return;
    }
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    game_engine::common::common_services::with_common_services(&common, || {
        let stores = crate::system::engine_stores::new_for_world();
        crate::system::engine_stores::with_active_stores(&stores, pre_rank_rescan_body);
    });
}

fn pre_rank_rescan_body() {
    let veteran = crate::upgrade::center::get_upgrade_center()
        .read()
        .unwrap()
        .find_veterancy_upgrade("VETERAN")
        .expect("C++ UpgradeCenter::init creates rank upgrades");
    let veteran_name = veteran.get_name().as_str().to_owned();
    assert_eq!(veteran_name, "Upgrade_Veterancy_VETERAN");
    let player = player();
    let first = upgrade(FIRST, UpgradeType::Player);
    player.add_upgrade(&first, UpgradeStatus::Complete, None);
    let owner = owner(
        "OwnedAdmissionPreRankRescan",
        &format!(
            " Behavior = StatusBitsUpgrade PreRankPlayerUpgrade\n TriggeredBy = {FIRST}\n ConflictsWith = {veteran_name}\n StatusToSet = MASKED\n End\n"
        ),
        true,
    );
    // Installed before set_team supplied the player: module has not run.
    assert!(!status(&owner, ObjectStatusMaskType::MASKED));
    owner.write().unwrap().on_veterancy_level_changed(
        VeterancyLevel::Regular,
        VeterancyLevel::Veteran,
        false,
    );
    assert!(
        owner
            .read()
            .unwrap()
            .completed_upgrades()
            .contains(UpgradeMaskType::from_bits_retain(veteran.mask().to_bits())),
        "rank giveUpgrade still follows the pre-rank scan"
    );
    assert!(
        status(&owner, ObjectStatusMaskType::MASKED),
        "pre-rank updateUpgradeModules ran before the VETERAN conflict existed (C++ order)"
    );
}
