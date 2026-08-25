//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn direct_xfer_accepts_known_outer_versions() {
    use crate::save_load::{XferLoad, XferSave};
    use std::io::Cursor;

    for version in 1..=WORLD_SNAPSHOT_DIRECT_XFER_VERSION {
        let mut source = WorldSnapshot::default();
        source.version = version;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = XferSave::new(&mut bytes);
            source
                .xfer(&mut writer)
                .expect("known direct-Xfer version writes");
        }
        let mut restored = WorldSnapshot::default();
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored
            .xfer(&mut reader)
            .expect("known direct-Xfer version reads");
        assert_eq!(restored.version, version);
    }
}

#[test]
fn companion_save_round_trips_mid_recoil_animation_and_discharge_sequence() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");
    let client_drawables = ClientDrawableWorldSnapshot {
        drawables: vec![ClientDrawableStateSnapshot {
            object_id: 9,
            draw_module_index: 0,
            source_template_name: "RecoilTank".to_string(),
            model_key: "RecoilTank".to_string(),
            selected_condition_state_index: 1,
            animation: Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: "RecoilTank.Fire".to_string(),
                frame: 3.5,
                mode: ClientDrawableAnimationMode::Once,
            }),
            last_seen_weapon_discharge_sequence: 17,
            recoil_slots: [
                vec![
                    ClientDrawableRecoilSnapshot {
                        phase: ClientDrawableRecoilPhase::RecoilStart,
                        shift: 0.05,
                        recoil_rate: 2.0,
                    },
                    ClientDrawableRecoilSnapshot {
                        phase: ClientDrawableRecoilPhase::Recoil,
                        shift: 0.40,
                        recoil_rate: 1.25,
                    },
                ],
                vec![ClientDrawableRecoilSnapshot {
                    phase: ClientDrawableRecoilPhase::Settle,
                    shift: 0.10,
                    recoil_rate: 0.50,
                }],
                Vec::new(),
            ],
        }],
    };
    let save_info = SaveGameInfo {
        filename: "mid_recoil".to_string(),
        display_name: "Mid Recoil".to_string(),
        description: "recoil phases".to_string(),
        map_name: "RecoilMap".to_string(),
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
            "mid_recoil",
            &GameLogic::new(),
            client_drawables.clone(),
            &save_info,
        )
        .expect("save mid-recoil");
    let (snapshot, _) = manager
        .load_game_snapshot("mid_recoil")
        .expect("load mid-recoil");
    assert_eq!(snapshot.client_drawables, client_drawables);
    assert_eq!(
        snapshot.client_drawables.drawables[0].last_seen_weapon_discharge_sequence,
        17
    );
}

#[test]
fn load_resets_model_state_before_recoil_restore() {
    let source = include_str!("../../../graphics/render_pipeline/pipeline_drawable_state.rs");
    let reset = source
        .find("*state = ObjectVisualState {")
        .expect("identity reset");
    let restore = source
        .find("if let Some(saved) = pending_restore")
        .expect("pending recoil restore");
    assert!(
        reset < restore,
        "C++ Drawable.cpp:4928 replaceModelConditionFlags must run before module recoil xfer"
    );
}

#[test]
fn companion_save_round_trips_w3d_ghost_snapshots() {
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use gamelogic::object::w3d_ghost_object::{
        Matrix3x4, OBJECTSHROUD_FOGGED, ParentGeometrySnapshot, RenderObjectClass,
        RenderObjectState, RenderSubObjectSnapshot, THE_W3D_GHOST_OBJECT_MANAGER, W3DDrawableInfo,
    };
    use std::time::{Duration, SystemTime};

    {
        let mut live = THE_W3D_GHOST_OBJECT_MANAGER.write().expect("ghost lock");
        *live = gamelogic::object::w3d_ghost_object::W3DGhostObjectManager::new();
        live.add_ghost_object(Some(42), true).unwrap();
        live.used_mut()[0].set_drawable_info(W3DDrawableInfo {
            drawable_id: 8,
            flags: 1,
            shroud_status_object_id: 42,
        });
        live.used_mut()[0].snapshot(
            0,
            0,
            false,
            &[RenderObjectState {
                name: "SavedGhost".to_string(),
                scale: 2.0,
                color: 0x1122_3344,
                transform: Matrix3x4::IDENTITY,
                sub_objects: vec![RenderSubObjectSnapshot {
                    name: "TURRET".to_string(),
                    visible: true,
                    transform: Matrix3x4::IDENTITY,
                }],
                class_id: RenderObjectClass::HLod,
            }],
            ParentGeometrySnapshot {
                geometry_type: 2,
                is_small: false,
                major_radius: 9.0,
                minor_radius: 3.0,
                position: [1.0, 2.0, 3.0],
                angle: 0.25,
            },
        );
        live.used_mut()[0].set_previous_shroudedness(0, OBJECTSHROUD_FOGGED);
    }

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("init");
    let save_info = SaveGameInfo {
        filename: "ghosts".to_string(),
        display_name: "Ghosts".to_string(),
        description: "ghost xfer".to_string(),
        map_name: "GhostMap".to_string(),
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
            "ghosts",
            &GameLogic::new(),
            ClientDrawableWorldSnapshot::default(),
            &save_info,
        )
        .expect("save ghosts");

    {
        let mut live = THE_W3D_GHOST_OBJECT_MANAGER.write().expect("ghost lock");
        *live = gamelogic::object::w3d_ghost_object::W3DGhostObjectManager::new();
    }

    let (snapshot, _) = manager.load_game_snapshot("ghosts").expect("decode ghosts");
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut GameLogic::new())
        .expect("restore ghosts");
    let live = THE_W3D_GHOST_OBJECT_MANAGER.read().expect("ghost lock");
    assert_eq!(live.used_count(), 1);
    let ghost = &live.used()[0];
    assert_eq!(ghost.parent_object_id(), Some(42));
    assert_eq!(ghost.snapshots(0)[0].render_object.name, "SavedGhost");
    assert!((ghost.snapshots(0)[0].render_object.scale - 2.0).abs() < f32::EPSILON);
    assert_eq!(ghost.snapshots(0)[0].render_object.color, 0x1122_3344);
    assert_eq!(
        ghost.snapshots(0)[0].render_object.sub_objects[0].name,
        "TURRET"
    );
    assert!(ghost.snapshots(0)[0].render_object.sub_objects[0].visible);
    assert_eq!(ghost.previous_shroudedness(0), Some(OBJECTSHROUD_FOGGED));
}

