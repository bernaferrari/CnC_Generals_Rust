use super::*;

#[test]
fn malformed_body_history_owner_preserves_receiving_history_and_pose() {
    let mut receiving = GameLogic::new();
    receiving.templates.insert(
        "BodyHistoryOwner".into(),
        crate::game_logic::ThingTemplate::new("BodyHistoryOwner"),
    );
    let id = receiving
        .create_object(
            "BodyHistoryOwner",
            crate::game_logic::Team::USA,
            glam::Vec3::new(3.0, 0.0, 9.0),
        )
        .unwrap();
    receiving.host_object_mut(id).unwrap().last_damage_info_type =
        Some(crate::game_logic::combat::DamageType::Flame);
    let matrix = receiving.host_object(id).unwrap().get_transform_matrix();
    let mut payload = capture(&receiving);
    payload.damage_info.push(ObjectDamageInfoPersist {
        object_id: id.0,
        last_damage_type: None,
    });
    let encoded = bincode_legacy::serialize(&payload).unwrap();
    let mut bytes = OXOB_MAGIC.to_vec();
    append_u32(&mut bytes, OXOB_VERSION);
    append_u32(&mut bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
    assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
    assert_eq!(
        receiving.host_object(id).unwrap().last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Flame)
    );
    assert_eq!(
        receiving.host_object(id).unwrap().get_transform_matrix(),
        matrix
    );
}

#[test]
fn malformed_structure_matrix_rejects_before_receiving_state_changes() {
    let mut receiving = GameLogic::new();
    let mut template = crate::game_logic::ThingTemplate::new("ReceivingTopple");
    template.add_kind_of(crate::game_logic::KindOf::Structure);
    template.authored_structure_topple = true;
    receiving.templates.insert(template.name.clone(), template);
    let id = receiving
        .create_object(
            "ReceivingTopple",
            crate::game_logic::Team::USA,
            glam::Vec3::new(2.0, 0.0, 7.0),
        )
        .unwrap();
    assert!(
        receiving
            .host_object_mut(id)
            .unwrap()
            .begin_structure_topple(
                0,
                None,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
            )
    );
    receiving.host_object_mut(id).unwrap().script_targetable = true;
    let before = receiving.host_object(id).unwrap().get_transform_matrix();
    let mut payload = capture(&receiving);
    assert_eq!(payload.structures.len(), 1);
    payload.structures[0].matrix[0] = f32::NAN;
    let encoded = bincode_legacy::serialize(&payload).unwrap();
    let mut bytes = OXOB_MAGIC.to_vec();
    append_u32(&mut bytes, OXOB_VERSION);
    append_u32(&mut bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
    assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
    let object = receiving.host_object(id).unwrap();
    assert!(object.script_targetable);
    assert!(object.structure_topple_data.is_some());
    assert_eq!(object.get_transform_matrix(), before);
}

#[test]
fn malformed_object_payload_preserves_receiving_script_flags_and_clocks() {
    let mut receiving = GameLogic::new();
    let template = crate::game_logic::ThingTemplate::new("ReceivingScriptStatusOwner");
    receiving.templates.insert(template.name.clone(), template);
    let id = receiving
        .create_object(
            "ReceivingScriptStatusOwner",
            crate::game_logic::Team::USA,
            glam::Vec3::ZERO,
        )
        .unwrap();
    let object = receiving.host_object_mut(id).unwrap();
    object.script_targetable = true;
    object.status.masked = true;
    object.single_use_command_used = true;
    object.pre_attack_ready_at = 19.0;
    object.last_fire_frame = 223;

    // Unknown version, truncated declared bytes, and invalid serialized
    // data all fail during candidate decoding, before any object reset.
    for (version, declared_len, payload) in [
        (OXOB_VERSION + 1, 0, Vec::new()),
        (OXOB_VERSION, 2, vec![0]),
        (OXOB_VERSION, 0, Vec::new()),
    ] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, version);
        append_u32(&mut bytes, declared_len);
        bytes.extend_from_slice(&payload);
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        let object = receiving.host_object(id).unwrap();
        assert!(object.script_targetable);
        assert!(object.status.masked);
        assert!(object.single_use_command_used);
        assert_eq!(object.pre_attack_ready_at, 19.0);
        assert_eq!(object.last_fire_frame, 223);
    }
}
use crate::game_logic::object::TurretSubState;
use crate::game_logic::{Player, Team, ThingTemplate};
use glam::Vec3;

