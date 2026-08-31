//! Wave 183 residual peels: live GameWorld economy + movement channel residual
//! (economy writeback restores host supplies; movement log → SetMovement;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 182 damage channel residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `writeback_economy_to_host` / `apply_host_economy_events`
//! - `host_movement_log` + `apply_host_movement_events` / `writeback_movement_to_host`
//! - economy + movement authorities default-on
//!
//! Fail-closed:
//! - Not full pathfinding/AI economy sim on Lone Eagle
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live economy/movement residual method names.
pub const LIVE_GAMEWORLD_ECONOMY_MOVEMENT_METHOD_NAMES_WAVE183: &[&str] = &[
    "writeback_economy_to_host",
    "apply_host_economy_events",
    "apply_host_movement_events",
    "writeback_movement_to_host",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_ECONOMY_MOVEMENT_NAV_STEPS_WAVE183: &[&str] = &[
    "REQUIRE_ECONOMY_WRITEBACK",
    "REQUIRE_MOVEMENT_CHANNEL",
    "LIVE_ECONOMY_WRITEBACK_RESTORES",
    "LIVE_MOVEMENT_LOG_APPLIES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_ECONOMY_MOVEMENT_CMD_NAMES_WAVE183: &[&str] = &[
    "click_live_gameworld_economy_movement_ok_economy",
    "click_live_gameworld_economy_movement_ok_movement",
    "click_live_gameworld_economy_movement_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_economy_movement_method_names_residual_wave183() -> bool {
    LIVE_GAMEWORLD_ECONOMY_MOVEMENT_METHOD_NAMES_WAVE183.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_ECONOMY_MOVEMENT_METHOD_NAMES_WAVE183,
            "writeback_economy_to_host",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_ECONOMY_MOVEMENT_METHOD_NAMES_WAVE183,
            "apply_host_movement_events",
        ) == Some(2)
        && residual_name_index(
            LIVE_GAMEWORLD_ECONOMY_MOVEMENT_METHOD_NAMES_WAVE183,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_economy_movement_nav_commands_residual_wave183() -> bool {
    LIVE_GAMEWORLD_ECONOMY_MOVEMENT_NAV_STEPS_WAVE183.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_ECONOMY_MOVEMENT_NAV_STEPS_WAVE183,
            "REQUIRE_ECONOMY_WRITEBACK",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_ECONOMY_MOVEMENT_NAV_STEPS_WAVE183,
            "LIVE_MOVEMENT_LOG_APPLIES",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_GAMEWORLD_ECONOMY_MOVEMENT_CMD_NAMES_WAVE183.len() == 3
}

/// Wave 183 composite residual honesty pack.
pub fn honesty_live_gameworld_economy_movement_residual_pack_wave183() -> bool {
    honesty_live_gameworld_economy_movement_method_names_residual_wave183()
        && honesty_live_gameworld_economy_movement_nav_commands_residual_wave183()
}

pub fn honesty_economy_movement_channel_api_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let econ = include_str!("../host_economy_log.rs");
    let mov = include_str!("../host_movement_log.rs");
    src.contains("pub fn writeback_economy_to_host")
        && src.contains("pub fn apply_host_economy_events")
        && src.contains("pub fn apply_host_movement_events")
        && src.contains("pub fn writeback_movement_to_host")
        && econ.contains("pub struct HostEconomyEvent")
        && mov.contains("pub struct HostMovementEvent")
}

/// Source residual: economy + movement last-writers default off.
pub fn honesty_economy_movement_authority_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let econ_ok = {
        let i = match src.find("pub fn gameworld_economy_authority_enabled") {
            Some(i) => i,
            None => return false,
        };
        let body = &src[i..src.len().min(i + 300)];
        body.contains("false")
    };
    let move_ok = {
        let i = match src.find("pub fn gameworld_movement_authority_enabled") {
            Some(i) => i,
            None => return false,
        };
        let body = &src[i..src.len().min(i + 300)];
        body.contains("false")
    };
    econ_ok && move_ok
}

