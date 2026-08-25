//! Behavior suite extracted from the original test module.
use super::*;

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
fn mid_frenzy_snapshot_restores_weapon_bonus_state() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "FrenzyInfantry".to_string(),
        ThingTemplate::new("FrenzyInfantry"),
    );
    source.add_player(Player::new(1, Team::China, "China", true));
    source.set_current_frame(120);

    let object_id = source
        .create_object("FrenzyInfantry", Team::China, Vec3::new(4.0, 0.0, 8.0))
        .expect("create frenzy unit");
    {
        let object = source.host_object_mut(object_id).expect("created object");
        object.apply_weapon_bonus_frenzy(2, 720);
        assert!(object.weapon_bonus_frenzy);
        assert_eq!(object.weapon_bonus_frenzy_level, 2);
        assert_eq!(object.weapon_bonus_frenzy_until_frame, 720);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let captured = snapshot
        .objects
        .get(&object_id)
        .expect("object in snapshot");
    assert!(captured.weapon_bonus_frenzy);
    assert_eq!(captured.weapon_bonus_frenzy_level, 2);
    assert_eq!(captured.weapon_bonus_frenzy_until_frame, 720);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let loaded = restored.host_object(object_id).expect("restored object");
    assert!(loaded.weapon_bonus_frenzy);
    assert_eq!(loaded.weapon_bonus_frenzy_level, 2);
    assert_eq!(loaded.weapon_bonus_frenzy_until_frame, 720);
    assert!(loaded.is_frenzy_buffed());
}

#[test]
fn omitted_frenzy_tail_defaults_inactive() {
    let snapshot = super::xfer_helpers::default_object_snapshot();
    assert!(!snapshot.weapon_bonus_frenzy);
    assert_eq!(snapshot.weapon_bonus_frenzy_level, 0);
    assert_eq!(snapshot.weapon_bonus_frenzy_until_frame, 0);
}

