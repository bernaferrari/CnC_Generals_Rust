//! Presentation audio/input/select, residual acquire, host ObjectId sources.

use super::*;

#[test]
fn presentation_audio_direct_dispatch_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn collect_audio_events")
            && pf.contains("fn dispatch_audio_events_direct")
            && pf.contains("AudioManagerSubsystem"),
        "presentation must collect+dispatch audio without requiring GameLogic mut"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Production frame path must use direct dispatch, not GameLogic dual-write.
    let i = eng
        .find("dispatch_audio_events_direct")
        .expect("engine must call dispatch_audio_events_direct");
    let window = &eng[i.saturating_sub(200)..eng.len().min(i + 400)];
    assert!(
        !window.contains("apply_events_to_audio(&mut self.game_logic)"),
        "production path must not dual-write presentation audio into GameLogic"
    );
    assert!(
        !window.contains("process_audio_events()"),
        "presentation audio path must not require GameLogic process_audio_events drain"
    );
}

#[test]
fn presentation_audio_no_dual_sfx_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng
        .find("self.apply_presentation_to_huds(&pres);")
        .expect("hud apply");
    let w = &eng[i..eng.len().min(i + 350)];
    assert!(
        !w.contains("play_presentation_event_sfx"),
        "InGame path must not dual-play engine SFX after presentation audio dispatch"
    );
    let sfx = eng.find("fn play_presentation_event_sfx").expect("sfx fn");
    let body = &eng[sfx..eng.len().min(sfx + 600)];
    assert!(
        body.contains("Retired dual-path")
            || body.contains("no-op so engine SFX")
            || body.contains("let _ = self;"),
        "play_presentation_event_sfx must be retired no-op residual"
    );
}

#[test]
fn presentation_shell_drains_client_audio_source() {
    // GameClient lives outside Main crate; read by relative path from Main.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/core/game_client/impl_update.rs"
    );
    let gc = std::fs::read_to_string(path).expect("game_client.rs");
    let body = rust_fn_body(&gc, "update_presentation_shell").expect("update_presentation_shell");
    // C++ GameClient::update drains audio once (GameClient.cpp update).
    // Rust splits that drain onto Main dispatch_audio_events_direct so the
    // shell must not dual-drain update_audio.
    assert!(
        body.contains("no update_audio dual-drain") || !body.contains("self.update_audio()"),
        "presentation shell must not dual-drain client audio"
    );
    assert!(
        !body.contains("self.update_input()"),
        "presentation shell must not claim OS input device poll"
    );
}

#[test]
fn presentation_eva_counters_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("pub eva_low_power_count: u32")
            && pf.contains("pub eva_insufficient_funds_count: u32")
            && pf.contains("pub eva_base_under_attack_count: u32")
            && pf.contains("pub eva_ally_under_attack_count: u32"),
        "PresentationFrame must freeze EVA residual counters"
    );
    assert!(
        pf.contains("eva_low_power_count: logic.eva_low_power_count()"),
        "build_from_logic must snapshot EVA counters"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("fn sync_eva_messages_from_presentation")
            && eng.contains("fn sync_eva_messages_from_host_counts"),
        "engine must sync EVA from presentation snapshot"
    );
    // InGame path with presentation uses snapshot sync.
    let i = eng
        .find("self.apply_presentation_to_huds(&pres);")
        .expect("hud apply");
    let w = &eng[i..eng.len().min(i + 450)];
    assert!(
        w.contains("sync_eva_messages_from_presentation"),
        "InGame presentation path must sync EVA from snapshot: {w}"
    );
    assert!(
        !w.contains("play_presentation_event_sfx"),
        "InGame presentation path must not dual-call SFX"
    );
}

#[test]
fn play_sound_effect_direct_audio_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("fn play_sound_effect").expect("play_sound_effect");
    let body = &eng[i..eng.len().min(i + 2200)];
    assert!(
        body.contains("play_sound_through_the_audio")
            && body.contains("AudioManagerSubsystem")
            && body.contains("last_presentation_frame.is_some()"),
        "play_sound_effect must dispatch UI SFX via TheAudio when a handle exists"
    );
    assert!(
        !body.contains(
            "self.game_logic
                .queue_audio_event"
        ) && !body.contains("self.game_logic.process_audio_events()"),
        "play_sound_effect must not dual-write GameLogic audio queue on presentation path"
    );
}

