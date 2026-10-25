// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
/// Residual: last ShellMap action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualShellMapAction {
    None = 0,
    Show = 1,
    Hide = 2,
    Toggle = 3,
}

fn residual_shell_map_action_store(action: ResidualShellMapAction, shell_map_on: bool) {
    if let Some(owner) = crate::gui::window_manager::current_window_manager_owner() {
        owner.set_residual_shell_map(action, shell_map_on);
    }
}

/// Residual: last ShellMap residual action for the receiving client.
pub fn residual_shell_map_last_action() -> ResidualShellMapAction {
    crate::gui::window_manager::current_window_manager_owner()
        .map(|owner| owner.residual_shell_map_action())
        .unwrap_or(ResidualShellMapAction::None)
}

/// Residual: shell map enabled latch for the receiving client.
pub fn residual_shell_map_is_on() -> bool {
    crate::gui::window_manager::current_window_manager_owner()
        .is_some_and(|owner| owner.residual_shell_map_on())
}

/// Residual: enable shell map flag without ClearGameData / map load.
pub fn simulate_shell_map_show() -> bool {
    let Some(owner) = crate::gui::window_manager::current_window_manager_owner() else {
        return false;
    };
    owner.with_scope(|| {
        let applied = try_with_shell_mut(|shell| {
            shell.shell_map_on = true;
        })
        .is_some();
        residual_shell_map_action_store(ResidualShellMapAction::Show, true);
        // Succeed even if shell borrow nested; residual latch is authoritative.
        let _ = applied;
        residual_shell_map_is_on()
    })
}

/// Residual: disable shell map flag without teardown side effects.
pub fn simulate_shell_map_hide() -> bool {
    let Some(owner) = crate::gui::window_manager::current_window_manager_owner() else {
        return false;
    };
    owner.with_scope(|| {
        let applied = try_with_shell_mut(|shell| {
            shell.shell_map_on = false;
        })
        .is_some();
        residual_shell_map_action_store(ResidualShellMapAction::Hide, false);
        let _ = applied;
        !residual_shell_map_is_on()
    })
}

/// Residual: toggle shell map residual latch.
pub fn simulate_shell_map_toggle() -> bool {
    let next = !residual_shell_map_is_on();
    if next {
        simulate_shell_map_show()
    } else {
        simulate_shell_map_hide()
    };
    residual_shell_map_action_store(ResidualShellMapAction::Toggle, next);
    residual_shell_map_is_on() == next
}

/// Residual: show then hide composite (menu transition honesty).
pub fn simulate_shell_map_prepare_cycle() -> bool {
    if !simulate_shell_map_show() {
        return false;
    }
    simulate_shell_map_hide()
}
