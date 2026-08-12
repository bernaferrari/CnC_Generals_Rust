//! Snapshot save/load residual tests.

use super::*;
use crate::game_logic::{
    AIState, Experience, GameLogic, HostStrikePhase, HostSuperweaponKind, KindOf, Object, ObjectId,
    Player, Team, ThingTemplate, VeterancyLevel, Weapon,
};
use glam::Vec3;

#[test]
fn snapshot_restore_rebuilds_state_and_object_id_counter() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("TestTank".to_string(), ThingTemplate::new("TestTank"));
    source.add_player(Player::new(1, Team::USA, "PlayerOne", true));
    source.set_current_frame(777);

    let object_id = source
        .create_object("TestTank", Team::USA, Vec3::new(11.0, 0.0, 7.0))
        .expect("failed to create source object");
    {
        let object = source
            .host_object_mut(object_id)
            .expect("created object should exist");
        object.health.current = 42.0;
        object.status.moving = true;
        object.movement.target_position = Some(Vec3::new(30.0, 0.0, 30.0));
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    assert_eq!(restored.get_current_frame(), 777);
    assert_eq!(restored.get_players().len(), 1);
    let restored_obj = restored
        .host_object(object_id)
        .expect("restored object should exist");
    assert_eq!(restored_obj.get_position(), Vec3::new(11.0, 0.0, 7.0));
    assert_eq!(restored_obj.health.current, 42.0);
    assert!(restored_obj.status.moving);
    assert_eq!(restored_obj.ai_state, AIState::Moving);

    let next_id = restored
        .create_object("TestTank", Team::USA, Vec3::ZERO)
        .expect("failed to create post-restore object");
    assert_eq!(next_id.0, object_id.0 + 1);
}

#[test]
fn snapshot_restore_rebuilds_pathfinding_passability() {
    let mut source = GameLogic::new();
    source.set_pathfinding_static_block(2, 3, true);
    source.set_pathfinding_static_block(5, 7, true);

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    assert!(snapshot.terrain.width > 0);
    assert!(snapshot.terrain.height > 0);

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    assert!(restored.is_pathfinding_static_blocked(2, 3));
    assert!(restored.is_pathfinding_static_blocked(5, 7));
    assert!(!restored.is_pathfinding_static_blocked(0, 0));
}

#[test]
fn snapshot_restore_rebuilds_terrain_height_samples() {
    let mut source = GameLogic::new();
    let (width, height, _) = source.snapshot_pathfinding_passability();
    let len = (width as usize).saturating_mul(height as usize);
    let mut heights = vec![0.0_f32; len];
    if width > 3 && height > 3 {
        heights[(3 * width + 3) as usize] = 18.0;
    } else if !heights.is_empty() {
        heights[0] = 18.0;
    }
    assert!(source.restore_terrain_heights_from_grid(width, height, &heights));

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    assert_eq!(snapshot.terrain.height_map.len(), len);

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let restored_heights = restored
        .snapshot_terrain_heights_for_path_grid()
        .expect("restored terrain samples should exist");
    assert_eq!(restored_heights.len(), len);
    assert!(restored_heights.iter().copied().fold(0.0_f32, f32::max) > 0.0);
}

#[test]
fn snapshot_restore_rebuilds_resource_depots_and_harvesters() {
    let mut source = GameLogic::new();

    let mut supply_template = ThingTemplate::new("TestSupplyPile");
    supply_template
        .add_kind_of(KindOf::Resource)
        .add_kind_of(KindOf::Harvestable);
    source
        .templates
        .insert("TestSupplyPile".to_string(), supply_template);

    let mut worker_template = ThingTemplate::new("TestWorker");
    worker_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Selectable);
    source
        .templates
        .insert("TestWorker".to_string(), worker_template);

    let supply_id = source
        .create_object("TestSupplyPile", Team::Neutral, Vec3::new(20.0, 0.0, 20.0))
        .expect("failed to create supply object");
    let worker_id = source
        .create_object("TestWorker", Team::USA, Vec3::new(15.0, 0.0, 20.0))
        .expect("failed to create worker object");

    {
        let supply = source
            .host_object_mut(supply_id)
            .expect("supply object should exist");
        supply.stored_resources.supplies = 2500;
    }
    {
        let worker = source
            .host_object_mut(worker_id)
            .expect("worker object should exist");
        worker.target = Some(supply_id);
        worker.ai_state = AIState::Gathering;
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let restored_supply = restored
        .host_object(supply_id)
        .expect("restored supply object should exist");
    assert_eq!(restored_supply.stored_resources.supplies, 2500);

    let restored_worker = restored
        .host_object(worker_id)
        .expect("restored worker should exist");
    assert_eq!(restored_worker.target, Some(supply_id));
    assert_eq!(restored_worker.ai_state, AIState::Gathering);
}

