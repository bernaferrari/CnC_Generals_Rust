//! Host AI skirmish activity — production path for Medium AI non-idle proof.

use crate::ai::AIDifficulty;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct AiSkirmishActivityResult {
    pub config_applied: bool,
    pub ai_players: usize,
    pub frames_advanced: u32,
    pub activity_count: u64,
    pub ai_structures: usize,
    pub ai_units_or_queue: usize,
    pub difficulty: String,
    pub status: String,
}

fn ensure_human_templates(logic: &mut GameLogic) {
    for (name, kind, hp) in [
        ("HumanCC", KindOf::CommandCenter, 2000.0),
        ("HumanRanger", KindOf::Infantry, 120.0),
    ] {
        if logic.templates.contains_key(name) {
            continue;
        }
        let mut t = ThingTemplate::new(name);
        t.set_health(hp);
        t.set_cost(100, 0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(kind);
        logic.templates.insert(name.into(), t);
    }
}

/// Run Medium GLA AI through host update path and measure production-linked activity.
pub fn run_medium_ai_skirmish_activity(frames: u32) -> AiSkirmishActivityResult {
    let config = golden_skirmish_config("AIActivityMap");
    let mut logic = GameLogic::new();
    let config_applied = apply_skirmish_config(&mut logic, &config).is_ok();
    ensure_human_templates(&mut logic);

    // Human presence for enemy assessment.
    let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));
    let _ = logic.create_object("HumanRanger", Team::USA, Vec3::new(-90.0, 0.0, -90.0));

    // Seed constructed GLA factories so production can run once AI queues teams.
    // AI still must start additional builds via process_building_queue.
    logic.ensure_ai_faction_templates(Team::GLA);
    for (name, pos) in [
        ("GLA_Barracks", Vec3::new(200.0, 0.0, 200.0)),
        ("GLA_ArmsDealer", Vec3::new(230.0, 0.0, 200.0)),
    ] {
        if let Some(id) = logic.create_object(name, Team::GLA, pos) {
            if let Some(obj) = logic.get_object_mut(id) {
                obj.status.under_construction = false;
                obj.construction_percent = 1.0;
            }
        }
    }

    let ai_players = logic.host_ai_player_count();
    let difficulty = logic
        .get_ai_status(1)
        .unwrap_or_else(|| "missing".into());

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        logic.update();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);

    let activity_count = logic.host_ai_activity_count();
    let ai_structures = logic
        .get_objects()
        .values()
        .filter(|o| o.team == Team::GLA && o.is_kind_of(KindOf::Structure))
        .count();
    let ai_units_or_queue = logic
        .get_objects()
        .values()
        .filter(|o| {
            o.team == Team::GLA
                && (o.is_kind_of(KindOf::Infantry)
                    || o.is_kind_of(KindOf::Vehicle)
                    || o
                        .building_data
                        .as_ref()
                        .map(|b| !b.production_queue.is_empty())
                        .unwrap_or(false))
        })
        .count();

    // Multi-interval depth: require more than a single one-shot action.
    let multi_action = activity_count >= 2
        || ai_structures >= 3
        || (activity_count >= 1 && ai_units_or_queue >= 1);
    let status = if config_applied
        && ai_players >= 1
        && frames_advanced > 0
        && multi_action
    {
        "success".into()
    } else {
        "partial".into()
    };

    let _ = AIDifficulty::Medium; // keep import path live for difficulty enum use
    AiSkirmishActivityResult {
        config_applied,
        ai_players,
        frames_advanced,
        activity_count,
        ai_structures,
        ai_units_or_queue,
        difficulty,
        status,
    }
}

pub fn format_ai_activity_report(r: &AiSkirmishActivityResult) -> String {
    format!(
        "config_applied={} ai_players={} frames={} activity={} structures={} units_or_queue={} difficulty={} status={}",
        r.config_applied,
        r.ai_players,
        r.frames_advanced,
        r.activity_count,
        r.ai_structures,
        r.ai_units_or_queue,
        r.difficulty,
        r.status
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medium_ai_is_non_idle_on_host_update_path() {
        let result = run_medium_ai_skirmish_activity(120);
        assert!(result.config_applied, "skirmish config must apply");
        assert!(
            result.ai_players >= 1,
            "Medium AI slot must register: {}",
            result.difficulty
        );
        assert!(result.frames_advanced > 0);
        assert!(
            result.activity_count >= 2
                || result.ai_structures >= 3
                || (result.activity_count >= 1 && result.ai_units_or_queue >= 1),
            "AI must show multi-interval activity: {}",
            format_ai_activity_report(&result)
        );
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_activity_report(&result)
        );
    }

    #[test]
    fn medium_ai_activity_grows_across_update_windows() {
        let config = golden_skirmish_config("AIGrowthMap");
        let mut logic = GameLogic::new();
        assert!(apply_skirmish_config(&mut logic, &config).is_ok());
        ensure_human_templates(&mut logic);
        let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));
        logic.ensure_ai_faction_templates(Team::GLA);
        for _ in 0..30 {
            logic.update();
        }
        let after_first = logic.host_ai_activity_count();
        let structs_first = logic
            .get_objects()
            .values()
            .filter(|o| o.team == Team::GLA && o.is_kind_of(KindOf::Structure))
            .count();
        assert!(after_first >= 1, "first AI build interval must fire");
        // Continue host updates across more AI intervals (next_building_time reopens).
        for _ in 0..90 {
            logic.update();
        }
        let after_more = logic.host_ai_activity_count();
        let structs_more = logic
            .get_objects()
            .values()
            .filter(|o| o.team == Team::GLA && o.is_kind_of(KindOf::Structure))
            .count();
        assert!(
            after_more >= 2 || structs_more > structs_first,
            "multi-interval AI must deepen: act {after_first}->{after_more} structs {structs_first}->{structs_more}"
        );
    }
}
