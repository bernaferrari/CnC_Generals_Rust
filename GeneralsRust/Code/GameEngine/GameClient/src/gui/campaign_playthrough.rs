//! C++ `CampaignManager` single-player / Generals Challenge playthrough walker.
//!
//! Walks INI-loaded campaigns (USA/GLA/China/CHALLENGE_*) via
//! `setCampaign` → `getCurrentMap` → `gotoNextMission`, the same sequence
//! DifficultySelect / ChallengeMenu use to start maps. Briefing fields come
//! from Mission INI (IntroMovie, LocationNameLabel, BriefingVoice, objectives).
//! Score hooks come from CampaignManager `setVictorious` / rank points /
//! FinalVictoryMovie (ScoreScreen path).

use super::campaign_manager::CampaignManager;
use super::challenge_generals::{ChallengeGenerals, NUM_GENERALS};

/// One mission step on a campaign chain, including briefing/score fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignPlaythroughStep {
    pub campaign: String,
    pub mission: String,
    pub map_name: String,
    pub is_challenge: bool,
    pub movie_label: String,
    pub location_name_label: String,
    pub briefing_voice: String,
    pub general_name: String,
    pub objective_line0: String,
    pub final_victory_movie: String,
    pub player_faction: String,
    pub rank_points: i32,
    pub victorious: bool,
}

fn step_from_manager(manager: &CampaignManager) -> Option<CampaignPlaythroughStep> {
    let campaign = manager.get_current_campaign()?;
    let mission = manager.get_current_mission()?;
    if mission.map_name.is_empty() {
        return None;
    }
    Some(CampaignPlaythroughStep {
        campaign: campaign.name.clone(),
        mission: mission.name.clone(),
        map_name: mission.map_name.clone(),
        is_challenge: campaign.is_challenge_campaign,
        movie_label: mission.movie_label.clone(),
        location_name_label: mission.location_name_label.clone(),
        briefing_voice: mission.briefing_voice.sound_file.clone(),
        general_name: mission.general_name.clone(),
        objective_line0: mission.mission_objectives_label[0].clone(),
        final_victory_movie: campaign.final_movie_name.clone(),
        player_faction: campaign.player_faction_name.clone(),
        rank_points: manager.get_rank_points(),
        victorious: manager.is_victorious(),
    })
}

/// Walk every mission in `campaign_name` following `NextMission` links.
pub fn play_through_campaign(
    manager: &mut CampaignManager,
    campaign_name: &str,
) -> Vec<CampaignPlaythroughStep> {
    manager.set_campaign(campaign_name);
    let mut steps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(step) = step_from_manager(manager) {
        if !seen.insert(step.mission.clone()) {
            break;
        }
        steps.push(step);
        if manager.goto_next_mission().is_none() {
            break;
        }
    }
    // ScoreScreen hook: campaign completion latches victorious + keeps rank points.
    if !steps.is_empty() {
        manager.set_victorious(true);
        if let Some(last) = steps.last_mut() {
            last.victorious = manager.is_victorious();
            last.rank_points = manager.get_rank_points();
        }
    }
    steps
}

/// Challenge menu Start: `setCampaign(general.campaign())` then first mission map.
pub fn start_challenge_playthrough(
    manager: &mut CampaignManager,
    generals: &ChallengeGenerals,
    general_index: usize,
) -> Option<CampaignPlaythroughStep> {
    let general = generals.challenge_generals().get(general_index)?;
    let campaign = general.campaign();
    if campaign.is_empty() || campaign.eq_ignore_ascii_case("unimplemented") {
        return None;
    }
    manager.set_campaign(campaign);
    let step = step_from_manager(manager)?;
    if !step.is_challenge {
        return None;
    }
    Some(step)
}

/// Retail Challenge.ini persona campaigns that are real (not `unimplemented`).
pub fn playable_challenge_campaign_names() -> [&'static str; 9] {
    [
        "CHALLENGE_0",
        "CHALLENGE_1",
        "CHALLENGE_2",
        "CHALLENGE_3",
        "CHALLENGE_4",
        "CHALLENGE_5",
        "CHALLENGE_6",
        "CHALLENGE_7",
        "CHALLENGE_8",
    ]
}

/// Map path as stored in Campaign.ini (`Maps\MD_USA01\MD_USA01.map`) → retail file stem.
pub fn campaign_map_stem(map_name: &str) -> String {
    let normalized = map_name.replace('\\', "/");
    Path::new(&normalized)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(map_name)
        .to_string()
}

use std::path::Path;

