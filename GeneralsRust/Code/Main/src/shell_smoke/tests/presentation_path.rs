//! Presentation path / shell update residual tests.

pub use super::*;

#[test]
fn presentation_path_prefers_local_drawable_tick() {
    let cnc = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        (cnc.contains("update_drawables_local") || cnc.contains("update_presentation_shell"))
            && cnc.contains("last_presentation_frame.is_some()"),
        "InGame with presentation must avoid OBJECT_REGISTRY drawable bind"
    );
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    assert!(
        gc.contains("fn update_drawables_local"),
        "GameClient must expose local drawable tick"
    );
}

#[test]
fn presentation_shell_update_is_wired() {
    let client_src = game_client::core::game_client::GAME_CLIENT_SRC;
    let engine_src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        client_src.contains("fn update_presentation_shell"),
        "GameClient must expose presentation shell tick"
    );
    assert!(
        engine_src.contains("update_presentation_shell"),
        "engine must call presentation shell when frame is set"
    );
    assert!(
        engine_src.contains("GENERALS_RUNTIME_HOST_WND"),
        "runtime host must soft-gate WND push for headless smoke"
    );
}

fn presentation_particle_client_mirror_same_frame() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("apply_particle_systems_to_client")
            && eng.contains("Same-frame particle residual: backfill client ParticleSystemManager"),
        "engine must apply presentation particles to client same frame"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("fn apply_particle_systems_to_client")
            && pf.contains("mirror_spawn_to_client_manager")
            && pf.contains("ParticleSystemSpawned"),
        "presentation must backfill client particle mirrors"
    );
    let cp = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/combat_particles.rs"
    ));
    assert!(
        cp.contains("pub(crate) fn mirror_spawn_to_client_manager"),
        "mirror helper must be callable from presentation residual"
    );
}

fn presentation_audio_processed_same_frame_after_apply() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("apply_events_to_audio")
            && eng.contains("Same-frame residual: drain presentation-queued audio now")
            && eng.contains("self.game_logic.process_audio_events()")
            && eng.contains("drain immediately so Select/Command is not delayed one tick"),
        "presentation audio must process same frame after apply and input SFX"
    );
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/game_logic.rs"
    ));
    assert!(
        gl.contains("pub(crate) fn process_audio_events"),
        "process_audio_events must be callable from engine residual path"
    );
}

#[test]
fn play_sound_effect_prefers_presentation_audio_queue() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = eng.find("fn play_sound_effect").expect("play_sound_effect");
    let body = &eng[i..eng.len().min(i + 2200)];
    assert!(
        body.contains("play_sound_through_the_audio")
            && body.contains("last_presentation_frame.is_some()")
            && body.contains("AudioManagerSubsystem")
            && body.contains("UnitSelect")
            && body.contains("UnitCommand")
            && !body.contains("SoundType::Select => \"UnitSelect\"")
            && !body.contains("SoundType::Command => \"UnitCommand\"")
            && !body.contains("self.game_logic.queue_audio_event")
            && !body.contains("self.game_logic.process_audio_events()"),
        "InGame SFX must not invent UnitSelect/UnitCommand; Voice* is pickAndPlay"
    );
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("MoveOrdered { unit") && !pf.contains("Some((\"UnitMove\", Some(*unit)))"),
        "MoveOrdered must not invent a UnitMove SFX; VoiceMove is pickAndPlay"
    );
}

#[test]
fn presentation_shell_input_audio_without_draw_dual_own() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    let start = gc
        .find("pub fn update_presentation_shell")
        .expect("presentation shell");
    let window = &gc[start..start + 12_000.min(gc.len() - start)];
    assert!(
        gc.contains("update_presentation_shell")
            && window.contains("without Main-owned input/audio or Display DRAW dual-ownership")
            && window.contains("update_drawables_local")
            && !window.contains("self.draw_display()?")
            && window.contains("update_input")
            && window.contains("update_audio"),
        "presentation shell ticks drawables/UI without dual-owning Main OS input/3D draw; client audio queue drains"
    );
    assert!(
        gc.contains("fn update_post_draw_ui") && gc.contains("crate::eva::update_eva_system()"),
        "Eva must tick from post-draw UI residual used by shell"
    );
}

