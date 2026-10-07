//! ExperienceTracker.cpp::xfer restores XP and rank without replaying promotions.
//! Exercise the ordinary Main library and real save-file codec without retail assets.

use generals_main::game_logic::{
    Experience, GameLogic, KindOf, ObjectId, Team, ThingTemplate, VeterancyLevel, Weapon,
};
use generals_main::save_load::{
    ExperienceEventSnapshot, GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo,
    SaveLoadError, SnapshotBuilder, VeterancyBonuses, Xfer, XferData, XferLoad, XferSave,
};
use glam::Vec3;
use std::io::Cursor;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

const UNIT: &str = "SaveExperienceUnit";

fn world(thresholds: [f32; 3]) -> GameLogic {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new(UNIT);
    template.is_trainable = true;
    template.veterancy_xp_thresholds = thresholds;
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    template.primary_weapon = Some(Weapon {
        damage: 10.0,
        reload_time: 1.0,
        ..Weapon::default()
    });
    world.templates.insert(UNIT.to_string(), template);
    world
}

fn state(
    world: &GameLogic,
    id: ObjectId,
    thresholds: [f32; 3],
) -> (f32, VeterancyLevel, f32, f32, f32, f32) {
    let object = world.host_object(id).expect("unit exists");
    assert!(object.is_trainable());
    assert_eq!(object.get_template().veterancy_xp_thresholds, thresholds);
    assert_eq!(world.templates[UNIT].veterancy_xp_thresholds, thresholds);
    let weapon = object.weapon.as_ref().expect("authored weapon exists");
    (
        object.experience.current,
        object.experience.level,
        object.health.current,
        object.health.maximum,
        weapon.damage,
        weapon.reload_time,
    )
}

fn save_info() -> SaveGameInfo {
    SaveGameInfo {
        filename: "experience".to_string(),
        display_name: "Experience round trip".to_string(),
        description: String::new(),
        map_name: "ExperienceFixture".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn round_trip(thresholds: [f32; 3], initial_xp: f32, level: VeterancyLevel, continuation: &[f32]) {
    let mut source = world(thresholds);
    let id = source
        .create_object(UNIT, Team::USA, Vec3::ZERO)
        .expect("spawn unit");
    // Use the production XP API; no post-admission XP/rank field repair.
    let object = source.host_object_mut(id).unwrap();
    if initial_xp == 0.0 && level != VeterancyLevel::Rookie {
        // A zero authored threshold legitimately permits Veteran at zero XP.
        assert!(object.set_min_veterancy_level(level));
    } else {
        object.gain_experience(initial_xp);
    }
    let before = state(&source, id, thresholds);
    assert_eq!((before.0, before.1), (initial_xp, level));

    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    manager
        .save_game("experience", &source, &save_info())
        .unwrap();
    let (snapshot, _) = manager.load_game_snapshot("experience").unwrap();
    let saved = &snapshot.objects[&id];
    assert_eq!(
        (saved.experience.current, saved.experience.level),
        (initial_xp, level)
    );
    if initial_xp > 0.0 {
        let events = &snapshot.experience_tracker.experience_events;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].object_id, id);
        assert_eq!(events[0].experience_gained, initial_xp);
    }

    let mut restored = world(thresholds);
    manager.load_game("experience", &mut restored).unwrap();
    let after = state(&restored, id, thresholds);
    eprintln!(
        "thresholds={thresholds:?} before={before:?} saved=({}, {:?}) immediately_restored={after:?}",
        saved.experience.current, saved.experience.level
    );
    assert_eq!(
        after, before,
        "load must not award XP or replay veterancy bonuses"
    );

    // An uninterrupted control must cross subsequent promotion boundaries on
    // exactly the same grants as the loaded unit.
    for &amount in continuation {
        source.host_object_mut(id).unwrap().gain_experience(amount);
        restored
            .host_object_mut(id)
            .unwrap()
            .gain_experience(amount);
        assert_eq!(
            state(&restored, id, thresholds),
            state(&source, id, thresholds)
        );
    }
}

#[test]
fn custom_thresholds_preserve_low_veteran_xp_and_continuation() {
    round_trip(
        [10.0, 100.0, 1000.0],
        10.0,
        VeterancyLevel::Veteran,
        &[39.0, 50.0, 1.0, 899.0, 1.0],
    );
}

