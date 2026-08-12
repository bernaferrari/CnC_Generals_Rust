//! Host GameLogic tests — `strategy_and_stealth`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn attack_ground_advances_reload_without_victim() {
    let mut game_logic = GameLogic::new();
    let attacker_id = setup_ground_attacker(
        &mut game_logic,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(20.0, 0.0, 0.0),
    );

    game_logic.frame = 60; // t=1s
    let last_fire_before = game_logic
        .host_object(attacker_id)
        .and_then(|obj| obj.weapon.as_ref())
        .map(|weapon| weapon.last_fire_time)
        .unwrap_or_default();

    game_logic.update_combat(&[attacker_id], 1.0 / 60.0);

    let last_fire_after = game_logic
        .host_object(attacker_id)
        .and_then(|obj| obj.weapon.as_ref())
        .map(|weapon| weapon.last_fire_time)
        .unwrap_or_default();
    assert!(
        last_fire_after > last_fire_before,
        "ground attack should consume a shot even when no unit is hit"
    );
}

#[test]
fn force_attack_ground_can_damage_friendlies() {
    let mut game_logic = GameLogic::new();
    let attacker_id = setup_ground_attacker(
        &mut game_logic,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(15.0, 0.0, 0.0),
    );
    let friendly_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(15.0, 0.0, 0.0))
        .expect("friendly should be created from template");

    game_logic.frame = 60; // t=1s
    let health_before = game_logic
        .host_object(friendly_id)
        .expect("friendly should exist")
        .health
        .current;

    game_logic.update_combat(&[attacker_id, friendly_id], 1.0 / 60.0);

    let health_after = game_logic
        .host_object(friendly_id)
        .expect("friendly should exist")
        .health
        .current;
    assert!(
        health_after < health_before,
        "forced ground attack should allow friendly fire like classic force-fire behavior"
    );
}

#[test]
fn camera_mod_final_look_toward_uses_remaining_script_camera_time() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.start_camera_move_to(CameraMoveToRequest {
        position: Vec3::new(200.0, 0.0, 120.0),
        seconds: 4.0,
        camera_stutter_seconds: 0.0,
        ease_in_seconds: 0.0,
        ease_out_seconds: 0.0,
    });
    game_logic
        .mission_scripts
        .push_camera_mod_final_look_toward(CameraModFinalLookTowardRequest {
            position: Vec3::new(300.0, 0.0, 220.0),
        });

    game_logic.evaluate_and_execute_scripts(0.0);

    let look = game_logic
        .take_camera_look_toward_request()
        .expect("mod final look toward should enqueue a look request");
    assert_eq!(look.position, Vec3::new(300.0, 0.0, 220.0));
    assert!(
        (look.duration_seconds - 4.0).abs() < 0.001,
        "mod final look should use remaining camera movement time"
    );
    assert_eq!(look.ease_in_seconds, 0.0);
    assert_eq!(look.ease_out_seconds, 0.0);
}

#[test]
fn camera_mod_look_toward_is_immediate_request() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_look_toward(CameraModLookTowardRequest {
            position: Vec3::new(150.0, 0.0, 50.0),
        });

    game_logic.evaluate_and_execute_scripts(0.0);

    let look = game_logic
        .take_camera_look_toward_request()
        .expect("mod look toward should enqueue look request");
    assert_eq!(look.position, Vec3::new(150.0, 0.0, 50.0));
    assert_eq!(look.duration_seconds, 0.0);
    assert!(!look.reverse_rotation);
}

#[test]
fn camera_mod_freeze_time_applies_to_next_script_camera_move() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.mission_scripts.push_camera_mod_freeze_time();
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !game_logic.is_script_camera_time_frozen(),
        "freeze time should arm until a scripted camera move starts"
    );

    game_logic
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(200.0, 0.0, 120.0),
            seconds: 3.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.is_script_camera_time_frozen(),
        "freeze time should be active during scripted camera movement"
    );

    for _ in 0..240 {
        game_logic.update_script_camera(1.0 / 60.0);
    }
    assert!(
        !game_logic.is_script_camera_time_frozen(),
        "freeze time should clear once scripted camera movement ends"
    );
}

#[test]
fn camera_mod_freeze_time_marks_simulation_as_frozen() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    assert!(!game_logic.is_time_frozen_for_simulation());

    game_logic.mission_scripts.push_camera_mod_freeze_time();
    game_logic
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(120.0, 0.0, 60.0),
            seconds: 2.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(game_logic.is_script_camera_time_frozen());
    assert!(game_logic.is_time_frozen_for_simulation());
}

#[test]
fn post_ai_commands_flushed_inside_game_logic() {
    // Structural: AI-phase command flush lives in update_simulation after update_ai.
    let src = include_str!("../game_logic.rs");
    let ai = src
        .find("self.update_ai(&object_ids, dt);")
        .expect("update_ai call");
    let mgr = src
        .find("ai_mgr.update(self, sim_time);")
        .expect("ai_mgr update");
    // Second process_commands after AI manager (phase 8b)
    let flush = src[mgr..]
        .find("self.process_commands();")
        .map(|o| mgr + o)
        .expect("post-AI flush");
    assert!(
        ai < mgr && mgr < flush,
        "process_commands must follow AI update; ai={ai} mgr={mgr} flush={flush}"
    );
    // Early command processing (phase 5) still exists before object updates.
    let early = src
        .find("self.process_commands();")
        .expect("early process_commands");
    assert!(
        early < ai,
        "phase-5 process_commands must precede update_ai"
    );
}

#[test]
fn projectiles_step_inside_game_logic_update() {
    // Engine mid-frame dual CombatSystem removed; drain+step is in update_simulation.
    let mut logic = GameLogic::new();
    let shooter = ObjectId(1);
    let target = ObjectId(2);
    let mut s = Object::new_simple(
        shooter,
        crate::game_logic::ObjectType::Infantry,
        "AmericaRanger".to_string(),
    );
    s.set_position(glam::Vec3::ZERO);
    let mut t = Object::new_simple(
        target,
        crate::game_logic::ObjectType::Infantry,
        "GLARebel".to_string(),
    );
    t.set_position(glam::Vec3::new(5.0, 0.0, 0.0));
    let hp_before = t.health.current;
    logic.objects.insert(shooter, s);
    logic.objects.insert(target, t);

    crate::game_logic::combat::queue_projectile(crate::game_logic::combat::PendingProjectile {
        shooter_id: shooter,
        shooter_pos: glam::Vec3::ZERO,
        source_context: None,
        target_id: Some(target),
        target_pos: Some(glam::Vec3::new(5.0, 0.0, 0.0)),
        damage: 25.0,
        speed: 1000.0,
        splash_radius: 0.0,
        is_homing: false,
        damage_type: crate::game_logic::combat::DamageType::Bullet,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        projectile_object_name: String::new(),
        projectile_lifecycle: None,
        fire_fx_name: String::new(),
        fire_ocl_name: String::new(),
        detonation_fx_name: String::new(),
        detonation_ocl_name: String::new(),
        exhaust_name: String::new(),
        secondary_damage: 0.0,
        secondary_damage_radius: 0.0,
        shock_wave_amount: 0.0,
        shock_wave_radius: 0.0,
        shock_wave_taper_off: 0.0,
        radius_damage_affects:
            crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
        projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
        scatter_radius: 0.0,
        min_weapon_speed: 0.0,
        scale_weapon_speed: false,
        attack_range: 0.0,
        min_attack_range: 0.0,
        historic_weapon_key: String::new(),
        historic_bonus_time_frames: 0,
        historic_bonus_count: 0,
        historic_bonus_radius: 0.0,
        historic_bonus_weapon: String::new(),
        die_on_detonate: false,
    });

    // One fixed step runs drain + projectile update.
    logic.update_simulation(LOGIC_FRAME_TIMESTEP);

    let snapped = logic.combat_system().projectile_count();
    let hp_after = logic
        .objects
        .get(&target)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // Either still in-flight on host combat system or already applied damage.
    assert!(
        snapped > 0 || hp_after < hp_before,
        "expected projectile retained or damage applied; count={snapped} hp {hp_before}->{hp_after}"
    );
}

#[test]
fn path_follow_owned_by_update_movement_single_step() {
    // Engine mid-frame PathfindingSystem::move_unit_along_path was removed.
    // Path following is sole ownership of GameLogic::update_movement.
    // This unit test exercises the host integrator; GameWorld authority is off.
    std::env::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
    let mut logic = GameLogic::new();
    let id = ObjectId(9001);
    let mut obj = Object::new_simple(
        id,
        crate::game_logic::ObjectType::Infantry,
        "AmericaRanger".to_string(),
    );
    let start = glam::Vec3::new(0.0, 0.0, 0.0);
    obj.set_position(start);
    obj.movement.path = vec![
        glam::Vec3::new(100.0, 0.0, 0.0),
        glam::Vec3::new(200.0, 0.0, 0.0),
    ];
    obj.movement.current_path_index = 0;
    obj.movement.max_speed = 30.0; // world units / second
                                   // Reach max speed in one frame so distance ≈ max_speed * dt.
    obj.movement.acceleration = 10_000.0;
    logic.objects.insert(id, obj);

    logic.update_movement(&[id], LOGIC_FRAME_TIMESTEP);

    let after = logic.objects.get(&id).expect("obj").get_position();
    let dist = (after - start).length();
    // One logic frame at 30 u/s ≈ 1.0 unit (LOGIC_FRAME_TIMESTEP = 1/30).
    assert!(
        dist > 0.5 && dist < 2.5,
        "expected single-frame path step, got dist={dist} pos={after:?}"
    );
    assert_eq!(
        logic.objects.get(&id).unwrap().movement.current_path_index,
        0,
        "should not skip waypoints in one frame"
    );
}

