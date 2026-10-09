//! hq-btakc: real kept-object dispatch and disk save/load, with separate controls.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

struct Fixture {
    logic: GameLogic,
    kept: ObjectId,
    live: ObjectId,
    zero_hp: ObjectId,
    bridge: ObjectId,
}

fn fixture() -> Fixture {
    let mut logic = GameLogic::new();
    logic.set_current_frame(37);
    logic.add_player(Player::new(0, Team::USA, "Keep save owner", true));
    let mut kept_template = ThingTemplate::new("TechOilDerrick");
    kept_template
        .set_health(500.0)
        .add_kind_of(KindOf::Structure);
    logic
        .templates
        .insert("TechOilDerrick".into(), kept_template);
    let mut live_template = ThingTemplate::new("KeptSaveLive");
    live_template.set_health(100.0);
    logic.templates.insert("KeptSaveLive".into(), live_template);
    let mut bridge_template = ThingTemplate::new("KeptSaveBridge");
    bridge_template
        .set_health(100.0)
        .add_kind_of(KindOf::Bridge);
    logic
        .templates
        .insert("KeptSaveBridge".into(), bridge_template);
    let kept = logic
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let live = logic
        .create_object("KeptSaveLive", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let zero_hp = logic
        .create_object("KeptSaveLive", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    let bridge = logic
        .create_object("KeptSaveBridge", Team::USA, Vec3::new(90.0, 0.0, 0.0))
        .unwrap();
    // Existing production dispatch selects KeepObject, without authoring its flags.
    logic.mark_object_for_destruction(kept, None);
    let husk = logic.host_object(kept).unwrap();
    assert!(husk.status.on_die_started);
    assert!(husk.status.keep_as_rubble && husk.status.effectively_dead);
    assert!(!husk.status.destroyed && husk.health.current == 0.0);
    assert_eq!(husk.keep_object_die.as_ref().unwrap().rubble_frame, 37);
    // Encoding control only: dead HP without dispatch must not invent retained state.
    logic.host_object_mut(zero_hp).unwrap().health.current = 0.0;
    // Compatibility control uses the real existing bridge-husk operation, not KeepObject dispatch.
    logic
        .host_object_mut(bridge)
        .unwrap()
        .convert_bridge_to_rubble_husk();
    Fixture {
        logic,
        kept,
        live,
        zero_hp,
        bridge,
    }
}

fn save_info() -> SaveGameInfo {
    SaveGameInfo {
        filename: "kept_rubble".into(),
        display_name: "Kept rubble".into(),
        description: "hq-btakc actual SaveFileManager route".into(),
        map_name: "KeptStateMap".into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH,
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn save_and_load(source: &GameLogic, target: &mut GameLogic) {
    let directory = tempfile::TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    manager
        .save_game("kept_rubble", source, &save_info())
        .unwrap();
    assert!(manager.get_save_path("kept_rubble").is_file());
    // Catalog installation is the same restored-map prerequisite used by existing save tests.
    target.templates = source.templates.clone();
    manager.load_game("kept_rubble", target).unwrap();
}

fn heal(logic: &mut GameLogic, id: ObjectId) {
    // Exercise the same public typed Healing path whose retained-state guards regress.
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    assert!(!object.take_damage_from_typed(25.0, None, DamageType::Healing, health_events,));
}

#[test]
fn keepobject_save_file_nonbridge_healing_remains_rejected_after_load() {
    let mut f = fixture();
    heal(&mut f.logic, f.kept);
    assert_eq!(f.logic.host_object(f.kept).unwrap().health.current, 0.0);
    let mut loaded = GameLogic::new();
    save_and_load(&f.logic, &mut loaded);
    heal(&mut loaded, f.kept);
    // First restored assertion is the actual behavioral failure on OLD.
    assert_eq!(
        loaded.host_object(f.kept).unwrap().health.current,
        0.0,
        "restored nonbridge KeepObject husk must reject Healing exactly as before save"
    );
    let kept = loaded.host_object(f.kept).unwrap();
    assert!(kept.status.on_die_started);
    assert!(kept.status.keep_as_rubble && kept.status.effectively_dead);
    assert!(!kept.status.destroyed && !kept.is_alive());
}

#[test]
fn keepobject_save_file_preserves_flags_module_frame_and_latch_independently() {
    let f = fixture();
    let mut loaded = GameLogic::new();
    save_and_load(&f.logic, &mut loaded);
    let kept = loaded.host_object(f.kept).unwrap();
    assert!(
        kept.status.keep_as_rubble,
        "actual dispatched KeepObject must retain the kept-object status flag"
    );
    assert!(kept.status.effectively_dead);
    assert!(kept.status.on_die_started);
    assert!(!kept.status.destroyed);
    let module = kept
        .keep_object_die
        .as_ref()
        .expect("retain existing owned module state");
    assert!(module.is_rubble);
    assert_eq!(module.rubble_frame, 37);
    assert_eq!(
        f.logic
            .host_object(f.kept)
            .unwrap()
            .keep_object_die
            .as_ref()
            .unwrap()
            .rubble_frame,
        37
    );
}

#[test]
fn keepobject_save_in_place_restores_retained_state_without_losing_live_controls() {
    let mut f = fixture();
    let directory = tempfile::TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    manager
        .save_game("kept_rubble", &f.logic, &save_info())
        .unwrap();
    // Change the actual existing target to make this an in-place restore test.
    let kept = f.logic.host_object_mut(f.kept).unwrap();
    kept.status.keep_as_rubble = false;
    kept.status.effectively_dead = false;
    kept.keep_object_die = None;
    manager.load_game("kept_rubble", &mut f.logic).unwrap();
    heal(&mut f.logic, f.kept);
    assert_eq!(
        f.logic.host_object(f.kept).unwrap().health.current,
        0.0,
        "in-place restore must reinstate nonbridge rubble's healing rejection"
    );
    assert!(f.logic.host_object(f.kept).unwrap().status.on_die_started);
    assert!(!f.logic.host_object(f.live).unwrap().status.keep_as_rubble);
    assert!(
        !f.logic
            .host_object(f.zero_hp)
            .unwrap()
            .status
            .on_die_started
    );
}

#[test]
fn keepobject_save_file_live_and_zero_hp_controls_do_not_invent_retained_state() {
    let f = fixture();
    let mut loaded = GameLogic::new();
    save_and_load(&f.logic, &mut loaded);
    for id in [f.live, f.zero_hp] {
        let object = loaded.host_object(id).unwrap();
        assert!(!object.status.keep_as_rubble && !object.status.effectively_dead);
        assert!(!object.status.on_die_started && !object.status.destroyed);
        assert!(object.keep_object_die.is_none());
    }
    assert!(loaded.host_object(f.live).unwrap().is_alive());
    assert!(!loaded.host_object(f.zero_hp).unwrap().is_alive());
    heal(&mut loaded, f.zero_hp);
    assert_eq!(
        loaded.host_object(f.zero_hp).unwrap().health.current,
        25.0,
        "zero HP alone must not invent KeepObject and suppress permitted healing"
    );
    assert!(!loaded.host_object(f.zero_hp).unwrap().status.on_die_started);
}

#[test]
fn keepobject_save_file_bridge_rubble_still_allows_existing_revival() {
    let mut f = fixture();
    let mut loaded = GameLogic::new();
    save_and_load(&f.logic, &mut loaded);
    for logic in [&mut f.logic, &mut loaded] {
        heal(logic, f.bridge);
        let bridge = logic.host_object(f.bridge).unwrap();
        assert_eq!(bridge.health.current, 25.0);
        assert!(!bridge.status.effectively_dead && !bridge.status.keep_as_rubble);
        assert!(!bridge.status.destroyed && bridge.is_alive());
        assert!(!bridge.status.on_die_started);
    }
}

#[test]
fn keepobject_snapshot_builder_restores_kept_state_alongside_exact_latch() {
    let f = fixture();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&f.logic)
        .unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = f.logic.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    let kept = loaded.host_object(f.kept).unwrap();
    assert!(kept.status.keep_as_rubble && kept.status.effectively_dead);
    assert!(kept.status.on_die_started);
    assert_eq!(kept.keep_object_die.as_ref().unwrap().rubble_frame, 37);
    for id in [f.live, f.zero_hp] {
        let object = loaded.host_object(id).unwrap();
        assert!(!object.status.keep_as_rubble && !object.status.effectively_dead);
        assert!(!object.status.on_die_started && object.keep_object_die.is_none());
    }
}
