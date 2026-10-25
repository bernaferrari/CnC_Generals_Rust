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

/// Runtime-host probes receive the actual native owner, never an active slot.
pub fn residual_shell_map_last_action(
    owner: &crate::gui::window_manager::WindowManagerOwner,
) -> ResidualShellMapAction {
    owner.residual_shell_map_action()
}
pub fn residual_shell_map_is_on(owner: &crate::gui::window_manager::WindowManagerOwner) -> bool {
    owner
        .with_shell_ref(|shell| shell.is_shell_map_on())
        .expect("receiving Shell read")
}
pub fn simulate_shell_map_show(owner: &crate::gui::window_manager::WindowManagerOwner) -> bool {
    owner
        .with_shell_mut(|shell| shell.shell_map_on = true)
        .expect("receiving Shell mutation");
    owner.set_residual_shell_map(ResidualShellMapAction::Show, true);
    residual_shell_map_is_on(owner)
}
pub fn simulate_shell_map_hide(owner: &crate::gui::window_manager::WindowManagerOwner) -> bool {
    owner
        .with_shell_mut(|shell| shell.shell_map_on = false)
        .expect("receiving Shell mutation");
    owner.set_residual_shell_map(ResidualShellMapAction::Hide, false);
    !residual_shell_map_is_on(owner)
}
pub fn simulate_shell_map_toggle(owner: &crate::gui::window_manager::WindowManagerOwner) -> bool {
    let next = !residual_shell_map_is_on(owner);
    if next {
        simulate_shell_map_show(owner);
    } else {
        simulate_shell_map_hide(owner);
    }
    owner.set_residual_shell_map(ResidualShellMapAction::Toggle, next);
    residual_shell_map_is_on(owner) == next
}
pub fn simulate_shell_map_prepare_cycle(
    owner: &crate::gui::window_manager::WindowManagerOwner,
) -> bool {
    if !simulate_shell_map_show(owner) {
        return false;
    }
    simulate_shell_map_hide(owner)
}