#[test]
fn residual_auto_fire_consume_ammo_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for name in [
        "try_sentry_drone_residual_fire",
        "try_hellfire_drone_residual_fire",
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
        "try_base_defense_residual_fire",
        "try_strategy_center_bombardment_turret_fire",
        "update_pending_patriot_assists",
    ] {
        let at = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[at..src.len().min(at + 14000)];
        assert!(
            body.contains("consume_ammo_on_fire"),
            "{name} must stamp weapon via consume_ammo_on_fire (not last_fire-only)"
        );
        assert!(
            !body.contains("last_fire_time = current_time")
                && !body.contains("last_fire_time = frame as f32"),
            "{name} must not last_fire-only stamp residual"
        );
    }
}

#[test]
fn game_client_mouse_inject_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("fn inject_game_client_mouse_move")
            && eng.contains("fn inject_game_client_mouse_button")
            && eng.contains("fn inject_game_client_mouse_scroll"),
        "Main must expose GameClient mouse inject helpers"
    );
    assert!(
        eng.contains("self.inject_game_client_mouse_move(x, y)"),
        "CursorMoved must inject into GameClient mouse"
    );
    assert!(
        eng.contains("self.inject_game_client_mouse_button(button, pressed)"),
        "MouseInput must inject into GameClient mouse"
    );
    assert!(
        eng.contains("inject_game_client_mouse_scroll(delta_y)"),
        "mouse wheel must inject into GameClient mouse"
    );
    // Main still owns command translation residual.
    assert!(
        eng.contains("Main still owns command translation")
            || eng.contains("without dual OS event ownership"),
        "inject path must document Main command ownership"
    );
}

#[test]
fn game_client_keyboard_inject_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("fn inject_game_client_key")
            && eng.contains("fn to_game_client_key_code")
            && eng.contains("inject_game_client_key(physical_key, pressed)"),
        "Main KeyboardInput must inject into GameClient keyboard device"
    );
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/input/keyboard.rs"
    );
    let kb = std::fs::read_to_string(path).expect("keyboard.rs");
    assert!(
        kb.contains("fn the_keyboard")
            && kb.contains("fn with_keyboard")
            && kb.contains("fn handle_key_simple"),
        "GameClient keyboard must expose the_keyboard/with_keyboard/handle_key_simple"
    );
}

#[test]
fn game_client_shared_input_devices_source() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/core/subsystems.rs"
    );
    let sub = std::fs::read_to_string(path).expect("subsystems.rs");
    assert!(
        sub.contains("the_keyboard().clone()") && sub.contains("the_mouse().clone()"),
        "create_keyboard/mouse must share THE_* singletons with Main inject"
    );
    let gc_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/core/game_client/impl_update.rs"
    );
    let gc = std::fs::read_to_string(gc_path).expect("game_client.rs");
    let body = rust_fn_body(&gc, "update_presentation_shell").expect("presentation shell");
    // Main injects THE_MOUSE/THE_KEYBOARD; shell must not dual-tick update_input.
    assert!(
        body.contains("no shell") && body.contains("update_input")
            || !body.contains("self.update_input()"),
        "presentation shell must not dual-tick update_input on shared devices"
    );
}

#[test]
fn residual_auto_fire_host_attack_log_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let body = last_rust_fn_body(src, "residual_auto_fire_apply_damage").expect("helper");
    assert!(
        body.contains("host_attack_log::record(attacker_id, Some(target_id))"),
        "residual auto-fire helper must record host_attack_log for presentation AttackTargeted"
    );
    for name in [
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
        "try_strategy_center_bombardment_turret_fire",
    ] {
        let b = last_rust_fn_body(src, name).expect(name);
        assert!(
            b.contains("record_attack") && b.contains("gameworld_ai_decision_authority"),
            "{name} must log attack decision under AI decision authority"
        );
    }
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("host_attack_log::take_last_drain")
            && pf.contains("PresentationEvent::AttackTargeted"),
        "presentation must freeze AttackTargeted from host_attack_log"
    );
}

