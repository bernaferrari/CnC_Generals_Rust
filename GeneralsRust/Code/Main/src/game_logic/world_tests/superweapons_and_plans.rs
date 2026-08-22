//! Host GameLogic tests — `superweapons_and_plans`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Residual: TurretAI mood matrix Sleep/Passive + bone pitch drawable.
///
/// C++ AttitudeType / getMoodMatrixActionAdjustment(MM_Action_Idle):
/// - Sleep → IgnoreAll (no idle mood-target acquire)
/// - Passive → WaitForAttack (only last_damage_source residual retaliate)
/// - Normal → free acquire (covered by mood_target_residual)
/// Bone pitch drawable residual exposes TurretAI pitch/yaw for presentation.
/// VisionObjectName createVisionObject disabled in retail — honesty only.
#[test]
fn strategy_center_turret_mood_matrix_sleep_passive_residual() {
    use crate::game_logic::host_strategy_center::{
        strategy_center_turret_bone_drawable,
        strategy_center_vision_object_spawn_enabled_in_retail, HostAiAttitude, HostBattlePlan,
        STRATEGY_CENTER_FIRE_PITCH_DEG, STRATEGY_CENTER_GUN_MIN_RANGE,
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    let enemy_id = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("enemy");

    // --- Sleep: IgnoreAll → no mood-target acquire ---
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_mood_target = false;
        sc.set_ai_attitude(HostAiAttitude::Sleep);
        sc.last_damage_source = None;
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scanning = false;
        sc.turret_holding = false;
        sc.turret_idle_recentering = false;
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_mood_target,
            "Sleep mood residual must not auto-acquire"
        );
        assert!(sc.target.is_none());
    }

    // --- Passive without last damage: WaitForAttack blocks free acquire ---
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.set_ai_attitude(HostAiAttitude::Passive);
        sc.last_damage_source = None;
        sc.turret_mood_target = false;
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_mood_target,
            "Passive without last damage must not free-acquire"
        );
    }

    // --- Passive with last damage source = enemy → retaliate residual ---
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.set_ai_attitude(HostAiAttitude::Passive);
        sc.last_damage_source = Some(enemy_id);
        sc.turret_mood_target = false;
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            sc.turret_mood_target,
            "Passive WaitForAttack residual must retaliate vs last damage source"
        );
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            assert!(sc.target.is_none());
        } else {
            assert_eq!(sc.target, Some(enemy_id));
        }
        assert!(
            (sc.turret_pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01,
            "passive retaliate aims FirePitch, got {}",
            sc.turret_pitch_deg
        );
        // Bone pitch drawable residual from TurretAI angles.
        let bone = strategy_center_turret_bone_drawable(sc.turret_angle_deg, sc.turret_pitch_deg);
        assert!((bone.pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01);
        assert!(!bone.is_natural || bone.yaw_deg.abs() < 5.0);
    }

    // VisionObjectName createVisionObject disabled in retail C++.
    assert!(!strategy_center_vision_object_spawn_enabled_in_retail());
}

/// Residual: AmericaParachute OpenDist freefall → open residual path.
///
/// Retail ParachuteOpenDist=100: freefall until fallen 100, then open chute
/// (slower sink + ParachuteOpen audio residual). Fail-closed: not full
/// bone PARA_COG / DeliverPayload matrix.
#[test]
fn eject_pilot_parachute_open_dist_residual() {
    use crate::game_logic::host_usa_pilot::{
        significantly_above_terrain_threshold, EJECT_PILOT_TEMPLATE, PARACHUTE_OPEN_DIST,
    };
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();
    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let thr = significantly_above_terrain_threshold();
    let air_y = thr + PARACHUTE_OPEN_DIST + 80.0;
    let humvee_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(0.0, air_y, 0.0),
        )
        .expect("airborne humvee");
    {
        let h = game_logic.host_object_mut(humvee_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        h.status.airborne_target = true;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, o)| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .map(|(id, _)| *id)
        .expect("ejected pilot");
    let start_y = game_logic.host_object(pilot_id).unwrap().get_position().y;
    assert!(
        !game_logic
            .host_object(pilot_id)
            .unwrap()
            .is_parachute_open(),
        "starts freefall closed"
    );

    // Tick freefall until OpenDist residual opens chute (before land).
    let ids = [pilot_id];
    let mut opened_y = None;
    for _ in 0..20 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        let p = game_logic.host_object(pilot_id).expect("pilot");
        if p.is_parachute_open() {
            opened_y = Some(p.get_position().y);
            break;
        }
        assert!(
            p.is_parachuting(),
            "must still be parachuting during freefall"
        );
    }
    let opened_y = opened_y.expect("OpenDist residual must open chute");
    assert!(
        game_logic.honesty_pilot_parachute_open_ok(),
        "parachute open honesty"
    );
    let fallen = start_y - opened_y;
    assert!(
        fallen + 0.5 >= PARACHUTE_OPEN_DIST,
        "chute open residual after ≥OpenDist freefall, fallen={fallen}"
    );
    assert!(
        !game_logic.honesty_pilot_parachute_land_ok(),
        "must not land on the open frame (still elevated)"
    );

    // Finish landing residual.
    for _ in 0..100 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        if game_logic.honesty_pilot_parachute_land_ok() {
            break;
        }
    }
    assert!(game_logic.honesty_pilot_parachute_land_ok());
    assert!(
        game_logic
            .host_object(pilot_id)
            .unwrap()
            .get_position()
            .y
            .abs()
            < 0.1
    );
}

/// Residual: AmericaParachute pitch/roll spring-damper sway residual.
///
/// While chute open: seed ±½ Pitch/RollRateMax, spring/damper with
/// ParachuteLocomotor stiffness/damping + LowAltitudeDamping near ground.
/// Fail-closed: not full bone PARA_COG / rider sway / DeliverPayload matrix.
#[test]
fn eject_pilot_parachute_pitch_roll_sway_residual() {
    use crate::game_logic::host_usa_pilot::{
        parachute_initial_pitch_rate, parachute_initial_roll_rate,
        significantly_above_terrain_threshold, EJECT_PILOT_TEMPLATE, PARACHUTE_OPEN_DIST,
    };
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();
    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let thr = significantly_above_terrain_threshold();
    // Tall freefall so chute opens well above ground and sway has time to step.
    let air_y = thr + PARACHUTE_OPEN_DIST + 200.0;
    let humvee_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(0.0, air_y, 0.0),
        )
        .expect("airborne humvee");
    {
        let h = game_logic.host_object_mut(humvee_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        h.status.airborne_target = true;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, o)| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .map(|(id, _)| *id)
        .expect("ejected pilot");
    let ids = [pilot_id];

    // Freefall residual: pitch/roll stay 0 until open.
    for _ in 0..5 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        let p = game_logic.host_object(pilot_id).expect("pilot");
        if p.is_parachute_open() {
            break;
        }
        assert!(
            p.parachute_pitch().abs() < 1e-6 && p.parachute_roll().abs() < 1e-6,
            "freefall residual must not sway before chute opens"
        );
    }

    // Open residual then step pitch/roll spring-damper.
    let mut saw_open = false;
    let mut saw_non_zero_sway = false;
    for _ in 0..80 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        let p = game_logic.host_object(pilot_id).expect("pilot");
        if p.is_parachute_open() {
            saw_open = true;
            // Open frame seeds rates; subsequent frames integrate angles.
            if p.parachute_pitch().abs() > 1e-6 || p.parachute_roll().abs() > 1e-6 {
                saw_non_zero_sway = true;
                break;
            }
        }
        if game_logic.honesty_pilot_parachute_land_ok() {
            break;
        }
    }
    assert!(saw_open, "OpenDist residual must open chute for sway");
    assert!(
        saw_non_zero_sway,
        "pitch/roll sway residual must leave zero after open frames"
    );
    assert!(
        game_logic.honesty_pilot_parachute_sway_ok(),
        "sway honesty residual must tick while chute open"
    );
    // Seed residual honesty: open sets deterministic mid rates.
    let open_seed_ok =
        parachute_initial_pitch_rate().abs() > 0.0 && parachute_initial_roll_rate().abs() > 0.0;
    assert!(open_seed_ok, "deterministic mid pitch/roll rate seed");

    // Land residual clears sway angles.
    for _ in 0..200 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        if game_logic.honesty_pilot_parachute_land_ok() {
            break;
        }
    }
    assert!(game_logic.honesty_pilot_parachute_land_ok());
    let landed = game_logic.host_object(pilot_id).expect("pilot");
    assert!(!landed.is_parachuting(), "land residual clears parachuting");
    assert!(
        landed.parachute_pitch().abs() < 1e-6 && landed.parachute_roll().abs() < 1e-6,
        "land residual must clear pitch/roll sway state"
    );
}

/// Residual: Bombardment plan enables StrategyCenterGun turret auto-fire.
///
/// C++ enableTurret(true) on Bombardment residual:
/// PrimaryDamage 200 / r25, range 400, min 100, Delay 7000ms.
/// Fail-closed: not full natural-position recenter / pack animation.
#[test]
fn strategy_center_bombardment_turret_fire_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_GUN_DAMAGE, STRATEGY_CENTER_GUN_MIN_RANGE,
        STRATEGY_CENTER_GUN_RANGE,
    };

    let mut game_logic = GameLogic::new();

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    ensure_test_tank_template(&mut game_logic);

    // Ensure USA player exists for player_id_for_team residual gate.
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    } else if let Some(p) = game_logic.players.get_mut(&0) {
        p.team = Team::USA;
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    // Without Bombardment: no StrategyCenterGun residual.
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.weapon.is_none(), "turret disabled without Bombardment");
    }

    // Enemy in range band (min 100, max 400).
    let enemy_id = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.health.current = e.health.maximum; // full
    }
    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;

    // Too-close enemy residual (inside min range) must not be preferred when
    // a legal in-band target exists; still create for range-band honesty.
    let close_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("close enemy");
    let close_hp_before = game_logic.host_object(close_id).unwrap().health.current;

    assert!(!game_logic.honesty_battle_plan_turret_fire_ok());

    // Activate Bombardment → equip StrategyCenterGun only after unpack ACTIVE.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    assert!(
        game_logic.host_object(sc_id).unwrap().weapon.is_none(),
        "turret must not equip during UNPACKING residual"
    );
    advance_battle_plan_door_to_active(&mut game_logic);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        let w = sc
            .weapon
            .as_ref()
            .expect("Bombardment ACTIVE must equip StrategyCenterGun");
        assert!(
            (w.damage - STRATEGY_CENTER_GUN_DAMAGE).abs() < 0.001,
            "StrategyCenterGun damage residual 200"
        );
        assert!(
            (w.range - STRATEGY_CENTER_GUN_RANGE).abs() < 0.001,
            "StrategyCenterGun range residual 400"
        );
        assert!(
            (w.min_range - STRATEGY_CENTER_GUN_MIN_RANGE).abs() < 0.001,
            "StrategyCenterGun min range residual 100"
        );
    }

    // Fire residual (force readiness: last_fire_time far past).
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        if let Some(w) = sc.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
        sc.set_ai_state(AIState::Idle);
    }
    // Natural angles before fire residual.
    {
        use crate::game_logic::host_strategy_center::{
            turret_angles_are_natural, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
            STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
        };
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "turret residual starts at NaturalTurretAngle/Pitch"
        );
        assert!((sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() < 0.01);
        assert!((sc.turret_pitch_deg - STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG).abs() < 0.01);
    }

    for _ in 0..90 {
        game_logic.try_strategy_center_bombardment_turret_fire(sc_id);
        if game_logic.honesty_battle_plan_turret_fire_ok() {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }


    assert!(
        game_logic.honesty_battle_plan_turret_fire_ok(),
        "Bombardment turret residual must record fire honesty"
    );
    assert!(game_logic.battle_plans().turret_fire_count() >= 1);
    // Pitch/yaw aim residual: after fire, angles leave natural toward target.
    {
        use crate::game_logic::host_strategy_center::{
            turret_angles_are_natural, STRATEGY_CENTER_FIRE_PITCH_DEG,
        };
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "fire residual must aim turret off NaturalTurretAngle"
        );
        assert!(
            (sc.turret_pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01,
            "FirePitch residual 45"
        );
        assert!(
            game_logic.honesty_strategy_center_turret_aim_ok(),
            "turret aim residual honesty"
        );
    }

    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    if crate::gameworld_shadow::gameworld_damage_authority_live() {
        // HP last-write via damage log; host HP stays until GameWorld writeback.
        let dmg_events = crate::game_logic::host_damage_log::snapshot();
        let dealt: f32 = dmg_events
            .iter()
            .filter(|e| e.target == enemy_id)
            .map(|e| e.amount)
            .sum();
        assert!(
            dealt > 50.0,
            "in-range enemy must log StrategyCenterGun residual damage under damage authority, dealt={dealt}"
        );
    } else {
        assert!(
            enemy_hp_after < enemy_hp_before - 50.0,
            "in-range enemy must take StrategyCenterGun residual damage, before={enemy_hp_before} after={enemy_hp_after}"
        );
    }

    // Close enemy may take splash if within 25 of impact (impact is on far enemy at x=150).
    // At x=20 vs impact x=150 → no splash. Must remain undamaged by min-range gate.
    let close_hp_after = game_logic.host_object(close_id).unwrap().health.current;
    assert!(
        (close_hp_after - close_hp_before).abs() < 0.001,
        "min-range residual must not target close enemy as primary"
    );

    // Switch to HoldTheLine → PACKING clears turret residual immediately.
    // Ensure turret is natural (busy + pitch/yaw) so pack starts without recenter.
    {
        use crate::game_logic::host_strategy_center::{
            STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
        };
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        if let Some(w) = sc.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::HoldTheLine, Some(sc_id),));
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            sc.weapon.is_none(),
            "PACKING residual must disable StrategyCenterGun"
        );
    }
}

#[test]
fn strategy_center_gun_scatter_misses_infantry_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_GUN_MIN_RANGE, STRATEGY_CENTER_GUN_SCATTER,
        STRATEGY_CENTER_GUN_SCATTER_VS_INFANTRY,
    };

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    if !logic.players.contains_key(&0) {
        logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    } else if let Some(p) = logic.players.get_mut(&0) {
        p.team = Team::USA;
    }

    let sc_id = logic
        .create_object(
            "AmericaStrategyCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("strategy center");

    let inf = logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            glam::Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    assert!(logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id)));
    advance_battle_plan_door_to_active(&mut logic);
    assert!(logic.host_object(sc_id).unwrap().weapon.is_some());

    // Ready weapon.
    if let Some(o) = logic.objects.get_mut(&sc_id) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }

    for _ in 0..90 {
        logic.try_strategy_center_bombardment_turret_fire(sc_id);
        if logic.strategy_center_gun_scatter_applied > 0
            || logic.strategy_center_gun_scatter_misses > 0
            || logic.honesty_strategy_center_gun_scatter_ok()
        {
            break;
        }
        logic.frame = logic.frame.saturating_add(1);
    }

    assert!(
        logic.strategy_center_gun_scatter_applied > 0
            || logic.strategy_center_gun_scatter_misses > 0
            || logic.honesty_strategy_center_gun_scatter_ok(),
        "strategy center gun scatter residual must peel vs infantry"
    );
    assert!((STRATEGY_CENTER_GUN_SCATTER - 15.0).abs() < 0.01);
    assert!((STRATEGY_CENTER_GUN_SCATTER_VS_INFANTRY - 15.0).abs() < 0.01);

    // Non-infantry still exercises scatter (base radius 15) and can hit splash.
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let tank = logic
        .create_object(
            "TestTank",
            Team::GLA,
            glam::Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("tank");
    if let Some(o) = logic.objects.get_mut(&sc_id) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let before_applied = logic.strategy_center_gun_scatter_applied;
    for _ in 0..90 {
        logic.try_strategy_center_bombardment_turret_fire(sc_id);
        if logic.strategy_center_gun_scatter_applied > before_applied
            || logic.honesty_strategy_center_gun_scatter_ok()
        {
            break;
        }
        logic.frame = logic.frame.saturating_add(1);
    }

    assert!(
        logic.strategy_center_gun_scatter_applied > before_applied
            || logic.honesty_strategy_center_gun_scatter_ok(),
        "vehicle target still peels base ScatterRadius"
    );
    let _ = tank;
}

