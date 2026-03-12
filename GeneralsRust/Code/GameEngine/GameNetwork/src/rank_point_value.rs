//! Rank point value utilities (C++ RankPointValue.h parity).

use crate::gamespy::config::GameSpyConfig;
use crate::gamespy::persistent_storage_thread::PSPlayerStats;
use game_engine::common::skirmish_battle_honors::{
    BATTLE_HONOR_CAMPAIGN_CHINA, BATTLE_HONOR_CAMPAIGN_GLA, BATTLE_HONOR_CAMPAIGN_USA,
};
use std::sync::{Arc, OnceLock, RwLock};

pub const MAX_RANKS: usize = 10;

pub const RANK_PRIVATE: usize = 0;
pub const RANK_CORPORAL: usize = 1;
pub const RANK_SERGEANT: usize = 2;
pub const RANK_LIEUTENANT: usize = 3;
pub const RANK_CAPTAIN: usize = 4;
pub const RANK_MAJOR: usize = 5;
pub const RANK_COLONEL: usize = 6;
pub const RANK_BRIGADIER_GENERAL: usize = 7;
pub const RANK_GENERAL: usize = 8;
pub const RANK_COMMANDER_IN_CHIEF: usize = 9;

#[derive(Debug, Clone)]
pub struct RankPoints {
    pub ranks: [i32; MAX_RANKS],
    pub win_multiplier: f32,
    pub lost_multiplier: f32,
    pub hour_spent_online_multiplier: f32,
    pub completed_solo_campaigns: f32,
    pub disconnect_multiplier: f32,
}

impl Default for RankPoints {
    fn default() -> Self {
        let config = GameSpyConfig::new_sync();
        let mut ranks = [0; MAX_RANKS];
        for idx in 0..MAX_RANKS {
            ranks[idx] = config.get_points_for_rank(idx as i32);
        }
        Self {
            ranks,
            win_multiplier: 3.0,
            lost_multiplier: 0.0,
            hour_spent_online_multiplier: 1.0,
            completed_solo_campaigns: 5.0,
            disconnect_multiplier: -1.0,
        }
    }
}

static THE_RANK_POINTS: OnceLock<Arc<RwLock<RankPoints>>> = OnceLock::new();

pub fn get_rank_point_values() -> Arc<RwLock<RankPoints>> {
    THE_RANK_POINTS
        .get_or_init(|| Arc::new(RwLock::new(RankPoints::default())))
        .clone()
}

pub fn calculate_rank(stats: &PSPlayerStats) -> i32 {
    if stats.id == 0 {
        return 0;
    }
    let rank_values = get_rank_point_values();
    let guard = rank_values.read().ok();
    let Some(values) = guard.as_ref() else {
        return 0;
    };

    let mut rank_points = 0.0_f32;

    let wins: i32 = stats.wins.values().map(|v| *v as i32).sum();
    rank_points += wins as f32 * values.win_multiplier;

    let losses: i32 = stats.losses.values().map(|v| *v as i32).sum();
    rank_points += losses as f32 * values.lost_multiplier;

    let duration_minutes: i32 = stats.duration.values().map(|v| *v as i32).sum();
    rank_points += (duration_minutes / 60) as f32 * values.hour_spent_online_multiplier;

    let mut disconnects: i32 = stats.discons.values().map(|v| *v as i32).sum();
    disconnects += stats.desyncs.values().map(|v| *v as i32).sum::<i32>();
    rank_points += disconnects as f32 * values.disconnect_multiplier;

    let campaign_mask = (BATTLE_HONOR_CAMPAIGN_USA
        | BATTLE_HONOR_CAMPAIGN_CHINA
        | BATTLE_HONOR_CAMPAIGN_GLA) as i32;
    if (stats.battle_honors & campaign_mask) != 0 {
        rank_points += values.completed_solo_campaigns;
    }

    rank_points.max(0.0).round() as i32
}

pub fn get_favorite_side(stats: &PSPlayerStats) -> i32 {
    let mut most_games = 0u32;
    let mut favorite = -1;
    for (side, games) in &stats.games {
        if *games >= most_games {
            most_games = *games;
            favorite = *side;
        }
    }
    if most_games == 0 {
        return -1;
    }
    if stats.games_as_random as u32 >= most_games {
        return 0;
    }
    favorite
}