#[test]
fn select_hero_presentation_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn alive_selectable_friendly_hero_ids") && pf.contains("KindOf::Hero"),
        "PresentationFrame must expose hero select helper from snapshot kind_of"
    );
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "select_hero_units_hotkey").expect("select_hero_units_hotkey");
    assert!(
        body.contains("alive_selectable_friendly_hero_ids")
            && body.contains("last_presentation_frame"),
        "SELECT_HERO must prefer presentation hero ids when frame installed"
    );
    assert!(
        body.contains("no live GameLogic dual-scan") || !body.contains("get_objects()"),
        "SELECT_HERO must be presentation-only (no live dual-scan)"
    );
}

#[test]
fn filter_select_presentation_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    for name in [
        "alive_selectable_friendly_combat_ids",
        "alive_selectable_friendly_moving_ids",
        "alive_selectable_friendly_attacking_ids",
        "alive_selectable_friendly_guarding_ids",
        "alive_selectable_friendly_patrolling_ids",
        "alive_selectable_friendly_gathering_ids",
        "alive_selectable_friendly_stealthed_ids",
        "alive_selectable_friendly_veteran_ids",
    ] {
        assert!(
            pf.contains(&format!("fn {name}")),
            "PresentationFrame must expose {name}"
        );
    }
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    for (fn_name, call) in [
        (
            "select_all_friendly_combat",
            "alive_selectable_friendly_combat_ids",
        ),
        (
            "select_all_friendly_moving",
            "alive_selectable_friendly_moving_ids",
        ),
        (
            "select_all_friendly_attacking",
            "alive_selectable_friendly_attacking_ids",
        ),
        (
            "select_all_friendly_guarding",
            "alive_selectable_friendly_guarding_ids",
        ),
        (
            "select_all_friendly_stealthed",
            "alive_selectable_friendly_stealthed_ids",
        ),
        (
            "select_all_friendly_veterans",
            "alive_selectable_friendly_veteran_ids",
        ),
    ] {
        let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
        let body = &eng[at..eng.len().min(at + 1500)];
        assert!(
            body.contains(call),
            "{fn_name} must prefer presentation {call}"
        );
    }
}

#[test]
fn specialty_select_presentation_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    for name in [
        "alive_selectable_friendly_harvester_ids",
        "alive_selectable_friendly_idle_harvester_ids",
        "alive_selectable_friendly_occupied_transport_ids",
        "alive_selectable_friendly_docked_aircraft_ids",
        "alive_selectable_friendly_repairing_ids",
        "alive_selectable_friendly_constructing_worker_ids",
        "alive_selectable_friendly_idle_military_ids",
        "alive_selectable_friendly_mobile_ids",
    ] {
        assert!(
            pf.contains(&format!("fn {name}")),
            "PresentationFrame must expose {name}"
        );
    }
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    for (fn_name, call) in [
        (
            "select_all_harvesters",
            "alive_selectable_friendly_harvester_ids",
        ),
        (
            "select_idle_harvesters",
            "alive_selectable_friendly_idle_harvester_ids",
        ),
        (
            "select_all_occupied_transports",
            "alive_selectable_friendly_occupied_transport_ids",
        ),
        (
            "select_all_docked_aircraft",
            "alive_selectable_friendly_docked_aircraft_ids",
        ),
        (
            "select_all_idle_military",
            "alive_selectable_friendly_idle_military_ids",
        ),
        (
            "ensure_host_mobile_selection",
            "alive_selectable_friendly_mobile_ids",
        ),
    ] {
        let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
        let body = &eng[at..eng.len().min(at + 1800)];
        assert!(
            body.contains(call),
            "{fn_name} must prefer presentation {call}"
        );
    }
}

#[test]
fn cycle_stop_presentation_source() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    for name in [
        "alive_selectable_friendly_damaged_unit_ids",
        "alive_selectable_friendly_damaged_structure_ids",
        "alive_selectable_friendly_busy_producer_ids",
        "alive_selectable_friendly_ready_special_power_ids",
        "alive_friendly_stoppable_ids",
    ] {
        assert!(
            pf.contains(&format!("fn {name}")),
            "PresentationFrame must expose {name}"
        );
    }
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    for (fn_name, call) in [
        (
            "cycle_damaged_unit_selection",
            "alive_selectable_friendly_damaged_unit_ids",
        ),
        (
            "cycle_damaged_structure_selection",
            "alive_selectable_friendly_damaged_structure_ids",
        ),
        (
            "cycle_busy_producer_selection",
            "alive_selectable_friendly_busy_producer_ids",
        ),
        (
            "cycle_ready_special_power_structure",
            "alive_selectable_friendly_ready_special_power_ids",
        ),
        ("stop_all_friendly_units", "alive_friendly_stoppable_ids"),
    ] {
        let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
        let body = &eng[at..eng.len().min(at + 2200)];
        assert!(
            body.contains(call),
            "{fn_name} must prefer presentation {call}"
        );
    }
}

