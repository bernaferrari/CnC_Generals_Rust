//! Healing, AI mood/guard, physics, turret, projectile, fire-intent channels.

use super::*;

#[test]
fn sole_healing_channel_via_set_sole_healing() {
    use crate::game_logic::host_sole_healing_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SoleHeal");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["HealTgt", "DozerA"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert(name.into(), t);
        }
    }
    let tgt = logic
        .create_object("HealTgt", Team::USA, glam::Vec3::new(30.0, 0.0, 30.0))
        .expect("tgt");
    let dozer = logic
        .create_object("DozerA", Team::USA, glam::Vec3::new(32.0, 0.0, 30.0))
        .expect("dozer");
    {
        let o = logic.host_object_mut(tgt).expect("o");
        o.sole_healing_benefactor = Some(dozer);
        o.sole_healing_benefactor_expiration_frame = 900;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&tgt.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.sole_healing_benefactor_id, Some(dozer.0));
    assert_eq!(e.sole_healing_benefactor_expiration_frame, 900);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&tgt).unwrap();
    assert_eq!(o.sole_healing_benefactor, Some(dozer));
    assert_eq!(o.sole_healing_benefactor_expiration_frame, 900);
}

#[test]
fn ai_mood_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiMood");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MoodU") {
        let mut t = ThingTemplate::new("MoodU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("MoodU".into(), t);
    }
    let oid = logic
        .create_object("MoodU", Team::USA, glam::Vec3::new(40.0, 0.0, 40.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.idle_since_frame = 120;
        o.mood_attack_check_rate = 45;
        o.auto_acquire_when_idle = false;
        o.attack_priority_set = Some("Soldier".into());
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.idle_since_frame, 120);
    assert_eq!(e.mood_attack_check_rate, 45);
    assert!(!e.auto_acquire_when_idle);
    assert_eq!(e.attack_priority_set, "Soldier");
    let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
    e.idle_since_frame = 0;
    e.mood_attack_check_rate = 30;
    e.auto_acquire_when_idle = true;
    e.attack_priority_set.clear();
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.idle_since_frame, 120);
    assert_eq!(o.mood_attack_check_rate, 45);
    assert!(!o.auto_acquire_when_idle);
    assert_eq!(o.attack_priority_set.as_deref(), Some("Soldier"));
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.idle_since_frame, 120);
    assert_eq!(e.mood_attack_check_rate, 45);
    assert!(!e.auto_acquire_when_idle);
    assert_eq!(e.attack_priority_set, "Soldier");
}

#[test]
fn guard_radius_is_owned_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardR");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("GuardU") {
        let mut t = ThingTemplate::new("GuardU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("GuardU".into(), t);
    }
    let oid = logic
        .create_object("GuardU", Team::USA, glam::Vec3::new(50.0, 0.0, 50.0))
        .expect("id");
    let position = glam::Vec3::new(55.0, 0.0, 55.0);
    logic
        .host_object_mut(oid)
        .unwrap()
        .set_guard_position(Some(position));
    assert!(logic.unit_command_set_guard_radius(oid, 175.0));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().guard_radius, 175.0);
    assert_eq!(
        shadow.world().entity(entity).unwrap().guard_position,
        Some([55.0, 0.0, 55.0])
    );
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .guard_radius = 999.0;
    assert_eq!(logic.host_object(oid).unwrap().guard_radius, 175.0);
    assert!(logic.unit_command_set_guard_radius(oid, 80.0));
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(entity).unwrap().guard_radius, 80.0);
    assert_eq!(
        logic.host_object(oid).unwrap().guard_position,
        Some(position)
    );
}

#[test]
fn production_door_channel_via_set_production_door() {
    use crate::game_logic::host_production_door_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdDoor");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DoorFact") {
        let mut t = ThingTemplate::new("DoorFact");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("DoorFact".into(), t);
    }
    let oid = logic
        .create_object("DoorFact", Team::USA, glam::Vec3::new(60.0, 0.0, 60.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.production_door_phase = 2;
        o.production_door_phase_end_frame = 500;
        o.production_door_hold_open = true;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.production_door_phase, 2);
    assert_eq!(e.production_door_phase_end_frame, 500);
    assert!(e.production_door_hold_open);

    shadow.sync_from_host(&logic);
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.production_door_phase, 2);
    assert_eq!(o.production_door_phase_end_frame, 500);
    assert!(o.production_door_hold_open);
}