#[test]
fn camera_mod_freeze_time_blocks_simulation_movement_updates() {
    // Host update_with_dt path (no shadow session): disable GameWorld movement authority.
    std::env::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
    let mut baseline = GameLogic::new();
    ensure_test_tank_template(&mut baseline);
    let baseline_unit = baseline
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("baseline unit should be created");
    {
        let obj = baseline
            .host_object_mut(baseline_unit)
            .expect("baseline unit should exist");
        obj.move_to(Vec3::new(120.0, 0.0, 0.0));
        obj.movement.max_speed = 60.0;
        obj.movement.acceleration = 3600.0;
    }
    let baseline_before = baseline
        .host_object(baseline_unit)
        .expect("baseline unit should exist")
        .get_position();
    baseline.update_with_dt(1.0 / 30.0);
    let baseline_after = baseline
        .host_object(baseline_unit)
        .expect("baseline unit should exist")
        .get_position();
    assert!(
        baseline_after.distance(baseline_before) > 0.5,
        "baseline simulation should advance movement when not frozen"
    );

    let mut frozen = GameLogic::new();
    frozen.scripts_loaded = true;
    ensure_test_tank_template(&mut frozen);
    let frozen_unit = frozen
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("frozen unit should be created");
    {
        let obj = frozen
            .host_object_mut(frozen_unit)
            .expect("frozen unit should exist");
        obj.move_to(Vec3::new(120.0, 0.0, 0.0));
        obj.movement.max_speed = 60.0;
        obj.movement.acceleration = 3600.0;
    }

    frozen.mission_scripts.push_camera_mod_freeze_time();
    frozen
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(220.0, 0.0, 120.0),
            seconds: 2.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    frozen.evaluate_and_execute_scripts(0.0);
    assert!(frozen.is_time_frozen_for_simulation());

    let frozen_before = frozen
        .host_object(frozen_unit)
        .expect("frozen unit should exist")
        .get_position();
    frozen.update_with_dt(1.0 / 60.0);
    let frozen_after = frozen
        .host_object(frozen_unit)
        .expect("frozen unit should exist")
        .get_position();
    assert!(
        frozen_after.distance(frozen_before) < 0.001,
        "movement should not advance while camera freeze-time is active"
    );
}

#[test]
fn camera_mod_freeze_angle_blocks_look_toward_until_move_finishes() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(180.0, 0.0, 90.0),
            seconds: 2.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.mission_scripts.push_camera_mod_freeze_angle();
    game_logic
        .mission_scripts
        .push_camera_mod_look_toward(CameraModLookTowardRequest {
            position: Vec3::new(400.0, 0.0, 300.0),
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.take_camera_look_toward_request().is_none(),
        "freeze angle should suppress scripted look-toward while move is active"
    );

    for _ in 0..180 {
        game_logic.update_script_camera(1.0 / 60.0);
    }

    game_logic
        .mission_scripts
        .push_camera_mod_look_toward(CameraModLookTowardRequest {
            position: Vec3::new(410.0, 0.0, 310.0),
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.take_camera_look_toward_request().is_some(),
        "look-toward should resume after scripted movement completes"
    );
}

#[test]
fn camera_mod_final_speed_multiplier_applies_to_next_script_camera_move() {
    let mut baseline = GameLogic::new();
    baseline.scripts_loaded = true;
    baseline
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(300.0, 0.0, 200.0),
            seconds: 6.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    baseline.evaluate_and_execute_scripts(0.0);
    for _ in 0..120 {
        baseline.update_script_camera(1.0 / 60.0);
    }
    let baseline_remaining = baseline.script_camera_remaining_seconds();

    let mut modified = GameLogic::new();
    modified.scripts_loaded = true;
    modified
        .mission_scripts
        .push_camera_mod_final_speed_multiplier(CameraModFinalSpeedMultiplierRequest {
            multiplier: 4,
        });
    modified.evaluate_and_execute_scripts(0.0);
    modified
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(300.0, 0.0, 200.0),
            seconds: 6.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    modified.evaluate_and_execute_scripts(0.0);
    for _ in 0..120 {
        modified.update_script_camera(1.0 / 60.0);
    }
    let modified_remaining = modified.script_camera_remaining_seconds();

    assert!(
        modified_remaining + 0.05 < baseline_remaining,
        "final speed multiplier should accelerate scripted camera progression"
    );
}

#[test]
fn camera_mod_rolling_average_arms_for_next_camera_path() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames: 7 });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        game_logic.script_camera_pending_rolling_average_frames,
        Some(7)
    );
}

#[test]
fn visual_speed_multiplier_scales_script_camera_update_rate() {
    let mut baseline = GameLogic::new();
    baseline.scripts_loaded = true;
    baseline
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(300.0, 0.0, 200.0),
            seconds: 6.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    baseline.evaluate_and_execute_scripts(0.0);
    baseline.evaluate_and_execute_scripts(1.0 / 60.0);
    let baseline_remaining = baseline.script_camera_remaining_seconds();

    let mut accelerated = GameLogic::new();
    accelerated.scripts_loaded = true;
    accelerated
        .mission_scripts
        .push_visual_speed_multiplier(VisualSpeedMultiplierRequest { multiplier: 3 });
    accelerated.evaluate_and_execute_scripts(0.0);
    accelerated
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(300.0, 0.0, 200.0),
            seconds: 6.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    accelerated.evaluate_and_execute_scripts(0.0);
    accelerated.evaluate_and_execute_scripts(1.0 / 60.0);
    let accelerated_remaining = accelerated.script_camera_remaining_seconds();

    assert_eq!(accelerated.visual_speed_multiplier(), 3.0);
    assert!(
        accelerated_remaining + 0.01 < baseline_remaining,
        "visual speed multiplier should speed up scripted camera updates"
    );
}

#[test]
fn freeze_and_unfreeze_time_toggle_script_freeze_state() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.mission_scripts.push_script_freeze_time(true);
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(game_logic.script_time_frozen_by_script);
    assert!(game_logic.is_script_time_frozen());

    game_logic.mission_scripts.push_script_freeze_time(false);
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(!game_logic.script_time_frozen_by_script);
    assert!(!game_logic.is_script_time_frozen());
}

#[test]
fn set_fps_limit_request_is_forwarded_to_engine() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_set_fps_limit(SetFpsLimitRequest { fps: 90 });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(game_logic.take_script_fps_limit_request(), Some(90));
    assert_eq!(game_logic.take_script_fps_limit_request(), None);
}

#[test]
fn move_camera_to_selection_uses_local_player_selection_center() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_tank_template(&mut game_logic);

    let first = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 200.0))
        .expect("first selected object should exist");
    let second = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(160.0, 0.0, 260.0))
        .expect("second selected object should exist");
    game_logic.select_objects(0, vec![first, second]);

    game_logic.mission_scripts.push_camera_move_to_selection();
    game_logic.evaluate_and_execute_scripts(0.0);

    let focus = game_logic
        .take_camera_focus_request()
        .expect("move camera to selection should produce focus request");
    assert_eq!(focus, Vec3::new(130.0, 0.0, 230.0));
}

#[test]
fn move_camera_to_selection_with_empty_selection_is_noop() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    game_logic.select_objects(0, Vec::new());
    game_logic.mission_scripts.push_camera_move_to_selection();
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.take_camera_focus_request().is_none(),
        "empty selection should not emit camera focus request"
    );
}

#[test]
fn camera_set_default_updates_script_camera_defaults() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_set_default(CameraSetDefaultRequest {
            pitch: 0.8,
            angle: 35.0,
            max_height: 2.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!((game_logic.script_default_camera_pitch - 0.8).abs() < f32::EPSILON);
    assert!(
        game_logic.script_default_camera_angle.abs() < f32::EPSILON,
        "C++ W3DView::setDefaultView ignores the angle parameter"
    );
    assert!((game_logic.script_default_camera_max_height - 2.0).abs() < f32::EPSILON);
}

#[test]
fn camera_set_default_sanitizes_non_finite_max_height() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_set_default(CameraSetDefaultRequest {
            pitch: 0.9,
            angle: 0.0,
            max_height: f32::NAN,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!((game_logic.script_default_camera_max_height - 1.0).abs() < f32::EPSILON);
}

#[test]
fn camera_set_default_allows_zero_max_height_scale_like_cpp() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_set_default(CameraSetDefaultRequest {
            pitch: 1.0,
            angle: 15.0,
            max_height: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(game_logic.script_default_camera_angle.abs() < f32::EPSILON);
    assert!(game_logic.script_default_camera_max_height.abs() < f32::EPSILON);
}

#[test]
fn script_screen_shake_requests_are_drained_for_engine() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_screen_shake(ScreenShakeRequest { intensity: 2 });
    game_logic
        .mission_scripts
        .push_screen_shake(ScreenShakeRequest { intensity: 5 });
    game_logic.evaluate_and_execute_scripts(0.0);

    let shakes = game_logic.take_screen_shake_requests();
    assert_eq!(shakes.len(), 2);
    assert_eq!(shakes[0].intensity, 2);
    assert_eq!(shakes[1].intensity, 5);
    assert!(
        game_logic.take_screen_shake_requests().is_empty(),
        "screen shake queue should be drained after take"
    );
}

#[test]
fn camera_add_shaker_requests_are_drained_for_engine() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_add_shaker(CameraAddShakerRequest {
            position: Vec3::new(10.0, 4.0, 20.0),
            amplitude: 3.5,
            duration_seconds: 2.0,
            radius: 120.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let shakers = game_logic.take_camera_add_shaker_requests();
    assert_eq!(shakers.len(), 1);
    assert_eq!(shakers[0].position, Vec3::new(10.0, 4.0, 20.0));
    assert!((shakers[0].amplitude - 3.5).abs() < f32::EPSILON);
    assert!((shakers[0].duration_seconds - 2.0).abs() < f32::EPSILON);
    assert!((shakers[0].radius - 120.0).abs() < f32::EPSILON);
    assert!(
        game_logic.take_camera_add_shaker_requests().is_empty(),
        "camera shaker queue should be drained after take"
    );
}

#[test]
fn camera_slave_mode_enable_and_disable_requests_are_drained_for_engine() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_slave_mode_enable(CameraSlaveModeRequest {
            thing_template_name: "CineRig".to_string(),
            bone_name: "Bone01".to_string(),
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let enable = game_logic
        .take_camera_slave_mode_enable_request()
        .expect("slave mode enable should be forwarded");
    assert_eq!(enable.thing_template_name, "CineRig");
    assert_eq!(enable.bone_name, "Bone01");
    assert!(
        !game_logic.take_camera_slave_mode_disable_request(),
        "enable should not set disable flag"
    );

    game_logic.mission_scripts.push_camera_slave_mode_disable();
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.take_camera_slave_mode_disable_request(),
        "disable should set disable flag"
    );
    assert!(
        game_logic.take_camera_slave_mode_enable_request().is_none(),
        "disable should clear pending enable request"
    );
}

#[test]
fn camera_move_home_prefers_local_command_center() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_command_center_template(&mut game_logic);
    game_logic.objects.clear();

    game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(80.0, 0.0, 90.0))
        .expect("fallback structure should exist");
    game_logic
        .create_object("TestCommandCenter", Team::USA, Vec3::new(320.0, 0.0, 410.0))
        .expect("command center should exist");

    game_logic.mission_scripts.push_camera_move_home();
    game_logic.evaluate_and_execute_scripts(0.0);

    let focus = game_logic
        .take_camera_focus_request()
        .expect("camera move home should produce focus request");
    assert_eq!(focus, Vec3::new(320.0, 0.0, 410.0));
}

#[test]
fn camera_move_home_falls_back_to_local_team_base() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_structure_template(&mut game_logic);
    game_logic.objects.clear();

    game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(190.0, 0.0, 260.0))
        .expect("team base structure should exist");

    game_logic.mission_scripts.push_camera_move_home();
    game_logic.evaluate_and_execute_scripts(0.0);

    let focus = game_logic
        .take_camera_focus_request()
        .expect("camera move home should focus local team base");
    assert_eq!(focus, Vec3::new(190.0, 0.0, 260.0));
}

