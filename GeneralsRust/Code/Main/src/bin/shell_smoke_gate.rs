use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
    let r = run_shell_smoke(10);
    println!("{}", format_shell_smoke_report(&r));
    if r.status == "success" {
        println!("shell_smoke_gate: PASS");
        std::process::exit(0);
    }
    eprintln!("shell_smoke_gate: FAIL {}", r.detail);
    std::process::exit(1);
}