#[test]
fn snapshot_round_trips_player_rank_skill_and_science_purchase_points() {
    let mut source = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Ranked", true);
    player.rank_level = 4;
    player.skill_points = 1_800;
    player.science_purchase_points = 7;
    source.add_player(player);

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    assert_eq!(
        snapshot.player_ranks,
        vec![PlayerRankSnapshot {
            player_id: 1,
            rank_level: 4,
            skill_points: 1_800,
            science_purchase_points: 7,
        }]
    );

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");
    let loaded = restored.get_player(1).expect("restored ranked player");
    assert_eq!(loaded.rank_level, 4);
    assert_eq!(loaded.skill_points, 1_800);
    assert_eq!(loaded.science_purchase_points, 7);
}

#[test]
fn snapshot_round_trips_power_sabotaged_till_frame() {
    let mut source = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Sabotaged", true);
    player.power_sabotaged_till_frame = 900;
    source.add_player(player);

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot creation failed");
    assert_eq!(
        snapshot.player_energy,
        vec![PlayerEnergySnapshot {
            player_id: 1,
            power_sabotaged_till_frame: 900,
        }]
    );

    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("snapshot restore failed");
    let loaded = restored.get_player(1).expect("restored sabotaged player");
    assert_eq!(loaded.power_sabotaged_till_frame, 900);
}

#[test]
fn snapshot_pre_v15_defaults_power_sabotage_frame() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(1, Team::USA, "Clean", true));
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    snapshot.version = 14;
    snapshot.player_energy.clear();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert_eq!(
        restored
            .get_player(1)
            .expect("player")
            .power_sabotaged_till_frame,
        0
    );
}