#[test]
fn default_thresholds_preserve_xp_and_continuation() {
    let thresholds = ThingTemplate::new(UNIT).veterancy_xp_thresholds;
    for (xp, level) in [
        (0.0, VeterancyLevel::Rookie),
        (59.0, VeterancyLevel::Rookie),
        (60.0, VeterancyLevel::Veteran),
        (149.0, VeterancyLevel::Veteran),
        (150.0, VeterancyLevel::Elite),
        (180.0, VeterancyLevel::Elite),
        (299.0, VeterancyLevel::Elite),
        (300.0, VeterancyLevel::Heroic),
        (310.0, VeterancyLevel::Heroic),
    ] {
        round_trip(thresholds, xp, level, &[1.0, 59.0, 90.0, 150.0]);
    }
}

#[test]
fn custom_elite_and_heroic_thresholds_preserve_xp() {
    round_trip([10.0, 20.0, 30.0], 20.0, VeterancyLevel::Elite, &[9.0, 1.0]);
    round_trip([10.0, 20.0, 30.0], 30.0, VeterancyLevel::Heroic, &[1.0]);
}

#[test]
fn zero_threshold_preserves_promoted_rank_without_inventing_xp() {
    round_trip(
        [0.0, 100.0, 1000.0],
        0.0,
        VeterancyLevel::Veteran,
        &[99.0, 1.0],
    );
}

#[test]
fn required_object_experience_wins_over_redundant_tracker_rows() {
    let thresholds = [10.0, 100.0, 1000.0];
    for xp in [0.0, 10.0] {
        let mut source = world(thresholds);
        let id = source.create_object(UNIT, Team::USA, Vec3::ZERO).unwrap();
        source.host_object_mut(id).unwrap().gain_experience(xp);
        source
            .host_object_mut(id)
            .unwrap()
            .set_experience_scalar(2.0);
        let before = state(&source, id, thresholds);
        let builder = SnapshotBuilder::new();
        for conflicting_tracker in [false, true] {
            let mut snapshot = builder.create_world_snapshot(&source).unwrap();
            snapshot.experience_tracker = Default::default();
            if conflicting_tracker {
                // Object XP/rank are required fields. Redundant rows cannot
                // redefine even an explicit zero/Rookie as "missing" XP.
                snapshot
                    .experience_tracker
                    .experience_events
                    .push(ExperienceEventSnapshot {
                        object_id: id,
                        experience_gained: 180.0,
                        source: "snapshot_state".to_string(),
                        timestamp: 0.0,
                    });
                snapshot.experience_tracker.veterancy_bonuses.insert(
                    id,
                    VeterancyBonuses {
                        health_bonus: 2.0,
                        damage_bonus: 2.0,
                        accuracy_bonus: 1.2,
                        range_bonus: 1.1,
                    },
                );
            }
            let mut restored = world(thresholds);
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            assert_eq!(restored.host_object(id).unwrap().experience_scalar, 2.0);
            assert_eq!(
                state(&restored, id, thresholds),
                before,
                "xp={xp}, conflicting_tracker={conflicting_tracker}"
            );
        }
    }
}

#[test]
fn missing_or_invalid_experience_is_not_a_recovery_sentinel() {
    let mut source = world([10.0, 100.0, 1000.0]);
    let id = source.create_object(UNIT, Team::USA, Vec3::ZERO).unwrap();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut object_json = serde_json::to_value(&snapshot.objects[&id]).unwrap();
    object_json.as_object_mut().unwrap().remove("experience");
    assert!(
        serde_json::from_value::<generals_main::save_load::ObjectSnapshot>(object_json).is_err()
    );

    let mut bytes = Vec::new();
    {
        let mut writer = XferSave::new(Cursor::new(&mut bytes));
        writer.xfer_f32(&mut 10.0).unwrap();
        writer.xfer_u32(&mut 99).unwrap();
    }
    let mut xp = Experience::default();
    assert!(matches!(
        xp.xfer(&mut XferLoad::new(Cursor::new(&bytes))),
        Err(SaveLoadError::Corrupted(_))
    ));
    for length in 0..bytes.len() {
        assert!(
            Experience::default()
                .xfer(&mut XferLoad::new(Cursor::new(&bytes[..length])))
                .is_err()
        );
    }

    // Rust schemas 23/24 are readable. Unsupported schema 22 records never
    // enter restore, so they cannot justify inventing XP from bonus values.
    snapshot.version = 22;
    let mut restored = world([10.0, 100.0, 1000.0]);
    assert!(matches!(
        builder.restore_from_snapshot(&snapshot, &mut restored),
        Err(SaveLoadError::VersionMismatch { .. })
    ));
    assert!(restored.host_objects().is_empty());
}