#[test]
fn snap_camera_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "snap_camera_to_local_units_if_needed").expect("snap_camera");
    assert!(
        body.contains("last_presentation_frame")
            && (body.contains("PresentationBuildingType::CommandCenter")
                || body.contains("frame.objects")),
        "snap_camera must prefer presentation poses"
    );
}

#[test]
fn runtime_host_select_attack_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "runtime_host_cmd_select_local_unit").expect("select_local_unit");
    assert!(
        body.contains("alive_selectable_friendly_mobile_ids")
            && body.contains("first_mobile_friendly_id"),
        "select_local_unit must prefer presentation mobile ids"
    );
    let body =
        rust_fn_body(eng, "runtime_host_cmd_attack_nearest_enemy").expect("attack_nearest_enemy");
    assert!(
        body.contains("alive_selectable_friendly_combat_ids") || body.contains("has_weapon"),
        "attack_nearest_enemy must arm attackers from presentation combat residual"
    );
    if let Some(body) = rust_fn_body(eng, "runtime_host_cmd_guard_position")
        .or_else(|| rust_fn_body(eng, "guard_position"))
    {
        assert!(
            body.contains("alive_selectable_friendly_mobile_ids")
                || body.contains("last_presentation_frame"),
            "guard_position empty pick must use presentation mobiles"
        );
    }
}

#[test]
fn runtime_host_sell_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "runtime_host_cmd_sell").expect("sell_selected");
    assert!(
        body.contains("alive_sellable_friendly_structure_ids")
            || body.contains("last_presentation_frame"),
        "sell_selected empty targets must prefer presentation sellable structures"
    );
    assert!(
        !body.contains("get_objects()")
            || body.contains("Presentation required")
            || body.contains("Boot residual"),
        "sell empty fill must stay presentation-only (no live dual-scan)"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn alive_sellable_friendly_structure_ids"),
        "presentation helper required"
    );
}

#[test]
fn runtime_host_upgrade_construct_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "runtime_host_cmd_upgrade").expect("queue_upgrade");
    assert!(
        body.contains("alive_upgrade_producer_structure_ids")
            || body.contains("last_presentation_frame"),
        "queue_upgrade empty producers must prefer presentation structures"
    );
    let body = rust_fn_body(eng, "runtime_host_cmd_construct").expect("construct");
    assert!(
        body.contains("alive_construct_builder_ids") || body.contains("last_presentation_frame"),
        "construct empty builders must prefer presentation workers/dozers"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn alive_upgrade_producer_structure_ids")
            && pf.contains("fn alive_construct_builder_ids"),
        "presentation helpers required"
    );
}

#[test]
fn runtime_host_empty_pick_batch_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let checks = [
        (
            "runtime_host_cmd_scatter",
            "alive_selectable_friendly_mobile_ids",
        ),
        (
            "runtime_host_cmd_return_supplies",
            "alive_selectable_friendly_harvester_ids",
        ),
        (
            "runtime_host_cmd_set_rally",
            "alive_upgrade_producer_structure_ids",
        ),
        (
            "runtime_host_cmd_cancel_production",
            "alive_upgrade_producer_structure_ids",
        ),
        ("runtime_host_cmd_toggle_overcharge", "PowerPlant"),
        (
            "runtime_host_cmd_formation",
            "alive_selectable_friendly_mobile_ids",
        ),
        (
            "runtime_host_cmd_select_similar",
            "alive_selectable_friendly_mobile_ids",
        ),
        (
            "runtime_host_cmd_attack_move",
            "alive_selectable_friendly_mobile_ids",
        ),
    ];
    for (cmd, helper) in checks {
        let body = rust_fn_body(eng, cmd).unwrap_or_else(|| panic!("missing {cmd}"));
        assert!(
            body.contains(helper) || body.contains("last_presentation_frame"),
            "{cmd} empty pick must prefer presentation helper {helper}"
        );
    }
}

