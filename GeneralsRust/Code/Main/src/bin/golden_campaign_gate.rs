//! Gate binary: SinglePlayer campaign residual path.
//!
//! Proves campaign start, mission scripts tick, frames advance, victory path,
//! without panicking. Fail-closed honesty:
//! - campaign_playable_claim — SP path advanced with scripts/victory
//! - retail_campaign_map_loaded — full MD_*/GC_* load_map (expected residual false)
//!
//! Usage:
//!   golden_campaign_gate [--map NAME] [--frames N]
//!
//! Exit 0 on campaign_playable_claim && status=success.
//! Exit 1 otherwise.

use generals_main::golden_campaign::{
    format_campaign_report, run_golden_campaign, DEFAULT_CAMPAIGN_FRAME_ADVANCE,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: golden_campaign_gate [--map NAME_OR_PATH] [--frames N]");
        println!("SinglePlayer campaign residual: scripts tick + frames + victory path.");
        return;
    }

    let mut map: Option<String> = None;
    let mut frames = DEFAULT_CAMPAIGN_FRAME_ADVANCE;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" if i + 1 < args.len() => {
                map = Some(args[i + 1].clone());
                i += 2;
            }
            "--frames" if i + 1 < args.len() => match args[i + 1].parse::<u32>() {
                Ok(n) if n > 0 => {
                    frames = n;
                    i += 2;
                }
                _ => {
                    eprintln!("invalid --frames value: {}", args[i + 1]);
                    std::process::exit(2);
                }
            },
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let result = run_golden_campaign(map.as_deref(), frames);
    println!("{}", format_campaign_report(&result));

    if result.campaign_playable_claim && result.status == "success" {
        println!(
            "golden_campaign_gate: PASS campaign_playable_claim=true retail_campaign_map_loaded={} scripts={} frames={}",
            result.retail_campaign_map_loaded,
            result.campaign_script_count,
            result.frames_advanced
        );
        if !result.retail_campaign_map_loaded {
            eprintln!(
                "golden_campaign_gate: residual retail_campaign_map_loaded=false (full MD_*/GC_* load_map deferred; scripts-only path proven)"
            );
        }
        if !result.victory_rule_applied {
            eprintln!(
                "golden_campaign_gate: residual victory_rule_applied=false (campaign override not observed for host map key)"
            );
        }
        std::process::exit(0);
    }

    eprintln!(
        "golden_campaign_gate: FAIL claim={} status={} started={} scripts_tick={} victory_eval={} frames={}",
        result.campaign_playable_claim,
        result.status,
        result.campaign_started,
        result.scripts_tick_ok,
        result.victory_eval_ok,
        result.frames_advanced
    );
    std::process::exit(1);
}