#[test]
fn snapshot_pre_v10_defaults_rank_skill_and_science_purchase_points() {
    let mut legacy = WorldSnapshot::default();
    legacy.version = WORLD_SNAPSHOT_DIRECT_XFER_V9_TAIL_VERSION;
    legacy.players.push(PlayerSnapshot {
        id: 3,
        name: "LegacyRank".to_string(),
        team: Team::China,
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
    legacy.player_ranks.push(PlayerRankSnapshot {
        player_id: 3,
        rank_level: 5,
        skill_points: 9_999,
        science_purchase_points: 12,
    });

    let mut restored = GameLogic::new();
    SnapshotBuilder::new()
        .restore_from_snapshot(&legacy, &mut restored)
        .expect("v9 predecessor defaults rank tail");
    let loaded = restored.get_player(3).expect("legacy player");
    assert_eq!(loaded.rank_level, 1);
    assert_eq!(loaded.skill_points, 0);
    assert_eq!(loaded.science_purchase_points, 0);
}

#[test]
fn direct_xfer_v10_round_trips_player_rank_tail() {
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V10_TAIL_VERSION;
    world.player_ranks.push(PlayerRankSnapshot {
        player_id: 11,
        rank_level: 3,
        skill_points: 420,
        science_purchase_points: 2,
    });

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v10 world");
        let mut sentinel = 0xC0DE_F00Du32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v10 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(
        restored.player_ranks,
        vec![PlayerRankSnapshot {
            player_id: 11,
            rank_level: 3,
            skill_points: 420,
            science_purchase_points: 2,
        }]
    );
    assert_eq!(sentinel, 0xC0DE_F00D);
}

#[test]
fn bincode_v9_migrates_without_player_rank_tail() {
    let mut source = WorldSnapshot::default();
    source.version = 9;
    source.player_ranks.push(PlayerRankSnapshot {
        player_id: 1,
        rank_level: 5,
        skill_points: 2_000,
        science_purchase_points: 4,
    });

    let payload = serialize_pre_v10_v9_fixture(source).expect("serialize exact v9 fixture");
    let (restored, path) = decode_bincode_world_snapshot(&payload).expect("migrate v9 fixture");

    assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyPreV10V9);
    assert_eq!(restored.version, WORLD_SNAPSHOT_BINCODE_VERSION);
    assert!(restored.player_ranks.is_empty());
}

#[test]
fn snapshot_restore_rebuilds_garrisoned_units_from_occupants() {
    let mut source = GameLogic::new();
    let mut bunker = ThingTemplate::new("TestBunker");
    bunker.add_kind_of(KindOf::Structure);
    source.templates.insert("TestBunker".to_string(), bunker);
    let mut ranger = ThingTemplate::new("TestRanger");
    ranger.add_kind_of(KindOf::Infantry);
    source.templates.insert("TestRanger".to_string(), ranger);

    let bunker_id = source
        .create_object("TestBunker", Team::USA, Vec3::ZERO)
        .expect("bunker");
    let ranger_id = source
        .create_object("TestRanger", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("ranger");
    {
        let bunker = source.host_object_mut(bunker_id).expect("bunker obj");
        bunker.occupants.push(ranger_id);
        if let Some(data) = bunker.building_data.as_mut() {
            data.garrisoned_units.push(ranger_id);
        }
    }
    if let Some(ranger) = source.host_object_mut(ranger_id) {
        ranger.contained_by = Some(bunker_id);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");

    let bunker = restored.host_object(bunker_id).expect("restored bunker");
    assert_eq!(bunker.occupants, vec![ranger_id]);
    let garrisoned = bunker
        .building_data
        .as_ref()
        .map(|data| data.garrisoned_units.clone())
        .unwrap_or_default();
    assert_eq!(
        garrisoned,
        vec![ranger_id],
        "BuildingData.garrisoned_units must mirror occupants after load"
    );
}

#[test]
fn snapshot_round_trips_object_instance_name() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("USA_Ranger".to_string(), ThingTemplate::new("USA_Ranger"));
    let id = source
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .expect("create");
    {
        let object = source.host_object_mut(id).expect("object");
        object.name = "ScriptNamedRanger".to_string();
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert!(
        snapshot
            .object_instance_guards
            .iter()
            .any(|entry| entry.object_id == id && entry.instance_name == "ScriptNamedRanger")
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let loaded = restored.host_object(id).expect("restored");
    assert_eq!(loaded.template_name, "USA_Ranger");
    assert_eq!(loaded.name, "ScriptNamedRanger");
}

#[test]
fn snapshot_round_trips_guard_anchors() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("TestTank".to_string(), ThingTemplate::new("TestTank"));
    let tank_id = source
        .create_object("TestTank", Team::USA, Vec3::new(1.0, 0.0, 2.0))
        .expect("tank");
    let target_id = source
        .create_object("TestTank", Team::China, Vec3::new(40.0, 0.0, 40.0))
        .expect("target");
    {
        let tank = source.host_object_mut(tank_id).expect("tank obj");
        tank.guard_position = Some(Vec3::new(10.0, 0.0, 20.0));
        tank.guard_target = Some(target_id);
        tank.guard_radius = 150.0;
        tank.guard_mode = GuardMode::WithoutPursuit;
        tank.ai_state = AIState::GuardingArea;
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");

    let loaded = restored.host_object(tank_id).expect("restored tank");
    assert_eq!(loaded.guard_position, Some(Vec3::new(10.0, 0.0, 20.0)));
    assert_eq!(loaded.guard_target, Some(target_id));
    assert!((loaded.guard_radius - 150.0).abs() < f32::EPSILON);
    assert_eq!(loaded.guard_mode, GuardMode::WithoutPursuit);
    assert_eq!(loaded.ai_state, AIState::GuardingArea);
}

#[test]
fn snapshot_pre_v11_defaults_instance_name_and_guard_anchors() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("USA_Ranger".to_string(), ThingTemplate::new("USA_Ranger"));
    let id = source
        .create_object("USA_Ranger", Team::USA, Vec3::new(8.0, 0.0, 4.0))
        .expect("create");
    {
        let object = source.host_object_mut(id).expect("object");
        object.name = "WouldBeLost".to_string();
        object.guard_position = Some(Vec3::new(1.0, 0.0, 1.0));
        object.guard_radius = 90.0;
        object.guard_mode = GuardMode::FlyingUnitsOnly;
    }

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    snapshot.version = WORLD_SNAPSHOT_DIRECT_XFER_V10_TAIL_VERSION;
    snapshot.object_instance_guards.clear();

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("v10 predecessor defaults name/guard tail");
    let loaded = restored.host_object(id).expect("legacy object");
    assert!(loaded.name.is_empty());
    assert_eq!(loaded.guard_position, None);
    assert_eq!(loaded.guard_target, None);
    assert_eq!(loaded.guard_radius, 0.0);
    assert_eq!(loaded.guard_mode, GuardMode::Normal);
}

#[test]
fn direct_xfer_v11_round_trips_instance_name_and_guard_tail() {
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V11_TAIL_VERSION;
    world
        .object_instance_guards
        .push(ObjectInstanceGuardSnapshot {
            object_id: ObjectId(42),
            instance_name: "NamedGuard".to_string(),
            guard_position: Some(Vec3::new(3.0, 0.0, 7.0)),
            guard_target: Some(ObjectId(9)),
            guard_radius: 120.0,
            guard_mode: GuardMode::FlyingUnitsOnly,
        });

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v11 world");
        let mut sentinel = 0xC0DE_F00Du32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v11 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(
        restored.object_instance_guards,
        vec![ObjectInstanceGuardSnapshot {
            object_id: ObjectId(42),
            instance_name: "NamedGuard".to_string(),
            guard_position: Some(Vec3::new(3.0, 0.0, 7.0)),
            guard_target: Some(ObjectId(9)),
            guard_radius: 120.0,
            guard_mode: GuardMode::FlyingUnitsOnly,
        }]
    );
    assert_eq!(sentinel, 0xC0DE_F00D);
}

#[test]
fn bincode_v10_migrates_without_instance_name_and_guard_tail() {
    let mut source = WorldSnapshot::default();
    source.version = 10;
    source
        .object_instance_guards
        .push(ObjectInstanceGuardSnapshot {
            object_id: ObjectId(1),
            instance_name: "ShouldDrop".to_string(),
            guard_position: Some(Vec3::ZERO),
            guard_target: None,
            guard_radius: 50.0,
            guard_mode: GuardMode::WithoutPursuit,
        });

    let payload = serialize_pre_v11_v10_fixture(source).expect("serialize exact v10 fixture");
    let (restored, path) = decode_bincode_world_snapshot(&payload).expect("migrate v10 fixture");

    assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyPreV11V10);
    assert_eq!(restored.version, WORLD_SNAPSHOT_BINCODE_VERSION);
    assert!(restored.object_instance_guards.is_empty());
}

#[test]
fn snapshot_round_trips_cia_vision_spied_and_registry() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(0, Team::USA, "USA", true));
    source
        .templates
        .insert("TestTank".to_string(), ThingTemplate::new("TestTank"));
    let caster = source
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    let enemy = source
        .create_object("TestTank", Team::China, Vec3::new(400.0, 0.0, 400.0))
        .expect("enemy");
    assert!(source.activate_cia_intelligence(0, Team::USA, Some(caster)));
    assert!(
        source
            .host_object(enemy)
            .unwrap()
            .is_vision_spied_by_player(0)
    );
    assert_eq!(source.cia_intelligence().active_count(), 1);
    let expires = source.cia_intelligence().active_scans()[0].expires_frame;

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert!(
        snapshot
            .vision_spied
            .iter()
            .any(|entry| entry.object_id == enemy && entry.vision_spied_mask != 0)
    );
    assert_eq!(snapshot.cia_intelligence.active_count(), 1);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert!(
        restored
            .host_object(enemy)
            .unwrap()
            .is_vision_spied_by_player(0),
        "load must keep Object.vision_spied_mask"
    );
    assert_eq!(restored.cia_intelligence().active_count(), 1);
    assert!(restored.cia_intelligence().is_object_vision_spied(0, enemy));
    assert_eq!(
        restored.cia_intelligence().active_scans()[0].expires_frame,
        expires
    );
}

