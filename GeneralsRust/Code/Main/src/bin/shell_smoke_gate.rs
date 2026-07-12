use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
    let r = run_shell_smoke(10);
    println!("{}", format_shell_smoke_report(&r));
    // Fail-closed: headless smoke must never claim retail playability.
    if r.status == "success" && !r.playable_claim {
        println!(
            "shell_smoke_gate: PASS (playable_claim={})",
            r.playable_claim
        );
        std::process::exit(0);
    }
    eprintln!(
        "shell_smoke_gate: FAIL status={} playable_claim={} {}",
        r.status, r.playable_claim, r.detail
    );
    std::process::exit(1);
}
