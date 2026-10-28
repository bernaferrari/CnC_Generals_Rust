use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team, VeterancyLevel};
use glam::Vec3;

const UNIT: &str = "StatusBitsStealthOwnerUnit";
const VICTIM: &str = "StatusBitsStealthOwnerVictim";

fn world(set: &str, clear: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "StatusBitsStealthOwner.ini".into(),
        format!(
            "Object {UNIT}\n IsTrainable = Yes\n KindOf = INFANTRY SELECTABLE\n ExperienceRequired = 0 100 250 450\n Body = ActiveBody XpBody\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StatusBitsUpgrade VeteranRankMarker\n TriggeredBy = Upgrade_Veterancy_VETERAN\n StatusToSet = {set}\n StatusToClear = {clear}\n End\nEnd\nObject {VICTIM}\n IsTrainable = Yes\n KindOf = STRUCTURE SELECTABLE SCORE\n ExperienceValue = 125 125 125 125\n Body = ActiveBody VictimBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ),
    )])
    .expect("receiving Object INI admitted");
    game.start_new_game(GameMode::SinglePlayer);
    let mut owner = Player::new(1, Team::USA, "Owner", false);
    owner.alliance_team = 1;
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    game.add_player(owner);
    game.add_player(enemy);
    let id = game
        .create_object_for_player(UNIT, 1, Vec3::ZERO)
        .expect("admitted test object");
    let victim = game
        .create_object_for_player(VICTIM, 2, Vec3::new(4.0, 0.0, 0.0))
        .expect("admitted XP source");
    (game, id, victim)
}

#[test]
fn status_bits_upgrade_updates_visibility_state_per_receiving_world() {
    let (mut first, first_id, first_victim) =
        world("STEALTHED DETECTED", "DETECTED");
    let (mut peer, peer_id, peer_victim) = world("STEALTHED DETECTED", "STEALTHED");
    assert_eq!(first_id, peer_id, "worlds reuse object IDs");

    // Public receiving-world damage dispatch crosses Veteran and executes the
    // admitted StatusBitsUpgrade callback (C++ setStatus, then clearStatus).
    assert_eq!(
        first.apply_host_hive_damage_from(
            first_victim,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(first_id),
        ),
        (true, false)
    );

    let upgraded = first.host_object(first_id).expect("upgraded object");
    assert_eq!(upgraded.experience.level, VeterancyLevel::Veteran);
    assert!(upgraded.has_object_status_bit("STEALTHED"));
    assert!(upgraded.status.stealthed);
    assert!(!upgraded.has_object_status_bit("DETECTED"));
    assert!(!upgraded.status.detected);
    // Hacker-income visibility consumes these ObjectStatus values.
    assert!(upgraded.status.stealthed && !upgraded.status.detected);
    assert!(
        !generals_main::game_logic::host_hacker_income::should_display_hacker_floating_cash(
            upgraded.status.stealthed,
            upgraded.status.detected,
            false,
            false,
            false,
            false,
            false,
        )
    );

    let untouched = peer.host_object(peer_id).expect("peer object");
    assert!(!untouched.has_object_status_bit("STEALTHED"));
    assert!(!untouched.status.stealthed);
    assert!(!untouched.has_object_status_bit("DETECTED"));
    assert!(!untouched.status.detected);

    assert_eq!(
        peer.apply_host_hive_damage_from(
            peer_victim,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(peer_id),
        ),
        (true, false)
    );
    let detected = peer.host_object(peer_id).expect("peer updated object");
    assert_eq!(detected.experience.level, VeterancyLevel::Veteran);
    assert!(detected.has_object_status_bit("DETECTED"));
    assert!(detected.status.detected);
    assert!(!detected.has_object_status_bit("STEALTHED"));
    assert!(!detected.status.stealthed);
    assert!(
        generals_main::game_logic::host_hacker_income::should_display_hacker_floating_cash(
            detected.status.stealthed,
            detected.status.detected,
            false,
            false,
            false,
            false,
            false,
        )
    );

    let first_after_peer = first.host_object(first_id).expect("first remains isolated");
    assert!(first_after_peer.status.stealthed);
    assert!(!first_after_peer.status.detected);
}

#[test]
fn stealth_status_setters_use_the_canonical_status_fields_only() {
    let (mut game, id, _) = world("NONE", "NONE");
    let object = game.host_object_mut(id).expect("receiving object");
    object.set_status_stealthed(true);
    object.set_status_detected(true);
    assert!(object.has_object_status_bit("STEALTHED"));
    assert!(object.has_object_status_bit("DETECTED"));

    object.set_status_stealthed(false);
    object.set_status_detected(false);
    assert!(!object.has_object_status_bit("STEALTHED"));
    assert!(!object.has_object_status_bit("DETECTED"));
}

#[test]
fn status_bits_visibility_projection_survives_encoded_save_restore() {
    use generals_main::save_load::{
        GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo,
    };
    use std::time::{Duration, UNIX_EPOCH};

    let (mut source, source_id, victim) = world("STEALTHED DETECTED", "DETECTED");
    assert_eq!(
        source.apply_host_hive_damage_from(
            victim,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(source_id),
        ),
        (true, false)
    );
    assert!(source.host_object(source_id).unwrap().status.stealthed);

    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "status-owner".into(),
        display_name: "Status owner continuation".into(),
        description: String::new(),
        map_name: "StatusOwnerFixture".into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    saves.save_game("status-owner", &source, &info).unwrap();

    // Admission provides the receiving Object templates before loading the
    // encoded snapshot, matching the normal game's load continuation.
    let (mut restored, _, _) = world("NONE", "NONE");
    saves.load_game("status-owner", &mut restored).unwrap();
    let loaded = restored
        .host_object(source_id)
        .expect("saved object restored");
    assert!(loaded.status.stealthed);
    assert!(!loaded.status.detected);
    assert!(loaded.has_object_status_bit("STEALTHED"));
}