#[test]
fn snapshot_round_trips_builder_id_and_dozer_build_task() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "AmericaVehicleDozer".to_string(),
        ThingTemplate::new("AmericaVehicleDozer"),
    );
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks.add_kind_of(KindOf::Structure);
    source
        .templates
        .insert("AmericaBarracks".to_string(), barracks);
    let dozer = source
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
        .expect("dozer");
    let scaffold = source
        .create_object("AmericaBarracks", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("scaffold");
    {
        let building = source.host_object_mut(scaffold).expect("scaffold obj");
        building.builder_id = Some(dozer);
        building.set_status_under_construction(true);
        building.construction_percent = 0.4;
    }
    {
        let unit = source.host_object_mut(dozer).expect("dozer obj");
        unit.dozer_task_build_target = Some(scaffold);
        unit.dozer_task_build_order_frame = 77;
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert!(
        snapshot
            .builder_tasks
            .iter()
            .any(|entry| { entry.object_id == scaffold && entry.builder_id == Some(dozer) })
    );
    assert!(snapshot.builder_tasks.iter().any(|entry| {
        entry.object_id == dozer
            && entry.dozer_task_build_target == Some(scaffold)
            && entry.dozer_task_build_order_frame == 77
    }));

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    let loaded_scaffold = restored.host_object(scaffold).expect("restored scaffold");
    assert_eq!(loaded_scaffold.builder_id, Some(dozer));
    let loaded_dozer = restored.host_object(dozer).expect("restored dozer");
    assert_eq!(loaded_dozer.dozer_task_build_target, Some(scaffold));
    assert_eq!(loaded_dozer.dozer_task_build_order_frame, 77);
}

#[test]
fn snapshot_round_trips_sell_list_mid_sell() {
    let mut source = GameLogic::new();
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let mut plant = ThingTemplate::new("AmericaPowerPlant");
    plant.add_kind_of(KindOf::Structure).set_health(500.0);
    plant.build_cost.supplies = 800;
    source
        .templates
        .insert("AmericaPowerPlant".to_string(), plant);
    let id = source
        .create_object("AmericaPowerPlant", Team::USA, Vec3::ZERO)
        .expect("plant");
    if let Some(obj) = source.host_object_mut(id) {
        obj.construction_percent = 1.0;
        obj.set_status_under_construction(false);
        obj.set_status_reconstructing(false);
    }
    assert!(source.start_sell_object(id));
    assert!(source.is_object_being_sold(id));
    let sell_frame = source.sell_list_for_snapshot()[0].1;

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert_eq!(
        snapshot.sell_list,
        vec![SellListEntrySnapshot {
            object_id: id,
            sell_frame,
        }]
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert!(
        restored.is_object_being_sold(id),
        "load must keep the building on BuildAssistant sell_list"
    );
    let before = restored.host_object(id).unwrap().construction_percent;
    restored.set_current_frame(u64::from(sell_frame.saturating_add(50)));
    restored.update_sell_list();
    let after = restored.host_object(id).unwrap().construction_percent;
    assert!(
        after < before,
        "restored sell_list must keep deconstructing (before={before}, after={after})"
    );
}

#[test]
fn direct_xfer_v13_round_trips_cia_builder_sell_tail() {
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_V13_TAIL_VERSION;
    world.vision_spied.push(ObjectVisionSpiedSnapshot {
        object_id: ObjectId(7),
        vision_spied_mask: 1,
    });
    world.builder_tasks.push(ObjectBuilderTaskSnapshot {
        object_id: ObjectId(8),
        builder_id: Some(ObjectId(9)),
        dozer_task_build_target: Some(ObjectId(8)),
        dozer_task_build_order_frame: 12,
    });
    world.sell_list.push(SellListEntrySnapshot {
        object_id: ObjectId(10),
        sell_frame: 33,
    });

    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        world.xfer(&mut writer).expect("write direct v13 world");
        let mut sentinel = 0xC0DE_F00Du32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }

    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read direct v13 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }

    assert_eq!(
        restored.vision_spied,
        vec![ObjectVisionSpiedSnapshot {
            object_id: ObjectId(7),
            vision_spied_mask: 1,
        }]
    );
    assert_eq!(
        restored.builder_tasks,
        vec![ObjectBuilderTaskSnapshot {
            object_id: ObjectId(8),
            builder_id: Some(ObjectId(9)),
            dozer_task_build_target: Some(ObjectId(8)),
            dozer_task_build_order_frame: 12,
        }]
    );
    assert_eq!(
        restored.sell_list,
        vec![SellListEntrySnapshot {
            object_id: ObjectId(10),
            sell_frame: 33,
        }]
    );
    assert_eq!(sentinel, 0xC0DE_F00D);
}