#[test]
fn camera_move_home_without_local_player_is_noop() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    game_logic.players.clear();

    game_logic.mission_scripts.push_camera_move_home();
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.take_camera_focus_request().is_none(),
        "camera move home should no-op when no local player exists"
    );
}

#[test]
fn weather_visibility_script_requests_apply_last_value() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    assert!(game_logic.weather_state().visible);

    game_logic.mission_scripts.push_weather_visible(false);
    game_logic.mission_scripts.push_weather_visible(true);
    game_logic.mission_scripts.push_weather_visible(false);
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        !game_logic.weather_state().visible,
        "weather visibility should follow the final script request"
    );

    game_logic.mission_scripts.push_weather_visible(true);
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(game_logic.weather_state().visible);
}

#[test]
fn popup_and_script_ui_requests_are_forwarded_into_runtime_state() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_popup_message(ScriptPopupMessageRequest {
            message: "Script popup".to_string(),
            x_percent: 45,
            y_percent: 60,
            width: 420,
            pause: true,
            pause_music: true,
            popup_generation: 0,
        });
    game_logic
        .mission_scripts
        .push_view_guardband(ViewGuardbandRequest {
            x_bias: 1.4,
            y_bias: 0.6,
        });
    game_logic
        .mission_scripts
        .push_camera_bw_mode(CameraBwModeRequest {
            enabled: true,
            frames: 30,
        });
    game_logic.mission_scripts.push_skybox_enabled(false);
    game_logic
        .mission_scripts
        .push_camera_motion_blur(CameraMotionBlurRequest::Basic {
            zoom_in: true,
            saturate: false,
        });
    game_logic
        .mission_scripts
        .push_camera_motion_blur(CameraMotionBlurRequest::Jump {
            position: Vec3::new(120.0, 20.0, 260.0),
            saturate: true,
        });
    game_logic
        .mission_scripts
        .push_named_timer_mutation(NamedTimerMutation::Add {
            name: "LaunchClock".to_string(),
            text: "Launch in".to_string(),
            countdown: true,
        });
    game_logic.mission_scripts.push_named_timer_display(false);
    game_logic
        .mission_scripts
        .push_superweapon_display_enabled(false);
    game_logic
        .mission_scripts
        .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Hide {
            object_id: 88,
        });
    game_logic
        .mission_scripts
        .push_cameo_flash(CameoFlashRequest {
            command_button_name: "CommandButtonA".to_string(),
            flash_count: 6,
        });

    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        !game_logic.is_paused,
        "popup pause is reconciled by Main's active-popup pause owner, not the raw script evaluator"
    );
    assert!(
        game_logic.take_music_stop_request(),
        "popup pause_music should request music stop"
    );

    let popups = game_logic.take_popup_message_requests();
    assert_eq!(popups.len(), 1);
    assert_eq!(popups[0].message, "Script popup");
    assert_eq!(popups[0].x_percent, 45);
    assert_eq!(popups[0].y_percent, 60);
    assert_eq!(popups[0].width, 420);
    assert!(popups[0].pause);
    assert!(popups[0].pause_music);
    assert_ne!(
        popups[0].popup_generation, 0,
        "mission-hook popups carry a live-only acknowledgement identity"
    );

    let guardband = game_logic
        .take_view_guardband_request()
        .expect("view guardband request should be pending");
    assert!((guardband.x_bias - 1.4).abs() < f32::EPSILON);
    assert!((guardband.y_bias - 0.6).abs() < f32::EPSILON);

    let bw = game_logic
        .take_camera_bw_mode_request()
        .expect("camera bw request should be pending");
    assert!(bw.enabled);
    assert_eq!(bw.frames, 30);

    assert!(
        !game_logic.script_skybox_enabled,
        "skybox flag should reflect latest script update"
    );
    assert_eq!(
        game_logic
            .script_cameo_flash_count
            .get("CommandButtonA")
            .copied(),
        Some(6)
    );
    assert_eq!(
        game_logic.script_named_timers.get("LaunchClock"),
        Some(&("Launch in".to_string(), true))
    );
    assert!(
        !game_logic.script_named_timer_display_shown,
        "named timer display should be disabled by script"
    );
    assert!(
        !game_logic.script_superweapon_display_enabled,
        "superweapon display should be disabled by script"
    );
    assert!(
        game_logic
            .script_superweapon_hidden_objects
            .contains(&ObjectId(88)),
        "hidden object list should include scripted object id"
    );

    let blur_requests = game_logic.take_camera_motion_blur_requests();
    assert_eq!(blur_requests.len(), 2);
    assert!(matches!(
        blur_requests[0],
        CameraMotionBlurRequest::Basic {
            zoom_in: true,
            saturate: false
        }
    ));
    assert!(matches!(
        blur_requests[1],
        CameraMotionBlurRequest::Jump {
            position,
            saturate: true
        } if position == Vec3::new(120.0, 20.0, 260.0)
    ));

    let jump_focus = game_logic
        .take_camera_focus_request()
        .expect("motion blur jump should emit a camera focus fallback");
    assert_eq!(jump_focus, Vec3::new(120.0, 20.0, 260.0));
}

#[test]
fn popup_runtime_residual_keeps_only_the_latest_cxx_active_dialog() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    // Retail InGameUI clears the old layout before opening the new one.  A
    // pause popup immediately followed by a non-pause popup must not leave a
    // stale presentation record that re-pauses the authoritative world.
    game_logic
        .mission_scripts
        .push_popup_message(ScriptPopupMessageRequest {
            message: "old pause".to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: true,
            pause_music: false,
            popup_generation: 0,
        });
    game_logic
        .mission_scripts
        .push_popup_message(ScriptPopupMessageRequest {
            message: "latest nonpause".to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: false,
            pause_music: false,
            popup_generation: 0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let popups = game_logic.take_popup_message_requests();
    assert_eq!(popups.len(), 1);
    assert_eq!(popups[0].message, "latest nonpause");
    assert!(!popups[0].pause);
    assert!(!game_logic.is_paused);

    // The converse retains exactly the newer pause record so Main may claim
    // and release only that popup's pause ownership.
    game_logic
        .mission_scripts
        .push_popup_message(ScriptPopupMessageRequest {
            message: "old nonpause".to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: false,
            pause_music: false,
            popup_generation: 0,
        });
    game_logic
        .mission_scripts
        .push_popup_message(ScriptPopupMessageRequest {
            message: "latest pause".to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: true,
            pause_music: false,
            popup_generation: 0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let popups = game_logic.take_popup_message_requests();
    assert_eq!(popups.len(), 1);
    assert_eq!(popups[0].message, "latest pause");
    assert!(popups[0].pause);
    assert!(!game_logic.is_paused);
}

#[test]
fn script_named_timer_and_superweapon_mutations_respect_order() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_named_timer_mutation(NamedTimerMutation::Add {
            name: "TimerA".to_string(),
            text: "Phase 1".to_string(),
            countdown: true,
        });
    game_logic
        .mission_scripts
        .push_named_timer_mutation(NamedTimerMutation::Remove {
            name: "TimerA".to_string(),
        });
    game_logic
        .mission_scripts
        .push_named_timer_mutation(NamedTimerMutation::Add {
            name: "TimerA".to_string(),
            text: "Phase 2".to_string(),
            countdown: false,
        });
    game_logic
        .mission_scripts
        .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Hide {
            object_id: 123,
        });
    game_logic
        .mission_scripts
        .push_superweapon_object_display_mutation(SuperweaponObjectDisplayMutation::Show {
            object_id: 123,
        });

    game_logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        game_logic.script_named_timers.get("TimerA"),
        Some(&("Phase 2".to_string(), false)),
        "later timer mutation should win"
    );
    assert!(
        !game_logic
            .script_superweapon_hidden_objects
            .contains(&ObjectId(123)),
        "show mutation should undo prior hide mutation for the same object"
    );
}

