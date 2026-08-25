// Mechanical extract from cnc_game_engine.rs `mod tests`.
// Child module via `#[path]`. include_str! paths stay sibling-relative.

#[test]
fn presentation_path_applies_frozen_direct_shroud_after_sync_before_pose() {
    let src = include_str!("camera_drain.rs");
    assert!(
        src.contains("apply_frozen_direct_presentation_poses"),
        "InGame presentation path must push keyed frozen poses without OBJECT_REGISTRY"
    );
    let helper = src
        .find("fn host_sync_presentation_direct_drawables(")
        .expect("shared direct drawable hydration helper");
    let helper_end = src[helper..]
        .find("/// Wave 590: boot/render residual")
        .map(|offset| helper + offset)
        .expect("next helper boundary");
    let helper = &src[helper..helper_end];
    let sync = helper
        .find("sync_presentation_drawables(sync_entries)")
        .expect("presentation drawable sync");
    let direct_shroud = helper
        .find("apply_frozen_direct_shroud_statuses(logic_frame, shroud_entries)")
        .expect("frozen raw shroud apply");
    let pose = helper
        .find("apply_frozen_direct_presentation_poses(pose_entries)")
        .expect("presentation pose apply");
    let window = &helper[sync..helper.len().min(pose + 300)];
    assert!(
        sync < direct_shroud && direct_shroud < pose,
        "raw direct status must apply only after sync and before pose"
    );
    assert!(
        helper.contains("direct_host_drawables")
            && helper.contains("object.drawable_shroud.direct_game_client_status()?")
            && helper.contains("FrozenDirectShroudStatus")
            && helper.contains("presentation_direct_drawable_state(")
            && helper.contains("FrozenDirectPresentationPose"),
        "only host-resident direct records may enter GameClient with one guarded binding key"
    );
    assert!(
        !window.contains(".filter(|o| !o.destroyed)"),
        "gameplay death must not prune a still-resident C++ Drawable"
    );
    assert!(
        !window.contains("apply_presentation_shroud_to_drawables"),
        "scalar ObjectVisibility must not compete with the raw direct-status path"
    );
    assert!(
        helper.contains("if !presentation_time_frozen")
            && helper.contains("apply_frozen_direct_shroud_statuses"),
        "C++ GameClient update must retain direct shroud state during script/tactical or game-pause freeze"
    );
    assert!(
        helper.contains("scene_hidden_by_stealth: pres.local_viewer_hides_stealthed(o)"),
        "direct hiddenByStealth must use the frozen viewer-relative C++ look, not generic stealth"
    );
}

#[test]
fn match_seed_primes_direct_bindings_before_first_ingame_render() {
    let camera = include_str!("camera_drain.rs");
    let seed_start = camera
        .find("pub(super) fn host_seed_presentation_after_match_start(")
        .expect("match-start presentation seed");
    let seed_end = camera[seed_start..]
        .find("/// Freeze-to-GameClient direct Drawable association boundary")
        .map(|offset| seed_start + offset)
        .expect("direct hydration helper follows seed");
    let seed = &camera[seed_start..seed_end];
    let frame = seed
        .find("self.last_presentation_frame = Some(pres);")
        .expect("seed installs immutable frame first");
    let direct_sync = seed
        .find("self.host_sync_presentation_direct_drawables(presentation_time_frozen);")
        .expect("seed primes direct bindings");
    assert!(
        frame < direct_sync
            && seed.contains("self.presentation_or_boot_time_frozen() || self.game_paused"),
        "first InGame render must receive direct bindings after frame install and with the C++ freeze gate"
    );

    let start_game = include_str!("start_game.rs");
    let start = &start_game[start_game
        .find("pub(super) fn complete_parked_match_start(")
        .expect("parked start finish authority")..];
    assert!(
        start
            .find("self.seed_presentation_after_match_start();")
            .expect("start-game presentation seed")
            < start
                .find("self.transition_to_state(GameState::InGame);")
                .expect("start-game InGame transition"),
        "new matches must hydrate direct bindings before their first InGame render"
    );

    let authority = include_str!("host_authority.rs");
    let restore = &authority[authority
        .find("pub(super) fn host_load_game_from_ui(")
        .expect("load-game authority")..];
    assert!(
        restore
            .find("self.seed_presentation_after_match_start();")
            .expect("load-game presentation seed")
            < restore
                .find("self.transition_to_state(GameState::InGame);")
                .expect("load-game InGame transition"),
        "staged restores must hydrate direct bindings before their first InGame render"
    );

    let shell = include_str!("shell.rs");
    let startup = &shell[shell
        .find("pub(super) fn host_finalize_startup_map_load(")
        .expect("startup-map authority")..];
    let startup_target = &startup[startup
        .find("if let Some(target_state) = target_state {")
        .expect("startup target transition")..];
    let ingame = &startup_target[startup_target
        .find("if target_state == GameState::InGame {")
        .expect("startup InGame branch")..];
    assert!(
        ingame
            .find("self.seed_presentation_after_match_start();")
            .expect("startup InGame seed")
            < ingame
                .find("self.transition_to_state(target_state);")
                .expect("startup target transition"),
        "CLI/initial-file startup must hydrate direct bindings before its first InGame redraw"
    );
}

#[test]
fn presentation_render_resolves_only_the_frozen_direct_scene_candidate_ledger() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let render = &src[src.find("pub fn render(&mut self)").expect("render entry")..];
    let ledger = render
        .find("let mut direct_scene_candidate_sink")
        .expect("Main-owned direct scene candidate boundary");
    let game_client_call = render[ledger..]
        .find("evaluate_frozen_direct_scene_shroud_candidates(")
        .map(|offset| ledger + offset)
        .expect("only Main may resolve the frozen candidate ledger against GameClient");
    let execute = render[ledger..]
        .find("render_pipeline.execute(")
        .map(|offset| ledger + offset)
        .expect("render pipeline receives the boundary callback");

    assert!(ledger < game_client_call && game_client_call < execute);
    let callback = &render[ledger..execute];
    assert!(
        callback.contains("FrozenDirectDrawableSceneCandidate")
            && callback.contains("PresentationDirectDrawableBindingKey")
            && callback.contains("DrawableId(candidate.drawable_id)")
            && callback.contains("presentation_logic_frame")
            && callback.contains("FrozenDirectDrawableSceneDecision")
            && callback.contains("FrozenDirectDrawableSceneOutcome")
            && callback.contains("SceneShroudDecision::HiddenDirectDrawable")
            && callback.contains("SceneShroudDecision::RenderDrawable"),
        "Main must map the immutable full candidate key and raw source facts exactly once"
    );
    assert!(
        callback.contains("hq-1a1")
            && !callback.contains("fow_visibility")
            && !callback.contains("get_shroud"),
        "scene decisions must not be inferred from FOW alpha or live shroud reads"
    );
}

