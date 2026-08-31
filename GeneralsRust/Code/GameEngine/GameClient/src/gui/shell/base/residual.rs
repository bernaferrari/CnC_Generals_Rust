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

static RESIDUAL_SHELL_MAP_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SHELL_MAP_ON: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_shell_map_action_store(action: ResidualShellMapAction) {
    RESIDUAL_SHELL_MAP_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last ShellMap residual action.
pub fn residual_shell_map_last_action() -> ResidualShellMapAction {
    match RESIDUAL_SHELL_MAP_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualShellMapAction::Show,
        2 => ResidualShellMapAction::Hide,
        3 => ResidualShellMapAction::Toggle,
        _ => ResidualShellMapAction::None,
    }
}

/// Residual: shell map enabled latch (independent of live 3D shell map load).
pub fn residual_shell_map_is_on() -> bool {
    RESIDUAL_SHELL_MAP_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: enable shell map flag without ClearGameData / map load.
pub fn simulate_shell_map_show() -> bool {
    let applied = try_with_shell_mut(|shell| {
        shell.shell_map_on = true;
    })
    .is_some();
    RESIDUAL_SHELL_MAP_ON.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_shell_map_action_store(ResidualShellMapAction::Show);
    // Succeed even if shell borrow nested; residual latch is authoritative.
    let _ = applied;
    residual_shell_map_is_on()
}

/// Residual: disable shell map flag without teardown side effects.
pub fn simulate_shell_map_hide() -> bool {
    let applied = try_with_shell_mut(|shell| {
        shell.shell_map_on = false;
    })
    .is_some();
    RESIDUAL_SHELL_MAP_ON.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_shell_map_action_store(ResidualShellMapAction::Hide);
    let _ = applied;
    !residual_shell_map_is_on()
}

/// Residual: toggle shell map residual latch.
pub fn simulate_shell_map_toggle() -> bool {
    let next = !residual_shell_map_is_on();
    if next {
        simulate_shell_map_show()
    } else {
        simulate_shell_map_hide()
    };
    residual_shell_map_action_store(ResidualShellMapAction::Toggle);
    residual_shell_map_is_on() == next
}

/// Residual: show then hide composite (menu transition honesty).
pub fn simulate_shell_map_prepare_cycle() -> bool {
    if !simulate_shell_map_show() {
        return false;
    }
    simulate_shell_map_hide()
}