/// The AI-owning Ranger every OXOB clock round-trip receives from authored INI.
fn admit_clock_owner(world: &mut GameLogic) {
    crate::fixture_content::admit_fixture_objects(
        world,
        &[crate::fixture_content::FixtureObject::new("AmericaInfantryRanger")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .module("Behavior = AIUpdateInterface ActualClockOwner\nEnd")],
    );
}

#[test]
fn snapshot_round_trips_object_xfer_clocks() {
    let mut source = GameLogic::new();
    admit_clock_owner(&mut source);
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let id = source
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(10.0, 0.0, 8.0),
        )
        .expect("unit");
    {
        let object = source.host_object_mut(id).expect("unit obj");
        object.status.disabled_held = true;
        object.single_use_command_used = true;
        object.ai_attitude = -1;
        object.custom_indicator_color = Some(0xFFFF_0000);
        object.vision_range = 220.0;
        object.shroud_clearing_range = 240.0;
        object.shroud_range = 80.0;
        object.pre_attack_ready_at = 12.5;
        object.pre_attack_target = Some(ObjectId(9));
        object.consecutive_shot_target = Some(ObjectId(9));
        object.max_shots_to_fire = 3;
        object.turret_ai.turret_angle_deg = 45.0;
        object.turret_ai.turret_substate = TurretSubState::Aim;
        object.turret_ai.turret_target_id = Some(ObjectId(9));
        object.stealth_frames_granted = 600;
        object.weapon_bonus_solo = 16;
        object.which_exit_path = 2;
        object.cheer_timer = 1.5;
        object.model_condition_bits |=
            1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
        object.weapon_scatter_targets_unused = [vec![2, 0], vec![1], Vec::new()];
        object.weapon_scatter_targets_inited = [true, true, false];
        object.weapon_bonus_horde = true;
        object.weapon_bonus_enthusiastic = true;
        object.weapon_bonus_subliminal = true;
        object.safe_occlusion_frame = 12_345;
        object.health_box_offset = [12.5, -3.0, 4.25];
        object.last_fire_frame = 4_200;
        object.is_recruitable = false;
        object.ignore_collisions_until_frame = 88;
    }
    source
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_guard_scan_deadline(Some(310));
    source
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_hunt_scan_deadline(Some(640));
    source
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_quick_exit_deadline(Some(930));

    let builder = super::super::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    assert!(
        find_oxob_suffix(&snapshot.lifecycle_tail).is_some(),
        "OXOB suffix must be appended to lifecycle tail"
    );

    // The loading game owns the same authored INI before the save is applied.
    let mut restored = GameLogic::new();
    admit_clock_owner(&mut restored);
    restored.add_player(Player::new(0, Team::USA, "USA", true));
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore");

    let loaded = restored.host_object(id).expect("restored unit");
    assert!(loaded.status.disabled_held);
    assert!(loaded.single_use_command_used);
    assert_eq!(loaded.ai_attitude, -1);
    assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
    assert_eq!(loaded.vision_range, 220.0);
    assert_eq!(loaded.shroud_clearing_range, 240.0);
    assert_eq!(loaded.shroud_range, 80.0);
    assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
    assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
    assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
    assert_eq!(loaded.max_shots_to_fire, 3);
    assert!((loaded.turret_ai.turret_angle_deg - 45.0).abs() < 1e-4);
    assert_eq!(loaded.turret_ai.turret_substate, TurretSubState::Aim);
    assert_eq!(loaded.turret_ai.turret_target_id, Some(ObjectId(9)));
    assert_eq!(loaded.stealth_frames_granted, 600);
    assert_eq!(loaded.weapon_bonus_solo, 16);
    assert!(
        loaded.is_receiving_difficulty_bonus,
        "m_isReceivingDifficultyBonus latch follows restored SOLO bits"
    );
    assert_eq!(loaded.which_exit_path, 2);
    assert!((loaded.cheer_timer - 1.5).abs() < 1e-4);
    assert!(
        object_has_special_cheering(&loaded),
        "SPECIAL_CHEERING must survive load"
    );
    assert_eq!(
        loaded.weapon_scatter_targets_unused,
        [vec![2, 0], vec![1], Vec::new()]
    );
    assert_eq!(loaded.weapon_scatter_targets_inited, [true, true, false]);
    assert!(loaded.weapon_bonus_horde);
    assert!(loaded.weapon_bonus_enthusiastic);
    assert!(loaded.weapon_bonus_subliminal);
    assert_eq!(loaded.safe_occlusion_frame, 12_345);
    assert_eq!(loaded.health_box_offset, [12.5, -3.0, 4.25]);
    assert_eq!(loaded.last_fire_frame, 4_200);
    assert!(
        !loaded.is_recruitable,
        "m_isRecruitable=false must survive load"
    );
    assert_eq!(loaded.ignore_collisions_until_frame, 88);
    assert_eq!(
        restored
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.guard_scan_deadline()),
        Some(310)
    );
    assert_eq!(
        restored
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.hunt_scan_deadline()),
        Some(640)
    );
    assert_eq!(
        restored
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.quick_exit_deadline()),
        Some(930),
        "quick-exit deadline must survive SnapshotBuilder/OXOB restore"
    );
}

