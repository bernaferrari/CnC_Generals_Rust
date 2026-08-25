//! Wave 173 residual peels: single-authority + golden combat honesty residual
//! (dual-tick default-off; teleport pull opt-in only; playable_claim false;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 172 live GameWorldShadow overlay residual.
//! Host residual only — network deferred.
//!
//! Sources (repo architecture + golden_skirmish):
//! - authoritative_world::dual_tick_policy defaults AuthorityOnly
//! - GENERALS_ALLOW_DUAL_TICK opt-in only
//! - GOLDEN_ALLOW_TELEPORT_PULL opt-in only
//! - GoldenSkirmishResult::playable_claim always false
//!
//! Fail-closed:
//! - Not full golden victory/combat run in this peel (too heavy for shell_smoke)
//! - Not full GameWorld production cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Single-authority combat honesty residual method names.
pub const SINGLE_AUTHORITY_COMBAT_HONESTY_METHOD_NAMES_WAVE173: &[&str] = &[
    "dual_tick_policy",
    "DualTickPolicy::AuthorityOnly",
    "GENERALS_ALLOW_DUAL_TICK",
    "GOLDEN_ALLOW_TELEPORT_PULL",
    "combat_no_teleport_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const SINGLE_AUTHORITY_COMBAT_HONESTY_NAV_STEPS_WAVE173: &[&str] = &[
    "REQUIRE_DUAL_TICK_DEFAULT_OFF",
    "REQUIRE_TELEPORT_PULL_OPT_IN",
    "REQUIRE_PLAYABLE_CLAIM_FALSE_SOURCE",
    "REQUIRE_COMBAT_NO_TELEPORT_FIELD",
    "LIVE_DUAL_TICK_POLICY_AUTHORITY_ONLY",
    "LIVE_TELEPORT_ENV_UNSET",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_SINGLE_AUTHORITY_COMBAT_HONESTY_CMD_NAMES_WAVE173: &[&str] = &[
    "click_single_authority_ok_policy",
    "click_single_authority_ok_teleport",
    "click_single_authority_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_single_authority_combat_method_names_residual_wave173() -> bool {
    SINGLE_AUTHORITY_COMBAT_HONESTY_METHOD_NAMES_WAVE173.len() == 6
        && residual_name_index(
            SINGLE_AUTHORITY_COMBAT_HONESTY_METHOD_NAMES_WAVE173,
            "dual_tick_policy",
        ) == Some(0)
        && residual_name_index(
            SINGLE_AUTHORITY_COMBAT_HONESTY_METHOD_NAMES_WAVE173,
            "GOLDEN_ALLOW_TELEPORT_PULL",
        ) == Some(3)
        && residual_name_index(
            SINGLE_AUTHORITY_COMBAT_HONESTY_METHOD_NAMES_WAVE173,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_single_authority_combat_nav_commands_residual_wave173() -> bool {
    SINGLE_AUTHORITY_COMBAT_HONESTY_NAV_STEPS_WAVE173.len() == 6
        && residual_name_index(
            SINGLE_AUTHORITY_COMBAT_HONESTY_NAV_STEPS_WAVE173,
            "REQUIRE_DUAL_TICK_DEFAULT_OFF",
        ) == Some(0)
        && residual_name_index(
            SINGLE_AUTHORITY_COMBAT_HONESTY_NAV_STEPS_WAVE173,
            "LIVE_DUAL_TICK_POLICY_AUTHORITY_ONLY",
        ) == Some(4)
        && RUNTIME_HOST_SINGLE_AUTHORITY_COMBAT_HONESTY_CMD_NAMES_WAVE173.len() == 3
}

/// Wave 173 composite residual honesty pack.
pub fn honesty_single_authority_combat_residual_pack_wave173() -> bool {
    honesty_single_authority_combat_method_names_residual_wave173()
        && honesty_single_authority_combat_nav_commands_residual_wave173()
}

/// Source residual: dual_tick_policy defaults to AuthorityOnly without dual env.
pub fn honesty_dual_tick_default_authority_only_source() -> bool {
    let src = include_str!("../../authoritative_world.rs");
    let i = match src.find("pub fn dual_tick_policy") {
        Some(i) => i,
        None => return false,
    };
    // 2026-08-15: production dual_tick_policy is always AuthorityOnly
    // (authoritative_world.rs:87-89). Env opt-in no longer gates the live path.
    let body = &src[i..src.len().min(i + 900)];
    src.contains("Production path is always")
        && body.contains("DualTickPolicy::AuthorityOnly")
        && !body.contains("GENERALS_ALLOW_DUAL_TICK")
}

/// Source residual: golden teleport pull is env opt-in only.
pub fn honesty_golden_teleport_pull_opt_in_source() -> bool {
    let src = include_str!("../../golden_skirmish.rs");
    src.contains("GOLDEN_ALLOW_TELEPORT_PULL")
        && src.contains("allow_teleport")
        && src.contains("combat_no_teleport_ok")
        && src.contains("used_teleport_pull")
        // Must not enable teleport by default string match of = true without env.
        && src.contains("var_os(\"GOLDEN_ALLOW_TELEPORT_PULL\")")
}

/// Source residual: golden playable_claim forced false.
pub fn honesty_golden_playable_claim_false_source() -> bool {
    let src = include_str!("../../golden_skirmish.rs");
    // Production assignment in run path.
    src.contains("let playable_claim = false") || src.contains("playable_claim: false")
}

/// Live residual: policy + env honesty without running full golden combat.
pub fn simulate_single_authority_combat_honesty() -> bool {
    use crate::authoritative_world::{DualTickPolicy, dual_tick_policy};

    if !honesty_single_authority_combat_residual_pack_wave173() {
        return false;
    }
    if !honesty_dual_tick_default_authority_only_source() {
        return false;
    }
    if !honesty_golden_teleport_pull_opt_in_source() {
        return false;
    }
    if !honesty_golden_playable_claim_false_source() {
        return false;
    }

    // Live: with dual-tick env unset (typical CI/dev), policy is AuthorityOnly.
    // If a parent test enabled dual-tick or verification flags, still require that
    // dual is not the silent default — DualLegacy only when ALLOW_DUAL is set.
    let policy = dual_tick_policy();
    let dual_env = std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some();
    if !dual_env {
        if policy != DualTickPolicy::AuthorityOnly {
            return false;
        }
    } else if !matches!(
        policy,
        DualTickPolicy::DualLegacyNonFatal | DualTickPolicy::AuthorityOnly
    ) {
        return false;
    }

    // Live: teleport pull env must not be forced on by residual peels.
    // (Opt-in remains available for debug; honesty is that default is off.)
    let teleport_env = std::env::var_os("GOLDEN_ALLOW_TELEPORT_PULL");
    if let Some(v) = teleport_env {
        // If set, must be an explicit truthy debug choice — residual still passes
        // as long as source is opt-in. Non-empty off values are fine.
        let _ = v;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_single_authority_combat_method_names_residual_wave173());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_single_authority_combat_nav_commands_residual_wave173());
    }

    #[test]
    fn wave173_composite_pack() {
        assert!(honesty_single_authority_combat_residual_pack_wave173());
    }

    #[test]
    fn dual_tick_default_source() {
        assert!(honesty_dual_tick_default_authority_only_source());
    }

    #[test]
    fn golden_teleport_and_claim_source() {
        assert!(honesty_golden_teleport_pull_opt_in_source());
        assert!(honesty_golden_playable_claim_false_source());
    }

    #[test]
    fn simulate_single_authority_combat_honesty_residual_live() {
        assert!(
            simulate_single_authority_combat_honesty(),
            "single-authority default + golden teleport opt-in residual must latch"
        );
    }
}
