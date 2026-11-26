//! Combat state, clocks, and ordered receipts.

use super::*;

#[test]
fn disable_deadlines_expire_independently_on_the_receiving_object() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DtCh");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object RangerDt\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RangerDt");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerDt".into(), t);
    }
    let oid = logic
        .create_object("RangerDt", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");

    {
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        o.apply_disabled_emp(
            500,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.apply_disabled_hacked(
            600,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.apply_disabled_paralyzed(
            700,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let o = logic.host_object(oid).expect("canonical owner");
    assert_eq!(
        (
            o.status.disabled_emp_until_frame,
            o.status.disabled_hacked_until_frame,
            o.status.disabled_paralyzed_until_frame
        ),
        (500, 600, 700)
    );
    let disabled = publish(&mut logic);
    assert!(actor(&disabled, oid).disabled_emp);
    assert!(actor(&disabled, oid).disabled_hacked);
    assert!(actor(&disabled, oid).disabled_paralyzed);

    let (o, receipts) = logic.host_object_and_health_events_mut(oid).unwrap();
    o.tick_disabled_emp(499, receipts);
    assert!(o.status.disabled_emp);
    o.tick_disabled_emp(500, receipts);
    assert!(!o.status.disabled_emp);
    assert_eq!(o.status.disabled_hacked_until_frame, 600);
    assert_eq!(o.status.disabled_paralyzed_until_frame, 700);
    let current = publish(&mut logic);
    assert!(!actor(&current, oid).disabled_emp);
    assert!(actor(&current, oid).disabled_hacked && actor(&current, oid).disabled_paralyzed);
    assert!(actor(&disabled, oid).disabled_emp);
}

#[test]
fn attack_and_stop_preserve_target_receipt_contract() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog");
    // C++ AIUpdate attack needs an authored WeaponSet weapon (WeaponSet.cpp).
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("AtkA")
                .kind_of("INFANTRY SELECTABLE")
                .health(100.0)
                .armed("AtkGun"),
            crate::fixture_content::FixtureObject::new("AtkB")
                .kind_of("SELECTABLE")
                .health(100.0),
        ],
        &[crate::fixture_content::FixtureWeapon::new("AtkGun")
            .damage(10.0)
            .range(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let a = logic
        .create_object("AtkA", Team::USA, glam::Vec3::ZERO)
        .expect("a");
    let b = logic
        .create_object("AtkB", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("b");
    {
        let (o, attack_events) = logic.host_object_and_attack_events_mut(a).unwrap();
        // Ensure can_attack path: weapon or kind
        o.attack_target(
            b,
            0,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    logic.attack_events.complete_boundary();
    let events = logic.attack_events.take_completed();
    assert!(
        events.iter().any(|e| e.attacker == a && e.target == b),
        "attack_target must log host_attack event: {events:?}"
    );
    {
        let o = logic.host_object_mut(a).unwrap();
        o.stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    }
    logic.attack_events.complete_boundary();
    let clears = logic.attack_events.take_completed();
    assert!(
        clears.is_empty()
            && logic.host_object(a).unwrap().target.is_none()
            && !logic.host_object(a).unwrap().status.attacking,
        "stop_attack must clear the canonical target without a targeting receipt: {clears:?}"
    );
}

#[test]
fn cooldown_values_are_frozen_before_owner_readiness_changes() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpCdCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.special_power_cooldown = 45.0;
        logic.templates.insert("SpU".into(), t);
    }
    let oid = logic
        .create_object("SpU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 45.0;
        o.special_power_cooldown_remaining = 18.0;
        o.special_power_ready = false;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(!row.special_power_ready);
    assert_eq!(
        (
            row.special_power_cooldown_remaining,
            row.special_power_cooldown
        ),
        (18.0, 45.0)
    );
    let object = logic.host_object_mut(oid).unwrap();
    object.special_power_cooldown_remaining = 0.0;
    object.set_special_power_ready(true);
    let current = publish(&mut logic);
    assert!(actor(&current, oid).special_power_ready);
    assert_eq!(actor(&current, oid).special_power_cooldown_remaining, 0.0);
    assert_eq!(actor(&frame, oid).special_power_cooldown_remaining, 18.0);
}

#[test]
fn timer_update_publishes_the_remaining_special_power_cooldown() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpTick");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpUnit\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SpUnit".into(), t);
    }
    let oid = logic
        .create_object("SpUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 5.0;
        o.set_special_power_ready(false);
        let became = o.tick_timers(
            1.0,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        let _ = became;
    }
    let owner = logic.host_object(oid).unwrap();
    assert!((owner.special_power_cooldown_remaining - 4.0).abs() < 1e-3);
    assert!(!owner.special_power_ready);
    let frame = publish(&mut logic);
    assert_eq!(actor(&frame, oid).special_power_cooldown_remaining, 4.0);
    assert!(!actor(&frame, oid).special_power_ready);
}

#[test]
fn ongoing_special_power_cooldown_keeps_earlier_frames_immutable() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpWb");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpWbU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpWbU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SpWbU".into(), t);
    }
    let oid = logic
        .create_object("SpWbU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 2.0;
        o.set_special_power_ready(false);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!(row.special_power_cooldown_remaining, 2.0);
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .special_power_cooldown_remaining,
        2.0
    );
    logic.host_object_mut(oid).unwrap().tick_timers(
        1.0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(
        actor(&publish(&mut logic), oid).special_power_cooldown_remaining,
        1.0
    );
    assert_eq!(actor(&frame, oid).special_power_cooldown_remaining, 2.0);
}

#[test]
fn owned_damage_carries_its_source_and_publishes_its_health() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DmgSrc");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object SrcA\nEnd\nObject SrcB\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["SrcA", "SrcB"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            t.set_health(100.0);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("SrcA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let victim = logic
        .create_object("SrcB", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("v");
    let kind = crate::game_logic::combat::DamageType::Unresistable;
    let context =
        crate::game_logic::object::DamageHitContext::new(logic.host_object(attacker), None, kind);
    logic
        .apply_owned_damage(
            victim,
            25.0,
            Some(attacker),
            kind,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
    assert_eq!(
        logic.host_object(victim).unwrap().last_damage_source,
        Some(attacker)
    );
    // The actual receiving nonlethal owner write retains its source.
    let events = logic.health_events.drain_damage();
    assert!(
        events
            .iter()
            .any(|e| { e.target == victim && e.source == Some(attacker) && e.amount >= 20.0 }),
        "damage log must carry source; got {events:?}"
    );
    assert_eq!(logic.host_object(victim).unwrap().health.current, 75.0);
    assert_eq!(actor(&publish(&mut logic), victim).health_current, 75.0);
}

#[test]
fn overcharge_status_publishes_the_receiving_toggle() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OcCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object OCU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("OCU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("OCU".into(), t);
    }
    let oid = logic
        .create_object("OCU", Team::China, glam::Vec3::new(17.0, 0.0, 17.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_overcharge_enabled(true);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.overcharge_enabled);
    assert!(logic.host_object(oid).unwrap().overcharge_enabled);
    logic.host_object_mut(oid).unwrap().overcharge_enabled = false;
    assert!(!actor(&publish(&mut logic), oid).overcharge_enabled);
    assert!(actor(&frame, oid).overcharge_enabled);
}

#[test]
fn detector_publication_uses_owned_range_and_scan_interval() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DetCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object DetU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DetU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DetU".into(), t);
    }
    let oid = logic
        .create_object("DetU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_detector_state(true, 175.0, 12);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.is_detector);
    assert_eq!(
        (row.detection_range, row.detection_rate_frames),
        (175.0, 12)
    );
    logic
        .host_object_mut(oid)
        .unwrap()
        .set_detector_state(false, 0.0, 0);
    assert!(!actor(&publish(&mut logic), oid).is_detector);
    assert_eq!(actor(&frame, oid).detection_range, 175.0);
}

#[test]
fn turret_pose_keeps_the_angles_published_before_later_writes() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TurretCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object TurU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("TurU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("TurU".into(), t);
    }
    let oid = logic
        .create_object("TurU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_ai.turret_angle_deg = 33.0;
        o.turret_ai.turret_pitch_deg = 12.0;
        o.turret_ai.turret_holding = true;
        o.turret_ai.turret_idle_scanning = false;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!((row.turret_angle_deg, row.turret_pitch_deg), (33.0, 12.0));
    assert!(row.turret_holding && !row.turret_idle_scanning);
    let turret = &mut logic.host_object_mut(oid).unwrap().turret_ai;
    turret.turret_angle_deg = 18.0;
    assert_eq!(actor(&publish(&mut logic), oid).turret_angle_deg, 18.0);
    assert_eq!(actor(&frame, oid).turret_angle_deg, 33.0);
}

#[test]
fn repulsor_activity_publishes_while_the_object_owns_its_deadline() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RpCh");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object RangerRp\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RangerRp");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerRp".into(), t);
    }
    let oid = logic
        .create_object("RangerRp", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let object = logic.host_object_mut(oid).unwrap();
        object.set_status_repulsor(true);
        object.arm_repulsor_countdown(60);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.repulsor);
    assert_eq!(logic.host_object(oid).unwrap().repulsor_until_frame, 60);
    logic
        .host_object_mut(oid)
        .unwrap()
        .arm_repulsor_countdown(12);
    assert_eq!(logic.host_object(oid).unwrap().repulsor_until_frame, 12);
    logic
        .host_object_mut(oid)
        .unwrap()
        .set_status_repulsor(false);
    assert!(!actor(&publish(&mut logic), oid).repulsor);
    assert!(actor(&frame, oid).repulsor);
}

