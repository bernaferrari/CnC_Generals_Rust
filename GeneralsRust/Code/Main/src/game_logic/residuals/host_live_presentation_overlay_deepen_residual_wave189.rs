//! Wave 189 residual peels: deepened PresentationFrame←GameWorld overlay residual
//! (velocity/selection/team_color/body_damage last-writer stamps; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 188 executable GameWorld presentation residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `PresentationFrame::overlay_gameworld_shadow` stamps motion + selection
//! - engine still calls overlay on main seed paths
//!
//! Fail-closed:
//! - Not full build_from_gameworld cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Deepened overlay residual method names.
pub const LIVE_PRESENTATION_OVERLAY_DEEPEN_METHOD_NAMES_WAVE189: &[&str] = &[
    "overlay_gameworld_shadow",
    "velocity",
    "move_max_speed",
    "selected",
    "team_color",
    "body_damage_state",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_OVERLAY_DEEPEN_NAV_STEPS_WAVE189: &[&str] = &[
    "REQUIRE_DEEP_OVERLAY_FIELDS",
    "REQUIRE_ENGINE_STILL_CALLS_OVERLAY",
    "LIVE_STAMP_VELOCITY_SELECTION",
    "LIVE_STAMP_TEAM_COLOR_BODY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_OVERLAY_DEEPEN_CMD_NAMES_WAVE189: &[&str] = &[
    "click_live_presentation_overlay_deepen_ok_velocity",
    "click_live_presentation_overlay_deepen_ok_selection",
    "click_live_presentation_overlay_deepen_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_overlay_deepen_method_names_residual_wave189() -> bool {
    LIVE_PRESENTATION_OVERLAY_DEEPEN_METHOD_NAMES_WAVE189.len() == 7
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_DEEPEN_METHOD_NAMES_WAVE189,
            "overlay_gameworld_shadow",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_DEEPEN_METHOD_NAMES_WAVE189,
            "body_damage_state",
        ) == Some(5)
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_DEEPEN_METHOD_NAMES_WAVE189,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_overlay_deepen_nav_commands_residual_wave189() -> bool {
    LIVE_PRESENTATION_OVERLAY_DEEPEN_NAV_STEPS_WAVE189.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_DEEPEN_NAV_STEPS_WAVE189,
            "REQUIRE_DEEP_OVERLAY_FIELDS",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_DEEPEN_NAV_STEPS_WAVE189,
            "LIVE_STAMP_VELOCITY_SELECTION",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_OVERLAY_DEEPEN_CMD_NAMES_WAVE189.len() == 3
}

/// Wave 189 composite residual honesty pack.
pub fn honesty_live_presentation_overlay_deepen_residual_pack_wave189() -> bool {
    honesty_live_presentation_overlay_deepen_method_names_residual_wave189()
        && honesty_live_presentation_overlay_deepen_nav_commands_residual_wave189()
}

/// Source residual: overlay stamps velocity/selection/team_color/body_damage.
pub fn honesty_overlay_deepen_fields_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let i = match src.find("fn overlay_gameworld_shadow") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 4500)];
    body.contains("ent.velocity")
        && body.contains("move_max_speed")
        && body.contains("ent.selected")
        && body.contains("ent.team_color")
        && body.contains("ent.body_damage_state")
        && body.contains("obj.velocity")
        && body.contains("obj.selected")
}

/// Source residual: engine still calls overlay on seed paths.
pub fn honesty_engine_still_calls_overlay_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 195: engine presentation helpers fold overlay/rebuild.
    (eng.contains("build_for_engine") || eng.contains("build_with_victory_for_engine"))
        && crate::presentation_frame::PRESENTATION_FRAME_SRC.contains("fn overlay_gameworld_shadow")
}

/// Live residual: overlay stamps deepened fields from desynced shadow entity.
pub fn simulate_live_presentation_overlay_deepen_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_presentation_overlay_deepen_residual_pack_wave189() {
        return false;
    }
    if !honesty_overlay_deepen_fields_source() {
        return false;
    }
    if !honesty_engine_still_calls_overlay_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlayDeep189");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("OverlayDeep189") {
        let mut t = ThingTemplate::new("OverlayDeep189");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("OverlayDeep189".into(), t);
    }
    let Some(oid) = logic.create_object("OverlayDeep189", Team::USA, Vec3::new(11.0, 0.0, 12.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [3.5, 0.0, -2.25];
        e.move_max_speed = 18.5;
        e.selected = true;
        e.team_color = [0.1, 0.2, 0.3, 1.0];
        e.body_damage_state = 2;
        e.health = 88.0;
    }

    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    if pres.objects.iter().all(|o| o.id != oid) {
        return false;
    }

    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    let Some(obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };

    if (obj.velocity.x - 3.5).abs() > 1e-3 || (obj.velocity.z + 2.25).abs() > 1e-3 {
        return false;
    }
    if (obj.move_max_speed - 18.5).abs() > 1e-3 {
        return false;
    }
    if !obj.selected {
        return false;
    }
    if obj.team_color != [0.1, 0.2, 0.3, 1.0] {
        return false;
    }
    if obj.body_damage_state != 2 {
        return false;
    }
    if (obj.health_current - 88.0).abs() > 1e-3 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_overlay_deepen_method_names_residual_wave189());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_overlay_deepen_nav_commands_residual_wave189());
    }

    #[test]
    fn wave189_composite_pack() {
        assert!(honesty_live_presentation_overlay_deepen_residual_pack_wave189());
    }

    #[test]
    fn overlay_deepen_sources() {
        assert!(honesty_overlay_deepen_fields_source());
        assert!(honesty_engine_still_calls_overlay_source());
    }

    #[test]
    fn simulate_live_presentation_overlay_deepen_honesty_residual_live() {
        assert!(
            simulate_live_presentation_overlay_deepen_honesty(),
            "deepened PresentationFrame overlay residual must latch"
        );
    }
}
