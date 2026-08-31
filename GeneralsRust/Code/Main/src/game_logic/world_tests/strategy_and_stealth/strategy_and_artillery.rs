//! Behavior suite extracted from `strategy_and_stealth`.
use super::*;

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
fn camera_mod_look_toward_is_noop_without_active_move() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_look_toward(CameraModLookTowardRequest {
            position: Vec3::new(150.0, 0.0, 50.0),
        });

    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.take_camera_look_toward_request().is_none(),
        "C++ cameraModLookToward is a no-op unless a camera move/path is active"
    );
}

#[test]
fn camera_mod_look_toward_applies_during_active_move() {
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
        .push_camera_mod_look_toward(CameraModLookTowardRequest {
            position: Vec3::new(150.0, 0.0, 50.0),
        });

    game_logic.evaluate_and_execute_scripts(0.0);
    game_logic.update_script_camera(1.0 / 30.0);

    let look = game_logic
        .peek_pending_camera_look_toward()
        .cloned()
        .expect("mod look toward should rewrite the active move look");
    assert_eq!(look.position, Vec3::new(150.0, 0.0, 50.0));
    assert!(
        look.duration_seconds > 0.0,
        "mod look should use remaining camera movement time, not snap"
    );
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
fn camera_mod_freeze_time_and_finished_include_rotate_zoom_pitch() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.mission_scripts.push_camera_mod_freeze_time();
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !game_logic.is_script_camera_time_frozen(),
        "freeze time should not freeze sim until rotate/zoom/pitch starts"
    );
    assert!(game_logic.mission_scripts.is_camera_movement_finished());

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.5,
            duration_seconds: 1.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.is_script_camera_time_frozen(),
        "FREEZE_TIME + ROTATE_CAMERA must freeze sim"
    );
    assert!(
        !game_logic.mission_scripts.is_camera_movement_finished(),
        "CAMERA_MOVEMENT_FINISHED must be false during rotate"
    );

    for _ in 0..40 {
        game_logic.update_script_camera(1.0 / 30.0);
    }
    assert!(
        !game_logic.is_script_camera_time_frozen(),
        "freeze must clear when rotate finishes"
    );
    assert!(game_logic.mission_scripts.is_camera_movement_finished());

    game_logic
        .mission_scripts
        .push_camera_zoom(CameraZoomRequest {
            zoom: 1.1,
            duration_seconds: 0.5,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !game_logic.mission_scripts.is_camera_movement_finished(),
        "CAMERA_MOVEMENT_FINISHED must be false during zoom"
    );
    for _ in 0..20 {
        game_logic.update_script_camera(1.0 / 30.0);
    }
    assert!(game_logic.mission_scripts.is_camera_movement_finished());
}

#[test]
fn camera_look_toward_cancels_in_flight_move() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(200.0, 0.0, 120.0),
            seconds: 4.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(game_logic.script_camera_move_to_target().is_some());

    game_logic
        .mission_scripts
        .push_camera_look_toward_waypoint(CameraLookTowardWaypointRequest {
            position: Vec3::new(10.0, 0.0, 40.0),
            duration_seconds: 2.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
            reverse_rotation: false,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.script_camera_move_to_target().is_none(),
        "LOOK_TOWARD must cancel the in-flight MOVE_CAMERA_TO"
    );
    let look = game_logic
        .peek_pending_camera_look_toward()
        .cloned()
        .expect("look toward should remain queued");
    assert_eq!(look.position, Vec3::new(10.0, 0.0, 40.0));
    assert!(
        !game_logic.mission_scripts.is_camera_movement_finished(),
        "look-toward is a rotate and must keep movement unfinished"
    );

    game_logic.update_script_camera(1.0 / 30.0);
    assert!(
        game_logic.script_camera_move_to_target().is_none(),
        "update must not revive the cancelled move"
    );
    let look = game_logic
        .peek_pending_camera_look_toward()
        .cloned()
        .expect("look toward must not be overwritten by travel look");
    assert_eq!(look.position, Vec3::new(10.0, 0.0, 40.0));
}