#[test]
fn presentation_fow_shroud_drawable_residual_no_object_registry() {
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let apply_idx = eng
        .find("apply_frozen_direct_shroud_statuses")
        .expect("engine must call frozen direct shroud apply");
    let apply_window = &eng[apply_idx..apply_idx.saturating_add(800).min(eng.len())];
    assert!(
        gc.contains("apply_frozen_direct_shroud_statuses")
            && gc.contains("FrozenDirectShroudStatus")
            && gc.contains("no OBJECT_REGISTRY")
            && eng.contains("drawable_shroud.direct_game_client_status()")
            && !apply_window.contains("OBJECT_REGISTRY"),
        "frozen raw direct shroud status must drive drawables without OBJECT_REGISTRY dual-read"
    );
}

#[test]
fn presentation_alliance_local_player_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 569/607/900: live `take_alliance_events` dual-read was removed;
    // the residual is now fail-closed via host_take_presentation_or_boot_alliance_events
    // (camera_drain.rs), which returns the frozen frame's alliance_events.
    assert!(
        eng.contains("fn host_take_presentation_or_boot_alliance_events")
            && eng.contains("pres.alliance_events.clone()")
            && eng.contains("Wave 569: alliance residual — prefer presentation freeze")
            && eng.contains("last_presentation_frame")
            && eng.contains("local_player_id"),
        "alliance notifications must prefer the presentation freeze residual"
    );
}

#[test]
fn presentation_camera_residual_prefers_frame() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("fn apply_presentation_camera_residual")
            && eng.contains("fn drain_live_camera_request_queues")
            && eng.contains("Prefer presentation-frozen camera residual")
            && eng.contains("apply_presentation_camera_residual(&pres)"),
        "InGame script camera must prefer presentation freeze over live take_* dual-read"
    );
    let i = eng
        .find("fn apply_pending_script_camera_requests")
        .expect("apply_pending_script_camera_requests");
    let window = &eng[i..i.saturating_add(900).min(eng.len())];
    assert!(
        window.contains("last_presentation_frame.clone()")
            && window.contains("drain_live_camera_request_queues"),
        "presentation path must drain live queues after apply"
    );
}

#[test]
fn presentation_popup_music_fps_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let camera_drain = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cnc_game_engine/camera_drain.rs"
    ));
    let popup_bridge = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cnc_game_engine/control_bar_bridge.rs"
    ));
    let scripts_camera = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/world_scripts/scripts_camera/script_runtime_camera.rs"
    ));
    let popup_dismiss = popup_bridge
        .find("fn host_dismiss_in_game_popup_message")
        .map(|start| &popup_bridge[start..])
        .unwrap_or("");
    let matching_ack_precedes_active_clear = popup_dismiss
        .find("popup_acknowledgement_matches_active_generation")
        .zip(popup_dismiss.find("self.game_logic.take_popup_message_requests();"))
        .is_some_and(|(guard, take)| guard < take);
    assert!(
        pf.contains("struct PresentationPopupMessage")
            && pf.contains("pause_music: p.pause_music")
            && eng.contains("fn apply_ingame_script_fps_limit_residual")
            && eng.contains(".and_then(|p| p.script_fps_limit)")
            && camera_drain.contains("pres.pending_popup_messages.last()")
            && camera_drain.contains("self.host_reconcile_active_popup_pause(Some(popup.pause))")
            && camera_drain.contains("self.host_reconcile_active_popup_pause(None)")
            && camera_drain.contains("pres.pending_music_stop")
            && scripts_camera.contains("let active_popup = popup_messages.last().cloned()")
            && scripts_camera.contains("self.pending_popup_messages.clear();")
            && matching_ack_precedes_active_clear
            && popup_dismiss.contains("self.host_clear_active_popup_presentation_residual();")
            && popup_dismiss.contains("self.host_reconcile_active_popup_pause(None);"),
        "popup/music/fps must freeze only the newest C++ popup, reconcile its owned pause, and clear it only after a matching acknowledgement"
    );
}

