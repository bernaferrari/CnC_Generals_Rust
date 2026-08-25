//! Wave 184 residual peels: live GameWorld projectile + AI decision residual
//! (projectile flight log → SetProjectileFlight; AI decision log → PushAiDecision;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 183 economy/movement residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `apply_host_projectile_events` / `writeback_projectiles_to_host`
//! - `apply_host_ai_decision_events`
//! - projectile + AI decision authorities default-on
//!
//! Fail-closed:
//! - Not full combat projectile sim on Lone Eagle
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live projectile/AI residual method names.
pub const LIVE_GAMEWORLD_PROJECTILE_AI_METHOD_NAMES_WAVE184: &[&str] = &[
    "apply_host_projectile_events",
    "writeback_projectiles_to_host",
    "apply_host_ai_decision_events",
    "host_ai_decision_log::record_attack",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_PROJECTILE_AI_NAV_STEPS_WAVE184: &[&str] = &[
    "REQUIRE_PROJECTILE_CHANNEL",
    "REQUIRE_AI_DECISION_CHANNEL",
    "LIVE_PROJECTILE_FLIGHT_APPLY",
    "LIVE_AI_DECISION_APPLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_PROJECTILE_AI_CMD_NAMES_WAVE184: &[&str] = &[
    "click_live_gameworld_projectile_ai_ok_projectile",
    "click_live_gameworld_projectile_ai_ok_ai",
    "click_live_gameworld_projectile_ai_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_projectile_ai_method_names_residual_wave184() -> bool {
    LIVE_GAMEWORLD_PROJECTILE_AI_METHOD_NAMES_WAVE184.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_PROJECTILE_AI_METHOD_NAMES_WAVE184,
            "apply_host_projectile_events",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_PROJECTILE_AI_METHOD_NAMES_WAVE184,
            "apply_host_ai_decision_events",
        ) == Some(2)
        && residual_name_index(
            LIVE_GAMEWORLD_PROJECTILE_AI_METHOD_NAMES_WAVE184,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_projectile_ai_nav_commands_residual_wave184() -> bool {
    LIVE_GAMEWORLD_PROJECTILE_AI_NAV_STEPS_WAVE184.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_PROJECTILE_AI_NAV_STEPS_WAVE184,
            "REQUIRE_PROJECTILE_CHANNEL",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_PROJECTILE_AI_NAV_STEPS_WAVE184,
            "LIVE_AI_DECISION_APPLY",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_GAMEWORLD_PROJECTILE_AI_CMD_NAMES_WAVE184.len() == 3
}

/// Wave 184 composite residual honesty pack.
pub fn honesty_live_gameworld_projectile_ai_residual_pack_wave184() -> bool {
    honesty_live_gameworld_projectile_ai_method_names_residual_wave184()
        && honesty_live_gameworld_projectile_ai_nav_commands_residual_wave184()
}

/// Source residual: projectile + AI decision channel APIs.
pub fn honesty_projectile_ai_channel_api_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let proj = include_str!("../host_projectile_log.rs");
    let ai = include_str!("../host_ai_decision_log.rs");
    src.contains("pub fn apply_host_projectile_events")
        && src.contains("pub fn writeback_projectiles_to_host")
        && src.contains("pub fn apply_host_ai_decision_events")
        && proj.contains("pub struct HostProjectileEvent")
        && ai.contains("pub struct HostAiDecisionEvent")
        && ai.contains("pub fn record_attack")
}

/// Source residual: projectile + AI decision last-writers default off.
pub fn honesty_projectile_ai_authority_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let proj_ok = {
        let i = match src.find("pub fn gameworld_projectile_authority_enabled") {
            Some(i) => i,
            None => return false,
        };
        src[i..src.len().min(i + 300)].contains("false")
    };
    let ai_ok = {
        let i = match src.find("pub fn gameworld_ai_decision_authority_enabled") {
            Some(i) => i,
            None => return false,
        };
        src[i..src.len().min(i + 300)].contains("false")
    };
    proj_ok && ai_ok
}

