//! Ordinary authored research owns completion; shadow publication cannot replay it.
//! CPP ProductionUpdate.cpp:874-939; Player.cpp:3005-3048, 3985-4065.
use super::*;
use crate::command_system::{CommandType, GameCommand, ModifierKeys, SpecialPowerType};
use crate::game_logic::host_upgrades::{
    HostUpgradePhase, HostUpgradeResearch, UPGRADE_INFANTRY_CAPTURE,
};
use crate::game_logic::{CapturePowerKind, ObjectId, Player};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use std::time::{Duration, SystemTime};

const OWNER: u32 = 7;
const OTHER: u32 = 8;
const PRODUCER: &str = "AmericaBarracks";
const INFANTRY: &str = "AuthoredOwnerCaptureInfantry";

fn install_rules(logic: &mut GameLogic, cost: u32, seconds: u32) {
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        logic,
        UPGRADE_INFANTRY_CAPTURE,
        "PLAYER",
        cost,
        seconds,
    );
    // Main's actual Object definition parser/projection, not synthetic capture flags.
    // Producer admission uses the existing static retail CommandSet fallback;
    // this fixture does not claim full authored Common CommandSet coverage.
    let source = format!(
        "Object {PRODUCER}\n  Type = Structure\n  KindOf = STRUCTURE SELECTABLE ATTACKABLE FS_BARRACKS\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 1000\n  End\n  Behavior = ProductionUpdate ModuleTag_Production\n  End\nEnd\nObject {INFANTRY}\n  Type = Infantry\n  KindOf = INFANTRY SELECTABLE ATTACKABLE\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 100\n  End\n  Behavior = SpecialAbility ModuleTag_Capture\n    SpecialPowerTemplate = SpecialAbilityRangerCaptureBuilding\n    StartsPaused = Yes\n  End\n  Behavior = UnpauseSpecialPowerUpgrade ModuleTag_CaptureUpgrade\n    SpecialPowerTemplate = SpecialAbilityRangerCaptureBuilding\n    TriggeredBy = {UPGRADE_INFANTRY_CAPTURE}\n  End\nEnd\n"
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&source, "upgrade_completion_owner.ini")
        .expect("real Object INI");
    for name in [PRODUCER, INFANTRY] {
        let definition = parser.get_definition(name).expect("authored definition");
        let template = GameLogic::build_template_from_object_definition(name, definition, None);
        assert!(template.is_kind_of(KindOf::Attackable));
        if name == INFANTRY {
            assert_eq!(template.capture_power, CapturePowerKind::Ranger);
            assert!(template.capture_starts_paused);
            assert_eq!(
                template.capture_upgrade_trigger.as_deref(),
                Some(UPGRADE_INFANTRY_CAPTURE)
            );
        } else {
            assert!(template.is_kind_of(KindOf::Structure));
            assert!(template.is_kind_of(KindOf::FSBarracks));
        }
        logic.templates.insert(name.to_owned(), template);
    }
}

fn admit(logic: &mut GameLogic) -> (ObjectId, ObjectId, ObjectId) {
    let mut owner = Player::new(OWNER, Team::USA, "Research owner", true);
    owner.resources.supplies = 5000;
    logic.add_player(owner);
    let mut other = Player::new(OTHER, Team::USA, "Same faction foreign owner", true);
    other.resources.supplies = 7777;
    logic.add_player(other);
    let producer = logic
        .create_object_for_player(PRODUCER, OWNER, Vec3::ZERO)
        .expect("producer");
    let infantry = logic
        .create_object_for_player(INFANTRY, OWNER, Vec3::new(80.0, 0.0, 0.0))
        .expect("infantry");
    let foreign = logic
        .create_object_for_player(INFANTRY, OTHER, Vec3::new(160.0, 0.0, 0.0))
        .expect("foreign infantry");
    let building = logic.host_object(producer).unwrap();
    assert!(building.building_data.is_some() && building.is_constructed());
    assert_eq!(building.owner_player_id, Some(OWNER));
    assert!(
        logic
            .host_object(infantry)
            .unwrap()
            .is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding)
    );
    assert!(
        logic
            .host_object(foreign)
            .unwrap()
            .is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding)
    );
    (producer, infantry, foreign)
}