#[test]
fn presentation_script_message_movie_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Movie take helpers moved to the EvaCamera script module (same residual:
    // pending script/radar movies consumed after presentation freeze/apply).
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/world_scripts/eva_camera.rs"
    ));
    assert!(
        eng.contains("fn host_take_presentation_or_boot_new_script_messages")
            && eng.contains("pres.new_script_messages.clone()")
            && eng.contains("apply_presentation_movie_residual")
            && eng.contains("fn apply_presentation_movie_residual")
            && eng.contains("fn presentation_or_boot_match_over_label")
            && eng.contains("pres.victory_label.clone()")
            && gl.contains("fn take_pending_movie")
            && gl.contains("fn take_pending_radar_movie"),
        "script messages/movies/victory status must prefer presentation freeze"
    );
}

#[test]
fn presentation_play_time_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("total_play_time_seconds: logic.get_total_play_time()")
            && pf.contains("ui.current_game_time = self.total_play_time_seconds")
            && eng.contains("fn presentation_or_boot_total_play_time")
            && eng.contains("pres.total_play_time_seconds")
            && eng.contains(
                "ui_state.current_game_time = self.presentation_or_boot_total_play_time()"
            ),
        "UI game time must prefer presentation total_play_time_seconds"
    );
}

#[test]
fn presentation_defeat_save_info_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    // GameLogic facade split: peek helpers live in game_logic/game_logic/mod.rs
    // (game_logic.rs is a 1 KB include_str! shim).
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/game_logic/mod.rs"
    ));
    assert!(
        pf.contains("defeated_player_ids")
            && pf.contains("logic.peek_defeat_events()")
            && gl.contains("fn peek_defeat_events")
            && eng.contains("pres.defeated_player_ids.clone()")
            && eng.contains("Wave 607/900: presentation freeze owns defeat residual when installed")
            // Wave 545/554: save metadata resolves via presentation_or_boot_* helpers
            // (host_authority.rs build_save_info), no live world_env/map dual-read.
            && eng.contains("presentation_or_boot_total_play_time()")
            && eng.contains("presentation_or_boot_map_name()"),
        "defeat notifications and save_info must prefer presentation freeze"
    );
}

#[test]
fn presentation_alliance_events_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    // GameLogic facade split: peek helpers live in game_logic/game_logic/mod.rs.
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/game_logic/mod.rs"
    ));
    assert!(
        pf.contains("pub alliance_events:")
            && pf.contains("logic.peek_alliance_events()")
            && gl.contains("fn peek_alliance_events")
            && eng.contains("Wave 569: alliance residual — prefer presentation freeze")
            && eng.contains("pres.alliance_events.clone()"),
        "alliance notifications must prefer presentation freeze over live take"
    );
}

#[test]
fn presentation_difficulty_game_mode_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    // Wave 554/609: difficulty/mode/map resolve via presentation_or_boot_* helpers
    // (camera_drain.rs presentation_or_boot_ai_difficulty / presentation_or_live_game_mode
    // / presentation_or_boot_map_name; host_authority.rs build_save_info consumes them).
    assert!(
        pf.contains("ai_difficulty: logic.get_difficulty()")
            && pf.contains("game_mode: logic.game_mode()")
            && eng.contains("fn presentation_or_boot_ai_difficulty")
            && eng.contains("pres.ai_difficulty")
            && eng.contains("pres.game_mode")
            && eng.contains("Wave 554: presentation freeze owns map-name residual when installed")
            && eng.contains("presentation_or_boot_map_name()"),
        "save/restart/runtime-host must prefer presentation difficulty/mode/map"
    );
}

#[test]
fn presentation_menu_shell_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 552/568: menu residual lives in presentation_affirms_shell_or_boot
    // (host_authority.rs) and apply_shell_script_fps_limit_residual (start_game.rs);
    // both still trust the freeze only when it affirms fow_shell_bypass.
    assert!(
        eng.contains("only trust freeze when it *affirms* shell-map")
            && eng.contains("Some(pres) if pres.fow_shell_bypass => true")
            && eng.contains("fn apply_shell_script_fps_limit_residual")
            && eng.contains("filter(|p| p.fow_shell_bypass)"),
        "menu shell tick must prefer presentation fow_shell_bypass when true"
    );
}