#[test]
fn snapshot_restore_recovers_veterancy_from_tracker_data() {
    let mut source = GameLogic::new();
    let mut tank_template = ThingTemplate::new("TestTank");
    tank_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    source
        .templates
        .insert("TestTank".to_string(), tank_template);

    let tank_id = source
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("failed to create tank");
    {
        let tank = source.host_object_mut(tank_id).expect("tank should exist");
        tank.gain_experience(180.0);
        assert_eq!(tank.experience.level, VeterancyLevel::Elite);
    }

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    let tank_snapshot = snapshot
        .objects
        .get_mut(&tank_id)
        .expect("tank snapshot should exist");
    tank_snapshot.experience = Experience::default();
    tank_snapshot.health.current = tank_snapshot.health.maximum.min(100.0);
    tank_snapshot.health.maximum = 100.0;

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let restored_tank = restored
        .host_object(tank_id)
        .expect("restored tank should exist");
    assert_eq!(restored_tank.experience.level, VeterancyLevel::Elite);
    assert!(restored_tank.health.maximum > 100.0);
}

#[test]
fn snapshot_restore_preserves_building_production_modules_and_object_upgrades() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(1, Team::USA, "USA", true));

    let mut barracks = ThingTemplate::new("USA_Barracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable);
    source
        .templates
        .insert("USA_Barracks".to_string(), barracks.clone());

    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_cost(225, 0);
    ranger.build_time = 12.0;
    source.templates.insert("USA_Ranger".to_string(), ranger);

    let barracks_id = source
        .create_object("USA_Barracks", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("failed to create barracks");
    assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));
    {
        let building = source
            .host_object_mut(barracks_id)
            .expect("barracks should exist");
        let building_data = building
            .building_data
            .as_mut()
            .expect("barracks should have building data");
        building_data.production_queue[0].progress = 4.5;
        building_data.rally_point = Some(Vec3::new(30.0, 0.0, 40.0));
        building.apply_upgrade_tag("UpgradeVeteranTraining");
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let restored_building = restored
        .host_object(barracks_id)
        .expect("restored barracks should exist");
    assert!(restored_building.has_upgrade_tag("UpgradeVeteranTraining"));
    let restored_data = restored_building
        .building_data
        .as_ref()
        .expect("restored barracks should keep building data");
    assert_eq!(restored_data.rally_point, Some(Vec3::new(30.0, 0.0, 40.0)));
    assert_eq!(restored_data.production_queue.len(), 1);
    let item = &restored_data.production_queue[0];
    assert_eq!(item.template_name, "USA_Ranger");
    assert_eq!(item.cost.supplies, 225);
    assert_eq!(item.total_time, 12.0);
    assert!((item.progress - 4.5).abs() < 0.001);
}

#[test]
fn snapshot_player_state_captures_population_build_queue_and_research() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(3, Team::USA, "Commander", true));
    {
        let player = source
            .get_player_mut(3)
            .expect("player should exist for state setup");
        player
            .unlocked_sciences
            .insert("SciencePathfinder".to_string());
        player
            .queued_upgrades
            .insert("UpgradeAdvancedTraining".to_string());
    }

    let mut barracks = ThingTemplate::new("USA_Barracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable);
    source
        .templates
        .insert("USA_Barracks".to_string(), barracks.clone());

    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_cost(225, 0);
    ranger.build_time = 8.0;
    source.templates.insert("USA_Ranger".to_string(), ranger);

    let barracks_id = source
        .create_object("USA_Barracks", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("failed to create barracks");
    source
        .create_object("USA_Ranger", Team::USA, Vec3::new(8.0, 0.0, 8.0))
        .expect("failed to create ranger");
    assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));
    assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    let player_snapshot = snapshot
        .players
        .iter()
        .find(|p| p.id == 3)
        .expect("player snapshot should exist");

    assert_eq!(player_snapshot.population.current, 1);
    assert_eq!(
        player_snapshot.build_queue,
        vec!["USA_Ranger".to_string(), "USA_Ranger".to_string()]
    );
    assert!(player_snapshot
        .tech_tree
        .unlocked_buildings
        .contains(&"USA_Barracks".to_string()));
    assert!(player_snapshot
        .tech_tree
        .unlocked_units
        .contains(&"USA_Ranger".to_string()));
    assert!(player_snapshot
        .tech_tree
        .unlocked_upgrades
        .contains(&"SciencePathfinder".to_string()));
    assert!(player_snapshot
        .research_queue
        .contains(&"UpgradeAdvancedTraining".to_string()));
    assert!(player_snapshot
        .tech_tree
        .research_progress
        .contains_key("UpgradeAdvancedTraining"));
}