/// Residual (hq-gq7n): combat kill must register real particle-system entries
/// (not log-only). Host registry + presentation observe path.
#[test]
fn combat_kill_spawns_particle_system_registry_entries() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 9999.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    // Fire path: muzzle + impact registry entries exist before destroy cleanup.
    assert!(
        game_logic.combat_particles().active_count() >= 2,
        "weapon fire must register muzzle/impact particle systems, got {}",
        game_logic.combat_particles().active_count()
    );
    assert_eq!(
        game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .len(),
        1,
        "muzzle flash entry required"
    );

    // Process destroy list (same end-of-step path as step_simulation).
    game_logic.process_destroy_list();

    assert!(
        game_logic.host_object(target_id).is_none(),
        "target must be removed after kill"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "kill must register DeathExplosion particle system entry"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathSmoke)
            .is_empty(),
        "kill must register DeathSmoke particle system entry"
    );
    assert!(
        game_logic.combat_particles().active_count() >= 4,
        "fire + death systems must remain registered (not log-only); count={}",
        game_logic.combat_particles().active_count()
    );

    // Every entry has a non-empty template name and stable id.
    for entry in game_logic.combat_particles().active_systems() {
        assert!(!entry.template_name.is_empty(), "template name required");
        assert!(entry.id > 0, "stable host system id required");
        assert!(entry.active);
    }

    #[cfg(feature = "game_client")]
    {
        // Client mirror path: at least one spawn should land in ParticleSystemManager.
        let mirrored = game_logic
            .combat_particles()
            .active_systems()
            .filter(|e| e.client_system_id.is_some())
            .count();
        assert!(
            mirrored > 0,
            "with game_client, host entries should mirror into client manager"
        );
        let guard =
            game_client::effects::get_particle_system_manager().expect("particle manager readable");
        let manager = guard.as_ref().expect("manager initialized");
        assert!(
            manager.active_system_count() > 0,
            "client ParticleSystemManager must hold systems after combat kill/fire"
        );
    }
}

/// Residual: cash bounty on kill awards cash to killer player.
/// C++ Player::doBountyForKill + CashBountyPower (SCIENCE_CashBounty*).
/// Fail-closed: not floating text / palace module matrix.
#[test]
fn cash_bounty_increases_cash_on_enemy_kill() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // GLA SCIENCE_CashBounty3 residual = 20% of build cost.
    assert!(
        game_logic.force_set_player_cash_bounty(0, 0.20),
        "must configure killer cash bounty"
    );
    assert!(
        (game_logic
            .get_player(0)
            .expect("usa player")
            .cash_bounty_percent
            - 0.20)
            .abs()
            < 1e-6
    );

    let cash_before = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    // TestTank build_cost = 600 → 20% = 120 bounty.
    let expected_bounty = crate::game_logic::host_cash_bounty::compute_bounty_award(600, 0.20);
    assert_eq!(expected_bounty, 120);

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 9999.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);
    game_logic.process_destroy_list();

    assert!(
        game_logic.host_object(target_id).is_none(),
        "target must be removed after kill"
    );

    let cash_after = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert!(
        cash_after > cash_before,
        "killer cash must increase on bounty kill (before={cash_before}, after={cash_after})"
    );
    assert_eq!(
        cash_after,
        cash_before.saturating_add(expected_bounty),
        "bounty must be ceil(build_cost * percent)"
    );
    assert!(
        game_logic.honesty_cash_bounty_ok(),
        "cash bounty residual honesty (configured + awarded)"
    );
    assert_eq!(
        game_logic.cash_bounty_earned_total(),
        expected_bounty,
        "registry must track earned bounty total"
    );
    // last_damage_source residual: killer ObjectId from combat fire path.
    assert!(
        game_logic
            .cash_bounty_registry()
            .honesty_last_damage_source_killer_ok(),
        "bounty floating text must prefer last_damage_source killer residual"
    );
    let ft = game_logic
        .cash_bounty_registry()
        .floating_texts
        .last()
        .expect("bounty floating text");
    assert_eq!(
        ft.killer_id, attacker_id,
        "killer id must be attacker residual"
    );
    assert_eq!(ft.amount, expected_bounty);
    assert_eq!(ft.color_rgba, (255, 255, 0, 255));
}

/// Residual: no cash bounty when percent is zero (fail-closed default).
#[test]
fn cash_bounty_zero_percent_does_not_award() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let cash_before = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 9999.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);
    game_logic.process_destroy_list();

    let cash_after = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after, cash_before,
        "no bounty when cash_bounty_percent is 0"
    );
    assert!(
        !game_logic.honesty_cash_bounty_award_ok(),
        "no award honesty when bounty disabled"
    );
}

/// Residual: SCIENCE_CashBounty unlock raises player cash_bounty_percent.
#[test]
fn cash_bounty_science_unlock_sets_percent() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let player = game_logic.get_player_mut(2).expect("gla player");
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty1"));
    assert!((player.cash_bounty_percent - 0.05).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty2"));
    assert!((player.cash_bounty_percent - 0.10).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty3"));
    assert!((player.cash_bounty_percent - 0.20).abs() < 1e-6);
    // Already unlocked — no change / false.
    assert!(!player.unlock_science("SCIENCE_CashBounty3"));
    assert!((player.cash_bounty_percent - 0.20).abs() < 1e-6);
}

#[test]
fn combat_fire_without_kill_still_spawns_muzzle_particle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 1.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }

    game_logic.frame = 30;
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.host_object(target_id).is_some(),
        "target should survive low damage"
    );
    assert_eq!(
        game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .len(),
        1
    );
    let muzzle = game_logic
        .combat_particles()
        .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)[0];
    assert_eq!(muzzle.template_name, "MuzzleFlash");
    assert_eq!(muzzle.source_object, Some(attacker_id));
}

/// Residual (hq-7zxm): host combat fire/kill must enqueue real audio events
/// (not silent no-op). Fail-closed: request path, not full Miles retail.
#[test]
fn combat_fire_queues_weapon_fire_audio_event() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 1.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }

    game_logic.frame = 30;
    game_logic.queued_audio_events.clear();
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    let fire_events: Vec<_> = game_logic
        .queued_audio_events
        .iter()
        .filter(|e| e.event_type == "WeaponFire")
        .collect();
    assert!(
        !fire_events.is_empty(),
        "weapon fire must queue WeaponFire audio request, got {:?}",
        game_logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.as_str())
            .collect::<Vec<_>>()
    );
    let fire = fire_events[0];
    assert_eq!(fire.object_id, Some(attacker_id));
    assert!(
        fire.position.is_some(),
        "weapon fire audio must be positional"
    );
    assert!(
        fire.priority > 0,
        "weapon fire audio priority must be non-zero"
    );
}

#[test]
fn combat_kill_queues_unit_die_audio_event() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker exists");
        attacker.attack_target(target_id);
        attacker.weapon = Some(Weapon {
            damage: 9999.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
    }

    game_logic.frame = 60;
    game_logic.queued_audio_events.clear();
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    // Fire request present before destroy list processing.
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "WeaponFire"),
        "kill path still fires WeaponFire first"
    );

    game_logic.process_destroy_list();

    let die_events: Vec<_> = game_logic
        .queued_audio_events
        .iter()
        .filter(|e| e.event_type == "UnitDie")
        .collect();
    assert!(
        !die_events.is_empty(),
        "kill must queue UnitDie audio request, got {:?}",
        game_logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.as_str())
            .collect::<Vec<_>>()
    );
    let die = die_events[0];
    assert_eq!(die.object_id, Some(target_id));
    assert!(die.position.is_some(), "death audio must be positional");
    assert!(
        game_logic.host_object(target_id).is_none(),
        "target must be removed after kill"
    );
}

/// Residual: host DaisyCutter / FuelAirBomb DoSpecialPower queues a strike
/// and completes with area damage (honesty: queue + complete, fail-closed
/// vs full retail OCL aircraft / MOAB upgrade parity).
#[test]
fn daisy_cutter_host_path_queues_and_completes_area_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{HostStrikePhase, HostSuperweaponKind};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_DaisyCutter");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(500.0, 0.0, 0.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("friend");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let friend = game_logic.host_object_mut(friend_id).expect("friend");
        friend.health.current = 500.0;
        friend.health.maximum = 500.0;
        friend.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_enemy_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(50.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::DaisyCutter,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Queue honesty: strike pending, caster on cooldown + SpecialAbility.
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::DaisyCutter),
        "DaisyCutter must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponDaisyCutter"),
        "activation must queue SuperweaponDaisyCutter audio"
    );

    // Before impact delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 89;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no damage before impact frame"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::DaisyCutter));

    // At impact: area damage + complete honesty.
    game_logic.frame = 90;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
        "DaisyCutter must complete on impact frame"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::DaisyCutter),
        "host path honesty requires completed strike"
    );

    let enemy_after = game_logic.host_object(enemy_id).map(|o| o.health.current);
    // Epicenter damage is large enough to kill residual test tank or leave 0.
    let enemy_dealt = test_observed_damage_to(enemy_id, 500.0, enemy_after.unwrap_or(0.0));
    assert!(
        enemy_dealt + 0.1 >= 500.0
            || enemy_after.is_none()
            || enemy_after == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at epicenter must take lethal DaisyCutter residual damage (dealt={enemy_dealt})"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take DaisyCutter residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside radius must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "DaisyCutterExplosion"),
        "impact must queue DaisyCutterExplosion audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::DaisyCutter);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(completed[0].objects_hit >= 1);
    assert!(completed[0].total_damage_applied > 0.0);

    game_logic.process_destroy_list();
}

/// Residual: A10 (Airstrike) host path queues and completes.
#[test]
fn a10_strike_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_A10ThunderboltMissileStrike1");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 200.0;
        enemy.health.maximum = 200.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Airstrike,
            target: PowerTarget::Location(Vec3::new(20.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(game_logic
        .special_power_strikes()
        .honesty_queue_ok(HostSuperweaponKind::A10Strike));

    game_logic.frame = 60;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::A10Strike),
        "A10 host path must complete"
    );
    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::A10Strike);
    assert_eq!(completed.len(), 1);
    assert!(completed[0].total_damage_applied > 0.0);
    assert!(completed[0].objects_hit >= 1);
}

