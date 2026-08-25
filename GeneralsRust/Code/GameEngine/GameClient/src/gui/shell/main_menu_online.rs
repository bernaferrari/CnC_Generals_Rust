//! C++ `MainMenuUtils.cpp` `startOnline` leftover after patch countdown.

use super::MainMenuState;
use crate::gui::shell::queue_shell_push;
use crate::shell_hooks::{SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED, THE_SHELL_HOOK_NAMES};
use game_network::download_manager::download_manager;
use game_network::gamespy::peer_defs::set_up_gamespy;
use gamelogic::helpers::TheScriptEngine;

pub enum OnlineHandoff {
    EnteredLogin,
    NeedPatchDownload,
    Cancelled,
}

fn has_queued_downloads() -> bool {
    download_manager()
        .lock()
        .ok()
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|manager| manager.is_active() || manager.is_file_queued_for_download())
        })
        .unwrap_or(false)
}

/// C++ `startOnline` after HTTP/DNS/patch countdown:
/// teardown cancel window, optional patch prompt, shell hook, SetUpGameSpy,
/// `TheShell->push("Menus/GameSpyLoginProfile.wnd")`.
pub fn start_online(state: &mut MainMenuState) -> OnlineHandoff {
    if !state.checking_for_patch_before_gamespy {
        return OnlineHandoff::Cancelled;
    }

    state.checking_for_patch_before_gamespy = false;
    state.checks_left_before_online = 0;
    state.online_cancel_window_open = false;

    if state.cant_connect_before_online {
        state.cant_connect_before_online = false;
        state.button_pushed = false;
        return OnlineHandoff::Cancelled;
    }
    state.cant_connect_before_online = false;
    if has_queued_downloads() {
        return OnlineHandoff::NeedPatchDownload;
    }

    TheScriptEngine::signal_ui_interact(
        THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED as usize],
    );
    set_up_gamespy("", "");
    queue_shell_push("Menus/GameSpyLoginProfile.wnd", false);
    log::info!("Patch check completed - entering GameSpyLoginProfile.wnd");
    OnlineHandoff::EnteredLogin
}
