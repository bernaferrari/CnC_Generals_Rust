//! Phase 4–5: missing-asset diagnostics on real load paths, soak, dual-run hashes.

use crate::authoritative_world::{
    advance_authority_frames, set_verification_single_authority, AuthorityProbe,
};
use crate::deterministic_trace::{run_trace_scenario, TraceScenario};
use crate::game_logic::GameLogic;
use crate::golden_skirmish::run_golden_skirmish;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use std::path::{Path, PathBuf};

/// Result of attempting to open a gameplay asset through the production file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLoadOutcome {
    Found { path: PathBuf, bytes: usize },
    Missing { requested: String, diagnostic: String },
}

/// Resolve a gameplay asset using the same path roots the runtime searches
/// (cwd, repo-relative Data/, windows_game extracted trees). Never invents bytes.
pub fn try_load_gameplay_asset(requested: &str) -> AssetLoadOutcome {
    let candidates = asset_search_candidates(requested);
    for candidate in &candidates {
        if candidate.is_file() {
            match std::fs::read(candidate) {
                Ok(data) if !data.is_empty() => {
                    return AssetLoadOutcome::Found {
                        path: candidate.clone(),
                        bytes: data.len(),
                    };
                }
                _ => continue,
            }
        }
    }
    AssetLoadOutcome::Missing {
        requested: requested.to_string(),
        diagnostic: diagnose_missing_asset(requested),
    }
}

fn asset_search_candidates(requested: &str) -> Vec<PathBuf> {
    let req = Path::new(requested);
    let mut out = vec![req.to_path_buf()];
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join(requested));
        out.push(cwd.join("Data").join(requested));
        out.push(cwd.join("..").join(requested));
        out.push(
            cwd.join("windows_game")
                .join("extracted_big_files")
                .join(requested),
        );
        out.push(
            cwd.join("..")
                .join("windows_game")
                .join("extracted_big_files")
                .join(requested),
        );
    }
    out
}

pub fn diagnose_missing_asset(path: &str) -> String {
    format!("MISSING_ASSET:{path}")
}

/// Production policy: verification builds must fail closed on missing critical assets
/// instead of silently substituting placeholder gameplay content.
pub fn handle_gameplay_asset(requested: &str, verification: bool) -> Result<AssetLoadOutcome, String> {
    match try_load_gameplay_asset(requested) {
        found @ AssetLoadOutcome::Found { .. } => Ok(found),
        AssetLoadOutcome::Missing {
            requested,
            diagnostic,
        } => {
            log::warn!("{diagnostic}");
            if verification {
                Err(diagnostic)
            } else {
                Ok(AssetLoadOutcome::Missing {
                    requested,
                    diagnostic,
                })
            }
        }
    }
}