/// Residual: CarpetBomb DoSpecialPower queues a delayed multi-point line
/// strike; damage applies after approach delay with DropDelay stagger.
/// Fail-closed: not full B52 OCL DeliverPayload transport Object.
#[test]
fn carpet_bomb_host_path_queues_and_applies_delayed_line_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        carpet_bomb_points, multi_strike_last_impact_frame, ArtilleryBarrageScienceTier,
        HostStrikePhase, HostSuperweaponKind, CARPET_BOMB_DAMAGE, CARPET_BOMB_DROP_DELAY_FRAMES,
        CARPET_BOMB_IMPACT_DELAY_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let target = Vec3::new(100.0, 0.0, 0.0);
    // Place enemies on DropVariance-adjusted residual epicenters.
    let points = carpet_bomb_points(target);
    let center = points[7];
    let outer = points[14];

    // player_id 0 maps to Team::USA for ownership validation residual.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_center_id = game_logic
        .create_object("TestTank", Team::GLA, center)
        .expect("enemy center");
    let enemy_outer_id = game_logic
        .create_object("TestTank", Team::GLA, outer)
        .expect("enemy outer bomb line");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, center)
        .expect("friend");

    for id in [enemy_center_id, enemy_outer_id, far_enemy_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CarpetBomb,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::CarpetBomb),
        "CarpetBomb must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponCarpetBomb"),
        "activation must queue SuperweaponCarpetBomb audio"
    );

    // Before impact delay: no damage.
    let health_before_center = game_logic
        .host_object(enemy_center_id)
        .unwrap()
        .health
        .current;
    let health_before_outer = game_logic
        .host_object(enemy_outer_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = CARPET_BOMB_IMPACT_DELAY_FRAMES - 1;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic
            .host_object(enemy_center_id)
            .unwrap()
            .health
            .current,
        health_before_center,
        "no damage before carpet bomb impact frame"
    );
    assert_eq!(
        game_logic
            .host_object(enemy_outer_id)
            .unwrap()
            .health
            .current,
        health_before_outer,
        "no outer-line damage before impact frame"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::CarpetBomb));

    // First DropDelay bomb: not complete yet (center/outer later).
    game_logic.frame = CARPET_BOMB_IMPACT_DELAY_FRAMES;
    game_logic.update_special_power_strikes();
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::CarpetBomb));
    assert!(
        CARPET_BOMB_DROP_DELAY_FRAMES > 0,
        "retail DropDelay residual must be non-zero"
    );

    // Jump to last DropDelay bomb: multi-point line damage + complete honesty.
    let activate = game_logic
        .special_power_strikes()
        .pending_of_kind(HostSuperweaponKind::CarpetBomb)
        .first()
        .map(|s| s.activate_frame)
        .or_else(|| {
            game_logic
                .special_power_strikes()
                .completed_of_kind(HostSuperweaponKind::CarpetBomb)
                .first()
                .map(|s| s.activate_frame)
        })
        .unwrap_or(0);
    let last = multi_strike_last_impact_frame(
        HostSuperweaponKind::CarpetBomb,
        activate,
        ArtilleryBarrageScienceTier::Level1,
    );
    game_logic.frame = last;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CarpetBomb),
        "CarpetBomb must complete on last DropDelay bomb frame"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::CarpetBomb),
        "host path honesty requires completed carpet bomb strike"
    );

    let center_hp = game_logic
        .host_object(enemy_center_id)
        .map(|o| o.health.current);
    let outer_hp = game_logic
        .host_object(enemy_outer_id)
        .map(|o| o.health.current);
    // Epicenter residual damage = CARPET_BOMB_DAMAGE (300) per bomb hit.
    let center_dealt = test_observed_damage_to(
        enemy_center_id,
        health_before_center,
        center_hp.unwrap_or(0.0),
    );
    let outer_dealt =
        test_observed_damage_to(enemy_outer_id, health_before_outer, outer_hp.unwrap_or(0.0));
    assert!(
        center_dealt + 0.1 >= CARPET_BOMB_DAMAGE
            || center_hp.is_none()
            || center_hp.map(|h| h < health_before_center - CARPET_BOMB_DAMAGE + 1.0) == Some(true)
            || center_hp == Some(0.0)
            || game_logic
                .host_object(enemy_center_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at center bomb line must take carpet bomb residual damage, got {center_hp:?} dealt={center_dealt}"
    );
    assert!(
        outer_dealt + 0.1 >= CARPET_BOMB_DAMAGE
            || outer_hp.is_none()
            || outer_hp.map(|h| h < health_before_outer - CARPET_BOMB_DAMAGE + 1.0) == Some(true)
            || outer_hp == Some(0.0)
            || game_logic
                .host_object(enemy_outer_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy on outer bomb epicenter must take multi-strike residual damage, got {outer_hp:?} dealt={outer_dealt}"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take CarpetBomb residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies off the bomb line must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "ExplosionCarpetBomb"),
        "impact must queue ExplosionCarpetBomb audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::CarpetBomb);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(
        completed[0].objects_hit >= 2,
        "multi-strike must hit both line enemies, hit={}",
        completed[0].objects_hit
    );
    assert!(completed[0].total_damage_applied > 0.0);
    assert_eq!(
        completed[0].multi_strike_applied,
        crate::game_logic::special_power_strikes::CARPET_BOMB_COUNT
    );

    game_logic.process_destroy_list();
}

/// Residual: ArtilleryBarrage DoSpecialPower queues a delayed multi-shell
/// scatter strike; damage applies after DelayDeliveryMax + per-shell stagger.
/// Fail-closed: not full ChinaArtilleryCannon OCL DeliverPayload transport Object.
#[test]
fn artillery_barrage_host_path_queues_and_applies_delayed_multi_shell_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        artillery_barrage_points, multi_strike_last_impact_frame, ArtilleryBarrageScienceTier,
        HostStrikePhase, HostSuperweaponKind, ARTILLERY_BARRAGE_DAMAGE,
        ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_ArtilleryBarrage1");
    }

    let target = Vec3::new(100.0, 0.0, 0.0);
    // WeaponErrorRadius residual scatter: place outer enemy on shell index 1.
    let points = artillery_barrage_points(target);
    let outer_shell = points[1];

    // player_id 0 maps to Team::USA for ownership validation residual.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_center_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy center");
    let enemy_outer_id = game_logic
        .create_object("TestTank", Team::GLA, outer_shell)
        .expect("enemy outer shell");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_center_id, enemy_outer_id, far_enemy_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Artillery,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::ArtilleryBarrage),
        "ArtilleryBarrage must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponArtilleryBarrage"),
        "activation must queue SuperweaponArtilleryBarrage audio"
    );

    // Before impact delay: no damage.
    let health_before_center = game_logic
        .host_object(enemy_center_id)
        .unwrap()
        .health
        .current;
    let health_before_outer = game_logic
        .host_object(enemy_outer_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 1;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic
            .host_object(enemy_center_id)
            .unwrap()
            .health
            .current,
        health_before_center,
        "no damage before artillery impact frame"
    );
    assert_eq!(
        game_logic
            .host_object(enemy_outer_id)
            .unwrap()
            .health
            .current,
        health_before_outer,
        "no outer-shell damage before impact frame"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage));

    // Lead shell at DelayDeliveryMax; remaining shells stagger via DelayDelivery residual.
    let activate = game_logic
        .special_power_strikes()
        .pending_of_kind(HostSuperweaponKind::ArtilleryBarrage)
        .first()
        .map(|s| s.activate_frame)
        .unwrap_or(0);
    let last = multi_strike_last_impact_frame(
        HostSuperweaponKind::ArtilleryBarrage,
        activate,
        ArtilleryBarrageScienceTier::Level1,
    );
    game_logic.frame = activate.saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
    game_logic.update_special_power_strikes();
    if !game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage)
    {
        game_logic.frame = last;
        game_logic.update_special_power_strikes();
    }

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage),
        "ArtilleryBarrage must complete after DelayDelivery stagger residual"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::ArtilleryBarrage),
        "host path honesty requires completed artillery barrage strike"
    );

    let center_hp = game_logic
        .host_object(enemy_center_id)
        .map(|o| o.health.current);
    let outer_hp = game_logic
        .host_object(enemy_outer_id)
        .map(|o| o.health.current);
    // Epicenter residual damage = ARTILLERY_BARRAGE_DAMAGE (105) per shell hit.
    let center_dealt = test_observed_damage_to(
        enemy_center_id,
        health_before_center,
        center_hp.unwrap_or(0.0),
    );
    let outer_dealt =
        test_observed_damage_to(enemy_outer_id, health_before_outer, outer_hp.unwrap_or(0.0));
    assert!(
        center_dealt + 0.1 >= ARTILLERY_BARRAGE_DAMAGE
            || center_hp.is_none()
            || center_hp.map(|h| h < health_before_center - ARTILLERY_BARRAGE_DAMAGE + 1.0)
                == Some(true)
            || center_hp == Some(0.0)
            || game_logic
                .host_object(enemy_center_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at center shell must take artillery residual damage, got {center_hp:?} dealt={center_dealt}"
    );
    assert!(
        outer_dealt + 0.1 >= ARTILLERY_BARRAGE_DAMAGE
            || outer_hp.is_none()
            || outer_hp.map(|h| h < health_before_outer - ARTILLERY_BARRAGE_DAMAGE + 1.0)
                == Some(true)
            || outer_hp == Some(0.0)
            || game_logic
                .host_object(enemy_outer_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy on outer shell epicenter must take multi-shell residual damage, got {outer_hp:?} dealt={outer_dealt}"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take ArtilleryBarrage residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside shell scatter must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "FX_ArtilleryBarrage"),
        "impact must queue FX_ArtilleryBarrage audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::ArtilleryBarrage);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(
        completed[0].objects_hit >= 2,
        "multi-shell must hit both scatter enemies, hit={}",
        completed[0].objects_hit
    );
    assert!(completed[0].total_damage_applied > 0.0);
    assert_eq!(
        completed[0].multi_strike_applied,
        crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_SHELL_COUNT
    );

    game_logic.process_destroy_list();
}

/// Residual: SupW_CruiseMissile / SUPERWEAPON_CruiseMissile DoSpecialPower
/// queues a delayed loft strike; MOAB area damage applies only after delay.
/// Fail-closed: not full NeutronMissileUpdate loft / door animation /
/// OCL FireWeapon projectile / MOABFlameWeapon secondary.
#[test]
fn cruise_missile_host_path_queues_and_applies_delayed_area_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostStrikePhase, HostSuperweaponKind, CRUISE_MISSILE_DAMAGE,
        CRUISE_MISSILE_IMPACT_DELAY_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let target = Vec3::new(100.0, 0.0, 0.0);

    // player_id 0 maps to Team::USA for ownership validation residual.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy");
    let near_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(140.0, 0.0, 0.0))
        .expect("near enemy");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_id, near_enemy_id, far_enemy_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CruiseMissile,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::CruiseMissile),
        "CruiseMissile must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponCruiseMissile"),
        "activation must queue SuperweaponCruiseMissile audio"
    );

    // Before impact delay: no damage (loft residual in flight).
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let near_health_before = game_logic
        .host_object(near_enemy_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = CRUISE_MISSILE_IMPACT_DELAY_FRAMES - 1;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no damage before cruise missile impact frame"
    );
    assert_eq!(
        game_logic
            .host_object(near_enemy_id)
            .unwrap()
            .health
            .current,
        near_health_before,
        "no near-radius damage before impact frame"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::CruiseMissile));

    // At impact: MOAB area damage + complete honesty.
    game_logic.frame = CRUISE_MISSILE_IMPACT_DELAY_FRAMES;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CruiseMissile),
        "CruiseMissile must complete on impact frame"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::CruiseMissile),
        "host path honesty requires completed cruise missile strike"
    );

    let enemy_hp = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let near_hp = game_logic
        .host_object(near_enemy_id)
        .map(|o| o.health.current);
    // Epicenter residual damage = CRUISE_MISSILE_DAMAGE (2000) — lethal to 500 HP.
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_hp.unwrap_or(0.0));
    let near_dealt =
        test_observed_damage_to(near_enemy_id, near_health_before, near_hp.unwrap_or(0.0));
    assert!(
        enemy_dealt + 0.1 >= health_before
            || enemy_hp.is_none()
            || enemy_hp == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at epicenter must take lethal CruiseMissile residual damage, got {enemy_hp:?} dealt={enemy_dealt}"
    );
    assert!(
        near_dealt > 0.0
            || near_hp.map(|h| h < near_health_before).unwrap_or(false)
            || near_hp.is_none(),
        "enemy inside MOAB radius must take CruiseMissile residual damage, got {near_hp:?} dealt={near_dealt}"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take CruiseMissile residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside MOAB radius must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "CruiseMissileImpact"),
        "impact must queue CruiseMissileImpact audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::CruiseMissile);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(
        completed[0].objects_hit >= 1,
        "cruise missile must hit epicenter enemy, hit={}",
        completed[0].objects_hit
    );
    assert!(
        completed[0].total_damage_applied >= CRUISE_MISSILE_DAMAGE - 1.0,
        "damage applied must cover MOAB primary, got {}",
        completed[0].total_damage_applied
    );

    game_logic.process_destroy_list();
}

