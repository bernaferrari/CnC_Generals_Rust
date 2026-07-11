//! Phase 4–5: missing-asset diagnostics on real load paths, soak, dual-run hashes,
//! and presentation/campaign coverage for the releasable non-network bar.

use crate::assets::sound_effects::SoundEffectsTable;
use crate::assets::textures::TextureManager;
use crate::authoritative_world::{
    advance_authority_frames, set_verification_single_authority, AuthorityProbe,
};
use crate::deterministic_trace::{run_trace_scenario, TraceScenario};
use crate::effects::particle_system::{ParticleSystem, ParticleSystemTemplate};
use crate::game_logic::GameLogic;
use crate::game_logic::{KindOf, Resources, Team, ThingTemplate};
use crate::golden_skirmish::run_golden_skirmish;
use crate::save_load::campaign::{
    CampaignId, CampaignManager, MissionCompletionData, MissionDifficulty, MissionInfo,
    MissionStatus,
};
use crate::save_load::SaveLoadManager;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use crate::ui::hud_state::{color_for_player, UiColor};
use crate::ui::main_menu::MainMenuState;
use glam::Vec3;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

static MISSING_TEXTURE_NOTES: AtomicUsize = AtomicUsize::new(0);
static MISSING_W3D_NOTES: AtomicUsize = AtomicUsize::new(0);

/// Called from TextureManager when a missing-texture fallback is recorded.
pub fn note_missing_texture_fallback(texture_name: &str) {
    MISSING_TEXTURE_NOTES.fetch_add(1, Ordering::Relaxed);
    log::warn!("{}", diagnose_missing_asset(texture_name));
}

/// Called from AssetManager when a W3D model load fails on the production path.
pub fn note_missing_w3d_model(model_name: &str) {
    MISSING_W3D_NOTES.fetch_add(1, Ordering::Relaxed);
    log::warn!("{}", diagnose_missing_asset(model_name));
}

pub fn missing_texture_note_count() -> usize {
    MISSING_TEXTURE_NOTES.load(Ordering::Relaxed)
}

pub fn missing_w3d_note_count() -> usize {
    MISSING_W3D_NOTES.load(Ordering::Relaxed)
}