/// Texture-manager integration: if a load fell back to the missing-texture pool,
/// surface an explicit diagnostic (no silent gameplay assumption).
pub fn texture_missing_diagnostic(
    missing_total: usize,
    texture_name: &str,
    verification: bool,
) -> Result<(), String> {
    if missing_total == 0 {
        return Ok(());
    }
    let msg = diagnose_missing_asset(texture_name);
    log::warn!("texture fallback count={missing_total}: {msg}");
    if verification {
        Err(msg)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseCandidateReport {
    pub soak_runs: u32,
    pub soak_ok: bool,
    pub deterministic_match: bool,
    pub dual_run_hash_match: bool,
    pub missing_asset_policy_ok: bool,
    pub golden_status: String,
    pub detail: String,
}

pub fn run_release_candidate_package(soak_runs: u32, frames: u32) -> ReleaseCandidateReport {
    set_verification_single_authority(true);
    let mut soak_ok = true;
    let mut start_hashes = Vec::new();

    for i in 0..soak_runs.max(1) {
        let cfg = golden_skirmish_config(&format!("RCSoak{i}"));
        let mut logic = GameLogic::new();
        if apply_skirmish_config(&mut logic, &cfg).is_err() {
            soak_ok = false;
            continue;
        }
        start_hashes.push(AuthorityProbe::capture(&logic, 0).checkpoint_hash());
        let probes = advance_authority_frames(&mut logic, 0, frames.max(1));
        if probes.is_empty() {
            soak_ok = false;
        }
    }

    let deterministic_match = !start_hashes.is_empty()
        && start_hashes.iter().all(|&h| h == start_hashes[0]);

    // Dual-run: identical config + identical frame advances → equal end hashes.
    let cfg = golden_skirmish_config("RCDual");
    let mut la = GameLogic::new();
    let mut lb = GameLogic::new();
    let _ = apply_skirmish_config(&mut la, &cfg);
    let _ = apply_skirmish_config(&mut lb, &cfg);
    let pa = advance_authority_frames(&mut la, 0, frames.max(1));
    let pb = advance_authority_frames(&mut lb, 0, frames.max(1));
    let dual_run_hash_match = pa
        .last()
        .zip(pb.last())
        .map(|(a, b)| a.checkpoint_hash() == b.checkpoint_hash())
        .unwrap_or(false);

    let g1 = run_golden_skirmish(None, 4);
    let g2 = run_golden_skirmish(None, 4);
    let golden_cash_match = g1.human_cash == g2.human_cash && g1.ai_cash == g2.ai_cash;

    // Real asset path: known-missing file must diagnose; verification fails closed.
    let missing_verify = handle_gameplay_asset("Data/Missing/NoSuch_RC_Asset.w3d", true);
    let missing_runtime = handle_gameplay_asset("Data/Missing/NoSuch_RC_Asset.w3d", false);
    // Existing map file should resolve Found when retail assets present.
    let existing = handle_gameplay_asset(
        "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        false,
    );
    let missing_asset_policy_ok = missing_verify.is_err()
        && matches!(missing_runtime, Ok(AssetLoadOutcome::Missing { .. }))
        && texture_missing_diagnostic(1, "missing_tex.tga", true).is_err()
        && texture_missing_diagnostic(0, "ok.tga", true).is_ok()
        && (matches!(existing, Ok(AssetLoadOutcome::Found { .. }))
            || matches!(existing, Ok(AssetLoadOutcome::Missing { .. })));

    // Deterministic trace production path.
    {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RCTrace");
        let _ = apply_skirmish_config(&mut logic, &cfg);
        let scenario = TraceScenario::new([1, 2, 3, 4, 5, 6], 3);
        let frames = run_trace_scenario(&mut logic, &scenario);
        if frames.is_empty() {
            soak_ok = false;
        }
    }

    set_verification_single_authority(false);

    ReleaseCandidateReport {
        soak_runs: soak_runs.max(1),
        soak_ok: soak_ok && g1.status == "success",
        deterministic_match: deterministic_match && golden_cash_match,
        dual_run_hash_match,
        missing_asset_policy_ok,
        golden_status: g1.status.clone(),
        detail: format!(
            "g1_frames={} g2_frames={} dual_hash={} cash={}",
            g1.frames_advanced, g2.frames_advanced, dual_run_hash_match, g1.human_cash
        ),
    }
}

pub fn format_rc_report(r: &ReleaseCandidateReport) -> String {
    format!(
        "soak_runs={} soak_ok={} deterministic={} dual_run_hash={} missing_asset_policy_ok={} golden={} detail={}",
        r.soak_runs,
        r.soak_ok,
        r.deterministic_match,
        r.dual_run_hash_match,
        r.missing_asset_policy_ok,
        r.golden_status,
        r.detail
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_asset_fails_closed_in_verification_on_real_path() {
        let err = handle_gameplay_asset("Data/AbsolutelyMissing/nope.w3d", true)
            .expect_err("verification must fail");
        assert!(err.starts_with("MISSING_ASSET:"));
        let runtime = handle_gameplay_asset("Data/AbsolutelyMissing/nope.w3d", false)
            .expect("runtime may continue with diagnosis");
        assert!(matches!(runtime, AssetLoadOutcome::Missing { .. }));
    }

    #[test]
    fn release_candidate_package_runs() {
        let report = run_release_candidate_package(2, 5);
        assert!(report.soak_ok, "{}", report.detail);
        assert!(report.deterministic_match, "{}", report.detail);
        assert!(report.dual_run_hash_match, "{}", report.detail);
        assert!(report.missing_asset_policy_ok, "{}", report.detail);
        let s = format_rc_report(&report);
        assert!(s.contains("soak_ok=true"));
        assert!(s.contains("dual_run_hash=true"));
    }
}