#[test]
fn visual_world_state_invalidation_only_follows_successful_world_changes() {
    let lifecycle = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let reset_at = lifecycle
        .find("pub fn invalidate_world_visual_state(")
        .expect("dedicated visual-world reset API");
    let reset_body = &lifecycle[reset_at..lifecycle.len().min(reset_at + 1_400)];
    assert!(reset_body.contains("clear_visual_world_state_components"));
    assert!(reset_body.contains("self.presentation_frame = None"));
    assert!(
        !reset_body.contains("clear_caches"),
        "renderer world reset must not evict globally valid asset caches"
    );

    let authority = include_str!("host_authority.rs");
    let assert_client_reset_precedes_renderer = |boundary: &str, label: &str| {
        let epoch_advance = boundary
            .find("self.host_advance_direct_visual_world_epoch()")
            .unwrap_or_else(|| panic!("{label} must advance direct visual identity"));
        let client_reset = boundary
            .find("self.game_client.invalidate_presentation_drawable_world()")
            .unwrap_or_else(|| panic!("{label} must reset volatile GameClient drawable state"));
        let renderer_reset = boundary
            .find("self.render_pipeline.invalidate_world_visual_state()")
            .unwrap_or_else(|| panic!("{label} must reset renderer-local state"));
        assert!(
            epoch_advance < client_reset && client_reset < renderer_reset,
            "{label} must advance direct identity, then reset GameClient associations before renderer object-ID timelines"
        );
    };
    let reset = &authority[authority
        .find("fn host_reset_game_logic(")
        .expect("logic reset boundary")..];
    let reset = &reset[..reset
        .find("fn host_destroy_object(")
        .expect("next reset-neighbor method")];
    assert!(
        reset.contains("self.render_pipeline.invalidate_world_visual_state()"),
        "GameLogic reset reuses object IDs and must clear renderer-local timelines"
    );
    assert_client_reset_precedes_renderer(reset, "GameLogic reset");

    let replacement = &authority[authority
        .find("fn host_replace_game_logic(")
        .expect("full replacement boundary")..];
    let replacement = &replacement[..replacement
        .find("fn host_replace_staged_restore_world(")
        .expect("next replacement-neighbor method")];
    assert_client_reset_precedes_renderer(replacement, "full GameLogic replacement");
    let replacement_reset = replacement
        .find("shadow.reset_for_world_boundary();")
        .expect("full replacement must reset GameWorld identity state");
    let replacement_sync = replacement
        .find("shadow.sync_from_host(&self.game_logic);")
        .expect("full replacement must seed the new GameWorld before probes");
    assert!(
        replacement_reset < replacement_sync,
        "full GameLogic replacement must reset then immediately sync its fresh GameWorld"
    );

    let staged_replacement = &authority[authority
        .find("fn host_replace_staged_restore_world(")
        .expect("staged replacement boundary")..];
    let staged_replacement = &staged_replacement[..staged_replacement
        .find("pub(super) fn host_save_game_authority(")
        .expect("next staged-replacement neighbor method")];
    assert_client_reset_precedes_renderer(staged_replacement, "committed staged restore");

    let map_load_source = include_str!("ui_commands.rs");
    let map_load = &map_load_source[map_load_source
        .find("fn host_load_map_or_default(")
        .expect("in-place map-load boundary")..];
    let success_guard = map_load
        .find("let Some(loaded) = loaded else")
        .expect("successful map-load guard");
    let reset_after_success = map_load
        .find("self.render_pipeline.invalidate_world_visual_state()")
        .expect("map-load visual reset");
    let client_reset_after_success = map_load
        .find("self.game_client.invalidate_presentation_drawable_world()")
        .expect("map-load GameClient drawable reset");
    let epoch_after_success = map_load
        .find("self.host_advance_direct_visual_world_epoch()")
        .expect("map-load direct visual epoch advance");
    assert!(
        success_guard < epoch_after_success
            && epoch_after_success < client_reset_after_success
            && client_reset_after_success < reset_after_success,
        "failed map loads must retain the active world's visual identity and state"
    );

    let setter_at = lifecycle
        .find("pub fn set_presentation_frame(")
        .expect("ordinary frame handoff setter");
    let setter_tail = &lifecycle[setter_at..];
    let setter_end = setter_tail
        .find("\n    }\n")
        .expect("presentation setter closing brace")
        + "\n    }\n".len();
    let setter = &setter_tail[..setter_end];
    assert!(
        !setter.contains("invalidate_world_visual_state"),
        "ordinary frame handoff must not restart W3D timelines"
    );
}

#[test]
fn direct_visual_world_epoch_is_runtime_only_and_nonzero_at_boot() {
    let types = include_str!("types.rs");
    assert!(
        types.contains("host_direct_visual_world_epoch: u64"),
        "direct visual generation must be a host runtime field"
    );
    assert!(
        types.contains("intentionally not part of a snapshot"),
        "C++ volatile Drawable association state must not become a durable schema field"
    );
    let boot = include_str!("boot.rs");
    assert!(
        boot.contains("host_direct_visual_world_epoch: 1"),
        "the initial direct visual epoch must be nonzero so invalid/default identities fail closed"
    );
}

#[test]
fn weapon_barrel_topology_prewarm_is_only_a_successful_world_boundary() {
    let source = include_str!("ui_commands.rs");
    let map_load_start = source
        .find("fn host_load_map_or_default(")
        .expect("map-load authority boundary");
    let map_load = &source[map_load_start..];
    let success_guard = map_load
        .find("let Some(loaded) = loaded else")
        .expect("successful map-load guard");
    let prewarm = map_load
        .find("self.prewarm_host_weapon_barrel_topologies_for_loaded_world()")
        .expect("host barrel topology prewarm");
    let visual_reset = map_load
        .find("self.render_pipeline.invalidate_world_visual_state()")
        .expect("new-world visual reset");
    assert!(
        success_guard < prewarm && prewarm < visual_reset,
        "only a successfully installed logical world may preload exact W3D barrel topology"
    );

    let helper_start = source
        .find("fn prewarm_host_weapon_barrel_topologies_for_loaded_world(")
        .expect("dedicated map-boundary helper");
    let helper = &source[helper_start..map_load_start + success_guard];
    assert!(helper.contains("prewarm_weapon_barrel_topology_models_for_objects"));
    assert!(helper.contains("prewarm_weapon_barrel_topologies_for_object_conditions"));
    assert!(
        !helper.contains("render_pipeline"),
        "host configuration must read immutable cached assets, not write from WGPU"
    );
}