#[test]
fn presentation_game_mode_helper_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("fn presentation_or_live_game_mode")
            && eng.contains("Prefer presentation game_mode residual when installed")
            && eng.matches("presentation_or_live_game_mode()").count() >= 7
            && eng.contains("should_keep_logic_running_while_iconic")
            && eng.contains("engine.presentation_or_live_game_mode()"),
        "load-screen/quick-save/restart/iconic must prefer presentation game_mode helper"
    );
}

#[test]
fn presentation_load_screen_roster_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("is_ai: logic.ai_manager_contains_player(id)")
            && pf.contains("color_rgb: p.color_rgb")
            && eng.contains("Prefer full presentation roster when installed")
            && eng.contains("frame.players.is_empty()")
            && eng.contains("is_ai: player.is_ai")
            && eng.contains("player.color_rgb")
            && eng.contains("apparent_text_color: Some(text_color)")
            && eng.contains("apparent_color is multiplayer color *index*")
            && eng.contains("visible: player.is_alive"),
        "load-screen init must expand slots from full presentation roster with is_ai/color"
    );
}

#[test]
fn presentation_cinematic_letterbox_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    assert!(
        gc.contains("fn apply_presentation_cinematic_letterbox")
            && gc.contains("enable_letter_box(enabled)")
            && eng.contains("apply_presentation_cinematic_letterbox(pres.cinematic_letterbox)")
            && eng.contains("Presentation cinematic letterbox residual"),
        "presentation cinematic_letterbox must drive GameClient display letterbox"
    );
}

#[test]
fn presentation_military_caption_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    // GameLogic facade split: the caption duration helper lives in the
    // ui_production world-script module.
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/world_scripts/ui_production.rs"
    ));
    assert!(
        pf.contains("military_caption_remaining_ms")
            && gl.contains("fn military_caption_remaining_ms")
            && gc.contains("fn apply_presentation_military_caption")
            && eng.contains("apply_presentation_military_caption")
            && eng.contains("pres.military_caption_remaining_ms"),
        "presentation military caption must freeze remaining_ms and apply to InGameUI"
    );
}

#[test]
fn presentation_cinematic_text_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let gc = game_client::core::game_client::GAME_CLIENT_SRC;
    // Wave (caption residual): cinematic text is a W3DDisplay caption driven via
    // arm_cinematic_overlay, explicitly *not* an InGameUI HUD message (C++
    // doDisplayCinematicText); the engine call site (camera_drain.rs) now passes
    // text/remaining_ms/font across lines.
    assert!(
        gc.contains("fn apply_presentation_cinematic_text")
            && gc.contains("arm_cinematic_overlay")
            && gc.contains("last_applied_cinematic_text")
            && eng.contains("apply_presentation_cinematic_text(")
            && eng.contains("pres.cinematic_text.as_deref()")
            && eng.contains("Cinematic text residual"),
        "presentation cinematic_text must drive the W3DDisplay cinematic caption with anti-spam"
    );
}

#[test]
fn presentation_camera_follow_residual() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    // GameLogic facade split: the follow-target peek lives in the EvaCamera
    // world-script module; the engine marker reworded (camera_drain.rs Wave 216).
    let gl = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/game_logic/world_scripts/eva_camera.rs"
    ));
    assert!(
        gl.contains("fn peek_camera_follow_target_position")
            && pf.contains("camera_follow_position")
            && pf.contains("peek_camera_follow_target_position")
            && eng.contains("pres.camera_follow_position")
            && eng.contains("presentation-frozen follow only"),
        "camera follow must prefer presentation freeze over live dual-read"
    );
}

#[test]
fn presentation_timers_cameo_superweapon_residual() {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        pf.contains("ui.named_timers = self.named_timers.clone()")
            && pf.contains("ui.named_timer_display_shown = self.named_timer_display_shown")
            && pf.contains("ui.cameo_flash = self.cameo_flash.clone()")
            && pf.contains("ui.superweapon_display_enabled = self.superweapon_display_enabled")
            && pf.contains(
                "ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone()"
            )
            && pf.contains("Script named-timer / cameo / superweapon residual"),
        "apply_to_ui_state must project named_timers/cameo/superweapon from presentation"
    );
}