#[test]
fn physics_motive_channel_via_set_physics_motive() {
    use crate::game_logic::host_physics_motive_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PhysMot");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PhysU") {
        let mut t = ThingTemplate::new("PhysU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("PhysU".into(), t);
    }
    let oid = logic
        .create_object("PhysU", Team::USA, glam::Vec3::new(70.0, 0.0, 70.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.motive_frames_remaining = 12;
        o.physics_mass = 2.5;
        o.physics_accel = glam::Vec3::new(1.0, 0.0, 0.5);
        o.forward_friction = 0.15;
        o.lateral_friction = 0.2;
        o.z_friction = 0.1;
        o.can_path_through_units = true;
        o.ignore_collisions_until_frame = 40;
        o.is_panicking = true;
        o.move_away_frames = 5;
        o.aerodynamic_friction = 0.05;
        o.extra_friction = 0.02;
        o.apply_friction_2d_when_airborne = true;
        o.center_of_mass_offset = -0.5;
        o.pitch_roll_yaw_factor = 1.2;
        o.immune_to_falling_damage = true;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.motive_frames_remaining, 12);
    assert!((e.physics_mass - 2.5).abs() < 1e-5);
    assert!((e.physics_accel[0] - 1.0).abs() < 1e-5);
    assert!(e.can_path_through_units);
    assert!(e.is_panicking);
    assert_eq!(e.ignore_collisions_until_frame, 40);
    assert!((e.aerodynamic_friction - 0.05).abs() < 1e-5);
    assert!(e.immune_to_falling_damage);
    assert!((e.aerodynamic_friction - 0.05).abs() < 1e-5);
    assert!(e.immune_to_falling_damage);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.motive_frames_remaining, 12);
    assert!((o.physics_mass - 2.5).abs() < 1e-5);
    assert!(o.can_path_through_units);
    assert!(o.is_panicking);
    assert_eq!(o.ignore_collisions_until_frame, 40);
    assert!((o.aerodynamic_friction - 0.05).abs() < 1e-5);
    assert!(o.immune_to_falling_damage);
    assert!((o.aerodynamic_friction - 0.05).abs() < 1e-5);
    assert!(o.immune_to_falling_damage);
}

#[test]
fn bounce_land_channel_via_set_bounce_land() {
    use crate::game_logic::host_bounce_land_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BounceL");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("BounceU") {
        let mut t = ThingTemplate::new("BounceU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("BounceU".into(), t);
    }
    let oid = logic
        .create_object("BounceU", Team::USA, glam::Vec3::new(80.0, 0.0, 80.0))
        .expect("id");
    let other = logic
        .create_object("BounceU", Team::USA, glam::Vec3::new(82.0, 0.0, 80.0))
        .expect("other");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.kill_when_resting_on_ground = true;
        o.bounce_land_events = 3;
        o.last_bounce_fall_dy = 12.0;
        o.bounce_sound_name = "Module:Bounce".into();
        o.last_bounce_volume = 0.75;
        o.bounce_audio_pending = 2;
        o.allow_collide_force = false;
        o.last_collidee = Some(other);
        o.ignore_collisions_with = Some(other);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.kill_when_resting_on_ground);
    assert_eq!(e.bounce_land_events, 3);
    assert!((e.last_bounce_fall_dy - 12.0).abs() < 1e-5);
    assert_eq!(e.bounce_sound_name, "Module:Bounce");
    assert!((e.last_bounce_volume - 0.75).abs() < 1e-5);
    assert_eq!(e.bounce_audio_pending, 2);
    assert!(!e.allow_collide_force);
    assert_eq!(e.last_collidee_id, Some(other.0));

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.kill_when_resting_on_ground);
    assert_eq!(o.bounce_land_events, 3);
    assert_eq!(o.bounce_audio_pending, 2);
    assert_eq!(o.last_collidee, Some(other));
}

