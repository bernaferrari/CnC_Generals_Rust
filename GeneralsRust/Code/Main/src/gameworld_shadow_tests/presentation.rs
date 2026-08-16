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
    // hq-nnuu 2026-08-15: GameClient split promoted game_client.rs →
    // core/game_client/impl_update.rs (GAME_CLIENT_SRC concat).
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/core/game_client/impl_update.rs"
    );
    let gc = std::fs::read_to_string(path).expect("impl_update.rs");
    let i = gc
        .find("fn update_presentation_shell")
        .expect("update_presentation_shell");
    let body = &gc[i..gc.len().min(i + 2500)];
    assert!(
        body.contains("update_audio"),
        "presentation shell must drain client-internal audio queue"
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
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
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
        eng.contains("inject_game_client_mouse_move(position.x as f32, position.y as f32)")
            || eng.contains("inject_game_client_mouse_move(position.x as f32"),
        "CursorMoved must inject into GameClient mouse"
    );
    assert!(
        eng.contains("inject_game_client_mouse_button(*button, pressed)"),
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
    // hq-nnuu 2026-08-15: shared THE_KEYBOARD/THE_MOUSE poll lives on
    // GameClient::update (GameClient.cpp:560-584), not the presentation
    // shell (shell must not dual-tick OS input).
    let gc_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameClient/src/core/game_client/impl_update.rs"
    );
    let gc = std::fs::read_to_string(gc_path).expect("impl_update.rs");
    let i = gc.find("fn update(").expect("GameClient::update");
    let body = &gc[i..gc.len().min(i + 4000)];
    assert!(
        body.contains("self.update_input()?") || body.contains("self.update_input()"),
        "GameClient::update must tick update_input on shared device handles"
    );
}

