//! Behavior suite extracted from the original test module.
use super::super::*;

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
fn direct_xfer_current_preserves_hacker_disable_tail_and_following_record() {
    use super::super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::game_logic::{HackerDisableChannelPhase, HackerDisableChannelState};
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(93);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
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

    assert_eq!(restored.version, WORLD_SNAPSHOT_DIRECT_XFER_VERSION);
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
fn direct_xfer_current_round_trips_logical_and_client_drawable_tails() {
    use super::super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(95);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
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
fn direct_xfer_current_round_trips_exact_player_template_binding_tail() {
    use super::super::xfer_helpers::default_object_snapshot;
    use crate::game_logic::SupplyTruckState;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
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
fn direct_xfer_current_round_trips_exact_shroud_tail() {
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
fn direct_xfer_current_round_trips_weapon_suspend_fx_tail_and_keeps_alignment() {
    use super::super::xfer_helpers::default_player_snapshot;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let object_id = ObjectId(96);
    let mut world = WorldSnapshot::default();
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
    let mut object = ObjectSnapshot {
        id: object_id,
        ..super::super::xfer_helpers::default_object_snapshot()
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
fn direct_xfer_current_round_trips_temporary_weapon_tail_and_keeps_alignment() {
    use super::super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
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
    world.version = WORLD_SNAPSHOT_DIRECT_XFER_VERSION;
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

#[test]
fn direct_xfer_current_appends_weapon_clip_residual_and_keeps_alignment() {
    use super::super::xfer_helpers::{default_object_snapshot, default_player_snapshot};
    use crate::game_logic::Weapon;
    use crate::save_load::{Xfer, XferLoad, XferSave};
    use std::io::Cursor;

    let clip_residual = |clip_size: u32,
                         clip_reload_time: f32,
                         splash_radius: f32,
                         reloading_clip: bool,
                         last_bonus_rof: f32| Weapon {
        clip_size,
        clip_reload_time,
        splash_radius,
        reloading_clip,
        last_bonus_rof,
        ..Weapon::default()
    };

    let build_world = |version: u32| {
        let mut world = WorldSnapshot::default();
        world.version = version;
        world.frame_number = 77;
        let object_id = ObjectId(404);
        let mut object = default_object_snapshot();
        object.id = object_id;
        object.template_name = "ClipResidualObject".to_string();
        object.weapons.push(clip_residual(5, 3.0, 12.5, true, 1.5));
        world.objects.insert(object_id, object);

        let mut player = default_player_snapshot();
        player.id = 3;
        player.name = "PostWeaponAlignment".to_string();
        world.players.push(player);
        world
    };

    // v21: the clip/splash/reload residual rides along and the reader stays
    // aligned for the fields after the object map.
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = XferSave::new(&mut bytes);
        build_world(WORLD_SNAPSHOT_DIRECT_XFER_VERSION)
            .xfer(&mut writer)
            .expect("write v21 world");
        let mut sentinel = 0xC0DE_CAFEu32;
        writer.xfer_u32(&mut sentinel).expect("write sentinel");
    }
    let mut restored = WorldSnapshot::default();
    let mut sentinel = 0u32;
    {
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        restored.xfer(&mut reader).expect("read v21 world");
        reader.xfer_u32(&mut sentinel).expect("read sentinel");
    }
    assert_eq!(restored.players[0].name, "PostWeaponAlignment");
    assert_eq!(sentinel, 0xC0DE_CAFE);
    let weapon = &restored
        .objects
        .get(&ObjectId(404))
        .expect("restored v21 object")
        .weapons[0];
    assert_eq!(weapon.clip_size, 5);
    assert!((weapon.clip_reload_time - 3.0).abs() < 1e-4);
    assert!((weapon.splash_radius - 12.5).abs() < 1e-4);
    assert!(weapon.reloading_clip);
    assert!((weapon.last_bonus_rof - 1.5).abs() < 1e-4);
}