fn queue(logic: &mut GameLogic, producer: ObjectId, cost: u32, frames: u32) {
    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_owned(),
        },
        player_id: OWNER,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![producer],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    assert_eq!(
        logic.get_player(OWNER).unwrap().resources.supplies,
        5000 - cost
    );
    assert!(
        logic
            .get_player(OWNER)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE)
    );
    assert!(
        !logic
            .get_player(OWNER)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE)
    );
    assert_eq!(logic.get_player(OTHER).unwrap().resources.supplies, 7777);
    let entry = only_entry(logic);
    assert_eq!(entry.phase, HostUpgradePhase::Queued);
    assert_eq!(entry.source_object, Some(producer));
    assert_eq!(entry.player_id, OWNER);
    assert_eq!(entry.build_cost_paid, cost);
    assert_eq!(entry.queue_frame, logic.getFrame());
    assert_eq!(entry.retail_research_frames, frames);
    assert_eq!(entry.residual_research_frames, frames);
}

fn only_entry(logic: &GameLogic) -> HostUpgradeResearch {
    let mut entries = logic.host_upgrades().entries_snapshot();
    assert_eq!(
        entries.len(),
        1,
        "ordinary command creates exactly one research record"
    );
    entries.pop().unwrap()
}

fn tick(logic: &mut GameLogic, shadow: &mut GameWorldShadow) {
    let frame = logic.get_current_frame();
    logic.update();
    assert_eq!(
        logic.get_current_frame(),
        frame + 1,
        "ordinary simulation tick"
    );
    super::super::run_post_logic_shadow_boundary(Some(shadow), logic);
}

fn complete(logic: &GameLogic, infantry: ObjectId, foreign: ObjectId, at_frame: u32) {
    assert!(
        !logic
            .get_player(OWNER)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE)
    );
    assert!(
        logic
            .get_player(OWNER)
            .unwrap()
            .completed_upgrades
            .contains(UPGRADE_INFANTRY_CAPTURE)
    );
    assert!(
        !logic
            .get_player(OTHER)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE)
    );
    assert_eq!(logic.get_player(OTHER).unwrap().resources.supplies, 7777);
    assert!(
        !logic
            .host_object(infantry)
            .unwrap()
            .is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding)
    );
    assert!(
        logic
            .host_object(foreign)
            .unwrap()
            .is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding)
    );
    let entry = only_entry(logic);
    assert_eq!(entry.phase, HostUpgradePhase::Completed);
    assert_eq!(entry.complete_frame, at_frame);
    assert_eq!(logic.host_upgrades().pending_count(), 0);
}

#[test]
fn upgrade_completion_owner_two_worlds_and_reset_reuse_keep_canonical_effects() {
    let _serial = authority_env_lock();
    let mut a = GameLogic::new();
    install_rules(&mut a, 321, 1);
    let a_ids = admit(&mut a);
    a.set_current_frame(10);
    queue(&mut a, a_ids.0, 321, 30);
    let a_queued = only_entry(&a);
    let mut b = GameLogic::new();
    assert!(b.host_upgrades().entries_snapshot().is_empty());
    assert_eq!(
        only_entry(&a),
        a_queued,
        "foreign construction cannot publish a completion"
    );
    install_rules(&mut b, 654, 2);
    let b_ids = admit(&mut b);
    assert_eq!(a_ids, b_ids, "same numerical ObjectIds in separate worlds");
    b.set_current_frame(10);
    queue(&mut b, b_ids.0, 654, 60);
    let mut a_shadow = GameWorldShadow::new(16);
    let mut b_shadow = GameWorldShadow::new(16);
    for _ in 0..29 {
        tick(&mut a, &mut a_shadow);
        tick(&mut b, &mut b_shadow);
    }
    assert_eq!(only_entry(&a).phase, HostUpgradePhase::Queued);
    assert_eq!(only_entry(&b).phase, HostUpgradePhase::Queued);
    tick(&mut a, &mut a_shadow);
    complete(&a, a_ids.1, a_ids.2, 39);
    let a_complete = only_entry(&a);
    let b_pending = only_entry(&b);
    super::super::run_post_logic_shadow_boundary(Some(&mut a_shadow), &mut a);
    assert_eq!(
        only_entry(&a),
        a_complete,
        "publication cannot replay a completion"
    );
    assert_eq!(
        only_entry(&b),
        b_pending,
        "equal IDs do not complete the foreign world"
    );
    for _ in 29..60 {
        tick(&mut b, &mut b_shadow);
    }
    complete(&b, b_ids.1, b_ids.2, 69);
    let b_complete = only_entry(&b);
    a.reset();
    assert!(a.host_upgrades().entries_snapshot().is_empty());
    assert!(a.get_players().is_empty());
    install_rules(&mut a, 321, 1);
    let reused = admit(&mut a);
    assert_eq!(a_ids, reused);
    assert_eq!(only_entry(&b), b_complete);
    queue(&mut a, reused.0, 321, 30);
    for _ in 0..29 {
        tick(&mut a, &mut a_shadow);
    }
    assert_eq!(only_entry(&a).phase, HostUpgradePhase::Queued);
    tick(&mut a, &mut a_shadow);
    complete(&a, reused.1, reused.2, 29);
    assert_eq!(only_entry(&b), b_complete);
}