#[test]
fn turret_extended_channel_via_set_turret() {
    use crate::game_logic::host_turret_log;
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TurretX");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("TurU") {
        let mut t = ThingTemplate::new("TurU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("TurU".into(), t);
    }
    let oid = logic
        .create_object("TurU", Team::USA, glam::Vec3::new(90.0, 0.0, 90.0))
        .expect("id");
    let tgt = logic
        .create_object("TurU", Team::China, glam::Vec3::new(100.0, 0.0, 90.0))
        .expect("tgt");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_angle_deg = 45.0;
        o.turret_pitch_deg = 10.0;
        o.turret_holding = true;
        o.turret_idle_scanning = false;
        o.turret_turn_rate_rad = 0.05;
        o.turret_recenter_frames = 60;
        o.turret_hold_until_frame = 200;
        o.turret_idle_recentering = true;
        o.turret_enabled = true;
        o.turret_rotating = true;
        o.turret_natural_angle_deg = 0.0;
        o.turret_natural_pitch_deg = 5.0;
        o.turret_target_id = Some(tgt);
        o.turret_force_attacking = true;
        o.turret_mood_target = false;
        o.turret_idle_scan_next_frame = 30;
        o.turret_idle_scan_desired_angle_deg = 90.0;
        o.turret_idle_scan_index = 2;
        o.turret_substate = TurretSubState::Aim;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.turret_angle_deg - 45.0).abs() < 1e-5);
    assert!((e.turret_turn_rate_rad - 0.05).abs() < 1e-5);
    assert_eq!(e.turret_recenter_frames, 60);
    assert!(e.turret_enabled);
    assert!(e.turret_rotating);
    assert_eq!(e.turret_target_host, tgt.0);
    assert_eq!(e.turret_substate, TurretSubState::Aim.ordinal());

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!((o.turret_angle_deg - 45.0).abs() < 1e-5);
    assert!((o.turret_turn_rate_rad - 0.05).abs() < 1e-5);
    assert!(o.turret_enabled);
    assert_eq!(o.turret_target_id, Some(tgt));
    assert_eq!(o.turret_substate, TurretSubState::Aim);
}

#[test]
fn stealth_delay_channel_via_set_stealth_delay() {
    use crate::game_logic::host_stealth_delay_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StealthD");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("StlU") {
        let mut t = ThingTemplate::new("StlU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("StlU".into(), t);
    }
    let oid = logic
        .create_object("StlU", Team::USA, glam::Vec3::new(110.0, 0.0, 110.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.stealth_allowed_frame = 300;
        o.stealth_delay_pending = true;
        o.stealth_delay_frames = 75;
        o.stealth_breaks_on_damage = true;
        o.detection_expires_frame = 450;
        o.camo_opacity_pulse_phase = 1.25;
        o.camo_heat_vision_opacity = 1.0;
        o.camo_net_sub_object_shown = true;
        o.camo_net_sub_object_observer_visible = true;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.stealth_allowed_frame, 300);
    assert!(e.stealth_delay_pending);
    assert_eq!(e.stealth_delay_frames, 75);
    assert!(e.stealth_breaks_on_damage);
    assert_eq!(e.detection_expires_frame, 450);
    assert!((e.camo_opacity_pulse_phase - 1.25).abs() < 1e-5);
    assert!(e.camo_net_sub_object_shown);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.stealth_delay_pending);
    assert_eq!(o.stealth_allowed_frame, 300);
    assert_eq!(o.stealth_delay_frames, 75);
    assert!(o.camo_net_sub_object_shown);
}

