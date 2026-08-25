// Mechanical extract from cnc_game_engine.rs `interactive_playability_evidence_tests`.
// Child module via `#[path]`.
//
// These exercise the **shipped** `note_*` setters (fail-closed contracts). Host
// inject must re-enter handle_mouse_button_input; forge paths that call note_*
// with hardcoded trues are rejected by source-scan + host-cmd tests below.

use super::InteractivePlayabilityEvidence;

#[test]
fn scripted_or_hover_only_input_cannot_claim_retail_navigation() {
    let mut evidence = InteractivePlayabilityEvidence::default();
    evidence.note_menu_wnd_click(true, true, false);
    evidence.note_skirmish_path_gadget(true, "MainMenu.wnd:ButtonSkirmish");
    evidence.note_offline_match_started(true, true);
    evidence.note_gameplay_order(true, true);

    assert!(!evidence.wnd_menu_to_match_complete());
    assert!(!evidence.gameplay_complete());
}

#[test]
fn physical_offline_menu_to_match_and_order_completes_evidence() {
    // handle_mouse_button_input only latches when windowed+consumed+hit all true.
    let mut evidence = InteractivePlayabilityEvidence::default();
    evidence.note_menu_wnd_click(true, true, true);
    evidence.note_skirmish_path_gadget(true, "MainMenu.wnd:ButtonSkirmish");
    evidence.note_offline_match_started(true, true);
    evidence.note_gameplay_order(true, true);

    assert!(evidence.wnd_menu_to_match_complete());
    assert!(evidence.gameplay_complete());
}

#[test]
fn network_or_headless_paths_do_not_complete_retail_evidence() {
    let mut evidence = InteractivePlayabilityEvidence::default();
    evidence.note_menu_wnd_click(false, true, true);
    evidence.note_skirmish_path_gadget(false, "MainMenu.wnd:ButtonSkirmish");
    evidence.note_offline_match_started(true, false);
    evidence.note_gameplay_order(false, true);

    assert!(!evidence.wnd_menu_to_match_complete());
    assert!(!evidence.gameplay_complete());
}

#[test]
fn headless_or_miss_hit_cannot_latch_menu_click() {
    let mut a = InteractivePlayabilityEvidence::default();
    a.note_menu_wnd_click(false, true, true);
    assert!(!a.menu_wnd_click, "headless must fail closed");
    let mut b = InteractivePlayabilityEvidence::default();
    b.note_menu_wnd_click(true, false, true);
    assert!(!b.menu_wnd_click, "non-consumed click must fail closed");
    let mut c = InteractivePlayabilityEvidence::default();
    c.note_menu_wnd_click(true, true, false);
    assert!(!c.menu_wnd_click, "miss hit must fail closed");
}

#[test]
fn gameplay_order_without_selection_or_menu_match_does_not_latch() {
    let mut evidence = InteractivePlayabilityEvidence::default();
    // No prior menu→match, even with selection.
    evidence.note_gameplay_order(true, true);
    assert!(!evidence.gameplay_complete());
    assert!(!evidence.gameplay_order);

    evidence.note_menu_wnd_click(true, true, true);
    evidence.note_skirmish_path_gadget(true, "MainMenu.wnd:ButtonSkirmish");
    evidence.note_offline_match_started(true, true);
    // Menu→match done but no selection.
    evidence.note_gameplay_order(true, false);
    assert!(!evidence.gameplay_order);
    assert!(!evidence.gameplay_complete());

    // Full path: selection required.
    evidence.note_gameplay_order(true, true);
    assert!(evidence.gameplay_complete());
}

#[test]
fn physical_build_and_produce_requires_construct_then_production() {
    let mut evidence = InteractivePlayabilityEvidence::default();

    // A valid production request alone is insufficient: the proof is about a
    // real build-and-produce sequence, not generic factory interaction.
    evidence.note_control_bar_production(true);
    assert!(!evidence.build_and_produce_complete());

    // Injected/non-physical events cannot arm either half of the sequence.
    evidence.note_control_bar_construct_arm(false);
    evidence.note_control_bar_production(false);
    assert!(!evidence.build_and_produce_complete());

    evidence.note_control_bar_construct_arm(true);
    assert!(!evidence.build_and_produce_complete());

    // The physical production request must follow the physical construct arm.
    evidence.note_control_bar_production(true);
    assert!(evidence.build_and_produce_complete());
}