#[test]
fn snapshot_restore_preserves_weather_state() {
    let mut source = GameLogic::new();
    source.set_weather_state("sandstorm", 0.7, 90.0, 30.0);
    source.set_weather_visible(false);

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    assert_eq!(snapshot.weather.current_weather, "sandstorm");
    assert!((snapshot.weather.weather_intensity - 0.7).abs() < 0.0001);
    assert!((snapshot.weather.weather_duration - 90.0).abs() < 0.0001);
    assert!((snapshot.weather.next_weather_change - 30.0).abs() < 0.0001);
    assert!(!snapshot.weather.visible);

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");
    let weather = restored.weather_state();
    assert_eq!(weather.current_weather, "sandstorm");
    assert!((weather.intensity - 0.7).abs() < 0.0001);
    assert!((weather.duration_remaining - 90.0).abs() < 0.0001);
    assert!((weather.next_change_time - 30.0).abs() < 0.0001);
    assert!(!weather.visible);
}

#[test]
fn snapshot_restore_rehydrates_paths_from_pathfinding_cache() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("TestMover".to_string(), ThingTemplate::new("TestMover"));

    let mover_id = source
        .create_object("TestMover", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("failed to create mover");
    {
        let mover = source
            .host_object_mut(mover_id)
            .expect("mover should exist for setup");
        mover.status.moving = true;
        mover.movement.target_position = Some(Vec3::new(21.0, 0.0, 11.0));
        mover.movement.path = vec![
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(11.0, 0.0, 6.0),
            Vec3::new(21.0, 0.0, 11.0),
        ];
    }

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    assert_eq!(snapshot.pathfinding_cache.cached_paths.len(), 1);
    {
        let mover_snap = snapshot
            .objects
            .get_mut(&mover_id)
            .expect("mover snapshot should exist");
        mover_snap.movement.path.clear();
        mover_snap.movement.current_path_index = 0;
        mover_snap.status.moving = false;
    }

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let mover = restored
        .host_object(mover_id)
        .expect("restored mover should exist");
    assert_eq!(mover.movement.path.len(), 3);
    assert_eq!(mover.movement.path[0], Vec3::new(1.0, 0.0, 1.0));
    assert_eq!(mover.movement.path[2], Vec3::new(21.0, 0.0, 11.0));
    assert!(mover.status.moving);
    assert_eq!(mover.ai_state, AIState::Moving);
}