#[test]
fn combat_attack_channel_via_set_combat_attack() {
    use crate::game_logic::{AttackSubState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CbtAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CbtU") {
        let mut t = ThingTemplate::new("CbtU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CbtU".into(), t);
    }
    let oid = logic
        .create_object("CbtU", Team::USA, glam::Vec3::new(130.0, 0.0, 130.0))
        .expect("id");
    let tgt = logic
        .create_object("CbtU", Team::China, glam::Vec3::new(160.0, 0.0, 130.0))
        .expect("tgt");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.pre_attack_target = Some(tgt);
        o.pre_attack_ready_at = 12.5;
        o.consecutive_shots_at_target = 3;
        o.max_shots_to_fire = 5;
        o.attack_substate = AttackSubState::FireWeapon;
        o.approach_timestamp = 90;
        o.consecutive_shot_target = Some(ObjectId(tgt.0));
        o.maintain_pos_valid = true;
        o.maintain_pos = Some(glam::Vec3::new(1.0, 2.0, 3.0));
        o.temporary_move_frames = 7;
        o.group_speed_factor = 0.85;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.pre_attack_target_host, tgt.0);
    assert!((e.pre_attack_ready_at - 12.5).abs() < 1e-5);
    assert_eq!(e.consecutive_shots_at_target, 3);
    assert_eq!(e.max_shots_to_fire, 5);
    assert_eq!(e.attack_substate_ordinal, 1);
    assert_eq!(e.approach_timestamp, 90);
    assert_eq!(e.continuous_fire_victim, tgt.0);
    assert!(e.maintain_pos_valid);
    assert_eq!(e.maintain_pos, Some([1.0, 2.0, 3.0]));
    assert_eq!(e.temporary_move_frames, 7);
    assert!((e.group_speed_factor - 0.85).abs() < 1e-5);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.pre_attack_target, Some(tgt));
    assert_eq!(o.attack_substate, AttackSubState::FireWeapon);
    assert_eq!(o.consecutive_shots_at_target, 3);
    assert_eq!(o.maintain_pos, Some(glam::Vec3::new(1.0, 2.0, 3.0)));
    assert!((o.group_speed_factor - 0.85).abs() < 1e-5);
}

#[test]
fn locomotor_channel_via_set_locomotor() {
    use crate::game_logic::host_locomotor_log;
    use crate::game_logic::{KindOf, LocomotorAppearance, LocomotorBehaviorZ, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Loco");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("LocoU") {
        let mut t = ThingTemplate::new("LocoU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LocoU".into(), t);
    }
    let oid = logic
        .create_object("LocoU", Team::USA, glam::Vec3::new(150.0, 0.0, 150.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.is_approach_path = true;
        o.on_invalid_movement_terrain = true;
        o.was_airborne_last_frame = true;
        o.can_move_backward = true;
        o.moving_backwards = true;
        o.no_slow_down_as_approaching_dest = true;
        o.turn_pivot_offset = -0.5;
        o.wander_width_factor = 0.2;
        o.loco_apply_2d_friction_airborne = true;
        o.loco_extra_2d_friction = 0.03;
        o.loco_preferred_height = 40.0;
        o.loco_preferred_height_damping = 0.7;
        o.loco_appearance = LocomotorAppearance::Wings;
        o.loco_behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        o.min_turn_speed = 5.5;
        o.physics_turning = crate::game_logic::PhysicsTurningType::TurnPositive;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.is_approach_path);
    assert!(e.was_airborne_last_frame);
    assert!(e.moving_backwards);
    assert!((e.turn_pivot_offset + 0.5).abs() < 1e-5);
    assert!((e.loco_preferred_height - 40.0).abs() < 1e-5);
    assert_eq!(
        e.loco_appearance_ordinal,
        LocomotorAppearance::Wings.to_ordinal()
    );
    assert_eq!(
        e.loco_behavior_z_ordinal,
        LocomotorBehaviorZ::AbsoluteHeight.to_ordinal()
    );
    assert!((e.min_turn_speed - 5.5).abs() < 1e-5);
    assert_eq!(e.physics_turning_ordinal, 1);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.is_approach_path);
    assert!(o.moving_backwards);
    assert_eq!(o.loco_appearance, LocomotorAppearance::Wings);
    assert!((o.loco_preferred_height - 40.0).abs() < 1e-5);
    assert!((o.min_turn_speed - 5.5).abs() < 1e-5);
    assert_eq!(
        o.physics_turning,
        crate::game_logic::PhysicsTurningType::TurnPositive
    );
}