#[test]
fn residual_auto_fire_host_attack_log_source() {
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let helper = src
        .find("fn residual_auto_fire_apply_damage")
        .expect("helper");
    let body = &src[helper..src.len().min(helper + 2500)];
    assert!(
        body.contains("host_attack_log::record(attacker_id, Some(target_id))"),
        "residual auto-fire helper must record host_attack_log for presentation AttackTargeted"
    );
    for name in [
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
        "try_strategy_center_bombardment_turret_fire",
    ] {
        let at = src.find(&format!("fn {name}")).expect(name);
        let b = &src[at..src.len().min(at + 9000)];
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
    let i = eng
        .find("fn select_hero_units_hotkey")
        .expect("select_hero_units_hotkey");
    let body = &eng[i..eng.len().min(i + 1200)];
    assert!(
        body.contains("alive_selectable_friendly_hero_ids")
            && body.contains("last_presentation_frame"),
        "SELECT_HERO must prefer presentation hero ids when frame installed"
    );
    assert!(
        body.contains("Boot residual only") || body.contains("is_hero()"),
        "SELECT_HERO must keep live GameLogic boot residual"
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
    let i = eng
        .find("fn snap_camera_to_local_units_if_needed")
        .expect("snap_camera");
    let body = &eng[i..eng.len().min(i + 4500)];
    assert!(
        body.contains("last_presentation_frame")
            && body.contains("PresentationBuildingType::CommandCenter")
            && body.contains("Boot residual only"),
        "snap_camera must prefer presentation poses with boot residual live scan"
    );
    assert!(
        body.contains("for o in &frame.objects"),
        "presentation path must iterate frame.objects for focus"
    );
}

#[test]
fn runtime_host_select_attack_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("select_local_unit").expect("select_local_unit");
    let body = &eng[i..eng.len().min(i + 1800)];
    assert!(
        body.contains("alive_selectable_friendly_mobile_ids")
            && body.contains("first_mobile_friendly_id")
            && body.contains("Boot residual only"),
        "select_local_unit must prefer presentation mobile ids"
    );
    let i = eng
        .find("attack_nearest_enemy")
        .expect("attack_nearest_enemy");
    let body = &eng[i..eng.len().min(i + 2800)];
    assert!(
        body.contains("alive_selectable_friendly_combat_ids") && body.contains("has_weapon"),
        "attack_nearest_enemy must arm attackers from presentation combat residual"
    );
    let i = eng.find("guard_position").expect("guard_position");
    let body = &eng[i..eng.len().min(i + 2000)];
    assert!(
        body.contains("alive_selectable_friendly_mobile_ids"),
        "guard_position empty pick must use presentation mobiles"
    );
}

#[test]
fn runtime_host_sell_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("sell_selected").expect("sell_selected");
    let body = &eng[i..eng.len().min(i + 4500)];
    assert!(
        body.contains("alive_sellable_friendly_structure_ids"),
        "sell_selected empty targets must prefer presentation sellable structures"
    );
    assert!(
        body.contains("Presentation required (no live get_objects dual-read)")
            || body.contains("Boot residual only"),
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
    let i = eng.find("queue_upgrade").expect("queue_upgrade");
    let body = &eng[i..eng.len().min(i + 3500)];
    assert!(
        body.contains("alive_upgrade_producer_structure_ids"),
        "queue_upgrade empty producers must prefer presentation structures"
    );
    assert!(
        body.contains("Boot residual only"),
        "queue_upgrade must keep boot residual live dual-scan"
    );
    let i = eng.find("dozer_construct").expect("construct");
    let body = &eng[i..eng.len().min(i + 3500)];
    assert!(
        body.contains("alive_construct_builder_ids"),
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
        ("scatter", "alive_selectable_friendly_mobile_ids"),
        (
            "return_to_supply",
            "alive_selectable_friendly_harvester_ids",
        ),
        ("set_rally", "alive_upgrade_producer_structure_ids"),
        ("cancel_queue", "alive_upgrade_producer_structure_ids"),
        ("overcharge", "PowerPlant"),
        ("create_formation", "alive_selectable_friendly_mobile_ids"),
        (
            "double_click_select",
            "alive_selectable_friendly_mobile_ids",
        ),
        ("attackmove", "alive_selectable_friendly_mobile_ids"),
    ];
    for (cmd, helper) in checks {
        let i = eng.find(cmd).unwrap_or_else(|| panic!("missing {cmd}"));
        let body = &eng[i..eng.len().min(i + 2800)];
        assert!(
            body.contains(helper),
            "{cmd} empty pick must prefer presentation helper {helper}"
        );
    }
}

#[test]
fn runtime_host_force_attack_presentation_source() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("force_attack_object").expect("force_attack");
    let body = &eng[i..eng.len().min(i + 2500)];
    assert!(
        body.contains("first_enemy_force_attack_id"),
        "force_attack_object must pick enemy from presentation"
    );
    assert!(
        body.contains("Boot residual only"),
        "force_attack must keep boot residual live dual-scan"
    );
    let i = eng.find("attack_nearest_enemy").expect("attack_nearest");
    let body = &eng[i..eng.len().min(i + 4500)];
    assert!(
        body.contains("first_enemy_force_attack_id"),
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
    let i = eng.find("dozer_construct").expect("construct");
    let body = &eng[i..eng.len().min(i + 7000)];
    assert!(
        body.contains("first_friendly_command_center_position"),
        "construct dozer spawn/loc must prefer presentation CC pose"
    );
    let i = eng.find("train_unit").expect("train");
    let body = &eng[i..eng.len().min(i + 5500)];
    assert!(
        body.contains("under_construction")
            && body.contains("last_presentation_frame")
            && body.contains("Boot residual only"),
        "train unfinished barracks discovery must prefer presentation"
    );
    assert!(
        body.contains("force_completed") && body.contains("Prefer force-completed + presentation"),
        "train producer must prefer force-completed + presentation barracks"
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
        body.contains("alive_selectable_friendly_idle_worker_ids")
            && body.contains("alive_selectable_friendly_busy_worker_ids"),
        "worker cycle must prefer presentation idle/busy worker ids"
    );
    let i = eng
        .find("fn cycle_unfinished_construction")
        .expect("unfinished");
    let body = &eng[i..eng.len().min(i + 1800)];
    assert!(
        body.contains("alive_selectable_friendly_unfinished_ids"),
        "unfinished cycle must prefer presentation unfinished ids"
    );
    let i = eng.find("fn resume_selected_construction").expect("resume");
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
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::ObjectId;

    host_fire_spawn_log::clear();
    host_fire_spawn_log::record_residual_hitscan(ObjectId(1), ObjectId(2));
    host_fire_spawn_log::record_residual_hitscan(ObjectId(3), ObjectId(4));
    let drained = host_fire_spawn_log::drain_residual_hitscans();
    assert_eq!(drained.len(), 2);
    assert!(host_fire_spawn_log::drain_residual_hitscans().is_empty());

    let apply_src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = apply_src
        .find("fn apply_host_fire_spawn_events")
        .expect("apply");
    let body = &apply_src[i..apply_src.len().min(i + 2200)];
    assert!(
        body.contains("drain_residual_hitscans") && body.contains("ev.damage = 0.0"),
        "apply must zero residual-hitscan spawn damage"
    );
    let helper = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let hi = helper
        .find("fn residual_auto_fire_apply_damage")
        .expect("helper");
    let hbody = &helper[hi..helper.len().min(hi + 6000)];
    assert!(
        hbody.contains("record_residual_hitscan") && hbody.contains("fire_spawn_authority_live"),
        "residual auto-fire must mark hitscan pairs for shadow (live gate)"
    );
}

#[test]
fn residual_auto_fire_records_ai_decision_source() {
    let helper = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let i = helper
        .find("fn residual_auto_fire_apply_damage")
        .expect("helper");
    let body = &helper[i..helper.len().min(i + 2000)];
    assert!(
        body.contains("host_ai_decision_log::record_attack")
            && body.contains("gameworld_ai_decision_authority")
            && body.contains("record_set_state"),
        "residual auto-fire must emit AI decision AttackTarget under AI_DECISION_AUTHORITY"
    );
}

#[test]
fn residual_auto_fire_ai_decision_writeback_behavioral_source() {
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
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
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
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
        let ai = include_str!("../ai.rs");
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
        let boot = eng
            .find("Boot residual only — pure priority residual acquire")
            .expect("engine boot pick residual marker");
        let body = &eng[boot..eng.len().min(boot + 2000)];
        assert!(
            body.contains("pick_best_priority_residual_target"),
            "engine boot mouse pick must use pure priority residual acquire"
        );
        let _ = i;
    }
    // UnitControl presentation mouse pick residual priority.
    {
        let uc = include_str!("../unit_control.rs");
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
        let ci = include_str!("../command_integration.rs");
        let i = ci
            .find("fn find_object_at_position")
            .expect("find_object_at_position");
        let body = &ci[i..ci.len().min(i + 3500)];
        assert!(
            body.contains("pick_best_priority_residual_target"),
            "command_integration mouse pick must use pure priority residual acquire"
        );
    }
    // Spectre orbit gattling residual nearest enemy.
    {
        let sp = include_str!("../game_logic/special_power_strikes/registry_fields.rs");
        assert!(
            sp.contains("if gattling_due") && sp.contains("pick_nearest_residual_target_xz"),
            "spectre gattling residual must use pure XZ acquire"
        );
    }
    // AI decisions + resource gather nearest residual.
    {
        let ai = include_str!("../ai_decisions.rs");
        assert!(
            ai.contains("fn find_nearest_enemy") && ai.contains("pick_nearest_residual_target"),
            "ai_decisions find_nearest_enemy must use pure residual acquire"
        );
        let res = include_str!("../game_logic/resources.rs");
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
            body.matches("pick_nearest_residual_target_xz").count() >= 2,
            "dozer finder must use pure residual XZ on presentation and boot paths"
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
            body.contains("pick_nearest_residual_target")
                && body.contains("ResidualAcquireCandidate"),
            "{name} must use pure residual acquire for airfield pick"
        );
    }
    // Money crate nearest picker residual.
    {
        let name = "update_money_crate_collides";
        let i = src
            .find(&format!("fn {name}"))
            .or_else(|| src.find(&format!("pub fn {name}")))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 9000)];
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
        let body = &src[i..src.len().min(i + 7000)];
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
    for name in [
        "find_dozer_bored_repair_target",
        "find_dozer_bored_mine_target",
        "update_battle_drone_repair_residual",
    ] {
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
    let helper = include_str!("../game_logic/host_residual_acquire.rs");
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
        total > 20,
        "expected many get_objects dual-scan sites, got {total}"
    );
    assert_eq!(
            unlabeled, 0,
            "all get_objects dual-scans must be labeled Boot residual or Fail-open live residual (unlabeled={unlabeled})"
        );
    assert!(
        eng.contains("Boot residual only — presentation pick owns InGame identity")
            || eng
                .contains("Boot residual only — presentation pose owns InGame camera slave follow"),
        "key presentation-first Boot residual labels present"
    );
}

#[test]
fn host_object_id_named_lookup_source() {
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
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
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let i = src.find("fn command_move").expect("command_move");
    let body = &src[i..src.len().min(i + 1600)];
    assert!(
        body.contains("move_object_with_pathfinding")
            && !body.contains("bridge_move_to_engine")
            && body.contains("obj.is_mobile()"),
        "command_move must use host pathfinding only (no ObjectFactory bridge)"
    );
    let i = src.find("fn command_attack").expect("command_attack");
    let body = &src[i..src.len().min(i + 2000)];
    assert!(
        body.contains("attack_target(target_id)") && !body.contains("bridge_attack_to_engine"),
        "command_attack must use host ObjectId attack_target only"
    );
}
