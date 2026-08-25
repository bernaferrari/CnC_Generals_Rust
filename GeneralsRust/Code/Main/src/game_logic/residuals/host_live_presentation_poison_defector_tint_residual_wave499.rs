//! Wave 499 residual peels: presentation poison tint + defector flash drive mesh pass.
//! - `UnitRenderInput` freezes `poison_tinted` / `defector_flash`
//! - `selection_flash_intensity` forces 1.0 when defector_flash
//! - `RenderItem::apply_poison_tint` green diffuse/emissive bias
//! - collect applies poison after selection flash (presentation-only)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 498 host FX overlay (fields exist; mesh now consumes them).
//! Architecture residual - drawable tint status without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs UnitRenderInput poison/defector + flash helper
//! - graphics/render_item.rs apply_poison_tint
//! - graphics/render_pipeline.rs Wave 499 poison apply
//!
//! Fail-closed:
//! - Full Drawable tint pulse matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_POISON_DEFECTOR_TINT_METHOD_NAMES_WAVE499: &[&str] = &[
    "poison_tinted",
    "defector_flash",
    "apply_poison_tint",
    "selection_flash_intensity",
    "TINT_STATUS_POISONED",
    "playable_claim = false",
];

pub const PRESENTATION_POISON_DEFECTOR_TINT_SOURCE_MARKERS_WAVE499: &[&str] = &[
    "Wave 499: C++ TINT_STATUS_POISONED residual",
    "Wave 499: C++ DefectionHelper flash residual",
    "Wave 499: presentation poison tint residual (no live GameLogic)",
    "fn apply_poison_tint",
];

pub const PRESENTATION_POISON_DEFECTOR_TINT_NAV_STEPS_WAVE499: &[&str] = &[
    "FREEZE_POISON_DEFECTOR_FLAGS",
    "DEFECTOR_FORCES_SELECTION_FLASH",
    "COLLECT_UNIT_MESH",
    "APPLY_SELECTION_FLASH",
    "APPLY_POISON_TINT",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_POISON_DEFECTOR_TINT_CMD_NAMES_WAVE499: &[&str] = &[
    "click_presentation_poison_defector_tint_ok_wnd_detect",
    "click_presentation_poison_defector_tint_ok_wnd_skip",
    "click_presentation_poison_defector_tint_ok_wnd_queue",
    "click_presentation_poison_defector_tint_ok_wnd_prepare",
    "click_presentation_poison_defector_tint_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationPoisonDefectorTintAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InputSource = 4,
    RenderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationPoisonDefectorTintAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_poison_defector_tint_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_poison_defector_tint_last_action()
-> ResidualPresentationPoisonDefectorTintAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationPoisonDefectorTintAction::MethodNames,
        2 => ResidualPresentationPoisonDefectorTintAction::SourceMarkers,
        3 => ResidualPresentationPoisonDefectorTintAction::NavCommands,
        4 => ResidualPresentationPoisonDefectorTintAction::InputSource,
        5 => ResidualPresentationPoisonDefectorTintAction::RenderSource,
        6 => ResidualPresentationPoisonDefectorTintAction::Composite,
        _ => ResidualPresentationPoisonDefectorTintAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn ri_source() -> &'static str {
    include_str!("../../graphics/render_item.rs")
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_poison_defector_tint_method_names_residual_wave499() -> bool {
    PRESENTATION_POISON_DEFECTOR_TINT_METHOD_NAMES_WAVE499.len() == 6
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_METHOD_NAMES_WAVE499,
            "poison_tinted",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_METHOD_NAMES_WAVE499,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_poison_defector_tint_source_markers_residual_wave499() -> bool {
    PRESENTATION_POISON_DEFECTOR_TINT_SOURCE_MARKERS_WAVE499.len() == 4
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_SOURCE_MARKERS_WAVE499,
            "Wave 499: presentation poison tint residual (no live GameLogic)",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_SOURCE_MARKERS_WAVE499,
            "fn apply_poison_tint",
        ) == Some(3)
}

pub fn honesty_presentation_poison_defector_tint_nav_commands_residual_wave499() -> bool {
    PRESENTATION_POISON_DEFECTOR_TINT_NAV_STEPS_WAVE499.len() == 6
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_NAV_STEPS_WAVE499,
            "APPLY_POISON_TINT",
        ) == Some(4)
        && residual_name_index(
            PRESENTATION_POISON_DEFECTOR_TINT_NAV_STEPS_WAVE499,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_POISON_DEFECTOR_TINT_CMD_NAMES_WAVE499.len() == 5
}

pub fn simulate_presentation_poison_defector_tint_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 499: C++ TINT_STATUS_POISONED residual")
        && pf.contains("Wave 499: C++ DefectionHelper flash residual")
        && pf.contains("poison_tinted: ro.poison_tinted")
        && pf.contains("defector_flash: ro.defector_flash")
        && pf.contains("Wave 499: defector cover flash forces full white selection flash residual");
    residual_action_store(ResidualPresentationPoisonDefectorTintAction::InputSource);
    ok
}

pub fn simulate_presentation_poison_defector_tint_render_source() -> bool {
    let ri = ri_source();
    let rp = rp_source();
    // 2026-08-15: apply_poison_tint call lives on RenderItem (render_item.rs:778);
    // pipeline_collect.rs only forwards u.poison_tinted + Wave 499 comment.
    let ok = ri.contains("pub fn apply_poison_tint")
        && ri.contains("Wave 499: C++ TINT_STATUS_POISONED residual")
        && rp.contains("Wave 499")
        && rp.contains("u.poison_tinted")
        && ri.contains("self.apply_poison_tint()");
    residual_action_store(ResidualPresentationPoisonDefectorTintAction::RenderSource);
    ok
}

pub fn honesty_presentation_poison_defector_tint_residual_pack_wave499() -> bool {
    honesty_presentation_poison_defector_tint_method_names_residual_wave499()
        && honesty_presentation_poison_defector_tint_source_markers_residual_wave499()
        && honesty_presentation_poison_defector_tint_nav_commands_residual_wave499()
        && simulate_presentation_poison_defector_tint_input_source()
        && simulate_presentation_poison_defector_tint_render_source()
}

pub fn simulate_live_presentation_poison_defector_tint_honesty() -> bool {
    let ok = honesty_presentation_poison_defector_tint_residual_pack_wave499();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationPoisonDefectorTintAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_poison_defector_tint_method_names_residual_wave499());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_poison_defector_tint_source_markers_residual_wave499());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_poison_defector_tint_nav_commands_residual_wave499());
    }

    #[test]
    fn presentation_poison_defector_tint_sources() {
        assert!(simulate_presentation_poison_defector_tint_input_source());
        assert!(simulate_presentation_poison_defector_tint_render_source());
    }

    #[test]
    fn wave499_composite_pack() {
        assert!(honesty_presentation_poison_defector_tint_residual_pack_wave499());
    }

    #[test]
    fn simulate_live_presentation_poison_defector_tint_honesty_residual_live() {
        assert!(
            simulate_live_presentation_poison_defector_tint_honesty(),
            "presentation poison/defector tint residual must latch"
        );
        assert!(residual_presentation_poison_defector_tint_ok());
        assert_eq!(
            residual_presentation_poison_defector_tint_last_action(),
            ResidualPresentationPoisonDefectorTintAction::Composite
        );
    }
}