/// Residual: America Paradrop / Airborne DoSpecialPower queues a drop and
/// spawns infantry near the target after approach delay.
/// Fail-closed: not full OCL cargo plane / parachute container path.
#[test]
fn america_paradrop_host_path_queues_and_spawns_infantry() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_paradrop::{
        HostParadropKind, HostParadropPhase, AMERICA_PARADROP_UNIT_COUNT,
        PARADROP_RESIDUAL_TEMPLATE,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_Paradrop1");
    }
    ensure_test_infantry_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(200.0, 0.0, 100.0);
    let infantry_count = |gl: &GameLogic| {
        gl.host_objects()
            .values()
            .filter(|o| o.is_kind_of(crate::game_logic::KindOf::Infantry))
            .count()
    };
    assert_eq!(infantry_count(&game_logic), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Paradrop,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .host_paradrops()
            .honesty_queue_ok(HostParadropKind::AmericaParadrop),
        "Paradrop must queue a pending host mission"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponParadrop"),
        "activation must queue SuperweaponParadrop audio"
    );
    // Cargo plane / parachute DeliverPayload residual may spawn before
    // infantry drop delay; residual infantry must not appear yet.
    assert_eq!(
        infantry_count(&game_logic),
        0,
        "no infantry before drop delay"
    );
    assert!(
        game_logic.host_paradrops.transports_spawned >= 1,
        "cargo plane residual should spawn on queue"
    );
    assert!(!game_logic
        .host_paradrops()
        .honesty_complete_ok(HostParadropKind::AmericaParadrop));

    game_logic.frame = 89;
    game_logic.update_paradrops();
    assert_eq!(
        infantry_count(&game_logic),
        0,
        "still no infantry one frame before drop"
    );

    game_logic.frame = 90;
    game_logic.update_paradrops();

    assert!(
        game_logic
            .host_paradrops()
            .honesty_complete_ok(HostParadropKind::AmericaParadrop),
        "Paradrop must complete with spawned units"
    );
    assert!(
        game_logic
            .host_paradrops()
            .honesty_host_path_ok(HostParadropKind::AmericaParadrop),
        "host path honesty requires completed drop with units"
    );

    let completed = game_logic
        .host_paradrops()
        .completed_of_kind(HostParadropKind::AmericaParadrop);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostParadropPhase::Completed);
    assert_eq!(
        completed[0].spawned_unit_ids.len(),
        AMERICA_PARADROP_UNIT_COUNT as usize,
        "must spawn residual America Paradrop1 infantry count"
    );

    let mut near_target = 0_u32;
    for id in &completed[0].spawned_unit_ids {
        let obj = game_logic.host_object(*id).expect("spawned infantry");
        assert_eq!(obj.team, Team::USA);
        assert!(
            obj.thing.template.name == PARADROP_RESIDUAL_TEMPLATE
                || obj.thing.template.name.contains("Infantry")
                || obj.thing.template.name.contains("Ranger"),
            "spawned residual infantry template, got {}",
            obj.thing.template.name
        );
        let pos = obj.get_position();
        let dx = pos.x - target.x;
        let dz = pos.z - target.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 80.0 {
            near_target += 1;
        }
    }
    assert_eq!(
        near_target, AMERICA_PARADROP_UNIT_COUNT,
        "all paradrop infantry must appear near target location"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "ParadropLanding"),
        "drop must queue ParadropLanding audio"
    );
    assert_eq!(
        infantry_count(&game_logic),
        AMERICA_PARADROP_UNIT_COUNT as usize,
        "infantry count after paradrop drop"
    );
}

/// Residual: GLA Rebel Ambush DoSpecialPower queues a spawn and
/// creates infantry near the target after fade delay.
/// Fail-closed: not full OCL CreateObject / science upgrade tiers.
#[test]
fn gla_ambush_host_path_queues_and_spawns_infantry() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_ambush::{
        HostAmbushKind, HostAmbushPhase, AMBUSH_RESIDUAL_TEMPLATE, GLA_AMBUSH1_UNIT_COUNT,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_RebelAmbush1");
    }
    ensure_test_infantry_template(&mut game_logic);

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
    let objects_before = game_logic.host_objects().len();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Ambush,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA (player 0 is USA; ownership check)
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .host_ambushes()
            .honesty_queue_ok(HostAmbushKind::GLARebelAmbush),
        "Ambush must queue a pending host mission"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "RebelAmbushActivated"),
        "activation must queue RebelAmbushActivated audio"
    );
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before,
        "no infantry before ambush fade delay"
    );
    assert!(!game_logic
        .host_ambushes()
        .honesty_complete_ok(HostAmbushKind::GLARebelAmbush));

    game_logic.frame = 89;
    game_logic.update_ambushes();
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before,
        "still no infantry one frame before spawn"
    );

    game_logic.frame = 90;
    game_logic.update_ambushes();

    assert!(
        game_logic
            .host_ambushes()
            .honesty_complete_ok(HostAmbushKind::GLARebelAmbush),
        "Ambush must complete with spawned units"
    );
    assert!(
        game_logic
            .host_ambushes()
            .honesty_host_path_ok(HostAmbushKind::GLARebelAmbush),
        "host path honesty requires completed ambush with units"
    );

    let completed = game_logic
        .host_ambushes()
        .completed_of_kind(HostAmbushKind::GLARebelAmbush);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostAmbushPhase::Completed);
    assert_eq!(
        completed[0].spawned_unit_ids.len(),
        GLA_AMBUSH1_UNIT_COUNT as usize,
        "must spawn residual Ambush1 infantry count"
    );

    let mut near_target = 0_u32;
    for id in &completed[0].spawned_unit_ids {
        let obj = game_logic.host_object(*id).expect("spawned infantry");
        assert_eq!(obj.team, Team::GLA);
        assert!(
            obj.thing.template.name == AMBUSH_RESIDUAL_TEMPLATE
                || obj.thing.template.name.contains("Infantry")
                || obj.thing.template.name.contains("Rebel"),
            "spawned residual infantry template, got {}",
            obj.thing.template.name
        );
        let pos = obj.get_position();
        let dx = pos.x - target.x;
        let dz = pos.z - target.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 80.0 {
            near_target += 1;
        }
    }
    assert_eq!(
        near_target, GLA_AMBUSH1_UNIT_COUNT,
        "all ambush infantry must appear near target location"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "RebelAmbushSpawn"),
        "spawn must queue RebelAmbushSpawn audio"
    );
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before + GLA_AMBUSH1_UNIT_COUNT as usize
    );
}

