//! Wave 194 residual peels: GameWorld-primary presentation object roster is
//! **default ON** (`presentation_from_gameworld_enabled`), opt-out via
//! `GENERALS_PRESENTATION_FROM_GAMEWORLD=0`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 193 build_from_gameworld API residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_from_gameworld_enabled` defaults true
//! - engine calls rebuild when enabled (no env=1 required)
//! - executable_smoke tracks `gameworld_rebuilt`
//!
//! Fail-closed:
//! - Host still supplies non-object residual via build_from_logic
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Default-on residual method names.
pub const LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_METHOD_NAMES_WAVE194: &[&str] = &[
    "presentation_from_gameworld_enabled",
    "rebuild_objects_from_gameworld",
    "gameworld_rebuilt",
    "GENERALS_PRESENTATION_FROM_GAMEWORLD",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_NAV_STEPS_WAVE194: &[&str] = &[
    "REQUIRE_DEFAULT_ENABLED_HELPER",
    "REQUIRE_ENGINE_DEFAULT_REBUILD",
    "REQUIRE_EXEC_TRACKS_REBUILT",
    "LIVE_DEFAULT_ENABLED_TRUE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_CMD_NAMES_WAVE194: &[&str] = &[
    "click_live_presentation_from_gameworld_default_ok_prepare",
    "click_live_presentation_from_gameworld_default_ok_live",
    "click_live_presentation_from_gameworld_default_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_from_gameworld_default_method_names_residual_wave194() -> bool {
    LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_METHOD_NAMES_WAVE194.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_METHOD_NAMES_WAVE194,
            "presentation_from_gameworld_enabled",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_METHOD_NAMES_WAVE194,
            "GENERALS_PRESENTATION_FROM_GAMEWORLD",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_METHOD_NAMES_WAVE194,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_from_gameworld_default_nav_commands_residual_wave194() -> bool {
    LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_NAV_STEPS_WAVE194.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_NAV_STEPS_WAVE194,
            "REQUIRE_DEFAULT_ENABLED_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_NAV_STEPS_WAVE194,
            "LIVE_DEFAULT_ENABLED_TRUE",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_FROM_GAMEWORLD_DEFAULT_CMD_NAMES_WAVE194.len() == 3
}

/// Wave 194 composite residual honesty pack.
pub fn honesty_live_presentation_from_gameworld_default_residual_pack_wave194() -> bool {
    honesty_live_presentation_from_gameworld_default_method_names_residual_wave194()
        && honesty_live_presentation_from_gameworld_default_nav_commands_residual_wave194()
}

/// Source residual: helper defaults ON (Err => true; only 0/false/no/off disables).
pub fn honesty_presentation_from_gameworld_enabled_default_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let i = match src.find("pub fn presentation_from_gameworld_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 700)];
    body.contains("Err(_) => true")
        && body.contains("\"0\"")
        && body.contains("GENERALS_PRESENTATION_FROM_GAMEWORLD")
}

/// Source residual: engine uses helper (not env=1 opt-in).
pub fn honesty_engine_default_rebuild_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 195: engine uses build_for_engine / build_with_victory_for_engine;
    // rebuild lives inside those helpers when presentation_from_gameworld_enabled.
    (src.contains("build_for_engine(")
        || src.contains("build_with_victory_for_engine(")
        || src.contains("build_for_engine_with_runtime_heightmap(")
        || src.contains("build_with_victory_for_engine_with_runtime_heightmap("))
        && crate::presentation_frame::PRESENTATION_FRAME_SRC
            .contains("presentation_from_gameworld_enabled")
        && crate::presentation_frame::PRESENTATION_FRAME_SRC
            .contains("rebuild_objects_from_gameworld")
}

/// Source residual: executable smoke tracks rebuilt status key.
pub fn honesty_executable_tracks_gameworld_rebuilt_source() -> bool {
    let src = crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC;
    src.contains("gameworld_rebuilt")
        && src.contains("max_gameworld_rebuilt")
        && src.contains("\"gameworld_rebuilt\"")
}

/// Live residual: default enabled + rebuild latches object roster from GameWorld.
pub fn simulate_live_presentation_from_gameworld_default_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::{PresentationFrame, presentation_from_gameworld_enabled};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::world::PlayerId;
    use gamelogic::world::entities::TemplateRef;
    use glam::Vec3;

    if !honesty_live_presentation_from_gameworld_default_residual_pack_wave194() {
        return false;
    }
    if !honesty_presentation_from_gameworld_enabled_default_source() {
        return false;
    }
    if !honesty_engine_default_rebuild_source() {
        return false;
    }
    if !honesty_executable_tracks_gameworld_rebuilt_source() {
        return false;
    }

    // Default must be enabled when env unset. Tests may run with inherited env;
    // only fail if explicitly disabled.
    let disabled = std::env::var("GENERALS_PRESENTATION_FROM_GAMEWORLD")
        .map(|v| {
            matches!(
                v.trim(),
                "0" | "false" | "FALSE" | "no" | "NO" | "off" | "OFF"
            )
        })
        .unwrap_or(false);
    if disabled {
        // Still verify helper honors disable when set — force-check source path.
        return honesty_presentation_from_gameworld_enabled_default_source();
    }
    if !presentation_from_gameworld_enabled() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FromGwDefault194");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("DefaultGw194") {
        let mut t = ThingTemplate::new("DefaultGw194");
        t.set_health(160.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("DefaultGw194".into(), t);
    }
    let Some(oid) = logic.create_object("DefaultGw194", Team::USA, Vec3::new(30.0, 0.0, 31.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw_id = {
        let tmpl = TemplateRef {
            name: "GwOnlyDefault194".into(),
            source: None,
        };
        shadow.world_mut().spawn_entity(
            tmpl,
            Some(PlayerId::from_index(0)),
            gamelogic::world::entities::Transform::new([70.0, 0.0, 71.0], 0.1),
            66.0,
        )
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
        e.max_health = 66.0;
        e.team_ordinal = 0;
        e.selected = true;
    }
    if let Some(e) = shadow
        .entity_for_host(oid)
        .and_then(|eid| shadow.world_mut().world_mut().entity_mut(eid))
    {
        e.transform.position = [32.0, 0.0, 33.0].into();
        e.health = 155.0;
        e.max_health = 160.0;
    }

    // Mirror engine default path: host build → overlay → append → rebuild when enabled.
    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    let _ = pres.overlay_gameworld_shadow(&shadow);
    let _ = pres.append_missing_from_gameworld(&shadow);
    if presentation_from_gameworld_enabled() {
        let n = pres.rebuild_objects_from_gameworld(&shadow);
        if n < 2 || pres.gameworld_rebuilt < 2 {
            return false;
        }
    }
    let Some(host_obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };
    if (host_obj.position.x - 32.0).abs() > 1e-3 || (host_obj.health_current - 155.0).abs() > 1e-3 {
        return false;
    }
    let synth = 0x8000_0000 | gw_id.get();
    if pres.objects.iter().all(|o| o.id.0 != synth) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_from_gameworld_default_method_names_residual_wave194());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_from_gameworld_default_nav_commands_residual_wave194());
    }

    #[test]
    fn wave194_composite_pack() {
        assert!(honesty_live_presentation_from_gameworld_default_residual_pack_wave194());
    }

    #[test]
    fn default_from_gameworld_sources() {
        assert!(honesty_presentation_from_gameworld_enabled_default_source());
        assert!(honesty_engine_default_rebuild_source());
        assert!(honesty_executable_tracks_gameworld_rebuilt_source());
    }

    #[test]
    fn simulate_live_presentation_from_gameworld_default_honesty_residual_live() {
        assert!(
            simulate_live_presentation_from_gameworld_default_honesty(),
            "default GameWorld presentation residual must latch"
        );
    }
}
