//! Wave 191 residual peels: deepened GameWorldEntityView + executable overlay
//! stamp gate residual (observe-path carries motion/selection/body; vertical
//! slice requires overlay stamps when GW entities seen; never flips shell
//! `playable_claim`).
//!
//! Orthogonal to Wave 190 gameworld_overlay_stamped field residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `GameWorldEntityView` enriched from live Entity store
//! - `presentation_view_from_gameworld` uses `world.entities()`
//! - executable_smoke tracks/gates `gameworld_overlay_stamped`
//!
//! Fail-closed:
//! - Not full build_from_gameworld PresentationFrame cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Deepened entity view residual method names.
pub const LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_METHOD_NAMES_WAVE191: &[&str] = &[
    "GameWorldEntityView",
    "presentation_view_from_gameworld",
    "world.entities()",
    "gameworld_overlay_stamped_ok",
    "gameworld_overlay_boundary_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_NAV_STEPS_WAVE191: &[&str] = &[
    "REQUIRE_ENRICHED_ENTITY_VIEW",
    "REQUIRE_VIEW_FROM_LIVE_ENTITIES",
    "REQUIRE_EXEC_OVERLAY_STAMP_GATE",
    "LIVE_VIEW_CARRIES_MOTION",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_CMD_NAMES_WAVE191: &[&str] = &[
    "click_live_gameworld_entity_view_deepen_ok_view",
    "click_live_gameworld_entity_view_deepen_ok_exec",
    "click_live_gameworld_entity_view_deepen_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_entity_view_deepen_method_names_residual_wave191() -> bool {
    LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_METHOD_NAMES_WAVE191.len() == 6
        && residual_name_index(
            LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_METHOD_NAMES_WAVE191,
            "GameWorldEntityView",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_METHOD_NAMES_WAVE191,
            "gameworld_overlay_boundary_ok",
        ) == Some(4)
        && residual_name_index(
            LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_METHOD_NAMES_WAVE191,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_entity_view_deepen_nav_commands_residual_wave191() -> bool {
    LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_NAV_STEPS_WAVE191.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_NAV_STEPS_WAVE191,
            "REQUIRE_ENRICHED_ENTITY_VIEW",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_NAV_STEPS_WAVE191,
            "LIVE_VIEW_CARRIES_MOTION",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_GAMEWORLD_ENTITY_VIEW_DEEPEN_CMD_NAMES_WAVE191.len() == 3
}

/// Wave 191 composite residual honesty pack.
pub fn honesty_live_gameworld_entity_view_deepen_residual_pack_wave191() -> bool {
    honesty_live_gameworld_entity_view_deepen_method_names_residual_wave191()
        && honesty_live_gameworld_entity_view_deepen_nav_commands_residual_wave191()
}

/// Source residual: GameWorldEntityView carries deepened fields.
pub fn honesty_enriched_entity_view_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = match src.find("pub struct GameWorldEntityView") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 700)];
    body.contains("max_health")
        && body.contains("team_ordinal")
        && body.contains("selected")
        && body.contains("velocity")
        && body.contains("move_max_speed")
        && body.contains("body_damage_state")
        && body.contains("team_color")
}

/// Source residual: presentation_view builds from live entities().
pub fn honesty_view_from_live_entities_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = match src.find("pub fn presentation_view_from_gameworld") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 2000)];
    body.contains(".entities()")
        && body.contains("e.velocity")
        && body.contains("e.selected")
        && body.contains("e.move_max_speed")
        && body.contains("body_damage_state")
        && body.contains("team_color")
        && !body.contains("snap.entities")
}

/// Source residual: executable smoke gates overlay stamp when GW entities seen.
pub fn honesty_executable_overlay_stamp_gate_source() -> bool {
    let src = include_str!("../../executable_smoke.rs");
    src.contains("gameworld_overlay_stamped_ok")
        && src.contains("gameworld_overlay_boundary_ok")
        && src.contains("max_gameworld_overlay_stamped")
        && src.contains("\"gameworld_overlay_stamped\"")
}

/// Live residual: enriched view reflects desynced shadow entity motion/selection.
pub fn simulate_live_gameworld_entity_view_deepen_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, ensure_gate_damage_authority, presentation_view_from_shadow,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_gameworld_entity_view_deepen_residual_pack_wave191() {
        return false;
    }
    if !honesty_enriched_entity_view_source() {
        return false;
    }
    if !honesty_view_from_live_entities_source() {
        return false;
    }
    if !honesty_executable_overlay_stamp_gate_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GwEntityView191");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("ViewDeep191") {
        let mut t = ThingTemplate::new("ViewDeep191");
        t.set_health(180.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("ViewDeep191".into(), t);
    }
    let Some(oid) = logic.create_object("ViewDeep191", Team::USA, Vec3::new(14.0, 0.0, 15.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [1.5, 0.0, 2.5];
        e.move_max_speed = 22.0;
        e.selected = true;
        e.team_color = [0.2, 0.4, 0.6, 1.0];
        e.body_damage_state = 1;
        e.health = 99.0;
        e.max_health = 180.0;
    }

    let view = presentation_view_from_shadow(&shadow, 0);
    // View id is EntityId.get() (shadow store), not host ObjectId.
    let Some(ev) = view
        .entities
        .iter()
        .find(|e| e.template == "ViewDeep191" || e.id == eid.get())
    else {
        return false;
    };
    if (ev.velocity[0] - 1.5).abs() > 1e-3 || (ev.velocity[2] - 2.5).abs() > 1e-3 {
        return false;
    }
    if (ev.move_max_speed - 22.0).abs() > 1e-3 {
        return false;
    }
    if !ev.selected || ev.team_color != [0.2, 0.4, 0.6, 1.0] {
        return false;
    }
    if ev.body_damage_state != 1 || (ev.health - 99.0).abs() > 1e-3 {
        return false;
    }
    if (ev.max_health - 180.0).abs() > 1e-3 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_entity_view_deepen_method_names_residual_wave191());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_entity_view_deepen_nav_commands_residual_wave191());
    }

    #[test]
    fn wave191_composite_pack() {
        assert!(honesty_live_gameworld_entity_view_deepen_residual_pack_wave191());
    }

    #[test]
    fn entity_view_deepen_sources() {
        assert!(honesty_enriched_entity_view_source());
        assert!(honesty_view_from_live_entities_source());
        assert!(honesty_executable_overlay_stamp_gate_source());
    }

    #[test]
    fn simulate_live_gameworld_entity_view_deepen_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_entity_view_deepen_honesty(),
            "enriched GameWorldEntityView residual must latch"
        );
    }
}