/// Residual: secondary_weapon + active_weapon_slot must survive snapshot save/load.
/// Prior gap: capture only stored primary in `weapons[0]`, restore left secondary None.
#[test]
fn snapshot_restore_preserves_secondary_weapon_and_active_slot() {
    let mut source = GameLogic::new();
    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    source.templates.insert("USA_Ranger".to_string(), ranger);

    let ranger_id = source
        .create_object("USA_Ranger", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("failed to create ranger");

    let primary = Weapon {
        damage: 25.0,
        range: 120.0,
        min_range: 0.0,
        reload_time: 0.5,
        last_fire_time: 12.5,
        ammo: Some(28),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    };
    let secondary = Weapon {
        damage: 80.0,
        range: 90.0,
        min_range: 5.0,
        reload_time: 2.0,
        last_fire_time: 3.25,
        ammo: Some(4),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 40.0,
        pre_attack_delay: 0.1,
        splash_radius: 0.0,
    };

    {
        let unit = source
            .host_object_mut(ranger_id)
            .expect("ranger should exist");
        unit.weapon = Some(primary.clone());
        unit.secondary_weapon = Some(secondary.clone());
        unit.active_weapon_slot = 1;
        unit.apply_upgrade_tag("Upgrade_AmericaRangerFlashBangGrenade");
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");

    let snap_obj = snapshot
        .objects
        .get(&ranger_id)
        .expect("ranger snapshot should exist");
    assert_eq!(
        snap_obj.weapons.len(),
        2,
        "secondary must be encoded as weapons[1]"
    );
    assert!((snap_obj.weapons[0].damage - primary.damage).abs() < f32::EPSILON);
    assert!((snap_obj.weapons[1].damage - secondary.damage).abs() < f32::EPSILON);
    assert!((snap_obj.weapons[1].last_fire_time - secondary.last_fire_time).abs() < 0.0001);
    assert_eq!(snap_obj.status.active_weapon_slot, 1);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");

    let unit = restored
        .host_object(ranger_id)
        .expect("restored ranger should exist");
    let restored_primary = unit
        .weapon
        .as_ref()
        .expect("primary weapon must survive load");
    let restored_secondary = unit
        .secondary_weapon
        .as_ref()
        .expect("secondary weapon must survive load");

    assert!((restored_primary.damage - primary.damage).abs() < f32::EPSILON);
    assert!((restored_primary.last_fire_time - primary.last_fire_time).abs() < 0.0001);
    assert_eq!(restored_primary.ammo, primary.ammo);

    assert!((restored_secondary.damage - secondary.damage).abs() < f32::EPSILON);
    assert!((restored_secondary.range - secondary.range).abs() < f32::EPSILON);
    assert!((restored_secondary.min_range - secondary.min_range).abs() < f32::EPSILON);
    assert!((restored_secondary.reload_time - secondary.reload_time).abs() < f32::EPSILON);
    assert!(
        (restored_secondary.last_fire_time - secondary.last_fire_time).abs() < 0.0001,
        "secondary last_fire_time must survive or reload timing desyncs"
    );
    assert_eq!(restored_secondary.ammo, secondary.ammo);
    assert!(
        (restored_secondary.projectile_speed - secondary.projectile_speed).abs() < f32::EPSILON
    );
    assert_eq!(unit.active_weapon_slot, 1);
    assert!(unit.has_upgrade_tag("Upgrade_AmericaRangerFlashBangGrenade"));
}

#[test]
fn snapshot_restore_preserves_secondary_only_weapon_slot() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("TestUnit".to_string(), ThingTemplate::new("TestUnit"));

    let id = source
        .create_object("TestUnit", Team::USA, Vec3::ZERO)
        .expect("create unit");
    let secondary = Weapon {
        damage: 50.0,
        range: 75.0,
        min_range: 0.0,
        reload_time: 1.0,
        last_fire_time: 9.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 100.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    };
    {
        let unit = source.host_object_mut(id).expect("unit");
        unit.weapon = None;
        unit.secondary_weapon = Some(secondary.clone());
        unit.active_weapon_slot = 1;
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");

    let unit = restored.host_object(id).expect("restored unit");
    assert!(
        unit.weapon.is_none(),
        "pad primary must not become a real primary weapon"
    );
    let sec = unit
        .secondary_weapon
        .as_ref()
        .expect("secondary-only must restore");
    assert!((sec.damage - 50.0).abs() < f32::EPSILON);
    assert!((sec.last_fire_time - 9.0).abs() < 0.0001);
    assert_eq!(unit.active_weapon_slot, 1);
}

#[test]
fn snapshot_weapon_layout_helpers_round_trip() {
    let primary = Weapon {
        damage: 10.0,
        range: 50.0,
        ..Weapon::default()
    };
    let secondary = Weapon {
        damage: 99.0,
        range: 40.0,
        last_fire_time: 1.5,
        ..Weapon::default()
    };

    // Both slots
    let mut obj = Object::new(ThingTemplate::new("T"), ObjectId(1), Team::USA);
    obj.weapon = Some(primary.clone());
    obj.secondary_weapon = Some(secondary.clone());
    let weapons = SnapshotBuilder::snapshot_object_weapons(&obj);
    let (p, s) = SnapshotBuilder::restore_object_weapons(&weapons);
    assert!((p.unwrap().damage - 10.0).abs() < f32::EPSILON);
    assert!((s.unwrap().damage - 99.0).abs() < f32::EPSILON);

    // Primary only (legacy)
    let weapons = vec![primary.clone()];
    let (p, s) = SnapshotBuilder::restore_object_weapons(&weapons);
    assert!(p.is_some());
    assert!(s.is_none());

    // Empty
    let (p, s) = SnapshotBuilder::restore_object_weapons(&[]);
    assert!(p.is_none() && s.is_none());
}

/// End-to-end SaveFileManager path: secondary stays bound after save → load.
#[test]
fn save_file_roundtrip_preserves_secondary_weapon() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("save manager init");

    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("SaveSecondaryRanger");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    source
        .templates
        .insert("SaveSecondaryRanger".to_string(), template);

    let id = source
        .create_object("SaveSecondaryRanger", Team::USA, Vec3::new(12.0, 0.0, 8.0))
        .expect("create ranger");
    {
        let unit = source.host_object_mut(id).expect("ranger");
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            last_fire_time: 1.0,
            ..Weapon::default()
        });
        unit.secondary_weapon = Some(Weapon {
            damage: 55.0,
            range: 80.0,
            reload_time: 1.5,
            last_fire_time: 4.5,
            ammo: Some(2),
            ..Weapon::default()
        });
        unit.active_weapon_slot = 1;
    }

    let info = SaveGameInfo {
        filename: "secondary_weapon_rt".to_string(),
        display_name: "Secondary Weapon Roundtrip".to_string(),
        description: "residual secondary_weapon save/load".to_string(),
        map_name: "ResidualMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("secondary_weapon_rt", &source, &info)
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("secondary_weapon_rt", &mut loaded)
        .expect("load");

    let unit = loaded.host_object(id).expect("loaded unit");
    let secondary = unit
        .secondary_weapon
        .as_ref()
        .expect("secondary must remain bound after file load");
    assert!((secondary.damage - 55.0).abs() < f32::EPSILON);
    assert!((secondary.last_fire_time - 4.5).abs() < 0.0001);
    assert_eq!(secondary.ammo, Some(2));
    assert_eq!(unit.active_weapon_slot, 1);
    assert!(unit.weapon.is_some());
}

