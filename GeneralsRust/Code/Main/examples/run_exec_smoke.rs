fn main() {
    let secs: u64 = std::env::var("GENERALS_EXEC_SMOKE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(90);
    let new_game = std::env::var("GENERALS_EXEC_SMOKE_NEW_GAME").is_ok();
    let r = generals_main::executable_smoke::run_executable_smoke(
        std::time::Duration::from_secs(secs),
        new_game,
    );
    println!(
        "{}",
        generals_main::executable_smoke::format_executable_smoke_report(&r)
    );
    println!("detail={}", r.detail);
    println!(
        "flags menu={} ingame={} shell_wnd={} map={:?}",
        r.reached_menu, r.reached_ingame, r.shell_wnd_ok, r.map_seen
    );
    if matches!(r.status.as_str(), "binary_missing") {
        std::process::exit(2);
    }
}