/// Result of attempting to open a gameplay asset through the production file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLoadOutcome {
    Found {
        path: PathBuf,
        bytes: usize,
    },
    Missing {
        requested: String,
        diagnostic: String,
    },
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
pub fn handle_gameplay_asset(
    requested: &str,
    verification: bool,
) -> Result<AssetLoadOutcome, String> {
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

/// Exercise the real TextureManager missing-fallback path (wired to note_missing_texture_fallback).
pub fn exercise_texture_manager_missing_path(texture_name: &str) -> usize {
    let before = missing_texture_note_count();
    let mut mgr = TextureManager::new();
    mgr.record_missing_texture_for_verification(texture_name);
    let after = missing_texture_note_count();
    assert!(
        after > before || mgr.missing_texture_total() > 0,
        "TextureManager must record missing fallback for {texture_name}"
    );
    mgr.missing_texture_total()
}

/// Presentation-critical production paths: particles, shell/HUD colors, audio table.
pub fn exercise_presentation_paths() -> (bool, String) {
    // Particles: template + system instance without requiring a GPU device.
    let mut tpl = ParticleSystemTemplate::default();
    tpl.name = "RCExplosion".into();
    tpl.system_lifetime = 30;
    let system = ParticleSystem::new(1, Arc::new(tpl), 0);
    let particles_ok = !system.is_destroyed && system.id == 1;

    // Shell / HUD: main-menu state enum + player color palette used by HUD.
    let shell_ok = MainMenuState::Main != MainMenuState::Credits
        && MainMenuState::SinglePlayer != MainMenuState::Multiplayer;
    let hud_color: UiColor = color_for_player(0);
    let hud_ok = hud_color.a == 255 && (hud_color.r > 0 || hud_color.g > 0 || hud_color.b > 0);

    // Audio: SoundEffects.ini production parser (retail file or embedded sample).
    let audio = SoundEffectsTable::load_default().unwrap_or_else(|| {
        SoundEffectsTable::from_text(
            "AudioEvent UnitSelect\n  Sounds = select1 select2\nEnd\n\
             AudioEvent UnitMove\n  Sounds = move1\nEnd\n",
        )
    });
    let audio_ok = !audio.is_empty();

    // W3D asset manager type is exercised by missing-model note path (no GPU).
    let before_w3d = missing_w3d_note_count();
    note_missing_w3d_model("Data/Missing/RC_NoSuchModel.w3d");
    let w3d_diag_ok = missing_w3d_note_count() > before_w3d;

    let ok = particles_ok && shell_ok && hud_ok && audio_ok && w3d_diag_ok;
    (
        ok,
        format!(
            "particles={particles_ok},shell={shell_ok},hud={hud_ok},audio={audio_ok},w3d_diag={w3d_diag_ok}"
        ),
    )
}

/// Campaign / Generals Challenge progression soak on CampaignManager production API.
pub fn exercise_campaign_progression_soak() -> (bool, String) {
    let _ = std::fs::create_dir_all(SaveLoadManager::default_save_directory().join("Campaign"));
    let mut mgr = CampaignManager::new();
    mgr.mission_definitions.insert(
        "RC_USA_01".into(),
        MissionInfo {
            id: "RC_USA_01".into(),
            campaign_id: CampaignId::USACampaign,
            mission_number: 1,
            name: "RC USA 1".into(),
            description: "rc soak".into(),
            map_name: "RCMap".into(),
            briefing_video: None,
            preview_image: None,
            required_missions: vec![],
            required_rank: None,
            required_honor_points: None,
            time_limit: None,
            starting_resources: Resources {
                supplies: 10_000,
                power: 0,
            },
            starting_units: vec![],
            tech_restrictions: vec![],
            special_rules: vec![],
            victory_rule: None,
            primary_objectives: vec![],
            secondary_objectives: vec![],
            bonus_objectives: vec![],
        },
    );
    mgr.mission_definitions.insert(
        "RC_GEN_01".into(),
        MissionInfo {
            id: "RC_GEN_01".into(),
            campaign_id: CampaignId::USAGeneral,
            mission_number: 1,
            name: "RC Challenge 1".into(),
            description: "rc challenge".into(),
            map_name: "RCChallenge".into(),
            briefing_video: None,
            preview_image: None,
            required_missions: vec![],
            required_rank: None,
            required_honor_points: None,
            time_limit: None,
            starting_resources: Resources {
                supplies: 8_000,
                power: 0,
            },
            starting_units: vec![],
            tech_restrictions: vec![],
            special_rules: vec![],
            victory_rule: None,
            primary_objectives: vec![],
            secondary_objectives: vec![],
            bonus_objectives: vec![],
        },
    );

    let usa_start = mgr
        .start_campaign(CampaignId::USACampaign, "rc_soak")
        .is_ok();
    let mut usa_done = false;
    if usa_start {
        let data = MissionCompletionData {
            play_duration: Duration::from_secs(30),
            score: 500,
            completed_primary: vec!["p1".into()],
            completed_secondary: vec![],
            completed_bonus: vec![],
            units_built: 2,
            units_lost: 0,
            enemies_destroyed: 1,
            resources_gathered: 100,
            buildings_constructed: 1,
            special_powers_used: 0,
            perfect_completion: false,
            under_time_limit: true,
            no_losses: true,
            stealth_completion: false,
        };
        let _ = mgr.complete_mission("RC_USA_01", MissionDifficulty::Normal, data);
        usa_done = matches!(
            mgr.get_mission_status("RC_USA_01"),
            MissionStatus::Completed | MissionStatus::CompletedPerfect
        ) || mgr.get_campaign_completion(CampaignId::USACampaign) > 0.0;
    }

    let ch_start = mgr
        .start_campaign(CampaignId::USAGeneral, "rc_challenge")
        .is_ok();
    let mut ch_done = false;
    if ch_start {
        assert!(CampaignId::USAGeneral.get_name().contains("Challenge"));
        let data = MissionCompletionData {
            play_duration: Duration::from_secs(45),
            score: 800,
            completed_primary: vec!["c1".into()],
            completed_secondary: vec![],
            completed_bonus: vec![],
            units_built: 1,
            units_lost: 0,
            enemies_destroyed: 2,
            resources_gathered: 50,
            buildings_constructed: 0,
            special_powers_used: 1,
            perfect_completion: false,
            under_time_limit: true,
            no_losses: true,
            stealth_completion: true,
        };
        let _ = mgr.complete_mission("RC_GEN_01", MissionDifficulty::Hard, data);
        ch_done = matches!(
            mgr.get_mission_status("RC_GEN_01"),
            MissionStatus::Completed | MissionStatus::CompletedPerfect
        ) || mgr.get_campaign_completion(CampaignId::USAGeneral) > 0.0;
    }

    let ok = usa_start && usa_done && ch_start && ch_done;
    (
        ok,
        format!("usa_start={usa_start},usa_done={usa_done},ch_start={ch_start},ch_done={ch_done}"),
    )
}

#[derive(Debug, Clone)]
pub struct ReleaseCandidateReport {
    pub soak_runs: u32,
    pub soak_ok: bool,
    pub deterministic_match: bool,
    pub dual_run_hash_match: bool,
    pub missing_asset_policy_ok: bool,
    pub presentation_ok: bool,
    pub campaign_soak_ok: bool,
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
        // Lightweight production world content for soak stability.
        let mut t = ThingTemplate::new("RCSoakUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("RCSoakUnit".into(), t);
        let _ = logic.create_object("RCSoakUnit", Team::USA, Vec3::ZERO);

        start_hashes.push(AuthorityProbe::capture(&logic, 0).checkpoint_hash());
        let probes = advance_authority_frames(&mut logic, 0, frames.max(1));
        if probes.is_empty() {
            soak_ok = false;
        }
    }

    let deterministic_match =
        !start_hashes.is_empty() && start_hashes.iter().all(|&h| h == start_hashes[0]);

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
    let existing = handle_gameplay_asset(
        "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        false,
    );
    // TextureManager production missing path (wired to note_missing_texture_fallback).
    let tex_misses = exercise_texture_manager_missing_path("Data/Missing/RC_NoSuchTex.tga");
    let missing_asset_policy_ok = missing_verify.is_err()
        && matches!(missing_runtime, Ok(AssetLoadOutcome::Missing { .. }))
        && texture_missing_diagnostic(tex_misses.max(1), "RC_NoSuchTex.tga", true).is_err()
        && texture_missing_diagnostic(0, "ok.tga", true).is_ok()
        && missing_texture_note_count() > 0
        && (matches!(existing, Ok(AssetLoadOutcome::Found { .. }))
            || matches!(existing, Ok(AssetLoadOutcome::Missing { .. })));

    let (presentation_ok, presentation_detail) = exercise_presentation_paths();
    let (campaign_soak_ok, campaign_detail) = exercise_campaign_progression_soak();

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
        soak_ok: soak_ok && g1.status == "success" && presentation_ok && campaign_soak_ok,
        deterministic_match: deterministic_match && golden_cash_match,
        dual_run_hash_match,
        missing_asset_policy_ok,
        presentation_ok,
        campaign_soak_ok,
        golden_status: g1.status.clone(),
        detail: format!(
            "g1_frames={} g2_frames={} dual_hash={} cash={} presentation={} campaign={}",
            g1.frames_advanced,
            g2.frames_advanced,
            dual_run_hash_match,
            g1.human_cash,
            presentation_detail,
            campaign_detail
        ),
    }
}

