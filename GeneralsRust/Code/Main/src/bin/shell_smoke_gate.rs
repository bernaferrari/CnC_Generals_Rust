use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() -> generals_sim::GameLogicResult<()> {
    let _ = env_logger::try_init();
    let r = run_shell_smoke(10)?;
    println!("{}", format_shell_smoke_report(&r));
    let pass = r.status == "success"
        && r.session_ok
        && r.control_bar_layout_ok
        && r.hud_selection_ok
        && r.selection_consumers_ok
        && r.screen_skirmish_ok
        && r.presentation_ok;
    if !pass {
        std::process::exit(1);
    }
    Ok(())
}
