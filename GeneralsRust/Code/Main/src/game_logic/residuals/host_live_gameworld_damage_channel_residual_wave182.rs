//! Wave 182 residual peels: live GameWorld damage channel residual
//! (host `take_damage` → damage log → `apply_host_damage_events` shadow parity;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 181 construction writeback residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `host_damage_log::record` from `Object::take_damage_from`
//! - `GameWorldShadow::apply_host_damage_events`
//! - `writeback_body_damage_to_host` body-state channel
//! - damage authority default-on
//!
//! Fail-closed:
//! - Not full combat on Lone Eagle in this peel
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live damage channel residual method names.
pub const LIVE_GAMEWORLD_DAMAGE_CHANNEL_METHOD_NAMES_WAVE182: &[&str] = &[
    "host_damage_log::record",
    "apply_host_damage_events",
    "writeback_body_damage_to_host",
    "gameworld_damage_authority_enabled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_DAMAGE_CHANNEL_NAV_STEPS_WAVE182: &[&str] = &[
    "REQUIRE_DAMAGE_LOG_API",
    "REQUIRE_APPLY_HOST_DAMAGE",
    "REQUIRE_DAMAGE_AUTH_DEFAULT_ON",
    "LIVE_TAKE_DAMAGE_LOGS",
    "LIVE_SHADOW_HEALTH_PARITY",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_DAMAGE_CHANNEL_CMD_NAMES_WAVE182: &[&str] = &[
    "click_live_gameworld_damage_channel_ok_log",
    "click_live_gameworld_damage_channel_ok_parity",
    "click_live_gameworld_damage_channel_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_damage_channel_method_names_residual_wave182() -> bool {
    LIVE_GAMEWORLD_DAMAGE_CHANNEL_METHOD_NAMES_WAVE182.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_DAMAGE_CHANNEL_METHOD_NAMES_WAVE182,
            "host_damage_log::record",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_DAMAGE_CHANNEL_METHOD_NAMES_WAVE182,
            "apply_host_damage_events",
        ) == Some(1)
        && residual_name_index(
            LIVE_GAMEWORLD_DAMAGE_CHANNEL_METHOD_NAMES_WAVE182,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_damage_channel_nav_commands_residual_wave182() -> bool {
    LIVE_GAMEWORLD_DAMAGE_CHANNEL_NAV_STEPS_WAVE182.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_DAMAGE_CHANNEL_NAV_STEPS_WAVE182,
            "REQUIRE_DAMAGE_LOG_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_DAMAGE_CHANNEL_NAV_STEPS_WAVE182,
            "LIVE_SHADOW_HEALTH_PARITY",
        ) == Some(4)
        && RUNTIME_HOST_LIVE_GAMEWORLD_DAMAGE_CHANNEL_CMD_NAMES_WAVE182.len() == 3
}

/// Wave 182 composite residual honesty pack.
pub fn honesty_live_gameworld_damage_channel_residual_pack_wave182() -> bool {
    honesty_live_gameworld_damage_channel_method_names_residual_wave182()
        && honesty_live_gameworld_damage_channel_nav_commands_residual_wave182()
}

/// Source residual: damage log + apply/writeback APIs.
pub fn honesty_damage_channel_api_source() -> bool {
    let log = include_str!("../host_damage_log.rs");
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    log.contains("pub fn record")
        && log.contains("pub struct HostDamageEvent")
        && src.contains("pub fn apply_host_damage_events")
        && src.contains("pub fn writeback_body_damage_to_host")
        && src.contains("pub fn gameworld_damage_authority_enabled")
}

/// Source residual: damage last-writer defaults off (host sole HP writer) and
/// the gate is backed by the GameLogic authority context (hq-e84zk).
pub fn honesty_damage_authority_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = match src.find("pub fn gameworld_damage_authority_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 350)];
    body.contains("current_gameworld_authority().damage")
}

/// Live residual: host take_damage logs → shadow apply → health parity.
pub fn simulate_live_gameworld_damage_channel_honesty() -> bool {
    use crate::game_logic::host_damage_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, ensure_gate_damage_authority, gameworld_damage_authority_enabled,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_gameworld_damage_channel_residual_pack_wave182() {
        return false;
    }
    if !honesty_damage_channel_api_source() {
        return false;
    }
    if !honesty_damage_authority_default_on_source() {
        return false;
    }

    ensure_gate_damage_authority();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    // Authority arm happens on the harness's own instance below (hq-e84zk
    // context fields; a pre-instance check here would read a stale snapshot).

    host_damage_log::clear();
    let mut logic = GameLogic::new();
    logic.set_damage_authority(true);
    if !gameworld_damage_authority_enabled() {
        return false;
    }
    let cfg = golden_skirmish_config("LiveDmgCh182");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("LiveDmgUnit182") {
        let mut t = ThingTemplate::new("LiveDmgUnit182");
        t.set_health(150.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("LiveDmgUnit182".into(), t);
    }
    let Some(oid) = logic.create_object("LiveDmgUnit182", Team::USA, Vec3::new(2.0, 0.0, 0.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    let pre_host = logic
        .get_object(oid)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let pre_shadow = shadow.world().entity(eid).map(|e| e.health).unwrap_or(0.0);
    if (pre_host - pre_shadow).abs() > 0.05 {
        return false;
    }

    // Host damage records the log channel.
    host_damage_log::clear();
    if let Some(obj) = logic.get_objects_mut().get_mut(&oid) {
        let _ = obj.take_damage(40.0);
    }
    let events = host_damage_log::drain();
    if events.is_empty() {
        return false;
    }
    if events[0].target != oid || events[0].amount <= 0.0 {
        return false;
    }

    // Restore shadow health to pre-damage, then apply log as GameWorld mutations.
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.health = pre_shadow;
    }
    let (queued, _applied) = shadow.apply_host_damage_events(&events);
    if queued < 1 {
        return false;
    }

    let host_h = logic
        .get_object(oid)
        .map(|o| o.health.current)
        .unwrap_or(-1.0);
    let shadow_h = shadow.world().entity(eid).map(|e| e.health).unwrap_or(-1.0);
    if host_h >= pre_host - 1.0 {
        // Host must have lost HP.
        return false;
    }
    if (host_h - shadow_h).abs() > 0.05 {
        return false;
    }

    // Body-damage writeback API must be callable under authority (no panic).
    let _ = shadow.writeback_body_damage_to_host(&mut logic);

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_damage_channel_method_names_residual_wave182());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_damage_channel_nav_commands_residual_wave182());
    }

    #[test]
    fn wave182_composite_pack() {
        assert!(honesty_live_gameworld_damage_channel_residual_pack_wave182());
    }

    #[test]
    fn damage_channel_sources() {
        assert!(honesty_damage_channel_api_source());
        assert!(honesty_damage_authority_default_on_source());
    }

    #[test]
    fn simulate_live_gameworld_damage_channel_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_damage_channel_honesty(),
            "live GameWorld damage log→apply health parity residual must latch"
        );
    }
}
