use generals_main::ai_skirmish_activity::{format_ai_activity_report, run_medium_ai_skirmish_activity};

fn main() {
    let frames = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(120u32);
    let result = run_medium_ai_skirmish_activity(frames);
    println!("{}", format_ai_activity_report(&result));
    if result.status == "success"
        && (result.activity_count >= 2
            || result.ai_structures >= 3
            || (result.activity_count >= 1 && result.ai_units_or_queue >= 1))
    {
        println!("ai_skirmish_gate: PASS");
        std::process::exit(0);
    }
    eprintln!(
        "ai_skirmish_gate: FAIL activity={} status={}",
        result.activity_count, result.status
    );
    std::process::exit(1);
}