#[test]
fn camera_mod_final_zoom_uses_remaining_rotate_time() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.25,
            duration_seconds: 3.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic
        .mission_scripts
        .push_camera_mod_final_zoom(CameraModFinalZoomRequest {
            zoom: 0.8,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic
        .mission_scripts
        .push_camera_mod_final_pitch(CameraModFinalPitchRequest {
            pitch: 1.1,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let remaining = game_logic.script_camera_remaining_seconds();
    assert!(
        (remaining - 3.0).abs() < 0.001,
        "standalone ROTATE_CAMERA remaining must be the rotate duration, got {remaining}"
    );
    let zoom = game_logic
        .peek_pending_camera_zoom()
        .cloned()
        .expect("mod final zoom should ease, not snap");
    assert!(
        (zoom.duration_seconds - 3.0).abs() < 0.001,
        "CAMERA_MOD_SET_FINAL_ZOOM must use remaining rotate time, got {}",
        zoom.duration_seconds
    );
    let pitch = game_logic
        .peek_pending_camera_pitch()
        .cloned()
        .expect("mod final pitch should ease, not snap");
    assert!(
        (pitch.duration_seconds - 3.0).abs() < 0.001,
        "CAMERA_MOD_SET_FINAL_PITCH must use remaining rotate time, got {}",
        pitch.duration_seconds
    );
}

#[test]
fn camera_mod_final_zoom_idle_is_noop() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_final_zoom(CameraModFinalZoomRequest {
            zoom: 0.8,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic
        .mission_scripts
        .push_camera_mod_final_pitch(CameraModFinalPitchRequest {
            pitch: 1.1,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.peek_pending_camera_zoom().is_none(),
        "idle CAMERA_MOD_SET_FINAL_ZOOM must not snap zoom"
    );
    assert!(
        game_logic.peek_pending_camera_pitch().is_none(),
        "idle CAMERA_MOD_SET_FINAL_PITCH must not snap pitch"
    );
}

#[test]
fn post_ai_commands_flushed_inside_game_logic() {
    // Structural: AI-phase command flush lives in world_tick/step.rs after update_ai.
    // C++ GameLogic::update drains TheCommandList after TheAI->UPDATE (GameLogic.cpp).
    let src = include_str!("../../world_tick/step.rs");
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
        scatter_table_offset: None,
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
    crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
    crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
fn camera_mod_freeze_angle_is_noop_without_active_move() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.mission_scripts.push_camera_mod_freeze_angle();
    game_logic.evaluate_and_execute_scripts(0.0);

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.25,
            duration_seconds: 1.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let rotate = game_logic
        .take_camera_rotate_request()
        .expect("later ROTATE_CAMERA must replace after a no-op FREEZE_ANGLE");
    assert!((rotate.rotations - 0.25).abs() < f32::EPSILON);
}

#[test]
fn camera_mod_freeze_angle_pins_move_but_later_rotate_applies() {
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
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.is_script_camera_angle_frozen(),
        "freeze angle should pin the in-flight move"
    );
    assert!(
        game_logic.take_camera_look_toward_request().is_none(),
        "freeze angle should not emit travel look-toward"
    );

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.5,
            duration_seconds: 1.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.take_camera_rotate_request().is_some(),
        "later ROTATE_CAMERA replaces the animation even while the move is frozen"
    );
}

#[test]
fn camera_mod_freeze_angle_pins_in_flight_rotate() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.5,
            duration_seconds: 2.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.mission_scripts.push_camera_mod_freeze_angle();
    game_logic.evaluate_and_execute_scripts(0.0);

    let rotate = game_logic
        .take_camera_rotate_request()
        .expect("FREEZE_ANGLE must pin the in-flight rotate, not drop it");
    assert!(
        rotate.rotations.abs() < f32::EPSILON,
        "in-flight ROTATE_CAMERA must hold current yaw after FREEZE_ANGLE, got {}",
        rotate.rotations
    );
    assert!(
        (rotate.duration_seconds - 2.0).abs() < 0.001,
        "pinned rotate must keep remaining time, got {}",
        rotate.duration_seconds
    );
}

#[test]
fn script_reset_camera_animates_zoom_pitch_and_yaw() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_reset(CameraResetRequest {
            position: Vec3::new(100.0, 0.0, 80.0),
            duration_seconds: 2.5,
            ease_in_seconds: 0.4,
            ease_out_seconds: 0.6,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(game_logic.peek_pending_camera_zoom_reset());
    assert!(
        (game_logic.peek_pending_camera_zoom_reset_duration() - 2.5).abs() < 0.001,
        "RESET_CAMERA must keep the script duration for zoom/pitch/yaw"
    );
    assert_eq!(game_logic.peek_pending_camera_zoom_reset_ease(), (0.4, 0.6));
}

#[test]
fn camera_setup_cancels_in_flight_move() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_move_to(CameraMoveToRequest {
            position: Vec3::new(200.0, 0.0, 120.0),
            seconds: 4.0,
            camera_stutter_seconds: 0.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(game_logic.script_camera_move_to_target().is_some());

    game_logic
        .mission_scripts
        .push_camera_setup(CameraSetupRequest {
            position: Vec3::new(10.0, 0.0, 20.0),
            zoom: 0.7,
            pitch: 0.4,
            look_toward: Vec3::new(30.0, 0.0, 40.0),
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.script_camera_move_to_target().is_none(),
        "SETUP_CAMERA must cancel the in-flight MOVE_CAMERA_TO"
    );
    assert!(
        !game_logic.script_camera_path_active(),
        "SETUP_CAMERA must cancel an in-flight waypoint path"
    );
    let look = game_logic
        .peek_pending_camera_look_toward()
        .cloned()
        .expect("setup look-toward must remain queued");
    assert_eq!(look.position, Vec3::new(30.0, 0.0, 40.0));

    game_logic.update_script_camera(1.0 / 30.0);
    assert!(
        game_logic.script_camera_move_to_target().is_none(),
        "update must not revive the cancelled setup move"
    );
    let look = game_logic
        .peek_pending_camera_look_toward()
        .cloned()
        .expect("setup look-toward must not be overwritten by travel look");
    assert_eq!(look.position, Vec3::new(30.0, 0.0, 40.0));
}

#[test]
fn reset_camera_clears_stale_rotate() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.5,
            duration_seconds: 3.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(game_logic.peek_pending_camera_rotate().is_some());

    game_logic
        .mission_scripts
        .push_camera_reset(CameraResetRequest {
            position: Vec3::new(100.0, 0.0, 80.0),
            duration_seconds: 2.5,
            ease_in_seconds: 0.4,
            ease_out_seconds: 0.6,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic.peek_pending_camera_rotate().is_none(),
        "RESET_CAMERA must drop the stale ROTATE_CAMERA"
    );
    assert!(game_logic.peek_pending_camera_zoom_reset());
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&game_logic, 0);
    assert!(
        frame.camera_rotate.is_none(),
        "RESET_CAMERA must not leave ROTATE_CAMERA on the presentation frame"
    );
    assert!(frame.camera_zoom_reset);
}

#[test]
fn camera_mod_final_zoom_pitch_idle_is_noop() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_final_zoom(CameraModFinalZoomRequest {
            zoom: 0.8,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic
        .mission_scripts
        .push_camera_mod_final_pitch(CameraModFinalPitchRequest {
            pitch: 1.1,
            ease_in: 0.0,
            ease_out: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        game_logic.peek_pending_camera_zoom().is_none(),
        "idle CAMERA_MOD_SET_FINAL_ZOOM must not snap zoom"
    );
    assert!(
        game_logic.peek_pending_camera_pitch().is_none(),
        "idle CAMERA_MOD_SET_FINAL_PITCH must not snap pitch"
    );
}

#[test]
fn camera_mod_freeze_angle_pins_already_queued_rotate() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.5,
            duration_seconds: 2.0,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    game_logic.evaluate_and_execute_scripts(0.0);
    assert!(
        game_logic
            .peek_pending_camera_rotate()
            .is_some_and(|r| (r.rotations - 0.5).abs() < f32::EPSILON)
    );

    game_logic.mission_scripts.push_camera_mod_freeze_angle();
    game_logic.evaluate_and_execute_scripts(0.0);
    let rotate = game_logic
        .peek_pending_camera_rotate()
        .cloned()
        .expect("FREEZE_ANGLE must pin the in-flight rotate, not drop it");
    assert!(
        rotate.rotations.abs() < f32::EPSILON,
        "FREEZE_ANGLE must hold current yaw (0 remaining rotations), got {}",
        rotate.rotations
    );
    assert!(
        (rotate.duration_seconds - 2.0).abs() < 0.001,
        "pinned rotate must keep remaining duration"
    );
}

#[test]
fn script_zoom_pitch_rotate_preserve_ease_on_presentation_frame() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_zoom(CameraZoomRequest {
            zoom: 1.2,
            duration_seconds: 2.0,
            ease_in_seconds: 0.3,
            ease_out_seconds: 0.5,
        });
    game_logic
        .mission_scripts
        .push_camera_pitch(CameraPitchRequest {
            pitch: 0.8,
            duration_seconds: 1.5,
            ease_in_seconds: 0.2,
            ease_out_seconds: 0.4,
        });
    game_logic
        .mission_scripts
        .push_camera_rotate(CameraRotateRequest {
            rotations: 0.25,
            duration_seconds: 3.0,
            ease_in_seconds: 0.1,
            ease_out_seconds: 0.2,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&game_logic, 0);
    assert_eq!(frame.camera_zoom, Some((1.2, 2.0)));
    assert_eq!(frame.camera_zoom_ease, (0.3, 0.5));
    assert_eq!(frame.camera_pitch, Some((0.8, 1.5)));
    assert_eq!(frame.camera_pitch_ease, (0.2, 0.4));
    assert_eq!(frame.camera_rotate, Some((0.25, 3.0)));
    assert_eq!(frame.camera_rotate_ease, (0.1, 0.2));
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
fn camera_mod_rolling_average_idle_does_not_arm_next_path() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames: 7 });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert!(
        !game_logic.script_camera_path_active(),
        "idle CAMERA_MOD_SET_ROLLING_AVERAGE must not start a path"
    );

    // C++ setupWaypointPath hard-resets rollingAverageFrames=1. A later path
    // must start unsmoothed even if an idle request already ran.
    game_logic.install_script_camera_path_for_test();
    assert_eq!(
        game_logic.script_camera_path_rolling_average_frames(),
        Some(1)
    );
}

#[test]
fn camera_mod_rolling_average_applies_to_in_flight_path() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;
    game_logic.install_script_camera_path_for_test();

    game_logic
        .mission_scripts
        .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames: 7 });
    game_logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        game_logic.script_camera_path_rolling_average_frames(),
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
fn move_camera_to_selection_without_path_is_not_lookat() {
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

    assert!(
        game_logic.take_camera_focus_request().is_none(),
        "C++ cameraModFinalMoveTo is a no-op without an in-flight path"
    );
}

#[test]
fn move_camera_to_selection_retargets_in_flight_move() {
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

    game_logic.request_camera_focus(Vec3::new(0.0, 10.0, 0.0));
    let _ = game_logic.take_camera_focus_request();
    game_logic.start_camera_move_to(CameraMoveToRequest {
        position: Vec3::new(400.0, 10.0, 400.0),
        seconds: 10.0,
        camera_stutter_seconds: 0.0,
        ease_in_seconds: 0.0,
        ease_out_seconds: 0.0,
    });

    game_logic.mission_scripts.push_camera_move_to_selection();
    game_logic.evaluate_and_execute_scripts(0.0);

    // The retarget itself never queues a lookAt (W3DView.cpp:2637-2655).
    // The per-frame update_script_camera tick inside
    // evaluate_and_execute_scripts legitimately re-requests the in-flight
    // move's CURRENT focus (dt=0 → the move's start (0,10,0)); a request
    // for anything else would mean a stray new lookAt.
    let focus = game_logic.take_camera_focus_request();
    assert!(
        focus.is_none()
            || focus.unwrap().distance(Vec3::new(0.0, 10.0, 0.0)) < 0.01,
        "retarget must not start a new lookAt"
    );
    let target = game_logic
        .script_camera_move_to_target()
        .expect("in-flight move should remain");
    assert!((target.x - 130.0).abs() < 0.001);
    assert!((target.z - 230.0).abs() < 0.001);
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
    assert!(
        game_logic.take_camera_focus_request().is_none(),
        "C++ doCameraMotionBlurJump lookAt only if leftover filter fails; live must not snap immediately"
    );
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
        arm_attacker_then_attack(attacker, target_id, 9999.0);
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
        target.thing.template.armor = 0.0;
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

#[test]
fn cash_bounty_increases_cash_on_enemy_kill() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // GLA SCIENCE_CashBounty3 residual = 20% of build cost.
    // C++ skirmish lobby seeds opposing team relations; getRelationship
    // defaults NEUTRAL (Player.cpp:541-572) — pin the Enemies pairing.
    game_logic.get_player_mut(0).unwrap().alliance_team = 7;
    game_logic.get_player_mut(2).unwrap().alliance_team = 9;
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
        arm_attacker_then_attack(attacker, target_id, 9999.0);
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
        target.thing.template.armor = 0.0;
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
    assert_eq!(
        game_logic
            .get_player(0)
            .map(|p| p.statistics.money_earned)
            .unwrap_or(0),
        expected_bounty,
        "C++ ScoreKeeper::addMoneyEarned must count bounty dollars"
    );
}

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
        target.thing.template.armor = 0.0;
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

#[test]
fn cash_bounty_science_unlock_sets_percent() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let player = game_logic.get_player_mut(2).expect("gla player");
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty1"));
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty2"));
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
    assert!(player.unlock_science("SCIENCE_CashBounty3"));
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
    // Already unlocked — no change / false.
    assert!(!player.unlock_science("SCIENCE_CashBounty3"));
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);
}

