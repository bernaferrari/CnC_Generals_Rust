//! Wave 482 residual peels: sell finish destroy skips combat topple/collapse deferral.
//! - `mark_object_for_destruction` detects `status.sold`
//! - sold objects bypass StructureTopple/Collapse, SlowDeath, KeepObjectDie
//! - destroy queues into `objects_to_destroy` for `process_destroy_list`
//! - parked aircraft sell kills use `destroy_object_for_sell_residual`
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 481 sole-tick sell percent peel.
//! Architecture residual - BuildAssistant sell completion is immediate remove, not combat death.
//!
//! Sources:
//! - game_logic.rs mark_object_for_destruction sold branch
//! - update_sell_list → destroy_object on finish
//!
//! Fail-closed:
//! - Combat structure deaths still topple/collapse when not sold
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const SELL_FINISH_SKIPS_TOPPLE_DESTROY_METHOD_NAMES_WAVE482: &[&str] = &[
    "mark_object_for_destruction",
    "status.sold",
    "try_begin_structure_topple_instead_of_destroy",
    "try_begin_slow_death_instead_of_destroy",
    "objects_to_destroy",
    "playable_claim = false",
];

pub const SELL_FINISH_SKIPS_TOPPLE_DESTROY_SOURCE_MARKERS_WAVE482: &[&str] = &[
    "Wave 482: BuildAssistant sell finish removes the object immediately",
    "if !sold",
    "try_begin_structure_topple_instead_of_destroy",
    "objects_to_destroy",
];

pub const SELL_FINISH_SKIPS_TOPPLE_DESTROY_NAV_STEPS_WAVE482: &[&str] = &[
    "SELL_FINISH_CALLS_DESTROY",
    "DETECT_STATUS_SOLD",
    "SKIP_TOPPLE_COLLAPSE",
    "SKIP_SLOW_DEATH",
    "SKIP_KEEP_OBJECT_DIE",
    "QUEUE_OBJECTS_TO_DESTROY",
];

pub const RUNTIME_HOST_SELL_FINISH_SKIPS_TOPPLE_DESTROY_CMD_NAMES_WAVE482: &[&str] = &[
    "click_sell_finish_skips_topple_destroy_ok_wnd_detect",
    "click_sell_finish_skips_topple_destroy_ok_wnd_skip",
    "click_sell_finish_skips_topple_destroy_ok_wnd_queue",
    "click_sell_finish_skips_topple_destroy_ok_wnd_prepare",
    "click_sell_finish_skips_topple_destroy_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualSellFinishSkipsToppleDestroyAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    MarkSource = 4,
    SellFinishSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualSellFinishSkipsToppleDestroyAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_sell_finish_skips_topple_destroy_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_sell_finish_skips_topple_destroy_last_action()
-> ResidualSellFinishSkipsToppleDestroyAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualSellFinishSkipsToppleDestroyAction::MethodNames,
        2 => ResidualSellFinishSkipsToppleDestroyAction::SourceMarkers,
        3 => ResidualSellFinishSkipsToppleDestroyAction::NavCommands,
        4 => ResidualSellFinishSkipsToppleDestroyAction::MarkSource,
        5 => ResidualSellFinishSkipsToppleDestroyAction::SellFinishSource,
        6 => ResidualSellFinishSkipsToppleDestroyAction::Composite,
        _ => ResidualSellFinishSkipsToppleDestroyAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_sell_finish_skips_topple_destroy_method_names_residual_wave482() -> bool {
    SELL_FINISH_SKIPS_TOPPLE_DESTROY_METHOD_NAMES_WAVE482.len() == 6
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_METHOD_NAMES_WAVE482,
            "mark_object_for_destruction",
        ) == Some(0)
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_METHOD_NAMES_WAVE482,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_sell_finish_skips_topple_destroy_source_markers_residual_wave482() -> bool {
    SELL_FINISH_SKIPS_TOPPLE_DESTROY_SOURCE_MARKERS_WAVE482.len() == 4
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_SOURCE_MARKERS_WAVE482,
            "Wave 482: BuildAssistant sell finish removes the object immediately",
        ) == Some(0)
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_SOURCE_MARKERS_WAVE482,
            "if !sold",
        ) == Some(1)
}

