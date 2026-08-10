// Assemble ShellSmokeResult from evaluated residual parts.

/// Assemble the host-smoke result from the evaluated residual parts.
///
/// Field groups are filled by helper fns (rustc 1.96 rejects include!/macros in
/// struct-literal field position). `playable_claim` is passed in (`false` at the
/// call site) and must not be flipped here.
#[rustfmt::skip]
pub fn assemble(
    host: super::host::HostSession,
    presentation: super::presentation::PresentationResiduals,
    early: super::honesty::EarlyHonesty,
    waves: super::honesty_waves::WaveHonesty,
    hud_selection_ok: bool,
    selection_consumers_ok: bool,
    shell_ui: super::shell::ShellUiResiduals,
    playable_claim: bool,
) -> ShellSmokeResult {
    // When assets present, map must load; when absent, still pass config+frames.
    let map_requirement_ok = super::claim::map_requirement_ok(host.map_resolved, host.map_loaded);

    // playable_claim is passed in from run_shell_smoke (`let playable_claim = false;`).
    // Do not flip it here.

    let host_path_ok = super::claim::host_path_ok(
        host.host_constructed,
        host.skirmish_config_ok,
        host.menu_config_ok,
        host.frames_ok,
        host.presentation_ok,
        hud_selection_ok,
        selection_consumers_ok,
        host.dual_tick_presentation_ok,
        shell_ui.screen_skirmish_ok,
        shell_ui.control_bar_layout_ok,
        map_requirement_ok,
    );

    // Limited claim: headless production host path is operational end-to-end.
    // Requires dual-tick presentation + multi-consumer selection + shell→InGame +
    // ControlBar.wnd ensure. Still not windowed W3D play (playable_claim stays false).
    let shell_host_playable_ok = host_path_ok;

    let status = super::claim::status_from_host_path(host_path_ok);

    let mut out = ShellSmokeResult::default();
    fill_core(&mut out, &host, hud_selection_ok);
    fill_presentation(&mut out, &presentation);
    fill_waves_72_150(&mut out, &early, &shell_ui);
    fill_waves_151_250(&mut out, &early);
    fill_waves_251_350(&mut out, &early);
    fill_waves_351_450(&mut out, &early, &waves);
    fill_waves_451_550(&mut out, &waves);
    fill_waves_551_650(&mut out, &waves);
    fill_waves_651_750(&mut out, &waves);
    fill_waves_751_850(&mut out, &waves);
    fill_waves_851_941(&mut out, &waves);
    fill_claim(
        &mut out,
        &shell_ui,
        selection_consumers_ok,
        shell_host_playable_ok,
        playable_claim,
        status,
    );
    out.detail = format_detail(&out, &shell_ui.layout_report);
    out
}
