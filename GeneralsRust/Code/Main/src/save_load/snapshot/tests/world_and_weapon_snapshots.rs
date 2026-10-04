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
fn snapshot_current_rejects_stale_template_pair() {
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
    // C++ ThingTemplate ctor defaults m_isTrainable=FALSE (ThingTemplate.cpp:994)
    // and ExperienceTracker::addExperiencePoints drops XP for untrainable
    // objects (ExperienceTracker.cpp:138-139), so a promotable tank models an
    // IsTrainable=Yes unit.
    tank_template.is_trainable = true;
    source
        .templates
        .insert("TestTank".to_string(), tank_template);

    // Control: identical chassis without IsTrainable — C++ must never promote it.
    let mut truck_template = ThingTemplate::new("TestTruck");
    truck_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    source
        .templates
        .insert("TestTruck".to_string(), truck_template);

    let tank_id = source
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("failed to create tank");
    {
        let tank = source.host_object_mut(tank_id).expect("tank should exist");
        tank.gain_experience(180.0);
        assert_eq!(tank.experience.level, VeterancyLevel::Elite);
    }

    let truck_id = source
        .create_object("TestTruck", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("failed to create truck");
    {
        let truck = source
            .host_object_mut(truck_id)
            .expect("truck should exist");
        truck.gain_experience(180.0);
        // C++ ExperienceTracker::addExperiencePoints: untrainable, so no XP and no level.
        assert_eq!(truck.experience.level, VeterancyLevel::Rookie);
        assert_eq!(truck.experience.current, 0.0);
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

    let restored_truck = restored
        .host_object(truck_id)
        .expect("restored truck should exist");
    // The tracker replay must not phantom-promote the untrainable control either.
    assert_eq!(restored_truck.experience.level, VeterancyLevel::Rookie);
    assert_eq!(restored_truck.experience.current, 0.0);
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

mod drawable_and_direct_xfer;
mod queued_strikes_and_upgrades;
