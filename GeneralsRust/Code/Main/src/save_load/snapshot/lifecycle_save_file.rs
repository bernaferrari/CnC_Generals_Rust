//! SaveFileManager round-trip for the v9 lifecycle tail.

use super::*;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use std::time::{Duration, SystemTime};

fn save_info(filename: &str) -> SaveGameInfo {
    SaveGameInfo {
        filename: filename.to_string(),
        display_name: filename.to_string(),
        description: "lifecycle tail save_file round-trip".to_string(),
        map_name: "LifecycleMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

#[test]
fn save_file_roundtrip_preserves_lifecycle_envelope() {
    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");

    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("LifecycleRanger");
    template.add_kind_of(KindOf::Infantry);
    source
        .templates
        .insert("LifecycleRanger".to_string(), template);
    let id = source
        .create_object("LifecycleRanger", Team::USA, Vec3::new(4.0, 0.0, 6.0))
        .expect("create");
    {
        let object = source.host_object_mut(id).expect("object");
        object.fire_weapon_when_dead_fired = true;
        object.emoticon_name = "Cheer".to_string();
        object.emoticon_frames_left = 18;
        object.carpet_bomb_payload = true;
    }

    manager
        .save_game(
            "lifecycle_tail_rt",
            &source,
            &save_info("lifecycle_tail_rt"),
        )
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("lifecycle_tail_rt", &mut loaded)
        .expect("load");

    let object = loaded.host_object(id).expect("loaded");
    assert!(object.fire_weapon_when_dead_fired);
    assert_eq!(object.emoticon_name, "Cheer");
    assert_eq!(object.emoticon_frames_left, 18);
    assert!(object.carpet_bomb_payload);
    assert!(!object.entity_lifecycle_envelope().module_states.is_empty());
}

#[test]
fn save_file_roundtrip_survives_unpaused_cooldown_and_keeps_weapon_clip() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::Weapon;

    // In-match seam: an UNPAUSED running special-power cooldown plus any
    // object (OXOB suffix always follows SPCD) used to abort the whole load —
    // SPCD v1 inferred its optional pauses table from a non-empty tail and
    // read the sibling magic as a ~1.1e9 table count.
    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");

    let mut source = GameLogic::new();
    source
        .templates
        .insert("LifecycleCannon".to_string(), ThingTemplate::new("LifecycleCannon"));
    let id = source
        .create_object("LifecycleCannon", Team::USA, Vec3::new(4.0, 0.0, 6.0))
        .expect("create");
    {
        let object = source.host_object_mut(id).expect("object");
        object
            .special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 88.0);
        object.weapon = Some(Weapon {
            clip_size: 5,
            clip_reload_time: 3.0,
            splash_radius: 12.5,
            reloading_clip: true,
            last_bonus_rof: 1.5,
            ..Weapon::default()
        });
    }

    manager
        .save_game("lifecycle_clip_rt", &source, &save_info("lifecycle_clip_rt"))
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("lifecycle_clip_rt", &mut loaded)
        .expect("load must not fail on unpaused cooldown");

    let object = loaded.host_object(id).expect("loaded");
    let remaining = object
        .special_power_cooldowns
        .get(&SpecialPowerType::ParticleCannon)
        .copied()
        .expect("cooldown restored");
    assert!((remaining - 88.0).abs() < 1e-4);
    assert!(object.special_power_paused.is_empty());
    let weapon = object.weapon.as_ref().expect("weapon restored");
    assert_eq!(weapon.clip_size, 5);
    assert!((weapon.clip_reload_time - 3.0).abs() < 1e-4);
    assert!((weapon.splash_radius - 12.5).abs() < 1e-4);
    assert!(weapon.reloading_clip);
    assert!((weapon.last_bonus_rof - 1.5).abs() < 1e-4);
}

#[test]
fn save_file_absent_lifecycle_tail_loads_empty() {
    let mut snapshot = WorldSnapshot::default();
    snapshot.lifecycle_tail.clear();
    let mut logic = GameLogic::new();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut logic)
        .expect("absent tail");
    assert!(logic.host_objects().is_empty());
    let decoded = decode_lifecycle_tail(&snapshot.lifecycle_tail).expect("empty");
    assert!(decoded.envelopes.is_empty());
}
