//! Single-authority simulation control for match runtime.
//!
//! Production path is always [`DualTickPolicy::AuthorityOnly`]. The dual crate
//! `GameLogic` tick machinery was removed (wave-1 no-legacy sweep); Main
//! GameLogic is the sole authority.

//! Wave 957: host_object/host_objects authority dual-read seal.
use crate::game_logic::GameLogic;

/// Snapshot of authoritative match state for probes and golden checkpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityProbe {
    pub frame: u32,
    pub object_count: usize,
    pub player_count: usize,
    pub local_supplies: u32,
    pub match_over: bool,
    pub victory_label: Option<String>,
}

impl AuthorityProbe {
    /// Capture probe without mutating victory evaluation (read-only fields only).
    pub fn capture(logic: &GameLogic, local_player_id: u32) -> Self {
        let local_supplies = logic
            .get_player(local_player_id)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        Self {
            frame: logic.get_frame(),
            object_count: logic.host_objects().registered_len(),
            player_count: logic.get_players().len(),
            local_supplies,
            match_over: false,
            victory_label: None,
        }
    }

    /// Capture the completed owner victory result without advancing simulation.
    pub fn capture_with_victory(logic: &GameLogic, local_player_id: u32) -> Self {
        let mut probe = Self::capture(logic, local_player_id);
        let victory = logic.current_victory_observation();
        probe.match_over = victory.match_over;
        probe.victory_label = victory.outcome.map(|v| format!("{v:?}"));
        probe
    }

    /// Stable-ish checkpoint fingerprint for golden-skirmish determinism checks.
    pub fn checkpoint_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.frame.hash(&mut h);
        self.object_count.hash(&mut h);
        self.player_count.hash(&mut h);
        self.local_supplies.hash(&mut h);
        self.match_over.hash(&mut h);
        self.victory_label.hash(&mut h);
        h.finish()
    }
}

/// Result of one authoritative logic-frame tick policy decision.
///
/// Only [`DualTickPolicy::AuthorityOnly`] exists: the vestigial dual-tick
/// variants and their env opt-in plumbing were deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualTickPolicy {
    /// Only Main GameLogic advances; gamelogic crate is not ticked.
    AuthorityOnly,
}

pub fn dual_tick_policy() -> DualTickPolicy {
    DualTickPolicy::AuthorityOnly
}

/// Advance Main GameLogic by `frames` and collect probes each frame (production path).
pub fn advance_authority_frames(
    logic: &mut GameLogic,
    local_player_id: u32,
    frames: u32,
) -> Vec<AuthorityProbe> {
    let mut out = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        logic.update();
        out.push(AuthorityProbe::capture(logic, local_player_id));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, GameMode, Player, Team};

    #[test]
    fn dual_tick_policy_is_always_single_authority() {
        assert_eq!(dual_tick_policy(), DualTickPolicy::AuthorityOnly);
    }

    #[test]
    fn probe_advances_with_main_game_logic_only() {
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        logic.clear_all_players();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let before = AuthorityProbe::capture(&logic, 0);
        let probes = advance_authority_frames(&mut logic, 0, 5);
        assert_eq!(probes.len(), 5);
        assert!(probes.last().unwrap().frame > before.frame);
        // Single world: object store is the Main GameLogic store.
        assert_eq!(
            probes.last().unwrap().player_count,
            logic.get_players().len()
        );
    }
}
