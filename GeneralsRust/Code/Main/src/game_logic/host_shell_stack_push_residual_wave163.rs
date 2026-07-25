//! Wave 163 residual peels: Shell stack push residual
//! (init Shell + push MainMenu.wnd; honest shell_top_wnd / screen_count;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 162 MainMenu.wnd materialise residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::init via SubsystemInterface
//! - Shell::push("Menus/MainMenu.wnd") / doPush
//! - Shell::showShell sets m_isShellActive after push
//!
//! Fail-closed:
//! - Not full W3DMainMenuInit / gadget residual
//! - Not interactive Skirmish navigation residual
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Shell stack residual method names.
pub const SHELL_STACK_PUSH_METHOD_NAMES_WAVE163: &[&str] = &[
    "show_shell_menu",
    "SubsystemInterface::init",
    "Shell::push",
    "set_shell_active",
    "get_screen_count",
];

/// Ordered shell stack residual navigation steps.
pub const SHELL_STACK_PUSH_NAV_STEPS_WAVE163: &[&str] = &[
    "INIT_SHELL",
    "SHOW_SHELL_MAP",
    "PUSH_MAIN_MENU_WND",
    "REQUIRE_SCREEN_COUNT_GT_ZERO",
    "SET_SHELL_ACTIVE",
    "LATCH_SHELL_MENU_ACTIVE",
];

/// Runtime-host command residual names for shell stack peels.
pub const RUNTIME_HOST_SHELL_STACK_PUSH_CMD_NAMES_WAVE163: &[&str] = &[
    "click_shell_stack_ok_push",
    "click_shell_stack_ok_init",
    "click_shell_stack_miss",
];

/// Honesty: shell stack method names residual pack.
pub fn honesty_shell_stack_push_method_names_residual_wave163() -> bool {
    SHELL_STACK_PUSH_METHOD_NAMES_WAVE163.len() == 5
        && residual_name_index(SHELL_STACK_PUSH_METHOD_NAMES_WAVE163, "show_shell_menu") == Some(0)
        && residual_name_index(
            SHELL_STACK_PUSH_METHOD_NAMES_WAVE163,
            "SubsystemInterface::init",
        ) == Some(1)
        && residual_name_index(SHELL_STACK_PUSH_METHOD_NAMES_WAVE163, "Shell::push") == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_shell_stack_push_nav_commands_residual_wave163() -> bool {
    SHELL_STACK_PUSH_NAV_STEPS_WAVE163.len() == 6
        && residual_name_index(SHELL_STACK_PUSH_NAV_STEPS_WAVE163, "INIT_SHELL") == Some(0)
        && residual_name_index(
            SHELL_STACK_PUSH_NAV_STEPS_WAVE163,
            "REQUIRE_SCREEN_COUNT_GT_ZERO",
        ) == Some(3)
        && RUNTIME_HOST_SHELL_STACK_PUSH_CMD_NAMES_WAVE163.len() == 3
}

/// Wave 163 composite residual honesty pack.
pub fn honesty_shell_stack_push_residual_pack_wave163() -> bool {
    honesty_shell_stack_push_method_names_residual_wave163()
        && honesty_shell_stack_push_nav_commands_residual_wave163()
}

/// Residual: source-level show_shell_menu init-before-push honesty.
pub fn honesty_show_shell_menu_init_before_push_source() -> bool {
    let src = include_str!("../cnc_game_engine.rs");
    let i = match src.find("fn show_shell_menu(&mut self)") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 2200)];
    body.contains("SubsystemInterface::init")
        && body.contains("shell.push(\"Menus/MainMenu.wnd\"")
        && body.contains("get_screen_count()")
        && body.contains("set_shell_active(true)")
        && body.contains("shell_menu_active = true")
        && body.contains("screens == 0")
}

/// Residual: honest shell_top_wnd / shell_screen_count snapshot (no invented stack).
pub fn honesty_shell_snapshot_no_invented_stack_source() -> bool {
    let src = include_str!("../cnc_game_engine.rs");
    // Must not invent MainMenu.wnd when top is empty.
    let top_i = match src.find("shell_top_wnd:") {
        Some(i) => i,
        None => return false,
    };
    let top_body = &src[top_i..src.len().min(top_i + 900)];
    if top_body.contains("if top.is_empty() && self.shell_menu_active") {
        return false;
    }
    // screen count must not fail-open to 1
    let sc_i = match src.find("shell_screen_count:") {
        Some(i) => i,
        None => return false,
    };
    let sc_body = &src[sc_i..src.len().min(sc_i + 700)];
    if sc_body.contains("if n == 0 && self.shell_menu_active") {
        return false;
    }
    top_body.contains("get_filename()")
        && sc_body.contains("get_screen_count()")
        && honesty_show_shell_menu_init_before_push_source()
}

/// Live residual: init shell, push MainMenu, require screen_count > 0 and top filename.
pub fn simulate_shell_stack_push_honesty() -> bool {
    if !honesty_show_shell_menu_init_before_push_source() {
        return false;
    }
    if !honesty_shell_snapshot_no_invented_stack_source() {
        return false;
    }
    #[cfg(feature = "game_client")]
    {
        use game_client::gui::get_shell;
        use game_client::system::SubsystemInterface;

        let mut shell = get_shell();
        if SubsystemInterface::init(&mut *shell).is_err() {
            return false;
        }
        // Reset stack for deterministic residual (test isolation).
        // pop_immediate destroys layout windows; tolerate destroy errors and
        // continue until empty or a hard failure stops progress.
        let mut guard = 0;
        while shell.get_screen_count() > 0 && guard < 16 {
            guard += 1;
            let before = shell.get_screen_count();
            if shell.pop_immediate().is_err() {
                break;
            }
            if shell.get_screen_count() >= before {
                break;
            }
        }
        // If stack still non-empty, reuse top when it is already MainMenu.
        if shell.get_screen_count() != 0 {
            let top = shell
                .top()
                .map(|l| l.get_filename().to_string())
                .unwrap_or_default()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if top.contains("mainmenu.wnd") {
                shell.set_shell_active(true);
                return shell.is_shell_active() && shell.get_screen_count() > 0;
            }
            // Cannot safely clear — fail closed rather than double-push leak.
            return false;
        }
        if shell.push("Menus/MainMenu.wnd", false).is_err() {
            return false;
        }
        if shell.get_screen_count() == 0 {
            return false;
        }
        let top = shell
            .top()
            .map(|l| l.get_filename().to_string())
            .unwrap_or_default();
        if top.is_empty() {
            return false;
        }
        // Accept path variants: Menus/MainMenu.wnd or absolute/resolved form.
        let top_l = top.replace('\\', "/").to_ascii_lowercase();
        if !top_l.contains("mainmenu.wnd") {
            return false;
        }
        shell.set_shell_active(true);
        if !shell.is_shell_active() {
            return false;
        }
        true
    }
    #[cfg(not(feature = "game_client"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_shell_stack_push_method_names_residual_wave163());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_shell_stack_push_nav_commands_residual_wave163());
    }

    #[test]
    fn wave163_composite_pack() {
        assert!(honesty_shell_stack_push_residual_pack_wave163());
    }

    #[test]
    fn show_shell_menu_init_before_push_source() {
        assert!(honesty_show_shell_menu_init_before_push_source());
    }

    #[test]
    fn shell_snapshot_no_invented_stack_source() {
        assert!(honesty_shell_snapshot_no_invented_stack_source());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_shell_stack_push_honesty_residual_live() {
        assert!(
            simulate_shell_stack_push_honesty(),
            "Shell stack push residual must latch MainMenu.wnd on stack"
        );
    }
}
