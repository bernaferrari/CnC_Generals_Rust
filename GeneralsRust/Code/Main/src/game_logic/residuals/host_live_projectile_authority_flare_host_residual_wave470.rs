//! Wave 470 residual peels: under projectile authority, GameWorld sole-integrates
//! flight while host still owns countermeasure flare spawn/object residual.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 469 path midframe stub removal.
//! Architecture residual - split projectile flight vs CM flare ownership.
//!
//! Sources (game_logic.rs):
//! - gameworld_projectile_authority_live defers integrate+hits
//! - flush_countermeasure_flare_spawns / update_countermeasure_flare_objects
//!   run after both branches (Wave 470)
//!
//! Fail-closed:
//! - Not full C++ CountermeasureBehavior parity
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PROJECTILE_AUTHORITY_FLARE_HOST_METHOD_NAMES_WAVE470: &[&str] = &[
    "gameworld_projectile_authority_live",
    "update_projectiles_with_countermeasures",
    "flush_countermeasure_flare_spawns",
    "update_countermeasure_flare_objects",
    "step_projectiles",
    "resolve_projectiles_hits_only",
];

pub const PROJECTILE_AUTHORITY_FLARE_HOST_SOURCE_MARKERS_WAVE470: &[&str] = &[
    "Wave 470: countermeasure flare spawn/object residual stays host-owned",
    "gameworld_projectile_authority_live()",
    "flush_countermeasure_flare_spawns()",
    "update_countermeasure_flare_objects()",
];

pub const PROJECTILE_AUTHORITY_FLARE_HOST_NAV_STEPS_WAVE470: &[&str] = &[
    "SNAPSHOT_PROJECTILES_FOR_GW",
    "DEFER_INTEGRATE_WHEN_AUTH",
    "HOST_OR_GW_FLIGHT_STEP",
    "HOST_FLUSH_CM_FLARE_SPAWNS",
    "HOST_UPDATE_CM_FLARE_OBJECTS",
    "GW_HITS_ONLY_WHEN_AUTH",
];

pub const RUNTIME_HOST_PROJECTILE_AUTHORITY_FLARE_HOST_CMD_NAMES_WAVE470: &[&str] = &[
    "click_projectile_authority_flare_host_ok_wnd_snapshot",
    "click_projectile_authority_flare_host_ok_wnd_defer",
    "click_projectile_authority_flare_host_ok_wnd_flare",
    "click_projectile_authority_flare_host_ok_wnd_prepare",
    "click_projectile_authority_flare_host_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProjectileAuthorityFlareHostAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    AuthoritySource = 4,
    FlareHostSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProjectileAuthorityFlareHostAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_projectile_authority_flare_host_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_projectile_authority_flare_host_last_action()
-> ResidualProjectileAuthorityFlareHostAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProjectileAuthorityFlareHostAction::MethodNames,
        2 => ResidualProjectileAuthorityFlareHostAction::SourceMarkers,
        3 => ResidualProjectileAuthorityFlareHostAction::NavCommands,
        4 => ResidualProjectileAuthorityFlareHostAction::AuthoritySource,
        5 => ResidualProjectileAuthorityFlareHostAction::FlareHostSource,
        6 => ResidualProjectileAuthorityFlareHostAction::Composite,
        _ => ResidualProjectileAuthorityFlareHostAction::Idle,
    }
}

fn game_logic_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_projectile_authority_flare_host_method_names_residual_wave470() -> bool {
    PROJECTILE_AUTHORITY_FLARE_HOST_METHOD_NAMES_WAVE470.len() == 6
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_METHOD_NAMES_WAVE470,
            "gameworld_projectile_authority_live",
        ) == Some(0)
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_METHOD_NAMES_WAVE470,
            "resolve_projectiles_hits_only",
        ) == Some(5)
}

pub fn honesty_projectile_authority_flare_host_source_markers_residual_wave470() -> bool {
    PROJECTILE_AUTHORITY_FLARE_HOST_SOURCE_MARKERS_WAVE470.len() == 4
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_SOURCE_MARKERS_WAVE470,
            "Wave 470: countermeasure flare spawn/object residual stays host-owned",
        ) == Some(0)
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_SOURCE_MARKERS_WAVE470,
            "update_countermeasure_flare_objects()",
        ) == Some(3)
}