#[test]
fn object_status_bits_survive_snapshot_restore() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("Tomahawk".to_string(), ThingTemplate::new("Tomahawk"));
    source.add_player(Player::new(1, Team::USA, "PlayerOne", true));
    let object_id = source
        .create_object("Tomahawk", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("create deployed unit");
    {
        let object = source.host_object_mut(object_id).expect("created object");
        object.set_status_unselectable(true);
        object.set_deployed(true);
        object.set_script_disabled(true);
        object.set_script_underpowered(true);
        object.set_script_unsellable(true);
        object.set_script_unstealthed(true);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let captured = snapshot
        .objects
        .get(&object_id)
        .expect("object in snapshot");
    assert!(captured.status.unselectable);
    assert!(captured.status.deployed);
    assert!(captured.status.disabled_script_disabled);
    assert!(captured.status.disabled_script_underpowered);
    assert!(captured.status.script_unsellable);
    assert!(captured.status.script_unstealthed);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let loaded = restored.host_object(object_id).expect("restored object");
    assert!(loaded.status.unselectable);
    assert!(loaded.is_deployed());
    assert!(loaded.is_script_disabled());
    assert!(loaded.is_script_underpowered());
    assert!(loaded.is_script_unsellable());
    assert!(loaded.is_script_unstealthed());
}

#[test]
fn script_held_and_attitude_survive_snapshot_restore() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("Ranger".to_string(), ThingTemplate::new("Ranger"));
    source.add_player(Player::new(1, Team::USA, "PlayerOne", true));
    let object_id = source
        .create_object("Ranger", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("create held unit");
    {
        let object = source.host_object_mut(object_id).expect("created object");
        object.set_status_disabled_held(true);
        object.ai_attitude = -1; // Passive
        object.is_receiving_difficulty_bonus = true;
        object.weapon_bonus_solo = 16;
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let captured = snapshot
        .objects
        .get(&object_id)
        .expect("object in snapshot");
    assert!(captured.status.disabled_held);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let loaded = restored.host_object(object_id).expect("restored object");
    assert!(
        loaded.status.disabled_held,
        "script DISABLED_HELD must survive load"
    );
    assert!(!loaded.can_move());
    assert_eq!(loaded.ai_attitude, -1, "Passive attitude must survive load");
    assert!(
        loaded.is_receiving_difficulty_bonus,
        "difficulty latch must survive load"
    );
    assert_eq!(loaded.weapon_bonus_solo, 16);
}

#[test]
fn omitted_object_status_bits_default_inactive() {
    let status = ObjectStatusSnapshot::default();
    assert!(!status.unselectable);
    assert!(!status.deployed);
    assert!(!status.disabled_script_disabled);
    assert!(!status.disabled_script_underpowered);
    assert!(!status.script_unsellable);
    assert!(!status.script_unstealthed);
    assert!(!status.disabled_paralyzed);
    assert_eq!(status.disabled_paralyzed_until_frame, 0);
    assert_eq!(status.spy_vision_disabled_until_frame, 0);
    assert!(!status.spy_vision_reset_timers);
    assert_eq!(status.spy_vision_hack_two_wake_frame, 0);
    assert!(!status.parachuting);
    assert!(!status.parachute_open);
    assert_eq!(status.parachute_start_height, 0.0);
    assert!(status.parachute_landing_override.is_none());
    assert!(!status.parachute_landing_override_set);
    assert!(!status.faerie_fire);
    assert_eq!(status.faerie_fire_until_frame, 0);
    assert!(!status.disabled_held);
}

#[test]
fn snapshot_v5_restores_exact_human_and_ai_skirmish_template_bindings() {
    game_engine::common::ini::ensure_player_templates_loaded();
    let (laser_index, tank_index) = {
        let store = game_engine::common::rts::player_template::get_player_template_store();
        (
            store
                .find_template_index("FactionAmericaLaserGeneral")
                .expect("retail Laser General") as i32,
            store
                .find_template_index("FactionChinaTankGeneral")
                .expect("retail Tank General") as i32,
        )
    };

    let laser =
        PlayerTemplateIdentity::from_exact_indexed_name("FactionAmericaLaserGeneral", laser_index)
            .expect("exact human selection");
    let tank =
        PlayerTemplateIdentity::from_exact_indexed_name("FactionChinaTankGeneral", tank_index)
            .expect("exact AI selection");

    let mut source = GameLogic::new();
    source.add_player(Player::new(0, Team::USA, "Human", true));
    source.add_player(Player::new(1, Team::China, "Computer", false));
    assert!(source.bind_player_template_identity(0, laser.clone()));
    assert!(source.bind_player_template_identity(1, tank.clone()));
    source.get_player_mut(0).expect("human").resources.supplies = 4_321;
    source.get_player_mut(1).expect("AI").resources.supplies = 1_234;

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("v5 snapshot");
    assert_eq!(snapshot.player_template_bindings.len(), 2);
    assert_eq!(snapshot.player_template_bindings[0].player_id, 0);
    assert_eq!(
        snapshot.player_template_bindings[0].template_index,
        laser_index
    );
    assert_eq!(snapshot.player_template_bindings[1].player_id, 1);
    assert_eq!(
        snapshot.player_template_bindings[1].template_index,
        tank_index
    );

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("v5 restore");
    assert_eq!(
        restored
            .player_template_identity(0)
            .expect("restored human binding"),
        &laser
    );
    assert_eq!(
        restored
            .player_template_identity(1)
            .expect("restored AI binding"),
        &tank
    );
    assert_eq!(
        restored.get_player(0).expect("human").resources.supplies,
        4_321,
        "restore must install identity only, without replaying template start cash"
    );
    assert_eq!(
        restored.get_player(1).expect("AI").resources.supplies,
        1_234,
        "Random was resolved before save and must not be selected again on load"
    );
}

#[test]
fn snapshot_v4_defaults_to_no_template_bindings_and_v5_rejects_stale_pair() {
    let mut legacy = WorldSnapshot::default();
    legacy.version = 4;
    legacy.players.push(PlayerSnapshot {
        id: 0,
        name: "Legacy".to_string(),
        team: Team::USA,
        is_human: true,
        is_active: true,
        resources: Resources::default(),
        population: PopulationInfo {
            current: 0,
            maximum: 0,
        },
        tech_tree: TechTreeSnapshot {
            unlocked_units: Vec::new(),
            unlocked_buildings: Vec::new(),
            unlocked_upgrades: Vec::new(),
            research_progress: Default::default(),
        },
        upgrades: Vec::new(),
        build_queue: Vec::new(),
        research_queue: Vec::new(),
        statistics: PlayerStatisticsSnapshot {
            units_built: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_lost: 0,
            damage_dealt: 0.0,
            damage_received: 0.0,
            resources_gathered: 0,
            experience_gained: 0.0,
        },
    });
    let builder = SnapshotBuilder::new();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&legacy, &mut restored)
        .expect("v4 predecessor defaults binding tail");
    assert!(restored.player_template_identity(0).is_none());

    let mut stale = legacy;
    stale.version = WORLD_SNAPSHOT_BINCODE_VERSION;
    stale
        .player_template_bindings
        .push(PlayerTemplateBindingSnapshot {
            player_id: 0,
            template_name: "FactionAmericaLaserGeneral".to_string(),
            template_index: -1,
        });
    let mut rejected = GameLogic::new();
    assert!(
        builder
            .restore_from_snapshot(&stale, &mut rejected)
            .is_err(),
        "stale name/index pairs must fail closed rather than choose a General"
    );
    assert!(rejected.player_template_identity(0).is_none());
}

#[test]
fn snapshot_restore_preserves_registered_host_ai_configuration() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(0, Team::USA, "Human", true));
    source.add_player(Player::new(1, Team::China, "Computer", false));
    source.add_ai_opponent(1, Team::China, AIDifficulty::Hard);
    source.set_ai_active(1, false);
    source.relocate_host_ai_base(1, Vec3::new(47.0, 0.0, -31.0));

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    let saved_ai = snapshot
        .ai_players
        .iter()
        .find(|ai| ai.player_id == 1)
        .expect("registered host AI must be serialized");
    assert_eq!(saved_ai.difficulty, "Hard");
    assert!(!saved_ai.is_active);
    assert_eq!(saved_ai.base_center, Some(Vec3::new(47.0, 0.0, -31.0)));

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");
    assert_eq!(restored.host_ai_difficulty(1), Some(AIDifficulty::Hard));
    assert!(!restored.is_host_ai_active(1));

    let restored_snapshot = builder
        .create_world_snapshot(&restored)
        .expect("re-snapshot restored host AI");
    let restored_ai = restored_snapshot
        .ai_players
        .iter()
        .find(|ai| ai.player_id == 1)
        .expect("restored host AI must remain registered");
    assert_eq!(restored_ai.base_center, saved_ai.base_center);
    assert_eq!(restored_ai.current_strategy, saved_ai.current_strategy);
    assert_eq!(
        restored_ai.strategic_state.current_phase,
        saved_ai.strategic_state.current_phase
    );
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
fn snapshot_restore_rebuilds_logic_u8_heights_like_cpp_visual_xfer() {
    // C++ W3DTerrainVisual::xfer v>=2 (W3DTerrainVisual.cpp:1231-1247)
    // persists raw u8 logic heights, not only path-grid f32 samples.
    {
        let mut terrain = gamelogic::terrain::get_terrain_logic()
            .write()
            .expect("terrain logic");
        terrain.restore_logic_height_map(2, 2, &[10, 20, 30, 40]);
    }

    let source = GameLogic::new();
    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    assert_eq!(snapshot.terrain.logic_width, 2);
    assert_eq!(snapshot.terrain.logic_height, 2);
    assert_eq!(snapshot.terrain.logic_heights, vec![10, 20, 30, 40]);

    {
        let mut terrain = gamelogic::terrain::get_terrain_logic()
            .write()
            .expect("terrain logic");
        terrain.restore_logic_height_map(2, 2, &[0, 0, 0, 0]);
    }
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");
    let bytes = gamelogic::terrain::get_terrain_logic()
        .read()
        .expect("terrain logic")
        .logic_height_map_bytes()
        .to_vec();
    assert_eq!(bytes, vec![10, 20, 30, 40]);
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
        // C++ ProductionUpdate snapshots the authoritative integer counter,
        // not just this presentation-facing float.
        building_data.production_queue[0].construction_frames = 135;
        // Save after one member of a source-backed Queue modifier batch: both
        // remaining quantity and the per-Object delay/burst state must survive
        // rather than rebuilding an arbitrary fresh queue after load.
        building_data.production_queue[0].quantity_total = 2;
        building_data.production_queue[0].quantity_produced = 1;
        building_data.exit_delay_remaining = 0.3;
        building_data.exit_delay_remaining_frames = 9;
        building_data.exit_burst_remaining = 0;
        building_data.queue_exit_state_initialized = true;
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
    assert_eq!(item.construction_frames, 135);
    assert_eq!(item.quantity_total, 2);
    assert_eq!(item.quantity_produced, 1);
    assert!(!item.is_upgrade());
    assert_eq!(restored_data.exit_delay_remaining_frames, 9);
    assert_eq!(restored_data.exit_burst_remaining, 0);
    assert!(restored_data.queue_exit_state_initialized);
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
    assert!(
        player_snapshot
            .tech_tree
            .unlocked_buildings
            .contains(&"USA_Barracks".to_string())
    );
    assert!(
        player_snapshot
            .tech_tree
            .unlocked_units
            .contains(&"USA_Ranger".to_string())
    );
    assert!(
        player_snapshot
            .tech_tree
            .unlocked_upgrades
            .contains(&"SciencePathfinder".to_string())
    );
    assert!(
        player_snapshot
            .research_queue
            .contains(&"UpgradeAdvancedTraining".to_string())
    );
    assert!(
        player_snapshot
            .tech_tree
            .research_progress
            .contains_key("UpgradeAdvancedTraining")
    );
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
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
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
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
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
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
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
    let (p, s, t) = SnapshotBuilder::restore_object_weapons(&weapons);
    assert!((p.unwrap().damage - 10.0).abs() < f32::EPSILON);
    assert!((s.unwrap().damage - 99.0).abs() < f32::EPSILON);
    assert!(t.is_none());

    // Primary only (legacy)
    let weapons = vec![primary.clone()];
    let (p, s, t) = SnapshotBuilder::restore_object_weapons(&weapons);
    assert!(p.is_some());
    assert!(s.is_none());
    assert!(t.is_none());

    // Empty
    let (p, s, t) = SnapshotBuilder::restore_object_weapons(&[]);
    assert!(p.is_none() && s.is_none() && t.is_none());
}