#[test]
fn absent_suffix_clears_stale_object_xfer() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = logic.host_object_mut(id).expect("unit");
        object.status.disabled_held = true;
        object.single_use_command_used = true;
        object.custom_indicator_color = Some(0xFF00_FF00);
        object.pre_attack_ready_at = 4.0;
        object.pre_attack_target = Some(ObjectId(2));
        object.consecutive_shot_target = Some(ObjectId(2));
        object.max_shots_to_fire = 1;
        object.weapon_bonus_solo = 18;
        object.which_exit_path = 3;
        object.cheer_timer = 2.0;
        object.model_condition_bits |=
            1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
        object.weapon_scatter_targets_unused = [vec![3], Vec::new(), Vec::new()];
        object.weapon_scatter_targets_inited = [true, false, false];
        object.weapon_bonus_horde = true;
        object.weapon_bonus_enthusiastic = true;
        object.weapon_bonus_subliminal = true;
        object.health_box_offset = [1.0, 2.0, 3.0];
        object.last_fire_frame = 99;
        object.is_recruitable = false;
        object.ignore_collisions_until_frame = 44;
    }
    logic
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_guard_scan_deadline(Some(12));
    logic
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_hunt_scan_deadline(Some(24));
    logic
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_quick_exit_deadline(Some(930));
    apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
    let object = logic.host_object(id).expect("unit");
    assert!(!object.status.disabled_held);
    assert!(!object.single_use_command_used);
    assert!(!object.is_receiving_difficulty_bonus);
    assert_eq!(object.custom_indicator_color, None);
    assert_eq!(object.pre_attack_ready_at, 0.0);
    assert_eq!(object.pre_attack_target, None);
    assert_eq!(object.consecutive_shot_target, None);
    assert_eq!(object.max_shots_to_fire, -1);
    assert_eq!(object.weapon_bonus_solo, 0);
    assert_eq!(object.which_exit_path, 0);
    assert_eq!(object.cheer_timer, 0.0);
    assert!(!object_has_special_cheering(&object));
    assert!(
        object
            .weapon_scatter_targets_unused
            .iter()
            .all(Vec::is_empty)
    );
    assert_eq!(object.weapon_scatter_targets_inited, [false, false, false]);
    assert!(!object.weapon_bonus_horde);
    assert!(!object.weapon_bonus_enthusiastic);
    assert!(!object.weapon_bonus_subliminal);
    assert_eq!(object.health_box_offset, [0.0, 0.0, 0.0]);
    assert_eq!(object.last_fire_frame, 0);
    assert!(
        object.is_recruitable,
        "absent OXOB resets m_isRecruitable to leftover default true"
    );
    assert_eq!(object.ignore_collisions_until_frame, 0);
    assert!(
        logic
            .host_objects()
            .registered_values()
            .all(|object| object.unit_ai_runtime.guard_scan_deadline() == Some(12))
    );
    assert!(
        logic
            .host_objects()
            .registered_values()
            .all(|object| object.unit_ai_runtime.hunt_scan_deadline() == Some(24))
    );
    assert!(
        logic
            .host_objects()
            .registered_values()
            .all(|object| object.unit_ai_runtime.quick_exit_deadline() == Some(930))
    );
}