#[test]
fn upgrade_completion_owner_actual_file_writer_restores_pending_and_completed_research() {
    let _serial = authority_env_lock();
    let dir = tempfile::TempDir::new().expect("save directory");
    let mut manager = SaveFileManager::with_save_directory(dir.path());
    manager
        .init()
        .expect("actual SaveFileManager initialization");
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "upgrade_owner".to_owned(),
        display_name: "Upgrade owner".to_owned(),
        description: "Authored ordinary research".to_owned(),
        map_name: "InlineRulesControl".to_owned(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_owned(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    let mut source = GameLogic::new();
    install_rules(&mut source, 321, 1);
    let ids = admit(&mut source);
    source.set_current_frame(10);
    queue(&mut source, ids.0, 321, 30);
    let mut shadow = GameWorldShadow::new(16);
    for _ in 0..10 {
        tick(&mut source, &mut shadow);
    }
    let pending = only_entry(&source);
    manager
        .save_game("upgrade_owner", &source, &info)
        .expect("actual pending writer");
    let mut restored = GameLogic::new();
    install_rules(&mut restored, 321, 1);
    manager
        .load_game("upgrade_owner", &mut restored)
        .expect("actual pending decoder/fixups");
    assert_eq!(restored.get_current_frame(), 20);
    assert_eq!(only_entry(&restored), pending);
    assert_eq!(restored.get_player(OWNER).unwrap().resources.supplies, 4679);
    assert!(
        restored
            .get_player(OWNER)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE)
    );
    assert!(
        restored
            .host_object(ids.1)
            .unwrap()
            .is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding)
    );
    let mut restored_shadow = GameWorldShadow::new(16);
    for _ in 0..19 {
        tick(&mut restored, &mut restored_shadow);
    }
    assert_eq!(only_entry(&restored).phase, HostUpgradePhase::Queued);
    tick(&mut restored, &mut restored_shadow);
    complete(&restored, ids.1, ids.2, 39);
    let completed = only_entry(&restored);
    manager
        .save_game("upgrade_owner", &restored, &info)
        .expect("actual completed writer");
    let mut loaded = GameLogic::new();
    install_rules(&mut loaded, 321, 1);
    manager
        .load_game("upgrade_owner", &mut loaded)
        .expect("actual completed decoder/fixups");
    complete(&loaded, ids.1, ids.2, 39);
    assert_eq!(only_entry(&loaded), completed);
    let mut loaded_shadow = GameWorldShadow::new(16);
    tick(&mut loaded, &mut loaded_shadow);
    assert_eq!(
        only_entry(&loaded),
        completed,
        "restored publication does not complete twice"
    );
    assert_eq!(
        only_entry(&source),
        pending,
        "restoring another world leaves the source research pending"
    );
}
