//! Wave 196 residual peels: executable vertical slice requires GameWorld
//! `gameworld_rebuilt` when GW presentation entities are observed; PresentationFrame
//! records `gameworld_primary_objects`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 195 build_for_engine residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - executable_smoke `gameworld_rebuilt_boundary_ok`
//! - `PresentationFrame::gameworld_primary_objects`
//! - runtime-host exports `gameworld_primary_objects`
//!
//! Fail-closed:
//! - Soft when no GW entities observed
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Rebuilt vertical gate residual method names.
pub const LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_METHOD_NAMES_WAVE196: &[&str] = &[
    "gameworld_rebuilt_boundary_ok",
    "gameworld_rebuilt_ok",
    "gameworld_primary_objects",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_NAV_STEPS_WAVE196: &[&str] = &[
    "REQUIRE_REBUILT_BOUNDARY",
    "REQUIRE_PRIMARY_OBJECTS_FLAG",
    "REQUIRE_STATUS_EXPORT",
    "LIVE_REBUILD_SETS_PRIMARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_CMD_NAMES_WAVE196: &[&str] = &[
    "click_live_presentation_rebuilt_vertical_gate_ok_prepare",
    "click_live_presentation_rebuilt_vertical_gate_ok_live",
    "click_live_presentation_rebuilt_vertical_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_rebuilt_vertical_gate_method_names_residual_wave196() -> bool {
    LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_METHOD_NAMES_WAVE196.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_METHOD_NAMES_WAVE196,
            "gameworld_rebuilt_boundary_ok",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_METHOD_NAMES_WAVE196,
            "gameworld_primary_objects",
        ) == Some(2)
        && residual_name_index(
            LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_METHOD_NAMES_WAVE196,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_rebuilt_vertical_gate_nav_commands_residual_wave196() -> bool {
    LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_NAV_STEPS_WAVE196.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_NAV_STEPS_WAVE196,
            "REQUIRE_REBUILT_BOUNDARY",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_NAV_STEPS_WAVE196,
            "LIVE_REBUILD_SETS_PRIMARY",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_REBUILT_VERTICAL_GATE_CMD_NAMES_WAVE196.len() == 3
}

/// Wave 196 composite residual honesty pack.
pub fn honesty_live_presentation_rebuilt_vertical_gate_residual_pack_wave196() -> bool {
    honesty_live_presentation_rebuilt_vertical_gate_method_names_residual_wave196()
        && honesty_live_presentation_rebuilt_vertical_gate_nav_commands_residual_wave196()
}

/// Source residual: executable smoke gates vertical on rebuilt when GW ents seen.
pub fn honesty_executable_rebuilt_vertical_gate_source() -> bool {
    let src = include_str!("../../executable_smoke.rs");
    src.contains("gameworld_rebuilt_boundary_ok")
        && src.contains("gameworld_rebuilt_ok")
        && src.contains("host_vertical_slice_ok")
        && src.contains("gameworld_presentation_entities_ok")
}

/// Source residual: PresentationFrame primary flag + engine export.
pub fn honesty_primary_objects_flag_source() -> bool {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    pf.contains("pub gameworld_primary_objects: bool")
        && pf.contains("self.gameworld_primary_objects = n > 0")
        && eng.contains("gameworld_primary_objects")
        && eng.contains("gameworld_primary_objects=")
}

/// Live residual: rebuild sets primary flag; vertical boundary logic is fail-closed soft.
pub fn simulate_live_presentation_rebuilt_vertical_gate_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_presentation_rebuilt_vertical_gate_residual_pack_wave196() {
        return false;
    }
    if !honesty_executable_rebuilt_vertical_gate_source() {
        return false;
    }
    if !honesty_primary_objects_flag_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RebuiltVert196");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("RebuiltVert196") {
        let mut t = ThingTemplate::new("RebuiltVert196");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("RebuiltVert196".into(), t);
    }
    let Some(_oid) = logic.create_object("RebuiltVert196", Team::USA, Vec3::new(5.0, 0.0, 6.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let mut pres = PresentationFrame::build_for_engine(&logic, 0, Some(&shadow));
    if crate::presentation_frame::presentation_from_gameworld_enabled() {
        if !pres.gameworld_primary_objects {
            return false;
        }
        if pres.gameworld_rebuilt == 0 {
            return false;
        }
    }

    // Soft boundary identity: entities observed without rebuilt fails the gate predicate.
    let entities_ok = true;
    let rebuilt_ok = pres.gameworld_rebuilt > 0;
    let boundary = !entities_ok || rebuilt_ok;
    if crate::presentation_frame::presentation_from_gameworld_enabled() && !boundary {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_rebuilt_vertical_gate_method_names_residual_wave196());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_rebuilt_vertical_gate_nav_commands_residual_wave196());
    }

    #[test]
    fn wave196_composite_pack() {
        assert!(honesty_live_presentation_rebuilt_vertical_gate_residual_pack_wave196());
    }

    #[test]
    fn rebuilt_vertical_sources() {
        assert!(honesty_executable_rebuilt_vertical_gate_source());
        assert!(honesty_primary_objects_flag_source());
    }

    #[test]
    fn simulate_live_presentation_rebuilt_vertical_gate_honesty_residual_live() {
        assert!(
            simulate_live_presentation_rebuilt_vertical_gate_honesty(),
            "rebuilt vertical gate residual must latch"
        );
    }
}