#[test]
fn ai_request_channel_via_set_ai_request() {
    use crate::game_logic::host_ai_request_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiReq");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AiU") {
        let mut t = ThingTemplate::new("AiU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AiU".into(), t);
    }
    let oid = logic
        .create_object("AiU", Team::USA, glam::Vec3::new(170.0, 0.0, 170.0))
        .expect("id");
    let victim = logic
        .create_object("AiU", Team::China, glam::Vec3::new(200.0, 0.0, 170.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.requested_victim_id = Some(victim);
        o.requested_destination = Some(glam::Vec3::new(9.0, 0.0, 8.0));
        o.prev_victim_pos = Some(glam::Vec3::new(1.0, 2.0, 3.0));
        o.crate_created = Some(ObjectId(99));
        o.guard_retaliate_victim = Some(victim);
        o.guard_retaliate_anchor = Some(glam::Vec3::new(4.0, 0.0, 5.0));
        o.path_timestamp = 77;
        o.disguise_pending_template = Some("FakeTank".into());
        o.disguise_pending_team = Some(Team::GLA);
        o.weapon_crate_upgrade = 2;
        o.armor_crate_upgrade = 1;
        o.selection_flash_remaining = 15;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.requested_victim_id, Some(victim.0));
    assert_eq!(e.requested_destination, Some([9.0, 0.0, 8.0]));
    assert_eq!(e.prev_victim_pos, Some([1.0, 2.0, 3.0]));
    assert_eq!(e.crate_created_host, 99);
    assert_eq!(e.guard_retaliate_victim_host, victim.0);
    assert_eq!(e.path_timestamp, 77);
    assert_eq!(e.disguise_pending_template, "FakeTank");
    assert_eq!(e.disguise_pending_team_ordinal, 2);
    assert_eq!(e.weapon_crate_upgrade, 2);
    assert_eq!(e.selection_flash_remaining, 15);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.requested_victim_id, Some(victim));
    assert_eq!(o.disguise_pending_template.as_deref(), Some("FakeTank"));
    assert_eq!(o.disguise_pending_team, Some(Team::GLA));
    assert_eq!(o.weapon_crate_upgrade, 2);
    assert_eq!(o.selection_flash_remaining, 15);
    assert_eq!(o.crate_created, Some(ObjectId(99)));
}

#[test]
fn hijacker_channel_via_set_hijacker() {
    use crate::game_logic::host_hijacker_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Hijack");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("HjU") {
        let mut t = ThingTemplate::new("HjU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HjU".into(), t);
    }
    let oid = logic
        .create_object("HjU", Team::USA, glam::Vec3::new(180.0, 0.0, 180.0))
        .expect("id");
    let vehicle = logic
        .create_object("HjU", Team::China, glam::Vec3::new(190.0, 0.0, 180.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hijack_vehicle_id = Some(vehicle);
        o.hijacker_in_vehicle = true;
        o.hijacker_update_active = true;
        o.hijacker_was_airborne = true;
        o.hijacker_eject_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
        o.hive_slave_respawn_frame = 250;
        o.next_detection_scan_frame = 33;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.hijack_vehicle_host, vehicle.0);
    assert!(e.hijacker_in_vehicle);
    assert!(e.hijacker_update_active);
    assert!(e.hijacker_was_airborne);
    assert_eq!(e.hijacker_eject_pos, Some([3.0, 1.0, 4.0]));
    assert_eq!(e.hive_slave_respawn_frame, 250);
    assert_eq!(e.next_detection_scan_frame, 33);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.hijack_vehicle_id, Some(vehicle));
    assert!(o.hijacker_in_vehicle);
    assert_eq!(o.hive_slave_respawn_frame, 250);
    assert_eq!(o.next_detection_scan_frame, 33);
    assert_eq!(o.hijacker_eject_pos, Some(glam::Vec3::new(3.0, 1.0, 4.0)));
}

#[test]
fn leech_range_channel_via_set_weapon_stats() {
    use crate::game_logic::host_weapon_stats_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Leech");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("LchU") {
        let mut t = ThingTemplate::new("LchU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("LchU".into(), t);
    }
    let oid = logic
        .create_object("LchU", Team::USA, glam::Vec3::new(210.0, 0.0, 210.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.leech_range_active_primary = true;
        o.leech_range_active_secondary = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.leech_range_active_primary);
    assert!(e.leech_range_active_secondary);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.leech_range_active_primary);
    assert!(o.leech_range_active_secondary);
}

