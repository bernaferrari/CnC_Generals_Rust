//! Synchronous native UI adapter on independent authored window trees.
use super::NativeScriptUi;
use crate::presentation_frame::camera::SessionCamera;
use game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks;
use game_client::gui::control_bar::ControlBar;
use game_client::gui::window_manager::WindowManager;

#[test]
fn script_letterbox_parent_delivery_is_synchronous_repeated_and_receiving_only() {
    let mut snow = game_client::snow::SnowManager::new();
    let mut peer_snow = game_client::snow::SnowManager::new();
    let mut receiving = WindowManager::new();
    let mut peer = WindowManager::new();
    let parent = receiving.create_window(None, 23, 117, 801, 100).unwrap();
    let foreign = peer.create_window(None, 41, 211, 1001, 100).unwrap();
    let (mut bar, mut peer_bar) = (ControlBar::new(), ControlBar::new());
    let (mut callbacks, mut peer_callbacks) =
        (ControlBarCallbacks::new(), ControlBarCallbacks::new());
    let (mut camera, mut peer_camera) = (SessionCamera::default(), SessionCamera::default());
    camera.resize_display(801, 603).unwrap();
    camera.set_view_height_px(482).unwrap();
    peer_camera.resize_display(1001, 751).unwrap();
    peer_camera.set_view_height_px(600).unwrap();
    NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks).letterbox_with_parent(
        &mut camera,
        true,
        false,
        Some(parent.clone()),
        game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
    );
    assert!(parent.borrow().is_hidden());
    assert_eq!(camera.viewport.size(), (801, 603));
    assert!(!foreign.borrow().is_hidden());
    assert_eq!(peer_camera.viewport.size(), (1001, 600));
    // An already-disabled display still executes an authored ShowControlBar.
    for _ in 0..2 {
        parent.borrow_mut().hide(true).unwrap();
        NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks).letterbox_with_parent(
            &mut camera,
            false,
            false,
            Some(parent.clone()),
            game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
        );
        assert!(!parent.borrow().is_hidden());
        assert_eq!(camera.viewport.size(), (801, 482));
    }
    NativeScriptUi::new(&mut peer_snow, &mut peer_bar, &mut peer_callbacks).letterbox_with_parent(
        &mut peer_camera,
        true,
        false,
        Some(foreign.clone()),
        game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
    );
    assert!(foreign.borrow().is_hidden());
    assert!(!parent.borrow().is_hidden());
    assert_eq!(camera.viewport.size(), (801, 482));
    assert_eq!(peer_camera.viewport.size(), (1001, 751));
    NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks).letterbox_with_parent(
        &mut camera,
        true,
        false,
        None,
        game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
    );
    assert_eq!(
        camera.viewport.size(),
        (801, 482),
        "missing parent cannot resize a foreign view"
    );
}

#[test]
fn native_weather_adapter_only_changes_its_borrowed_snow() {
    use crate::game_logic::ScriptUiEffects;
    let mut snow = game_client::snow::SnowManager::new();
    let peer = game_client::snow::SnowManager::new();
    let mut bar = ControlBar::new();
    let mut callbacks = ControlBarCallbacks::new();
    for visible in [false, true, false] {
        NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks).weather_visible(visible);
        assert_eq!(snow.is_visible(), visible);
        assert!(peer.is_visible());
    }
}