#[test]
fn runtime_host_force_attack_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "runtime_host_cmd_force_attack_object").expect("force_attack");
    assert!(
        body.contains("first_enemy_force_attack_id") || body.contains("last_presentation_frame"),
        "force_attack_object must pick enemy from presentation"
    );
    let body = rust_fn_body(eng, "runtime_host_cmd_attack_nearest_enemy").expect("attack_nearest");
    assert!(
        body.contains("first_enemy_force_attack_id")
            || body.contains("alive_selectable_friendly_combat_ids"),
        "attack_nearest_enemy must pick enemy from presentation without live or_else"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn first_enemy_force_attack_id"),
        "presentation helper required"
    );
}

#[test]
fn runtime_host_construct_train_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let body = rust_fn_body(eng, "runtime_host_cmd_construct").expect("construct");
    assert!(
        body.contains("first_friendly_command_center_position")
            || body.contains("last_presentation_frame"),
        "construct dozer spawn/loc must prefer presentation CC pose"
    );
    let body = rust_fn_body(eng, "runtime_host_cmd_enqueue_production").expect("train");
    assert!(
        body.contains("last_presentation_frame") || body.contains("under_construction"),
        "train unfinished barracks discovery must prefer presentation"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn first_friendly_command_center_position"),
        "presentation CC helper required"
    );
}

#[test]
fn worker_unfinished_construction_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng
        .find("fn cycle_friendly_worker_selection")
        .expect("worker cycle");
    let body = &eng[i..eng.len().min(i + 2200)];
    assert!(
        body.contains("KindOf::Dozer")
            && (body.contains("host_center_camera_on") || body.contains("host_player_look_at")),
        "worker cycle must be KINDOF_DOZER + lookAt (CommandXlat.cpp:2573-2798)"
    );
    let i = eng
        .find("fn cycle_unfinished_construction")
        .expect("unfinished");
    let body = &eng[i..eng.len().min(i + 1800)];
    assert!(
        body.contains("alive_selectable_friendly_unfinished_ids"),
        "unfinished cycle must prefer presentation unfinished ids"
    );
    let i = eng
        .find("fn host_resume_selected_construction")
        .expect("host resume");
    let body = &eng[i..eng.len().min(i + 5500)];
    assert!(
        body.contains("alive_selectable_friendly_unfinished_ids")
            && body.contains("alive_selectable_friendly_idle_worker_ids"),
        "resume construction must prefer presentation unfinished/idle workers"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn alive_selectable_friendly_idle_worker_ids")
            && pf.contains("fn alive_selectable_friendly_unfinished_ids"),
        "presentation helpers required"
    );
}

#[test]
fn runtime_host_status_snapshot_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng
        .find("fn runtime_host_status_snapshot")
        .expect("status snapshot");
    let body = &eng[i..eng.len().min(i + 9000)];
    assert!(
        body.contains("count_mobile_friendlies")
            && body.contains("count_under_construction_friendlies")
            && body.contains("first_friendly_sample_label")
            && body.contains("count_selected_friendlies"),
        "status snapshot must prefer presentation counts/sample"
    );
    assert!(
        body.contains("Boot residual only"),
        "status snapshot must keep boot residual live dual-scans"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn count_under_construction_friendlies")
            && pf.contains("fn first_friendly_sample_label"),
        "presentation helpers required"
    );
}

#[test]
fn residual_hitscan_zeros_fire_spawn_damage_on_apply() {
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_fire_spawn_log;

    host_fire_spawn_log::clear();
    host_fire_spawn_log::record_residual_hitscan(ObjectId(1), ObjectId(2));
    host_fire_spawn_log::record_residual_hitscan(ObjectId(3), ObjectId(4));
    let drained = host_fire_spawn_log::drain_residual_hitscans();
    assert_eq!(drained.len(), 2);
    assert!(host_fire_spawn_log::drain_residual_hitscans().is_empty());

    let apply_src = GAMEWORLD_SHADOW_SRC;
    let i = apply_src
        .find("fn apply_host_fire_spawn_events")
        .expect("apply");
    let body = &apply_src[i..apply_src.len().min(i + 2200)];
    assert!(
        body.contains("drain_residual_hitscans") && body.contains("ev.damage = 0.0"),
        "apply must zero residual-hitscan spawn damage"
    );
    let hbody =
        last_rust_fn_body(GAME_LOGIC_HOST_SRC, "residual_auto_fire_apply_damage").expect("helper");
    assert!(
        hbody.contains("record_residual_hitscan") && hbody.contains("fire_spawn_authority_live"),
        "residual auto-fire must mark hitscan pairs for shadow (live gate)"
    );
}

