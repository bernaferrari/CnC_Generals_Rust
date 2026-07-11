use generals_main::playability_integration::{
    build_playability_audit, current_unresolved_blocker_examples,
    has_unresolved_blockers_with_headers, PlayabilityAuditSummary, PlayabilityGateConfig,
    PlayabilityPhase,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    let show_help = args.iter().any(|a| a == "--help" || a == "-h");
    let include_network = args.iter().any(|a| a == "--include-network");

    if show_help {
        print_help();
        return ExitCode::SUCCESS;
    }

    let target_phase = resolve_phase(&args);
    if strict {
        println!("Playability gate mode: strict");
        if target_phase.is_none() {
            println!(
                "Strict default phase: {}",
                PlayabilityPhase::PlayabilityReleaseCandidate.as_str()
            );
        }
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

            let unresolved = has_unresolved_blockers_with_headers(&summary);

            // Explicit --phase always enforces that phase's thresholds.
            if let Some(phase) = target_phase {
                if !summary.pass_gate(phase) {
                    eprintln!("phase gate failed: {}", summary.gate_description(phase));
                    return ExitCode::from(1);
                }
            }

            // --strict (with or without --phase) must never claim success on incomplete
            // matrix parity: apply release-candidate honesty when no phase is given,
            // and always fail on evaluate_strict_gate when --strict is set alone.
            if strict {
                if let Err(reason) = summary.evaluate_strict_gate() {
                    eprintln!("strict mode failed: {reason}");
                    if summary.stale_mapped_path_count() > 0 {
                        for path in summary.stale_mapped_paths.iter().take(12) {
                            eprintln!("  stale: {path}");
                        }
                    }
                    if summary.matrix_missing_entry_count() > 0 {
                        eprintln!(
                            "  matrix MISSING/UNKNOWN rows: {}",
                            summary.matrix_missing_entry_count()
                        );
                    }
                    println!(
                        "stale_mapped_paths={} matrix_missing={} total_parity={:.1}%",
                        summary.stale_mapped_path_count(),
                        summary.matrix_missing_entry_count(),
                        summary.total_parity_percent()
                    );
                    println!("{} unresolved blocker events", unresolved_events(&summary));
                    return ExitCode::from(1);
                }
            } else if unresolved {
                eprintln!("playability gate completed with warnings");
            }

            println!(
                "stale_mapped_paths={} matrix_missing={} total_parity={:.1}%",
                summary.stale_mapped_path_count(),
                summary.matrix_missing_entry_count(),
                summary.total_parity_percent()
            );
            println!("{} unresolved blocker events", unresolved_events(&summary));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("failed to build playability audit: {}", err);
            ExitCode::from(2)
        }
    }
}

fn unresolved_events(summary: &PlayabilityAuditSummary) -> usize {
    // Honest total: missing-file report + headers + matrix MISSING/UNKNOWN + stale.
    // Matrix MISSING was previously invisible here when the missing-files report was empty.
    summary.total_unresolved_including_headers() + summary.matrix_missing_entry_count()
}

fn print_help() {
    println!("Usage: playability_audit [--phase PHASE] [--strict] [--include-network] [--matrix-path PATH] [--missing-path PATH] [--mismatch-path PATH]");
    println!("\nComputes a single-file parity audit summary from PORT_*.txt artifacts.");
    println!("Flags:");
    println!("  --strict            Fail closed: release-candidate parity + zero blockers");
    println!("                      (matrix MISSING, stale FOUND paths, incomplete inputs)");
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