#[test]
fn snapshot_weapon_layout_preserves_tertiary_slot_and_active_identity() {
    let primary = Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    };
    let secondary = Weapon {
        damage: 20.0,
        range: 120.0,
        ..Weapon::default()
    };
    let tertiary = Weapon {
        damage: 30.0,
        range: 200.0,
        last_fire_time: 4.0,
        ammo: Some(19),
        ..Weapon::default()
    };
    let mut object = Object::new(ThingTemplate::new("ThreeSlotUnit"), ObjectId(7), Team::USA);
    object.weapon = Some(primary);
    object.secondary_weapon = Some(secondary);
    object.tertiary_weapon = Some(tertiary.clone());
    object.active_weapon_slot = 2;

    let weapons = SnapshotBuilder::snapshot_object_weapons(&object);
    assert_eq!(weapons.len(), 3);
    let (restored_primary, restored_secondary, restored_tertiary) =
        SnapshotBuilder::restore_object_weapons(&weapons);
    assert!(restored_primary.is_some());
    assert!(restored_secondary.is_some());
    let restored_tertiary = restored_tertiary.expect("tertiary must stay at index 2");
    assert!((restored_tertiary.damage - tertiary.damage).abs() < f32::EPSILON);
    assert_eq!(restored_tertiary.ammo, tertiary.ammo);
    assert!((restored_tertiary.last_fire_time - tertiary.last_fire_time).abs() < f32::EPSILON);
}