#[test]
fn control_bar_physical_proof_is_carried_by_request_provenance_not_from_user() {
    let src = include_str!("control_bar_bridge.rs");
    assert!(
        src.contains("take_host_control_bar_published_requests")
            && src.contains("is_physical_window_mouse_input"),
        "Main must consume the detailed request provenance rather than infer physical input"
    );
    assert!(
        src.contains("host_control_bar_evidence_eligible(physical_os_input)")
            && src.contains("note_control_bar_construct_arm")
            && src.contains("note_control_bar_production"),
        "only validated physical Control Bar actions may advance the build-and-produce proof"
    );
    assert!(
        src.contains("ui_object_is_dozer") && src.contains("local live dozer or worker"),
        "a stale generic selection must not count as a DozerConstruct arm"
    );
    assert!(
        src.contains("self.runtime_host_window_visible()"),
        "physical Control Bar evidence requires a real visible window, not merely a non-headless host"
    );
}

#[test]
fn physical_popup_save_load_requires_confirmed_physical_successes_in_order() {
    let mut evidence = InteractivePlayabilityEvidence::default();

    // A load alone cannot claim continuation, even if it is physical.
    evidence.note_popup_load_confirmation_succeeded(true);
    assert!(!evidence.save_load_continue_complete());

    // Runtime-host/injected actions fail closed and cannot supply either half.
    evidence.note_popup_save_confirmation_succeeded(false);
    evidence.note_popup_load_confirmation_succeeded(false);
    assert!(!evidence.save_load_continue_complete());

    evidence.note_popup_save_confirmation_succeeded(true);
    assert!(!evidence.save_load_continue_complete());

    // The load must also be a validated physical confirmation success.
    evidence.note_popup_load_confirmation_succeeded(false);
    assert!(!evidence.save_load_continue_complete());
    evidence.note_popup_load_confirmation_succeeded(true);
    assert!(evidence.save_load_continue_complete());
}

#[test]
fn popup_save_load_physical_proof_uses_published_confirmation_provenance_and_success() {
    let src = include_str!("runtime_host/gameplay.rs");
    assert!(
        src.contains("take_host_popup_save_load_published_requests")
            && src.contains("is_physical_window_mouse_input"),
        "Main must consume PopupSaveLoad's captured request provenance rather than infer physical input"
    );
    assert!(
        src.contains("host_popup_save_load_evidence_eligible(physical_os_input)")
            && src.contains("note_popup_save_confirmation_succeeded")
            && src.contains("note_popup_load_confirmation_succeeded"),
        "only validated physical Popup confirmations may advance save/load evidence"
    );
    assert!(
        src.contains("self.runtime_host_window_visible()")
            && src.contains("GameState::InGame | GameState::Paused")
            && src.contains("GameMode::SinglePlayer")
            && src.contains("GameMode::Skirmish"),
        "PopupSaveLoad evidence requires a visible offline match, including a real paused match"
    );

    let save_authority = src
        .find("match self.host_save_game_authority")
        .expect("Popup save authority branch");
    let save_ok = src[save_authority..]
        .find("Ok(()) =>")
        .map(|offset| save_authority + offset)
        .expect("Popup save success branch");
    let save_note = src[save_authority..]
        .find("note_popup_save_confirmation_succeeded")
        .map(|offset| save_authority + offset)
        .expect("Popup save evidence note");
    assert!(
        save_ok < save_note,
        "save evidence must be latched only after snapshot authority returns Ok"
    );

    let load_authority = src
        .find("match self.load_game_from_ui")
        .expect("Popup load authority branch");
    let load_ok = src[load_authority..]
        .find("Ok(()) =>")
        .map(|offset| load_authority + offset)
        .expect("Popup load success branch");
    let load_note = src[load_authority..]
        .find("note_popup_load_confirmation_succeeded")
        .map(|offset| load_authority + offset)
        .expect("Popup load evidence note");
    assert!(
        load_ok < load_note,
        "load evidence must be latched only after snapshot authority returns Ok"
    );
}