#[test]
fn ui_command_path_prefers_presentation_object_identity() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    for token in [
        "fn presentation_ro",
        "fn ui_object_alive",
        "fn ui_object_is_dozer",
        "fn ui_object_can_produce",
        "fn ui_production_queue_head",
        "fn ui_selected_ids",
        "fn ui_special_power_ready",
        "fn ui_special_power_type_if_ready",
    ] {
        assert!(
            eng.contains(token),
            "missing UI presentation helper {token}"
        );
    }
    // Producer fail-open live scan only when no presentation frame.
    assert!(
        eng.contains("last_presentation_frame.is_none()"),
        "producer fail-open must gate on missing presentation frame"
    );
    // Dozer/producer filters use presentation-first helpers.
    assert!(
        eng.contains("self.ui_object_is_dozer(id)")
            && eng.contains("self.ui_object_can_produce(id)")
            && eng.contains("self.ui_production_queue_head(id)"),
        "UI command filters must call presentation-first helpers"
    );
    // Wave 214: force-completed producer pick is presentation-only (no live classify).
    assert!(
        eng.contains("Wave 214: force-completed IDs classified from presentation freeze only")
            && eng.contains("force_completed")
            && eng.contains("can_produce")
            && eng.contains("no live GameLogic dual-read residual")
            && eng
                .contains("Wave 214: force-completed IDs classified from presentation freeze only"),
        "force-completed producer pick must be presentation-only"
    );
}

#[test]
fn sample_startup_camera_heights_prefers_presentation_height_grid() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let idx = eng
        .find("fn sample_startup_camera_heights")
        .expect("camera height helper");
    let body = &eng[idx..idx + 1200];
    assert!(
        body.contains("presentation")
            && body.contains("sample_height")
            && body.contains("world_env"),
        "camera height helper must sample presentation world_env height grid"
    );
    assert!(
        body.contains("Option<&crate::presentation_frame::PresentationFrame>")
            || body.contains("presentation: Option"),
        "camera height helper must take optional PresentationFrame"
    );
}

#[test]
fn render_ui_state_prefers_presentation_without_live_update() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 591: real consumer lives in host_build_render_ui_state_from_presentation.
    // Prefer last production def (tests may embed the signature string).
    let marker =
        "fn host_build_render_ui_state_from_presentation(&mut self) -> crate::ui::GameUIState";
    let mut i = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(marker) {
        i = Some(from + rel);
        from = from + rel + marker.len();
    }
    let i = i.expect("render presentation UI consumer helper");
    let window = &src[i..(i + 2000).min(src.len())];
    assert!(
        window.contains("GameUIState::default()") && window.contains("pres.apply_to_ui_state"),
        "InGame render must build UI state from PresentationFrame default+apply"
    );
    assert!(
        window.contains("Boot/loading residual only")
            && window.contains("update_ui_state(self.current_player_id)"),
        "boot residual may still call update_ui_state without presentation"
    );
    // Ensure the presentation branch does not call live update_ui_state first.
    // Comments may mention update_ui_state; require no host/live call before boot arm.
    let branch_end = window
        .find("Boot/loading residual only")
        .unwrap_or(window.len());
    let presentation_branch = &window[..branch_end];
    assert!(
        !presentation_branch.contains("host_update_ui_state(")
            && !presentation_branch.contains("self.update_ui_state("),
        "presentation branch must not call live update_ui_state"
    );
    assert!(
        src.contains("self.host_build_render_ui_state_from_presentation()"),
        "render path must call host render UI presentation helper"
    );
}

fn presentation_path_ticks_drawables_like_cpp() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Build token from pieces so this test source does not self-match.
    let token = format!("// {}{}:", "PRES_SHELL_ONLY_", "DRAWABLE_TICK");
    let i = src.find(&token).expect("presentation shell token comment");
    let w = &src[i..src.len().min(i + 500)];
    assert!(
        w.contains("update_presentation_shell")
            && w.contains("update_drawables_local")
            && !w.contains("game_client.update_drawables("),
        "presentation client path must use shell-only drawable tick"
    );
}

#[test]
fn show_shell_menu_sets_shell_active_for_wnd_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let i = src.find("fn show_shell_menu").expect("show_shell_menu");
    let body = &src[i..src.len().min(i + 2200)];
    assert!(
        body.contains("SubsystemInterface::init"),
        "show_shell_menu must init Shell before push (TLS starts uninitialized)"
    );
    assert!(
        body.contains("set_shell_active(true)"),
        "show_shell_menu must set Shell::is_shell_active after MainMenu push"
    );
    assert!(
        body.contains("shell_menu_active = true"),
        "engine shell_menu_active residual required after successful stack push"
    );
    assert!(
        body.contains("get_screen_count()"),
        "show_shell_menu must verify screen stack before latching active"
    );
    assert!(
        !body.contains("will continue without a main menu") || body.contains("screens == 0"),
        "empty stack must not latch shell_menu_active"
    );
}

fn match_start_presentation_seed_uses_shadow_overlay() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 590: match-start peels through host_seed_presentation_after_match_start.
    let needle = "fn host_seed_presentation_after_match_start(";
    let i = src
        .find(needle)
        .expect("host_seed_presentation_after_match_start");
    let body = &src[i..src.len().min(i + 1400)];
    assert!(
        body.contains("host_sync_shadow_and_build_presentation"),
        "match-start seed must use presentation build boundary (Wave 926)"
    );
    assert!(
        !body.contains("build_and_apply_for_hud"),
        "seed must not skip shadow via build_and_apply_for_hud"
    );
    // Thin wrapper still exists for callers.
    assert!(
        src.contains("fn seed_presentation_after_match_start")
            && src.contains("host_seed_presentation_after_match_start()"),
        "seed_presentation_after_match_start must delegate to host helper"
    );
    // Boot/render residual seed via host_ensure_presentation_frame_for_render.
    let j = src
        .find("Boot/Menu residual: if no frame yet")
        .expect("boot residual comment");
    let boot_call = &src[j..src.len().min(j + 500)];
    assert!(
        boot_call.contains("host_ensure_presentation_frame_for_render"),
        "boot path must call host_ensure_presentation_frame_for_render"
    );
    let k = src
        .find("fn host_ensure_presentation_frame_for_render(")
        .expect("host_ensure_presentation_frame_for_render");
    let boot = &src[k..src.len().min(k + 900)];
    assert!(
        boot.contains("host_sync_shadow_and_build_presentation"),
        "boot presentation seed must use presentation build boundary (Wave 926)"
    );
}

#[test]
fn apply_presentation_to_huds_dual_no_recurse_residual() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let marker = "fn apply_presentation_to_huds(";
    let i = src.find(marker).expect("dual HUD apply helper");
    let body = &src[i..src.len().min(i + 450)];
    assert!(
        body.contains("pres.apply_to_game_hud(&mut self.game_hud)"),
        "must apply presentation freeze to engine GameHUD"
    );
    assert!(
        body.contains("pres.apply_to_game_hud(self.ui_manager.game_hud_mut())"),
        "must apply presentation freeze to UIManager GameHUD"
    );
    // Body must not recurse into itself (stack overflow residual).
    let after_sig = match body.split_once('{') {
        Some((_, rest)) => rest,
        None => "",
    };
    assert!(
        !after_sig.contains("self.apply_presentation_to_huds("),
        "apply_presentation_to_huds must not call itself"
    );
}

