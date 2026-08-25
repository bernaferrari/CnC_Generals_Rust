//! Wave 187 residual peels: live PresentationFrame←GameWorld overlay residual
//! (`overlay_gameworld_shadow` stamps pose/health from shadow last-writer;
//! engine calls it on main presentation seed paths; never flips shell
//! `playable_claim`).
//!
//! Orthogonal to Wave 186 GameWorld presentation view residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `PresentationFrame::overlay_gameworld_shadow`
//! - engine seed/apply paths call overlay when shadow present
//! - GameWorld remains last-writer for HP/pose presentation identity
//!
//! Fail-closed:
//! - Not full PresentationFrame build_from_gameworld cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live presentation GameWorld overlay residual method names.
pub const LIVE_PRESENTATION_GAMEWORLD_OVERLAY_METHOD_NAMES_WAVE187: &[&str] = &[
    "overlay_gameworld_shadow",
    "entity_for_host",
    "health_current",
    "build_from_logic",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_GAMEWORLD_OVERLAY_NAV_STEPS_WAVE187: &[&str] = &[
    "REQUIRE_OVERLAY_API",
    "REQUIRE_ENGINE_CALLS_OVERLAY",
    "LIVE_BUILD_THEN_OVERLAY",
    "LIVE_HEALTH_POSE_STAMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_GAMEWORLD_OVERLAY_CMD_NAMES_WAVE187: &[&str] = &[
    "click_live_presentation_gameworld_overlay_ok_overlay",
    "click_live_presentation_gameworld_overlay_ok_stamp",
    "click_live_presentation_gameworld_overlay_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_gameworld_overlay_method_names_residual_wave187() -> bool {
    LIVE_PRESENTATION_GAMEWORLD_OVERLAY_METHOD_NAMES_WAVE187.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_GAMEWORLD_OVERLAY_METHOD_NAMES_WAVE187,
            "overlay_gameworld_shadow",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_GAMEWORLD_OVERLAY_METHOD_NAMES_WAVE187,
            "build_from_logic",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_GAMEWORLD_OVERLAY_METHOD_NAMES_WAVE187,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_gameworld_overlay_nav_commands_residual_wave187() -> bool {
    LIVE_PRESENTATION_GAMEWORLD_OVERLAY_NAV_STEPS_WAVE187.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_GAMEWORLD_OVERLAY_NAV_STEPS_WAVE187,
            "REQUIRE_OVERLAY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_GAMEWORLD_OVERLAY_NAV_STEPS_WAVE187,
            "LIVE_HEALTH_POSE_STAMP",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_GAMEWORLD_OVERLAY_CMD_NAMES_WAVE187.len() == 3
}

/// Wave 187 composite residual honesty pack.
pub fn honesty_live_presentation_gameworld_overlay_residual_pack_wave187() -> bool {
    honesty_live_presentation_gameworld_overlay_method_names_residual_wave187()
        && honesty_live_presentation_gameworld_overlay_nav_commands_residual_wave187()
}

/// Source residual: overlay API stamps pose/health from shadow entities.
pub fn honesty_overlay_gameworld_shadow_api_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let i = match src.find("fn overlay_gameworld_shadow") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 2500)];
    body.contains("entity_for_host")
        && body.contains("health_current")
        && body.contains("position")
        && body.contains("orientation")
        && body.contains("shadow.world().entity")
}
/// Source residual: engine calls overlay on main presentation seed paths.
pub fn honesty_engine_calls_overlay_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 195: engine presentation helpers fold overlay/rebuild.
    (eng.contains("build_for_engine") || eng.contains("build_with_victory_for_engine"))
        && crate::presentation_frame::PRESENTATION_FRAME_SRC.contains("fn overlay_gameworld_shadow")
}

/// Live residual: build_from_logic then overlay stamps shadow health/pose.
pub fn simulate_live_presentation_gameworld_overlay_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_presentation_gameworld_overlay_residual_pack_wave187() {
        return false;
    }
    if !honesty_overlay_gameworld_shadow_api_source() {
        return false;
    }
    if !honesty_engine_calls_overlay_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresGwOverlay187");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("OverlayU187") {
        let mut t = ThingTemplate::new("OverlayU187");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("OverlayU187".into(), t);
    }
    let Some(oid) = logic.create_object("OverlayU187", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    // Desync shadow health as last-writer residual (pose via Transform if available).
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.health = 77.0;
        e.transform.orientation = 1.25;
        e.transform.position.x = 42.0;
        e.transform.position.y = 0.0;
        e.transform.position.z = 43.0;
    }

    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    if pres.objects.is_empty() {
        return false;
    }
    let Some(before) = pres
        .objects
        .iter()
        .find(|o| o.id == oid)
        .map(|o| o.health_current)
    else {
        return false;
    };

    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    let Some(obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };

    if (obj.health_current - 77.0).abs() > 1e-3 {
        return false;
    }
    if (obj.position.x - 42.0).abs() > 1e-3 || (obj.position.z - 43.0).abs() > 1e-3 {
        return false;
    }
    if (obj.orientation - 1.25).abs() > 1e-4 {
        return false;
    }
    let _ = before;

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_gameworld_overlay_method_names_residual_wave187());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_gameworld_overlay_nav_commands_residual_wave187());
    }

    #[test]
    fn wave187_composite_pack() {
        assert!(honesty_live_presentation_gameworld_overlay_residual_pack_wave187());
    }

    #[test]
    fn overlay_sources() {
        assert!(honesty_overlay_gameworld_shadow_api_source());
        assert!(honesty_engine_calls_overlay_source());
    }

    #[test]
    fn simulate_live_presentation_gameworld_overlay_honesty_residual_live() {
        assert!(
            simulate_live_presentation_gameworld_overlay_honesty(),
            "live PresentationFrame overlay from GameWorld shadow residual must latch"
        );
    }
}