#[test]
fn options_chrome_is_not_skirmish_path_and_cannot_complete_wnd_nav() {
    let mut evidence = InteractivePlayabilityEvidence::default();
    evidence.note_menu_wnd_click(true, true, true);
    evidence.note_skirmish_path_gadget(true, "MainMenu.wnd:ButtonOptions");
    evidence.note_offline_match_started(true, true);
    assert!(!evidence.skirmish_path);
    assert!(!evidence.wnd_menu_to_match_complete());
}

#[test]
fn inject_miss_leaves_menu_click_false_and_claim_incomplete() {
    // Simulates inject_winit_equivalent_named_gadget_click miss: handle never
    // sees windowed+consumed+hit, so note_menu_wnd_click is never called with
    // all trues. start_game alone cannot complete menu→match.
    let mut evidence = InteractivePlayabilityEvidence::default();
    evidence.note_offline_match_started(true, true);
    assert!(!evidence.match_started_from_menu_wnd);
    assert!(!evidence.wnd_menu_to_match_complete());
    evidence.note_gameplay_order(true, true);
    assert!(!evidence.gameplay_complete());
}

#[test]
fn host_winit_menu_nav_must_call_named_gadget_inject_not_direct_note() {
    // Call-graph residual: production winit_menu_nav body.
    let src = include_str!("runtime_host/shell_core.rs");
    let start = src
        .find("fn runtime_host_cmd_winit_menu_nav")
        .expect("winit_menu_nav");
    let end = src[start..]
        .find("fn runtime_host_cmd_winit_gameplay_order")
        .map(|i| start + i)
        .unwrap_or(start + 2500);
    let body = &src[start..end];
    assert!(
        body.contains("inject_winit_equivalent_named_gadget_click"),
        "winit_menu_nav must call named-gadget inject"
    );
    assert!(
        body.contains("MainMenu.wnd:ButtonSinglePlayer")
            && body.contains("MainMenu.wnd:ButtonSkirmish"),
        "winit_menu_nav must target menu→match MainMenu gadgets"
    );
    // Parent/Ruler/Options are layout chrome — must not be nav success candidates.
    assert!(
        !body.contains("MainMenuParent")
            && !body.contains("MainMenuRuler")
            && !body.contains("ButtonOptions"),
        "winit_menu_nav must not treat Parent/Ruler/Options as menu→match success"
    );
    assert!(
        body.contains("shell_menu_active") && body.contains("winit_menu_nav_miss:shell_not_active"),
        "bare Menu without shell_menu_active must miss"
    );
    assert!(
        body.contains("no_menu_match_gadget") || body.contains("ButtonSinglePlayer"),
        "miss residual must require menu→match gadgets"
    );
    // Must not forge note_menu_wnd_click directly.
    assert!(
        !body.contains("note_menu_wnd_click"),
        "winit_menu_nav must not call note_menu_wnd_click (forge)"
    );
}

#[test]
fn host_winit_gameplay_order_must_call_inject_not_direct_note() {
    let src = include_str!("runtime_host/shell_core.rs");
    let start = src
        .find("fn runtime_host_cmd_winit_gameplay_order")
        .expect("winit_gameplay_order");
    let body = &src[start..src.len().min(start + 1800)];
    assert!(
        body.contains("inject_winit_equivalent_gameplay_order_click"),
        "winit_gameplay_order must call RMB inject"
    );
    assert!(
        !body.contains("note_gameplay_order"),
        "winit_gameplay_order must not call note_gameplay_order directly (forge)"
    );
    assert!(
        body.contains("runtime_host_headless"),
        "must fail closed when headless"
    );
}

#[test]
fn inject_named_gadget_click_routes_through_handle_mouse_button_input() {
    let src = include_str!("input.rs");
    let start = src
        .find("fn inject_winit_equivalent_named_gadget_click")
        .expect("inject named");
    let body = &src[start..src.len().min(start + 1500)];
    assert!(
        body.contains("named_gadget_center_if_hittable"),
        "inject must resolve a live named gadget"
    );
    assert!(
        body.contains("inject_winit_equivalent_mouse_button")
            || body.contains("handle_mouse_button_input"),
        "inject must re-enter the shared mouse latch"
    );
    assert!(
        !body.contains("note_menu_wnd_click"),
        "inject must not call note_menu_wnd_click itself"
    );
    // No skip-WM / preverified forge args on inject click path.
    assert!(
        !body.contains("skip_wm_dispatch")
            && !body.contains("preverified_gadget_hit")
            && !body.contains("true, true)"),
        "named inject must not pass skip-WM or preverified hit forges"
    );
}