#[test]
fn live_letterbox_overlay_queues_scripted_camera_fade() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let i = src
        .find("fn queue_live_letterbox_and_cinematic_overlays")
        .expect("letterbox overlay helper");
    let body = &src[i..src.len().min(i + 1800)];
    assert!(
        body.contains("queue_live_camera_fade"),
        "live overlay queue must stamp scripted CAMERA_FADE for the 3D blit"
    );
    assert!(
        body.contains("pres.camera_fade"),
        "fade overlay must read frozen PresentationFrame.camera_fade"
    );
}

use super::{
    CnCGameEngine, GameMode, GameState, StartupNewGameDispatch, should_exit_for_smoke_test,
    should_keep_logic_running_while_iconic,
};
use crate::command_line::CommandLineArgs;
use game_engine::common::global_data::{
    test_isolation_lock, with_global_data_restored as with_global_data_snapshot_restored,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn with_global_and_startup_state_snapshot_restored<F: FnOnce()>(f: F) {
    let _guard = test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let global_snapshot = game_engine::common::global_data::read().clone();
    let previous_difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
    let previous_rank_points =
        gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
    let previous_session =
        crate::game_logic::host_faction_skirmish_residual::live_host_session_difficulty();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    *game_engine::common::global_data::write() = global_snapshot;
    gamelogic::helpers::TheScriptEngine::set_global_difficulty(previous_difficulty);
    gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(previous_rank_points);
    crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(
        match previous_session {
            Some(crate::ai::AIDifficulty::Easy) => 0,
            Some(crate::ai::AIDifficulty::Medium) => 1,
            Some(crate::ai::AIDifficulty::Hard) => 2,
            Some(crate::ai::AIDifficulty::Brutal) => 3,
            None => i32::MIN,
        },
    );
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn create_temp_test_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "generals_main_{prefix}_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn startup_deferred_budget_is_disabled() {
    let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, None, 0);
    assert_eq!(budget, 0);
}

#[test]
fn startup_deferred_budget_is_enabled_for_visible_menu_frames() {
    let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(12), 12);
    assert_eq!(budget, 4);
}

#[test]
fn smoke_test_exit_only_after_menu_startup_complete() {
    assert!(should_exit_for_smoke_test(
        true,
        GameState::Menu,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        false,
        GameState::Menu,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Loading,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Menu,
        0.995,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Menu,
        1.0,
        true
    ));
}

#[test]
fn configured_startup_shell_map_disables_missing_shell_map() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name = "__definitely_missing_shell_map__".to_string();
        }

        let shell_map = CnCGameEngine::configured_startup_shell_map();
        assert!(shell_map.is_none());

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.shell_map_on);
    });
}

#[test]
fn windowed_menu_attempts_shellmapmd_when_named() {
    // C++ Shell::showShellMap(TRUE) (Shell.cpp:448-466) starts GAME_SHELL
    // with m_shellMapName on the main menu. Pre-fix windowed boot skipped
    // that decode whenever start_in_menu was true (hq-4yc1).
    assert!(
        CnCGameEngine::windowed_shell_map_load_decision(
            true,
            Some(CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP),
        ),
        "windowed main menu must attempt ShellMapMD when a name is configured"
    );
    assert!(
        !CnCGameEngine::windowed_shell_map_load_decision(true, None),
        "missing shell-map name must fail-soft so Menu can still open"
    );
    assert!(
        !CnCGameEngine::windowed_shell_map_load_decision(true, Some("   ")),
        "blank shell-map name must fail-soft"
    );
}

#[test]
fn configured_startup_shell_map_uses_disk_shellmapmd_when_cache_empty() {
    // C++ GameEngine.cpp:633-642 only disables ShellMapOn when MapCache
    // lacks m_shellMapName. An empty boot cache must still accept a
    // disk-resident ShellMapMD extract (hq-4yc1).
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name.clear();
        }

        let resolved_asset = CnCGameEngine::resolve_windowed_shell_map_asset(
            CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP,
        );
        let shell_map = CnCGameEngine::configured_startup_shell_map();
        if let Some(resolved) = resolved_asset {
            let selected = shell_map.expect("disk-resident ShellMapMD must be selected");
            assert!(
                selected.eq_ignore_ascii_case(&resolved)
                    || selected.to_ascii_lowercase().contains("shellmapmd"),
                "windowed boot must select ShellMapMD (got {selected})"
            );
            assert!(
                game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        } else {
            assert!(
                shell_map.is_none(),
                "missing ShellMapMD asset must fail-soft (documented asset gate)"
            );
            assert!(
                !game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        }
    });
}

#[test]
fn configured_startup_shell_map_remaps_stale_shellmap1_to_shellmapmd() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name = r"Maps\ShellMap1\ShellMap1.map".to_string();
        }
        let resolved_md = CnCGameEngine::resolve_windowed_shell_map_asset(
            CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP,
        )
        .or_else(|| CnCGameEngine::resolve_windowed_shell_map_asset("ShellMapMD"));
        let selected = CnCGameEngine::configured_startup_shell_map();
        if resolved_md.is_some() {
            let selected = selected.expect("stale ShellMap1 must remap to ShellMapMD");
            assert!(
                selected.to_ascii_lowercase().contains("shellmapmd"),
                "got {selected}"
            );
            assert!(
                game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        }
    });
}

#[test]
fn windowed_boot_source_loads_shell_map_instead_of_skipping() {
    let src = include_str!("shell.rs");
    let worker = src
        .split("pub(super) fn spawn_startup_map_load(")
        .nth(1)
        .and_then(|s| s.split("pub(super) fn finalize_startup_map_load(").next())
        .expect("startup worker bounded");
    assert!(
        worker.contains("windowed_shell_map_load_decision")
            && worker.contains("load_map_with_progress")
            && worker.contains("Shell.cpp:448-466"),
        "windowed boot worker must fail-soft load ShellMapMD like C++ showShellMap"
    );
    assert!(
        !worker.contains("start_in_menu: skipping blocking shell-map load"),
        "windowed Menu must not skip a named ShellMapMD decode"
    );

    let finalize = src
        .split("pub(super) fn host_finalize_startup_map_load(")
        .nth(1)
        .and_then(|s| s.split("pub(super) fn abandon_startup_load_worker(").next())
        .expect("finalize bounded");
    let apply = finalize
        .find("if let Some(active_map_name) = result.loaded_map_name.as_ref()")
        .expect("shell map presentation apply");
    let menu = finalize
        .find("let fallback_to_menu = result.start_in_menu")
        .expect("menu fallback");
    assert!(
        apply < menu,
        "C++ shell map backdrop must be installed before Menu state"
    );
}

#[test]
fn effective_fps_limit_prefers_script_override() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(Some(45), false, 30, 2.0, true, true);
    assert_eq!(limit, Some(45));
}

#[test]
fn effective_fps_limit_honors_cpp_tivo_replay_rule_for_global_limit() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.0, true, true);
    assert_eq!(limit, None);
}

#[test]
fn effective_fps_limit_disables_global_limit_for_fast_visual_multiplier() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.5, false, false);
    assert_eq!(limit, None);
}