#[test]
fn oxob_round_trips_indicator_vision_single_use_preattack() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let id = source
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = source.host_object_mut(id).expect("unit");
        object.single_use_command_used = true;
        object.custom_indicator_color = Some(0xFFFF_0000);
        object.vision_range = 220.0;
        object.shroud_clearing_range = 240.0;
        object.shroud_range = 80.0;
        object.pre_attack_ready_at = 12.5;
        object.pre_attack_target = Some(ObjectId(9));
        object.consecutive_shot_target = Some(ObjectId(9));
        object.max_shots_to_fire = 3;
    }

    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    assert!(
        find_oxob_suffix(&bytes).is_some(),
        "OXOB suffix must encode indicator/vision/single-use/pre-attack"
    );

    let mut dest = GameLogic::new();
    dest.templates = source.templates.clone();
    dest.add_player(Player::new(0, Team::USA, "USA", true));
    let dest_id = dest
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("dest unit");
    assert_eq!(dest_id, id);
    dest.host_object_mut(dest_id)
        .unwrap()
        .restore_base_ai_continuation(
            source
                .host_object(id)
                .unwrap()
                .capture_base_ai_continuation(),
        )
        .unwrap();
    apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

    let loaded = dest.host_object(dest_id).expect("loaded");
    assert!(loaded.single_use_command_used);
    assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
    assert_eq!(loaded.vision_range, 220.0);
    assert_eq!(loaded.shroud_clearing_range, 240.0);
    assert_eq!(loaded.shroud_range, 80.0);
    assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
    assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
    assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
    assert_eq!(loaded.max_shots_to_fire, 3);
}

#[test]
fn oxob_round_trips_cheer_scatter_horde_bonus() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let id = source
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = source.host_object_mut(id).expect("unit");
        object.cheer_timer = 2.25;
        object.model_condition_bits |=
            1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
        object.weapon_scatter_targets_unused = [vec![4, 1, 0], Vec::new(), vec![2]];
        object.weapon_scatter_targets_inited = [true, false, true];
        object.weapon_bonus_horde = true;
        object.weapon_bonus_enthusiastic = true;
        object.weapon_bonus_subliminal = true;
    }

    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    assert!(find_oxob_suffix(&bytes).is_some());

    let mut dest = GameLogic::new();
    dest.templates = source.templates.clone();
    dest.add_player(Player::new(0, Team::USA, "USA", true));
    let dest_id = dest
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("dest unit");
    apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

    let loaded = dest.host_object(dest_id).expect("loaded");
    assert!((loaded.cheer_timer - 2.25).abs() < 1e-4);
    assert!(object_has_special_cheering(&loaded));
    assert_eq!(
        loaded.weapon_scatter_targets_unused,
        [vec![4, 1, 0], Vec::new(), vec![2]]
    );
    assert_eq!(loaded.weapon_scatter_targets_inited, [true, false, true]);
    assert!(loaded.weapon_bonus_horde);
    assert!(loaded.weapon_bonus_enthusiastic);
    assert!(loaded.weapon_bonus_subliminal);
}

#[test]
fn oxob_round_trips_safe_occlusion_frame() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let id = source
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = source.host_object_mut(id).expect("unit");
        // Garrison/tunnel exit stamp (now + delay) or HUGE_FRAME leftover.
        object.safe_occlusion_frame = 30_090;
    }

    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    assert!(find_oxob_suffix(&bytes).is_some());

    let mut dest = GameLogic::new();
    dest.templates = source.templates.clone();
    dest.add_player(Player::new(0, Team::USA, "USA", true));
    let dest_id = dest
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("dest unit");
    {
        let object = dest.host_object_mut(dest_id).expect("dest");
        object.safe_occlusion_frame = 90;
    }
    apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

    let loaded = dest.host_object(dest_id).expect("loaded");
    assert_eq!(
        loaded.safe_occlusion_frame, 30_090,
        "m_safeOcclusionFrame must survive load"
    );
}

#[test]
fn absent_suffix_keeps_template_safe_occlusion_frame() {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = logic.host_object_mut(id).expect("unit");
        object.safe_occlusion_frame = 777;
    }
    apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
    let object = logic.host_object(id).expect("unit");
    assert_eq!(
        object.safe_occlusion_frame, 777,
        "absent OXOB must not wipe template/garrison occlusion clock"
    );
}