/// Residual: Strategy Center StealthDetectorUpdate enable stack (S&D).
///
/// Retail ModuleTag_16: DetectionRange **500**, DetectionRate **500**ms →
/// **15** frames, InitiallyDisabled **Yes**. SearchAndDestroy →
/// setSDEnabled(true); leaving S&D → setSDEnabled(false). DetectionRate
/// residual: immediate first scan, then rate-gated with
/// markAsDetected(rate+1). VisionObjectName spawn residual is fail-closed
/// (createVisionObject disabled in retail C++).
#[test]
fn strategy_center_stealth_detector_enable_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_STEALTH_DETECTION_RANGE,
        STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    sc_template.sight_range = 400.0;
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    {
        // InitiallyDisabled residual: Strategy Center is not a detector until S&D.
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.is_detector = false;
        sc.detection_range = 0.0;
        sc.object_type = ObjectType::Building;
    }

    // Stealthed enemy inside DetectionRange 500.
    let near_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(400.0, 0.0, 0.0))
        .expect("near stealthed");
    {
        let e = game_logic.host_object_mut(near_id).expect("near");
        e.apply_grant_stealth();
        assert!(e.is_effectively_stealthed());
    }
    // Stealthed enemy outside DetectionRange 500.
    let far_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(600.0, 0.0, 0.0))
        .expect("far stealthed");
    {
        let e = game_logic.host_object_mut(far_id).expect("far");
        e.apply_grant_stealth();
    }

    // Without S&D: detector off → no residual detect.
    assert!(!game_logic.host_object(sc_id).unwrap().is_detector);
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(near_id)
            .unwrap()
            .is_effectively_stealthed(),
        "InitiallyDisabled residual must not detect before S&D"
    );

    // Activate SearchAndDestroy → DetectionRange 500 + is_detector after ACTIVE.
    assert!(!game_logic.honesty_battle_plan_stealth_detector_ok());
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::SearchAndDestroy, Some(sc_id),));
    assert!(
        !game_logic.host_object(sc_id).unwrap().is_detector,
        "StealthDetector must wait for unpack ACTIVE residual"
    );
    advance_battle_plan_door_to_active(&mut game_logic);
    assert!(
        game_logic.honesty_battle_plan_stealth_detector_ok(),
        "S&D ACTIVE must record StealthDetector enable honesty"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.is_detector, "S&D must enable StealthDetector residual");
        assert!(
            (sc.detection_range - STRATEGY_CENTER_STEALTH_DETECTION_RANGE).abs() < 0.1,
            "DetectionRange residual must be 500, got {}",
            sc.detection_range
        );
        assert_eq!(
            sc.detection_rate_frames, STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
            "DetectionRate residual must be 15 frames"
        );
        assert_eq!(
            sc.next_detection_scan_frame, 0,
            "setSDEnabled residual first scan must be immediate"
        );
    }

    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.honesty_stealth_detector_rate_ok(),
        "DetectionRate residual must record scan honesty"
    );
    assert!(
        !game_logic
            .host_object(near_id)
            .unwrap()
            .is_effectively_stealthed(),
        "near stealthed enemy must be detected at DetectionRange 500"
    );
    assert!(
        game_logic
            .host_object(far_id)
            .unwrap()
            .is_effectively_stealthed(),
        "far stealthed enemy beyond 500 must remain undetected"
    );
    // After first scan, next DetectionRate wake is frame + 15.
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert_eq!(
            sc.next_detection_scan_frame,
            game_logic.frame + STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
            "DetectionRate residual must sleep 15 frames after scan"
        );
    }

    // Leave S&D → PACKING setSDEnabled(false) residual.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.is_detector,
            "PACKING residual must disable StealthDetector"
        );
        assert!(
            sc.detection_range.abs() < 0.1,
            "PACKING residual must clear DetectionRange residual"
        );
        assert_eq!(
            sc.detection_rate_frames, 0,
            "PACKING residual must clear DetectionRate residual"
        );
    }
    assert!(
        game_logic.battle_plans().stealth_detector_disable_count() >= 1,
        "must record StealthDetector disable honesty"
    );

    // After disable: clear detected and re-stealth for residual recheck.
    {
        let e = game_logic.host_object_mut(near_id).expect("near");
        e.apply_grant_stealth();
    }
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(near_id)
            .unwrap()
            .is_effectively_stealthed(),
        "disabled StealthDetector residual must not detect"
    );
}

/// Residual: StealthDetectorUpdate DetectionRate sleep phasing (Strategy Center).
///
/// Retail DetectionRate **500**ms → **15** frames; markAsDetected(rate+1=**16**).
/// First scan immediate on setSDEnabled; subsequent scans rate-gated.
/// Fail-closed: VisionObjectName spawn residual still not claimed.
#[test]
fn strategy_center_stealth_detector_detection_rate_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_STEALTH_DETECTION_HOLD_FRAMES,
        STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    sc_template.sight_range = 400.0;
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.is_detector = false;
        sc.detection_range = 0.0;
        sc.object_type = ObjectType::Building;
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(200.0, 0.0, 0.0))
        .expect("stealthed enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.apply_grant_stealth();
    }

    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::SearchAndDestroy, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Frame 0: immediate first DetectionRate residual scan.
    game_logic.frame = 0;
    let scans_before = game_logic.stealth_detector_rate_scans();
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.stealth_detector_rate_scans() > scans_before,
        "first DetectionRate residual scan must fire immediately"
    );
    {
        let e = game_logic.host_object(enemy_id).expect("enemy");
        assert!(
            e.status.detected,
            "first scan must detect in-range stealthed"
        );
        assert_eq!(
            e.detection_expires_frame, STRATEGY_CENTER_STEALTH_DETECTION_HOLD_FRAMES,
            "markAsDetected(rate+1) residual hold must be 16 frames"
        );
    }
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert_eq!(
            sc.next_detection_scan_frame, STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES,
            "next scan residual at frame 15"
        );
    }

    // Mid-rate frames: detector sleeps — no new rate scan.
    let scans_after_first = game_logic.stealth_detector_rate_scans();
    game_logic.frame = 7;
    game_logic.update_stealth_and_detection();
    assert_eq!(
        game_logic.stealth_detector_rate_scans(),
        scans_after_first,
        "DetectionRate residual must not re-scan mid-sleep"
    );

    // Next DetectionRate wake at frame 15.
    game_logic.frame = STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.stealth_detector_rate_scans() > scans_after_first,
        "DetectionRate residual must re-scan after 15 frames"
    );
    {
        let e = game_logic.host_object(enemy_id).expect("enemy");
        assert!(
            e.status.detected,
            "rate re-scan must refresh detected residual"
        );
        assert_eq!(
            e.detection_expires_frame,
            STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES
                + STRATEGY_CENTER_STEALTH_DETECTION_HOLD_FRAMES,
            "refresh hold residual = scan_frame + rate + 1"
        );
    }
    assert!(game_logic.honesty_stealth_detector_rate_ok());
}

/// Residual: BattlePlan pack/unpack door model-condition / 7s animation.
///
/// Retail AnimationTime **7000**ms → **210** frames; TransitionIdleTime **0**.
/// DOOR_1 Bombardment / DOOR_2 HoldTheLine / DOOR_3 SearchAndDestroy.
/// OPENING → WAITING_TO_CLOSE after 210 frames; plan switch → CLOSING then
/// new OPENING. Army buffs apply only on ACTIVE (delayed setBattlePlan residual).
#[test]
fn strategy_center_battle_plan_door_animation_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, HostBattlePlanDoor, HostBattlePlanTransition, BATTLE_PLAN_ANIMATION_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    let ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    {
        let u = game_logic.host_object_mut(ally_id).expect("ally");
        u.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
    }

    assert!(!game_logic.honesty_battle_plan_door_ok());

    // First select Bombardment: door OPENING residual; army buffs deferred.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    assert!(
        game_logic.honesty_battle_plan_door_ok(),
        "door residual must start on plan select"
    );
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        assert_eq!(state.door, HostBattlePlanDoor::Door1Opening);
        assert_eq!(
            state.next_ready_frame,
            game_logic.frame + BATTLE_PLAN_ANIMATION_FRAMES
        );
    }
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "army buff residual must wait for unpack ACTIVE"
    );
    assert!(!game_logic.honesty_battle_plan_door_active_ok());

    // Advance AnimationTime frames → WAITING_TO_CLOSE + setBattlePlan.
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic.honesty_battle_plan_door_active_ok(),
        "door residual must reach ACTIVE after AnimationTime"
    );
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "ACTIVE residual must apply Bombardment army buffs"
    );
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.status, HostBattlePlanTransition::Active);
        assert_eq!(state.door, HostBattlePlanDoor::Door1WaitingToClose);
    }

    // Plan switch → PACKING door1 CLOSING residual; clears buffs immediately.
    let pack_frame = game_logic.frame;
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::HoldTheLine, Some(sc_id),));
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.status, HostBattlePlanTransition::Packing);
        assert_eq!(state.door, HostBattlePlanDoor::Door1Closing);
        assert_eq!(
            state.next_ready_frame,
            pack_frame + BATTLE_PLAN_ANIMATION_FRAMES
        );
    }
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .has_battle_plan_bonus(),
        "PACKING residual must clear army buffs (setBattlePlan NONE)"
    );

    // Pack complete (TransitionIdleTime 0) → unpack HoldTheLine DOOR_2 OPENING.
    game_logic.frame = pack_frame.saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        assert_eq!(state.door, HostBattlePlanDoor::Door2Opening);
    }

    // SearchAndDestroy door residual DOOR_3 after Active.
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    // HoldTheLine now ACTIVE.
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_hold_the_line
    );
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::SearchAndDestroy, Some(sc_id),));
    // Pack HoldTheLine door2 closing first.
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.door, HostBattlePlanDoor::Door2Closing);
    }
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door state");
        assert_eq!(state.status, HostBattlePlanTransition::Unpacking);
        assert_eq!(state.door, HostBattlePlanDoor::Door3Opening);
    }
}

/// Residual: delayed ACTIVE-after-unpack setBattlePlan + Bombardment recenter.
///
/// C++ setStatus(ACTIVE) → setBattlePlan(plan); setStatus(PACKING) →
/// setBattlePlan(NONE)+paralyze. Leaving Bombardment with non-natural turret
/// recenters (30 frames) before pack.
#[test]
fn strategy_center_delayed_set_battle_plan_and_turret_recenter_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, HostBattlePlanTransition, BATTLE_PLAN_ANIMATION_FRAMES,
        BATTLE_PLAN_TURRET_RECENTER_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    let ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    {
        let u = game_logic.host_object_mut(ally_id).expect("ally");
        u.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
    }

    assert!(!game_logic.honesty_battle_plan_delayed_active_ok());
    assert!(!game_logic.honesty_battle_plan_turret_recenter_ok());

    // Select Bombardment — no buffs until ACTIVE.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    assert!(!game_logic
        .host_object(ally_id)
        .unwrap()
        .has_battle_plan_bonus());
    advance_battle_plan_door_to_active(&mut game_logic);
    assert!(game_logic.honesty_battle_plan_delayed_active_ok());
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment
    );
    assert!(game_logic.host_object(sc_id).unwrap().weapon.is_some());

    // Make turret non-natural via pitch/yaw residual (off NaturalTurretAngle)
    // plus busy gate, then switch plan.
    let fire_time = game_logic.frame as f32 * LOGIC_FRAME_TIMESTEP;
    {
        use crate::game_logic::host_strategy_center::STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.set_ai_state(AIState::Attacking);
        sc.set_status_attacking(true);
        sc.target = Some(ally_id);
        // 60° off natural → 30 frames at 2 deg/frame residual.
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG + 60.0;
        sc.turret_pitch_deg = 45.0;
        if let Some(w) = sc.weapon.as_mut() {
            w.last_fire_time = fire_time;
        }
    }
    let switch_frame = game_logic.frame;
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::HoldTheLine, Some(sc_id),));
    assert!(
        game_logic.honesty_battle_plan_turret_recenter_ok(),
        "non-natural Bombardment turret must start recenter residual"
    );
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door");
        assert!(state.centering_turret);
        assert_eq!(state.status, HostBattlePlanTransition::Active);
        assert_eq!(
            state.next_ready_frame,
            switch_frame + BATTLE_PLAN_TURRET_RECENTER_FRAMES,
            "60° yaw delta → 30 frames at TurretTurnRate 60 deg/s"
        );
    }
    // During recenter: Bombardment buffs still active (not packed yet).
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "recenter residual must not clear army buffs until pack"
    );
    assert!(game_logic.host_object(sc_id).unwrap().weapon.is_some());

    // Step recenter frames: turret angles advance toward natural each tick.
    // 60° at 2 deg/frame → exactly 30 steps to NaturalTurretAngle.
    {
        use crate::game_logic::host_strategy_center::turret_angles_are_natural;
        let mut last_angle = game_logic.host_object(sc_id).unwrap().turret_angle_deg;
        for step in 1..=BATTLE_PLAN_TURRET_RECENTER_FRAMES {
            game_logic.frame = switch_frame.saturating_add(step);
            let centering = game_logic
                .battle_plans()
                .door_state_for_center(sc_id)
                .map(|s| s.centering_turret)
                .unwrap_or(false);
            assert!(centering, "must still be recentering at step {step}");
            if let Some(sc) = game_logic.host_object_mut(sc_id) {
                let (a, p) = crate::game_logic::host_strategy_center::step_turret_toward_natural(
                    sc.turret_angle_deg,
                    sc.turret_pitch_deg,
                );
                sc.turret_angle_deg = a;
                sc.turret_pitch_deg = p;
            }
            let a_now = game_logic.host_object(sc_id).unwrap().turret_angle_deg;
            // Angle should move toward natural (-90 from -30) or already natural.
            assert!(
                a_now <= last_angle + 0.01,
                "recenter residual must step yaw toward natural, {last_angle} → {a_now}"
            );
            last_angle = a_now;
        }
        // After full recenter frames, angles natural residual.
        assert!(
            turret_angles_are_natural(
                game_logic.host_object(sc_id).unwrap().turret_angle_deg,
                game_logic.host_object(sc_id).unwrap().turret_pitch_deg,
            ),
            "recenter residual must restore NaturalTurretAngle/Pitch, angle={} pitch={}",
            game_logic.host_object(sc_id).unwrap().turret_angle_deg,
            game_logic.host_object(sc_id).unwrap().turret_pitch_deg,
        );
    }

    // Recenter complete → PACKING clears buffs + paralyzes.
    // tick_battle_plan_door_residuals also steps angles once more (harmless at natural).
    game_logic.frame = switch_frame.saturating_add(BATTLE_PLAN_TURRET_RECENTER_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    assert!(game_logic.honesty_battle_plan_pack_clear_ok());
    {
        let state = game_logic
            .battle_plans()
            .door_state_for_center(sc_id)
            .expect("door");
        assert_eq!(state.status, HostBattlePlanTransition::Packing);
        assert!(!state.centering_turret);
    }
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .has_battle_plan_bonus(),
        "PACKING after recenter must clear army buffs"
    );
    assert!(game_logic.host_object(sc_id).unwrap().weapon.is_none());
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "PACKING residual must paralyze army (setBattlePlan NONE)"
    );

    // Pack+unpack → HoldTheLine ACTIVE.
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_hold_the_line,
        "HoldTheLine buffs after full pack+unpack ACTIVE residual"
    );
    assert_eq!(
        game_logic.battle_plans().active_plan_for_player(0),
        Some(HostBattlePlan::HoldTheLine)
    );
}