#[test]
fn startup_new_game_dispatch_prefers_last_queued_message() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    let mut first = GameMessage::new(GameMessageType::NewGame);
    first.append_integer_argument(0);
    first.append_integer_argument(0);
    first.append_integer_argument(0);

    let mut replay = GameMessage::new(GameMessageType::NewGame);
    replay.append_integer_argument(3);
    replay.append_integer_argument(1);
    replay.append_integer_argument(42);
    replay.append_integer_argument(90);

    let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
        first,
        GameMessage::new(GameMessageType::ClearGameData),
        replay,
    ])
    .expect("expected startup dispatch");

    assert_eq!(dispatch.game_mode, GameMode::Replay);
    assert_eq!(dispatch.difficulty, super::GameDifficulty::Medium);
    assert_eq!(dispatch.rank_points, 42);
    assert_eq!(dispatch.max_fps, Some(90));
}

#[test]
fn startup_new_game_dispatch_applies_script_side_effects() {
    with_global_and_startup_state_snapshot_restored(|| {
        let dispatch = StartupNewGameDispatch {
            game_mode_code: 0,
            game_mode: GameMode::SinglePlayer,
            difficulty_code: 2,
            difficulty: super::GameDifficulty::Hard,
            rank_points: 77,
            max_fps: None,
        };

        let prepared_map = CnCGameEngine::apply_startup_new_game_dispatch(dispatch);
        assert!(prepared_map.is_none());
        assert_eq!(
            gamelogic::helpers::TheScriptEngine::get_global_difficulty(),
            2
        );
        assert_eq!(
            crate::game_logic::host_faction_skirmish_residual::live_host_session_difficulty(),
            Some(crate::ai::AIDifficulty::Hard)
        );
        assert_eq!(
            gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start(),
            77
        );
    });
}

#[test]
fn restart_mission_reposts_new_game_payload_like_cpp() {
    // C++ QuitMenu.cpp:211-216 appends MSG_NEW_GAME(mode, diff, rank, fps).
    let quit = include_str!("quit_menu_bridge.rs");
    let restart = include_str!("camera_drain.rs");
    let dispatch = include_str!("dispatch.rs");
    assert!(
        quit.contains("host_restart_mission_from_dispatch(dispatch)"),
        "QuitMenu restart must keep the NewGame dispatch payload"
    );
    assert!(
        !quit.contains("host_restart_mission_from_ui()"),
        "QuitMenu must not strip NewGame into without_player_template restart"
    );
    assert!(
        restart.contains("host_restart_mission_from_dispatch(identity.dispatch)"),
        "host restart must replay last NewGame difficulty/rank/template"
    );
    assert!(
        dispatch.contains("record_last_new_game_identity")
            && dispatch.contains("player_template: request.player_template.clone()"),
        "NewGame drain must remember Challenge PlayerTemplate for restart"
    );
}

#[test]
fn startup_new_game_dispatch_requires_pending_file_for_startup_map_preparation() {
    with_global_and_startup_state_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.map_name = "Maps\\Unexpected\\Unexpected.map".to_string();
            global.pending_file.clear();
        }

        let dispatch = StartupNewGameDispatch {
            game_mode_code: 0,
            game_mode: GameMode::SinglePlayer,
            difficulty_code: 1,
            difficulty: super::GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        };

        let prepared_map = CnCGameEngine::apply_startup_new_game_dispatch(dispatch);
        assert!(prepared_map.is_none());

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, "Maps\\Unexpected\\Unexpected.map");
        assert!(global.pending_file.is_empty());
    });
}

#[test]
fn map_select_new_game_dispatch_consumes_the_exact_runtime_pending_map() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    with_global_and_startup_state_snapshot_restored(|| {
        let expected_map = "Maps\\Official\\MapSelectExact.map".to_string();
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.map_name = "Maps\\Stale\\Stale.map".to_string();
            global.pending_file = expected_map.clone();
        }

        let mut message = GameMessage::new(GameMessageType::NewGame);
        message.append_integer_argument(0); // GAME_SINGLE_PLAYER
        message.append_integer_argument(1); // DIFFICULTY_NORMAL
        message.append_integer_argument(0);
        let dispatch = CnCGameEngine::startup_new_game_dispatch_from_message(&message)
            .expect("MapSelect NewGame must decode");

        assert_eq!(dispatch.game_mode, GameMode::SinglePlayer);
        assert_eq!(
            CnCGameEngine::apply_startup_new_game_dispatch(dispatch),
            Some(expected_map.clone())
        );

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, expected_map);
        assert!(global.pending_file.is_empty());
    });
}

#[cfg(feature = "game_client")]
#[test]
fn campaign_launch_descriptor_precedes_stale_map_and_hud_faction() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 7,
        map_name: "Maps\\Campaign\\ExactLaunch.map".to_string(),
        campaign_name: "USA".to_string(),
        // Deliberately differs from the campaign name: C++'s campaign player
        // faction is more specific than a stale HUD/default fallback.
        campaign_player_faction: "FactionChina".to_string(),
        is_challenge: false,
        player_template_name: None,
        player_template_index: None,
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: None,
    };

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor))
            .expect("ordinary campaign descriptor must resolve");
    assert_eq!(
        overrides.map.as_deref(),
        Some("Maps\\Campaign\\ExactLaunch.map")
    );
    assert_eq!(overrides.faction.as_deref(), Some("China"));
    assert_eq!(
        overrides
            .player_template
            .as_ref()
            .map(|identity| identity.template_name.as_str()),
        Some("FactionChina"),
        "a normal campaign PlayerFaction that exactly resolves must retain its template"
    );

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::Skirmish, Some(&descriptor))
            .expect("non-single-player dispatch ignores the campaign descriptor");
    assert!(overrides.map.is_none());
    assert!(overrides.faction.is_none());
    assert!(overrides.player_template.is_none());
}

#[cfg(feature = "game_client")]
#[test]
fn challenge_launch_rejects_a_missing_or_unpaired_selected_general() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 9,
        map_name: "Maps\\Challenge\\Exact.map".to_string(),
        campaign_name: "CHALLENGE_0".to_string(),
        campaign_player_faction: "FactionAmericaAirForceGeneral".to_string(),
        is_challenge: true,
        player_template_name: None,
        player_template_index: None,
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: Some(30),
    };

    assert!(
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor),)
            .is_err()
    );

    let source = include_str!("dispatch.rs");
    let rejection = &source[source
        .find("Rejecting Challenge MSG_NEW_GAME")
        .expect("Challenge rejection branch")..];
    assert!(
        rejection.contains("Self::clear_pending_campaign_start_map()"),
        "a rejected typed Challenge launch must not leak its mirrored pending map"
    );
}