#[test]
fn fire_intent_channel_via_set_fire_intent() {
    // writeback_fire_intent_to_host is gated on the opt-in AI attack
    // authority (host sole writer default after 0c4d18623); the channel
    // under test is the shadow writeback, so opt in like the projectile
    // and movement authority tests.
    use crate::game_logic::host_fire_intent_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("FireInt");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("FiU") {
        let mut t = ThingTemplate::new("FiU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("FiU".into(), t);
    }
    let oid = logic
        .create_object("FiU", Team::USA, glam::Vec3::new(220.0, 0.0, 220.0))
        .expect("id");
    let victim = logic
        .create_object("FiU", Team::China, glam::Vec3::new(240.0, 0.0, 220.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.last_fire_victim_host = victim.0;
        o.last_fire_slot = 1;
        o.last_fire_damage = 42.0;
        o.last_fire_range = 150.0;
        o.last_fire_sim_time = 9.5;
        o.last_fire_frame = 285;
        o.fire_intent_count = 3;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.last_fire_victim_host, victim.0);
    assert_eq!(e.last_fire_slot, 1);
    assert!((e.last_fire_damage - 42.0).abs() < 1e-5);
    assert!((e.last_fire_range - 150.0).abs() < 1e-5);
    assert!((e.last_fire_sim_time - 9.5).abs() < 1e-5);
    assert_eq!(e.last_fire_frame, 285);
    assert_eq!(e.fire_intent_count, 3);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.last_fire_victim_host, victim.0);
    assert_eq!(o.fire_intent_count, 3);
    assert!((o.last_fire_damage - 42.0).abs() < 1e-5);
    assert_eq!(o.last_fire_slot, 1);
}

#[test]
fn projectile_flight_channel_via_set_projectile_flight() {
    let mut logic = GameLogic::new();
    let weapon = crate::game_logic::Weapon {
        damage: 25.0,
        range: 500.0,
        ..Default::default()
    };
    let id = logic.combat_system.fire_projectile(
        glam::Vec3::new(10.0, 1.0, 20.0),
        glam::Vec3::new(100.0, 1.0, 20.0),
        &weapon,
        ObjectId(7),
        Some(ObjectId(8)),
        200.0,
    );
    let projectile = logic.combat_system.projectile_mut(id).unwrap();
    projectile.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    projectile.lifetime = 0.5;
    projectile.max_lifetime = 3.0;
    projectile.is_homing = true;
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let p = shadow
        .world()
        .projectile(id.0)
        .expect("actual owner's flight projection");
    assert_eq!(p.host_id, id.0);
    assert_eq!(p.position, [10.0, 1.0, 20.0]);
    assert_eq!(p.velocity, [5.0, 0.0, 0.0]);
    assert_eq!(p.target_position, [100.0, 1.0, 20.0]);
    assert_eq!(p.damage, 25.0);
    assert_eq!(p.shooter_host, 7);
    assert_eq!(p.target_host, 8);
    assert_eq!(p.speed, 200.0);
    assert!(p.is_homing && p.active);
    assert!(logic.combat_system.remove_projectile(id));
    shadow.sync_from_host(&logic);
    assert!(shadow.world().projectile(id.0).is_none());
}

#[test]
fn parsed_missile_kill_self_snapshot_holds_shadow_pose_and_retires() {
    use crate::game_logic::combat::{DamageType, Projectile};
    use crate::game_logic::host_projectile_log;
    use crate::game_logic::weapon_bootstrap::HostProjectileLifecycle;
    use gamelogic::world::ProjectileFlightState;

    host_projectile_log::clear();
    let mut logic = GameLogic::new();
    let id = logic.combat_system.fire_projectile(
        glam::Vec3::new(10.0, 1.0, 20.0),
        glam::Vec3::new(100.0, 1.0, 20.0),
        &crate::game_logic::Weapon {
            damage: 25.0,
            ..Default::default()
        },
        ObjectId(7),
        Some(ObjectId(8)),
        200.0,
    );
    let mut projectile = Projectile::new(
        id,
        glam::Vec3::new(10.0, 1.0, 20.0),
        glam::Vec3::new(100.0, 1.0, 20.0),
        25.0,
        DamageType::Explosive,
        ObjectId(7),
        Some(ObjectId(8)),
    );
    projectile.speed = 200.0;
    projectile.velocity = glam::Vec3::new(200.0, 0.0, 0.0);
    projectile.is_homing = true;
    projectile.lifetime = 11.0 / 30.0;
    projectile.set_projectile_lifecycle(Some(HostProjectileLifecycle::Missile {
        try_to_follow_target: true,
        fuel_lifetime_frames: 11,
        detonate_on_no_fuel: true,
        kill_self_delay_frames: 3,
    }));
    // This is set only by the parsed missile lifecycle after its real
    // detonation/target-loss transition; no fallback name or generic timeout
    // can produce the shadow hold state.
    projectile.missile_kill_self_started_frame = Some(11);

    *logic.combat_system.projectile_mut(id).unwrap() = projectile;
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.world().projectile(id.0).unwrap().flight_state,
        ProjectileFlightState::MissileKillSelfHold
    );
    logic.combat_system.update_projectiles_with_countermeasures(
        1.0 / 30.0,
        &mut logic.objects,
        None,
        12,
        &mut logic.health_events,
        &mut logic.ownership_events,
    );
    shadow.sync_from_host(&logic);
    let held = shadow.world().projectile(id.0).expect("held");
    assert_eq!(held.position, [10.0, 1.0, 20.0]);
    assert_eq!(held.velocity, [200.0, 0.0, 0.0]);
    assert_eq!(held.target_position, [100.0, 1.0, 20.0]);
    assert!(
        (held.lifetime - 12.0 / 30.0).abs() < 1e-6,
        "only the host-authoritative KILL_SELF delay should age"
    );

    // The projectile's elapsed-age clock advances once per actual fixed step.
    logic.combat_system.update_projectiles_with_countermeasures(
        1.0 / 30.0,
        &mut logic.objects,
        None,
        13,
        &mut logic.health_events,
        &mut logic.ownership_events,
    );
    assert!(logic.combat_system.get_projectiles().contains_key(&id));
    logic.combat_system.update_projectiles_with_countermeasures(
        1.0 / 30.0,
        &mut logic.objects,
        None,
        14,
        &mut logic.health_events,
        &mut logic.ownership_events,
    );
    shadow.sync_from_host(&logic);
    assert!(logic.combat_system.get_projectiles().get(&id).is_none());
    assert!(shadow.world().projectile(id.0).is_none());
}