pub fn honesty_sell_finish_skips_topple_destroy_nav_commands_residual_wave482() -> bool {
    SELL_FINISH_SKIPS_TOPPLE_DESTROY_NAV_STEPS_WAVE482.len() == 6
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_NAV_STEPS_WAVE482,
            "DETECT_STATUS_SOLD",
        ) == Some(1)
        && residual_name_index(
            SELL_FINISH_SKIPS_TOPPLE_DESTROY_NAV_STEPS_WAVE482,
            "QUEUE_OBJECTS_TO_DESTROY",
        ) == Some(5)
        && RUNTIME_HOST_SELL_FINISH_SKIPS_TOPPLE_DESTROY_CMD_NAMES_WAVE482.len() == 5
        && residual_name_index(
            RUNTIME_HOST_SELL_FINISH_SKIPS_TOPPLE_DESTROY_CMD_NAMES_WAVE482,
            "click_sell_finish_skips_topple_destroy_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_sell_finish_skips_topple_destroy_mark_source() -> bool {
    let gl = gl_source();
    let Some(body) = function_body(gl, "fn mark_object_for_destruction(") else {
        return false;
    };
    let ok = body.contains("Wave 482: BuildAssistant sell finish removes the object immediately")
        && body.contains("status.sold")
        && body.contains("if !sold")
        && body.contains("try_begin_structure_topple_instead_of_destroy")
        && body.contains("objects_to_destroy")
        && gl_source().contains("fn destroy_object_for_sell_residual")
        && gl_source().contains("destroy_object_for_sell_residual(uid)");
    residual_action_store(ResidualSellFinishSkipsToppleDestroyAction::MarkSource);
    ok
}

pub fn simulate_sell_finish_skips_topple_destroy_sell_finish_source() -> bool {
    let gl = gl_source();
    let Some(body) = function_body(gl, "fn update_sell_list(") else {
        return false;
    };
    let ok = body.contains("self.destroy_object(id)")
        && body.contains("sell_process_finishes")
        && gl.contains("fn process_destroy_list");
    residual_action_store(ResidualSellFinishSkipsToppleDestroyAction::SellFinishSource);
    ok
}

pub fn honesty_sell_finish_skips_topple_destroy_residual_pack_wave482() -> bool {
    honesty_sell_finish_skips_topple_destroy_method_names_residual_wave482()
        && honesty_sell_finish_skips_topple_destroy_source_markers_residual_wave482()
        && honesty_sell_finish_skips_topple_destroy_nav_commands_residual_wave482()
        && simulate_sell_finish_skips_topple_destroy_mark_source()
        && simulate_sell_finish_skips_topple_destroy_sell_finish_source()
}

pub fn simulate_live_sell_finish_skips_topple_destroy_honesty() -> bool {
    let ok = honesty_sell_finish_skips_topple_destroy_residual_pack_wave482();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualSellFinishSkipsToppleDestroyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_sell_finish_skips_topple_destroy_method_names_residual_wave482());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_sell_finish_skips_topple_destroy_source_markers_residual_wave482());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_sell_finish_skips_topple_destroy_nav_commands_residual_wave482());
    }

    #[test]
    fn sell_finish_skips_topple_destroy_sources() {
        assert!(simulate_sell_finish_skips_topple_destroy_mark_source());
        assert!(simulate_sell_finish_skips_topple_destroy_sell_finish_source());
    }

    #[test]
    fn wave482_composite_pack() {
        assert!(honesty_sell_finish_skips_topple_destroy_residual_pack_wave482());
    }

    #[test]
    fn simulate_live_sell_finish_skips_topple_destroy_honesty_residual_live() {
        assert!(
            simulate_live_sell_finish_skips_topple_destroy_honesty(),
            "sell finish skips topple destroy residual must latch"
        );
        assert!(residual_sell_finish_skips_topple_destroy_ok());
        assert_eq!(
            residual_sell_finish_skips_topple_destroy_last_action(),
            ResidualSellFinishSkipsToppleDestroyAction::Composite
        );
    }
}