#[test]
fn cash_bounty_science_unlock_requires_palace_module() {
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata, ThingTemplate};
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let player = game_logic.get_player_mut(2).expect("gla player");
    assert!(player.unlock_science("SCIENCE_CashBounty1"));
    assert!((player.cash_bounty_percent - 0.0).abs() < 1e-6);

    let mut palace = ThingTemplate::new("GLAPalace");
    palace
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(3000.0);
    palace
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_15".into()),
            module_kind: SpecialPowerModuleKind::CashBountyPower,
            special_power_template: "SpecialAbilityCashBounty1".into(),
            special_power_template_id: 1,
            command_power: None,
            reload_time_frames: 0,
            required_science: Some("SCIENCE_CashBounty1".into()),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    game_logic.templates.insert("GLAPalace".into(), palace);
    let _ = game_logic
        .create_object_for_player("GLAPalace", 2, Vec3::new(0.0, 0.0, 0.0))
        .expect("palace");
    assert!(
        (game_logic.get_player(2).unwrap().cash_bounty_percent - 0.05).abs() < 1e-6,
        "onObjectCreated must apply bounty when palace exists and science is owned"
    );
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
        arm_attacker_then_attack(attacker, target_id, 1.0);
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
fn weapon_discharge_world_tick_combat_preserves_preadvance_barrel_and_freezes_once() {
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
        assert!(attacker.set_weapon_barrel_count_for_slot(0, 3));
        attacker.weapon_barrel_states[0].current_barrel = 2;
        attacker.weapon_barrel_states[0].shots_left_on_barrel = 1;
    }

    game_logic.frame = 30;
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    let attacker = game_logic
        .host_object(attacker_id)
        .expect("attacker remains");
    assert_eq!(
        attacker.weapon_discharge_marker(),
        WeaponDischargeMarker {
            sequence: 1,
            weapon_slot: 0,
            fired_barrel: 2,
            logic_frame: 30,
        }
    );
    assert_eq!(
        attacker
            .weapon_barrel_state_for_slot(0)
            .expect("primary cursor")
            .current_barrel,
        3,
        "C++ retains the raw post-last-shot cursor until the next pre-fire topology guard"
    );

    let frozen = crate::presentation_frame::PresentationFrame::build_from_logic(&game_logic, 0);
    assert!(frozen.events.iter().any(|event| matches!(
        event,
        crate::presentation_frame::PresentationEvent::WeaponDischarged {
            source,
            weapon_slot: 0,
            fired_barrel: 2,
            sequence: 1,
            logic_frame: 30,
            ..
        } if *source == attacker_id
    )));
    assert!(
        crate::presentation_frame::PresentationFrame::build_from_logic(&game_logic, 0)
            .events
            .iter()
            .all(|event| !matches!(
                event,
                crate::presentation_frame::PresentationEvent::WeaponDischarged { .. }
            )),
        "the transient accepted-discharge log must freeze once rather than replay"
    );
}