#[cfg(feature = "game_client")]
#[test]
fn challenge_launch_retains_the_exact_selected_template_name_and_index() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    game_engine::common::ini::ensure_player_templates_loaded();
    let (tank_index, air_force_index) = {
        let store = game_engine::common::rts::player_template::get_player_template_store();
        (
            store
                .find_template_index("FactionChinaTankGeneral")
                .expect("retail Tank General template") as i32,
            store
                .find_template_index("FactionAmericaAirForceGeneral")
                .expect("retail Air Force General template") as i32,
        )
    };

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 10,
        map_name: "Maps\\Challenge\\ExactTank.map".to_string(),
        campaign_name: "CHALLENGE_0".to_string(),
        campaign_player_faction: "FactionChinaTankGeneral".to_string(),
        is_challenge: true,
        player_template_name: Some("FactionChinaTankGeneral".to_string()),
        player_template_index: Some(tank_index),
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: Some(30),
    };

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor))
            .expect("matched Challenge template must resolve");
    let identity = overrides
        .player_template
        .expect("Challenge must carry exact PlayerTemplate identity");
    assert_eq!(identity.template_name, "FactionChinaTankGeneral");
    assert_eq!(identity.template_index, Some(tank_index));
    assert_eq!(overrides.faction.as_deref(), Some("China"));

    let stale = HostCampaignLaunchDescriptor {
        player_template_index: Some(air_force_index),
        ..descriptor
    };
    assert!(
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&stale),)
            .is_err()
    );
}

#[test]
fn campaign_faction_identity_maps_only_exact_cpp_base_names() {
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("FactionAmerica").as_deref(),
        Some("USA")
    );
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("china").as_deref(),
        Some("China")
    );
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("FactionGLA").as_deref(),
        Some("GLA")
    );
    assert!(CnCGameEngine::base_faction_from_campaign_faction("TankGeneral").is_none());
}

#[test]
fn startup_new_game_dispatch_ignores_unrelated_messages() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
        GameMessage::new(GameMessageType::Invalid),
        GameMessage::new(GameMessageType::ClearGameData),
    ]);

    assert!(dispatch.is_none());
}

#[test]
fn take_new_game_dispatch_drains_stream_and_keeps_other_messages() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType, get_message_stream};

    let stream = get_message_stream();
    {
        let mut g = stream.write().unwrap_or_else(|e| e.into_inner());
        g.clear_messages();
        g.append_message(GameMessageType::ClearGameData);
        let ng = g.append_message(GameMessageType::NewGame);
        ng.append_integer_argument(2); // GAME_SKIRMISH
        ng.append_integer_argument(1);
        ng.append_integer_argument(0);
        ng.append_integer_argument(30);
        g.append_message(GameMessageType::Invalid);
    }

    let dispatch = CnCGameEngine::take_new_game_dispatch_from_common_stream()
        .expect("NewGame must be drained");
    assert_eq!(dispatch.game_mode, GameMode::Skirmish);
    assert_eq!(dispatch.max_fps, Some(30));

    let g = stream.read().unwrap_or_else(|e| e.into_inner());
    assert_eq!(g.message_count(), 2, "non-NewGame messages must remain");
    let types: Vec<_> = g
        .get_messages()
        .iter()
        .map(|m| m.get_type().clone())
        .collect();
    assert!(
        types
            .iter()
            .any(|t| matches!(t, GameMessageType::ClearGameData))
    );
    assert!(types.iter().any(|t| matches!(t, GameMessageType::Invalid)));
    assert!(!types.iter().any(|t| matches!(t, GameMessageType::NewGame)));
    // silence unused import if GameMessage only used above via type
    let _ = GameMessage::new(GameMessageType::Invalid);
}

#[test]
fn take_clear_game_data_drains_stream_and_keeps_other_messages() {
    // C++ ScriptEngine.cpp:5514-5518 appends MSG_CLEAR_GAME_DATA; Main must
    // consume it so scripted VICTORY/DEFEAT actually ends the live match.
    use game_engine::common::message_stream::{GameMessageType, get_message_stream};

    let stream = get_message_stream();
    {
        let mut g = stream.write().unwrap_or_else(|e| e.into_inner());
        g.clear_messages();
        g.append_message(GameMessageType::Invalid);
        g.append_message(GameMessageType::ClearGameData);
    }

    assert!(CnCGameEngine::take_clear_game_data_from_common_stream());
    {
        let g = stream.read().unwrap_or_else(|e| e.into_inner());
        let types: Vec<_> = g
            .get_messages()
            .iter()
            .map(|m| m.get_type().clone())
            .collect();
        assert!(
            !types
                .iter()
                .any(|t| matches!(t, GameMessageType::ClearGameData)),
            "ClearGameData must be consumed so the match can end"
        );
        assert!(types.iter().any(|t| matches!(t, GameMessageType::Invalid)));
    }
    assert!(!CnCGameEngine::take_clear_game_data_from_common_stream());
}

#[test]
fn clear_game_data_pushes_score_screen_not_main_menu() {
    // C++ GameLogicDispatch.cpp:223-253 / :439 default showScoreScreen.
    assert!(CnCGameEngine::clear_game_data_should_push_score_screen(
        false, true, true
    ));
    assert!(
        !CnCGameEngine::clear_game_data_should_push_score_screen(true, true, true),
        "in-shell + in-game must not push ScoreScreen"
    );
    assert!(!CnCGameEngine::clear_game_data_should_push_score_screen(
        false, true, false
    ));

    let src = include_str!("dispatch.rs");
    let consume = src
        .split("fn host_consume_clear_game_data")
        .nth(1)
        .and_then(|s| s.split("fn host_restart_mission_from_dispatch").next())
        .expect("host_consume_clear_game_data");
    assert!(
        consume.contains("host_push_score_screen_like_cpp")
            && !consume.contains("return_to_main_menu_after_match"),
        "MSG_CLEAR_GAME_DATA must push ScoreScreen, not Main Menu"
    );
    assert!(
        src.contains("show_shell(false)")
            && src.contains("Menus/ScoreScreen.wnd")
            && src.contains("fn host_push_score_screen_like_cpp"),
        "clearGameData must showShell(FALSE) after ScoreScreen push"
    );
}

#[test]
fn peek_new_game_leaves_message_for_propagate_messages() {
    // C++ GameLogic::logicMessageDispatcher MSG_NEW_GAME
    // (GameLogicDispatch.cpp:396-423) consumes the streamed message after
    // MessageStream::propagateMessages. Host start peeks so pump can still
    // deliver NewGame to crate GameLogic.
    use game_engine::common::message_stream::{GameMessageType, get_message_stream};

    let stream = get_message_stream();
    {
        let mut g = stream.write().unwrap_or_else(|e| e.into_inner());
        g.clear_messages();
        g.append_message(GameMessageType::ClearGameData);
        let ng = g.append_message(GameMessageType::NewGame);
        ng.append_integer_argument(2); // GAME_SKIRMISH
        ng.append_integer_argument(1);
        ng.append_integer_argument(0);
        ng.append_integer_argument(30);
    }

    let peeked = CnCGameEngine::peek_new_game_dispatch_from_common_stream()
        .expect("NewGame must be visible without removing it");
    assert_eq!(peeked.game_mode, GameMode::Skirmish);
    assert_eq!(peeked.max_fps, Some(30));

    {
        let g = stream.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            g.get_messages()
                .iter()
                .any(|m| matches!(m.get_type(), GameMessageType::NewGame)),
            "peek must leave MSG_NEW_GAME on the stream for pump_message_stream"
        );
    }

    let _ = CnCGameEngine::take_new_game_dispatch_from_common_stream();
}

