use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
    let r = run_shell_smoke(10);
    println!("{}", format_shell_smoke_report(&r));
    // Fail-closed retail: headless smoke must never claim full W3D playability.
    // Limited host claim (shell_host_playable_ok) is required for success and is
    // independent of playable_claim — see shell_smoke module docs.
    let pass = r.status == "success"
        && !r.playable_claim
        && r.shell_host_playable_ok
        && r.control_bar_layout_ok
        && r.hud_selection_ok
        && r.screen_skirmish_ok;
    if pass {
        println!(
            "shell_smoke_gate: PASS (playable_claim={} shell_host_playable_ok={} control_bar={} hud_sel={} screen={} map_loaded={})",
            r.playable_claim,
            r.shell_host_playable_ok,
            r.control_bar_layout_ok,
            r.hud_selection_ok,
            r.screen_skirmish_ok,
            r.map_loaded
        );
        std::process::exit(0);
    }
    eprintln!(
        "shell_smoke_gate: FAIL status={} playable_claim={} shell_host_playable_ok={} control_bar={} {}",
        r.status, r.playable_claim, r.shell_host_playable_ok, r.control_bar_layout_ok, r.detail
    );
    std::process::exit(1);
}