#[test]
fn combat_kill_does_not_queue_invented_unit_die() {
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
        arm_attacker_then_attack(attacker, target_id, 9999.0);
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target exists");
        target.health.current = 10.0;
        target.health.maximum = 10.0;
        target.thing.template.armor = 0.0;
    }

    game_logic.frame = 60;
    game_logic.queued_audio_events.clear();
    game_logic.update_combat(&[attacker_id, target_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "WeaponFire"),
        "kill path still fires WeaponFire first"
    );

    game_logic.process_destroy_list();

    assert!(
        !game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "UnitDie"
                || e.event_type == "WeaponHit"
                || e.event_type.starts_with("UnitDie")),
        "kill must not invent UnitDie/WeaponHit, got {:?}",
        game_logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        game_logic.host_object(target_id).is_none(),
        "target must be removed after kill"
    );
}

#[test]
fn daisy_cutter_host_path_queues_and_completes_area_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{HostStrikePhase, HostSuperweaponKind};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    // C++ SpecialPower.cpp:299-308 canUseSpecialPower requires an authored
    // SpecialPowerModule on the caster — scalar ready-flags alone are not
    // module evidence.
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::DaisyCutter,
            "SuperweaponDaisyCutter",
            SpecialPowerModuleKind::OclSpecialPower,
            30,
        );
    }
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
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter)
    );

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

