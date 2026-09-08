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
            object_count: logic.host_objects().len(),
            player_count: logic.get_players().len(),
            local_supplies,
            match_over: false,
            victory_label: None,
        }
    }

    /// Capture probe after evaluating victory on the authoritative world (mutating).
    pub fn capture_with_victory(logic: &mut GameLogic, local_player_id: u32) -> Self {
        let mut probe = Self::capture(logic, local_player_id);
        if let Some(v) = logic.evaluate_victory_condition() {
            probe.match_over = true;
            probe.victory_label = Some(format!("{v:?}"));
        }
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
    fn dual_tick_env_plumbing_is_deleted() {
        let aw = include_str!("authoritative_world.rs");
        let prod = aw.split("#[cfg(test)]").next().expect("prod");
        assert!(
            !prod.contains("GENERALS_ALLOW_DUAL_TICK")
                && !prod.contains("GENERALS_VERIFY_SINGLE_AUTHORITY")
                && !prod.contains("fn set_verification_single_authority")
                && !prod.contains("fn apply_post_authority_crate_tick"),
            "dual-tick env gating and apply helper were removed with the dual crate tick"
        );
    }

    #[test]
    fn production_dual_tick_policy_is_authority_only_and_gameworld_shadow_on() {
        assert!(matches!(dual_tick_policy(), DualTickPolicy::AuthorityOnly));
        assert!(
            crate::gameworld_shadow::gameworld_shadow_enabled(),
            "GameWorld shadow is production-on (opt out GENERALS_GAMEWORLD_SHADOW=0)"
        );
        assert!(
            !crate::gameworld_shadow::gameworld_movement_authority_enabled()
                && !crate::gameworld_shadow::gameworld_damage_authority_enabled()
                && !crate::gameworld_shadow::gameworld_economy_authority_enabled()
                && !crate::gameworld_shadow::gameworld_production_authority_enabled(),
            "last-writer authorities default off — host GameLogic is the sole writer"
        );
        let aw = include_str!("authoritative_world.rs");
        let prod = aw.split("#[cfg(test)]").next().expect("prod");
        assert!(prod.contains("DualTickPolicy::AuthorityOnly"));
        assert!(
            !prod.contains("GENERALS_ALLOW_DUAL_TICK"),
            "dual crate tick path is removed"
        );
        let shadow = include_str!("gameworld_shadow/mod.rs");
        assert!(shadow.contains("GENERALS_GAMEWORLD_SHADOW"));
        assert!(
            shadow.contains("Last-writer") || shadow.contains("sole writer"),
            "shadow docs must state host sole writer"
        );
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
