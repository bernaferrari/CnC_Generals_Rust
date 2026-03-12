//! Player System - Placeholder implementation
//!
//! Core player class managing all player-specific data and behavior.
//! This would be one of the most complex classes in the system.

use crate::common::rts::{
    AcademyStats, Energy, Handicap, MissionStats, Money, PlayerHandle, ScienceType, ScoreKeeper,
    SCIENCE_INVALID,
};
use std::collections::HashSet;
/// Player structure - central hub for player data
#[derive(Debug)]
pub struct Player {
    /// Player index
    index: i32,
    /// Player name
    name: String,
    /// Money/resource management
    money: Money,
    /// Energy production/consumption
    energy: Energy,
    /// Academy statistics for advice
    academy_stats: AcademyStats,
    /// Mission statistics
    mission_stats: MissionStats,
    /// Handicap modifiers
    handicap: Handicap,
    /// Score keeping
    score_keeper: ScoreKeeper,
    /// Sciences currently owned by the player
    sciences: HashSet<ScienceType>,
    /// Sciences that are currently disabled (cannot be used)
    sciences_disabled: HashSet<ScienceType>,
    /// Sciences hidden from UI until unlocked
    sciences_hidden: HashSet<ScienceType>,
    /// Science purchase points available
    science_purchase_points: i32,
}

impl Player {
    pub fn new(index: i32) -> Self {
        let mut player = Self {
            index,
            name: String::new(),
            money: Money::new(),
            energy: Energy::new(),
            academy_stats: AcademyStats::new(),
            mission_stats: MissionStats::new(),
            handicap: Handicap::new(),
            score_keeper: ScoreKeeper::new(),
            sciences: HashSet::new(),
            sciences_disabled: HashSet::new(),
            sciences_hidden: HashSet::new(),
            science_purchase_points: 0,
        };
        let handle = PlayerHandle::new(index.max(0) as u32);
        player.energy.init(handle);
        player.academy_stats.init(handle);
        player.score_keeper.reset(index);
        player
    }

    // Accessors
    pub fn get_player_index(&self) -> i32 {
        self.index
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_money(&self) -> &Money {
        &self.money
    }
    pub fn get_money_mut(&mut self) -> &mut Money {
        &mut self.money
    }
    pub fn get_energy(&self) -> &Energy {
        &self.energy
    }
    pub fn get_energy_mut(&mut self) -> &mut Energy {
        &mut self.energy
    }
    pub fn get_academy_stats(&self) -> &AcademyStats {
        &self.academy_stats
    }
    pub fn get_academy_stats_mut(&mut self) -> &mut AcademyStats {
        &mut self.academy_stats
    }
    pub fn get_mission_stats(&self) -> &MissionStats {
        &self.mission_stats
    }
    pub fn get_mission_stats_mut(&mut self) -> &mut MissionStats {
        &mut self.mission_stats
    }
    pub fn get_handicap(&self) -> &Handicap {
        &self.handicap
    }
    pub fn get_score_keeper(&self) -> &ScoreKeeper {
        &self.score_keeper
    }
    pub fn get_score_keeper_mut(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    /// Initialize player
    pub fn init(&mut self, name: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        self.money.init();
        let handle = PlayerHandle::new(self.index.max(0) as u32);
        self.energy.init(handle);
        self.academy_stats.init(handle);
        self.handicap.init();
        self.mission_stats.init();
        self.sciences.clear();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();
        self.science_purchase_points = 0;
    }

    /// Update player (called each frame)
    pub fn update(&mut self) {
        self.academy_stats.update();
    }

    /// Check whether this player already owns the specified science
    pub fn has_science(&self, science: ScienceType) -> bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }

    /// Grant a science to the player
    pub fn grant_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences.insert(science);
    }

    /// Disable a science (remains known but unusable)
    pub fn disable_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences_disabled.insert(science);
    }

    /// Hide a science (used by UI gating, retains knowledge state)
    pub fn hide_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.insert(science);
    }

    /// Check if a science is disabled
    pub fn is_science_disabled(&self, science: ScienceType) -> bool {
        self.sciences_disabled.contains(&science)
    }

    /// Check if a science is hidden
    pub fn is_science_hidden(&self, science: ScienceType) -> bool {
        self.sciences_hidden.contains(&science)
    }

    /// Adjust the player's science purchase points
    pub fn add_science_purchase_points(&mut self, delta: i32) {
        self.science_purchase_points += delta;
    }

    /// Current purchase points
    pub fn get_science_purchase_points(&self) -> i32 {
        self.science_purchase_points
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new(0)
    }
}

impl super::science::ScienceAccess for Player {
    fn has_science(&self, science: ScienceType) -> bool {
        Player::has_science(self, science)
    }
}