#[test]
fn healing_icon_uses_the_receiving_benefactor_deadline() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SoleHeal");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object HealTgt\nEnd\nObject DozerA\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["HealTgt", "DozerA"] {
        {
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
    let frame = publish(&mut logic);
    let row = actor(&frame, tgt);
    assert!(row.show_healing);
    let o = logic.host_object(tgt).unwrap();

    assert_eq!(o.sole_healing_benefactor, Some(dozer));
    assert_eq!(o.sole_healing_benefactor_expiration_frame, 900);
    logic
        .host_object_mut(tgt)
        .unwrap()
        .sole_healing_benefactor_expiration_frame = 0;
    assert!(!actor(&publish(&mut logic), tgt).show_healing);
    assert!(actor(&frame, tgt).show_healing);
}

#[test]
fn turret_publication_keeps_target_and_slew_state_on_the_owner() {
    use crate::game_logic::object::TurretSubState;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TurretX");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object TurU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
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
        o.turret_ai.turret_angle_deg = 45.0;
        o.turret_ai.turret_pitch_deg = 10.0;
        o.turret_ai.turret_holding = true;
        o.turret_ai.turret_idle_scanning = false;
        o.turret_ai.turret_turn_rate_rad = 0.05;
        o.turret_ai.turret_recenter_frames = 60;
        o.turret_ai.turret_hold_until_frame = 200;
        o.turret_ai.turret_idle_recentering = true;
        o.turret_ai.turret_enabled = true;
        o.turret_ai.turret_rotating = true;
        o.turret_ai.turret_natural_angle_deg = 0.0;
        o.turret_ai.turret_natural_pitch_deg = 5.0;
        o.turret_ai.turret_target_id = Some(tgt);
        o.turret_ai.turret_force_attacking = true;
        o.turret_ai.turret_mood_target = false;
        o.turret_ai.turret_idle_scan_next_frame = 30;
        o.turret_ai.turret_idle_scan_desired_angle_deg = 90.0;
        o.turret_ai.turret_idle_scan_index = 2;
        o.turret_ai.turret_substate = TurretSubState::Aim;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!((row.turret_angle_deg, row.turret_pitch_deg), (45.0, 10.0));
    let o = logic.host_object(oid).unwrap();

    let turret = &o.turret_ai;
    assert_eq!(turret.turret_turn_rate_rad, 0.05);
    assert_eq!(turret.turret_recenter_frames, 60);
    assert!(turret.turret_enabled && turret.turret_rotating);
    assert_eq!(turret.turret_target_id, Some(tgt));
    assert_eq!(turret.turret_substate, TurretSubState::Aim);
}

