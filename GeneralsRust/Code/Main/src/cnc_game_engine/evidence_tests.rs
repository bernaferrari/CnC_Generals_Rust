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
        src.contains("ui_object_is_dozer")
            && src.contains("local live dozer or worker"),
        "a stale generic selection must not count as a DozerConstruct arm"
    );
    assert!(
        src.contains("self.runtime_host_window_visible()"),
        "physical Control Bar evidence requires a real visible window, not merely a non-headless host"
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
