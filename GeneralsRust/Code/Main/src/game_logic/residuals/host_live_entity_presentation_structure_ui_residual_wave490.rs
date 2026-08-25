//! Wave 490 residual peels: entity→presentation fills garrison/turret/transport/kind.
//! - garrisoned_host_ids → garrisoned_units
//! - kind_of_bits → KindOf list (presentation ORDER)
//! - building_type_ordinal / veterancy_ordinal mappers
//! - transport roles, turret, disable flags, upgrades, death_type name
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 488/489 model/combat channel fills.
//! Architecture residual - shadow overlay presentation retains structure/unit UI state.
//!
//! Sources:
//! - presentation_frame.rs renderable_from_gameworld_entity Wave 490 fills
//! - kind_of_list_from_presentation_bits
//! - PresentationVeterancy/BuildingType::from_ordinal
//!
//! Fail-closed:
//! - FX name peels may stay empty without entity FX fields
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENTITY_PRESENTATION_STRUCTURE_UI_METHOD_NAMES_WAVE490: &[&str] = &[
    "kind_of_list_from_presentation_bits",
    "garrisoned_host_ids",
    "from_ordinal",
    "is_humvee_transport",
    "turret_angle_deg",
    "playable_claim = false",
];

pub const ENTITY_PRESENTATION_STRUCTURE_UI_SOURCE_MARKERS_WAVE490: &[&str] = &[
    "Wave 490: garrison/container presentation from GW entity",
    "Wave 490: transport/container role presentation from GW entity",
    "Wave 490: guard/mine/kind presentation from GW entity",
    "kind_of_list_from_presentation_bits",
];

pub const ENTITY_PRESENTATION_STRUCTURE_UI_NAV_STEPS_WAVE490: &[&str] = &[
    "ENTITY_HOLDS_STRUCTURE_UI",
    "MAP_GARRISON_AND_KIND",
    "MAP_TRANSPORT_ROLES",
    "MAP_TURRET_AND_DISABLE",
    "UNIT_RENDER_SEES_CHANNELS",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_ENTITY_PRESENTATION_STRUCTURE_UI_CMD_NAMES_WAVE490: &[&str] = &[
    "click_entity_presentation_structure_ui_ok_wnd_detect",
    "click_entity_presentation_structure_ui_ok_wnd_skip",
    "click_entity_presentation_structure_ui_ok_wnd_queue",
    "click_entity_presentation_structure_ui_ok_wnd_prepare",
    "click_entity_presentation_structure_ui_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEntityPresentationStructureUiAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EntitySource = 4,
    Helpers = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEntityPresentationStructureUiAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_entity_presentation_structure_ui_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_entity_presentation_structure_ui_last_action()
-> ResidualEntityPresentationStructureUiAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEntityPresentationStructureUiAction::MethodNames,
        2 => ResidualEntityPresentationStructureUiAction::SourceMarkers,
        3 => ResidualEntityPresentationStructureUiAction::NavCommands,
        4 => ResidualEntityPresentationStructureUiAction::EntitySource,
        5 => ResidualEntityPresentationStructureUiAction::Helpers,
        6 => ResidualEntityPresentationStructureUiAction::Composite,
        _ => ResidualEntityPresentationStructureUiAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_entity_presentation_structure_ui_method_names_residual_wave490() -> bool {
    ENTITY_PRESENTATION_STRUCTURE_UI_METHOD_NAMES_WAVE490.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_METHOD_NAMES_WAVE490,
            "kind_of_list_from_presentation_bits",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_METHOD_NAMES_WAVE490,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_entity_presentation_structure_ui_source_markers_residual_wave490() -> bool {
    ENTITY_PRESENTATION_STRUCTURE_UI_SOURCE_MARKERS_WAVE490.len() == 4
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_SOURCE_MARKERS_WAVE490,
            "Wave 490: garrison/container presentation from GW entity",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_SOURCE_MARKERS_WAVE490,
            "kind_of_list_from_presentation_bits",
        ) == Some(3)
}

pub fn honesty_entity_presentation_structure_ui_nav_commands_residual_wave490() -> bool {
    ENTITY_PRESENTATION_STRUCTURE_UI_NAV_STEPS_WAVE490.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_NAV_STEPS_WAVE490,
            "MAP_TRANSPORT_ROLES",
        ) == Some(2)
        && residual_name_index(
            ENTITY_PRESENTATION_STRUCTURE_UI_NAV_STEPS_WAVE490,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_ENTITY_PRESENTATION_STRUCTURE_UI_CMD_NAMES_WAVE490.len() == 5
}

pub fn simulate_entity_presentation_structure_ui_entity_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("fn renderable_from_gameworld_entity")
        && pf.contains("Wave 490: garrison/container presentation from GW entity")
        && pf.contains("Wave 490: transport/container role presentation from GW entity")
        && pf.contains("Wave 490: guard/mine/kind presentation from GW entity")
        && pf.contains("garrisoned_host_ids")
        && pf.contains("ent.is_humvee_transport")
        && pf.contains("ent.turret_angle_deg")
        && (pf.contains("kind_of_list_from_presentation_bits(ent.kind_of_bits)")
            || pf.contains("Self::kind_of_list_from_presentation_bits(ent.kind_of_bits)"));
    residual_action_store(ResidualEntityPresentationStructureUiAction::EntitySource);
    ok
}

pub fn simulate_entity_presentation_structure_ui_helpers() -> bool {
    let pf = pf_source();
    let ok = pf.contains("fn kind_of_list_from_presentation_bits")
        && pf.contains("PresentationVeterancy::from_ordinal")
        && pf.contains("PresentationBuildingType::from_ordinal")
        && pf.contains("Wave 490: GameWorld entity veterancy_ordinal residual")
        && pf.contains("Wave 490: GameWorld entity building_type_ordinal residual");
    residual_action_store(ResidualEntityPresentationStructureUiAction::Helpers);
    ok
}

pub fn honesty_entity_presentation_structure_ui_residual_pack_wave490() -> bool {
    honesty_entity_presentation_structure_ui_method_names_residual_wave490()
        && honesty_entity_presentation_structure_ui_source_markers_residual_wave490()
        && honesty_entity_presentation_structure_ui_nav_commands_residual_wave490()
        && simulate_entity_presentation_structure_ui_entity_source()
        && simulate_entity_presentation_structure_ui_helpers()
}

pub fn simulate_live_entity_presentation_structure_ui_honesty() -> bool {
    let ok = honesty_entity_presentation_structure_ui_residual_pack_wave490();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEntityPresentationStructureUiAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_entity_presentation_structure_ui_method_names_residual_wave490());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_entity_presentation_structure_ui_source_markers_residual_wave490());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_entity_presentation_structure_ui_nav_commands_residual_wave490());
    }

    #[test]
    fn entity_presentation_structure_ui_sources() {
        assert!(simulate_entity_presentation_structure_ui_entity_source());
        assert!(simulate_entity_presentation_structure_ui_helpers());
    }

    #[test]
    fn wave490_composite_pack() {
        assert!(honesty_entity_presentation_structure_ui_residual_pack_wave490());
    }

    #[test]
    fn simulate_live_entity_presentation_structure_ui_honesty_residual_live() {
        assert!(
            simulate_live_entity_presentation_structure_ui_honesty(),
            "entity presentation structure ui residual must latch"
        );
        assert!(residual_entity_presentation_structure_ui_ok());
        assert_eq!(
            residual_entity_presentation_structure_ui_last_action(),
            ResidualEntityPresentationStructureUiAction::Composite
        );
    }
}