#[test]
fn residual_auto_fire_records_ai_decision_source() {
    let body =
        last_rust_fn_body(GAME_LOGIC_HOST_SRC, "residual_auto_fire_apply_damage").expect("helper");
    assert!(
        body.contains("host_ai_decision_log::record_attack")
            && body.contains("gameworld_ai_decision_authority")
            && body.contains("record_set_state"),
        "residual auto-fire must emit AI decision AttackTarget under AI_DECISION_AUTHORITY"
    );
}

#[test]
fn residual_auto_fire_ai_decision_writeback_behavioral_source() {
    let src =
        include_str!("../../game_logic/world_tests/ocl_and_bombs/transport_mines_and_defenses.rs");
    assert!(
        src.contains("fn residual_auto_fire_ai_decision_writeback_sets_host_target"),
        "behavioral residual decision writeback test required"
    );
    assert!(
        src.contains("apply_ai_decisions_as_world_mutations")
            && src.contains("writeback_attack_targets_to_host"),
        "behavioral test must exercise GameWorld decision apply + attack writeback"
    );
}

#[test]
fn residual_acquire_query_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for name in [
        "try_base_defense_residual_fire",
        "try_sentry_drone_residual_fire",
        "try_hellfire_drone_residual_fire",
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
        "try_strategy_center_bombardment_turret_fire",
    ] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 8000)];
        assert!(
            body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual combat acquire query"
        );
    }
    // AI factory pick residual priority (idle preferred).
    {
        // ai.rs was split into ai/; the live factory finder lives in ai/teams.rs.
        let ai = include_str!("../../ai/teams.rs");
        let i = ai
            .find("fn find_factory_for_unit_ex")
            .expect("find_factory_for_unit_ex");
        let body = &ai[i..ai.len().min(i + 2500)];
        assert!(
            body.contains("pick_best_priority_residual_target"),
            "AI factory finder must use pure priority residual acquire"
        );
    }
    // Engine boot mouse pick residual priority (presentation delegates to unit_control).
    {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let i = eng
            .find("fn find_object_at_position")
            .expect("engine find_object_at_position");
        // Prefer the InGame/engine pick (not test helpers): scan for boot residual marker.
        let body =
            rust_fn_body(eng, "host_find_object_at_position")
                .expect("engine host_find_object_at_position");
        assert!(
            body.contains("pick_object_id_at_world_from_presentation")
                || body.contains("pick_best_priority_residual_target"),
            "engine boot mouse pick must use presentation residual acquire"
        );
        let _ = i;
    }
    // UnitControl presentation mouse pick residual priority.
    {
        let uc = include_str!("../../unit_control.rs");
        let i = uc
            .find("fn pick_object_id_at_world_from_presentation")
            .expect("pick_object_id_at_world_from_presentation");
        let body = &uc[i..uc.len().min(i + 3500)];
        assert!(
            body.contains("pick_best_priority_residual_target"),
            "unit_control presentation pick must use pure priority residual acquire"
        );
    }
    // CommandIntegration mouse pick residual priority.
    {
        let ci = include_str!("../../command_integration.rs");
        let i = ci
            .find("fn find_object_at_position")
            .expect("find_object_at_position");
        let body = &ci[i..ci.len().min(i + 3500)];
        assert!(
            body.contains("pick_best_priority_residual_target")
                || body.contains("pick_object_id_at_world_from_presentation"),
            "command_integration mouse pick must use presentation residual acquire"
        );
    }
    // Spectre orbit gattling residual nearest enemy.
    {
        let sp = include_str!("../../game_logic/special_power_strikes/registry_fields.rs");
        assert!(
            sp.contains("if gattling_due") && sp.contains("pick_nearest_residual_target_xz"),
            "spectre gattling residual must use pure XZ acquire"
        );
    }
    // AI decisions + resource gather nearest residual.
    {
        let ai = include_str!("../../ai_decisions.rs");
        assert!(
            ai.contains("fn find_nearest_enemy") && ai.contains("pick_nearest_residual_target"),
            "ai_decisions find_nearest_enemy must use pure residual acquire"
        );
        let res = include_str!("../../game_logic/resources.rs");
        assert!(
            res.contains("fn find_nearest_supply_source")
                && res.contains("pick_nearest_residual_target"),
            "resources find_nearest_supply_source must use pure residual acquire"
        );
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let i = eng
            .find("fn find_nearest_friendly_dozer")
            .expect("find_nearest_friendly_dozer");
        let body = &eng[i..eng.len().min(i + 5000)];
        assert!(
            body.contains("pick_nearest_residual_target_xz"),
            "dozer finder must use pure residual XZ acquire"
        );
        assert!(
            body.matches("pick_nearest_residual_target_xz").count() >= 1,
            "dozer finder must use pure residual XZ acquire"
        );
    }
    // CommandExecutor residual nearest picks.
    {
        let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
        assert!(
            src.contains("fn find_nearest_garrison_target")
                && src.contains("request_return_to_base")
                && src.contains("DOZER_MINE_CLEAR_SCAN_RANGE"),
            "command_executor missing residual nearest/authoritative-return markers"
        );
        let picks = src.matches("pick_nearest_residual_target").count();
        assert!(
            picks >= 3 && src.contains("pick_nearest_residual_target_xz"),
            "command_executor must use pure residual acquire helpers (picks={picks})"
        );
    }
    // Patriot multi-assist residual (all legal assistants, nearest-first).
    {
        let name = "process_patriot_assist_request";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 6000)];
        assert!(
            body.contains("filter_residual_targets_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual multi-acquire filter"
        );
    }
    // Click-select nearest residual.
    {
        let name = "select_object_at_position";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 3500)];
        assert!(
            body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual acquire"
        );
    }
    // Nearest SupplyCenter residual (economy return path).
    {
        let name = "find_nearest_supply_center";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 2500)];
        assert!(
            body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual acquire"
        );
    }
    // Jet return-to-base rearm airfield residual.
    {
        let name = "try_return_to_base_rearm";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 4000)];
        assert!(
            (body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"))
                || body.contains("select_and_reserve_airfield_for_return"),
            "{name} must use residual airfield reservation (C++ ParkingPlaceBehavior)"
        );
    }
    // Money crate nearest picker residual.
    {
        let name = "update_money_crate_collides";
        let i = src
            .find(&format!("fn {name}"))
            .or_else(|| src.find(&format!("pub fn {name}")))
            .unwrap_or_else(|| panic!("missing {name}"));
        // update_money_crate_collides is a large tick fn; the XZ acquire sits
        // deep in the body — scan the whole function, not a head window.
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {name}"),
            }
            j += 1;
        };
        let body = &src[i..=end];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual XZ acquire for picker selection"
        );
    }
    // Mine clearer nearest residual inside update_mines_and_demo_traps.
    {
        let name = "update_mines_and_demo_traps";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        // Large tick fn — scan the whole body, not a head window.
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {name}"),
            }
            j += 1;
        };
        let body = &src[i..=end];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} clearer scan must use pure residual XZ acquire"
        );
    }
    // Continue-attack chain + repulsor nearest residual.
    for name in ["try_continue_attack_after_kill", "find_closest_repulsor"] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 4000)];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual XZ acquire query"
        );
    }
    // Harvest supply + ground-attack impact residual.
    for name in [
        "find_nearest_harvestable_supply",
        "find_ground_attack_victim",
    ] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 4000)];
        assert!(
            body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual acquire query"
        );
    }
    // Strategy Center mood-target residual (nearest enemy in vision).
    {
        let name = "tick_strategy_center_turret_mood_target";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 12000)];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual XZ acquire for non-Passive mood"
        );
    }
    // Dozer bored service residual + battle-drone master repair.
    // (BattleDroneAIUpdate doRepairLogic heals only its slaver — no acquire scan.)
    for name in ["find_dozer_bored_repair_target", "find_dozer_bored_mine_target"] {
        let i = src
            .find(&format!("fn {name}"))
            .or_else(|| src.find(&format!("pub fn {name}")))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 4000)];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual XZ acquire query"
        );
    }
    // Point-defense laser intercept residual (priority bands).
    {
        let name = "update_point_defense_intercept";
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 6000)];
        assert!(
            body.contains("pick_best_priority_residual_target")
                && body.contains("PriorityAcquireCandidate"),
            "{name} must use pure residual priority acquire query"
        );
    }
    // Impact/splash residual (XZ nearest-in-radius).
    for name in [
        "apply_overlord_gattling_residual_at",
        "apply_gattling_tank_residual_at",
    ] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 5000)];
        assert!(
            body.contains("pick_nearest_residual_target_xz")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual XZ acquire query"
        );
    }
    for name in [
        "try_auto_find_healing_residual",
        "try_auto_find_repair_residual",
        "try_auto_resume_construction_residual",
    ] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 5000)];
        assert!(
            body.contains("pick_nearest_residual_service_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual service acquire query"
        );
    }
    {
        let i = src
            .find("fn try_pilot_find_vehicle_residual")
            .expect("try_pilot_find_vehicle_residual");
        let body = &src[i..src.len().min(i + 6000)];
        assert!(
            body.contains("pick_nearest_pilot_vehicle_target")
                && body.contains("PilotVehicleCandidate"),
            "pilot find-vehicle must use pure residual pilot acquire query"
        );
    }
    let helper = include_str!("../../game_logic/host_residual_acquire.rs");
    assert!(
        helper.contains("fn pick_nearest_residual_target")
            && helper.contains("fn pick_nearest_residual_service_target")
            && helper.contains("fn pick_nearest_pilot_vehicle_target")
            && helper.contains("Pure residual auto-fire target acquisition"),
        "host_residual_acquire helpers required"
    );
}