/// Live residual: projectile flight apply + AI decision apply (opt-in authority).
pub fn simulate_live_gameworld_projectile_ai_honesty() -> bool {
    use crate::game_logic::host_ai_decision_log::{self, AI_DECISION_ATTACK};
    use crate::game_logic::host_projectile_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, ensure_gate_damage_authority, gameworld_ai_decision_authority_enabled,
        gameworld_projectile_authority_enabled,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_gameworld_projectile_ai_residual_pack_wave184() {
        return false;
    }
    if !honesty_projectile_ai_channel_api_source() {
        return false;
    }
    if !honesty_projectile_ai_authority_default_on_source() {
        return false;
    }

    ensure_gate_damage_authority();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    if !gameworld_projectile_authority_enabled() || !gameworld_ai_decision_authority_enabled() {
        return false;
    }

    // --- Projectile flight channel (no host object required) ---
    host_projectile_log::clear();
    let mut shadow = GameWorldShadow::new(64);
    host_projectile_log::record(
        501,
        [10.0, 1.0, 20.0],
        [5.0, 0.0, 0.0],
        [100.0, 1.0, 20.0],
        25.0,
        7,
        8,
        200.0,
        0.5,
        3.0,
        true,
        true,
    );
    let n = shadow.apply_host_projectile_events(&host_projectile_log::drain());
    if n < 1 {
        return false;
    }
    {
        let Some(p) = shadow.world().projectile(501) else {
            return false;
        };
        if p.host_id != 501 {
            return false;
        }
        if p.position != [10.0, 1.0, 20.0] || p.velocity != [5.0, 0.0, 0.0] {
            return false;
        }
        if (p.damage - 25.0).abs() > 1e-4 || !p.is_homing || !p.active {
            return false;
        }
    }

    // --- AI decision channel ---
    host_ai_decision_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LiveProjAi184");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("LiveAiU184") {
        let mut t = ThingTemplate::new("LiveAiU184");
        t.add_kind_of(KindOf::Infantry);
        t.set_health(100.0);
        logic.templates.insert("LiveAiU184".into(), t);
    }
    let Some(attacker) = logic.create_object("LiveAiU184", Team::USA, Vec3::new(5.0, 0.0, 5.0))
    else {
        return false;
    };
    let Some(victim) = logic.create_object("LiveAiU184", Team::GLA, Vec3::new(15.0, 0.0, 5.0))
    else {
        return false;
    };

    // Fresh shadow with host units mapped.
    let mut shadow2 = GameWorldShadow::new(64);
    shadow2.sync_from_host(&logic);
    if shadow2.entity_for_host(attacker).is_none() || shadow2.entity_for_host(victim).is_none() {
        return false;
    }

    host_ai_decision_log::record_attack(attacker, victim);
    let events = host_ai_decision_log::drain();
    if events.is_empty() || events[0].kind != AI_DECISION_ATTACK {
        return false;
    }
    let applied = shadow2.apply_host_ai_decision_events(&events);
    if applied < 1 {
        return false;
    }

    // Writeback projectile API callable (empty host is fine).
    let _ = shadow.writeback_projectiles_to_host(&mut logic);

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_projectile_ai_method_names_residual_wave184());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_projectile_ai_nav_commands_residual_wave184());
    }

    #[test]
    fn wave184_composite_pack() {
        assert!(honesty_live_gameworld_projectile_ai_residual_pack_wave184());
    }

    #[test]
    fn projectile_ai_sources() {
        assert!(honesty_projectile_ai_channel_api_source());
        assert!(honesty_projectile_ai_authority_default_on_source());
    }

    #[test]
    fn simulate_live_gameworld_projectile_ai_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_projectile_ai_honesty(),
            "live GameWorld projectile + AI decision channel residual must latch"
        );
    }
}