/// Residual: GLA SCUD Storm host path queues and completes.
#[test]
fn scud_storm_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 800.0;
        enemy.health.maximum = 800.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ScudStorm,
            target: PowerTarget::Object(enemy_id),
        },
        player_id: 2, // Team::GLA
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(game_logic
        .special_power_strikes()
        .honesty_queue_ok(HostSuperweaponKind::ScudStorm));

    // First missile at PreAttackDelay = 90 frames (multi-missile residual).
    use crate::game_logic::special_power_strikes::{
        multi_strike_last_impact_frame, ArtilleryBarrageScienceTier, SCUD_STORM_PRE_ATTACK_FRAMES,
    };
    game_logic.frame = SCUD_STORM_PRE_ATTACK_FRAMES;
    game_logic.update_special_power_strikes();
    // Mid-storm: first wave applied, not necessarily complete.
    assert!(
        game_logic
            .special_power_strikes()
            .pending_of_kind(HostSuperweaponKind::ScudStorm)
            .first()
            .map(|s| s.multi_strike_applied >= 1)
            .unwrap_or(false)
            || game_logic
                .special_power_strikes()
                .honesty_complete_ok(HostSuperweaponKind::ScudStorm),
        "first ScudStorm missile residual must apply"
    );

    // Jump to last missile DelayBetweenShots residual frame.
    let activate = game_logic
        .special_power_strikes()
        .pending_of_kind(HostSuperweaponKind::ScudStorm)
        .first()
        .map(|s| s.activate_frame)
        .or_else(|| {
            game_logic
                .special_power_strikes()
                .completed_of_kind(HostSuperweaponKind::ScudStorm)
                .first()
                .map(|s| s.activate_frame)
        })
        .unwrap_or(0);
    let last = multi_strike_last_impact_frame(
        HostSuperweaponKind::ScudStorm,
        activate,
        ArtilleryBarrageScienceTier::Level1,
    );
    game_logic.frame = last;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::ScudStorm),
        "SCUD Storm host path must complete"
    );
    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::ScudStorm);
    assert_eq!(completed.len(), 1);
    assert!(completed[0].objects_hit >= 1);
    assert!(completed[0].total_damage_applied > 0.0);
    assert!(
        completed[0].multi_strike_applied >= 9,
        "ClipSize 9 multi-missile residual must apply all missiles"
    );
    assert!(
        game_logic.special_power_strikes().honesty_toxin_ok(),
        "ScudStorm must spawn LargePoisonField residual"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "ScudStormImpact"),
        "SCUD impact audio residual required"
    );
}

/// Residual: ParticleCannon continuous beam host path
/// (charge residual → beam field spawn → multi-pulse damage).
#[test]
fn particle_cannon_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, PARTICLE_BEAM_AUDIO, PARTICLE_BEAM_DAMAGE_PER_PULSE,
        PARTICLE_BEAM_TICK_INTERVAL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    // First beam pulse swath epicenter residual (pulse 0 walks -half Swath distance).
    let beam_target = Vec3::new(10.0, 0.0, 0.0);
    let first_pulse_pos =
        beam_target + crate::game_logic::special_power_strikes::particle_swath_offset(0);
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, first_pulse_pos)
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Survive first pulse so multi-pulse residual is observable.
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon,
            target: PowerTarget::Location(beam_target),
        },
        player_id: 1, // Team::China
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(game_logic
        .special_power_strikes()
        .honesty_queue_ok(HostSuperweaponKind::ParticleCannon));
    assert!(
        game_logic.special_power_strikes().beam_fields().is_empty(),
        "beam must not spawn before charge residual completes"
    );

    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 119;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no beam damage before charge residual frame 120"
    );

    // Beam start: field spawn + first pulse.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 120;
    game_logic.update_special_power_strikes();
    game_logic.update_special_power_strikes();
    assert!(game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::ParticleCannon));
    assert!(
        game_logic.special_power_strikes().honesty_beam_ok(),
        "ParticleCannon must spawn continuous beam residual"
    );
    assert!(game_logic
        .special_power_strikes()
        .honesty_host_path_ok(HostSuperweaponKind::ParticleCannon));
    assert!(
        game_logic.special_power_strikes().honesty_beam_damage_ok(),
        "first beam pulse must apply residual damage"
    );

    let after_first = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let first_dealt = test_observed_damage_to(enemy_id, health_before, after_first);
    assert!(
        first_dealt > 0.0 || after_first < health_before,
        "enemy must take first continuous beam pulse (before={health_before}, after={after_first}, dealt={first_dealt})"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == PARTICLE_BEAM_AUDIO
                || e.event_type == "ParticleCannonBeamStart"
                || e.event_type == "SuperweaponParticleCannon"),
        "beam residual must queue activation/beam audio"
    );

    // Second pulse after tick interval.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 120 + PARTICLE_BEAM_TICK_INTERVAL_FRAMES;
    game_logic.update_special_power_strikes();
    let after_second = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let second_dealt = test_observed_damage_to(enemy_id, after_first, after_second);
    assert!(
        second_dealt > 0.0 || after_second < after_first,
        "second continuous beam pulse must apply more damage (first={after_first}, second={after_second}, dealt={second_dealt})"
    );
    let _ = PARTICLE_BEAM_DAMAGE_PER_PULSE;
}

/// Residual: CleanupArea clears toxin/radiation fields + mines at location.

#[test]

fn cleanup_stream_projectile_flies_and_clears() {
    use crate::game_logic::host_cleanup_area::{
        cleanup_stream_flight_frames, CLEANUP_STREAM_MISSILE_FUEL_FRAMES, HOST_CLEANUP_PROJECTILE,
        HOST_CLEANUP_PROJECTILE_STREAM,
    };
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);

    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    logic.templates.insert("USA_Ambulance".to_string(), amb_tpl);

    let amb = logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let anthrax_caster = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("anthrax caster");
    let aim = Vec3::new(20.0, 0.0, 0.0);
    let strike_id = logic.special_power_strikes_mut().queue(
        HostSuperweaponKind::AnthraxBomb,
        anthrax_caster,
        Team::GLA,
        aim,
        0,
    );
    logic.frame = 90;
    logic
        .special_power_strikes_mut()
        .record_impact_complete(strike_id, 0.0, 0, 0);
    let fields_before = logic.special_power_strikes().toxin_fields().len();
    assert!(fields_before > 0, "seed toxin field");

    assert!(logic.activate_cleanup_area(0, aim, Some(amb)));
    assert!(logic.honesty_cleanup_stream_projectile_ok());
    let snap = logic.projectile_stream_snapshot();
    assert!(
        snap.iter().any(|(sid, name, pts, _)| {
            *sid == amb && name == HOST_CLEANUP_PROJECTILE_STREAM && !pts.is_empty()
        }),
        "CleanupHazardProjectileStream residual should register points"
    );
    assert_eq!(
        logic.special_power_strikes().toxin_fields().len(),
        fields_before,
        "toxin should remain until stream impact"
    );

    let max_steps = cleanup_stream_flight_frames(20.0)
        .saturating_add(CLEANUP_STREAM_MISSILE_FUEL_FRAMES)
        .max(20);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_cleanup_stream_projectiles();
        if !logic
            .objects
            .values()
            .any(|o| o.cleanup_stream_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    assert!(
        logic.special_power_strikes().toxin_fields().is_empty()
            || logic.honesty_cleanup_area_clear_ok(),
        "cleanup stream impact should clear toxin residual"
    );
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.cleanup_stream_projectile && o.is_alive()),
        "cleanup projectile should detonate"
    );
    let _ = HOST_CLEANUP_PROJECTILE;
}

#[test]
fn cleanup_area_residual_clears_hazards_and_mines() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_cleanup_area::{
        CLEANUP_AREA_ACTIVATE_AUDIO, HOST_CLEANUP_AREA_RADIUS,
    };
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, ANTHRAX_TOXIN_DAMAGE_PER_TICK,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // Residual ambulance caster template.
    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), amb_tpl);

    let caster_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Spawn anthrax toxin residual via strike, then clear with CleanupArea.
    let anthrax_caster = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("anthrax caster");
    let strike_id = game_logic.special_power_strikes_mut().queue(
        HostSuperweaponKind::AnthraxBomb,
        anthrax_caster,
        Team::GLA,
        Vec3::new(20.0, 0.0, 0.0),
        0,
    );
    // Force complete impact to spawn toxin field at (20,0,0).
    game_logic.frame = 90;
    game_logic
        .special_power_strikes_mut()
        .record_impact_complete(strike_id, 0.0, 0, 0);
    assert_eq!(
        game_logic.special_power_strikes().toxin_fields().len(),
        1,
        "setup: toxin field must exist before cleanup"
    );

    // Place enemy land mine near cleanup target.
    let mine_id = game_logic
        .place_land_mine(Team::GLA, Vec3::new(15.0, 0.0, 0.0), Some(anthrax_caster))
        .expect("mine");
    assert!(game_logic
        .host_object(mine_id)
        .and_then(|o| o.mine_data.as_ref())
        .is_some());

    let target = Vec3::new(20.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::Location(target),
        },
        player_id: 0, // Team::USA
        command_id: 88,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // CleanupStreamProjectile residual: advance flight to impact clear.
    if game_logic.cleanup_stream_missiles_spawned > 0 {
        for _ in 0..40 {
            game_logic.frame = game_logic.frame.saturating_add(1);
            game_logic.update_cleanup_stream_projectiles();
            if !game_logic
                .objects
                .values()
                .any(|o| o.cleanup_stream_projectile && o.is_alive())
            {
                break;
            }
        }
        game_logic.process_destroy_list();
    }

    assert!(
        game_logic.honesty_cleanup_area_activate_ok()
            || game_logic.honesty_cleanup_stream_projectile_ok(),
        "CleanupArea must record activation"
    );
    assert!(
        game_logic.honesty_cleanup_area_clear_ok(),
        "CleanupArea must clear at least one residual hazard/mine"
    );
    assert!(
        game_logic.honesty_cleanup_area_ok(),
        "CleanupArea host path honesty"
    );
    assert!(
        game_logic.special_power_strikes().toxin_fields().is_empty(),
        "toxin field in radius must be cleared"
    );
    // Mine disarmed (detonated residual bookkeeping) and queued for destroy.
    let mine_disarmed = game_logic
        .host_object(mine_id)
        .and_then(|o| o.mine_data.as_ref())
        .map(|d| d.detonated)
        .unwrap_or(true);
    assert!(
        mine_disarmed,
        "enemy mine in cleanup radius must be disarmed"
    );
    game_logic.process_destroy_list();
    assert!(
        game_logic.host_object(mine_id).is_none()
            || game_logic
                .host_object(mine_id)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(true),
        "disarmed mine must leave destroy residual"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == CLEANUP_AREA_ACTIVATE_AUDIO),
        "cleanup must queue detox audio"
    );
    assert!(
        (game_logic.cleanup_areas().activations()[0].radius - HOST_CLEANUP_AREA_RADIUS).abs() < 0.1
    );
    let _ = ANTHRAX_TOXIN_DAMAGE_PER_TICK;
}