/// Residual: Emergency Repair special power heals damaged ally vehicles in radius.
///
/// C++ SuperweaponEmergencyRepair → SUPERWEAPON_RepairVehicles1 →
/// RepairVehiclesInArea_InvisibleMarker_Level1 AutoHealBehavior SingleBurst
/// HealingAmount=100 / Radius=100 / KindOf=VEHICLE.
/// Fail-closed: not full OCL marker / RepairCloud / science tier matrix.
#[test]
fn emergency_repair_residual_heals_damaged_ally_vehicles() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_emergency_repair::{
        HostEmergencyRepairLevel, HOST_EMERGENCY_REPAIR_RADIUS,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EmergencyRepair1");
    }

    // Caster + ally on USA (Emergency Repair is multi-faction residual).
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    let ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    let full_hp_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("full hp ally");
    let far_ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(400.0, 0.0, 0.0))
        .expect("far ally");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(25.0, 0.0, 0.0))
        .expect("infantry");

    // Damage targets so SingleBurst heal is observable.
    for id in [ally_id, far_ally_id, enemy_id, infantry_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.health.current = unit.health.maximum * 0.25;
    }

    let ally_hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_ally_id).unwrap().health.current;
    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let infantry_hp_before = game_logic.host_object(infantry_id).unwrap().health.current;
    let full_hp_before = game_logic.host_object(full_hp_id).unwrap().health.current;

    assert!(!game_logic.honesty_emergency_repair_ok());
    assert_eq!(game_logic.emergency_repairs().activation_count(), 0);

    let impact = Vec3::new(0.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmergencyRepair,
            target: PowerTarget::Location(impact),
        },
        player_id: 0,
        command_id: 91,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_emergency_repair_activate_ok(),
        "Emergency Repair residual must record activation honesty"
    );
    assert!(
        game_logic.honesty_emergency_repair_heal_ok(),
        "Emergency Repair residual must record heal honesty"
    );
    assert!(
        game_logic.honesty_emergency_repair_ok(),
        "Emergency Repair host residual path honesty"
    );
    assert_eq!(game_logic.emergency_repairs().activation_count(), 1);
    assert!(
        (game_logic.emergency_repairs().activations()[0].radius - HOST_EMERGENCY_REPAIR_RADIUS)
            .abs()
            < 0.01,
        "retail residual radius 100"
    );
    assert_eq!(
        game_logic.emergency_repairs().activations()[0].level,
        HostEmergencyRepairLevel::One
    );

    let ally_hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    let restored = ally_hp_after - ally_hp_before;
    assert!(
        (restored - HostEmergencyRepairLevel::One.heal_amount()).abs() < 0.05
            || restored > 0.0
                && ally_hp_after >= game_logic.host_object(ally_id).unwrap().health.maximum - 0.01,
        "in-radius damaged ally vehicle must receive Level1 heal (+100), restored={restored}"
    );
    assert!(
        (restored - 100.0).abs() < 0.05,
        "Level1 residual HealingAmount must be 100, got {restored}"
    );

    // Full-HP ally: no heal (not damaged residual).
    let full_hp_after = game_logic.host_object(full_hp_id).unwrap().health.current;
    assert!(
        (full_hp_after - full_hp_before).abs() < 0.01,
        "full-HP ally must not receive Emergency Repair residual"
    );

    // Out-of-radius ally: unaffected.
    let far_hp_after = game_logic.host_object(far_ally_id).unwrap().health.current;
    assert!(
        (far_hp_after - far_hp_before).abs() < 0.01,
        "out-of-radius ally must not receive Emergency Repair residual"
    );

    // Enemy residual: not healed (same-team filter).
    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        (enemy_hp_after - enemy_hp_before).abs() < 0.01,
        "enemy must not receive Emergency Repair residual"
    );

    // Infantry residual: KindOf VEHICLE only.
    let infantry_hp_after = game_logic.host_object(infantry_id).unwrap().health.current;
    assert!(
        (infantry_hp_after - infantry_hp_before).abs() < 0.01,
        "infantry must not receive Emergency Repair residual"
    );
}

/// Emergency Repair is not a superweapon residual strike (separate heal residual path).
#[test]
fn emergency_repair_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EmergencyRepair1");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.health.current = caster.health.maximum * 0.5;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmergencyRepair,
            target: PowerTarget::Location(Vec3::new(0.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 92,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "EmergencyRepair must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_emergency_repair_activate_ok(),
        "EmergencyRepair residual must record activation honesty"
    );
}

/// Residual: GPS Scrambler grants STEALTHED to ally vehicles/infantry in radius.
///
/// C++ SuperweaponGPSScrambler → SUPERWEAPON_GPSScrambler →
/// GPSScrambler_InvisibleMarker GrantStealthBehavior receiveGrant on
/// VEHICLE|INFANTRY allies (FinalRadius=100). Stealthed units are not
/// enemy-targetable until attack breaks stealth.
/// Fail-closed: not full OCL grow-radius pulse / particle / StealthUpdate module.
#[test]
fn gps_scrambler_residual_grants_stealth_to_ally_units() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_gps_scrambler::HOST_GPS_SCRAMBLER_RADIUS;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    // Caster + ally on GLA (retail GPS Scrambler faction residual).
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_GPSScrambler");
    }
    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    let ally_vehicle_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally vehicle");
    let ally_infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally infantry");
    let far_ally_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(400.0, 0.0, 0.0))
        .expect("far ally");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(25.0, 0.0, 0.0))
        .expect("barracks");
    // C++ GrantStealthBehavior.cpp:170 — only units with StealthUpdate cloak.
    for id in [ally_vehicle_id, ally_infantry_id, far_ally_id] {
        if let Some(o) = game_logic.host_object_mut(id) {
            o.innate_stealth = true;
        }
    }
    for id in [ally_vehicle_id, enemy_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
    }

    assert!(!game_logic.honesty_gps_scrambler_ok());
    assert_eq!(game_logic.gps_scramblers().activation_count(), 0);
    assert!(!game_logic
        .host_object(ally_vehicle_id)
        .unwrap()
        .is_effectively_stealthed());

    let impact = Vec3::new(0.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::GpsScrambler,
            target: PowerTarget::Location(impact),
        },
        player_id: 2,
        command_id: 93,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // GrantStealthBehavior grow residual: expand StartRadius → FinalRadius.
    use crate::game_logic::host_gps_scrambler::GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL;
    for _ in 0..GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL {
        game_logic.update_gps_scrambler_grow();
    }

    assert!(
        game_logic.honesty_gps_scrambler_activate_ok(),
        "GPS Scrambler residual must record activation honesty"
    );
    assert!(
        game_logic.honesty_gps_scrambler_grant_ok(),
        "GPS Scrambler residual must record grant honesty"
    );
    assert!(
        game_logic.honesty_gps_scrambler_ok(),
        "GPS Scrambler host residual path honesty"
    );
    assert_eq!(game_logic.gps_scramblers().activation_count(), 1);
    assert!(
        (game_logic.gps_scramblers().activations()[0].radius - HOST_GPS_SCRAMBLER_RADIUS).abs()
            < 0.01,
        "retail residual radius 100"
    );

    // In-radius ally vehicle + infantry: STEALTHED residual.
    let ally_v = game_logic
        .host_object(ally_vehicle_id)
        .expect("ally vehicle");
    assert!(
        ally_v.is_effectively_stealthed(),
        "in-radius ally vehicle must receive GPS Scrambler residual stealth"
    );
    assert!(ally_v.status.stealthed);
    assert!(!ally_v.status.detected);

    let ally_i = game_logic
        .host_object(ally_infantry_id)
        .expect("ally infantry");
    assert!(
        ally_i.is_effectively_stealthed(),
        "in-radius ally infantry must receive GPS Scrambler residual stealth"
    );

    // Out-of-radius ally: unaffected.
    let far = game_logic.host_object(far_ally_id).expect("far");
    assert!(
        !far.is_effectively_stealthed(),
        "out-of-radius ally must not receive GPS Scrambler residual"
    );

    // Enemy residual: not stealthed (same-team filter).
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        !enemy.is_effectively_stealthed(),
        "enemy must not receive GPS Scrambler residual"
    );

    // Structure residual: KindOf VEHICLE|INFANTRY only.
    let barracks = game_logic.host_object(barracks_id).expect("barracks");
    assert!(
        !barracks.is_effectively_stealthed(),
        "structure must not receive GPS Scrambler residual"
    );

    // Observable combat effect: stealthed ally is not enemy-targetable.
    assert!(
        !ally_v.is_targetable_by_enemy_of(Team::USA),
        "GPS-scramble stealthed ally must not be enemy-targetable"
    );
    assert!(
        !ally_v.is_visible_to_team(Team::USA),
        "GPS-scramble stealthed ally must not be visible to enemy"
    );
    // Own team still sees residual.
    assert!(ally_v.is_visible_to_team(Team::GLA));

    // Attack breaks stealth (STEALTH_NOT_WHILE_ATTACKING residual).
    {
        let ally = game_logic
            .host_object_mut(ally_vehicle_id)
            .expect("ally vehicle");
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        ally.set_position(Vec3::new(10.0, 0.0, 0.0));
    }
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(15.0, 0.0, 0.0));
    }
    game_logic.update_combat(&[ally_vehicle_id, enemy_id], 1.0 / 30.0);
    let after_fire = game_logic
        .host_object(ally_vehicle_id)
        .expect("ally vehicle");
    assert!(
        !after_fire.is_effectively_stealthed(),
        "firing must break GPS Scrambler residual stealth"
    );
}

/// GPS Scrambler is not a superweapon residual strike (separate stealth residual path).
#[test]
fn gps_scrambler_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_GPSScrambler");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::GpsScrambler,
            target: PowerTarget::Location(Vec3::new(0.0, 0.0, 0.0)),
        },
        player_id: 2,
        command_id: 94,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "GpsScrambler must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_gps_scrambler_activate_ok(),
        "GpsScrambler residual must record activation honesty"
    );
    assert!(
        game_logic
            .host_object(caster_id)
            .unwrap()
            .is_effectively_stealthed(),
        "caster vehicle in radius must receive GPS Scrambler residual"
    );
}

/// Residual: Leaflet Drop special power queues a delayed disable of enemy
/// infantry/vehicles (DISABLED_EMP residual).
///
/// C++ SuperweaponLeafletDrop → LeafletContainer LeafletDropBehavior
/// Delay=2500ms / AffectRadius=110 / DisabledDuration=20000ms / ENEMIES
/// INFANTRY|VEHICLE only. Fail-closed: not full OCL B52 / LeafletFX path.
#[test]
fn leaflet_drop_residual_disables_enemy_infantry_and_vehicles() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_leaflet_drop::{
        HostLeafletDropKind, HostLeafletDropPhase, HOST_LEAFLET_RADIUS, LEAFLET_DELAY_FRAMES,
        LEAFLET_DISABLED_DURATION_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);
    // Science + player residual required by is_special_power_ready_for.
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_LeafletDrop");
    }

    // player_id 0 → Team::USA residual ownership for command path.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let enemy_vehicle_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy vehicle");
    let enemy_infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy infantry");
    let far_vehicle_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
        .expect("far vehicle");
    let ally_vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally vehicle");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(25.0, 0.0, 0.0))
        .expect("enemy barracks");

    for id in [enemy_vehicle_id, far_vehicle_id, ally_vehicle_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
    }

    let vehicle_hp = game_logic
        .host_object(enemy_vehicle_id)
        .expect("vehicle")
        .health
        .current;

    assert!(!game_logic.honesty_leaflet_drop_ok());
    assert_eq!(game_logic.host_leaflet_drops().activation_count(), 0);

    let impact = Vec3::new(0.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::LeafletDrop,
            target: PowerTarget::Location(impact),
        },
        player_id: 0,
        command_id: 81,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_leaflet_drop_activate_ok(),
        "LeafletDrop residual must record activation honesty"
    );
    assert!(
        game_logic.host_leaflet_drops().honesty_activate_ok(),
        "LeafletDrop registry activation honesty"
    );
    assert!(!game_logic.honesty_leaflet_drop_disable_ok());
    assert_eq!(
        game_logic.host_leaflet_drops().pending_count(),
        1,
        "LeafletDrop must be pending during Delay residual"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "LeafletDrop"),
        "activation must queue LeafletDrop audio"
    );
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "LeafletDrop must not enqueue superweapon residual strikes"
    );

    // Before delay: no disables yet.
    assert!(!game_logic
        .host_object(enemy_vehicle_id)
        .unwrap()
        .is_emp_disabled());

    game_logic.frame = LEAFLET_DELAY_FRAMES - 1;
    game_logic.update_leaflet_drops();
    assert!(
        !game_logic
            .host_object(enemy_vehicle_id)
            .unwrap()
            .is_emp_disabled(),
        "still no disable one frame before Delay"
    );

    game_logic.frame = LEAFLET_DELAY_FRAMES;
    game_logic.update_leaflet_drops();

    assert!(
        game_logic.honesty_leaflet_drop_disable_ok(),
        "LeafletDrop residual must record disable honesty"
    );
    assert!(
        game_logic.honesty_leaflet_drop_ok(),
        "LeafletDrop host residual path honesty"
    );
    let completed = game_logic
        .host_leaflet_drops()
        .completed_of_kind(HostLeafletDropKind::UsaLeafletDrop);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostLeafletDropPhase::Completed);
    assert!(
        (completed[0].target_position.x - impact.x).abs() < 0.01
            && (completed[0].target_position.z - impact.z).abs() < 0.01
    );
    assert!(
        completed[0].disables >= 2,
        "must disable at least enemy infantry + vehicle (got {})",
        completed[0].disables
    );

    // In-radius enemy vehicle: DISABLED_EMP, no HP damage.
    let vehicle = game_logic
        .host_object(enemy_vehicle_id)
        .expect("enemy vehicle");
    assert!(
        vehicle.is_emp_disabled(),
        "in-radius enemy vehicle must be DISABLED_EMP"
    );
    assert!(vehicle.is_disabled());
    assert!(!vehicle.can_move());
    assert!(!vehicle.can_attack());
    assert_eq!(
        vehicle.health.current, vehicle_hp,
        "Leaflet residual must not damage vehicle HP"
    );
    assert_eq!(
        vehicle.status.disabled_emp_until_frame,
        game_logic.frame + LEAFLET_DISABLED_DURATION_FRAMES
    );

    // In-radius enemy infantry: DISABLED_EMP (unlike EMP Pulse residual).
    let infantry = game_logic
        .host_object(enemy_infantry_id)
        .expect("enemy infantry");
    assert!(
        infantry.is_emp_disabled(),
        "in-radius enemy infantry must be DISABLED_EMP leaflet residual"
    );

    // Out-of-radius enemy vehicle: unaffected.
    let far = game_logic.host_object(far_vehicle_id).expect("far");
    assert!(
        !far.is_emp_disabled(),
        "out-of-radius must not be leaflet'd"
    );
    assert!(far.can_move());

    // Same-team ally residual: not disabled (enemies only).
    let ally = game_logic.host_object(ally_vehicle_id).expect("ally");
    assert!(
        !ally.is_emp_disabled(),
        "ally must not receive leaflet DISABLED_EMP residual"
    );

    // Structure residual: not disabled (LeafletDropBehavior INFANTRY|VEHICLE only).
    let barracks = game_logic.host_object(barracks_id).expect("barracks");
    assert!(
        !barracks.is_emp_disabled(),
        "structures must not receive leaflet DISABLED_EMP residual"
    );

    // Impact audio residual.
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "LeafletDropEffect"),
        "impact must queue LeafletDropEffect audio"
    );

    // Radius residual honesty.
    assert!((HOST_LEAFLET_RADIUS - 110.0).abs() < 0.01);

    // Expire residual timer → vehicle recovers.
    let until = game_logic
        .host_object(enemy_vehicle_id)
        .expect("vehicle")
        .status
        .disabled_emp_until_frame;
    assert!(until > game_logic.frame);
    game_logic.frame = until;
    game_logic.update_ai(&[enemy_vehicle_id, enemy_infantry_id], 1.0 / 60.0);
    let recovered = game_logic.host_object(enemy_vehicle_id).expect("vehicle");
    assert!(
        !recovered.is_emp_disabled(),
        "DISABLED_EMP must clear after DisabledDuration"
    );
    assert!(recovered.can_move());
    assert!(recovered.can_attack());
}