#[test]
fn named_gadget_center_requires_under_cursor_hit_not_geometry_only() {
    let src = include_str!("input.rs");
    let start = src
        .find("fn named_gadget_center_if_hittable")
        .expect("named_gadget_center_if_hittable");
    let body = &src[start..src.len().min(start + 2200)];
    assert!(
        body.contains("get_window_under_cursor"),
        "named_gadget_center_if_hittable must under-cursor hit-test (not geometry-only)"
    );
    assert!(
        !body.contains("Skip\n            // get_window_under_cursor")
            && !body.contains("Skip get_window_under_cursor"),
        "must not document skipping under-cursor hit"
    );
}

#[test]
fn inject_mouse_button_uses_physical_handle_path_without_skip_wm() {
    let src = include_str!("input.rs");
    let start = src
        .find("fn inject_winit_equivalent_mouse_button")
        .expect("inject mouse button");
    let body = &src[start..src.len().min(start + 600)];
    assert!(
        body.contains("handle_mouse_button_input"),
        "inject must call handle_mouse_button_input"
    );
    // Injected origin — same handler, must not claim physical evidence.
    assert!(
        body.contains("MouseInputOrigin::Injected"),
        "inject must pass Injected origin so playable_claim is not latched"
    );
    assert!(
        !body.contains("skip_wm_dispatch") && !body.contains("preverified_gadget_hit"),
        "inject must not pass skip-WM or preverified hit forges"
    );
}

#[test]
fn handle_mouse_button_input_must_not_forge_wnd_used_or_skip_right_click() {
    let src = include_str!("input.rs");
    let start = src
        .find("fn handle_mouse_button_input")
        .expect("handle_mouse_button_input");
    // Through end of function (next pub fn inject cursor).
    let end = src[start..]
        .find("fn inject_winit_equivalent_cursor_at")
        .map(|i| start + i)
        .unwrap_or(start + 5000);
    let body = &src[start..end];
    assert!(
        body.contains("dispatch_os_mouse_to_window_manager"),
        "handle must use real WM dispatch for wnd_used"
    );
    assert!(
        body.contains("note_os_wnd_widget_tree_hit") || body.contains("live_hit"),
        "handle must live hit-test"
    );
    assert!(
        !body.contains("skip_wm_dispatch") && !body.contains("preverified_gadget_hit"),
        "handle must not take skip-WM / preverified forge parameters"
    );
    assert!(
        body.contains("handle_right_click"),
        "RMB release must call handle_right_click on world path"
    );
    assert!(
        body.contains("MouseInputOrigin::Physical"),
        "claim latches must require Physical origin"
    );
    // No inject-only stub branch that latches order without handle_right_click.
    assert!(
        !body.contains("Inject gameplay order"),
        "must not keep inject-only RMB evidence stub"
    );
}

#[test]
fn physical_winit_mouse_input_drives_retail_menus_with_provenance_gates() {
    // C++ Win32 mouse → WindowXlat → TheWindowManager.
    // Rust: WindowEvent::MouseInput → MouseInputOrigin::Physical →
    // handle_mouse_button_input → dispatch_os_mouse_to_window_manager.
    // Injected re-entry shares WM dispatch but cannot latch playable_claim.
    let input = include_str!("input.rs");
    let route = input
        .find("WindowEvent::MouseInput {")
        .expect("winit MouseInput match arm");
    let route_body = &input[route..route + 500];
    assert!(
        route_body.contains("MouseInputOrigin::Physical"),
        "OS WindowEvent::MouseInput must be Physical origin"
    );
    assert!(
        route_body.contains("handle_mouse_button_input"),
        "Physical OS mouse must share handle_mouse_button_input"
    );

    let start = input
        .find("fn handle_mouse_button_input")
        .expect("handle_mouse_button_input");
    let end = input[start..]
        .find("fn inject_winit_equivalent_cursor_at")
        .map(|i| start + i)
        .unwrap_or(start + 5000);
    let body = &input[start..end];
    assert!(
        body.contains("dispatch_os_mouse_to_window_manager"),
        "Physical mouse must hit live WindowManager, not a forged Used"
    );
    assert!(
        body.contains("matches!(origin, MouseInputOrigin::Physical)")
            && body.contains("note_menu_wnd_click")
            && body.contains("note_skirmish_path_gadget"),
        "menu-nav evidence latches only for Physical origin"
    );
    assert!(
        body.contains("MouseInputOrigin::Injected") || input.contains("MouseInputOrigin::Injected"),
        "injected control-file clicks must remain a distinct origin"
    );
}

