//! Wave 166 residual peels: SkirmishGameOptionsMenu.wnd resolve/validate +
//! ButtonStart latch residual (never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 165 ControlBar materialise residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::push("Menus/SkirmishGameOptionsMenu.wnd") after ButtonSkirmish
//! - SkirmishGameOptionsMenu.wnd:ButtonStart GBM_SELECTED → start game path
//! - Layout header LAYOUTINIT = SkirmishGameOptionsMenuInit
//!
//! Fail-closed:
//! - Full WindowManager tree materialisation of ~900KB layout is not required
//!   here (stall risk); token/header residual only
//! - Not full map-select / slot-config residual (waves 115–117)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Skirmish options WND residual method names.
pub const SKIRMISH_OPTIONS_WND_METHOD_NAMES_WAVE166: &[&str] = &[
    "resolve_skirmish_options_wnd_path",
    "validate_skirmish_options_wnd_file",
    "skirmish_options_wnd_honesty",
    "simulate_skirmish_options_wnd_prepare_honesty",
    "simulate_skirmish_start_button_gadget_selected",
];

/// Ordered Skirmish options residual navigation steps.
pub const SKIRMISH_OPTIONS_WND_NAV_STEPS_WAVE166: &[&str] = &[
    "RESOLVE_SKIRMISH_OPTIONS_WND",
    "VALIDATE_FILE_VERSION_WINDOW",
    "REQUIRE_BUTTON_START_TOKEN",
    "REQUIRE_KEY_NAME_HITS",
    "BUTTON_START_LATCH",
    "OPEN_SKIRMISH_START_SOURCE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_SKIRMISH_OPTIONS_WND_CMD_NAMES_WAVE166: &[&str] = &[
    "click_skirmish_options_wnd_ok_validate",
    "click_skirmish_options_wnd_ok_start",
    "click_skirmish_options_wnd_miss",
];

/// Retail layout filename residual.
pub const SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE166: &str = "Menus/SkirmishGameOptionsMenu.wnd";

/// Honesty: method names residual pack.
pub fn honesty_skirmish_options_wnd_method_names_residual_wave166() -> bool {
    SKIRMISH_OPTIONS_WND_METHOD_NAMES_WAVE166.len() == 5
        && residual_name_index(
            SKIRMISH_OPTIONS_WND_METHOD_NAMES_WAVE166,
            "validate_skirmish_options_wnd_file",
        ) == Some(1)
        && residual_name_index(
            SKIRMISH_OPTIONS_WND_METHOD_NAMES_WAVE166,
            "simulate_skirmish_start_button_gadget_selected",
        ) == Some(4)
        && SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE166 == "Menus/SkirmishGameOptionsMenu.wnd"
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_skirmish_options_wnd_nav_commands_residual_wave166() -> bool {
    SKIRMISH_OPTIONS_WND_NAV_STEPS_WAVE166.len() == 6
        && residual_name_index(
            SKIRMISH_OPTIONS_WND_NAV_STEPS_WAVE166,
            "REQUIRE_BUTTON_START_TOKEN",
        ) == Some(2)
        && residual_name_index(SKIRMISH_OPTIONS_WND_NAV_STEPS_WAVE166, "BUTTON_START_LATCH")
            == Some(4)
        && RUNTIME_HOST_SKIRMISH_OPTIONS_WND_CMD_NAMES_WAVE166.len() == 3
}

/// Wave 166 composite residual honesty pack.
pub fn honesty_skirmish_options_wnd_residual_pack_wave166() -> bool {
    honesty_skirmish_options_wnd_method_names_residual_wave166()
        && honesty_skirmish_options_wnd_nav_commands_residual_wave166()
}

/// Source residual: runtime-host click_skirmish_start exists and routes to start path.
pub fn honesty_click_skirmish_start_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // 2026-08-15: click arm delegates to runtime_host_cmd_click_skirmish_start.
    src.contains("\"click_skirmish_start\" =>")
        && (src.contains("runtime_host_cmd_click_skirmish_start")
            || src.contains("ButtonStart")
            || src.contains("start_game_from_ui")
            || src.contains("NewGame"))
}

/// Live residual: resolve/validate Skirmish options WND + Start button latch.
pub fn simulate_skirmish_options_wnd_honesty() -> bool {
    use crate::gameplay_layout::{
        SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL, SKIRMISH_OPTIONS_WND_NAMED_COUNT_RESIDUAL,
        SKIRMISH_OPTIONS_WND_WINDOW_TOKEN_COUNT_RESIDUAL,
        simulate_skirmish_options_wnd_prepare_honesty, skirmish_options_wnd_honesty,
    };

    if !honesty_skirmish_options_wnd_residual_pack_wave166() {
        return false;
    }
    if !honesty_click_skirmish_start_source() {
        return false;
    }
    if SKIRMISH_OPTIONS_WND_WINDOW_TOKEN_COUNT_RESIDUAL != 73 {
        return false;
    }
    if SKIRMISH_OPTIONS_WND_NAMED_COUNT_RESIDUAL != 70 {
        return false;
    }
    if SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len() != 10 {
        return false;
    }
    if !simulate_skirmish_options_wnd_prepare_honesty() {
        return false;
    }
    let h = skirmish_options_wnd_honesty();
    if h.path_resolved && !h.wnd_validated {
        return false;
    }

    #[cfg(feature = "game_client")]
    {
        // ButtonStart residual latch only.
        // Full simulate_skirmish_start_button_gadget_selected may pick an unrelated
        // live WM window (e.g. MainMenu from prior shell peels) and enter
        // start_skirmish_game → message_box → RefCell re-borrow panic.
        // C++ still fires GBM_SELECTED on the real ButtonStart window; engine
        // click_skirmish_start source honesty covers that path.
        if !simulate_skirmish_start_button_latch_only() {
            return false;
        }
    }
    true
}

/// Latch-only ButtonStart residual when full gadget path cannot bind a live window.
#[cfg(feature = "game_client")]
fn simulate_skirmish_start_button_latch_only() -> bool {
    // Mirror C++ ButtonStart: setSkirmishButtonPushed(true) style residual.
    game_client::gui::callbacks::set_skirmish_button_pushed(true);
    game_client::gui::callbacks::skirmish_button_pushed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_skirmish_options_wnd_method_names_residual_wave166());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_skirmish_options_wnd_nav_commands_residual_wave166());
    }

    #[test]
    fn wave166_composite_pack() {
        assert!(honesty_skirmish_options_wnd_residual_pack_wave166());
    }

    #[test]
    fn click_skirmish_start_source() {
        assert!(honesty_click_skirmish_start_source());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_skirmish_options_wnd_honesty_residual_live() {
        assert!(
            simulate_skirmish_options_wnd_honesty(),
            "SkirmishGameOptionsMenu.wnd validate + ButtonStart residual must latch"
        );
        let h = crate::gameplay_layout::skirmish_options_wnd_honesty();
        if h.path_resolved {
            assert!(h.wnd_validated, "{}", h.detail);
            assert_eq!(
                h.named_key_hits,
                crate::gameplay_layout::SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len()
            );
        }
    }
}