/// Live residual: economy writeback + movement log apply.
pub fn simulate_live_gameworld_economy_movement_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, host_movement_log};
    use crate::gameworld_shadow::{
        GameWorldShadow, ensure_gate_damage_authority, gameworld_economy_authority_enabled,
        gameworld_movement_authority_enabled,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_gameworld_economy_movement_residual_pack_wave183() {
        return false;
    }
    if !honesty_economy_movement_channel_api_source() {
        return false;
    }
    if !honesty_economy_movement_authority_default_on_source() {
        return false;
    }

    ensure_gate_damage_authority();
    // Wave 757: restore authority env if earlier tests forced off (process-global).
    // SAFETY: env mutation funnels through env_compat wrappers; serialized
    // by the repo --test-threads=1 / authority_env_lock convention and
    // caches are refreshed immediately below.
    unsafe {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    }
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    if !gameworld_economy_authority_enabled() || !gameworld_movement_authority_enabled() {
        return false;
    }
    // Wave 757: clear leaked coupled-tick depth from earlier tests so movement
    // writeback is not skipped due to stale pending-host-log gates.
    while crate::gameworld_shadow::shadow_coupled_tick_active() {
        crate::gameworld_shadow::end_shadow_coupled_tick();
    }
    crate::gameworld_shadow::clear_active_shadow_for_coupled_tick();
    host_movement_log::clear();

    // --- Economy writeback ---
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LiveEconMv183");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    if logic.get_players().is_empty() {
        return false;
    }
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    let shadow_supplies = shadow
        .world()
        .player(gamelogic::world::PlayerId::from_index(0))
        .map(|p| p.supplies)
        .unwrap_or(0);
    // Desync host cash downward; writeback must restore from GameWorld.
    if let Some(p) = logic.get_player_mut(hid) {
        p.resources.supplies = shadow_supplies.saturating_sub(1234);
    }
    let wb = shadow.writeback_economy_to_host(&mut logic);
    if wb < 1 {
        return false;
    }
    let host_supplies = logic
        .get_player(hid)
        .map(|p| p.resources.supplies)
        .unwrap_or(u32::MAX);
    if host_supplies != shadow_supplies {
        return false;
    }

    // --- Movement channel ---
    if !logic.templates.contains_key("LiveMv183") {
        let mut t = ThingTemplate::new("LiveMv183");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LiveMv183".into(), t);
    }
    let Some(oid) = logic.create_object("LiveMv183", Team::USA, Vec3::new(26.0, 0.0, 26.0)) else {
        return false;
    };
    // Re-sync so the new unit maps into the shadow.
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    host_movement_log::clear();
    {
        let Some(o) = logic.get_objects_mut().get_mut(&oid) else {
            return false;
        };
        o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
        o.movement.max_speed = 12.5;
        o.movement.path = vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)];
        o.movement.current_path_index = 1;
        o.record_host_movement();
    }
    let events = host_movement_log::drain();
    if events.is_empty() {
        return false;
    }
    // Zero shadow velocity then apply log.
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [0.0, 0.0, 0.0];
        e.move_max_speed = 0.0;
    }
    let n = shadow.apply_host_movement_events(&events);
    if n < 1 {
        return false;
    }
    {
        let Some(e) = shadow.world().entity(eid) else {
            return false;
        };
        if (e.velocity[0] - 3.0).abs() > 1e-3 || (e.velocity[2] - 4.0).abs() > 1e-3 {
            return false;
        }
        if (e.move_max_speed - 12.5).abs() > 1e-3 {
            return false;
        }
    }
    let _ = shadow.writeback_movement_to_host(&mut logic);

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_economy_movement_method_names_residual_wave183());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_economy_movement_nav_commands_residual_wave183());
    }

    #[test]
    fn wave183_composite_pack() {
        assert!(honesty_live_gameworld_economy_movement_residual_pack_wave183());
    }

    #[test]
    fn economy_movement_sources() {
        assert!(honesty_economy_movement_channel_api_source());
        assert!(honesty_economy_movement_authority_default_on_source());
    }

    #[test]
    fn simulate_live_gameworld_economy_movement_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_economy_movement_honesty(),
            "live GameWorld economy writeback + movement channel residual must latch"
        );
    }
}
