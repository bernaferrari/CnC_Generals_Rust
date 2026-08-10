//! Shell screen ownership + ControlBar.wnd residual honesty.

#![allow(unused_imports)]

use super::imports::*;

pub(super) struct ShellUiResiduals {
    pub screen_skirmish_ok: bool,
    pub control_bar_layout_ok: bool,
    pub control_bar_path_resolved: bool,
    pub control_bar_wnd_validated: bool,
    pub control_bar_window_loaded: bool,
    pub control_bar_window_count: usize,
    pub control_bar_wave76_residual_ok: bool,
    pub layout_report: String,
}

pub(super) fn evaluate_shell_ui() -> ShellUiResiduals {
    // Shell → InGame residual: production StartGame transitions Skirmish→Loading→GameHUD
    // and ensure_gameplay_layouts (ControlBar.wnd) on InGame enter.
    let mut ui_mgr = UIManager::new(1024, 768);
    ui_mgr.transition_to_screen(Screen::Skirmish);
    let at_skirmish = ui_mgr.current_screen() == Some(Screen::Skirmish)
        && Screen::Skirmish.is_shell_owned_pregame();
    ui_mgr.transition_to_screen(Screen::Loading);
    let at_loading = ui_mgr.current_screen() == Some(Screen::Loading)
        && !Screen::Loading.is_shell_owned_pregame();
    // Attempt headless WindowManager load when game_client is enabled (ShowControlBar
    // residual). AssetsUnavailable remains honest when WindowZH is not checked out.
    // This is **not** windowed W3D retail — only layout script → window tree.
    #[cfg(feature = "game_client")]
    let layout_honesty = control_bar_layout_honesty(true);
    #[cfg(not(feature = "game_client"))]
    let layout_honesty = control_bar_layout_honesty(false);
    let layout_status = layout_honesty.status.clone();
    let layout_report = format_control_bar_honesty(&layout_honesty);
    let control_bar_path_resolved = layout_honesty.path_resolved;
    let control_bar_wnd_validated = layout_honesty.wnd_validated;
    let control_bar_window_loaded = layout_honesty.window_loaded;
    let control_bar_window_count = layout_honesty.window_count;
    let control_bar_layout_ok = match &layout_status {
        GameplayLayoutStatus::Ready { path, loaded } => {
            // Ready after structural validate. Prefer WindowManager load when assets
            // present (`loaded=true`); validated-only (`loaded=false`) is still ok.
            path.contains("ControlBar")
                && control_bar_wnd_validated
                && (*loaded == control_bar_window_loaded)
                && (!*loaded || control_bar_window_count > 0)
        }
        // Honest residual when WindowZH assets are not checked out.
        GameplayLayoutStatus::AssetsUnavailable { searched } => {
            !searched.is_empty() && layout_honesty.assets_unavailable && !control_bar_window_loaded
        }
        GameplayLayoutStatus::LoadFailed { .. } => false,
    };
    let control_bar_wave76_residual_ok = honesty_control_bar_residual_pack_wave76_ok(
        control_bar_window_loaded,
        control_bar_window_count,
    );
    ui_mgr.transition_to_screen(Screen::GameHUD);
    let at_ingame = ui_mgr.current_screen() == Some(Screen::GameHUD)
        && !Screen::GameHUD.is_shell_owned_pregame();
    let screen_skirmish_ok = at_skirmish
        && at_loading
        && at_ingame
        && Screen::MainMenu.is_shell_owned_pregame()
        && Screen::startup_entry_screen(true) == Screen::MainMenu;

    ShellUiResiduals {
        screen_skirmish_ok,
        control_bar_layout_ok,
        control_bar_path_resolved,
        control_bar_wnd_validated,
        control_bar_window_loaded,
        control_bar_window_count,
        control_bar_wave76_residual_ok,
        layout_report,
    }
}
