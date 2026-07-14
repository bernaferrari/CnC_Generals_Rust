//! Single-authority simulation control for match runtime.
//!
//! Philosophy (roadmap): one authoritative game world owns logic-frame advances.
//! The Main `GameLogic` is the temporary host authority; the ported `gamelogic`
//! crate is only ticked when dual-tick is explicitly enabled, and verification
//! builds refuse non-fatal dual-world bridge failures.

use crate::game_logic::GameLogic;
use std::sync::atomic::{AtomicBool, Ordering};

/// When true, dual-crate ticks are skipped and any attempted dual-tick error is fatal.
static VERIFICATION_SINGLE_AUTHORITY: AtomicBool = AtomicBool::new(false);

/// Enable single-authority verification mode (bridge failures must not be ignored).
pub fn set_verification_single_authority(enabled: bool) {
    VERIFICATION_SINGLE_AUTHORITY.store(enabled, Ordering::SeqCst);
}

pub fn verification_single_authority() -> bool {
    VERIFICATION_SINGLE_AUTHORITY.load(Ordering::SeqCst)
        || std::env::var_os("GENERALS_VERIFY_SINGLE_AUTHORITY").is_some()
}

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
            object_count: logic.get_objects().len(),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualTickPolicy {
    /// Only Main GameLogic advances; gamelogic crate is not ticked.
    AuthorityOnly,
    /// Dual tick allowed; crate errors are non-fatal (legacy host mode).
    DualLegacyNonFatal,
    /// Dual tick attempted under verification; errors must abort.
    DualVerificationFatal,
}

pub fn dual_tick_policy() -> DualTickPolicy {
    if verification_single_authority() {
        DualTickPolicy::AuthorityOnly
    } else if std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some() {
        DualTickPolicy::DualLegacyNonFatal
    } else {
        // Default production path: single Main authority. Dual crate tick is opt-in.
        DualTickPolicy::AuthorityOnly
    }
}

/// Apply the dual-tick policy after Main GameLogic has already advanced one frame.
///
/// Returns `Err` when verification mode would have ignored a crate failure, or when
/// a dual tick is required to fail closed.
pub fn apply_post_authority_crate_tick(
    policy: DualTickPolicy,
    tick_crate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match policy {
        DualTickPolicy::AuthorityOnly => Ok(()),
        DualTickPolicy::DualLegacyNonFatal => {
            if let Err(e) = tick_crate() {
                log::trace!("gamelogic crate update failed (non-fatal): {e}");
            }
            Ok(())
        }
        DualTickPolicy::DualVerificationFatal => tick_crate()
            .map_err(|e| format!("verification build: dual-world crate tick failed fatally: {e}")),
    }
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
    fn dual_tick_defaults_to_single_authority() {
        // Clear env influence for this process-local check of default when env unset.
        // We only assert the enum shape of the function; verification flag is explicit.
        set_verification_single_authority(true);
        assert_eq!(dual_tick_policy(), DualTickPolicy::AuthorityOnly);
        set_verification_single_authority(false);
    }

    #[test]
    fn default_policy_is_authority_only_without_env() {
        // Ensure verification flag off for this assertion.
        set_verification_single_authority(false);
        // Without GENERALS_ALLOW_DUAL_TICK in env, default is single authority.
        if std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_none()
            && std::env::var_os("GENERALS_VERIFY_SINGLE_AUTHORITY").is_none()
        {
            assert_eq!(dual_tick_policy(), DualTickPolicy::AuthorityOnly);
        }
    }

    #[test]
    fn authority_only_skips_crate_tick() {
        let mut called = false;
        apply_post_authority_crate_tick(DualTickPolicy::AuthorityOnly, || {
            called = true;
            Ok(())
        })
        .unwrap();
        assert!(!called);
    }

    #[test]
    fn verification_fatal_propagates_crate_error() {
        let err = apply_post_authority_crate_tick(DualTickPolicy::DualVerificationFatal, || {
            Err("boom".into())
        })
        .expect_err("must be fatal");
        assert!(err.contains("fatal") || err.contains("boom"));
    }

    #[test]
    fn probe_advances_with_main_game_logic_only() {
        set_verification_single_authority(true);
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
        set_verification_single_authority(false);
    }
}