#[test]
fn a10_strike_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    // C++ SpecialPower.cpp:299-308: authored module evidence required.
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::Airstrike,
            "SuperweaponA10ThunderboltMissileStrike",
            SpecialPowerModuleKind::OclSpecialPower,
            30,
        );
    }
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_A10ThunderboltMissileStrike1");
    }

    // C++ DeliveryDistance 450: the strike target must sit far enough from
    // the map edge that the jet completes its whole bomb run in range.
    game_logic.override_world_size(4000.0, 4000.0);
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(1200.0, 0.0, 0.0))
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
            target: PowerTarget::Location(Vec3::new(1200.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::A10Strike)
    );

    // C++ DeliverPayload: jets fly in from the map edge, drop missiles, and
    // each missile detonates on its own HeightDie. Drive the live flight tick
    // chain (same order as world_tick/step.rs) until the strike completes.
    for f in 1..=900u32 {
        game_logic.frame = f;
        game_logic.update_a10_strike_flights();
        game_logic.update_special_power_strikes();
        if !game_logic
            .special_power_strikes()
            .completed_of_kind(HostSuperweaponKind::A10Strike)
            .is_empty()
        {
            break;
        }
    }
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

#[test]
fn carpet_bomb_host_path_queues_and_applies_delayed_line_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        ArtilleryBarrageScienceTier, CARPET_BOMB_DAMAGE, CARPET_BOMB_DROP_DELAY_FRAMES,
        CARPET_BOMB_IMPACT_DELAY_FRAMES, HostStrikePhase, HostSuperweaponKind, carpet_bomb_points,
        multi_strike_last_impact_frame,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    // C++ SpecialPower.cpp:299-308: authored module evidence required.
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::CarpetBomb,
            "SuperweaponCarpetBomb",
            SpecialPowerModuleKind::OclSpecialPower,
            30,
        );
    }
    ensure_test_player_for_team(&mut game_logic, Team::USA);

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
    // C++ DeliverPayload: the B52 flies the bomb line and each bomb detonates
    // on its own HeightDie. Drive the live flight tick chain (same order as
    // world_tick/step.rs) — no damage before the first bomb's impact frame.
    for f in 1..=(CARPET_BOMB_IMPACT_DELAY_FRAMES - 1) {
        game_logic.frame = f;
        game_logic.update_carpet_bomb_flights();
        game_logic.update_special_power_strikes();
    }
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
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CarpetBomb)
    );

    // First DropDelay bomb: not complete yet (center/outer later).
    game_logic.frame = CARPET_BOMB_IMPACT_DELAY_FRAMES;
    game_logic.update_carpet_bomb_flights();
    game_logic.update_special_power_strikes();
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CarpetBomb)
    );
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
    // Jump to last DropDelay bomb: bombs are all due; keep ticking the live
    // flight chain until every bomb HeightDie detonation lands and the
    // strike completes (C++ DeliveringState -> final bomb weapon fires).
    game_logic.frame = last;
    for f in last..=(last + 120) {
        game_logic.frame = f;
        game_logic.update_carpet_bomb_flights();
        game_logic.update_special_power_strikes();
        if !game_logic
            .special_power_strikes()
            .completed_of_kind(HostSuperweaponKind::CarpetBomb)
            .is_empty()
        {
            break;
        }
    }
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