#[test]
fn menu_shell_pumps_new_game_after_host_start() {
    // C++ GameLogicDispatch.cpp:396 MSG_NEW_GAME is delivered via the
    // command list, not stripped before MessageStream::propagateMessages.
    let src = include_str!("camera_drain.rs");
    let start = src
        .find("fn host_tick_game_client_menu_shell(")
        .expect("menu shell helper");
    let body = &src[start..];
    let peek = body
        .find("take_pending_new_game_start_request")
        .expect("host peek");
    let start_ui = body
        .find("start_game_from_ui(request)")
        .expect("host start");
    let pump = body[start_ui..]
        .find("pump_message_stream")
        .map(|offset| start_ui + offset)
        .expect("pump after host start");
    assert!(
        peek < start_ui && start_ui < pump,
        "host start must peek NewGame, then pump so crate GameLogic sees MSG_NEW_GAME"
    );
}

#[test]
fn menu_does_not_skip_world_scene_after_warmup() {
    // Loading is the only state that may omit the 3D pass.
    let src = include_str!("shell.rs");
    let start = src
        .find("fn should_skip_world_scene_for_shell_menu")
        .expect("skip fn");
    let body = &src[start..src.len().min(start + 700)];
    assert!(
        body.contains("GameState::Loading"),
        "Loading may skip the world"
    );
    assert!(
        !body.contains("menu_world_frames_rendered >="),
        "Menu must keep drawing the shell map"
    );
}

#[test]
fn startup_camera_metadata_uses_xz_ground_not_xy() {
    let src = include_str!("shell.rs");
    let start = src
        .find("fn bootstrap_camera_for_loaded_map")
        .expect("bootstrap camera");
    let body = &src[start..src.len().min(start + 2200)];
    assert!(
        body.contains("Vec2::new(pos.x, pos.z)"),
        "InitialCamera ground focus is X/Z, not X/Y"
    );
    assert!(
        !body.contains("Vec2::new(pos.x, pos.y)"),
        "must not treat camera height as a map axis"
    );
}

#[test]
fn startup_camera_focus_prefers_shell_metadata_before_default_seed() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        true,
        Some(glam::Vec2::new(12.0, 34.0)),
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(focus, glam::Vec2::new(12.0, 34.0));
}

#[test]
fn startup_camera_focus_falls_back_to_shell_seed_without_metadata() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        true,
        None,
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(
        focus,
        glam::Vec2::new(
            87.0 * gamelogic::common::MAP_XY_FACTOR,
            77.0 * gamelogic::common::MAP_XY_FACTOR,
        )
    );
}

#[test]
fn startup_camera_focus_keeps_non_shell_fallback_order() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        false,
        None,
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(focus, glam::Vec2::new(56.0, 78.0));
}

#[test]
fn startup_mode_requires_new_game_dispatch_for_non_menu_startup() {
    let mut start_in_menu = false;
    let mut map_to_load = Some("Maps\\ShellMapMD\\ShellMapMD.map".to_string());

    let mode = CnCGameEngine::resolve_startup_mode_from_dispatch(
        &mut start_in_menu,
        &mut map_to_load,
        None,
        false,
    );

    assert_eq!(mode, GameMode::Shell);
    assert!(start_in_menu);
    assert!(map_to_load.is_none());
}

#[test]
fn startup_initial_file_helper_matches_cpp_table_and_gating() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.initial_file.clear();
        }

        let replay_args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Replays\\demo.rep".to_string(),
        ];
        let replay_parsed = CommandLineArgs::parse_from_args(replay_args).unwrap();
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, true),
            Some("Replays\\demo.rep".to_string())
        );
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, false),
            None
        );

        let replay_alias_args = vec![
            "generals".to_string(),
            "-replay".to_string(),
            "Replays\\demo.rep".to_string(),
        ];
        let replay_alias_parsed = CommandLineArgs::parse_from_args(replay_alias_args).unwrap();
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_alias_parsed, true),
            None
        );
    });
}

#[test]
fn startup_initial_file_helper_prefers_runtime_initial_file_state() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.initial_file = "Replays\\runtime.rep".to_string();
        }

        let cli_args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Maps\\cli\\cli.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(cli_args).unwrap();

        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&parsed, true),
            Some("Replays\\runtime.rep".to_string())
        );
    });
}

#[test]
fn startup_initial_file_split_matches_cpp_suffix_rules() {
    let (map_file, replay_file) =
        CnCGameEngine::split_startup_initial_file(Some("Maps\\Test\\Test.map".to_string()));
    assert_eq!(map_file, Some("Maps\\Test\\Test.map".to_string()));
    assert!(replay_file.is_none());

    let (map_file, replay_file) =
        CnCGameEngine::split_startup_initial_file(Some("Replays\\demo.rep".to_string()));
    assert!(map_file.is_none());
    assert_eq!(replay_file, Some("Replays\\demo.rep".to_string()));
}

#[test]
fn apply_command_line_overrides_keeps_initial_map_side_effects_until_startup_handling() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Maps\\Test\\Test.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.initial_file, "Maps\\Test\\Test.map");
        assert!(global.pending_file.is_empty());
        assert!(global.writable.shell_map_on);
        assert!(global.writable.play_intro);
        assert!(!global.writable.after_intro);
    });
}

#[test]
fn sync_after_intro_when_intro_disabled_marks_after_intro() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.play_intro = false;
            global.writable.after_intro = false;
        }

        CnCGameEngine::sync_after_intro_when_intro_disabled();

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.play_intro);
        assert!(global.writable.after_intro);
    });
}

#[test]
fn game_logic_gate_without_network_matches_cpp_pause_behavior() {
    assert!(CnCGameEngine::should_update_game_logic_frame(false, None));
    assert!(!CnCGameEngine::should_update_game_logic_frame(true, None));
}

#[test]
fn game_logic_gate_with_network_uses_frame_ready_only() {
    assert!(CnCGameEngine::should_update_game_logic_frame(
        false,
        Some(true)
    ));
    assert!(CnCGameEngine::should_update_game_logic_frame(
        true,
        Some(true)
    ));
    assert!(!CnCGameEngine::should_update_game_logic_frame(
        false,
        Some(false)
    ));
    assert!(!CnCGameEngine::should_update_game_logic_frame(
        true,
        Some(false)
    ));
}

#[test]
fn network_gate_skips_runtime_network_lookup_until_multiplayer_exists() {
    assert_eq!(CnCGameEngine::network_frame_data_ready_gate(false), None);
}

