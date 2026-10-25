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