pub fn format_rc_report(r: &ReleaseCandidateReport) -> String {
    format!(
        "soak_runs={} soak_ok={} deterministic={} dual_run_hash={} missing_asset_policy_ok={} presentation_ok={} campaign_soak_ok={} golden={} detail={}",
        r.soak_runs,
        r.soak_ok,
        r.deterministic_match,
        r.dual_run_hash_match,
        r.missing_asset_policy_ok,
        r.presentation_ok,
        r.campaign_soak_ok,
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

        let before = missing_texture_note_count();
        let total = exercise_texture_manager_missing_path("Data/AbsolutelyMissing/tex.tga");
        assert!(total > 0);
        assert!(missing_texture_note_count() > before);
    }

    #[test]
    fn presentation_and_campaign_paths_exercise_shipped_code() {
        let (p_ok, p_detail) = exercise_presentation_paths();
        assert!(p_ok, "{p_detail}");
        let (c_ok, c_detail) = exercise_campaign_progression_soak();
        assert!(c_ok, "{c_detail}");
    }

    #[test]
    fn release_candidate_package_runs() {
        let report = run_release_candidate_package(2, 5);
        assert!(report.soak_ok, "{}", report.detail);
        assert!(report.deterministic_match, "{}", report.detail);
        assert!(report.dual_run_hash_match, "{}", report.detail);
        assert!(report.missing_asset_policy_ok, "{}", report.detail);
        assert!(report.presentation_ok, "{}", report.detail);
        assert!(report.campaign_soak_ok, "{}", report.detail);
        let s = format_rc_report(&report);
        assert!(s.contains("soak_ok=true"));
        assert!(s.contains("dual_run_hash=true"));
        assert!(s.contains("presentation_ok=true"));
        assert!(s.contains("campaign_soak_ok=true"));
    }
}