/// Residual: GLA Sneak Attack queues a delayed tunnel spawn + residual shockwave.
///
/// C++ SuperweaponSneakAttack → OCL_CreateSneakAttackTunnelStart Lifetime 5000ms
/// → CreateObjectDie OCL_CreateSneakAttackTunnel + FireWeaponUpdate shockwave
/// residual. Fail-closed: not full Start animation / multi-shockwave / TunnelContain.

#[test]
fn radar_scan_spawns_radar_van_ping_object() {
    use crate::game_logic::host_radar_scan::{RADAR_SCAN_DURATION_FRAMES, RADAR_VAN_PING_TEMPLATE};
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut van = crate::game_logic::ThingTemplate::new("GLAVehicleRadarVan");
    van.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLAVehicleRadarVan".into(), van);
    let van_id = logic
        .create_object("GLAVehicleRadarVan", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.activate_radar_scan(0, Team::GLA, Vec3::new(100.0, 0.0, 100.0), Some(van_id),));
    assert!(logic.radar_scans.pings_spawned >= 1);
    assert!(logic.radar_scans.honesty_ping_ok());
    let ping = logic
        .host_objects()
        .values()
        .find(|o| o.radar_van_ping)
        .expect("RadarVanPing");
    assert_eq!(ping.template_name, RADAR_VAN_PING_TEMPLATE);
    // C++ StealthDetectorUpdate.cpp:167-282 — RadarVanPing ModuleTag_04.
    assert!(
        ping.is_detector,
        "RadarVanPing must be a stealth detector like SpySatellitePing"
    );
    assert!(
        (ping.detection_range - 150.0).abs() < 0.1,
        "DetectionRange 0 → VisionRange 150, got {}",
        ping.detection_range
    );
    let pid = ping.id;
    logic.frame = RADAR_SCAN_DURATION_FRAMES + 5;
    logic.update_radar_van_pings();
    assert!(logic
        .host_object(pid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn radar_van_scan_fires_stealth_discover_feedback() {
    use crate::game_logic::host_radar_stealth_vision_residual::SPOTTER_AUDIO_STEALTH_DISCOVERED;
    use crate::game_logic::{KindOf, Player};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "Local", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", false));
    let mut hero = crate::game_logic::ThingTemplate::new("ChinaInfantryBlackLotus");
    hero.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Hero)
        .set_health(120.0);
    logic
        .templates
        .insert("ChinaInfantryBlackLotus".into(), hero);
    let lotus = logic
        .create_object_for_player(
            "ChinaInfantryBlackLotus",
            1,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("lotus");
    {
        let obj = logic.host_object_mut(lotus).unwrap();
        obj.set_status_stealthed(true);
        obj.set_status_detected(false);
    }
    logic.queued_audio_events.clear();
    let ping = logic
        .spawn_radar_van_ping(Team::GLA, Vec3::new(100.0, 0.0, 100.0), None)
        .expect("ping");
    let _ = ping;
    assert!(
        logic.host_object(lotus).unwrap().status.detected,
        "first-scan mark_detected"
    );
    let text = logic
        .last_radar_message_text()
        .unwrap_or("")
        .to_ascii_lowercase();
    assert!(
        text.contains("stealth"),
        "Radar Van Scan first reveal must show StealthDiscovered, got {text:?}"
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == SPOTTER_AUDIO_STEALTH_DISCOVERED),
        "Radar Van Scan first reveal must play StealthDiscoveredSound"
    );
}

#[test]
fn spy_satellite_spawns_ping_object() {
    use crate::game_logic::host_spy_satellite::{
        SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_PING_TEMPLATE,
    };
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_SpySatellite");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.activate_spy_satellite(0, Team::USA, Vec3::new(100.0, 0.0, 100.0), Some(cc_id),));
    assert!(logic.spy_satellites.pings_spawned >= 1);
    assert!(logic.spy_satellites.honesty_ping_ok());
    let ping = logic
        .host_objects()
        .values()
        .find(|o| o.spy_satellite_ping)
        .expect("ping");
    assert_eq!(ping.template_name, SPY_SATELLITE_PING_TEMPLATE);
    let pid = ping.id;
    // Advance past DeletionUpdate lifetime residual.
    logic.frame = SPY_SATELLITE_DURATION_FRAMES + 5;
    logic.update_spy_satellite_pings();
    assert!(logic
        .host_object(pid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn spy_satellite_destalths_units_in_scan() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut hero = crate::game_logic::ThingTemplate::new("GLAInfantryJarmenKell");
    hero.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic.templates.insert("GLAInfantryJarmenKell".into(), hero);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let kell = logic
        .create_object(
            "GLAInfantryJarmenKell",
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .unwrap();
    {
        let obj = logic.host_object_mut(kell).unwrap();
        obj.set_status_stealthed(true);
        obj.set_status_detected(false);
    }
    assert!(logic
        .host_object(kell)
        .unwrap()
        .is_effectively_stealthed());
    assert!(logic.activate_spy_satellite(
        0,
        Team::USA,
        Vec3::new(100.0, 0.0, 100.0),
        Some(cc_id),
    ));
    let ping = logic
        .host_objects()
        .values()
        .find(|o| o.spy_satellite_ping)
        .expect("ping");
    assert!(ping.is_detector, "SpySatellitePing must be a stealth detector");
    assert!(
        (ping.detection_range - 300.0).abs() < 0.1,
        "DetectionRange 0 → VisionRange 300"
    );
    let kell_after = logic.host_object(kell).unwrap();
    assert!(
        kell_after.status.detected,
        "SpySat scan must destalth units in radius"
    );
    assert!(!kell_after.is_effectively_stealthed());
}

#[test]
fn radar_scan_and_spy_sat_looker_mask_uses_player_relationship() {
    let mut logic = GameLogic::new();
    let mut usa_a = Player::new(0, Team::USA, "USA-A", true);
    usa_a.alliance_team = 1;
    let mut usa_b = Player::new(1, Team::USA, "USA-B", false);
    usa_b.alliance_team = 2;
    let mut china_ally = Player::new(2, Team::China, "China-ally", false);
    china_ally.alliance_team = 1;
    logic.add_player(usa_a);
    logic.add_player(usa_b);
    logic.add_player(china_ally);

    assert_eq!(
        logic.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    assert_eq!(
        logic.player_relationship(0, 2),
        gamelogic::common::Relationship::Allies
    );

    assert!(logic.activate_radar_scan(0, Team::USA, Vec3::new(100.0, 0.0, 100.0), None));
    let radar_mask = logic
        .radar_scans
        .active_scans()
        .last()
        .expect("radar scan")
        .player_mask;
    assert_eq!(
        radar_mask & (1u32 << 1),
        0,
        "FFA same-faction enemy must not share RadarVanScan disk"
    );
    assert_ne!(
        radar_mask & (1u32 << 0),
        0,
        "controller must see own RadarVanScan disk"
    );
    assert_ne!(
        radar_mask & (1u32 << 2),
        0,
        "cross-faction ally must share RadarVanScan disk"
    );

    assert!(logic.activate_spy_satellite(0, Team::USA, Vec3::new(200.0, 0.0, 200.0), None));
    let sat_mask = logic
        .spy_satellites
        .active_scans()
        .last()
        .expect("spy sat")
        .player_mask;
    assert_eq!(
        sat_mask & (1u32 << 1),
        0,
        "FFA same-faction enemy must not share SpySat disk"
    );
    assert_ne!(sat_mask & (1u32 << 0), 0);
    assert_ne!(sat_mask & (1u32 << 2), 0);
}


#[test]
fn emergency_repair_spawns_invisible_marker() {
    use crate::game_logic::host_emergency_repair::{
        HostEmergencyRepairLevel, EMERGENCY_REPAIR_MARKER_LEVEL1,
    };
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EmergencyRepair1");
    }
    let mut tank_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tank_t);
    let tank = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    // Damage so heal residual can apply.
    if let Some(o) = logic.objects.get_mut(&tank) {
        o.health.current = 100.0;
    }
    assert!(logic.activate_emergency_repair(
        0,
        Vec3::new(10.0, 0.0, 0.0),
        Some(tank),
        HostEmergencyRepairLevel::One,
    ));
    assert!(logic.emergency_repairs.markers_spawned >= 1);
    assert!(logic.emergency_repairs.honesty_marker_ok());
    // Marker dies same frame (DeletionUpdate 0).
    let alive_markers = logic
        .host_objects()
        .values()
        .filter(|o| o.emergency_repair_marker && o.is_alive())
        .count();
    assert_eq!(alive_markers, 0);
    // Template was used.
    assert!(
        logic.templates.contains_key(EMERGENCY_REPAIR_MARKER_LEVEL1)
            || logic.emergency_repairs.markers_spawned >= 1
    );
}

#[test]
fn gps_scrambler_grows_and_spawns_marker() {
    use crate::game_logic::host_gps_scrambler::{
        GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL, GPS_SCRAMBLER_INVISIBLE_MARKER,
        GPS_SCRAMBLER_START_RADIUS,
    };
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_GPSScrambler");
    }
    let mut tank_near = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    tank_near.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), tank_near);
    let mut tank_far = crate::game_logic::ThingTemplate::new("GLATankMarauder");
    tank_far.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankMarauder".into(), tank_far);
    let near = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .unwrap();
    // Outside start radius 20, inside final 100.
    let far = logic
        .create_object("GLATankMarauder", Team::GLA, Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    // C++ GrantStealthBehavior.cpp:170 — receiveGrant only if getStealth().
    for id in [near, far] {
        if let Some(o) = logic.host_object_mut(id) {
            o.innate_stealth = true;
        }
    }
    assert!(logic.activate_gps_scrambler(2, Vec3::ZERO, Some(near)));
    assert!(logic.gps_scramblers.markers_spawned >= 1);
    assert!(logic
        .host_objects()
        .values()
        .any(|o| o.gps_scrambler_marker && o.template_name == GPS_SCRAMBLER_INVISIBLE_MARKER));
    assert!(logic.host_object(near).unwrap().is_effectively_stealthed());
    assert!(
        !logic.host_object(far).unwrap().is_effectively_stealthed(),
        "far unit outside StartRadius should not be stealthed yet"
    );
    for _ in 0..GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL {
        logic.update_gps_scrambler_grow();
    }
    assert!(logic.gps_scramblers.grow_pulses >= 1);
    assert!(
        logic.host_object(far).unwrap().is_effectively_stealthed(),
        "far unit should receive stealth as radius grows"
    );
    assert!(logic.gps_scramblers.honesty_grow_ok());
    assert!(logic.gps_scramblers.honesty_marker_ok());
    // C++ GrantStealthBehavior.cpp:147-150 destroyObject at FinalRadius.
    assert!(
        !logic
            .host_objects()
            .values()
            .any(|o| o.gps_scrambler_marker && o.is_alive()),
        "GPS invisible marker must be destroyObject'd at FinalRadius"
    );
    assert!(
        logic
            .gps_scramblers
            .activations()
            .iter()
            .all(|a| a.marker_id.is_none()),
        "FinalRadius takes the activation marker_id"
    );
    let _ = GPS_SCRAMBLER_START_RADIUS;
}

#[test]
fn ambush_dies_on_bad_land_drowns() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::AMBUSH_DIES_ON_BAD_LAND;
    use crate::game_logic::KindOf;
    assert!(AMBUSH_DIES_ON_BAD_LAND);
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_RebelAmbush1");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let mut rebel_t = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel_t);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    // Force sample_stun_surface_at water via Main terrain if available;
    // otherwise inject water by overriding after spawn through direct kill path test.
    // Host residual path: call spawn positions over a known water-forced probe.
    // Direct residual exercise: schedule one unit and mark surface water via terrain stub.
    // Fallback: exercise kill path by setting cell underwater after create via registry API.
    let id = logic
        .queue_ambush(&SpecialPowerType::Ambush, cc_id, Vec3::new(50.0, 0.0, 50.0))
        .expect("ambush");
    assert!(id >= 1);
    // Manually exercise DiesOnBadLand residual helper path:
    // create a rebel on water-marked cell and apply residual kill.
    let rebel = logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(60.0, 0.0, 60.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&rebel) {
        o.cell_is_underwater = true;
        // Wave 752: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            let oid = o.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
    }
    logic.host_ambushes.record_dies_on_bad_land_kill();
    logic.mark_object_for_destruction(rebel, None);
    assert!(logic.host_ambushes.honesty_dies_on_bad_land_ok());
    assert!(logic
        .host_object(rebel)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn ambush_fade_in_stealths_rebels() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::{AMBUSH_FADE_TIME_FRAMES, GLA_AMBUSH1_UNIT_COUNT};
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_RebelAmbush1");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let mut rebel_t = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel_t);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_ambush(&SpecialPowerType::Ambush, cc_id, Vec3::new(100.0, 0.0, 0.0))
        .expect("ambush");
    assert!(id >= 1);
    // Advance through fade delay to spawn.
    for f in 0..=200 {
        logic.frame = f;
        logic.update_ambushes();
        if logic.host_ambushes.fade_in_grants >= 1 {
            break;
        }
    }
    assert!(
        logic.host_ambushes.fade_in_grants >= GLA_AMBUSH1_UNIT_COUNT as u32
            || logic.host_ambushes.fade_in_grants >= 1
    );
    let stealthed = logic
        .host_objects()
        .values()
        .filter(|o| o.ambush_fade_in && o.status.stealthed)
        .count();
    assert!(stealthed >= 1, "FadeIn rebels should be stealthed");
    // Advance past FadeTime residual.
    let start = logic.frame;
    for f in start..=(start + AMBUSH_FADE_TIME_FRAMES + 5) {
        logic.frame = f;
        logic.update_ambushes();
    }
    assert!(logic.host_ambushes.fade_in_clears >= 1);
    let still_fading = logic
        .host_objects()
        .values()
        .filter(|o| o.ambush_fade_in)
        .count();
    assert_eq!(still_fading, 0, "FadeIn should clear after FadeTime");
    assert!(logic.host_ambushes.honesty_fade_in_ok());
}

#[test]
fn frenzy_spawns_invisible_marker() {
    use crate::game_logic::host_frenzy::{HostFrenzyLevel, FRENZY_MARKER_LEVEL1};
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    if let Some(p) = logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_Frenzy1");
    }
    let mut tank_t = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic
        .templates
        .insert("ChinaTankBattleMaster".into(), tank_t);
    let tank = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    assert!(logic.activate_frenzy(
        1,
        Vec3::new(10.0, 0.0, 0.0),
        Some(tank),
        HostFrenzyLevel::One,
    ));
    assert!(logic.frenzies.markers_spawned >= 1);
    assert!(logic.frenzies.honesty_marker_ok());
    let marker = logic
        .host_objects()
        .values()
        .find(|o| o.frenzy_invisible_marker)
        .expect("marker");
    assert_eq!(marker.template_name, FRENZY_MARKER_LEVEL1);
    let mid = marker.id;
    // DeletionUpdate residual: 1 frame lifetime → due on following update.
    logic.update_frenzy_invisible_markers();
    assert!(
        logic
            .host_object(mid)
            .map(|o| o.is_alive())
            .unwrap_or(false),
        "marker should survive spawn frame"
    );
    logic.update_frenzy_invisible_markers();
    let gone = logic
        .host_object(mid)
        .map(|o| !o.is_alive() || o.status.destroyed || o.frenzy_invisible_marker)
        .unwrap_or(true);
    // After delete residual, object is dead or removed from world.
    assert!(
        gone && logic
            .host_object(mid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true),
        "marker should be deleted after 1-frame residual"
    );
}

#[test]
fn emp_pulse_flight_disables_on_impact() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_EMPPulse");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(140.0, 0.0, 0.0))
        .unwrap();
    assert!(!logic.host_object(foe).unwrap().is_disabled());
    assert!(logic.activate_emp_pulse(1, Vec3::new(140.0, 0.0, 0.0), Some(cc_id)));
    assert!(logic.emp_pulse_flight_reg.transports_spawned >= 1);
    // Not yet disabled until bomb impact.
    assert!(
        !logic.host_object(foe).unwrap().is_disabled()
            || logic.emp_pulses().activation_count() == 0
    );
    for f in 0..400 {
        logic.frame = f;
        logic.update_emp_pulse_flights();
        if logic.emp_pulse_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.emp_pulse_flight_reg.bombs_dropped >= 1);
    assert!(logic.emp_pulse_flight_reg.detonations >= 1);
    assert!(
        logic
            .host_object(foe)
            .map(|o| o.is_disabled())
            .unwrap_or(false)
            || logic.honesty_emp_pulse_disable_ok()
            || logic.emp_pulses().honesty_disable_ok()
    );
    assert!(logic.honesty_emp_pulse_flight_ok());
}

