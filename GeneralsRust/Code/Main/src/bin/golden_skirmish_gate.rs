use generals_main::golden_skirmish::{format_golden_report, run_golden_skirmish};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut map = None;
    let mut frames = 30u32;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" if i + 1 < args.len() => {
                map = Some(args[i + 1].clone());
                i += 2;
            }
            "--frames" if i + 1 < args.len() => {
                frames = args[i + 1].parse().unwrap_or(30);
                i += 2;
            }
            "--help" | "-h" => {
                println!("Usage: golden_skirmish_gate [--map PATH] [--frames N]");
                return;
            }
            other => {
                eprintln!("unknown arg {other}");
                std::process::exit(2);
            }
        }
    }
    let result = run_golden_skirmish(map.as_deref(), frames);
    println!("{}", format_golden_report(&result));
    // Full vertical-slice gate: config, frames, all gameplay steps, victory, save/load.
    let pass = result.config_applied
        && result.frames_advanced > 0
        && result.moved_units
        && result.gathered
        && result.constructed
        && result.produced
        && result.upgraded
        && result.fought
        && result.victory
        && result.save_load_ok
        && result.status == "success"
        && !result.ai_disabled_for_slice
        && result.playable_claim;
    if pass {
        println!("golden_skirmish_gate: PASS (natural host path; AI on; playable_claim=true; not windowed retail)");
        std::process::exit(0);
    }
    eprintln!(
        "golden_skirmish_gate: FAIL victory={} save_load={} status={} ai_off={} playable_claim={}",
        result.victory,
        result.save_load_ok,
        result.status,
        result.ai_disabled_for_slice,
        result.playable_claim
    );
    std::process::exit(1);
}