fn ensure_strike_test_tank(logic: &mut GameLogic) {
    let mut t = ThingTemplate::new("StrikeTestTank");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("StrikeTestTank".to_string(), t);
}

/// Residual: DaisyCutter queued mid-flight must survive snapshot and still
/// apply area damage once the restored impact frame is reached.
#[test]
fn special_power_daisy_cutter_mid_flight_save_load_still_impacts() {
    use crate::command_system::SpecialPowerType;

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);

    let caster_id = source
        .create_object("StrikeTestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = source.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }

    // Activate at frame 0 → DaisyCutter impact at frame 90.
    source.set_current_frame(0);
    let strike_id = source
        .queue_special_power_strike(
            &SpecialPowerType::DaisyCutter,
            caster_id,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("DaisyCutter must queue");

    // Mid-flight: save before impact.
    source.set_current_frame(45);
    source.update_special_power_strikes();
    assert_eq!(
        source.special_power_strikes().pending_count(),
        1,
        "strike must still be queued mid-flight"
    );
    assert!(source
        .special_power_strikes()
        .honesty_queue_ok(HostSuperweaponKind::DaisyCutter));
    let health_mid = source.host_object(enemy_id).unwrap().health.current;
    assert!((health_mid - 500.0).abs() < 0.1, "no damage mid-flight");

    // Combat particle residual from activation should be present for snapshot.
    assert!(
        source.combat_particles().system_count() >= 1,
        "activation should spawn combat particle residual"
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot mid-flight DaisyCutter");
    assert_eq!(snapshot.special_power_strikes.strikes.len(), 1);
    assert_eq!(
        snapshot.special_power_strikes.strikes[0].phase,
        HostStrikePhase::Queued
    );
    assert_eq!(snapshot.special_power_strikes.strikes[0].impact_frame, 90);
    assert!(
        !snapshot.combat_particles.systems.is_empty(),
        "combat particles must be captured in WorldSnapshot"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore mid-flight DaisyCutter");

    assert_eq!(restored.get_current_frame(), 45);
    assert_eq!(restored.special_power_strikes().pending_count(), 1);
    let restored_strike = restored
        .special_power_strikes()
        .get(strike_id)
        .expect("pending strike must survive load");
    assert_eq!(restored_strike.impact_frame, 90);
    assert_eq!(restored_strike.phase, HostStrikePhase::Queued);
    assert!(
        restored.combat_particles().system_count() >= 1,
        "combat particle registry must restore active systems"
    );

    // Still before impact after load: no damage.
    restored.set_current_frame(89);
    restored.update_special_power_strikes();
    assert!((restored.host_object(enemy_id).unwrap().health.current - 500.0).abs() < 0.1);
    assert!(!restored
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::DaisyCutter));

    // Impact after remaining delay: damage applied.
    restored.set_current_frame(90);
    restored.update_special_power_strikes();
    assert!(
        restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
        "DaisyCutter must complete after mid-flight load"
    );
    let enemy_after = restored.host_object(enemy_id).map(|o| o.health.current);
    assert!(
        enemy_after.is_none()
            || enemy_after == Some(0.0)
            || restored
                .host_object(enemy_id)
                .map(|o| o.status.destroyed || o.health.current < 500.0)
                .unwrap_or(true),
        "enemy must take DaisyCutter residual damage after load (got {enemy_after:?})"
    );
    let completed = restored
        .special_power_strikes()
        .get(strike_id)
        .expect("completed strike");
    assert_eq!(completed.phase, HostStrikePhase::Completed);
    assert!(completed.total_damage_applied > 0.0);
    assert!(completed.objects_hit >= 1);
}

/// Residual: A10 strike mid-flight save/load continues remaining delay and impacts.
#[test]
fn special_power_a10_mid_flight_save_load_still_impacts() {
    use crate::command_system::SpecialPowerType;

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);

    let caster_id = source
        .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
        .expect("caster");
    let enemy_id = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = source.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 200.0;
        enemy.health.maximum = 200.0;
        enemy.thing.template.armor = 0.0;
    }

    // A10 delay is 60 frames.
    source.set_current_frame(100);
    let strike_id = source
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            caster_id,
            Vec3::new(15.0, 0.0, 0.0),
        )
        .expect("A10 must queue");
    assert_eq!(
        source
            .special_power_strikes()
            .get(strike_id)
            .unwrap()
            .impact_frame,
        160
    );

    source.set_current_frame(130);
    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("A10 mid-flight snapshot");

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("A10 restore");

    assert_eq!(restored.get_current_frame(), 130);
    assert!(restored
        .special_power_strikes()
        .honesty_queue_ok(HostSuperweaponKind::A10Strike));

    restored.set_current_frame(159);
    restored.update_special_power_strikes();
    assert!((restored.host_object(enemy_id).unwrap().health.current - 200.0).abs() < 0.1);

    restored.set_current_frame(160);
    restored.update_special_power_strikes();
    assert!(
        restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::A10Strike),
        "A10 must complete after mid-flight load"
    );
    let health = restored
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        health < 200.0 || restored.host_object(enemy_id).is_none(),
        "A10 residual damage must apply post-load (health={health})"
    );
}

