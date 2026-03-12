use std::env;
use std::path::Path;
use std::process;
use ww3d_validation::{
    export_full_report, load_log, run_full_validation, texture_analysis::print_diff,
    texture_analysis::print_summary, CompatibilityValidator, TextureDecisionLog,
};

fn usage() {
    eprintln!("Usage:");
    eprintln!("  ww3d_validation export <human_report> <decisions_log>");
    eprintln!("  ww3d_validation diff <lhs_log> <rhs_log>");
    eprintln!("  ww3d_validation summary <log>");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "export" if args.len() == 4 => {
            let human = Path::new(&args[2]);
            let decisions = Path::new(&args[3]);
            if let Err(err) = export_full_report(human, decisions) {
                eprintln!("Failed to export report: {err}");
                process::exit(1);
            }
        }
        "diff" if args.len() == 4 => match (load_log(&args[2]), load_log(&args[3])) {
            (Ok(lhs), Ok(rhs)) => {
                let diff = ww3d_validation::texture_analysis::diff_logs(&lhs, &rhs);
                print_diff(&diff);
            }
            (Err(err), _) | (_, Err(err)) => {
                eprintln!("Error loading logs: {err}");
                process::exit(1);
            }
        },
        "summary" if args.len() == 3 => match load_log(&args[2]) {
            Ok(log) => print_summary(&log),
            Err(err) => {
                eprintln!("Error loading log: {err}");
                process::exit(1);
            }
        },
        _ => {
            usage();
            process::exit(1);
        }
    }
}