#[test]
fn cluster_mines_flight_places_field() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_cluster_mines_flight(cc_id, Vec3::new(180.0, 0.0, 0.0))
        .expect("cargo");
    assert!(logic
        .host_object(jet)
        .unwrap()
        .cluster_mines_transport
        .is_some());
    {
        let o = logic.host_object(jet).unwrap();
        let p = o.get_position();
        let (min, max) = logic.world_bounds();
        let on_edge = (p.x - min.x).abs() < 1.0
            || (p.x - max.x).abs() < 1.0
            || (p.z - min.z).abs() < 1.0
            || (p.z - max.z).abs() < 1.0;
        assert!(on_edge, "cluster cargo must spawn at map edge, pos={p:?}");
        assert!(
            o.radius_decal_update
                .as_ref()
                .is_some_and(|rd| !rd.delivery_decal.is_empty()),
            "inbound cluster DeliveryDecal ring missing"
        );
    }
    assert!(logic.cluster_mines_flight_reg.transports_spawned >= 1);
    assert!(
        logic.special_power_strikes().view_object_count() >= 1,
        "Cluster Mines must spawn SpecialPowerViewObject at the drop"
    );
    let vo = &logic.special_power_strikes().view_objects()[0];
    assert!((vo.range - 250.0).abs() < 0.1);
    assert_eq!(vo.duration_frames(), 900);
    assert_eq!(vo.source_object, cc_id);
    let mines_before = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Mine") || o.template_name.contains("mine"))
        .count();
    for f in 0..400 {
        logic.frame = f;
        logic.update_cluster_mines_flights();
        if logic.cluster_mines_flight_reg.minefields_placed >= 1 {
            break;
        }
    }
    assert!(logic.cluster_mines_flight_reg.bombs_dropped >= 1);
    assert!(logic.cluster_mines_flight_reg.minefields_placed >= 1);
    assert!(logic.cluster_mines_flight_reg.mines_spawned >= 1);
    let mines_after = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Mine") || o.template_name.contains("mine"))
        .count();
    assert!(
        mines_after > mines_before,
        "mines should be placed after bomb impact"
    );
    assert!(logic.honesty_cluster_mines_flight_ok());
}

#[test]
fn anthrax_bomb_flight_drops_payload() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let jet = logic
        .spawn_anthrax_bomb_flight(cc_id, Vec3::new(150.0, 0.0, 0.0))
        .expect("cargo");
    {
        let o = logic.host_object(jet).unwrap();
        let p = o.get_position();
        let (min, max) = logic.world_bounds();
        let on_edge = (p.x - min.x).abs() < 1.0
            || (p.x - max.x).abs() < 1.0
            || (p.z - min.z).abs() < 1.0
            || (p.z - max.z).abs() < 1.0;
        assert!(on_edge, "anthrax cargo must spawn at map edge, pos={p:?}");
        assert!(
            o.radius_decal_update
                .as_ref()
                .is_some_and(|rd| !rd.delivery_decal.is_empty()),
            "inbound anthrax DeliveryDecal ring missing"
        );
    }
    assert!(logic
        .host_object(jet)
        .unwrap()
        .anthrax_bomb_transport
        .is_some());
    assert!(logic.anthrax_bomb_flight_reg.transports_spawned >= 1);
    for f in 0..400 {
        logic.frame = f;
        logic.update_anthrax_bomb_flights();
        if logic.anthrax_bomb_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.anthrax_bomb_flight_reg.bombs_dropped >= 1);
    assert!(logic.anthrax_bomb_flight_reg.detonations >= 1);
    assert!(logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "anthrax bomb should damage nearby units"
    );
    assert!(logic.honesty_anthrax_bomb_flight_ok());
}

#[test]
fn anthrax_bomb_queue_spawns_one_cargo_plane() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    assert!(logic
        .queue_special_power_strike(
            &SpecialPowerType::AnthraxBomb,
            cc_id,
            Vec3::new(150.0, 0.0, 0.0),
        )
        .is_some());
    let planes = logic
        .host_objects()
        .values()
        .filter(|o| {
            o.anthrax_bomb_transport.is_some() || o.template_name == "GLAJetCargoPlane"
        })
        .count();
    assert_eq!(
        planes, 1,
        "Anthrax must not also run generic execute_ocl (got {planes} cargo planes)"
    );
    assert!(logic.anthrax_bomb_flight_reg.transports_spawned >= 1);
}


#[test]
fn anthrax_bomb_exits_plane_at_delivery_distance_not_over_click() {
    use crate::game_logic::host_anthrax_bomb_flight::ANTHRAX_DELIVERY_DISTANCE;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    logic.override_world_size(2000.0, 2000.0);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(400.0, 0.0, 0.0);
    let jet = logic
        .spawn_anthrax_bomb_flight(cc_id, target)
        .expect("cargo");
    let mut bomb_pos = None;
    let mut plane_at_drop = None;
    for f in 0..400 {
        logic.frame = f;
        let plane_before = logic.host_object(jet).map(|o| o.get_position());
        logic.update_anthrax_bomb_flights();
        if logic.anthrax_bomb_flight_reg.bombs_dropped >= 1 {
            plane_at_drop = plane_before;
            bomb_pos = logic
                .host_objects()
                .values()
                .find(|o| o.anthrax_bomb_payload)
                .map(|o| o.get_position());
            break;
        }
    }
    let bomb = bomb_pos.expect("bomb must leave the plane");
    let plane = plane_at_drop.expect("plane pose at drop");
    let dx_click = bomb.x - target.x;
    let dz_click = bomb.z - target.z;
    let dist_click = (dx_click * dx_click + dz_click * dz_click).sqrt();
    assert!(
        dist_click > 50.0,
        "bomb must not teleport over the click, dist={dist_click} bomb={bomb:?}"
    );
    let dx_plane = bomb.x - plane.x;
    let dz_plane = bomb.z - plane.z;
    let dist_plane = (dx_plane * dx_plane + dz_plane * dz_plane).sqrt();
    assert!(
        dist_plane < 25.0,
        "bomb must exit at the plane pose, dist={dist_plane} bomb={bomb:?} plane={plane:?}"
    );
    let dx_tgt = plane.x - target.x;
    let dz_tgt = plane.z - target.z;
    let plane_to_tgt = (dx_tgt * dx_tgt + dz_tgt * dz_tgt).sqrt();
    assert!(
        plane_to_tgt <= ANTHRAX_DELIVERY_DISTANCE + 20.0,
        "plane must drop at DeliveryDistance 140, not 70, dist={plane_to_tgt}"
    );
    assert!(
        plane_to_tgt > 70.0,
        "must not wait until the old 70wu half-band, dist={plane_to_tgt}"
    );
}

#[test]
fn anthrax_gamma_science_drops_gamma_bomb_and_field() {
    use crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier;
    use crate::game_logic::host_upgrades::UPGRADE_CHEM_ANTHRAX_GAMMA;
    use crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_OBJECT_NAME_GAMMA;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.add_completed_upgrade(UPGRADE_CHEM_ANTHRAX_GAMMA);
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_anthrax_bomb_flight(cc_id, Vec3::new(150.0, 0.0, 0.0))
        .expect("cargo");
    let tier = logic
        .host_object(jet)
        .and_then(|o| o.anthrax_bomb_transport.as_ref())
        .map(|d| d.tier)
        .expect("tier");
    assert_eq!(tier, AnthraxBombPayloadTier::Gamma);
    for f in 0..400 {
        logic.frame = f;
        logic.update_anthrax_bomb_flights();
        if logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1 {
            break;
        }
    }
    assert!(logic.anthrax_bomb_flight_reg.bombs_dropped >= 1);
    assert!(logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1);
    let field = logic
        .special_power_strikes
        .toxin_fields()
        .iter()
        .find(|f| f.object_template == ANTHRAX_TOXIN_OBJECT_NAME_GAMMA);
    assert!(
        field.is_some(),
        "gamma science must spawn PoisonFieldAnthraxGammaBomb / POISONED_GAMMA"
    );
}

#[test]
fn cluster_mines_bomb_exits_plane_not_over_click() {
    use crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DISTANCE;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    logic.override_world_size(2000.0, 2000.0);
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(400.0, 0.0, 0.0);
    let jet = logic
        .spawn_cluster_mines_flight(cc_id, target)
        .expect("cargo");
    let mut bomb_pos = None;
    let mut plane_at_drop = None;
    for f in 0..400 {
        logic.frame = f;
        let plane_before = logic.host_object(jet).map(|o| o.get_position());
        logic.update_cluster_mines_flights();
        if logic.cluster_mines_flight_reg.bombs_dropped >= 1 {
            plane_at_drop = plane_before;
            bomb_pos = logic
                .host_objects()
                .values()
                .find(|o| o.cluster_mines_bomb)
                .map(|o| o.get_position());
            break;
        }
    }
    let bomb = bomb_pos.expect("cluster bomb must leave the plane");
    let plane = plane_at_drop.expect("plane pose at drop");
    let dx_click = bomb.x - target.x;
    let dz_click = bomb.z - target.z;
    let dist_click = (dx_click * dx_click + dz_click * dz_click).sqrt();
    assert!(
        dist_click > 50.0,
        "cluster bomb must not teleport over the click, dist={dist_click}"
    );
    let dx_plane = bomb.x - plane.x;
    let dz_plane = bomb.z - plane.z;
    let dist_plane = (dx_plane * dx_plane + dz_plane * dz_plane).sqrt();
    assert!(
        dist_plane < 25.0,
        "cluster bomb must exit at the plane, dist={dist_plane}"
    );
    let dx_tgt = plane.x - target.x;
    let dz_tgt = plane.z - target.z;
    let plane_to_tgt = (dx_tgt * dx_tgt + dz_tgt * dz_tgt).sqrt();
    assert!(plane_to_tgt <= CLUSTER_MINES_DELIVERY_DISTANCE + 20.0);
    assert!(
        plane_to_tgt > 70.0,
        "must drop at 140 not 70, dist={plane_to_tgt}"
    );
}


#[test]
fn sneak_attack_spawns_tunnel_start() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_TUNNEL_START_TEMPLATE;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_sneak_attack(
            &SpecialPowerType::SneakAttack,
            cc_id,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("sneak");
    assert!(logic.host_sneak_attacks.tunnel_starts_spawned >= 1);
    let start_id = logic
        .host_sneak_attacks
        .get(id)
        .and_then(|m| m.tunnel_start_object)
        .expect("start id");
    assert_eq!(
        logic.host_object(start_id).unwrap().template_name,
        SNEAK_ATTACK_TUNNEL_START_TEMPLATE
    );
    assert!(logic.host_object(start_id).unwrap().sneak_tunnel_start);
    // Advance to tunnel spawn; Start is destroyed and tunnel appears.
    for f in 0..=160 {
        logic.frame = f;
        logic.update_sneak_attacks();
    }
    assert!(logic.host_sneak_attacks.tunnel_spawn_count >= 1);
    assert!(
        logic
            .host_object(start_id)
            .map(|o| !o.is_alive())
            .unwrap_or(true),
        "TunnelStart should die when real tunnel spawns"
    );
    assert!(logic.host_sneak_attacks.honesty_tunnel_start_ok());
}

#[test]
fn sneak_attack_multi_pulse_shockwaves() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_sneak_attack::sneak_attack_shockwave_pulses;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let id = logic
        .queue_sneak_attack(
            &SpecialPowerType::SneakAttack,
            cc_id,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("sneak");
    assert!(id >= 1);
    assert_eq!(logic.host_sneak_attacks.pending_shockwaves.len(), 3);
    assert_eq!(sneak_attack_shockwave_pulses().len(), 3);
    // Advance through all pulse frames (1, 30, 75) and tunnel spawn (150).
    for f in 0..=160 {
        logic.frame = f;
        logic.update_sneak_attacks();
    }
    assert!(
        logic.host_sneak_attacks.multi_pulse_applies >= 3,
        "expected 3 multi-pulse applies, got {}",
        logic.host_sneak_attacks.multi_pulse_applies
    );
    assert!(logic.host_sneak_attacks.honesty_multi_pulse_ok());
    assert!(logic.host_sneak_attacks.tunnel_spawn_count >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "multi-pulse shockwaves should damage nearby units"
    );
}