/// Bincode / SaveFileManager path also keeps pending strikes.
#[test]
fn save_file_roundtrip_preserves_pending_special_power_strike() {
    use crate::command_system::SpecialPowerType;
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("save manager init");

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);
    let caster = source
        .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
        .expect("caster");
    let enemy = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = source.host_object_mut(enemy).unwrap();
        e.health.current = 300.0;
        e.health.maximum = 300.0;
        e.thing.template.armor = 0.0;
    }
    source.set_current_frame(0);
    source
        .queue_special_power_strike(
            &SpecialPowerType::DaisyCutter,
            caster,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("queue");
    source.set_current_frame(30);

    let info = SaveGameInfo {
        filename: "special_power_strike_rt".to_string(),
        display_name: "Special Power Strike Roundtrip".to_string(),
        description: "residual pending strike save/load".to_string(),
        map_name: "ResidualMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("special_power_strike_rt", &source, &info)
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("special_power_strike_rt", &mut loaded)
        .expect("load");

    assert_eq!(loaded.get_current_frame(), 30);
    assert_eq!(loaded.special_power_strikes().pending_count(), 1);
    loaded.set_current_frame(90);
    loaded.update_special_power_strikes();
    assert!(
        loaded
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
        "file-loaded strike must complete"
    );
    let health = loaded
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        health < 300.0 || loaded.host_object(enemy).is_none(),
        "damage after file load (health={health})"
    );
}