#[test]
fn oxob_round_trips_health_box_last_fire_recruit_ignore() {
    let mut source = GameLogic::new();
    source.templates.insert(
        "AmericaInfantryRanger".to_string(),
        ThingTemplate::new("AmericaInfantryRanger"),
    );
    source.add_player(Player::new(0, Team::USA, "USA", true));
    let id = source
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let object = source.host_object_mut(id).expect("unit");
        object.health_box_offset = [8.0, 20.0, -1.5];
        object.last_fire_frame = 77;
        object.is_recruitable = false;
        object.ignore_collisions_until_frame = 150;
        object.locomotor_upgrade = true;
        object.jet_ai.cur_locomotor_set = Some("SET_NORMAL_UPGRADED".to_string());
        object.braking = 40.0;
        object.max_lift = 9.5;
        object.braking_factor = 0.25;
        object.close_enough_dist = Some(25.0);
        object.close_enough_dist_3d = true;
        object.donut_timer = 80;
        object.loco_preferred_height = 12.0;
        object.loco_preferred_height_damping = 0.4;
        object.is_braking = true;
        object.is_climbing = true;
        object.doing_three_point_turn = true;
        object.cur_locomotor_name = Some("WorkerShoes".to_string());
        object.locomotor_surfaces = 0x11;
        object.downhill_only = true;
        object.maintain_pos_valid = true;
        object.maintain_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
        object.wander_angle_offset = 0.7;
        object.wander_offset_increment = 0.15;
        object.wander_offset_increasing = true;
    }
    // Guard/hunt scan clocks travel in the BaseAI capsule (AIUpdateInterface::xfer),
    // not in the OXOB record; snapshot_round_trips_object_xfer_clocks covers them.

    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    assert!(find_oxob_suffix(&bytes).is_some());

    let mut dest = GameLogic::new();
    dest.templates = source.templates.clone();
    dest.add_player(Player::new(0, Team::USA, "USA", true));
    let dest_id = dest
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
        .expect("dest unit");
    {
        let object = dest.host_object_mut(dest_id).expect("dest");
        object.health_box_offset = [99.0, 99.0, 99.0];
        object.last_fire_frame = 1;
        object.is_recruitable = true;
        object.ignore_collisions_until_frame = 3;
        object.locomotor_surfaces = 0x40;
        object.cur_locomotor_name = Some("CreateTime".to_string());
    }
    apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

    let loaded = dest.host_object(dest_id).expect("loaded");
    assert_eq!(
        loaded.health_box_offset,
        [8.0, 20.0, -1.5],
        "m_healthBoxOffset must survive load"
    );
    assert_eq!(
        loaded.last_fire_frame, 77,
        "Weapon::m_lastFireFrame must survive load"
    );
    assert!(
        !loaded.is_recruitable,
        "AIUpdateInterface::m_isRecruitable=false must survive load"
    );
    assert_eq!(
        loaded.ignore_collisions_until_frame, 150,
        "m_ignoreCollisionsUntil must survive load"
    );
    assert!(loaded.locomotor_upgrade);
    assert_eq!(
        loaded.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_NORMAL_UPGRADED")
    );
    assert!((loaded.braking - 40.0).abs() < 0.001);
    assert!((loaded.max_lift - 9.5).abs() < 0.001);
    assert!((loaded.braking_factor - 0.25).abs() < 0.001);
    assert_eq!(loaded.close_enough_dist, Some(25.0));
    assert!(loaded.close_enough_dist_3d);
    assert_eq!(loaded.donut_timer, 80);
    assert!((loaded.loco_preferred_height - 12.0).abs() < 0.001);
    assert!((loaded.loco_preferred_height_damping - 0.4).abs() < 0.001);
    assert!(loaded.is_braking);
    assert!(loaded.is_climbing);
    assert!(loaded.doing_three_point_turn);
    assert_eq!(loaded.cur_locomotor_name.as_deref(), Some("WorkerShoes"));
    assert_eq!(loaded.locomotor_surfaces, 0x11);
    assert!(loaded.downhill_only);
    assert!(loaded.maintain_pos_valid);
    assert_eq!(loaded.maintain_pos, Some(glam::Vec3::new(3.0, 1.0, 4.0)));
    assert!((loaded.wander_angle_offset - 0.7).abs() < 0.001);
    assert!((loaded.wander_offset_increment - 0.15).abs() < 0.001);
    assert!(loaded.wander_offset_increasing);
}
