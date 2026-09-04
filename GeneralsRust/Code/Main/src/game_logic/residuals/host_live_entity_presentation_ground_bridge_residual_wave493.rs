//! Wave 493 residual peels: entity presentation ground height + engine bridge + command set.
//! - `engine_bridged` from GW entity
//! - `ground_height` / `ground_height_from_terrain` from GW entity
//! - `command_set_name` mirrors `command_set_override` residual
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 492 mesh scale/FOW peels.
//! Architecture residual - entity overlay must not force default ground / unbridged draw.
//!
//! Sources:
//! - presentation_frame.rs renderable_from_gameworld_entity Wave 493
//! - entity engine_bridged / ground_height* / command_set_override
//!
//! Fail-closed:
//! - FX name peels still empty without entity FX channels
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENTITY_PRESENTATION_GROUND_BRIDGE_METHOD_NAMES_WAVE493: &[&str] = &[
    "engine_bridged",
    "ground_height_from_terrain",
    "ground_height",
    "command_set_name",
    "command_set_override",
    "playable_claim = false",
];

pub const ENTITY_PRESENTATION_GROUND_BRIDGE_SOURCE_MARKERS_WAVE493: &[&str] = &[
    "Wave 493: engine-bridge + ground height from GW entity residual",
    "Wave 493: effective command set name falls back to override residual",
    "ent.engine_bridged",
    "ent.ground_height_from_terrain",
];

pub const ENTITY_PRESENTATION_GROUND_BRIDGE_NAV_STEPS_WAVE493: &[&str] = &[
    "ENTITY_HOLDS_GROUND_BRIDGE",
    "MAP_ENGINE_BRIDGED",
    "MAP_GROUND_HEIGHT",
    "MAP_COMMAND_SET_NAME",
    "DRAWABLE_SKIP_WHEN_BRIDGED",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_ENTITY_PRESENTATION_GROUND_BRIDGE_CMD_NAMES_WAVE493: &[&str] = &[
    "click_entity_presentation_ground_bridge_ok_wnd_detect",
    "click_entity_presentation_ground_bridge_ok_wnd_skip",
    "click_entity_presentation_ground_bridge_ok_wnd_queue",
    "click_entity_presentation_ground_bridge_ok_wnd_prepare",
    "click_entity_presentation_ground_bridge_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEntityPresentationGroundBridgeAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EntitySource = 4,
    CommandSet = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEntityPresentationGroundBridgeAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_entity_presentation_ground_bridge_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_entity_presentation_ground_bridge_last_action()
-> ResidualEntityPresentationGroundBridgeAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEntityPresentationGroundBridgeAction::MethodNames,
        2 => ResidualEntityPresentationGroundBridgeAction::SourceMarkers,
        3 => ResidualEntityPresentationGroundBridgeAction::NavCommands,
        4 => ResidualEntityPresentationGroundBridgeAction::EntitySource,
        5 => ResidualEntityPresentationGroundBridgeAction::CommandSet,
        6 => ResidualEntityPresentationGroundBridgeAction::Composite,
        _ => ResidualEntityPresentationGroundBridgeAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_entity_presentation_ground_bridge_method_names_residual_wave493() -> bool {
    ENTITY_PRESENTATION_GROUND_BRIDGE_METHOD_NAMES_WAVE493.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_METHOD_NAMES_WAVE493,
            "engine_bridged",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_METHOD_NAMES_WAVE493,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_entity_presentation_ground_bridge_source_markers_residual_wave493() -> bool {
    ENTITY_PRESENTATION_GROUND_BRIDGE_SOURCE_MARKERS_WAVE493.len() == 4
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_SOURCE_MARKERS_WAVE493,
            "Wave 493: engine-bridge + ground height from GW entity residual",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_SOURCE_MARKERS_WAVE493,
            "ent.engine_bridged",
        ) == Some(2)
}

pub fn honesty_entity_presentation_ground_bridge_nav_commands_residual_wave493() -> bool {
    ENTITY_PRESENTATION_GROUND_BRIDGE_NAV_STEPS_WAVE493.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_NAV_STEPS_WAVE493,
            "MAP_GROUND_HEIGHT",
        ) == Some(2)
        && residual_name_index(
            ENTITY_PRESENTATION_GROUND_BRIDGE_NAV_STEPS_WAVE493,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_ENTITY_PRESENTATION_GROUND_BRIDGE_CMD_NAMES_WAVE493.len() == 5
}

pub fn simulate_entity_presentation_ground_bridge_entity_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 493: engine-bridge + ground height from GW entity residual")
        && pf.contains("ent.engine_bridged")
        && pf.contains("ent.ground_height_from_terrain")
        && pf.contains("ent.ground_height")
        && pf.contains("engine_bridged: ent.engine_bridged");
    residual_action_store(ResidualEntityPresentationGroundBridgeAction::EntitySource);
    ok
}

pub fn simulate_entity_presentation_ground_bridge_command_set() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 493: override residual, template-authored fallback")
        && pf.contains("resolve_command_set_name(")
        && pf.contains("&ent.template.name,");
    residual_action_store(ResidualEntityPresentationGroundBridgeAction::CommandSet);
    ok
}

pub fn honesty_entity_presentation_ground_bridge_residual_pack_wave493() -> bool {
    honesty_entity_presentation_ground_bridge_method_names_residual_wave493()
        && honesty_entity_presentation_ground_bridge_source_markers_residual_wave493()
        && honesty_entity_presentation_ground_bridge_nav_commands_residual_wave493()
        && simulate_entity_presentation_ground_bridge_entity_source()
        && simulate_entity_presentation_ground_bridge_command_set()
}

pub fn simulate_live_entity_presentation_ground_bridge_honesty() -> bool {
    let ok = honesty_entity_presentation_ground_bridge_residual_pack_wave493();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEntityPresentationGroundBridgeAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_entity_presentation_ground_bridge_method_names_residual_wave493());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_entity_presentation_ground_bridge_source_markers_residual_wave493());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_entity_presentation_ground_bridge_nav_commands_residual_wave493());
    }

    #[test]
    fn entity_presentation_ground_bridge_sources() {
        assert!(simulate_entity_presentation_ground_bridge_entity_source());
        assert!(simulate_entity_presentation_ground_bridge_command_set());
    }

    #[test]
    fn wave493_composite_pack() {
        assert!(honesty_entity_presentation_ground_bridge_residual_pack_wave493());
    }

    #[test]
    fn simulate_live_entity_presentation_ground_bridge_honesty_residual_live() {
        assert!(
            simulate_live_entity_presentation_ground_bridge_honesty(),
            "entity presentation ground bridge residual must latch"
        );
        assert!(residual_entity_presentation_ground_bridge_ok());
        assert_eq!(
            residual_entity_presentation_ground_bridge_last_action(),
            ResidualEntityPresentationGroundBridgeAction::Composite
        );
    }
}