#[test]
fn iconic_minimized_mode_keeps_network_sessions_running() {
    assert!(should_keep_logic_running_while_iconic(
        GameMode::Multiplayer
    ));
    assert!(should_keep_logic_running_while_iconic(GameMode::Lan));
    assert!(should_keep_logic_running_while_iconic(GameMode::Internet));
    assert!(!should_keep_logic_running_while_iconic(
        GameMode::SinglePlayer
    ));
    assert!(!should_keep_logic_running_while_iconic(GameMode::Skirmish));
    assert!(!should_keep_logic_running_while_iconic(GameMode::Shell));
}

#[test]
fn command_line_fps_order_matches_cpp_fps_then_nofpslimit() {
    let args = vec![
        "generals".to_string(),
        "-fps".to_string(),
        "60".to_string(),
        "-nofpslimit".to_string(),
    ];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
    assert!(!writable.use_fps_limit);
    assert_eq!(writable.frames_per_second_limit, 30000);
}

#[test]
fn command_line_fps_order_matches_cpp_nofpslimit_then_fps() {
    let args = vec![
        "generals".to_string(),
        "-nofpslimit".to_string(),
        "-fps".to_string(),
        "60".to_string(),
    ];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
    assert!(!writable.use_fps_limit);
    assert_eq!(writable.frames_per_second_limit, 60);
}

#[test]
fn command_line_window_resolution_overrides_sync_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-win".to_string(),
            "-xres".to_string(),
            "1280".to_string(),
            "-yres".to_string(),
            "720".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.windowed);
        assert_eq!(global.writable.x_resolution, 1280);
        assert_eq!(global.writable.y_resolution, 720);
    });
}

#[test]
fn command_line_noaudio_overrides_sync_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-noaudio".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.audio_on);
        assert!(!global.writable.speech_on);
        assert!(!global.writable.sounds_on);
        assert!(!global.writable.music_on);
    });
}

#[test]
fn command_line_startup_parity_flags_apply_in_argv_order() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-particleEdit".to_string(),
            "-fullscreen".to_string(),
            "-benchmark".to_string(),
            "9".to_string(),
            "-playStats".to_string(),
            "4".to_string(),
            "-seed".to_string(),
            "-1".to_string(),
            "-netMinPlayers".to_string(),
            "3".to_string(),
            "-forceBenchmark".to_string(),
            "-nomusic".to_string(),
            "-noshaders".to_string(),
            "-scriptDebug".to_string(),
            "-winCursors".to_string(),
            "-constantDebug".to_string(),
            "-showTeamDot".to_string(),
            "-nomovecamera".to_string(),
            "-NoShellAnim".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.windowed);
        assert!(global.writable.particle_edit);
        assert!(global.writable.script_debug);
        assert!(global.writable.win_cursors);
        assert!(!global.writable.animate_windows);
        assert!(!global.writable.music_on);
        assert!(global.writable.play_sizzle);
        assert_eq!(global.writable.chip_set_type, 1);
        assert!(global.writable.force_benchmark);
        assert!(global.writable.constant_debug_update);
        assert!(global.writable.show_team_dot);
        assert!(global.writable.disable_camera_movement);
        assert_eq!(global.writable.fixed_seed, -1);
        assert_eq!(global.writable.net_min_players, 3);
        assert_eq!(global.writable.benchmark_timer, 9);
        assert_eq!(global.writable.play_stats, 4);
    });
}

#[test]
fn command_line_standalone_nosizzle_is_ignored_during_startup_overrides() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-nosizzle".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.play_sizzle);
    });
}

#[test]
fn command_line_jump_to_frame_matches_cpp_no_draw_behavior() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-jumpToFrame".to_string(),
            "240".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        let debug_gated = CnCGameEngine::allow_debug_startup_flags();
        assert_eq!(global.writable.no_draw, debug_gated);
        if debug_gated {
            assert!(!global.writable.use_fps_limit);
            assert_eq!(global.writable.frames_per_second_limit, 30000);
        }
    });
}

#[test]
fn startup_water_weather_preload_paths_match_cpp_order() {
    assert_eq!(
        CnCGameEngine::startup_water_weather_ini_paths(),
        [
            "Data/INI/Default/Water.ini",
            "Data/INI/Water.ini",
            "Data/INI/Default/Weather.ini",
            "Data/INI/Weather.ini",
        ]
    );
}

#[test]
fn preload_startup_water_weather_inis_returns() {
    // Shipped boot path: must return (fail-open on missing files). A hang here
    // is the observed Loading stall at "Preloading water and weather settings".
    let start = std::time::Instant::now();
    CnCGameEngine::preload_startup_water_weather_inis();
    assert!(
        start.elapsed() < std::time::Duration::from_secs(15),
        "preload_startup_water_weather_inis must return quickly, elapsed {:?}",
        start.elapsed()
    );
}

#[test]
fn startup_ai_data_preload_paths_match_cpp_order() {
    assert_eq!(
        CnCGameEngine::startup_ai_data_ini_paths(),
        ["Data/INI/Default/AIData.ini", "Data/INI/AIData.ini",]
    );
}

#[test]
fn startup_audio_failure_quits_only_when_audio_is_enabled() {
    assert!(CnCGameEngine::startup_audio_should_quit(false, false));
    assert!(!CnCGameEngine::startup_audio_should_quit(true, false));
    assert!(!CnCGameEngine::startup_audio_should_quit(false, true));
}

#[test]
fn debug_startup_flag_gating_matches_build_mode() {
    assert_eq!(
        CnCGameEngine::allow_debug_startup_flags(),
        cfg!(any(debug_assertions, feature = "internal"))
    );
}

#[test]
fn command_line_map_override_syncs_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-map".to_string(),
            "Maps\\ShellMap1.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, "Maps\\ShellMap1\\ShellMap1.map");
    });
}

#[test]
fn command_line_mod_override_updates_active_mod_and_loads_best_effort() {
    with_global_data_snapshot_restored(|| {
        let temp_root = create_temp_test_dir("mod_override");
        let user_data_dir = temp_root.join("UserData");
        let mod_dir = user_data_dir.join("Mods").join("TestMod");
        std::fs::create_dir_all(&mod_dir).unwrap();

        {
            let mut global = game_engine::common::global_data::write();
            global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
        }

        let args = vec![
            "generals".to_string(),
            "-mod".to_string(),
            std::path::Path::new("Mods")
                .join("TestMod")
                .to_string_lossy()
                .into_owned(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let expected = format!("{}{}", mod_dir.to_string_lossy(), std::path::MAIN_SEPARATOR);
        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.mod_dir, expected);
        assert!(global.writable.mod_big.is_empty());
        assert_eq!(
            global
                .get_override("active_mod")
                .and_then(|value| value.as_str()),
            Some(expected.as_str())
        );

        let _ = fs::remove_dir_all(temp_root);
    });
}

#[test]
fn command_line_update_images_sets_writable_flag() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-updateimages".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.should_update_tga_to_dds);
    });
}

#[test]
fn command_line_update_images_alias_is_case_insensitive() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-UpDaTeDdS".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.should_update_tga_to_dds);
    });
}