fn ensure_upgrade_test_templates(logic: &mut GameLogic) {
    if !logic.templates.contains_key("TestInfantry") {
        let mut t = ThingTemplate::new("TestInfantry");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(80.0)
            .set_cost(100, 0);
        logic.templates.insert("TestInfantry".to_string(), t);
    }
    if !logic.templates.contains_key("TestBuilding") {
        let mut t = ThingTemplate::new("TestBuilding");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1200.0)
            .set_cost(500, -1);
        logic.templates.insert("TestBuilding".to_string(), t);
    }
    if !logic.templates.contains_key("TestBarracks") {
        let mut t = ThingTemplate::new("TestBarracks");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBarracks)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(600, -1);
        logic.templates.insert("TestBarracks".to_string(), t);
    }
}

/// Residual: CaptureBuilding queued mid-flight must survive snapshot and still
/// complete with capture unlock after load.
#[test]
fn host_upgrade_capture_mid_flight_save_load_completes_unlock() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        HostUpgradeKind, HostUpgradePhase, UPGRADE_INFANTRY_CAPTURE,
    };

    let mut source = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    source.add_player(player);
    ensure_upgrade_test_templates(&mut source);

    let barracks_id = source
        .create_object("TestBarracks", Team::USA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");
    // Stand outside the barracks/building static path footprint so post-load
    // CaptureBuilding can A* (same live gap as 12-unit spawn sitting inside
    // the structure block). Upgrade residual is what this test persists.
    let captor_id = source
        .create_object("TestInfantry", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("captor");
    let building_id = source
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    // Queue capture research; do NOT update yet (mid-flight residual window).
    source.set_current_frame(20);
    source.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    source.process_commands();

    assert!(
        source
            .get_player(0)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player research queue must hold Capture mid-flight"
    );
    assert!(
        !source
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must not unlock before research completes"
    );
    assert_eq!(source.host_upgrades().pending_count(), 1);
    assert!(
        source
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host residual must record pending Capture research"
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot mid-flight Capture upgrade");
    assert_eq!(snapshot.host_upgrades.entries.len(), 1);
    assert_eq!(
        snapshot.host_upgrades.entries[0].phase,
        HostUpgradePhase::Queued
    );
    assert_eq!(
        snapshot.host_upgrades.entries[0].kind,
        HostUpgradeKind::CaptureBuilding
    );
    assert!(
        snapshot.players.iter().any(|p| p
            .research_queue
            .iter()
            .any(|n| n.contains("Capture") || n == UPGRADE_INFANTRY_CAPTURE)),
        "player research_queue must also capture in-flight upgrade"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore mid-flight Capture upgrade");

    assert_eq!(restored.get_current_frame(), 20);
    assert_eq!(restored.host_upgrades().pending_count(), 1);
    assert!(
        restored
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host registry pending Capture must survive load"
    );
    assert!(
        restored
            .get_player(0)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player queued upgrade must survive load"
    );
    assert!(
        !restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must still be mid-research after load"
    );

    // Complete research after load.
    restored.update();

    assert!(
        restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture unlock must complete after mid-flight load"
    );
    assert!(
        restored
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "registry must record Capture complete after load"
    );
    assert!(
        restored.host_upgrades().honesty_capture_unlock_ok(),
        "capture unlock honesty after load"
    );
    assert!(
        restored
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CaptureBuilding),
        "host path honesty for Capture after load"
    );
    let captor = restored
        .host_object(captor_id)
        .expect("captor after complete");
    assert!(
        captor.has_upgrade_tag(UPGRADE_INFANTRY_CAPTURE),
        "captor must receive capture upgrade tag after post-load complete"
    );

    // Ability now available.
    restored.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    restored.process_commands();
    let captor = restored
        .host_object(captor_id)
        .expect("captor after unlock");
    assert_eq!(
        captor.ai_state,
        AIState::Capturing,
        "CaptureBuilding must work after mid-flight save/load + complete"
    );
}

/// Bincode / SaveFileManager path also keeps pending host upgrade research.
#[test]
fn save_file_roundtrip_preserves_pending_host_upgrade() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_INFANTRY_CAPTURE};
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("save manager init");

    let mut source = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    source.add_player(player);
    ensure_upgrade_test_templates(&mut source);
    let barracks = source
        .create_object("TestBarracks", Team::USA, Vec3::ZERO)
        .expect("barracks");
    source.set_current_frame(5);
    source.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![barracks],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    source.process_commands();
    assert_eq!(source.host_upgrades().pending_count(), 1);

    let info = SaveGameInfo {
        filename: "host_upgrade_rt".to_string(),
        display_name: "Host Upgrade Roundtrip".to_string(),
        description: "residual pending upgrade save/load".to_string(),
        map_name: "ResidualMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("host_upgrade_rt", &source, &info)
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("host_upgrade_rt", &mut loaded)
        .expect("load");

    assert_eq!(loaded.get_current_frame(), 5);
    assert_eq!(loaded.host_upgrades().pending_count(), 1);
    assert!(loaded
        .host_upgrades()
        .honesty_queue_ok(HostUpgradeKind::CaptureBuilding));
    loaded.update();
    assert!(
        loaded
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "file-loaded pending upgrade must complete"
    );
    assert!(
        loaded
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "file-loaded registry must record complete"
    );
}

/// Wave 79: Drawable residual camo_stealth_look survives snapshot capture/restore.
#[test]
fn drawable_camo_stealth_look_snapshot_residual_wave79() {
    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("CamoDrawableSnap");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable);
    source.templates.insert("CamoDrawableSnap".into(), template);
    let id = source
        .create_object("CamoDrawableSnap", Team::GLA, glam::Vec3::ZERO)
        .expect("create");
    {
        let obj = source./* Wave 950 */ host_object_mut(id).expect("obj");
        // HostCamoStealthLook::VisibleDetected = 3
        obj.camo_stealth_look = 3;
        obj.status.stealthed = true;
        obj.status.detected = true;
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert_eq!(obj_snap.status.camo_stealth_look, 3);
    assert!(obj_snap.status.stealthed);
    assert!(obj_snap.status.detected);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    let obj = restored.host_object(id).expect("restored obj");
    assert_eq!(obj.camo_stealth_look, 3);
    assert!(obj.status.stealthed);
    assert!(obj.status.detected);
    assert!(honesty_drawable_residual_fields_wave79_ok());
}

/// Popup and host write the same Common CHUNK_*.sav tokens. Load restores
/// into the store `host_authoritative_*` reads.
#[test]
fn popup_and_host_write_common_sav_chunks_and_restore_authority() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::fs;
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");

    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("AuthTank");
    template
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    source.templates.insert("AuthTank".into(), template);
    source.add_player(Player::new(1, Team::USA, "P1", true));
    let id = source
        .create_object("AuthTank", Team::USA, Vec3::new(12.0, 0.0, 8.0))
        .expect("create");
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.health.current = 77.0;
        obj.health.maximum = 200.0;
    }

    let info = SaveGameInfo {
        filename: "auth_rt".to_string(),
        display_name: "Auth".to_string(),
        description: "host authoritative restore".to_string(),
        map_name: "AuthMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("auth_rt", &source, &info)
        .expect("save");

    let path = manager.get_save_path("auth_rt");
    assert!(
        path.extension().and_then(|e| e.to_str()) == Some("sav"),
        "host must write .sav like Popup"
    );
    let bytes = fs::read(&path).expect("read sav");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("CHUNK_GameState") && text.contains("CHUNK_GameLogic") && text.contains("SG_EOF"),
        "host file must use the same Common .sav chunk tokens as Popup"
    );

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager.load_game("auth_rt", &mut loaded).expect("load");

    let hp = loaded
        .host_authoritative_health(id)
        .expect("authoritative HP");
    assert!((hp - 77.0).abs() < 0.01, "restored HP must be host_authoritative, got {hp}");
    let pose = loaded
        .host_authoritative_pose(id)
        .expect("authoritative pose");
    assert!((pose[0] - 12.0).abs() < 0.01 && (pose[2] - 8.0).abs() < 0.01);
}
