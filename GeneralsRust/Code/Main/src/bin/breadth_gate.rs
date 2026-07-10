use generals_main::breadth_scenarios::{format_breadth_report, run_all_breadth};

fn main() {
    let results = run_all_breadth();
    println!("{}", format_breadth_report(&results));
    if results.iter().all(|r| r.ok) {
        println!("breadth_gate: PASS");
        std::process::exit(0);
    }
    eprintln!("breadth_gate: FAIL");
    std::process::exit(1);
}
