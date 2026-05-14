#![cfg(feature = "internal")]

use generals_main::cnc_game_engine::{parity_test_support::StateMachineParityHarness, GameState};
use generals_main::ui::Screen;

#[test]
fn startup_loading_reaches_menu_state() {
    let mut harness = StateMachineParityHarness::default();
    harness.set_loading_state();
    harness.complete_startup_loading_to_menu();

    assert_eq!(harness.current_state(), GameState::Menu);
    assert_eq!(harness.ui_screen(), Some(Screen::MainMenu));
    assert!(!harness.game_paused());
    assert!(!harness.game_logic_paused());
    assert_eq!(harness.pending_state(), None);
}

#[test]
fn new_game_success_transitions_into_playing() {
    let mut harness = StateMachineParityHarness::default();
    harness.set_dirty_play_state();
    harness.complete_new_game_success();

    assert_eq!(harness.current_state(), GameState::InGame);
    assert_eq!(harness.ui_screen(), Some(Screen::GameHUD));
    assert!(!harness.game_paused());
    assert!(!harness.game_logic_paused());
    assert!(!harness.match_over());
    assert!(!harness.victory_summary_present());
    assert!(harness.selected_objects().is_empty());
}

#[test]
fn load_game_success_transitions_into_playing() {
    let mut harness = StateMachineParityHarness::default();
    harness.set_loading_state();
    harness.set_dirty_play_state();
    harness.complete_load_game_success();

    assert_eq!(harness.current_state(), GameState::InGame);
    assert_eq!(harness.ui_screen(), Some(Screen::GameHUD));
    assert!(!harness.game_paused());
    assert!(!harness.game_logic_paused());
    assert!(!harness.match_over());
    assert!(!harness.victory_summary_present());
    assert!(harness.selected_objects().is_empty());
}

#[test]
fn exit_to_menu_resets_match_state_and_returns_to_menu() {
    let mut harness = StateMachineParityHarness::default();
    harness.set_dirty_play_state();
    harness.return_to_main_menu_after_match();

    assert_eq!(harness.current_state(), GameState::Menu);
    assert_eq!(harness.ui_screen(), Some(Screen::MainMenu));
    assert!(!harness.game_paused());
    assert!(!harness.game_logic_paused());
    assert!(!harness.match_over());
    assert!(!harness.victory_summary_present());
    assert!(harness.selected_objects().is_empty());
    assert_eq!(harness.pending_state(), None);
}

#[test]
fn quit_request_dedupes_and_avoids_repeated_exit_requests() {
    let mut harness = StateMachineParityHarness::default();

    assert!(harness.request_quit());
    assert_eq!(harness.pending_state(), Some(GameState::Exiting));
    assert_eq!(harness.quit_requests_emitted(), 1);

    assert!(!harness.request_quit());
    assert_eq!(harness.pending_state(), Some(GameState::Exiting));
    assert_eq!(harness.quit_requests_emitted(), 1);

    harness.apply_pending_state_change();
    assert_eq!(harness.current_state(), GameState::Exiting);
    assert!(!harness.request_quit());
    assert_eq!(harness.quit_requests_emitted(), 1);
}