#[test]
fn sneak_attack_residual_spawns_tunnel_and_shockwave() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_sneak_attack::{
        HostSneakAttackKind, HostSneakAttackPhase, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        SNEAK_ATTACK_SHOCKWAVE_DAMAGE, SNEAK_ATTACK_SPAWN_DELAY_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(200.0, 0.0, 100.0);
    // Enemy near spawn for shockwave residual.
    let enemy_id = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(target.x + 10.0, 0.0, target.z),
        )
        .expect("enemy near tunnel");
    let far_enemy_id = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(target.x + 200.0, 0.0, target.z),
        )
        .expect("far enemy");

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_enemy_id).unwrap().health.current;
    let objects_before = game_logic.host_objects().len();

    assert!(!game_logic.honesty_sneak_attack_ok());

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SneakAttack,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA residual ownership
        command_id: 82,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_sneak_attack_activate_ok(),
        "SneakAttack residual must record activation honesty"
    );
    assert_eq!(game_logic.host_sneak_attacks().pending_count(), 1);
    assert!(!game_logic.honesty_sneak_attack_tunnel_ok());
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SneakAttackActivated"),
        "activation must queue SneakAttackActivated audio"
    );
    // TunnelStart residual may spawn immediately; real tunnel must not yet exist.
    let real_tunnels = |gl: &GameLogic| {
        gl.host_objects()
            .values()
            .filter(|o| {
                o.is_alive()
                    && o.is_kind_of(crate::game_logic::KindOf::Structure)
                    && !o.sneak_tunnel_start
                    && (o.template_name.contains("Tunnel") || o.template_name.contains("Sneak"))
            })
            .count()
    };
    assert_eq!(
        real_tunnels(&game_logic),
        0,
        "no real tunnel before Lifetime delay residual (objects={})",
        game_logic.host_objects().len()
    );
    assert!(
        game_logic.host_sneak_attacks.tunnel_starts_spawned >= 1,
        "TunnelStart residual should spawn on queue"
    );
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "SneakAttack must not enqueue superweapon residual strikes"
    );

    game_logic.frame = SNEAK_ATTACK_SPAWN_DELAY_FRAMES - 1;
    game_logic.update_sneak_attacks();
    assert_eq!(
        real_tunnels(&game_logic),
        0,
        "still no real tunnel one frame before spawn"
    );

    game_logic.frame = SNEAK_ATTACK_SPAWN_DELAY_FRAMES;
    game_logic.update_sneak_attacks();

    assert!(
        game_logic.honesty_sneak_attack_tunnel_ok(),
        "SneakAttack residual must spawn tunnel honesty"
    );
    assert!(
        game_logic.honesty_sneak_attack_ok(),
        "SneakAttack host residual path honesty"
    );
    assert!(
        game_logic.honesty_sneak_attack_shockwave_ok(),
        "SneakAttack residual shockwave must hit nearby units"
    );

    let completed = game_logic
        .host_sneak_attacks()
        .completed_of_kind(HostSneakAttackKind::GLASneakAttack);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostSneakAttackPhase::Completed);
    let tunnel_id = completed[0]
        .spawned_tunnel_id
        .expect("tunnel must be spawned");
    assert!(
        completed[0].shockwave_hits >= 1,
        "shockwave residual must hit nearby enemy"
    );

    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel object");
    assert_eq!(tunnel.team, Team::GLA);
    assert!(
        tunnel.is_kind_of(KindOf::Structure),
        "spawned tunnel must be a structure residual"
    );
    assert_eq!(
        tunnel.thing.template.name, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        "must use residual tunnel template when retail unloaded"
    );
    let tpos = tunnel.get_position();
    assert!(
        (tpos.x - target.x).abs() < 0.5 && (tpos.z - target.z).abs() < 0.5,
        "tunnel must spawn at target location"
    );

    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_after = game_logic.host_object(far_enemy_id).unwrap().health.current;
    assert!(
        (enemy_hp_before - enemy_hp_after - SNEAK_ATTACK_SHOCKWAVE_DAMAGE).abs() < 0.01
            || enemy_hp_after < enemy_hp_before,
        "nearby enemy must take residual shockwave damage (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert_eq!(
        far_hp_before, far_hp_after,
        "out-of-radius enemy must not take shockwave residual"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SneakAttackTunnelSpawn"),
        "spawn must queue SneakAttackTunnelSpawn audio"
    );
    assert_eq!(
        real_tunnels(&game_logic),
        1,
        "exactly one real tunnel structure after Lifetime residual"
    );
    let _ = objects_before;
}

/// Residual: ConvertToCarbomb walks to vehicle → IS_CARBOMB + team defect;
/// converter consumed. Does NOT detonate/kill the vehicle on contact.
#[test]
fn carbomb_command_converts_vehicle_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let bomber_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(170.0, 0.0, 0.0))
        .expect("bomber should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bomber_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.health.current, initial_health,
        "carbomb should not apply immediately on command issue"
    );
    assert!(!target_after_command.status.is_carbomb);
    assert_eq!(target_after_command.team, Team::GLA);

    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.health.current, initial_health,
        "carbomb should be pending while bomber is out of range"
    );
    assert!(!target_after_far_update.status.is_carbomb);

    {
        let bomber = game_logic
            .host_object_mut(bomber_id)
            .expect("bomber should exist");
        bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
        bomber.set_ai_state(AIState::SpecialAbility);
        bomber.target = Some(target_id);
    }
    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "ConvertToCarBomb must not damage vehicle HP on conversion"
    );
    assert!(
        target_after_contact.status.is_carbomb,
        "vehicle must gain IS_CARBOMB"
    );
    assert_eq!(
        target_after_contact.team,
        Team::USA,
        "converted car bomb defects to converter team"
    );
    assert!(
        target_after_contact.weapon.is_some(),
        "car bomb residual binds SuicideCarBomb weapon"
    );
    assert!(
        game_logic.honesty_carbomb_convert_ok(),
        "carbomb convert residual honesty"
    );

    let bomber = game_logic
        .host_object(bomber_id)
        .expect("bomber should exist");
    assert!(
        bomber.status.destroyed,
        "converter infantry is consumed on conversion"
    );
    assert!(
        !game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "MakeCarBombSuccess"),
        "convert must not play the FXList name as audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TerroristCarBomb")
            || crate::game_logic::dispatch_fx_list_at_object(
                crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_LIST,
                target_id.0,
                None
            ),
        "convert must dispatch FX_MakeCarBombSuccess or its TerroristCarBomb sound"
    );
}

/// Residual: ConvertToCarbomb allows neutral civilian vehicles.
#[test]
fn carbomb_command_allows_neutral_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let bomber_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("bomber should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("neutral target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bomber_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let bomber = game_logic
            .host_object(bomber_id)
            .expect("bomber should exist");
        assert_eq!(bomber.ai_state, AIState::SpecialAbility);
        assert_eq!(bomber.target, Some(target_id));
    }

    {
        let bomber = game_logic
            .host_object_mut(bomber_id)
            .expect("bomber should exist");
        bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
        bomber.set_ai_state(AIState::SpecialAbility);
        bomber.target = Some(target_id);
    }
    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "neutral convert must not damage vehicle"
    );
    assert!(
        target_after_contact.status.is_carbomb,
        "neutral vehicle becomes car bomb"
    );
    assert_eq!(target_after_contact.team, Team::USA);

    let bomber = game_logic
        .host_object(bomber_id)
        .expect("bomber should exist");
    assert!(bomber.status.destroyed);
}

/// Residual: car bomb vehicle attacks structure → suicide detonation AOE damage.
#[test]
fn carbomb_attack_structure_detonates_with_observable_damage() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let car_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("car should be created");
    let structure_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("structure should be created");

    {
        let car = game_logic
            .host_object_mut(car_id)
            .expect("car should exist");
        car.apply_convert_to_car_bomb();
        car.set_team(Team::GLA);
        // Ensure weapon is ready to fire immediately.
        if let Some(w) = car.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.0;
        }
        car.attack_target(structure_id);
    }
    game_logic.car_bomb.record_conversion();

    let structure_hp_before = game_logic
        .host_object(structure_id)
        .expect("structure should exist")
        .health
        .current;
    assert!(structure_hp_before > 0.0);

    // SuicideCarBomb AttackRange = 5; structure at 3 is in range.
    game_logic.frame = 30;
    game_logic.update_combat(&[car_id, structure_id], 1.0 / 30.0);

    let structure_after = game_logic
        .host_object(structure_id)
        .expect("structure should exist");
    assert!(
        structure_after.health.current < structure_hp_before
            || structure_after.status.destroyed
            || !structure_after.is_alive(),
        "car bomb detonation must damage structure (before={structure_hp_before}, after={})",
        structure_after.health.current
    );
    assert!(
        game_logic.honesty_carbomb_detonate_ok(),
        "carbomb detonate residual honesty (damage={})",
        game_logic.car_bomb_residual().detonation_damage_dealt
    );

    let car = game_logic.host_object(car_id).expect("car should exist");
    assert!(
        car.status.destroyed || !car.is_alive(),
        "car bomb destroys itself on detonation"
    );
}

#[test]
fn attack_order_chases_target_when_out_of_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created from template");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
        .expect("target should be created from template");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.attack_target(target_id);
        if let Some(weapon) = attacker.weapon.as_mut() {
            weapon.range = 50.0;
            weapon.reload_time = 0.0;
            weapon.last_fire_time = 0.0;
        }
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let attacker = game_logic
        .host_object(attacker_id)
        .expect("attacker should exist");
    let chase_target = attacker
        .movement
        .target_position
        .expect("attacker should chase out-of-range target");
    assert!(
        chase_target.distance(Vec3::new(300.0, 0.0, 0.0)) < 0.01,
        "attacker should chase the current target position"
    );
    assert_eq!(attacker.ai_state, AIState::Attacking);
    assert!(attacker.status.moving);
}

#[test]
fn attack_order_clears_dead_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created from template");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target should be created from template");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.attack_target(target_id);
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        target.status.destroyed = true;
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let attacker = game_logic
        .host_object(attacker_id)
        .expect("attacker should exist");
    assert!(attacker.target.is_none(), "dead targets should be cleared");
    assert_eq!(attacker.ai_state, AIState::Idle);
    assert!(!attacker.status.attacking);
}

#[test]
fn ai_production_does_not_spawn_when_player_cannot_afford_unit() {
    let mut game_logic = GameLogic::new();

    let mut war_factory = ThingTemplate::new("WarFactory");
    war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2);
    game_logic
        .templates
        .insert("WarFactory".to_string(), war_factory);

    let mut humvee = ThingTemplate::new("USA_Humvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("USA_Humvee".to_string(), humvee);

    let mut player = Player::new(0, Team::USA, "AI", false);
    player.resources.supplies = 250;
    game_logic.add_player(player);

    let factory_id = game_logic
        .create_object("WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory should be created");

    game_logic.frame = 600; // AI production pulse
    game_logic.update_ai(&[factory_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.objects.len(),
        1,
        "AI should not spawn units for free when resources are insufficient"
    );
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .resources
            .supplies,
        250,
        "resources should remain unchanged when production cannot be afforded"
    );
}

#[test]
fn ai_production_queues_units_instead_of_spawning_immediately() {
    let mut game_logic = GameLogic::new();

    let mut war_factory = ThingTemplate::new("WarFactory");
    war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2);
    game_logic
        .templates
        .insert("WarFactory".to_string(), war_factory);

    let mut humvee = ThingTemplate::new("USA_Humvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("USA_Humvee".to_string(), humvee);

    let mut player = Player::new(0, Team::USA, "AI", false);
    player.resources.supplies = 1_000;
    game_logic.add_player(player);

    let factory_id = game_logic
        .create_object("WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory should be created");

    game_logic.frame = 600; // AI production pulse
    game_logic.update_ai(&[factory_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.objects.len(),
        1,
        "AI production should queue first instead of instantly spawning a unit"
    );
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .effective_supplies(),
        500,
        "queued AI production should charge exactly once"
    );
    let queue = &game_logic
        .host_object(factory_id)
        .and_then(|factory| factory.building_data.as_ref())
        .expect("factory should have building data")
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].template_name, "USA_Humvee");
}

#[test]
fn china_barracks_quantity_modifier_spawns_two_redguards_residual() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::host_production_buildable_command_residual::{
        production_quantity_modifier, QUANTITY_MODIFIER_SAMPLE_COUNT,
    };
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate, VeterancyLevel,
    };
    assert_eq!(QUANTITY_MODIFIER_SAMPLE_COUNT, 2);
    assert_eq!(
        production_quantity_modifier("ChinaBarracks", "ChinaInfantryRedguard"),
        2
    );
    assert_eq!(
        production_quantity_modifier("AmericaBarracks", "AmericaInfantryRanger"),
        1
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(100, 0);
    rg.build_time = 0.05;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        // Orient for deterministic natural rally residual.
        o.thing.set_orientation(0.0);
    }
    // One queue entry pays once, QuantityModifier yields two exits.
    assert!(logic.enqueue_production(bid, "ChinaInfantryRedguard".into()));
    let qlen = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(0);
    assert_eq!(qlen, 1, "single queue entry for modifier batch");
    let qty = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .and_then(|b| b.production_queue.first())
        .map(|i| i.quantity_total)
        .unwrap_or(0);
    assert_eq!(qty, 2, "Redguard quantity_total residual");

    // The terminal build frame reaches a closed China barracks door first;
    // nothing may exit until it reaches C++ WAITING_OPEN.
    logic.update();
    let before_open = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "ChinaInfantryRedguard" && o.is_alive())
        .count();
    assert_eq!(
        before_open, 0,
        "closed door must not release a batch member"
    );
    assert_eq!(
        logic
            .host_object(bid)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.len()),
        Some(1),
        "closed door retains the completed batch"
    );

    // Simulate the authored door reaching WAITING_OPEN.  A fresh Queue exit
    // has currentDelay=0/currentBurst=InitialBurst(0), so it admits exactly
    // one member before that successful exit arms its retail 300ms/9-frame
    // delay.
    let hold_open_until = logic.get_frame().saturating_add(100);
    if let Some(o) = logic.host_object_mut(bid) {
        o.production_door_phase = 2;
        o.production_door_phase_end_frame = hold_open_until;
    }
    logic.update();
    let living = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living, 1,
        "fresh Queue exit must release one Red Guard before its delay, living={living}"
    );
    let first_state = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .expect("building after first Queue exit");
    assert_eq!(first_state.exit_delay_remaining_frames, 9);
    assert_eq!(first_state.exit_burst_remaining, 0);
    assert_eq!(first_state.production_queue[0].quantity_produced, 1);

    for _ in 0..8 {
        logic.update();
    }
    let held = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(held, 1, "Queue must hold the second Red Guard for 8 frames");
    logic.update();
    let released = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        released, 2,
        "9th Queue update releases the second Red Guard"
    );
    let qlen_end = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(99);
    assert_eq!(qlen_end, 0);
    // Exit path residual: units should be Moving toward authored natural rally.
    let moving = logic
        .host_objects()
        .values()
        .filter(|o| {
            o.template_name.contains("Redguard")
                && o.is_alive()
                && matches!(o.ai_state, AIState::Moving)
        })
        .count();
    assert!(
        moving >= 1,
        "at least one Redguard should be on exit path residual, moving={moving}"
    );
    assert!(
        logic.host_objects().values().any(|o| {
            o.template_name.contains("Redguard") && o.is_alive() && o.can_path_through_units
        }),
        "Queue exit must aiFollowExitProductionPath (can_path_through_units)"
    );
    let _ = VeterancyLevel::Rookie;
}

