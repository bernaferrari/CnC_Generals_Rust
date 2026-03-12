use generals_main::playability_integration::{
    build_playability_audit, current_unresolved_blocker_examples,
    has_unresolved_blockers_with_headers, PlayabilityAuditSummary, PlayabilityGateConfig,
    PlayabilityPhase,
};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    let show_help = args.iter().any(|a| a == "--help" || a == "-h");
    let include_network = args.iter().any(|a| a == "--include-network");

    if show_help {
        print_help();
        return;
    }

    let target_phase = resolve_phase(&args);
    if strict {
        println!("Playability gate mode: strict");
    }
    if let Some(phase) = target_phase {
        println!("Phase gate: {}", phase.as_str());
    }
    if include_network {
        println!("Scope: including GameNetwork");
    }

    let config = resolve_config(&args);
    match build_playability_audit(&config) {
        Ok(summary) => {
            print_report(
                &summary,
                config.matrix_path.as_path(),
                config.missing_files_path.as_path(),
                config.mismatch_files_path.as_path(),
            );
            let strict_mode = strict && target_phase.is_none();
            let unresolved = if strict_mode {
                summary.total_unresolved_including_headers() > 0
            } else {
                has_unresolved_blockers_with_headers(&summary)
            };

            if let Some(phase) = target_phase {
                if !summary.pass_gate(phase) {
                    eprintln!("phase gate failed: {}", summary.gate_description(phase));
                    std::process::exit(1);
                }
            }

            if strict_mode && unresolved {
                eprintln!("strict mode failed: unresolved blockers remain");
                std::process::exit(1);
            }
            if !strict_mode && unresolved {
                eprintln!("playability gate completed with warnings");
            }

            println!("{} unresolved blocker events", unresolved_events(&summary));
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("failed to build playability audit: {}", err);
            std::process::exit(2);
        }
    }
}

fn unresolved_events(summary: &PlayabilityAuditSummary) -> usize {
    summary.total_unresolved_including_headers()
}

fn print_help() {
    println!("Usage: playability_audit [--phase PHASE] [--strict] [--include-network] [--matrix-path PATH] [--missing-path PATH] [--mismatch-path PATH]");
    println!("\nComputes a single-file parity audit summary from PORT_*.txt artifacts.");
    println!("Flags:");
    println!("  --strict            Fail unless no unresolved blockers are detected");
    println!("  --phase PHASE       Run phase gate (baseline|gameplay|saveload|ui|release)");
    println!("  --include-network   Include deferred GameNetwork rows in parity checks");
    println!("  --matrix-path PATH  Override PORT_FILE_MATRIX source path");
    println!("  --missing-path PATH Override PORT_MISSING_FILES_BY_SUBSYSTEM path");
    println!("  --mismatch-path PATH Override PORT_FILE_MISMATCHES_BY_SUBSYSTEM path");
    println!("  -h, --help         Show this help text");
}

fn resolve_phase(args: &[String]) -> Option<PlayabilityPhase> {
    let mut i = 0;
    while i < args.len() {
        if args[i].as_str() == "--phase" && i + 1 < args.len() {
            if let Some(phase) = PlayabilityPhase::from_arg(&args[i + 1]) {
                return Some(phase);
            }
            eprintln!(
                "unknown phase '{}'; expected baseline|gameplay|saveload|ui|release",
                args[i + 1]
            );
        }
        i += 1;
    }
    None
}

fn resolve_config(args: &[String]) -> PlayabilityGateConfig {
    let mut matrix_path = None;
    let mut missing_path = None;
    let mut mismatch_path = None;
    let mut include_network = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--matrix-path" if i + 1 < args.len() => {
                matrix_path = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--missing-path" if i + 1 < args.len() => {
                missing_path = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--mismatch-path" if i + 1 < args.len() => {
                mismatch_path = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--include-network" => {
                include_network = true;
            }
            _ => {}
        }
        i += 1;
    }

    PlayabilityGateConfig::new(
        matrix_path.unwrap_or_else(PlayabilityGateConfig::default_matrix_path),
        missing_path.unwrap_or_else(PlayabilityGateConfig::default_missing_files_path),
        mismatch_path.unwrap_or_else(PlayabilityGateConfig::default_mismatch_files_path),
    )
    .with_include_network(include_network)
}

fn print_report(
    summary: &PlayabilityAuditSummary,
    matrix_path: &std::path::Path,
    missing_path: &std::path::Path,
    mismatch_path: &std::path::Path,
) {
    println!("Playability Audit Report");
    println!("Matrix: {}", matrix_path.display());
    println!("Missing files: {}", missing_path.display());
    println!("Mismatches: {}", mismatch_path.display());
    println!();
    println!("{}", summary);
    let blockers = current_unresolved_blocker_examples(summary, 8);
    if !blockers.is_empty() {
        println!();
        println!("Sample unresolved entries:");
        for line in blockers {
            println!("  - {}", line);
        }
    }
}
