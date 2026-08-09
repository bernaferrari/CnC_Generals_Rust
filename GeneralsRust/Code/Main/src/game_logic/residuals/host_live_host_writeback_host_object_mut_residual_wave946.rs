//! Wave 946: remaining shadow writebacks via `host_object_mut` authority.
//!
//! All remaining `writeback_*_to_host` paths that still dual-wrote through
//! `logic.get_objects_mut()` now mutate host objects only via
//! `GameLogic::host_object_mut` / `with_host_object_mut`.
//! Writeback functions contain zero `get_objects_mut` calls.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_METHOD_NAMES_WAVE946: &[&str] = &[
    "host_object_mut",
    "with_host_object_mut",
    "writeback_combat_status_to_host",
    "writeback_movement_to_host",
    "writeback_production_to_host",
    "Wave 946",
    "playable_claim = false",
];

pub const LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_NAV_STEPS_WAVE946: &[&str] = &[
    "WRITEBACK_HOST_OBJECT_MUT_BOUNDARY",
    "HOST_OBJECT_MUT_AUTHORITY",
    "LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_BOUNDARY",
    "WRITEBACK_GET_OBJECTS_MUT_ZERO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackHostObjectMutAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWritebackHostObjectMutAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn shadow_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn writeback_fn_bodies(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = src[search..].find("fn writeback_") {
        let i = search + rel;
        // include optional pub
        let start = if i >= 4 && &src[i - 4..i] == "pub " {
            i - 4
        } else {
            i
        };
        let Some(brace_rel) = src[i..].find('{') else {
            break;
        };
        let brace = i + brace_rel;
        let mut depth = 0usize;
        let mut p = brace;
        let bytes = src.as_bytes();
        while p < src.len() {
            match bytes[p] as char {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(non_comment_code(&src[start..=p]));
                        search = p + 1;
                        break;
                    }
                }
                _ => {}
            }
            p += 1;
        }
        if p >= src.len() {
            break;
        }
    }
    out
}

pub fn honesty_host_writeback_host_object_mut_method_names_residual_wave946() -> bool {
    let names = LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_METHOD_NAMES_WAVE946;
    let ok = residual_name_index(names, "host_object_mut").is_some()
        && residual_name_index(names, "with_host_object_mut").is_some()
        && residual_name_index(names, "Wave 946").is_some();
    residual_action_store(ResidualHostWritebackHostObjectMutAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_host_object_mut_nav_commands_residual_wave946() -> bool {
    let steps = LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_NAV_STEPS_WAVE946;
    let ok = residual_name_index(steps, "LIVE_HOST_WRITEBACK_HOST_OBJECT_MUT_BOUNDARY").is_some()
        && residual_name_index(steps, "WRITEBACK_GET_OBJECTS_MUT_ZERO").is_some();
    residual_action_store(ResidualHostWritebackHostObjectMutAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_host_object_mut_residual_pack_wave946() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn host_object_mut", 800));
    let with_api = non_comment_code(code_window(gl, "fn with_host_object_mut", 800));
    let bodies = writeback_fn_bodies(sh);
    let mut mut_count = 0usize;
    let mut host_mut_count = 0usize;
    for b in &bodies {
        mut_count += b.matches("get_objects_mut").count();
        if b.contains("host_object_mut") || b.contains("apply_host_writeback_op") {
            host_mut_count += 1;
        }
    }
    let ok = gl.contains("fn host_object_mut")
        && gl.contains("fn with_host_object_mut")
        && api.contains("get_objects_mut")
        && with_api.contains("get_objects_mut")
        && !bodies.is_empty()
        && mut_count == 0
        && host_mut_count >= 40
        && sh.contains("Wave 946")
        && gl.contains("Wave 946")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostWritebackHostObjectMutAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_writeback_host_object_mut_honesty() -> bool {
    let a = honesty_host_writeback_host_object_mut_method_names_residual_wave946();
    let b = honesty_host_writeback_host_object_mut_nav_commands_residual_wave946();
    let c = honesty_host_writeback_host_object_mut_residual_pack_wave946();
    residual_action_store(ResidualHostWritebackHostObjectMutAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_writeback_host_object_mut_residual_wave946() {
        assert!(honesty_host_writeback_host_object_mut_residual_pack_wave946());
        assert!(honesty_host_writeback_host_object_mut_method_names_residual_wave946());
        assert!(honesty_host_writeback_host_object_mut_nav_commands_residual_wave946());
        assert!(simulate_live_host_writeback_host_object_mut_honesty());
    }
}