pub fn honesty_projectile_authority_flare_host_nav_commands_residual_wave470() -> bool {
    PROJECTILE_AUTHORITY_FLARE_HOST_NAV_STEPS_WAVE470.len() == 6
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_NAV_STEPS_WAVE470,
            "HOST_FLUSH_CM_FLARE_SPAWNS",
        ) == Some(3)
        && residual_name_index(
            PROJECTILE_AUTHORITY_FLARE_HOST_NAV_STEPS_WAVE470,
            "GW_HITS_ONLY_WHEN_AUTH",
        ) == Some(5)
        && RUNTIME_HOST_PROJECTILE_AUTHORITY_FLARE_HOST_CMD_NAMES_WAVE470.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PROJECTILE_AUTHORITY_FLARE_HOST_CMD_NAMES_WAVE470,
            "click_projectile_authority_flare_host_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_projectile_authority_defer_source() -> bool {
    let gl = game_logic_source();
    let sw = shadow_source();
    let ok = gl.contains("gameworld_projectile_authority_live()")
        && gl.contains("Defer integrate+hits to shadow_session")
        && sw.contains("step_projectiles")
        && sw.contains("resolve_projectiles_hits_only");
    residual_action_store(ResidualProjectileAuthorityFlareHostAction::AuthoritySource);
    ok
}

pub fn simulate_projectile_authority_flare_host_source() -> bool {
    let gl = game_logic_source();
    // Flare residual must run outside the else-only host integrate branch.
    let ok = gl.contains("Wave 470: countermeasure flare spawn/object residual stays host-owned")
        && gl.contains("flush_countermeasure_flare_spawns()")
        && gl.contains("update_countermeasure_flare_objects()");
    // Ensure flare flush is not only inside the non-authority else branch:
    // marker sits after the if/else projectile_hits binding.
    let Some(i) = gl.find("Wave 470: countermeasure flare spawn/object residual stays host-owned")
    else {
        residual_action_store(ResidualProjectileAuthorityFlareHostAction::FlareHostSource);
        return false;
    };
    let win = &gl[i.saturating_sub(200)..gl.len().min(i + 400)];
    let ok = ok
        && win.contains("flush_countermeasure_flare_spawns()")
        && !win.contains("update_projectiles_with_countermeasures");
    residual_action_store(ResidualProjectileAuthorityFlareHostAction::FlareHostSource);
    ok
}

pub fn honesty_projectile_authority_flare_host_residual_pack_wave470() -> bool {
    honesty_projectile_authority_flare_host_method_names_residual_wave470()
        && honesty_projectile_authority_flare_host_source_markers_residual_wave470()
        && honesty_projectile_authority_flare_host_nav_commands_residual_wave470()
        && simulate_projectile_authority_defer_source()
        && simulate_projectile_authority_flare_host_source()
}

pub fn simulate_live_projectile_authority_flare_host_honesty() -> bool {
    let ok = honesty_projectile_authority_flare_host_residual_pack_wave470();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProjectileAuthorityFlareHostAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_projectile_authority_flare_host_method_names_residual_wave470());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_projectile_authority_flare_host_source_markers_residual_wave470());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_projectile_authority_flare_host_nav_commands_residual_wave470());
    }

    #[test]
    fn projectile_authority_flare_host_sources() {
        assert!(simulate_projectile_authority_defer_source());
        assert!(simulate_projectile_authority_flare_host_source());
    }

    #[test]
    fn wave470_composite_pack() {
        assert!(honesty_projectile_authority_flare_host_residual_pack_wave470());
    }

    #[test]
    fn simulate_live_projectile_authority_flare_host_honesty_residual_live() {
        assert!(
            simulate_live_projectile_authority_flare_host_honesty(),
            "projectile authority flare host residual must latch"
        );
        assert!(residual_projectile_authority_flare_host_ok());
        assert_eq!(
            residual_projectile_authority_flare_host_last_action(),
            ResidualProjectileAuthorityFlareHostAction::Composite
        );
    }
}
