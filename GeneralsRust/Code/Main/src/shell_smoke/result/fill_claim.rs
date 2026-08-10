// Fill claim / shell-ui residual fields. playable_claim is the passed-in value.
#[rustfmt::skip]
fn fill_claim(
    out: &mut ShellSmokeResult,
    shell_ui: &super::shell::ShellUiResiduals,
    selection_consumers_ok: bool,
    shell_host_playable_ok: bool,
    playable_claim: bool,
    status: String,
) {
    out.screen_skirmish_ok = shell_ui.screen_skirmish_ok;
    out.control_bar_layout_ok = shell_ui.control_bar_layout_ok;
    out.control_bar_path_resolved = shell_ui.control_bar_path_resolved;
    out.control_bar_wnd_validated = shell_ui.control_bar_wnd_validated;
    out.control_bar_window_loaded = shell_ui.control_bar_window_loaded;
    out.control_bar_window_count = shell_ui.control_bar_window_count;
    out.selection_consumers_ok = selection_consumers_ok;
    out.shell_host_playable_ok = shell_host_playable_ok;
    out.playable_claim = playable_claim;
    out.status = status;
}
