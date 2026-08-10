#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

// ---------------------------------------------------------------------------
// Wave 157 residual: presentation boundary honesty peels
// ---------------------------------------------------------------------------

/// Residual: last presentation-boundary residual action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualPresentationBoundaryAction {
    None = 0,
    SourceExecute = 1,
    SourceCollect = 2,
    SourceFallbackCounter = 3,
    SourceCncExecute = 4,
    Composite = 5,
}

pub(super) static RESIDUAL_PB_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub(super) static RESIDUAL_PB_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn residual_pb_action_store(action: ResidualPresentationBoundaryAction) {
    RESIDUAL_PB_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last presentation-boundary residual action.
pub fn residual_presentation_boundary_last_action() -> ResidualPresentationBoundaryAction {
    match RESIDUAL_PB_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualPresentationBoundaryAction::SourceExecute,
        2 => ResidualPresentationBoundaryAction::SourceCollect,
        3 => ResidualPresentationBoundaryAction::SourceFallbackCounter,
        4 => ResidualPresentationBoundaryAction::SourceCncExecute,
        5 => ResidualPresentationBoundaryAction::Composite,
        _ => ResidualPresentationBoundaryAction::None,
    }
}

/// Residual: composite honesty latch.
pub fn residual_presentation_boundary_ok() -> bool {
    RESIDUAL_PB_OK.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: `execute` must not take live `&GameLogic`.
pub fn simulate_presentation_boundary_execute_source() -> bool {
    let src = include_str!("../render_pipeline.rs");
    let exec_at = match src.find("pub fn execute(") {
        Some(i) => i,
        None => return false,
    };
    let window = &src[exec_at..src.len().min(exec_at + 500)];
    let ok = !window.contains("game_logic: Option<&GameLogic>")
        && !window.contains("game_logic: &GameLogic")
        && !window.contains("game_logic: &mut GameLogic");
    residual_pb_action_store(ResidualPresentationBoundaryAction::SourceExecute);
    ok
}

/// Residual: collect_render_items must prefer presentation unit inputs.
pub fn simulate_presentation_boundary_collect_source() -> bool {
    let src = include_str!("../render_pipeline.rs");
    let at = match src.find("fn collect_render_items(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[at..src.len().min(at + 2500)];
    let ok = body.contains("presentation_frame")
        && body.contains("unit_render_inputs")
        && body.contains("debug_last_presentation_live_fallback_reads = 0");
    residual_pb_action_store(ResidualPresentationBoundaryAction::SourceCollect);
    ok
}

/// Residual: live-fallback honesty counter API present.
pub fn simulate_presentation_boundary_fallback_counter_source() -> bool {
    let src = include_str!("../render_pipeline.rs");
    let ok = src.contains("debug_last_presentation_live_fallback_reads")
        && src.contains("last_presentation_live_fallback_reads")
        && src.contains("presentation_live_fallback_honesty_ok");
    residual_pb_action_store(ResidualPresentationBoundaryAction::SourceFallbackCounter);
    ok
}

/// Residual: engine execute call site is presentation-only (no GameLogic arg).
pub fn simulate_presentation_boundary_cnc_execute_source() -> bool {
    let cnc = include_str!("../../cnc_game_engine.rs");
    // Prefer the real call site: `self.render_pipeline.execute(` with args.
    let at = cnc
        .match_indices("self.render_pipeline.execute(")
        .map(|(i, _)| i)
        .find(|&i| {
            let window = &cnc[i..cnc.len().min(i + 200)];
            window.contains("view_matrix") || window.contains("&mut self.graphics_system")
        });
    let Some(at) = at else {
        return false;
    };
    let window = &cnc[at..cnc.len().min(at + 450)];
    let ok = !window.contains("game_logic") && !window.contains("&self.game_logic");
    residual_pb_action_store(ResidualPresentationBoundaryAction::SourceCncExecute);
    ok
}

/// Residual: composite presentation boundary honesty pack (source residual).
pub fn simulate_presentation_boundary_prepare_honesty() -> bool {
    let a = simulate_presentation_boundary_execute_source();
    let b = simulate_presentation_boundary_collect_source();
    let c = simulate_presentation_boundary_fallback_counter_source();
    let d = simulate_presentation_boundary_cnc_execute_source();
    let ok = a && b && c && d;
    RESIDUAL_PB_OK.store(ok, std::sync::atomic::Ordering::Relaxed);
    residual_pb_action_store(ResidualPresentationBoundaryAction::Composite);
    ok
}