#[test]
fn macos_device_event_button_is_physical_only_when_cursor_in_window() {
    let run = include_str!("run_loop.rs");
    let start = run
        .find("Event::DeviceEvent")
        .expect("DeviceEvent::Button residual");
    let body = &run[start..run.len().min(start + 900)];
    assert!(
        body.contains("macos_cursor_client_if_in_window")
            && body.contains("MouseInputOrigin::Physical")
            && body.contains("handle_mouse_button_input"),
        "inactive-NSApp HID must share Physical handle_mouse_button_input only in-window"
    );
    assert!(
        !body.contains("note_menu_wnd_click"),
        "DeviceEvent must not forge note_menu_wnd_click"
    );
}

#[test]
fn physical_gather_dropoff_latch_rejects_unvalidated_input() {
    let mut evidence = InteractivePlayabilityEvidence::default();

    // Runtime-host/injected/background paths must reach the setter with false
    // after failing provenance, carrier, player, amount, or match checks.
    evidence.note_physical_gather_resources(false);
    assert!(!evidence.gather_resources_complete());

    // The sticky evidence type accepts only the already-validated edge from
    // Main's tracked-carrier ReturningResources drain.
    evidence.note_physical_gather_resources(true);
    assert!(evidence.gather_resources_complete());
}

#[test]
fn physical_gather_proof_requires_physical_accepted_order_and_real_dropoff() {
    let input = include_str!("input.rs");
    assert!(
        input.contains("rmb_scroll_started_physically")
            && input.contains("self.handle_right_click(origin, physical_rmb_gesture)"),
        "Gather proof must carry actual press+release mouse provenance into the RMB command"
    );

    let mouse = super::ENGINE_SRC;
    assert!(
        mouse.contains("PhysicalGatherAttempt")
            && mouse.contains("MouseInputOrigin::Physical")
            && mouse.contains("take_accepted_gather_commands"),
        "only a physical RMB command may bind to executor-confirmed Gather carriers"
    );
    assert!(
        mouse.contains("event.carrier_ids")
            && mouse.contains("attempt.player_id != self.local_player_id_for_ui()")
            && mouse.contains("self.physical_gather_carrier_ids.extend"),
        "only the accepted local selected carrier subset may be tracked"
    );
    assert!(
        mouse.contains("dropoff.carried_amount > 0")
            && mouse.contains("dropoff.player_id == local_player_id")
            && mouse.contains("contains(&dropoff.carrier_id)")
            && mouse.contains("host_physical_gather_evidence_eligible"),
        "latch requires positive local tracked-carrier deposit in a visible offline match"
    );
    assert!(
        mouse.contains("runtime_host_window_visible()")
            && mouse.contains("GameMode::SinglePlayer")
            && mouse.contains("GameMode::Skirmish"),
        "headless/hidden/network paths must fail closed"
    );

    let executor = include_str!("../command_executor/leftover.rs");
    let gather_start = executor
        .find("fn execute_gather")
        .expect("Gather executor authority");
    let gather = &executor[gather_start..];
    assert!(
        gather.contains("unit.is_resource_collector()") && !gather.contains("unit.is_worker()"),
        "accepted Gather carriers must use semantic HARVESTER capability, not a builder/name heuristic"
    );

    let command_system = include_str!("../command_system/system_impl.rs");
    assert!(
        command_system.contains("u.is_alive && u.is_resource_collector")
            && command_system.contains("unit_is_resource_collector(unit_id)"),
        "presentation-frozen and boot command classification must agree on HARVESTER"
    );

    let template_parse = include_str!("../game_logic/world_objects/spawn_templates/definition.rs");
    assert!(
        template_parse.contains("let is_harvester = has_kind(\"harvester\")")
            && template_parse.contains("has_kind(\"harvestable\")")
            && !template_parse.contains("kind_of.contains(\"harvest\")"),
        "HARVESTER must not be conflated with HARVESTABLE or a Resource template"
    );

    let deposit = include_str!("../game_logic/world_objects/support_states/update.rs");
    let returning = deposit
        .find("AIState::ReturningResources")
        .expect("ReturningResources deposit branch");
    let credit = deposit[returning..]
        .find("player.credit_supplies(credited)")
        .map(|offset| returning + offset)
        .expect("concrete player credit");
    let event = deposit[returning..]
        .find("record_supply_dropoff_event")
        .map(|offset| returning + offset)
        .expect("typed dropoff event");
    assert!(
        credit < event,
        "dropoff evidence event must be emitted after the real player credit, not from passive income"
    );

    let runtime = include_str!("runtime.rs");
    assert!(
        runtime.contains("physical_gather_resources={}"),
        "runtime status must publish the exact physical_gather_resources field"
    );
}

