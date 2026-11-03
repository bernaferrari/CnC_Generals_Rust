//! Synchronous native UI adapter on independent authored window trees.
use super::NativeScriptUi;
use game_client::gui::control_bar::ControlBar;
use game_client::gui::game_window::WindowEventEffects;
use game_client::gui::window_manager::WindowManager;

#[test]
fn native_weather_adapter_only_changes_its_borrowed_snow() {
    let mut snow = game_client::snow::SnowManager::new();
    let peer = game_client::snow::SnowManager::new();
    let mut bar = ControlBar::new();
    for visible in [false, true, false] {
        NativeScriptUi::new(
            &mut snow,
            &mut bar,
            game_client::gui::window_manager::WindowManagerOwner::new(),
        )
        .weather_visible(visible);
        assert_eq!(snow.is_visible(), visible);
        assert!(peer.is_visible());
    }
}

#[test]
fn create_window_requires_effects_and_stays_local_to_manager() {
    WindowEventEffects::with_standalone(|effects| {
        let mut receiving = WindowManager::new();
        let mut peer = WindowManager::new();
        let parent = receiving
            .create_window(None, 23, 117, 801, 100, effects)
            .unwrap();
        let foreign = peer
            .create_window(None, 41, 211, 1001, 100, effects)
            .unwrap();
        assert!(!parent.borrow().is_hidden());
        assert!(!foreign.borrow().is_hidden());
    });
}