#[test]
fn snapshot_restore_preserves_tertiary_weapon_and_permanent_lock() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "ThreeSlotSave".to_string(),
        ThingTemplate::new("ThreeSlotSave"),
    );
    let id = source
        .create_object("ThreeSlotSave", Team::USA, Vec3::ZERO)
        .expect("create three-slot source");
    let tertiary = Weapon {
        damage: 73.0,
        range: 220.0,
        last_fire_time: 3.5,
        ammo: Some(18),
        ..Weapon::default()
    };
    {
        let object = source.host_object_mut(id).expect("source object");
        object.weapon = Some(Weapon {
            damage: 7.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.secondary_weapon = Some(Weapon {
            damage: 17.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.tertiary_weapon = Some(tertiary.clone());
        assert!(object.set_weapon_lock(2, WeaponLockType::LockedPermanently));
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");

    let object = restored.host_object(id).expect("restored object");
    assert_eq!(object.active_weapon_slot, 2);
    assert_eq!(object.weapon_lock_slot, 2);
    assert_eq!(object.weapon_lock_type, WeaponLockType::LockedPermanently);
    let restored_tertiary = object.tertiary_weapon.as_ref().expect("third slot");
    assert!((restored_tertiary.damage - tertiary.damage).abs() < f32::EPSILON);
    assert_eq!(restored_tertiary.ammo, tertiary.ammo);
    assert!((restored_tertiary.last_fire_time - tertiary.last_fire_time).abs() < f32::EPSILON);
}

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

#[test]
fn snapshot_roundtrip_stages_primary_and_secondary_barrel_cursors() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "BarrelCursorTank".to_string(),
        ThingTemplate::new("BarrelCursorTank"),
    );
    let id = source
        .create_object("BarrelCursorTank", Team::USA, Vec3::ZERO)
        .expect("create barrel cursor object");
    source.restore_weapon_discharge_next_sequence(43);
    {
        let object = source.host_object_mut(id).expect("source object");
        object.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.secondary_weapon = Some(Weapon {
            damage: 20.0,
            range: 80.0,
            ..Weapon::default()
        });
        object.weapon_barrel_states[0].current_barrel = 2;
        object.weapon_barrel_states[0].shots_left_on_barrel = 1;
        object.weapon_barrel_states[1].current_barrel = 1;
        object.weapon_barrel_states[1].shots_left_on_barrel = 1;
        assert!(object.restore_weapon_discharge_marker(42, 1, 1, 9_001));
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let cursor_snapshot = &snapshot.objects[&id].weapon_barrel_states;
    assert_eq!(cursor_snapshot[0].current_barrel, 2);
    assert_eq!(cursor_snapshot[1].current_barrel, 1);
    assert_eq!(snapshot.next_weapon_discharge_sequence, 43);
    assert_eq!(cursor_snapshot[1].shots_left_on_barrel, 1);
    let marker_snapshot = &snapshot.objects[&id];
    assert_eq!(marker_snapshot.last_weapon_discharge_sequence, 42);
    assert_eq!(marker_snapshot.last_weapon_discharge_slot, 1);
    assert_eq!(marker_snapshot.last_weapon_discharge_barrel, 1);
    assert_eq!(marker_snapshot.last_weapon_discharge_frame, 9_001);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let staged_resnapshot = builder
        .create_world_snapshot(&restored)
        .expect("re-snapshot before fresh topology is available");
    assert_eq!(
        staged_resnapshot.objects[&id].weapon_barrel_states[0], cursor_snapshot[0],
        "a save made before W3D topology validation must retain the raw staged primary cursor"
    );
    assert_eq!(
        staged_resnapshot.objects[&id].weapon_barrel_states[1], cursor_snapshot[1],
        "a save made before W3D topology validation must retain the raw staged secondary cursor"
    );
    let object = restored.host_object_mut(id).expect("restored object");
    // The restore call stored raw cursors while the host still had its safe
    // one-barrel fallback. Applying validated topology consumes them once.
    assert!(object.set_weapon_barrel_count_for_slot(0, 3));
    assert!(object.set_weapon_barrel_count_for_slot(1, 2));
    assert_eq!(
        object
            .weapon_barrel_state_for_slot(0)
            .expect("primary cursor")
            .current_barrel,
        2
    );
    assert_eq!(
        object
            .weapon_barrel_state_for_slot(1)
            .expect("secondary cursor")
            .current_barrel,
        1
    );
    assert_eq!(
        object.weapon_discharge_marker(),
        crate::game_logic::WeaponDischargeMarker {
            sequence: 42,
            weapon_slot: 1,
            fired_barrel: 1,
            logic_frame: 9_001,
        }
    );
    assert_eq!(restored.weapon_discharge_next_sequence_for_snapshot(), 43);
}

#[test]
fn snapshot_roundtrip_pristine_authored_shots_per_barrel_does_not_become_one() {
    let ini_content = r#"
Weapon __RustSnapshotPristineFiveShotWeapon
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 5
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(ini_content),
        1
    );

    let mut source = GameLogic::new();
    let mut template = ThingTemplate::new("SnapshotPristineFiveShotTank");
    template.set_primary_weapon_name("__RustSnapshotPristineFiveShotWeapon");
    source
        .templates
        .insert("SnapshotPristineFiveShotTank".to_string(), template);
    let id = source
        .create_object("SnapshotPristineFiveShotTank", Team::USA, Vec3::ZERO)
        .expect("create source weapon object");

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert_eq!(
        snapshot.objects[&id].weapon_barrel_states[0].shots_left_on_barrel, 0,
        "uninitialized Main cursor must serialize the v4 authored-cadence sentinel"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let restored = restored
        .host_object_mut(id)
        .expect("restored source weapon object");
    assert!(
        restored.set_weapon_barrel_count_for_slot(0, 2),
        "fresh validated topology may configure the restored first barrel"
    );
    assert_eq!(
        {
            let state = restored
                .weapon_barrel_state_for_slot(0)
                .expect("restored PRIMARY cursor");
            (
                state.current_barrel,
                state.shots_per_barrel,
                state.shots_left_on_barrel,
            )
        },
        (0, 5, 5),
        "a pristine five-shot Weapon must resume with its authored first barrel cadence"
    );
    for _ in 0..4 {
        restored.advance_weapon_barrel_after_shot(0);
    }
    assert_eq!(
        restored
            .weapon_barrel_state_for_slot(0)
            .expect("post-shot PRIMARY cursor")
            .current_barrel,
        0,
        "the first four shots remain on barrel zero"
    );
    restored.advance_weapon_barrel_after_shot(0);
    assert_eq!(
        restored
            .weapon_barrel_state_for_slot(0)
            .expect("fifth-shot PRIMARY cursor")
            .current_barrel,
        1,
        "the authored fifth shot advances to the next validated barrel"
    );
}

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
    assert!(
        source
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::DaisyCutter)
    );
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
    assert!(
        !restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter)
    );

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
    assert!(
        restored
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::A10Strike)
    );

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

    // C++ Upgrade.ini BuildTime=30s is 900 logic frames at 30 FPS.  A single
    // update must preserve the pending queue, not unlock it early.
    for _ in 0..899 {
        restored.update();
    }
    assert!(
        !restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture must remain locked before the authored 30-second duration"
    );
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
    // The 900-frame research interval intentionally advances the simulation
    // far beyond the original test's one-frame window.  Re-establish the
    // authored in-range capture setup before testing the now-unlocked ability;
    // this keeps the assertion about snapshot/research state independent of
    // autonomous movement during the elapsed research time.
    if let Some(captor) = restored.host_object_mut(captor_id) {
        captor.set_position(Vec3::ZERO);
        captor.target = None;
        captor.set_ai_state(AIState::Idle);
    }
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
    assert!(
        loaded
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding)
    );
    for _ in 0..900 {
        loaded.update();
    }
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