#[test]
fn campaign_prelude_precedes_ui_map_work_and_logic_initializer() {
    // Loading + authored prelude happen on host_start_game_from_ui; map work
    // is parked for the next Loading tick (complete_parked_match_start) so
    // runtime-host can publish state=Loading. C++ startNewGame still enters
    // the load screen before loadMap.
    let start_game = include_str!("start_game.rs");
    let start = start_game
        .find("pub(super) fn host_start_game_from_ui")
        .expect("host start-game authority");
    let after_start = &start_game[start..];
    let park_end = after_start[1..]
        .find("\n    pub(super) fn ")
        .map(|offset| offset + 1)
        .unwrap_or(after_start.len());
    let park_body = &after_start[..park_end];
    let loading = park_body
        .find("transition_to_state(GameState::Loading)")
        .expect("Loading transition");
    let prelude = park_body
        .find("self.run_cpp_load_screen_prelude()")
        .expect("campaign prelude pump");
    assert!(
        loading < prelude && park_body.contains("pending_match_start"),
        "UI start must consume the prelude then park before session/map work"
    );

    let finish = start_game
        .find("pub(super) fn complete_parked_match_start")
        .expect("parked start finish");
    let after_finish = &start_game[finish..];
    let finish_end = after_finish[1..]
        .find("\n    pub(super) fn ")
        .map(|offset| offset + 1)
        .unwrap_or(after_finish.len());
    let finish_body = &after_finish[..finish_end];
    let clear_residuals = finish_body
        .find("self.host_clear_match_residuals()")
        .expect("session clear boundary");
    let map_load = finish_body
        .find("self.host_load_map_or_default(&map_name)")
        .expect("map-load authority");
    assert!(
        clear_residuals < map_load,
        "parked finish must consume the prelude-already-run start before session/map work"
    );

    let shell = include_str!("shell.rs");
    let shell_start = shell
        .find("pub(super) fn run_cpp_load_screen_prelude")
        .expect("safe Main prelude wrapper");
    let shell_after_start = &shell[shell_start..];
    let shell_end = shell_after_start[1..]
        .find("\n    pub(super) fn ")
        .map(|offset| offset + 1)
        .unwrap_or(shell_after_start.len());
    let shell_body = &shell_after_start[..shell_end];
    assert!(
        shell_body.contains("game_client::gui::load_screen::run_load_screen_prelude(kind)"),
        "Main must delegate to the shared render-pump-only prelude driver"
    );
    assert!(
        !shell_body.contains("service_windows_os")
            && !shell_body.contains("serviceWindowsOS")
            && !shell_body.contains("dispatch_event"),
        "the synchronous wrapper must not re-enter the platform event loop"
    );

    // GameLogic's independent initialization path calls the same hook before
    // `GameInitializer`, so campaign starts outside Main's host UI retain the
    // C++ ordering too.
    let client_hooks = include_str!("../../../GameEngine/GameClient/src/helpers.rs");
    let hooks_start = client_hooks
        .find("fn begin_load_screen(&self")
        .expect("GameClient load-screen hook");
    let hooks_after_start = &client_hooks[hooks_start..];
    let hooks_end = hooks_after_start
        .find("\n    fn update_load_screen(&self")
        .unwrap_or(hooks_after_start.len());
    let hooks_body = &hooks_after_start[..hooks_end];
    let init = hooks_body
        .find("init_load_screen(kind")
        .expect("load-screen init");
    let logic_prelude = hooks_body
        .find("run_load_screen_prelude(kind)")
        .expect("logic-owned prelude");
    assert!(
        init < logic_prelude,
        "logic hook must initialize the authored screen before it pumps it"
    );

    let game_logic = include_str!("../../../GameEngine/GameLogic/src/helpers/game_logic.rs");
    let start_new_game = game_logic
        .find("Self::begin_load_screen(Self::get_game_mode(), is_load_game)")
        .expect("logic load-screen begin");
    let initializer = game_logic[start_new_game..]
        .find("GameInitializer::initialize_game(params)")
        .map(|offset| start_new_game + offset)
        .expect("logic map initializer");
    assert!(
        start_new_game < initializer,
        "logic-owned load-screen hook must run before the map initializer"
    );
}

