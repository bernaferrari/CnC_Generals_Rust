//! Runs a real headless skirmish session and checks its presentation/UI consumers.
mod helpers;
mod host;
mod hud;
mod imports;

mod maps;
mod report;
mod result;

mod shell;
pub use report::format_shell_smoke_report;
pub use result::ShellSmokeResult;

pub fn run_shell_smoke(frames: u32) -> generals_sim::GameLogicResult<ShellSmokeResult> {
    let mut logic = crate::game_logic::GameLogic::new();
    // Standalone archive mounting and menu localization still use the application's service scope.
    logic.with_own_services_mut(|logic| run_shell_session(logic, frames))
}

fn run_shell_session(
    logic: &mut crate::game_logic::GameLogic,
    frames: u32,
) -> generals_sim::GameLogicResult<ShellSmokeResult> {
    let host = host::run_host_session(logic, frames)?;
    let (hud_selection_ok, selection_consumers_ok) = hud::evaluate_selection(&host, logic);
    let shell_ui = shell::evaluate_shell_ui();
    let host_path_ok = host.host_constructed
        && host.skirmish_config_ok
        && host.menu_config_ok
        && host.frames_ok
        && host.presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && shell_ui.screen_skirmish_ok
        && shell_ui.control_bar_layout_ok
        && (!host.map_resolved || host.map_loaded);
    let mut out = ShellSmokeResult {
        hud_selection_ok,
        selection_consumers_ok,
        session_ok: host_path_ok,
        status: if host_path_ok { "success" } else { "partial" },
        detail: format!(
            "frames={} map_loaded={} selection={} presentation={} layout={}",
            host.frames_advanced,
            host.map_loaded,
            hud_selection_ok,
            host.presentation_ok,
            shell_ui.layout_report
        ),
        ..ShellSmokeResult::default()
    };
    out.host_constructed = host.host_constructed;
    out.skirmish_config_ok = host.skirmish_config_ok;
    out.menu_config_ok = host.menu_config_ok;
    out.map_resolved = host.map_resolved;
    out.map_loaded = host.map_loaded;
    out.frames_advanced = host.frames_advanced;
    out.frames_ok = host.frames_ok;
    out.presentation_ok = host.presentation_ok;
    out.screen_skirmish_ok = shell_ui.screen_skirmish_ok;
    out.control_bar_layout_ok = shell_ui.control_bar_layout_ok;
    out.control_bar_path_resolved = shell_ui.control_bar_path_resolved;
    out.control_bar_wnd_validated = shell_ui.control_bar_wnd_validated;
    out.control_bar_window_loaded = shell_ui.control_bar_window_loaded;
    out.control_bar_window_count = shell_ui.control_bar_window_count;
    Ok(out)
}

#[cfg(test)]
mod tests;
