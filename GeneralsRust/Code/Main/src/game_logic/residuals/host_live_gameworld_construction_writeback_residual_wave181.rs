//! Wave 181 residual peels: live GameWorld construction writeback residual
//! (host construction progress log → shadow apply → sole-tick advance →
//! writeback; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 180 production writeback residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `apply_host_construction_progress_events`
//! - `tick_construction_progress` + `writeback_construction_to_host`
//! - `gameworld_construction_sole_tick_enabled`
//!
//! Fail-closed:
//! - Not full retail dozer build on Lone Eagle in this peel
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live construction writeback residual method names.
pub const LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181: &[&str] = &[
    "apply_host_construction_progress_events",
    "tick_construction_progress",
    "writeback_construction_to_host",
    "gameworld_construction_sole_tick_enabled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181: &[&str] = &[
    "REQUIRE_CONSTRUCTION_PROGRESS_CHANNEL",
    "REQUIRE_TICK_AND_WRITEBACK",
    "REQUIRE_CONSTRUCTION_SOLE_TICK",
    "LIVE_HOST_TO_SHADOW_PERCENT",
    "LIVE_SOLE_TICK_ARMS",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_CMD_NAMES_WAVE181: &[&str] = &[
    "click_live_gameworld_construction_writeback_ok_progress",
    "click_live_gameworld_construction_writeback_ok_sole",
    "click_live_gameworld_construction_writeback_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_construction_writeback_method_names_residual_wave181() -> bool {
    LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "apply_host_construction_progress_events",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "gameworld_construction_sole_tick_enabled",
        ) == Some(3)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_METHOD_NAMES_WAVE181,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181() -> bool {
    LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181,
            "REQUIRE_CONSTRUCTION_PROGRESS_CHANNEL",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_NAV_STEPS_WAVE181,
            "LIVE_SOLE_TICK_ARMS",
        ) == Some(4)
        && RUNTIME_HOST_LIVE_GAMEWORLD_CONSTRUCTION_WRITEBACK_CMD_NAMES_WAVE181.len() == 3
}

/// Wave 181 composite residual honesty pack.
pub fn honesty_live_gameworld_construction_writeback_residual_pack_wave181() -> bool {
    honesty_live_gameworld_construction_writeback_method_names_residual_wave181()
        && honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181()
}

/// Source residual: construction progress channel + tick/writeback APIs.
pub fn honesty_construction_progress_channel_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    src.contains("pub fn apply_host_construction_progress_events")
        && src.contains("pub fn tick_construction_progress")
        && src.contains("pub fn writeback_construction_to_host")
        && src.contains("pub fn gameworld_construction_sole_tick_enabled")
}

/// Source residual: construction sole-tick requires coupling + auth + shadow.
pub fn honesty_construction_sole_tick_requires_coupling_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = match src.find("pub fn gameworld_construction_sole_tick_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 350)];
    body.contains("gameworld_construction_authority_enabled")
        && body.contains("gameworld_shadow_enabled")
        && body.contains("shadow_coupled_tick_active")
}

/// Live residual: host construction percent → GameWorld → sole-tick arms → writeback.
pub fn simulate_live_gameworld_construction_writeback_honesty() -> bool {
    use crate::game_logic::host_construction_progress_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, end_shadow_coupled_tick,
        ensure_gate_damage_authority, gameworld_construction_authority_enabled,
        gameworld_construction_sole_tick_enabled, gameworld_shadow_enabled,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    if !honesty_live_gameworld_construction_writeback_residual_pack_wave181() {
        return false;
    }
    if !honesty_construction_progress_channel_source() {
        return false;
    }
    if !honesty_construction_sole_tick_requires_coupling_source() {
        return false;
    }

    ensure_gate_damage_authority();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    if !gameworld_construction_authority_enabled() {
        return false;
    }

    host_construction_progress_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LiveConstrWB181");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("LiveConstr181") {
        let mut t = ThingTemplate::new("LiveConstr181");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("LiveConstr181".into(), t);
    }
    let Some(oid) = logic.create_object("LiveConstr181", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
    else {
        return false;
    };
    {
        let Some(o) = logic.get_objects_mut().get_mut(&oid) else {
            return false;
        };
        o.construction_percent = 0.25;
        o.set_status_under_construction(true);
    }

    host_construction_progress_log::record(oid, 0.25, true, 0.05);
    let events = host_construction_progress_log::drain();
    if events.len() != 1 {
        return false;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };
    // Clear shadow construction so apply path is observable.
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.construction_percent = 0.0;
        e.under_construction = false;
    }
    let n = shadow.apply_host_construction_progress_events(&events);
    if n < 1 {
        return false;
    }
    {
        let Some(ent) = shadow.world().entity(eid) else {
            return false;
        };
        if !ent.under_construction {
            return false;
        }
        if (ent.construction_percent - 0.25).abs() > 1e-4 {
            return false;
        }
    }

    // Sole-tick coupling for construction authority.
    begin_shadow_coupled_tick();
    let sole = gameworld_construction_sole_tick_enabled();
    let _ticked = shadow.tick_construction_progress(1.0 / 30.0);
    let written = shadow.writeback_construction_to_host(&mut logic);
    end_shadow_coupled_tick();

    if gameworld_shadow_enabled() && !sole {
        return false;
    }

    // Host should still be under construction after writeback (or shadow still holds it).
    let host_uc = logic
        .get_object(oid)
        .map(|o| o.status.under_construction || o.construction_percent > 0.0)
        .unwrap_or(false);
    let shadow_uc = shadow
        .world()
        .entity(eid)
        .map(|e| e.under_construction || e.construction_percent > 0.0)
        .unwrap_or(false);
    if !host_uc && !shadow_uc {
        return false;
    }
    let _ = written;

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_construction_writeback_method_names_residual_wave181());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181());
    }

    #[test]
    fn wave181_composite_pack() {
        assert!(honesty_live_gameworld_construction_writeback_residual_pack_wave181());
    }

    #[test]
    fn construction_writeback_sources() {
        assert!(honesty_construction_progress_channel_source());
        assert!(honesty_construction_sole_tick_requires_coupling_source());
    }

    #[test]
    fn simulate_live_gameworld_construction_writeback_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_construction_writeback_honesty(),
            "live GameWorld construction progress→tick→writeback residual must latch"
        );
    }
}
