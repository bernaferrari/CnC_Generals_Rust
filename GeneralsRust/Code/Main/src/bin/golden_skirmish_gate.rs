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
    // Map present: main combat on map armies — synthetic_combat=false, playable_claim=true.
    // Map absent: synthetic host soup — synthetic_combat=true, playable_claim=false.
    let map_same_world_ok = !result.map_loaded
        || (result.map_combat_ok
            && result.same_world_production_ok
            && result.same_world_victory_ok
            && result.players_preserved_on_load);
    let combat_claim_ok = if result.map_loaded {
        !result.synthetic_combat && result.playable_claim
    } else {
        result.synthetic_combat && !result.playable_claim
    };
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
        && combat_claim_ok
        && result.ai_structure_templates_retained
        && map_same_world_ok;
    if pass {
        println!(
            "golden_skirmish_gate: PASS (AI on; map_loaded={} synthetic_combat={} playable_claim={}; ai_templates_retained=true; map_same_world_prod={} map_same_world_victory={} retail_prod={})",
            result.map_loaded,
            result.synthetic_combat,
            result.playable_claim,
            result.same_world_production_ok,
            result.same_world_victory_ok,
            result.retail_production_chain_ok
        );
        std::process::exit(0);
    }
    eprintln!(
        "golden_skirmish_gate: FAIL victory={} save_load={} status={} ai_off={} synthetic={} playable_claim={} ai_templates_retained={} map_combat={} same_world_prod={} same_world_victory={} players_preserved={} retail_prod={} map_loaded={}",
        result.victory,
        result.save_load_ok,
        result.status,
        result.ai_disabled_for_slice,
        result.synthetic_combat,
        result.playable_claim,
        result.ai_structure_templates_retained,
        result.map_combat_ok,
        result.same_world_production_ok,
        result.same_world_victory_ok,
        result.players_preserved_on_load,
        result.retail_production_chain_ok,
        result.map_loaded
    );
    std::process::exit(1);
}