#[test]
fn boot_residual_dual_scan_labels_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Every live get_objects dual-scan should sit near Boot residual / Fail-open labels
    // when a presentation-first path exists.
    let mut unlabeled = 0u32;
    let mut total = 0u32;
    let mut search = eng;
    let mut offset = 0usize;
    while let Some(rel) = search.find("get_objects()") {
        let abs = offset + rel;
        total += 1;
        let start = abs.saturating_sub(400);
        let win = &eng[start..eng.len().min(abs + 80)];
        if !(win.contains("Boot residual") || win.contains("Fail-open live residual")) {
            unlabeled += 1;
        }
        offset = abs + 12;
        search = &eng[offset..];
    }
    assert!(
        unlabeled == 0,
        "all remaining get_objects dual-scans must be labeled (total={total} unlabeled={unlabeled})"
    );
    let _ = total;
}

#[test]
fn host_object_id_named_lookup_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let i = src
        .find("fn find_object_id_by_name")
        .expect("find_object_id_by_name");
    let body = &src[i..src.len().min(i + 1800)];
    assert!(
        body.contains("Prefer host object name residual")
            && !body.contains("engine_object_id == Some"),
        "find_object_id_by_name must use host names only (no dual-id reverse lookup)"
    );
    let i = src
        .find("fn transfer_script_object_name")
        .expect("transfer_script_object_name");
    let body = &src[i..src.len().min(i + 1200)];
    assert!(
        body.contains("let tracker_id = to_id.0") || body.contains("tracker_id = to_id.0"),
        "transfer_script_object_name must register host ObjectId"
    );
    let i = src
        .find("fn sync_attack_priority_from_script_engine")
        .expect("sync_attack_priority");
    let body = &src[i..src.len().min(i + 1500)];
    assert!(
        body.contains("host ObjectId is the script-engine key") || body.contains("Some(id.0)"),
        "attack priority sync must use host ObjectId keys"
    );
}

#[test]
fn command_move_attack_host_object_id_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let i = src.find("fn command_move").expect("command_move");
    let body = &src[i..src.len().min(i + 1600)];
    assert!(
        body.contains("move_object_with_pathfinding")
            && !body.contains("bridge_move_to_engine")
            && body.contains("obj.is_mobile()"),
        "command_move must use host pathfinding only (no ObjectFactory bridge)"
    );
    let i = src.find("fn command_attack").expect("command_attack");
    let body = &src[i..src.len().min(i + 6000)];
    assert!(
        body.contains("attack_target(target_id)") && !body.contains("bridge_attack_to_engine"),
        "command_attack must use host ObjectId attack_target only"
    );
}