#[test]
fn stealth_detection_expires_frame_survives_snapshot_and_clears_detected() {
    let mut source = GameLogic::new();
    source.set_current_frame(50);
    let mut template = ThingTemplate::new("StealthExpirySnap");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable);
    source
        .templates
        .insert("StealthExpirySnap".into(), template);
    let id = source
        .create_object("StealthExpirySnap", Team::GLA, glam::Vec3::ZERO)
        .expect("create");
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.status.stealthed = true;
        obj.status.detected = true;
        obj.detection_expires_frame = 100;
        obj.stealth_allowed_frame = 80;
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert_eq!(obj_snap.status.detection_expires_frame, 100);
    assert_eq!(obj_snap.status.stealth_allowed_frame, 80);
    assert!(obj_snap.status.detected);
    assert!(obj_snap.status.stealthed);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    {
        let obj = restored.host_object(id).expect("restored obj");
        assert_eq!(obj.detection_expires_frame, 100);
        assert_eq!(obj.stealth_allowed_frame, 80);
        assert!(obj.status.detected);
        assert!(obj.status.stealthed);
    }

    // Before expiry: DETECTED must hold (C++ `m_detectionExpiresFrame > now`).
    restored.frame = 99;
    restored.update_stealth_and_detection();
    {
        let obj = restored.host_object(id).expect("pre-expiry");
        assert!(
            obj.status.detected,
            "DETECTED must hold before expiry frame"
        );
        assert_eq!(obj.detection_expires_frame, 100);
    }

    // At expiry: host gate `frame >= detection_expires_frame` clears DETECTED.
    restored.frame = 100;
    restored.update_stealth_and_detection();
    let obj = restored.host_object(id).expect("post-expiry");
    assert!(
        !obj.status.detected,
        "DETECTED must expire after load once detection_expires_frame is reached"
    );
    assert!(
        obj.status.stealthed,
        "stealth remains after detection expires"
    );
    assert_eq!(obj.detection_expires_frame, 0);
}

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
    template.add_kind_of(KindOf::Vehicle).set_health(200.0);
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
    manager.save_game("auth_rt", &source, &info).expect("save");

    let path = manager.get_save_path("auth_rt");
    assert!(
        path.extension().and_then(|e| e.to_str()) == Some("sav"),
        "host must write .sav like Popup"
    );
    let bytes = fs::read(&path).expect("read sav");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("CHUNK_GameState")
            && text.contains("CHUNK_GameLogic")
            && text.contains("SG_EOF"),
        "host file must use the same Common .sav chunk tokens as Popup"
    );

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager.load_game("auth_rt", &mut loaded).expect("load");

    let hp = loaded
        .host_authoritative_health(id)
        .expect("authoritative HP");
    assert!(
        (hp - 77.0).abs() < 0.01,
        "restored HP must be host_authoritative, got {hp}"
    );
    let pose = loaded
        .host_authoritative_pose(id)
        .expect("authoritative pose");
    assert!((pose[0] - 12.0).abs() < 0.01 && (pose[2] - 8.0).abs() < 0.01);
}

#[test]
fn companion_aware_save_preserves_client_drawable_snapshot() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");
    let client_drawables = ClientDrawableWorldSnapshot {
        drawables: vec![ClientDrawableStateSnapshot {
            object_id: 17,
            draw_module_index: 1,
            source_template_name: "CompanionTank".to_string(),
            model_key: "UVCompanion".to_string(),
            selected_condition_state_index: 3,
            animation: Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: "UVCompanion.UVCompanion".to_string(),
                frame: 8.25,
                mode: ClientDrawableAnimationMode::Loop,
            }),
            last_seen_weapon_discharge_sequence: 31,
            recoil_slots: [
                vec![ClientDrawableRecoilSnapshot {
                    phase: ClientDrawableRecoilPhase::Settle,
                    shift: 0.125,
                    recoil_rate: 0.75,
                }],
                Vec::new(),
                Vec::new(),
            ],
        }],
    };
    let save_info = SaveGameInfo {
        filename: "client_drawable_companion".to_string(),
        display_name: "Client Drawable Companion".to_string(),
        description: "v4 renderer companion".to_string(),
        map_name: "CompanionMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };

    manager
        .save_game_with_client_drawable_snapshot(
            "client_drawable_companion",
            &GameLogic::new(),
            client_drawables.clone(),
            &save_info,
        )
        .expect("save companion");
    let (snapshot, loaded_info) = manager
        .load_game_snapshot("client_drawable_companion")
        .expect("decode companion");
    assert_eq!(loaded_info.map_name, "CompanionMap");
    assert_eq!(snapshot.client_drawables, client_drawables);
}

