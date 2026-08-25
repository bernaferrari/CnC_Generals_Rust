//! Wave 190 residual peels: PresentationFrame gameworld_overlay_stamped residual
//! (overlay records stamp count; engine exports it on runtime-host status;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 189 deepened overlay field stamps.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `PresentationFrame::gameworld_overlay_stamped`
//! - `overlay_gameworld_shadow` sets stamp count
//! - status `gameworld_overlay_stamped`
//!
//! Fail-closed:
//! - Not full build_from_gameworld cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Overlay stamp residual method names.
pub const LIVE_PRESENTATION_OVERLAY_STAMP_METHOD_NAMES_WAVE190: &[&str] = &[
    "gameworld_overlay_stamped",
    "overlay_gameworld_shadow",
    "gameworld_overlay_stamped=",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_OVERLAY_STAMP_NAV_STEPS_WAVE190: &[&str] = &[
    "REQUIRE_STAMP_FIELD",
    "REQUIRE_OVERLAY_SETS_STAMP",
    "REQUIRE_STATUS_EXPORTS_STAMP",
    "LIVE_OVERLAY_INCREMENTS_STAMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_OVERLAY_STAMP_CMD_NAMES_WAVE190: &[&str] = &[
    "click_live_presentation_overlay_stamp_ok_field",
    "click_live_presentation_overlay_stamp_ok_status",
    "click_live_presentation_overlay_stamp_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_overlay_stamp_method_names_residual_wave190() -> bool {
    LIVE_PRESENTATION_OVERLAY_STAMP_METHOD_NAMES_WAVE190.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_STAMP_METHOD_NAMES_WAVE190,
            "gameworld_overlay_stamped",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_STAMP_METHOD_NAMES_WAVE190,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_overlay_stamp_nav_commands_residual_wave190() -> bool {
    LIVE_PRESENTATION_OVERLAY_STAMP_NAV_STEPS_WAVE190.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_STAMP_NAV_STEPS_WAVE190,
            "REQUIRE_STAMP_FIELD",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_OVERLAY_STAMP_NAV_STEPS_WAVE190,
            "LIVE_OVERLAY_INCREMENTS_STAMP",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_OVERLAY_STAMP_CMD_NAMES_WAVE190.len() == 3
}

/// Wave 190 composite residual honesty pack.
pub fn honesty_live_presentation_overlay_stamp_residual_pack_wave190() -> bool {
    honesty_live_presentation_overlay_stamp_method_names_residual_wave190()
        && honesty_live_presentation_overlay_stamp_nav_commands_residual_wave190()
}

/// Source residual: PresentationFrame field + overlay assignment.
pub fn honesty_overlay_stamp_field_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    src.contains("pub gameworld_overlay_stamped: usize")
        && src.contains("self.gameworld_overlay_stamped = updated")
        && src.contains("fn overlay_gameworld_shadow")
}

/// Source residual: engine status exports stamp count.
pub fn honesty_engine_exports_overlay_stamp_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    eng.contains("gameworld_overlay_stamped")
        && eng.contains("gameworld_overlay_stamped={}")
        && eng.contains("last_presentation_frame")
}

/// Live residual: overlay sets non-zero stamp after shadow desync.
pub fn simulate_live_presentation_overlay_stamp_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_presentation_overlay_stamp_residual_pack_wave190() {
        return false;
    }
    if !honesty_overlay_stamp_field_source() {
        return false;
    }
    if !honesty_engine_exports_overlay_stamp_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlayStamp190");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("StampU190") {
        let mut t = ThingTemplate::new("StampU190");
        t.set_health(150.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("StampU190".into(), t);
    }
    let Some(oid) = logic.create_object("StampU190", Team::USA, Vec3::new(9.0, 0.0, 9.0)) else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.health = 55.0;
        e.selected = true;
    }

    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    if pres.gameworld_overlay_stamped != 0 {
        return false;
    }
    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    if pres.gameworld_overlay_stamped == 0 {
        return false;
    }
    if pres.gameworld_overlay_stamped != n {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_overlay_stamp_method_names_residual_wave190());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_overlay_stamp_nav_commands_residual_wave190());
    }

    #[test]
    fn wave190_composite_pack() {
        assert!(honesty_live_presentation_overlay_stamp_residual_pack_wave190());
    }

    #[test]
    fn overlay_stamp_sources() {
        assert!(honesty_overlay_stamp_field_source());
        assert!(honesty_engine_exports_overlay_stamp_source());
    }

    #[test]
    fn simulate_live_presentation_overlay_stamp_honesty_residual_live() {
        assert!(
            simulate_live_presentation_overlay_stamp_honesty(),
            "PresentationFrame gameworld_overlay_stamped residual must latch"
        );
    }
}
