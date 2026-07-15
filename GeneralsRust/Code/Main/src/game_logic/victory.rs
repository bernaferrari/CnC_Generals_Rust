use super::Team;
use std::time::Duration;

/// Outcome state for a player at the end of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerOutcome {
    Won,
    Lost,
    Draw,
}

/// Aggregated result information for a single player.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerResult {
    pub player_id: u32,
    pub player_name: String,
    pub faction: Team,
    pub units_built: u32,
    pub units_destroyed: u32,
    pub units_lost: u32,
    pub structures_built: u32,
    pub structures_destroyed: u32,
    pub structures_lost: u32,
    pub resources_collected: u32,
    pub resources_spent: u32,
    pub outcome: PlayerOutcome,
}

/// Overall victory summary that mirrors the C++ score screen payload.
#[derive(Debug, Clone, PartialEq)]
pub struct VictorySummary {
    pub mission_name: Option<String>,
    pub duration: Option<Duration>,
    pub player_results: Vec<PlayerResult>,
}

impl Default for VictorySummary {
    fn default() -> Self {
        Self::new()
    }
}

impl VictorySummary {
    pub fn new() -> Self {
        Self {
            mission_name: None,
            duration: None,
            player_results: Vec::new(),
        }
    }
}

pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryCondition {
    Winner(u32),
    Draw,
}