#[test]
fn bincode_v12_migrates_without_cia_builder_sell_tail() {
    let mut source = WorldSnapshot::default();
    source.version = 12;
    source.vision_spied.push(ObjectVisionSpiedSnapshot {
        object_id: ObjectId(1),
        vision_spied_mask: 4,
    });
    source.builder_tasks.push(ObjectBuilderTaskSnapshot {
        object_id: ObjectId(2),
        builder_id: Some(ObjectId(3)),
        dozer_task_build_target: None,
        dozer_task_build_order_frame: 0,
    });
    source.sell_list.push(SellListEntrySnapshot {
        object_id: ObjectId(4),
        sell_frame: 9,
    });

    let payload = serialize_pre_v13_v12_fixture(source).expect("serialize exact v12 fixture");
    let (restored, path) = decode_bincode_world_snapshot(&payload).expect("migrate v12 fixture");

    assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyPreV13V12);
    assert_eq!(restored.version, WORLD_SNAPSHOT_BINCODE_VERSION);
    assert!(restored.vision_spied.is_empty());
    assert!(restored.builder_tasks.is_empty());
    assert!(restored.sell_list.is_empty());
    assert_eq!(restored.cia_intelligence.active_count(), 0);
}

#[test]
fn snapshot_round_trips_sole_heal_contain_original_team_formation() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("HealUnit".to_string(), ThingTemplate::new("HealUnit"));
    source
        .templates
        .insert("GarrBldg".to_string(), ThingTemplate::new("GarrBldg"));
    source.add_player(Player::new(1, Team::USA, "USA", true));
    source.set_current_frame(400);

    let healer = source
        .create_object("HealUnit", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("healer");
    let patient = source
        .create_object("HealUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
        .expect("patient");
    let building = source
        .create_object("GarrBldg", Team::China, Vec3::new(8.0, 0.0, 8.0))
        .expect("building");
    {
        let object = source.host_object_mut(patient).expect("patient");
        object.sole_healing_benefactor = Some(healer);
        object.sole_healing_benefactor_expiration_frame = 480;
        object.set_formation(7, glam::Vec2::new(-12.0, 4.0));
    }
    source.stamp_contained_by_frame(patient, 350);
    {
        let object = source.host_object_mut(building).expect("building");
        object.object_type = crate::game_logic::ObjectType::Building;
        let mut building_data = crate::game_logic::buildings::BuildingData::new(
            crate::game_logic::buildings::BuildingType::Bunker,
        );
        building_data.original_team = Some(Team::China);
        object.building_data = Some(building_data);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("persist snapshot");
    assert!(
        snapshot
            .object_persist
            .iter()
            .any(|entry| entry.object_id == patient
                && entry.sole_healing_benefactor == Some(healer)
                && entry.contained_by_frame == Some(350)
                && entry.formation_id == 7)
    );
    assert!(
        snapshot
            .object_persist
            .iter()
            .any(|entry| entry.object_id == building && entry.original_team == Some(Team::China))
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore persist");
    let patient_obj = restored.host_object(patient).expect("patient restored");
    assert_eq!(patient_obj.sole_healing_benefactor, Some(healer));
    assert_eq!(patient_obj.sole_healing_benefactor_expiration_frame, 480);
    assert_eq!(patient_obj.formation_id, 7);
    assert!((patient_obj.formation_offset.x + 12.0).abs() < 0.01);
    assert_eq!(restored.contained_by_frame_for_snapshot(patient), Some(350));
    let building_obj = restored.host_object(building).expect("building restored");
    assert_eq!(
        building_obj
            .building_data
            .as_ref()
            .and_then(|data| data.original_team),
        Some(Team::China)
    );
}

#[test]
fn snapshot_round_trips_experience_sink_and_scalar() {
    let mut source = GameLogic::new();
    source
        .templates
        .insert("XpUnit".to_string(), ThingTemplate::new("XpUnit"));
    source.add_player(Player::new(1, Team::USA, "USA", true));
    let tank = source
        .create_object("XpUnit", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("tank");
    let rider = source
        .create_object("XpUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
        .expect("rider");
    {
        let object = source.host_object_mut(rider).expect("rider");
        object.set_experience_sink(Some(tank));
        object.set_experience_scalar(2.0);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("xp snapshot");
    assert!(
        snapshot.object_experience_trackers.iter().any(|entry| {
            entry.object_id == rider
                && entry.experience_sink == Some(tank)
                && (entry.experience_scalar - 2.0).abs() < f32::EPSILON
        }),
        "C++ ExperienceTracker::xfer sink+scalar must persist"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore xp");
    let rider_obj = restored.host_object(rider).expect("rider restored");
    assert_eq!(rider_obj.experience_sink, Some(tank));
    assert!((rider_obj.experience_scalar - 2.0).abs() < f32::EPSILON);
}

#[test]
fn bincode_v13_migrates_without_object_persist_tail() {
    let mut source = WorldSnapshot::default();
    source.version = 13;
    source.object_persist.push(ObjectPersistTailSnapshot {
        object_id: ObjectId(1),
        sole_healing_benefactor: Some(ObjectId(2)),
        sole_healing_benefactor_expiration_frame: 9,
        contained_by_frame: Some(3),
        original_team: Some(Team::USA),
        formation_id: 4,
        formation_offset: [1.0, 2.0],
        stealth_opacity: 0.5,
        terrain_decal_type: 1,
        terrain_decal_size: 3.5,
    });
    let payload = serialize_pre_v14_v13_fixture(source).expect("serialize v13");
    let (restored, path) = decode_bincode_world_snapshot(&payload).expect("migrate v13");
    assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyPreV14V13);
    assert!(restored.object_persist.is_empty());
    assert!(restored.client_drawable_visuals.is_empty());
}

#[test]
fn snapshot_round_trips_scoring_restriction_cave_tunnel_airfield() {
    gamelogic::helpers::TheGameLogic::set_scoring_enabled(false);
    let mut source = GameLogic::new();
    source.set_limit_superweapons(true);
    source
        .cave_system_residual_mut()
        .register_cave(ObjectId(10), 1, Team::USA);
    let _ =
        source
            .cave_system_residual_mut()
            .record_enter(1, ObjectId(20), ObjectId(10), Team::USA);
    source
        .tunnel_network_residual_mut()
        .on_tunnel_created(1, ObjectId(30));
    let _ = source
        .tunnel_network_residual_mut()
        .record_enter(1, ObjectId(40), ObjectId(30));
    source.restore_airfield_parking_spaces(vec![(
        ObjectId(50),
        vec![(Some(ObjectId(60)), false), (None, false)],
    )]);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert!(!snapshot.is_scoring_enabled);
    assert!(snapshot.limit_superweapons);
    assert!(snapshot.cave_system.is_in_network(1, ObjectId(20)));
    assert!(snapshot.tunnel_network.is_in_network(1, ObjectId(40)));
    assert_eq!(
        snapshot.airfield_parking.fields[0].spaces[0].object_id,
        Some(ObjectId(60))
    );

    gamelogic::helpers::TheGameLogic::set_scoring_enabled(true);
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert!(!gamelogic::helpers::TheGameLogic::is_scoring_enabled());
    assert!(restored.skirmish_rules().limit_superweapons);
    assert!(
        restored
            .cave_system_residual()
            .is_in_network(1, ObjectId(20))
    );
    assert!(
        restored
            .tunnel_network_residual()
            .is_in_network(1, ObjectId(40))
    );
    assert_eq!(
        restored.snapshot_airfield_parking_spaces()[0].1[0].0,
        Some(ObjectId(60))
    );
    gamelogic::helpers::TheGameLogic::set_scoring_enabled(true);
}

#[test]
fn snapshot_pre_v17_defaults_scoring_and_empty_pools() {
    let mut snapshot = WorldSnapshot::default();
    snapshot.version = WORLD_SNAPSHOT_DIRECT_XFER_V16_TAIL_VERSION;
    snapshot.is_scoring_enabled = false;
    snapshot.limit_superweapons = true;
    snapshot
        .cave_system
        .register_cave(ObjectId(1), 0, Team::USA);
    let builder = SnapshotBuilder::new();
    // Direct restore of a v16-shaped in-memory record still carries the
    // live fields; bincode v16 migration is the empty-default path.
    let mut v16 = snapshot;
    v16.is_scoring_enabled = true;
    v16.limit_superweapons = false;
    v16.cave_system = crate::game_logic::HostCaveSystem::new();
    v16.tunnel_network = crate::game_logic::HostTunnelNetworkRegistry::new();
    v16.airfield_parking = AirfieldParkingWorldSnapshot::default();
    let mut restored = GameLogic::new();
    restored.set_limit_superweapons(true);
    builder
        .restore_from_snapshot(&v16, &mut restored)
        .expect("restore");
    assert!(gamelogic::helpers::TheGameLogic::is_scoring_enabled());
    assert!(!restored.skirmish_rules().limit_superweapons);
    assert_eq!(restored.cave_system_residual().contain_count(0), 0);
}

#[test]
fn snapshot_round_trips_v18_ui_script_radar_water_drawable() {
    gamelogic::helpers::TheGameLogic::set_rank_level_limit(8);
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(false);
    gamelogic::helpers::TheGameLogic::set_hulk_max_lifetime_override(42);
    gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(15);
    let _ = gamelogic::scripting::engine::initialize_script_engine();
    let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
        engine.restore_named_trackers(
            &[("MissionClock".into(), 90, true)],
            &[("GateOpen".into(), true)],
        );
        engine.restore_named_reveals(&[(
            "BaseLook".into(),
            "WP_Base".into(),
            250.0,
            "PlyrAmerica".into(),
        )]);
    });
    let mut source = GameLogic::new();
    source.upsert_script_named_timer("LaunchClock", "Launch in", true);
    source.restore_script_named_timer_display_shown(false);
    source.restore_script_superweapon_display_enabled(false);
    source.restore_script_superweapon_hidden_objects([ObjectId(88)]);
    source.restore_radar_script_state(false, true);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert_eq!(snapshot.persist_v18.rank_level_limit, 8);
    assert!(!snapshot.persist_v18.draw_icon_ui);
    assert_eq!(snapshot.persist_v18.script_hulk_max_lifetime_override, 42);
    assert_eq!(snapshot.persist_v18.rank_points_to_add_at_game_start, 15);
    assert!(!snapshot.persist_v18.named_timer_display_shown);
    assert_eq!(snapshot.persist_v18.named_timers[0].name, "LaunchClock");
    assert!(snapshot.persist_v18.superweapon_hidden_by_script);
    assert!(snapshot.persist_v18.radar_forced);
    assert!(snapshot.persist_v18.radar_hidden);
    assert_eq!(snapshot.persist_v18.script_named_reveals.len(), 1);
    assert_eq!(
        snapshot.persist_v18.script_named_reveals[0].reveal_name,
        "BaseLook"
    );
    assert_eq!(
        snapshot.persist_v18.script_named_reveals[0].waypoint_name,
        "WP_Base"
    );
    assert!(
        (snapshot.persist_v18.script_named_reveals[0].radius_to_reveal - 250.0).abs()
            < f32::EPSILON
    );
    assert_eq!(
        snapshot.persist_v18.script_named_reveals[0].player_name,
        "PlyrAmerica"
    );

    gamelogic::helpers::TheGameLogic::set_rank_level_limit(1000);
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(true);
    gamelogic::helpers::TheGameLogic::set_hulk_max_lifetime_override(-1);
    gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(0);
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert_eq!(gamelogic::helpers::TheGameLogic::get_rank_level_limit(), 8);
    assert!(!gamelogic::helpers::TheGameLogic::get_draw_icon_ui());
    assert_eq!(
        gamelogic::helpers::TheGameLogic::get_hulk_max_lifetime_override(),
        42
    );
    assert_eq!(
        gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start(),
        15
    );
    assert_eq!(
        restored.peek_script_named_timers().get("LaunchClock"),
        Some(&("Launch in".to_string(), true))
    );
    assert!(!restored.peek_script_named_timer_display_shown());
    assert!(!restored.peek_script_superweapon_display_enabled());
    assert!(
        restored
            .peek_script_superweapon_hidden_objects()
            .contains(&ObjectId(88))
    );
    assert!(restored.radar_forced());
    assert!(!restored.radar_script_enabled());
    let _ = gamelogic::scripting::engine::with_script_engine_ref(|engine| {
        let counter = engine.get_counter("MissionClock").expect("counter");
        assert_eq!(counter.value, 90);
        assert!(counter.is_countdown_timer);
        assert_eq!(engine.get_flag("GateOpen").map(|f| f.value), Some(true));
        let reveals = engine.snapshot_named_reveals();
        assert_eq!(reveals.len(), 1);
        assert_eq!(reveals[0].0, "BaseLook");
        assert_eq!(reveals[0].1, "WP_Base");
        assert!((reveals[0].2 - 250.0).abs() < f32::EPSILON);
        assert_eq!(reveals[0].3, "PlyrAmerica");
    });
    gamelogic::helpers::TheGameLogic::set_rank_level_limit(1000);
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(true);
    gamelogic::helpers::TheGameLogic::set_hulk_max_lifetime_override(-1);
    gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(0);
}

#[test]
fn snapshot_pre_v18_defaults_persist_tail() {
    let mut snapshot = WorldSnapshot::default();
    snapshot.version = WORLD_SNAPSHOT_DIRECT_XFER_V17_TAIL_VERSION;
    snapshot.persist_v18.draw_icon_ui = false;
    snapshot.persist_v18.named_timer_display_shown = false;
    let builder = SnapshotBuilder::new();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");
    assert!(gamelogic::helpers::TheGameLogic::get_draw_icon_ui());
    assert!(restored.peek_script_named_timer_display_shown());
}

#[test]
fn spy_vision_update_timers_survive_snapshot_and_keep_sabotage_dark() {
    let mut source = GameLogic::new();
    source.set_current_frame(200);
    source.templates.insert(
        "ChinaInternetCenter".into(),
        ThingTemplate::new("ChinaInternetCenter"),
    );
    source.add_player(Player::new(1, Team::China, "China", true));
    let id = source
        .create_object("ChinaInternetCenter", Team::China, Vec3::ZERO)
        .expect("create");
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.apply_spy_vision_disabled_until(350);
        obj.status.spy_vision_reset_timers = true;
        obj.status.spy_vision_hack_two_wake_frame = 500;
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert_eq!(obj_snap.status.spy_vision_disabled_until_frame, 350);
    assert!(obj_snap.status.spy_vision_reset_timers);
    assert_eq!(obj_snap.status.spy_vision_hack_two_wake_frame, 500);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    {
        let obj = restored.host_object(id).expect("restored");
        assert_eq!(obj.status.spy_vision_disabled_until_frame, 350);
        assert!(obj.status.spy_vision_reset_timers);
        assert_eq!(obj.status.spy_vision_hack_two_wake_frame, 500);
        assert!(
            obj.is_spy_vision_disabled(200),
            "sabotage disable must still be dark at the saved frame"
        );
    }

    restored.frame = 349;
    {
        let obj = restored.host_object_mut(id).expect("pre-expiry");
        obj.tick_spy_vision_disabled(349);
        assert!(obj.is_spy_vision_disabled(349));
        assert_eq!(obj.status.spy_vision_disabled_until_frame, 350);
    }

    restored.frame = 350;
    let obj = restored.host_object_mut(id).expect("post-expiry");
    obj.tick_spy_vision_disabled(350);
    assert!(
        !obj.is_spy_vision_disabled(350),
        "disabledUntilFrame must expire on the saved frame after load"
    );
    assert_eq!(obj.status.spy_vision_disabled_until_frame, 0);
    assert_eq!(
        obj.status.spy_vision_hack_two_wake_frame, 500,
        "Hack II wake must survive disable expiry"
    );
}

#[test]
fn disabled_paralyzed_freeze_survives_snapshot_until_saved_frame() {
    let mut source = GameLogic::new();
    source.set_current_frame(80);
    source
        .templates
        .insert("USARanger".into(), ThingTemplate::new("USARanger"));
    source.add_player(Player::new(1, Team::USA, "USA", true));
    let id = source
        .create_object("USARanger", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .expect("create");
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.apply_disabled_paralyzed(140);
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert!(obj_snap.status.disabled_paralyzed);
    assert_eq!(obj_snap.status.disabled_paralyzed_until_frame, 140);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    {
        let obj = restored.host_object(id).expect("restored");
        assert!(obj.status.disabled_paralyzed);
        assert_eq!(obj.status.disabled_paralyzed_until_frame, 140);
        assert!(obj.is_disabled(), "plan-change freeze must hold after load");
    }

    restored.frame = 139;
    {
        let obj = restored.host_object_mut(id).expect("pre-expiry");
        obj.tick_disabled_paralyzed(139);
        assert!(obj.status.disabled_paralyzed);
        assert_eq!(obj.status.disabled_paralyzed_until_frame, 140);
    }

    restored.frame = 140;
    let obj = restored.host_object_mut(id).expect("post-expiry");
    obj.tick_disabled_paralyzed(140);
    assert!(
        !obj.status.disabled_paralyzed,
        "paralyze must expire on the saved frame after load"
    );
    assert_eq!(obj.status.disabled_paralyzed_until_frame, 0);
}

#[test]
fn parachute_contain_mid_fall_survives_snapshot() {
    let mut source = GameLogic::new();
    source.set_current_frame(30);
    source.templates.insert(
        "AmericaInfantryPilot".into(),
        ThingTemplate::new("AmericaInfantryPilot"),
    );
    source.add_player(Player::new(1, Team::USA, "USA", true));
    let id = source
        .create_object(
            "AmericaInfantryPilot",
            Team::USA,
            Vec3::new(12.0, 180.0, 8.0),
        )
        .expect("create");
    let landing = Vec3::new(40.0, 0.0, 22.0);
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.apply_eject_parachuting();
        obj.open_eject_parachute();
        obj.status.parachute_pitch = 0.15;
        obj.status.parachute_roll = -0.08;
        obj.status.parachute_pitch_rate = 0.02;
        obj.status.parachute_roll_rate = -0.01;
        obj.set_parachute_override_destination(landing);
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert!(obj_snap.status.parachuting);
    assert!(obj_snap.status.parachute_open);
    assert!(obj_snap.status.parachute_start_height > 0.0);
    assert!((obj_snap.status.parachute_pitch - 0.15).abs() < f32::EPSILON);
    assert!((obj_snap.status.parachute_roll + 0.08).abs() < f32::EPSILON);
    assert!((obj_snap.status.parachute_pitch_rate - 0.02).abs() < f32::EPSILON);
    assert!((obj_snap.status.parachute_roll_rate + 0.01).abs() < f32::EPSILON);
    assert_eq!(obj_snap.status.parachute_landing_override, Some(landing));
    assert!(obj_snap.status.parachute_landing_override_set);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    let loaded = restored.host_object(id).expect("restored");
    assert!(loaded.is_parachuting());
    assert!(loaded.is_parachute_open());
    assert!(loaded.status.parachute_start_height > 0.0);
    assert!((loaded.parachute_pitch() - 0.15).abs() < f32::EPSILON);
    assert!((loaded.status.parachute_roll + 0.08).abs() < f32::EPSILON);
    assert!((loaded.status.parachute_pitch_rate - 0.02).abs() < f32::EPSILON);
    assert!((loaded.status.parachute_roll_rate + 0.01).abs() < f32::EPSILON);
    assert_eq!(loaded.parachute_landing_override(), Some(landing));
    assert!(loaded.has_parachute_landing_override());
}

#[test]
fn faerie_fire_timer_survives_snapshot_and_expires_on_saved_frame() {
    let mut source = GameLogic::new();
    source.set_current_frame(60);
    source
        .templates
        .insert("GLARebel".into(), ThingTemplate::new("GLARebel"));
    source.add_player(Player::new(1, Team::GLA, "GLA", true));
    let id = source
        .create_object("GLARebel", Team::GLA, Vec3::new(3.0, 0.0, 3.0))
        .expect("create");
    {
        let obj = source.host_object_mut(id).expect("obj");
        obj.apply_faerie_fire(90);
    }

    let builder = SnapshotBuilder::new();
    let snap = builder.create_world_snapshot(&source).expect("snap");
    let obj_snap = snap.objects.get(&id).expect("obj snap");
    assert!(obj_snap.status.faerie_fire);
    assert_eq!(obj_snap.status.faerie_fire_until_frame, 90);

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snap, &mut restored)
        .expect("restore");
    {
        let obj = restored.host_object(id).expect("restored");
        assert!(obj.is_faerie_fire());
        assert_eq!(obj.faerie_fire_until_frame, 90);
    }

    restored.frame = 89;
    {
        let obj = restored.host_object_mut(id).expect("pre-expiry");
        obj.tick_faerie_fire(89);
        assert!(obj.is_faerie_fire());
        assert_eq!(obj.faerie_fire_until_frame, 90);
    }

    restored.frame = 90;
    let obj = restored.host_object_mut(id).expect("post-expiry");
    obj.tick_faerie_fire(90);
    assert!(
        !obj.is_faerie_fire(),
        "FAERIE_FIRE must expire on the saved frame after load"
    );
    assert_eq!(obj.faerie_fire_until_frame, 0);
}