#[test]
fn projectile_authority_steps_flight_and_writeback() {
    use crate::game_logic::host_projectile_log;
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();

    host_projectile_log::clear();
    // Seed one ballistic projectile on host combat system.
    {
        use crate::game_logic::Weapon;
        use crate::game_logic::combat::DamageType;
        let mut w = Weapon {
            damage: 10.0,
            range: 500.0,
            ..Weapon::default()
        };
        w.projectile_speed = 100.0;
        let id = logic.combat_system.fire_projectile(
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(100.0, 0.0, 0.0),
            &w,
            ObjectId(1),
            None,
            100.0,
        );
        assert_eq!(
            id.0,
            logic
                .combat_system
                .get_projectiles()
                .keys()
                .next()
                .unwrap()
                .0
        );
    }
    host_projectile_log::record_snapshot(logic.combat_system.projectiles_snapshot());
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let before = shadow
        .world()
        .projectiles()
        .values()
        .next()
        .unwrap()
        .position[0];
    let stepped = logic.combat_system.integrate_projectiles_only(1.0 / 30.0);
    shadow.sync_from_host(&logic);
    assert!(stepped >= 1);
    let after = shadow
        .world()
        .projectiles()
        .values()
        .next()
        .unwrap()
        .position[0];
    assert!(
        after > before,
        "projectile should advance along +X (before={before} after={after})"
    );
    shadow.sync_from_host(&logic);

    let p = logic
        .combat_system
        .get_projectiles()
        .values()
        .next()
        .unwrap();
    assert!((p.position.x - after).abs() < 1e-4);
}