#[test]
fn configured_skirmish_start_restamps_mode_after_map_clear_before_physical_evidence() {
    // The full CnCGameEngine constructor owns a live WGPU window, so this
    // regression locks the production authority sequence itself.  It must
    // remain a real configured Skirmish start, not a runtime-host simulation.
    let start_game = include_str!("start_game.rs");
    let start = start_game
        .find("pub(super) fn complete_parked_match_start")
        .expect("parked start finish authority");
    let after_start = &start_game[start..];
    let end = after_start[1..]
        .find("\n    pub(super) fn ")
        .map(|offset| offset + 1)
        .unwrap_or(after_start.len());
    let body = &after_start[..end];

    let skirmish_config = body
        .find("host_apply_skirmish_config_authority(config)")
        .expect("configured Skirmish authority branch");
    let map_load = body
        .find("let Some(loaded_map_name) = self.host_load_map_or_default(&map_name)")
        .expect("successful map-load authority guard");
    let successful_mode_stamp = body[map_load..]
        .find("self.host_match_game_mode = Some(mode);")
        .map(|offset| map_load + offset)
        .expect("mode re-stamped after successful map load");
    let ingame = body[successful_mode_stamp..]
        .find("transition_to_state(GameState::InGame)")
        .map(|offset| successful_mode_stamp + offset)
        .expect("InGame transition");

    assert!(
        skirmish_config < map_load
            && map_load < successful_mode_stamp
            && successful_mode_stamp < ingame,
        "a configured Skirmish must retain GameMode::Skirmish after the map-clear boundary and before InGame"
    );
    assert!(
        body[map_load..successful_mode_stamp].contains("let Some(loaded_map_name)")
            && body[map_load..successful_mode_stamp].contains("return_to_main_menu_after_match"),
        "only a successful map load may re-stamp the selected mode"
    );

    let residuals = include_str!("host_authority.rs");
    let clear_start = residuals
        .find("pub(super) fn host_clear_match_residuals")
        .expect("match residual clear authority");
    let clear_after_start = &residuals[clear_start..];
    let clear_end = clear_after_start[1..]
        .find("\n    pub(super) fn ")
        .map(|offset| offset + 1)
        .unwrap_or(clear_after_start.len());
    assert!(
        clear_after_start[..clear_end].contains("self.host_match_game_mode = None;"),
        "explicit match-reset paths must clear match mode rather than preserve stale eligibility; failed staged loads retain their active world"
    );

    for (source, gate) in [
        (
            include_str!("control_bar_bridge.rs"),
            "host_control_bar_evidence_eligible",
        ),
        (
            super::ENGINE_SRC,
            "host_physical_gather_evidence_eligible",
        ),
        (
            include_str!("runtime_host/gameplay.rs"),
            "host_popup_save_load_evidence_eligible",
        ),
    ] {
        assert!(
            source.contains(gate)
                && source.contains("self.host_match_game_mode")
                && source.contains("GameMode::Skirmish"),
            "{gate} must accept the authoritative Skirmish mode after a physical input reaches it"
        );
    }
}