/// Resolve a Campaign.ini map name against extracted MapsZH.
pub fn resolve_campaign_map_file(map_name: &str) -> Option<std::path::PathBuf> {
    let stem = campaign_map_stem(map_name);
    let rel = map_name.replace('\\', "/");
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors().take(8) {
            roots.push(ancestor.to_path_buf());
        }
    }
    for root in roots {
        let candidates = [
            root.join(&rel),
            root.join("windows_game/extracted_big_files/MapsZH")
                .join(&rel),
            root.join("windows_game/extracted_big_files/MapsZH/Maps")
                .join(&stem)
                .join(format!("{stem}.map")),
        ];
        for path in candidates {
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// Public helper used by host/golden: init INI store then walk USA/GLA/China
/// plus every Challenge persona campaign and all 12 Challenge generals.
pub fn run_retail_campaign_and_challenge_playthrough() -> CampaignPlaythroughReport {
    let mut manager = CampaignManager::new();
    manager.init();

    let usa = play_through_campaign(&mut manager, "USA");
    let gla = play_through_campaign(&mut manager, "GLA");
    let china = play_through_campaign(&mut manager, "China");
    let training = play_through_campaign(&mut manager, "TRAINING");

    let mut challenge_campaigns: Vec<(String, Vec<CampaignPlaythroughStep>)> = Vec::new();
    for name in playable_challenge_campaign_names() {
        let steps = play_through_campaign(&mut manager, name);
        challenge_campaigns.push((name.to_string(), steps));
    }

    let mut generals = ChallengeGenerals::new();
    generals.init();
    let mut challenge_general_starts = Vec::new();
    let mut challenge_generals_unimplemented = 0usize;
    for index in 0..NUM_GENERALS {
        match start_challenge_playthrough(&mut manager, &generals, index) {
            Some(step) => challenge_general_starts.push((index, step)),
            None => challenge_generals_unimplemented += 1,
        }
    }

    let challenge = challenge_general_starts
        .first()
        .map(|(_, step)| step.clone())
        .or_else(|| {
            challenge_campaigns
                .first()
                .and_then(|(_, steps)| steps.first().cloned())
        });

    let usa_maps_on_disk = usa
        .iter()
        .filter(|s| resolve_campaign_map_file(&s.map_name).is_some())
        .count();
    let challenge_map_on_disk = challenge
        .as_ref()
        .and_then(|s| resolve_campaign_map_file(&s.map_name))
        .is_some();

    CampaignPlaythroughReport {
        campaigns_loaded: manager.campaign_count(),
        usa_missions: usa,
        gla_missions: gla,
        china_missions: china,
        training_missions: training,
        challenge_campaigns,
        challenge_general_starts,
        challenge_generals_unimplemented,
        challenge_first: challenge,
        usa_maps_on_disk,
        challenge_map_on_disk,
    }
}

/// Report for host/golden honesty.
#[derive(Debug, Clone)]
pub struct CampaignPlaythroughReport {
    pub campaigns_loaded: usize,
    pub usa_missions: Vec<CampaignPlaythroughStep>,
    pub gla_missions: Vec<CampaignPlaythroughStep>,
    pub china_missions: Vec<CampaignPlaythroughStep>,
    pub training_missions: Vec<CampaignPlaythroughStep>,
    pub challenge_campaigns: Vec<(String, Vec<CampaignPlaythroughStep>)>,
    pub challenge_general_starts: Vec<(usize, CampaignPlaythroughStep)>,
    pub challenge_generals_unimplemented: usize,
    pub challenge_first: Option<CampaignPlaythroughStep>,
    pub usa_maps_on_disk: usize,
    pub challenge_map_on_disk: bool,
}

impl CampaignPlaythroughReport {
    /// True when Campaign.ini USA/GLA/China chains and all playable Challenge
    /// generals walk with briefing/score hooks populated.
    pub fn playthrough_ok(&self) -> bool {
        let usa_briefing_ok = self.usa_missions.first().is_some_and(|s| {
            s.map_name.to_ascii_lowercase().contains("md_usa01") && !s.movie_label.is_empty()
        });
        let usa_last_ok = self
            .usa_missions
            .last()
            .is_some_and(|s| s.map_name.to_ascii_lowercase().contains("md_usa05") && s.victorious);
        let gla_briefing_ok = self.gla_missions.first().is_some_and(|s| {
            s.map_name.to_ascii_lowercase().contains("md_gla01")
                && s.movie_label.to_ascii_lowercase().contains("md_gla01")
        });
        let gla_last_ok = self
            .gla_missions
            .last()
            .is_some_and(|s| s.map_name.to_ascii_lowercase().contains("md_gla05") && s.victorious);
        let china_briefing_ok = self.china_missions.first().is_some_and(|s| {
            s.map_name.to_ascii_lowercase().contains("md_chi01")
                && s.movie_label.to_ascii_lowercase().contains("md_china01")
        });
        let china_last_ok = self
            .china_missions
            .last()
            .is_some_and(|s| s.map_name.to_ascii_lowercase().contains("md_chi05") && s.victorious);
        let training_briefing_ok = self.training_missions.first().is_some_and(|s| {
            !s.location_name_label.is_empty()
                || !s.briefing_voice.is_empty()
                || !s.objective_line0.is_empty()
        });
        let challenge_ok = self.challenge_campaigns.len() == 9
            && self.challenge_campaigns.iter().all(|(_, steps)| {
                steps.len() >= 7
                    && steps.iter().all(|s| {
                        s.is_challenge
                            && s.map_name.to_ascii_lowercase().contains("gc_")
                            && !s.general_name.is_empty()
                            && !s.movie_label.is_empty()
                    })
            });
        let challenge_score_ok = self
            .challenge_campaigns
            .first()
            .and_then(|(_, steps)| steps.last())
            .is_some_and(|s| s.victorious && !s.final_victory_movie.is_empty());
        let generals_ok = self.challenge_general_starts.len() == 9
            && self.challenge_generals_unimplemented == 3
            && NUM_GENERALS == 12
            && self.challenge_general_starts.len() + self.challenge_generals_unimplemented
                == NUM_GENERALS;

        self.campaigns_loaded >= 12
            && self.usa_missions.len() == 5
            && self.gla_missions.len() == 5
            && self.china_missions.len() == 5
            && usa_briefing_ok
            && usa_last_ok
            && gla_briefing_ok
            && gla_last_ok
            && china_briefing_ok
            && china_last_ok
            && training_briefing_ok
            && self.usa_maps_on_disk >= 1
            && challenge_ok
            && challenge_score_ok
            && generals_ok
            && self
                .challenge_first
                .as_ref()
                .is_some_and(|s| s.is_challenge && !s.map_name.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usa_gla_china_and_all_challenge_generals_playthrough() {
        let report = run_retail_campaign_and_challenge_playthrough();
        assert!(
            report.playthrough_ok(),
            "campaign playthrough failed: campaigns={} usa={} gla={} china={} training={} usa_disk={} challenge_campaigns={} gen_starts={} unimplemented={} challenge={:?}",
            report.campaigns_loaded,
            report.usa_missions.len(),
            report.gla_missions.len(),
            report.china_missions.len(),
            report.training_missions.len(),
            report.usa_maps_on_disk,
            report.challenge_campaigns.len(),
            report.challenge_general_starts.len(),
            report.challenge_generals_unimplemented,
            report.challenge_first
        );
        assert!(
            report.usa_missions[0]
                .map_name
                .to_ascii_lowercase()
                .contains("md_usa01")
        );
        assert_eq!(
            campaign_map_stem(&report.usa_missions[0].map_name).to_ascii_lowercase(),
            "md_usa01"
        );
        assert!(!report.usa_missions[0].movie_label.is_empty());
        assert!(resolve_campaign_map_file(&report.usa_missions[0].map_name).is_some());
        // C++ ScoreScreen startNextCampaignGame uses CampaignManager::getCurrentMap
        // after gotoNextMission; USA chain is MD_USA01 → MD_USA02.
        let mut score_mgr = CampaignManager::new();
        score_mgr.init();
        score_mgr.set_campaign("USA");
        let first = score_mgr.get_current_map().unwrap_or_default();
        assert!(first.to_ascii_lowercase().contains("md_usa01"));
        score_mgr.set_victorious(true);
        assert!(score_mgr.goto_next_mission().is_some());
        let next = score_mgr.get_current_map().unwrap_or_default();
        assert!(
            next.to_ascii_lowercase().contains("md_usa02"),
            "score-screen continue must load next USA mission, got {next}"
        );
        let challenge = report.challenge_first.expect("challenge first mission");
        assert!(challenge.map_name.to_ascii_lowercase().contains("gc_"));
        assert_eq!(report.challenge_campaigns.len(), 9);
        assert_eq!(report.challenge_general_starts.len(), 9);
        assert_eq!(report.challenge_generals_unimplemented, 3);
        assert_eq!(
            report.challenge_general_starts.len() + report.challenge_generals_unimplemented,
            NUM_GENERALS
        );
        assert_eq!(NUM_GENERALS, 12);
        assert_eq!(report.gla_missions.len(), 5);
        assert!(
            report.gla_missions[0]
                .movie_label
                .to_ascii_lowercase()
                .contains("md_gla01")
        );
        assert!(report.gla_missions[4].victorious);
        assert_eq!(report.china_missions.len(), 5);
        assert!(
            report.china_missions[0]
                .movie_label
                .to_ascii_lowercase()
                .contains("md_china01")
        );
        assert!(report.china_missions[4].victorious);
        let challenge0_last = report.challenge_campaigns[0]
            .1
            .last()
            .expect("CHALLENGE_0 last mission");
        assert!(challenge0_last.victorious);
        assert!(
            challenge0_last
                .final_victory_movie
                .to_ascii_lowercase()
                .contains("victory")
        );
    }
}