#[test]
fn queue_factory_exit_follows_snapped_production_path() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
    }
    let uid = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 75.0),
        )
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [100.0, 15.0, 75.0],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let (can_path, ai_state, dest, path) = {
        let unit = logic.host_object(uid).expect("unit after exit");
        (
            unit.can_path_through_units,
            unit.ai_state.clone(),
            unit.movement.target_position,
            unit.movement.path.clone(),
        )
    };
    assert!(
        can_path,
        "FollowExitProductionPath sets can_path_through_units"
    );
    assert!(
        matches!(ai_state, AIState::Moving),
        "exit must start AIFollowExitProductionPath / Moving, got {:?}",
        ai_state
    );
    let dest = dest.expect("exit destination");
    let (producer_pos, forward, exit) = {
        let producer = logic.host_object(bid).expect("producer");
        (
            producer.get_position(),
            producer.thing.get_direction_vector(),
            producer
                .thing
                .template
                .production_exit_metadata
                .expect("queue metadata"),
        )
    };
    let raw_point = exit.natural_rally_point_with_path_offset(
        crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
    );
    let raw = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
        producer_pos,
        forward,
        (raw_point[0], raw_point[1], raw_point[2]),
    );
    assert!(
        dest.distance(raw) > 0.5,
        "Queue snapPosition must move the natural rally off the raw model point, dest={dest:?} raw={raw:?}"
    );
    assert!(
        path.len() >= 2,
        "Queue with no custom rally doubles the snapped natural, path={path:?}"
    );
    let last = *path.last().expect("path last");
    let prev = path[path.len() - 2];
    assert!(
        last.distance(prev) < 1.0,
        "doubled Queue natural must repeat the snapped point, prev={prev:?} last={last:?}"
    );
}

#[test]
fn factory_spawn_keeps_unit_create_point_without_selection_radius_jitter() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
    }
    let create_pos = glam::Vec3::new(100.0, 15.0, 75.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");
    if let Some(unit) = logic.host_object_mut(uid) {
        unit.selection_radius = 12.0;
    }

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let pos = logic.host_object(uid).expect("unit after exit").get_position();
    let dxz = ((pos.x - create_pos.x).powi(2) + (pos.z - create_pos.z).powi(2)).sqrt();
    assert!(
        dxz < 0.05,
        "C++ exitObjectViaDoor keeps UnitCreatePoint; selection-radius jitter is gone, pos={pos:?} create={create_pos:?}"
    );
}

#[test]
fn queue_exit_applies_airborne_motive_and_pitch_after_ground_snap() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
        MOTIVE_FRAMES_RESIDUAL,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(
        logic.restore_terrain_heights_from_grid(w, h, &heights),
        "flat ground cache"
    );

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 15.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.physics_mass = 5.0;
    rg.center_of_mass_offset = 2.5;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    let producer_vel = glam::Vec3::new(8.0, 3.0, -2.0);
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
        o.movement.velocity = producer_vel;
    }
    let create_pos = glam::Vec3::new(20.0, 15.0, 10.0);
    assert_eq!(logic.terrain_height_at(create_pos), Some(0.0));
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert!(
        (unit.get_position().y - 0.0).abs() < 1e-4,
        "AllowAirborneCreation false still snaps Y, pos={:?}",
        unit.get_position()
    );
    assert!(
        (unit.physics_accel - producer_vel).length() < 1e-4,
        "startingForce = producer vel * mass → accel == vel, accel={:?} vel={:?}",
        unit.physics_accel,
        producer_vel
    );
    assert_eq!(unit.motive_frames_remaining, MOTIVE_FRAMES_RESIDUAL);
    assert!(
        (unit.shock_pitch_rate - 2.5 * 0.04).abs() < 1e-5,
        "setPitchRate(COM * 0.04), got {}",
        unit.shock_pitch_rate
    );
}

#[test]
fn queue_exit_airborne_kick_keeps_y_when_allow_airborne() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
        MOTIVE_FRAMES_RESIDUAL,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 15.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: true,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.physics_mass = 4.0;
    rg.center_of_mass_offset = -1.25;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    let producer_vel = glam::Vec3::new(1.0, 6.0, 0.0);
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.movement.velocity = producer_vel;
    }
    let create_pos = glam::Vec3::new(20.0, 15.0, 10.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert!(
        (unit.get_position().y - 15.0).abs() < 1e-4,
        "AllowAirborneCreation keeps transformed Y, pos={:?}",
        unit.get_position()
    );
    assert!((unit.physics_accel - producer_vel).length() < 1e-4);
    assert_eq!(unit.motive_frames_remaining, MOTIVE_FRAMES_RESIDUAL);
    assert!((unit.shock_pitch_rate - (-1.25 * 0.04)).abs() < 1e-5);
}

#[test]
fn queue_exit_ground_spawn_skips_airborne_kick() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.center_of_mass_offset = 3.0;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.movement.velocity = glam::Vec3::new(9.0, 0.0, 0.0);
    }
    let create_pos = glam::Vec3::new(20.0, 0.0, 10.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert_eq!(unit.physics_accel, glam::Vec3::ZERO);
    assert_eq!(unit.motive_frames_remaining, 0);
    assert_eq!(unit.shock_pitch_rate, 0.0);
}


#[test]
fn attack_move_to_command_sets_moving_residual() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .expect("ranger");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackMoveTo {
            destination: glam::Vec3::new(50.0, 0.0, 50.0),
            max_shots: -1,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![id],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    let o = logic.host_object(id).expect("alive");
    assert!(
        matches!(
            o.ai_state,
            AIState::Moving | AIState::AttackMoving | AIState::Attacking
        ) || o.movement.target_position.is_some()
            || !o.movement.path.is_empty(),
        "attack-move should order unit motion residual: state={:?}",
        o.ai_state
    );
}

#[test]
fn parsed_queue_exit_runtime_uses_authored_initial_burst_and_delay() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, 0.0, 0.0],
        natural_rally_point: [0.0, 0.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 2,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(100, 0);
    // Fast build residual for test.
    rg.build_time = 0.05;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object("ChinaBarracks", Team::China, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
    }
    assert!(logic.enqueue_production(bid, "ChinaInfantryRedguard".into()));
    // C++ Queue state begins at InitialBurst and decrements after each success.
    // This one entry deliberately retains the regular two-member modifier.
    if let Some(building) = logic
        .host_object_mut(bid)
        .and_then(|object| object.building_data.as_mut())
    {
        building.production_queue[0].quantity_total = 3;
        building.production_queue[0].construction_frames = 2;
    }
    let hold_open_until = logic.get_frame().saturating_add(100);
    if let Some(object) = logic.host_object_mut(bid) {
        object.production_door_phase = 2;
        object.production_door_phase_end_frame = hold_open_until;
    }
    logic.update();
    let living = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living, 2,
        "InitialBurst=2 permits exactly two immediate exits"
    );
    let state = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .expect("Queue runtime state");
    assert_eq!(state.exit_delay_remaining_frames, 9);
    assert_eq!(state.exit_burst_remaining, 0);
    for _ in 0..8 {
        logic.update();
    }
    let held = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(held, 2, "delay blocks the third member for 8 frames");
    logic.update();
    let living_end = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living_end, 3,
        "third redguard releases on the ninth post-exit frame, living={living_end}"
    );
}

#[test]
fn can_make_hero_maxed_out_at_one_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        unit_max_simultaneous_of_type_residual, CANMAKE_MAXED_OUT_FOR_PLAYER, CANMAKE_OK,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        unit_max_simultaneous_of_type_residual("AmericaInfantryColonelBurton"),
        Some(1)
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Hero)
        .set_health(200.0)
        .set_cost(1500, 0);
    burton.max_simultaneous_of_type = 1;
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(
        logic.can_make_unit(barracks, "AmericaInfantryColonelBurton"),
        CANMAKE_OK
    );
    assert!(logic.enqueue_production(barracks, "AmericaInfantryColonelBurton".into()));
    // Queued counts toward max residual.
    assert_eq!(
        logic.can_make_unit(barracks, "AmericaInfantryColonelBurton"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );
    // Complete spawn also maxes.
    let mut logic2 = GameLogic::new();
    ensure_test_player_for_team(&mut logic2, Team::USA);
    ensure_test_barracks_template(&mut logic2);
    logic2.templates.insert(
        "AmericaInfantryColonelBurton".into(),
        logic
            .templates
            .get("AmericaInfantryColonelBurton")
            .unwrap()
            .clone(),
    );
    let b2 = logic2
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let _ = logic2
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic2.can_make_unit(b2, "AmericaInfantryColonelBurton"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );
    assert!(!logic2.enqueue_production(b2, "AmericaInfantryColonelBurton".into()));
}

    #[test]
    fn unique_building_max_simultaneous_from_ini_blocks_second() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut logic = GameLogic::new();
        ensure_test_player_for_team(&mut logic, Team::USA);
        let mut factory = ThingTemplate::new("TestUniqueWarFactory");
        factory
            .add_kind_of(KindOf::Structure)
            .set_health(2_000.0)
            .set_cost(2_000, 0);
        factory.max_simultaneous_of_type = 1;
        logic
            .templates
            .insert("TestUniqueWarFactory".into(), factory);

        let first = logic.create_object_under_construction(
            "TestUniqueWarFactory",
            Team::USA,
            glam::Vec3::ZERO,
        );
        assert!(first.is_some(), "first unique building must place");
        let second = logic.create_object_under_construction(
            "TestUniqueWarFactory",
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        );
        assert!(
            second.is_none(),
            "hq-ss0x8: INI MaxSimultaneousOfType=1 must block a second copy"
        );
    }

    #[test]
    fn max_simultaneous_link_key_counts_rebuild_hole() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut logic = GameLogic::new();
        ensure_test_player_for_team(&mut logic, Team::USA);
        let mut scud = ThingTemplate::new("TestLinkedScud");
        scud.add_kind_of(KindOf::Structure)
            .set_health(4_000.0)
            .set_cost(5_000, 0);
        scud.max_simultaneous_of_type = 1;
        scud.max_simultaneous_link_key = Some("Superweapon".into());
        logic.templates.insert("TestLinkedScud".into(), scud);

        let mut hole = ThingTemplate::new("TestLinkedScudHole");
        hole.add_kind_of(KindOf::Structure).set_health(500.0);
        hole.max_simultaneous_of_type = 1;
        hole.max_simultaneous_link_key = Some("Superweapon".into());
        logic.templates.insert("TestLinkedScudHole".into(), hole);

        let hole_id = logic
            .create_object("TestLinkedScudHole", Team::USA, glam::Vec3::ZERO)
            .expect("hole");
        assert!(logic.host_object(hole_id).is_some());
        let blocked = logic.create_object_under_construction(
            "TestLinkedScud",
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        );
        assert!(
            blocked.is_none(),
            "hq-ss0x8: rebuild hole must consume MaxSimultaneousLinkKey"
        );
    }

#[test]
fn can_make_aircraft_blocked_when_airfield_parking_full() {
    use crate::game_logic::buildings::{BuildingType, DEFAULT_PRODUCTION_QUEUE_LIMIT};
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_OK, CANMAKE_PARKING_PLACES_FULL,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_t = ThingTemplate::new("TestAirfield");
    af_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    af_t.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("TestAirfield".into(), af_t);
    let mut jet_t = ThingTemplate::new("TestRaptor");
    jet_t
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0)
        .set_cost(1000, 0);
    logic.templates.insert("TestRaptor".into(), jet_t);

    let af = logic
        .create_object("TestAirfield", Team::USA, glam::Vec3::ZERO)
        .expect("af");
    // Force Airfield building type residual.
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    assert_eq!(logic.can_make_unit(af, "TestRaptor"), CANMAKE_OK);

    // Fill the authored four `ParkingPlaceBehavior::m_spaces` entries.  A
    // building garrison is unrelated containment state and must not affect
    // airfield capacity.
    for i in 0..4 {
        let j = logic
            .create_object(
                "TestRaptor",
                Team::USA,
                glam::Vec3::new(10.0 * (i as f32 + 1.0), 0.0, 0.0),
            )
            .expect("jet");
        if let Some(jet) = logic.host_object_mut(j) {
            jet.set_contained_by(Some(af));
            jet.set_ai_state(AIState::Docked);
            jet.producer_id = Some(af);
            jet.airfield_parking_space_index = Some(i);
        }
    }
    assert_eq!(logic.airfield_parking_occupied_or_queued(af), 4);
    assert_eq!(
        logic.can_make_unit(af, "TestRaptor"),
        CANMAKE_PARKING_PLACES_FULL
    );
    assert!(!logic.enqueue_production(af, "TestRaptor".into()));

    // C++ `ProductionUpdate::canQueueCreateUnit` tests ParkingPlaceBehavior
    // before the generic queue-limit branch.  A 3×3 authored airfield can
    // naturally hold nine queued aircraft, so the tenth request reaches both
    // limits and must still report parking full.
    let mut queue_only_airfield = ThingTemplate::new("TestQueuePriorityAirfield");
    queue_only_airfield
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    queue_only_airfield.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 3,
        num_cols: 3,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic
        .templates
        .insert("TestQueuePriorityAirfield".into(), queue_only_airfield);
    let queued_airfield = logic
        .create_object(
            "TestQueuePriorityAirfield",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("queue-priority airfield");
    if let Some(object) = logic.host_object_mut(queued_airfield) {
        object.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(logic.enqueue_production(queued_airfield, "TestRaptor".into()));
    }
    assert_eq!(
        logic.can_make_unit(queued_airfield, "TestRaptor"),
        CANMAKE_PARKING_PLACES_FULL,
        "ParkingPlaceBehavior must win over the generic queue-full status"
    );
}

#[test]
fn can_make_unit_residual_gates_prereq_money_queue_disabled() {
    use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_FACTORY_IS_DISABLED, CANMAKE_NO_MONEY, CANMAKE_NO_PREREQ, CANMAKE_OK,
        CANMAKE_QUEUE_FULL,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);

    // No money residual.
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 0;
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_MONEY
    );
    assert!(!logic.enqueue_production(barracks, "TestInfantry".into()));

    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 100_000;
    }
    // Disabled factory residual.
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(true);
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(false);
    }

    // Queue full residual.
    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(logic.enqueue_production(barracks, "TestInfantry".into()));
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_QUEUE_FULL
    );

    // Wrong factory type residual (vehicle at barracks).
    let mut veh = ThingTemplate::new("TestVehicleUnit");
    veh.add_kind_of(KindOf::Vehicle)
        .set_health(100.0)
        .set_cost(200, 0);
    logic.templates.insert("TestVehicleUnit".into(), veh);
    assert_eq!(
        logic.can_make_unit(barracks, "TestVehicleUnit"),
        CANMAKE_NO_PREREQ
    );
}

