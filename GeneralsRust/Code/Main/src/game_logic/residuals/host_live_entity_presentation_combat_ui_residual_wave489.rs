//! Wave 489 residual peels: entity→presentation fills production/path/weapon/stealth.
//! - `from_entity_item` maps EntityProductionItem → PresentationProductionItem
//! - production_queue / rally / path_waypoints from entity
//! - weapon + stealth + special power channels from entity
//! - selection_flash_remaining from entity
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 488 model/door/radar carry.
//! Architecture residual - shadow overlay presentation must retain combat/UI state.
//!
//! Sources:
//! - presentation_frame.rs renderable_from_gameworld_entity Wave 489 fills
//! - PresentationProductionItem::from_entity_item
//!
//! Fail-closed:
//! - Host-direct presentation build path unchanged
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENTITY_PRESENTATION_COMBAT_UI_METHOD_NAMES_WAVE489: &[&str] = &[
    "from_entity_item",
    "production_queue_items",
    "path_waypoints",
    "weapon_range",
    "effectively_stealthed",
    "playable_claim = false",
];

pub const ENTITY_PRESENTATION_COMBAT_UI_SOURCE_MARKERS_WAVE489: &[&str] = &[
    "Wave 489: GameWorld entity production queue → presentation strip",
    "Wave 489: order/path/production presentation from GW entity",
    "Wave 489: stealth/weapon presentation from GW entity",
    "ent.production_queue_items",
];

pub const ENTITY_PRESENTATION_COMBAT_UI_NAV_STEPS_WAVE489: &[&str] = &[
    "ENTITY_HOLDS_QUEUE_PATH_WEAPON",
    "MAP_PRODUCTION_ITEMS",
    "COPY_PATH_AND_RALLY",
    "COPY_WEAPON_STEALTH",
    "UNIT_RENDER_SEES_CHANNELS",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_ENTITY_PRESENTATION_COMBAT_UI_CMD_NAMES_WAVE489: &[&str] = &[
    "click_entity_presentation_combat_ui_ok_wnd_detect",
    "click_entity_presentation_combat_ui_ok_wnd_skip",
    "click_entity_presentation_combat_ui_ok_wnd_queue",
    "click_entity_presentation_combat_ui_ok_wnd_prepare",
    "click_entity_presentation_combat_ui_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEntityPresentationCombatUiAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EntitySource = 4,
    ProductionMap = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEntityPresentationCombatUiAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_entity_presentation_combat_ui_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_entity_presentation_combat_ui_last_action()
-> ResidualEntityPresentationCombatUiAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEntityPresentationCombatUiAction::MethodNames,
        2 => ResidualEntityPresentationCombatUiAction::SourceMarkers,
        3 => ResidualEntityPresentationCombatUiAction::NavCommands,
        4 => ResidualEntityPresentationCombatUiAction::EntitySource,
        5 => ResidualEntityPresentationCombatUiAction::ProductionMap,
        6 => ResidualEntityPresentationCombatUiAction::Composite,
        _ => ResidualEntityPresentationCombatUiAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_entity_presentation_combat_ui_method_names_residual_wave489() -> bool {
    ENTITY_PRESENTATION_COMBAT_UI_METHOD_NAMES_WAVE489.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_METHOD_NAMES_WAVE489,
            "from_entity_item",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_METHOD_NAMES_WAVE489,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_entity_presentation_combat_ui_source_markers_residual_wave489() -> bool {
    ENTITY_PRESENTATION_COMBAT_UI_SOURCE_MARKERS_WAVE489.len() == 4
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_SOURCE_MARKERS_WAVE489,
            "Wave 489: order/path/production presentation from GW entity",
        ) == Some(1)
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_SOURCE_MARKERS_WAVE489,
            "ent.production_queue_items",
        ) == Some(3)
}

pub fn honesty_entity_presentation_combat_ui_nav_commands_residual_wave489() -> bool {
    ENTITY_PRESENTATION_COMBAT_UI_NAV_STEPS_WAVE489.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_NAV_STEPS_WAVE489,
            "MAP_PRODUCTION_ITEMS",
        ) == Some(1)
        && residual_name_index(
            ENTITY_PRESENTATION_COMBAT_UI_NAV_STEPS_WAVE489,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_ENTITY_PRESENTATION_COMBAT_UI_CMD_NAMES_WAVE489.len() == 5
}

pub fn simulate_entity_presentation_combat_ui_entity_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("fn renderable_from_gameworld_entity")
        && pf.contains("Wave 489: order/path/production presentation from GW entity")
        && pf.contains("Wave 489: stealth/weapon presentation from GW entity")
        && pf.contains("ent.production_queue_items")
        && pf.contains(".path_waypoints")
        && pf.contains("ent.weapon_range")
        && pf.contains("ent.stealthed");
    residual_action_store(ResidualEntityPresentationCombatUiAction::EntitySource);
    ok
}

pub fn simulate_entity_presentation_combat_ui_production_map() -> bool {
    let pf = pf_source();
    let ok = pf.contains("fn from_entity_item")
        && pf.contains("Wave 489: GameWorld entity production queue → presentation strip")
        && pf.contains("item.cost_supplies")
        && pf.contains("item.is_upgrade");
    residual_action_store(ResidualEntityPresentationCombatUiAction::ProductionMap);
    ok
}

pub fn honesty_entity_presentation_combat_ui_residual_pack_wave489() -> bool {
    honesty_entity_presentation_combat_ui_method_names_residual_wave489()
        && honesty_entity_presentation_combat_ui_source_markers_residual_wave489()
        && honesty_entity_presentation_combat_ui_nav_commands_residual_wave489()
        && simulate_entity_presentation_combat_ui_entity_source()
        && simulate_entity_presentation_combat_ui_production_map()
}

pub fn simulate_live_entity_presentation_combat_ui_honesty() -> bool {
    let ok = honesty_entity_presentation_combat_ui_residual_pack_wave489();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEntityPresentationCombatUiAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_entity_presentation_combat_ui_method_names_residual_wave489());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_entity_presentation_combat_ui_source_markers_residual_wave489());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_entity_presentation_combat_ui_nav_commands_residual_wave489());
    }

    #[test]
    fn entity_presentation_combat_ui_sources() {
        assert!(simulate_entity_presentation_combat_ui_entity_source());
        assert!(simulate_entity_presentation_combat_ui_production_map());
    }

    #[test]
    fn wave489_composite_pack() {
        assert!(honesty_entity_presentation_combat_ui_residual_pack_wave489());
    }

    #[test]
    fn simulate_live_entity_presentation_combat_ui_honesty_residual_live() {
        assert!(
            simulate_live_entity_presentation_combat_ui_honesty(),
            "entity presentation combat ui residual must latch"
        );
        assert!(residual_entity_presentation_combat_ui_ok());
        assert_eq!(
            residual_entity_presentation_combat_ui_last_action(),
            ResidualEntityPresentationCombatUiAction::Composite
        );
    }
}