#[test]
fn direct_xfer_v2_object_snapshot_omits_hacker_disable_tail_and_keeps_alignment() {
    use super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::game_logic::{HackerDisableChannelPhase, HackerDisableChannelState};
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(91);
    let mut pre_hdb_world = WorldSnapshot::default();
    pre_hdb_world.version = 2;
    pre_hdb_world.frame_number = 4_321;
    pre_hdb_world.random_seed = 0x1BAD_B002;

    let mut object = default_object_snapshot();
    object.id = object_id;
    object.template_name = "PreHdbDirectXferObject".to_string();
    object.hacker_disable_channel = Some(HackerDisableChannelState::new(
        ObjectId(92),
        HackerDisableChannelPhase::Preparing,
        1_500,
    ));
    pre_hdb_world.objects.insert(object_id, object);

    // A non-empty record after the object map ensures the loader has to stay
    // aligned through subsequent world fields, not merely reach EOF.
    let mut player = default_player_snapshot();
    player.id = 17;
    player.name = "PostObjectAlignment".to_string();
    pre_hdb_world.players.push(player);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        pre_hdb_world
            .xfer(&mut writer)
            .expect("write exact pre-HDB direct-Xfer world");
        let mut trailing_sentinel = 0xC0DE_CAFEu32;
        writer
            .xfer_u32(&mut trailing_sentinel)
            .expect("write trailing sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut trailing_sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored
            .xfer(&mut reader)
            .expect("read exact pre-HDB direct-Xfer world");
        reader
            .xfer_u32(&mut trailing_sentinel)
            .expect("read aligned trailing sentinel");
    }

    assert_eq!(restored.version, 2);
    assert_eq!(restored.frame_number, 4_321);
    assert_eq!(restored.random_seed, 0x1BAD_B002);
    assert_eq!(restored.players[0].name, "PostObjectAlignment");
    assert!(
        restored
            .objects
            .get(&object_id)
            .expect("restored v2 object")
            .hacker_disable_channel
            .is_none()
    );
    assert_eq!(
        restored
            .objects
            .get(&object_id)
            .expect("restored v2 object")
            .weapon_barrel_states,
        default_weapon_barrel_state_snapshots()
    );
    assert_eq!(
        restored.next_weapon_discharge_sequence,
        default_next_weapon_discharge_sequence()
    );
    assert!(restored.client_drawables.drawables.is_empty());
    assert_eq!(trailing_sentinel, 0xC0DE_CAFE);
}

#[test]
fn direct_xfer_v3_preserves_hacker_disable_tail_and_keeps_alignment_after_bincode_v4() {
    use super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::game_logic::{HackerDisableChannelPhase, HackerDisableChannelState};
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(93);
    let mut world = WorldSnapshot::default();
    world.version = 3;
    let mut object = default_object_snapshot();
    object.id = object_id;
    object.template_name = "V3HdbDirectXferObject".to_string();
    object.hacker_disable_channel = Some(HackerDisableChannelState::new(
        ObjectId(94),
        HackerDisableChannelPhase::Packing,
        777,
    ));
    world.objects.insert(object_id, object);
    let mut player = default_player_snapshot();
    player.id = 19;
    player.name = "V3PostObjectAlignment".to_string();
    world.players.push(player);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v3 world");
        let mut sentinel = 0xA3B4_C5D6u32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v3 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(restored.version, 3);
    assert_eq!(restored.players[0].name, "V3PostObjectAlignment");
    assert_eq!(
        restored
            .objects
            .get(&object_id)
            .and_then(|object| object.hacker_disable_channel),
        Some(HackerDisableChannelState::new(
            ObjectId(94),
            HackerDisableChannelPhase::Packing,
            777,
        ))
    );
    let object = restored
        .objects
        .get(&object_id)
        .expect("restored v3 object");
    assert_eq!(
        object.weapon_barrel_states,
        default_weapon_barrel_state_snapshots()
    );
    assert_eq!(object.last_weapon_discharge_sequence, 0);
    assert_eq!(restored.next_weapon_discharge_sequence, 1);
    assert!(restored.client_drawables.drawables.is_empty());
    assert_eq!(sentinel, 0xA3B4_C5D6);
}

#[test]
fn direct_xfer_v4_round_trips_logical_and_client_drawable_tails() {
    use super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(95);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V4_TAIL_VERSION;
    world.next_weapon_discharge_sequence = 43;
    let mut object = default_object_snapshot();
    object.id = object_id;
    object.template_name = "V4TailDirectXferObject".to_string();
    object.weapon_barrel_states = [
        WeaponBarrelStateSnapshot {
            current_barrel: 2,
            shots_left_on_barrel: 4,
        },
        WeaponBarrelStateSnapshot {
            current_barrel: 1,
            shots_left_on_barrel: 3,
        },
        WeaponBarrelStateSnapshot {
            current_barrel: 0,
            shots_left_on_barrel: 2,
        },
    ];
    object.last_weapon_discharge_sequence = 42;
    object.last_weapon_discharge_slot = 1;
    object.last_weapon_discharge_barrel = 2;
    object.last_weapon_discharge_frame = 7_654;
    world.objects.insert(object_id, object);
    let mut player = default_player_snapshot();
    player.id = 20;
    player.name = "V4PostTailAlignment".to_string();
    world.players.push(player);
    world
        .client_drawables
        .drawables
        .push(ClientDrawableStateSnapshot {
            object_id: object_id.0,
            draw_module_index: 2,
            source_template_name: "V4TailDirectXferObject".to_string(),
            model_key: "UVV4Tail".to_string(),
            selected_condition_state_index: 5,
            animation: Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: "UVV4Tail.UVV4Tail".to_string(),
                frame: 12.5,
                mode: ClientDrawableAnimationMode::Loop,
            }),
            last_seen_weapon_discharge_sequence: 42,
            recoil_slots: [
                vec![ClientDrawableRecoilSnapshot {
                    phase: ClientDrawableRecoilPhase::Recoil,
                    shift: 0.25,
                    recoil_rate: 1.5,
                }],
                Vec::new(),
                Vec::new(),
            ],
        });

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v4 world");
        let mut sentinel = 0xD4E5_F607u32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v4 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    let object = restored
        .objects
        .get(&object_id)
        .expect("restored v4 object");
    assert_eq!(
        object.weapon_barrel_states,
        [
            WeaponBarrelStateSnapshot {
                current_barrel: 2,
                shots_left_on_barrel: 4,
            },
            WeaponBarrelStateSnapshot {
                current_barrel: 1,
                shots_left_on_barrel: 3,
            },
            WeaponBarrelStateSnapshot {
                current_barrel: 0,
                shots_left_on_barrel: 2,
            },
        ]
    );
    assert_eq!(object.last_weapon_discharge_sequence, 42);
    assert_eq!(object.last_weapon_discharge_slot, 1);
    assert_eq!(object.last_weapon_discharge_barrel, 2);
    assert_eq!(object.last_weapon_discharge_frame, 7_654);
    assert_eq!(restored.next_weapon_discharge_sequence, 43);
    assert_eq!(restored.players[0].name, "V4PostTailAlignment");
    assert_eq!(restored.client_drawables.drawables.len(), 1);
    let drawable = &restored.client_drawables.drawables[0];
    assert_eq!(drawable.object_id, object_id.0);
    assert_eq!(drawable.last_seen_weapon_discharge_sequence, 42);
    assert_eq!(
        drawable.recoil_slots[0][0].phase,
        ClientDrawableRecoilPhase::Recoil
    );
    assert_eq!(sentinel, 0xD4E5_F607);
}