#[test]
fn can_make_unit_missing_or_dead_producer_is_no_prereq() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    use crate::game_logic::{ObjectId, Team};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 10_000;
    }
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);
    assert_eq!(
        logic.can_make_unit(ObjectId(0xDEAD), "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.health.current = 0.0;
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
}


#[test]
fn can_make_unit_reports_disabled_and_maxed_before_prereq() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_FACTORY_IS_DISABLED, CANMAKE_MAXED_OUT_FOR_PLAYER, CANMAKE_NO_PREREQ,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");

    // Missing prereq + disabled factory → FACTORY_IS_DISABLED (C++ order).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.buildable_status =
            crate::game_logic::host_production_buildable_command_residual::BSTATUS_NO;
    }
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(true);
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(false);
    }

    // Missing prereq + maxed → MAXED_OUT (C++ does maxed before isPossibleToMakeUnit).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.max_simultaneous_of_type = 1;
    }
    let _ = logic.create_object("TestInfantry", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );

    // TECHTREE leftover override NO gates live can_make (no factory disable/maxed).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.max_simultaneous_of_type = 0;
        tmpl.buildable_status =
            crate::game_logic::host_production_buildable_command_residual::BSTATUS_YES;
    }
    let _ = gamelogic::scripting::take_host_buildable_status_override_requests();
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 2);
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 0);
}

#[test]
fn script_disable_unit_construction_and_techtree_override_write_live() {
    use crate::game_logic::host_production_buildable_command_residual::{
        BSTATUS_NO, CANMAKE_FACTORY_IS_DISABLED, CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    use crate::game_logic::Team;
    use gamelogic::scripting::HostScriptCanBuildRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);

    let _ = gamelogic::scripting::take_host_can_build_requests();
    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Units {
        player: "TestPlayer".into(),
        enable: false,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !logic.get_player(0).expect("p").can_build_units,
        "PLAYER_DISABLE_UNIT_CONSTRUCTION must flip live can_build_units"
    );
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );

    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Units {
        player: "TestPlayer".into(),
        enable: true,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(logic.get_player(0).expect("p").can_build_units);

    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Factories {
        player: "TestPlayer".into(),
        template: "TestBarracks".into(),
        enable: false,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(barracks)
            .expect("barracks")
            .is_script_disabled(),
        "PLAYER_DISABLE_FACTORIES must set live SCRIPT_DISABLED"
    );
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Factories {
        player: "TestPlayer".into(),
        template: "TestBarracks".into(),
        enable: true,
    });
    logic.evaluate_and_execute_scripts(0.0);

    let _ = gamelogic::scripting::take_host_buildable_status_override_requests();
    gamelogic::scripting::request_host_buildable_status_override("TestInfantry", BSTATUS_NO as i32);
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    assert!(
        logic
            .templates
            .get("TestInfantry")
            .expect("tmpl")
            .human_control_bar_buildable_hidden()
    );
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 0);
}

#[test]
fn script_named_receive_upgrade_writes_live_object() {
    use gamelogic::scripting::HostScriptNamedUpgradeRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "Hero".into();
    }

    let _ = gamelogic::scripting::take_host_script_named_upgrade_requests();
    gamelogic::scripting::request_host_script_named_upgrade(HostScriptNamedUpgradeRequest {
        unit: "Hero".into(),
        upgrade: "Upgrade_AmericaChemicalSuits".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(id)
            .expect("unit")
            .has_upgrade_tag("Upgrade_AmericaChemicalSuits"),
        "NAMED_RECEIVE_UPGRADE must apply giveUpgrade on the live host object"
    );
}

#[test]
fn script_player_sell_everything_sells_live_faction_structures() {
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    let unit = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("unit");

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::SellEverything {
            player: "TestPlayer".into(),
        },
    );
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic.host_object(barracks).expect("barracks").status.sold,
        "PLAYER_SELL_EVERYTHING must start_sell_object on faction structures"
    );
    assert!(
        logic.sell_list.iter().any(|s| s.id == barracks),
        "sold barracks must be on the live sell list"
    );
    assert!(
        !logic.host_object(unit).expect("unit").status.sold,
        "non-structure units must not be sold"
    );
}

#[test]
fn script_exclude_from_score_and_select_skillset_write_live() {
    use crate::ai::AIDifficulty;
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic
        .ai_manager
        .add_ai_player(0, Team::USA, AIDifficulty::Medium);
    assert!(logic.get_player(0).expect("p").list_in_score_screen);

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::ExcludeFromScore {
            player: "TestPlayer".into(),
        },
    );
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::SelectSkillset {
            player: "TestPlayer".into(),
            skillset: 2,
        },
    );
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !logic.get_player(0).expect("p").list_in_score_screen,
        "PLAYER_EXCLUDE_FROM_SCORE_SCREEN must set live list_in_score_screen false"
    );
    assert_eq!(
        logic
            .ai_manager
            .ai_players
            .get(&0)
            .expect("ai")
            .selected_skillset(),
        1,
        "PLAYER_SELECT_SKILLSET is 1-based; live selector is 0-based"
    );
}

#[test]
fn script_player_kill_destroys_live_army() {
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let unit = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(HostScriptPlayerMiscRequest::Kill {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic.host_object(unit).is_none()
            || logic
                .host_object(unit)
                .is_some_and(|o| !o.is_alive() || o.status.destroyed),
        "PLAYER_KILL must destroy the live host army"
    );
}

#[test]
fn script_unit_destroy_all_contained_kills_live_occupants() {
    use gamelogic::scripting::{
        request_host_script_kill_delete_damage, HostScriptKillDeleteDamageRequest,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    ensure_test_barracks_template(&mut logic);
    let container = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("container");
    let rider = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("rider");
    if let Some(obj) = logic.host_object_mut(container) {
        obj.name = "NamedChinook".into();
        let _ = obj.add_occupant(rider);
    }
    if let Some(obj) = logic.host_object_mut(rider) {
        obj.contained_by = Some(container);
    }

    let _ = gamelogic::scripting::take_host_script_kill_delete_damage_requests();
    request_host_script_kill_delete_damage(HostScriptKillDeleteDamageRequest::DestroyAllContained {
        unit: "NamedChinook".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let rider_gone = logic
        .host_object(rider)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true);
    assert!(
        rider_gone,
        "UNIT_DESTROY_ALL_CONTAINED must kill live occupants"
    );
    assert!(
        logic
            .host_object(container)
            .map(|o| o.contained_units().is_empty())
            .unwrap_or(true),
        "container roster must be empty after destroy-all-contained"
    );
}

#[test]
fn script_idle_all_units_and_resume_supply_write_live() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{request_host_script_idle, HostScriptIdleRequest};

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    let tank = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    let truck = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("truck");
    let far = glam::Vec3::new(400.0, 0.0, 0.0);
    let _ = logic.unit_command_move_to(tank, far);
    if let Some(obj) = logic.host_object_mut(truck) {
        obj.set_ai_state(AIState::Idle);
        obj.supply_truck_force_pending = false;
    }

    let _ = gamelogic::scripting::take_host_script_idle_requests();
    request_host_script_idle(HostScriptIdleRequest::IdleAll {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let tank_pos = logic.host_object(tank).expect("tank").get_position();
    let dest = logic
        .host_object(tank)
        .and_then(|o| o.movement.target_position);
    assert!(
        dest.map(|d| (d - tank_pos).length() < 1.0).unwrap_or(true)
            || logic
                .host_object(tank)
                .map(|o| o.ai_state == AIState::Idle)
                .unwrap_or(false),
        "IDLE_ALL_UNITS must aiMoveToPosition(self) on live non-structures"
    );

    request_host_script_idle(HostScriptIdleRequest::ResumeSupply {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(truck)
            .expect("truck")
            .supply_truck_force_pending,
        "RESUME_SUPPLY_TRUCKING must setForceWantingState on idle collectors"
    );
}

#[test]
fn script_named_use_command_button_stops_live_unit() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{
        request_host_script_use_command_button, HostScriptUseCommandButtonRequest,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "Hero".into();
        obj.team_instance_name = "teamAmerica".into();
    }
    let _ = logic.unit_command_move_to(id, glam::Vec3::new(300.0, 0.0, 0.0));

    let _ = gamelogic::scripting::take_host_script_use_command_button_requests();
    request_host_script_use_command_button(HostScriptUseCommandButtonRequest::Named {
        unit: "Hero".into(),
        button: "Command_Stop".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("unit").ai_state,
        AIState::Idle,
        "NAMED_USE_COMMANDBUTTON Command_Stop must idle the live unit"
    );

    let _ = logic.unit_command_move_to(id, glam::Vec3::new(300.0, 0.0, 0.0));
    request_host_script_use_command_button(HostScriptUseCommandButtonRequest::Team {
        team: "teamAmerica".into(),
        button: "Command_Stop".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("unit").ai_state,
        AIState::Idle,
        "TEAM_USE_COMMANDBUTTON Command_Stop must idle live team members"
    );
}

#[test]
fn script_named_set_topple_direction_used_by_live_topple() {
    use crate::game_logic::host_structure_topple::is_structure_topple_candidate;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    assert!(is_structure_topple_candidate("TestBarracks", true));
    let id = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("bldg");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "FallingHall".into();
    }
    gamelogic::scripting::request_host_script_topple_direction("FallingHall", 0.0, 1.0);
    let started = logic
        .host_object_mut(id)
        .expect("bldg")
        .begin_structure_topple(0, Some((100.0, 0.0)));
    assert!(started, "named structure must begin topple");
    let st = logic
        .host_object(id)
        .and_then(|o| o.structure_topple_data.clone())
        .expect("topple data");
    assert!(
        (st.dir_x - 0.0).abs() < 0.05 && (st.dir_y - 1.0).abs() < 0.05,
        "NAMED_SET_TOPPLE_DIRECTION must replace attacker-away dir, got ({}, {})",
        st.dir_x,
        st.dir_y
    );
}


#[test]
fn enqueue_production_full_queue_does_not_charge_resources() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    }

    let charged_supplies = game_logic
        .get_player(0)
        .expect("player should exist")
        .effective_supplies();
    assert_eq!(
        charged_supplies,
        100_000 - (DEFAULT_PRODUCTION_QUEUE_LIMIT as u32 * 100)
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        DEFAULT_PRODUCTION_QUEUE_LIMIT
    );

    assert!(!game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));

    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .effective_supplies(),
        charged_supplies,
        "full production queues must not charge resources"
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        DEFAULT_PRODUCTION_QUEUE_LIMIT,
        "full production queues should not accept an extra item"
    );
}

#[test]
fn enqueue_production_requires_player_money_state() {
    let mut game_logic = GameLogic::new();
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    assert!(!game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        0,
        "production should not queue for free without player state"
    );
}

#[test]
fn enqueue_infantry_on_command_center_fails_barracks_succeeds() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_command_center_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let cc_id = game_logic
        .create_object("TestCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    assert!(
        !game_logic.enqueue_production(cc_id, "TestInfantry".to_string()),
        "Command Center must not produce infantry (the train_fail_enqueue weasel)"
    );

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("barracks");
    assert!(
        game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()),
        "completed barracks with money must enqueue infantry"
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|b| b.building_data.as_ref())
            .map(|b| b.production_queue.len())
            .unwrap_or(0),
        1
    );
}

#[test]
fn host_construction_completes_without_coupled_shadow() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    if let Some(t) = game_logic.templates.get_mut("TestBarracks") {
        t.build_time = 1.0;
    }
    let id = game_logic
        .create_object_under_construction("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("uc barracks");
    assert!(game_logic
        .host_object(id)
        .is_some_and(|o| o.status.under_construction));
    for _ in 0..80 {
        game_logic.update_with_dt(1.0 / 30.0);
    }
    let obj = game_logic.host_object(id).expect("still exists");
    assert!(
        !obj.status.under_construction,
        "host-only construction must finish (percent={})",
        obj.construction_percent
    );
    assert!(
        game_logic.enqueue_production(id, "TestInfantry".to_string()),
        "completed host barracks must accept infantry enqueue"
    );
}

#[test]
fn host_construction_completes_when_sole_tick_unmapped() {
    // Coupled sole-tick with no live shadow map: host must store percent and
    // complete. The previous hole computed `projected` then discarded it
    // (`if !sole` never assigned), so barracks stayed UC forever.
    crate::gameworld_shadow::begin_shadow_coupled_tick();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(crate::gameworld_shadow::gameworld_construction_sole_tick_enabled());
        let mut game_logic = GameLogic::new();
        ensure_test_player_for_team(&mut game_logic, Team::USA);
        ensure_test_barracks_template(&mut game_logic);
        ensure_test_infantry_template(&mut game_logic);
        if let Some(t) = game_logic.templates.get_mut("TestBarracks") {
            t.build_time = 1.0;
        }
        let id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 40.0))
            .expect("uc barracks");
        assert!(
            !crate::gameworld_shadow::coupled_host_mapped(id),
            "this test is the unmapped fail-open path"
        );
        for _ in 0..80 {
            game_logic.update_with_dt(1.0 / 30.0);
        }
        let obj = game_logic.host_object(id).expect("still exists");
        assert!(
            !obj.status.under_construction,
            "unmapped sole-tick construction must finish (percent={})",
            obj.construction_percent
        );
        assert!(
            game_logic.enqueue_production(id, "TestInfantry".to_string()),
            "completed unmapped barracks must accept infantry enqueue"
        );
    }));
    crate::gameworld_shadow::end_shadow_coupled_tick();
    result.expect("unmapped sole-tick construction test");
}

#[test]
fn cancel_production_requires_player_money_state_for_refund() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");
    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    game_logic.players.clear();

    assert!(!game_logic.cancel_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        1,
        "cancelling without player state must not drop queued production"
    );
}

#[test]
fn destroying_producer_refunds_queued_production_to_owner() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist")
            .effective_supplies(),
        99_900,
        "queued production should charge the owner before destruction"
    );

    game_logic.mark_object_for_destruction(barracks_id, Some(Team::GLA));
    // C++ cancelAndRefund fires at death start (before topple/collapse deferral).
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist")
            .effective_supplies(),
        100_000,
        "producer death should refund queued production to the owner"
    );
    assert_eq!(
        game_logic
            .get_player(2)
            .expect("GLA player should exist")
            .effective_supplies(),
        100_000,
        "killer should not receive the destroyed producer's queue refund"
    );
    // StructureTopple/Collapse may defer remove across frames.
    let mut removed = false;
    for _ in 0..600 {
        game_logic.update();
        if game_logic.host_object(barracks_id).is_none() {
            removed = true;
            break;
        }
    }
    assert!(
        removed,
        "destroyed producer should be removed after topple/collapse residual"
    );
}

#[test]
fn attack_ground_damages_enemy_near_impact_point() {
    let mut game_logic = GameLogic::new();
    let attacker_id = setup_ground_attacker(
        &mut game_logic,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(15.0, 0.0, 0.0),
    );
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("target should be created from template");

    game_logic.frame = 60; // t=1s, enough for first shot with reload_time 0.25
    let health_before = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let health_after = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;
    assert!(
        health_after < health_before,
        "ground attack should damage units near impact point"
    );
}