/// Residual: CleanupArea does not queue superweapon strikes.
#[test]
fn cleanup_area_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), amb_tpl);
    let caster_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::Location(Vec3::new(10.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 89,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().pending_count(),
        0,
        "CleanupArea must not enqueue superweapon residual strikes"
    );
    assert!(game_logic.honesty_cleanup_area_activate_ok());
}

/// Residual: NuclearMissile (China NeutronMissile) queues delayed area damage
/// and spawns residual radiation field after impact.
/// Fail-closed: not full OCL flight / multi-blast SlowDeath / cleanup-hazard.
#[test]
fn nuclear_missile_host_path_queues_damage_after_delay_and_radiation() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, NUKE_RADIATION_AUDIO, NUKE_RADIATION_DAMAGE_PER_TICK,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    // Survivor for radiation residual: far enough to avoid lethal blast but
    // inside radiation radius (200). Blast outer = 210, max 3500 at ≤60.
    // Place at ~150: mid falloff still high; use high HP survivor that lives
    // past blast if outside inner, then takes radiation ticks.
    let rad_victim_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("rad victim");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(800.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 800.0;
        enemy.health.maximum = 800.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let v = game_logic
            .host_object_mut(rad_victim_id)
            .expect("rad victim");
        // Blast falloff at 150: inner 60, outer 210 → t=(150-60)/150=0.6 → dmg=3500*0.4=1400
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(40.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::NuclearMissile,
            target: PowerTarget::Location(target),
        },
        player_id: 1, // Team::China
        command_id: 50,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::NuclearMissile),
        "NuclearMissile must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponNuclearMissile"),
        "activation must queue SuperweaponNuclearMissile audio"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .radiation_fields()
            .is_empty(),
        "radiation must not spawn before impact"
    );

    // Before impact delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let rad_before = game_logic
        .host_object(rad_victim_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = 179;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no blast damage before impact frame 180"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::NuclearMissile));

    // At impact: blast + radiation field spawn + first radiation tick.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 180;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::NuclearMissile),
        "NuclearMissile must complete on impact frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_radiation_ok(),
        "NuclearMissile must spawn residual radiation"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::NuclearMissile),
        "host path honesty requires complete blast + radiation spawn"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .neutron_slow_death_field_count()
            >= 1,
        "NuclearMissile impact must arm NeutronMissileSlowDeath multi-blast residual"
    );

    // Advance multi-blast residual through Blast6 (~1180ms @ 30 FPS).
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_neutron_slow_death_fields();
        game_logic.update_nuclear_radiation_fields();
    }

    let enemy_after = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after.unwrap_or(0.0));
    assert!(
        enemy_dealt + 0.1 >= health_before
            || enemy_after.is_none()
            || enemy_after == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed || !o.is_alive())
                .unwrap_or(true),
        "enemy at epicenter must take lethal NuclearMissile multi-blast residual damage (dealt={enemy_dealt}, after={enemy_after:?})"
    );

    // Radiation victim took multi-blast falloff and/or radiation ticks.
    let rad_after = game_logic
        .host_object(rad_victim_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let rad_dealt = test_observed_damage_to(rad_victim_id, rad_before, rad_after);
    assert!(
        rad_dealt > 0.0 || rad_after < rad_before,
        "mid-radius victim must take multi-blast and/or radiation damage (before={rad_before}, after={rad_after}, dealt={rad_dealt})"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside blast/radiation radius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "NuclearMissileImpact"),
        "impact must queue NuclearMissileImpact audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == NUKE_RADIATION_AUDIO),
        "impact must queue radiation ambient residual"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    // Second radiation tick after interval: more residual damage if still alive.
    let rad_mid = game_logic
        .host_object(rad_victim_id)
        .map(|o| o.health.current);
    if let Some(mid_hp) = rad_mid {
        crate::game_logic::host_damage_log::clear();
        // Frame is already past impact + multi-blast advance; step one radiation interval.
        game_logic.frame = game_logic.frame.saturating_add(23);
        game_logic.update_special_power_strikes();
        game_logic.update_nuclear_radiation_fields();
        let rad_later = game_logic
            .host_object(rad_victim_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        let tick_dealt = test_observed_damage_to(rad_victim_id, mid_hp, rad_later);
        assert!(
            tick_dealt + 0.1 >= NUKE_RADIATION_DAMAGE_PER_TICK * 0.5
                || rad_later < mid_hp - NUKE_RADIATION_DAMAGE_PER_TICK * 0.5
                || rad_later == 0.0
                || game_logic.host_object(rad_victim_id).is_none(),
            "second radiation tick must apply residual damage (mid={mid_hp}, later={rad_later}, dealt={tick_dealt})"
        );
        assert!(
            game_logic
                .special_power_strikes()
                .honesty_radiation_damage_ok(),
            "radiation damage honesty after tick"
        );
    }

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::NuclearMissile);
    assert_eq!(completed.len(), 1);
    // Instant impact hits suppressed; multi-blast residual applies damage.
    assert_eq!(
        completed[0].phase,
        crate::game_logic::special_power_strikes::HostStrikePhase::Completed
    );
    assert!(
        game_logic
            .special_power_strikes()
            .neutron_slow_death_spawned_total()
            >= 1
            || game_logic
                .special_power_strikes()
                .neutron_slow_death_field_count()
                >= 1
            || completed[0].objects_hit >= 1
            || completed[0].total_damage_applied > 0.0,
        "nuclear path must arm multi-blast residual or record blast damage"
    );

    game_logic.process_destroy_list();
}

/// Residual: SpectreGunship (USA SPECIAL_SPECTRE_GUNSHIP) queues delayed orbit
/// insertion then periodic damage ticks in AttackAreaRadius.
/// Fail-closed: not full SpectreGunshipUpdate OCL / gattling / howitzer projectile.
#[test]
fn spectre_gunship_host_path_queues_orbit_damage_over_time() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, SPECTRE_GATTLING_DAMAGE, SPECTRE_ORBIT_AUDIO,
        SPECTRE_ORBIT_DAMAGE_PER_TICK, SPECTRE_ORBIT_TICK_INTERVAL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_SpectreGunshipSolo");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(800.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Enough HP for multiple howitzer residual ticks.
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(40.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpectreGunship,
            target: PowerTarget::Location(target),
        },
        player_id: 0, // Team::USA (from_player_id)
        command_id: 60,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::SpectreGunship),
        "SpectreGunship must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponSpectreGunship"),
        "activation must queue SuperweaponSpectreGunship audio"
    );
    assert!(
        game_logic.special_power_strikes().orbit_fields().is_empty(),
        "orbit field must not spawn before insertion frame"
    );

    // Before insertion delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 89;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no orbit damage before insertion frame 90"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::SpectreGunship));

    // At insertion: orbit field spawn + first damage tick.
    game_logic.frame = 90;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::SpectreGunship),
        "SpectreGunship must complete on insertion frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_orbit_ok(),
        "SpectreGunship must spawn residual orbit field"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::SpectreGunship),
        "host path honesty requires complete insertion + orbit spawn"
    );

    let enemy_after = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after);
    assert!(
        enemy_dealt > 0.0 || enemy_after < health_before,
        "enemy in orbit radius must take residual howitzer tick damage (before={health_before}, after={enemy_after}, dealt={enemy_dealt})"
    );
    // First insertion tick: howitzer (80 in r25) + gattling (90 nearest).
    let expected_first = SPECTRE_ORBIT_DAMAGE_PER_TICK + SPECTRE_GATTLING_DAMAGE;
    assert!(
        (enemy_dealt - expected_first).abs() < 0.1
            || (health_before - enemy_after - expected_first).abs() < 0.1
            || enemy_after == 0.0,
        "first tick damage should match howitzer+gattling residual (before={health_before}, after={enemy_after}, dealt={enemy_dealt})"
    );
    assert!(
        game_logic.special_power_strikes().honesty_gattling_ok(),
        "Spectre gattling residual honesty"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside AttackAreaRadius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SpectreGunshipVoiceArrive"),
        "insertion must queue SpectreGunshipVoiceArrive audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == SPECTRE_ORBIT_AUDIO),
        "insertion must queue Spectre orbit ambient residual"
    );

    // Second orbit tick after interval: more residual damage over time.
    let mid_hp = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .expect("enemy still alive for second tick");
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES;
    game_logic.update_special_power_strikes();
    let later_hp = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let tick_dealt = test_observed_damage_to(enemy_id, mid_hp, later_hp);
    assert!(
        tick_dealt + 0.1 >= SPECTRE_ORBIT_DAMAGE_PER_TICK * 0.5
            || later_hp < mid_hp - SPECTRE_ORBIT_DAMAGE_PER_TICK * 0.5
            || later_hp == 0.0
            || game_logic.host_object(enemy_id).is_none(),
        "second orbit tick must apply residual damage over time (mid={mid_hp}, later={later_hp}, dealt={tick_dealt})"
    );
    assert!(
        game_logic.special_power_strikes().honesty_orbit_damage_ok(),
        "orbit damage honesty after tick"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::SpectreGunship);
    assert_eq!(completed.len(), 1);
    // No one-shot impact blast; orbit residual owns damage applications.
    assert!(
        game_logic
            .special_power_strikes()
            .orbit_damage_applications_total()
            >= 2
    );

    game_logic.process_destroy_list();
}