#[test]
fn direct_xfer_v5_round_trips_exact_player_template_binding_tail() {
    use super::xfer_helpers::default_object_snapshot;
    use crate::game_logic::SupplyTruckState;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V5_TAIL_VERSION;
    world
        .player_template_bindings
        .push(PlayerTemplateBindingSnapshot {
            player_id: 7,
            template_name: "FactionAmericaLaserGeneral".to_string(),
            template_index: 12,
        });
    let mut collector = default_object_snapshot();
    collector.id = ObjectId(81);
    collector.collector_runtime = Some(CollectorRuntimeSnapshot {
        owner_player_id: Some(7),
        producer_id: Some(ObjectId(80)),
        preferred_dock_id: Some(ObjectId(80)),
        target: Some(ObjectId(79)),
        supply_center_spawn_behavior_fired: true,
        supply_truck_state: SupplyTruckState::DockingCenter,
        supply_truck_force_pending: true,
        supply_truck_next_dock_action_frame: 1_234,
        stored_supply_boxes: 6,
    });
    world.objects.insert(collector.id, collector);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v5 world");
        let mut sentinel = 0xB7C8_D9EAu32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v5 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(
        restored.player_template_bindings,
        vec![PlayerTemplateBindingSnapshot {
            player_id: 7,
            template_name: "FactionAmericaLaserGeneral".to_string(),
            template_index: 12,
        }]
    );
    assert_eq!(
        restored
            .objects
            .get(&ObjectId(81))
            .and_then(|object| object.collector_runtime.as_ref())
            .expect("v5 collector tail"),
        &CollectorRuntimeSnapshot {
            owner_player_id: Some(7),
            producer_id: Some(ObjectId(80)),
            preferred_dock_id: Some(ObjectId(80)),
            target: Some(ObjectId(79)),
            supply_center_spawn_behavior_fired: true,
            supply_truck_state: SupplyTruckState::DockingCenter,
            supply_truck_force_pending: true,
            supply_truck_next_dock_action_frame: 1_234,
            stored_supply_boxes: 6,
        }
    );
    assert_eq!(sentinel, 0xB7C8_D9EA);
}

#[test]
fn direct_xfer_v6_round_trips_exact_shroud_tail() {
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use gamelogic::system::shroud_manager::{
        ShroudCellSnapshot, ShroudGridSnapshot, ShroudPendingUndoRevealSnapshot, ShroudSnapshot,
    };
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
    world.shroud = ShroudSnapshot {
        grid: Some(ShroudGridSnapshot {
            width: 2,
            height: 1,
            cell_size: 50.0,
            cells: vec![
                ShroudCellSnapshot {
                    current_shroud: std::array::from_fn(|player| match player {
                        0 => -2,
                        1 => 0,
                        _ => 1,
                    }),
                    active_shroud_level: std::array::from_fn(
                        |player| {
                            if player == 1 { 3 } else { 0 }
                        },
                    ),
                },
                ShroudCellSnapshot::default(),
            ],
        }),
        pending_undo_shroud_reveals: vec![ShroudPendingUndoRevealSnapshot {
            where_pos: [12.0, 0.0, -8.0],
            how_far: 75.0,
            for_whom: 5,
            expiration_frame: 7_654,
        }],
        pending_full_reveal_players: vec![2],
        pending_permanent_reveal_players: vec![3],
    };

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v6 world");
        let mut sentinel = 0xC8D9_EAFB_u32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v6 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(restored.shroud, world.shroud);
    assert_eq!(sentinel, 0xC8D9_EAFB);
}

#[test]
fn direct_xfer_v7_round_trips_weapon_suspend_fx_tail_and_keeps_alignment() {
    use super::xfer_helpers::default_player_snapshot;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(96);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V7_TAIL_VERSION;
    let mut object = ObjectSnapshot {
        id: object_id,
        ..super::xfer_helpers::default_object_snapshot()
    };
    object.template_name = "V7SuspendFxDirectXferObject".to_string();
    object.weapons = vec![Weapon::default(), Weapon::default(), Weapon::default()];
    object.weapon_suspend_fx_frames = vec![1_234, 0, 5_678];
    world.objects.insert(object_id, object);
    let mut player = default_player_snapshot();
    player.id = 21;
    player.name = "V7PostSuspendFxAlignment".to_string();
    world.players.push(player);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v7 world");
        let mut sentinel = 0xE9FA_0B1Cu32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v7 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    let restored_object = restored
        .objects
        .get(&object_id)
        .expect("restored v7 object");
    assert_eq!(
        restored_object.weapon_suspend_fx_frames,
        vec![1_234, 0, 5_678]
    );
    assert_eq!(restored.players[0].name, "V7PostSuspendFxAlignment");
    assert_eq!(sentinel, 0xE9FA_0B1C);
}