#[test]
fn publication_preserves_stealth_and_detection_deadlines() {
    let mut logic = GameLogic::new();
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("StlU")
            .kind_of("INFANTRY")
            .health(100.0)],
    );
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
        o.detection_expires_frame = 450;
        o.camo_opacity_pulse_phase = 1.25;
        o.queue_second_material_pass(1.0);
    }
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(
        (o.stealth_allowed_frame, o.detection_expires_frame),
        (300, 450)
    );
    assert_eq!(o.camo_opacity_pulse_phase, 1.25);
}

#[test]
fn publication_preserves_attack_preparation_shot_limits_and_aim_state() {
    use crate::game_logic::AttackSubState;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CbtAtk");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object CbtU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
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
    let temporary_deadline = logic.get_frame().wrapping_add(6);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.pre_attack_target = Some(tgt);
        o.pre_attack_ready_at = 12.5;
        o.consecutive_shots_at_target = 3;
        o.max_shots_to_fire = 5;
        o.set_attack_substate(AttackSubState::FireWeapon);
        o.set_approach_timestamp(90);
        o.consecutive_shot_target = Some(ObjectId(tgt.0));
        o.maintain_pos_valid = true;
        o.maintain_pos = Some(glam::Vec3::new(1.0, 2.0, 3.0));
        o.unit_ai_runtime
            .begin_temporary_move(crate::game_logic::object::TemporaryAiMove {
                deadline: temporary_deadline,
                goal: o.get_position(),
                saved_destination: o.requested_destination,
                saved_path_goal: o.path_goal_position,
            });
        o.group_speed_factor = 0.85;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.continuous_fire_consecutive, 3);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.pre_attack_target, Some(tgt));
    assert_eq!(o.pre_attack_ready_at, 12.5);
    assert_eq!(o.consecutive_shots_at_target, 3);
    assert_eq!(o.max_shots_to_fire, 5);
    assert_eq!(o.attack_substate(), AttackSubState::FireWeapon);
    assert_eq!(o.approach_timestamp(), 90);
    assert_eq!(o.consecutive_shot_target, Some(tgt));
    assert_eq!(o.maintain_pos, Some(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(o.temporary_ai_move_remaining(logic.get_frame()), 7);
    assert_eq!(o.group_speed_factor, 0.85);
}

#[test]
fn publication_preserves_per_slot_leech_range_state() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Leech");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object LchU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
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
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert!(o.leech_range_active_primary && o.leech_range_active_secondary);
}