#[test]
fn direct_xfer_v8_round_trips_temporary_weapon_tail_and_keeps_alignment() {
    use super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::game_logic::host_temporary_weapon_behavior::{
        FireWeaponWhenDamagedRuntimeState, FireWeaponWhenDamagedWeaponRole,
        FireWeaponWhenDeadRuntimeState, TemporaryWeaponConstructionDefaults,
        TemporaryWeaponRuntimeBundle, TemporaryWeaponRuntimeKey, TemporaryWeaponRuntimeSpec,
        TemporaryWeaponRuntimeState, TemporaryWeaponSlot,
    };
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(99);
    let key = TemporaryWeaponRuntimeKey {
        module_source_index: 41,
        role: FireWeaponWhenDamagedWeaponRole::ReactionDamaged,
    };
    let spec = TemporaryWeaponRuntimeSpec {
        key,
        weapon_template_name: "V8TemporaryReactionWeapon".to_string(),
        weapon_slot: TemporaryWeaponSlot::Primary,
    };
    let mut weapon = TemporaryWeaponRuntimeState::from_cxx_constructor(
        &spec,
        TemporaryWeaponConstructionDefaults {
            clip_size: 6,
            clip_reload_frames: 17,
            scatter_target_count: 2,
            ..Default::default()
        },
        1234,
    );
    weapon.reload_ammo_from_cxx(
        TemporaryWeaponConstructionDefaults {
            clip_size: 6,
            clip_reload_frames: 17,
            scatter_target_count: 2,
            ..Default::default()
        },
        1234,
    );
    weapon.last_fire_frame = 1250;
    weapon.current_barrel = 2;
    weapon.suspend_fx_frame = 1300;

    let mut damaged = FireWeaponWhenDamagedRuntimeState {
        module_source_index: key.module_source_index,
        ..Default::default()
    };
    assert!(damaged.replace_weapon_state(weapon));
    let runtime = TemporaryWeaponRuntimeBundle {
        damaged: vec![damaged],
        dead: vec![FireWeaponWhenDeadRuntimeState {
            module_source_index: 42,
            upgrade_executed: true,
        }],
    };

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V8_TAIL_VERSION;
    let mut object = default_object_snapshot();
    object.id = object_id;
    object.temporary_weapon_runtime = Some(runtime.clone());
    world.objects.insert(object_id, object);
    let mut player = default_player_snapshot();
    player.id = 22;
    player.name = "V8PostTemporaryWeaponAlignment".to_string();
    world.players.push(player);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v8 world");
        let mut sentinel = 0xABCD_0123_u32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v8 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(
        restored
            .objects
            .get(&object_id)
            .expect("restored v8 object")
            .temporary_weapon_runtime,
        Some(runtime)
    );
    assert_eq!(restored.players[0].name, "V8PostTemporaryWeaponAlignment");
    assert_eq!(sentinel, 0xABCD_0123);
}

#[test]
fn direct_xfer_v6_omits_weapon_suspend_fx_tail_and_keeps_alignment() {
    use super::xfer_helpers::default_object_snapshot;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(97);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V6_TAIL_VERSION;
    let mut object = default_object_snapshot();
    object.id = object_id;
    object.weapon_suspend_fx_frames = vec![9_999];
    world.objects.insert(object_id, object);

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v6 world");
        let mut sentinel = 0xFA0B_1C2Du32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v6 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert!(
        restored
            .objects
            .get(&object_id)
            .expect("restored v6 object")
            .weapon_suspend_fx_frames
            .is_empty()
    );
    assert_eq!(sentinel, 0xFA0B_1C2D);
}

#[test]
fn bincode_v6_migrates_without_weapon_suspend_fx_tail() {
    let object_id = ObjectId(98);
    let mut source = WorldSnapshot::default();
    source.version = 6;
    let mut object = super::xfer_helpers::default_object_snapshot();
    object.id = object_id;
    object.weapons = vec![Weapon::default()];
    object.weapon_suspend_fx_frames = vec![7_777];
    source.objects.insert(object_id, object);

    let payload = serialize_pre_v7_v6_fixture(source).expect("serialize exact v6 fixture");
    let (restored, path) = decode_bincode_world_snapshot(&payload).expect("migrate v6 fixture");

    assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyPreV7V6);
    assert_eq!(restored.version, WORLD_SNAPSHOT_BINCODE_VERSION);
    let restored_object = restored
        .objects
        .get(&object_id)
        .expect("migrated v6 object");
    assert_eq!(restored_object.weapons.len(), 1);
    assert!(restored_object.weapon_suspend_fx_frames.is_empty());
}

#[test]
fn direct_xfer_rejects_future_outer_version_before_body_consumption() {
    use crate::save_load::{SaveLoadError, Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        let mut future_version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION + 1;
        let mut seconds = 0x1122_3344_5566_7788u64;
        let mut nanos = 0xA1B2_C3D4u32;
        let mut frame = 0x1020_3040_5060_7080u64;
        writer.xfer_u32(&mut future_version).expect("write version");
        writer
            .xfer_u64(&mut seconds)
            .expect("write timestamp seconds");
        writer.xfer_u32(&mut nanos).expect("write timestamp nanos");
        writer.xfer_u64(&mut frame).expect("write frame sentinel");
    }

    let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
    let mut restored = WorldSnapshot::default();
    assert!(matches!(
        restored.xfer(&mut reader),
        Err(SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
            actual,
        }) if actual == WORLD_SNAPSHOT_DIRECT_XFER_VERSION + 1
    ));

    let mut seconds = 0u64;
    let mut nanos = 0u32;
    let mut frame = 0u64;
    reader
        .xfer_u64(&mut seconds)
        .expect("timestamp bytes remain");
    reader.xfer_u32(&mut nanos).expect("nanoseconds remain");
    reader.xfer_u64(&mut frame).expect("frame bytes remain");
    assert_eq!(seconds, 0x1122_3344_5566_7788);
    assert_eq!(nanos, 0xA1B2_C3D4);
    assert_eq!(frame, 0x1020_3040_5060_7080);
}

#[test]
fn direct_xfer_rejects_future_writer_before_emitting_any_record_bytes() {
    use crate::save_load::{SaveLoadError, XferSave};
    use std::io::Cursor;

    let mut future = WorldSnapshot::default();
    future.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION + 1;
    let mut bytes = Cursor::new(Vec::new());
    let err = {
        let mut writer = XferSave::new(&mut bytes);
        future
            .xfer(&mut writer)
            .expect_err("future direct writer must fail closed")
    };
    assert!(matches!(
        err,
        SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
            actual,
        } if actual == WORLD_SNAPSHOT_DIRECT_XFER_VERSION + 1
    ));
    assert!(bytes.into_inner().is_empty());
}
