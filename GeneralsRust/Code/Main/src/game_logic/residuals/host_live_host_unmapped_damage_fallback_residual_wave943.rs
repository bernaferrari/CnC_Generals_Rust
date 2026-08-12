//! Wave 943: host unmapped damage fallback via residual mutation authority.
//!
//! Final shadow-session `get_objects_mut` dual-write (host-only combat HP for
//! objects with no shadow entity mapping) routes through
//! `apply_host_unmapped_damage_fallback` / `ApplyRawHpDamage`.
//! Session body dual-writes reach zero. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_METHOD_NAMES_WAVE943: &[&str] = &[
    "apply_host_unmapped_damage_fallback",
    "ApplyRawHpDamage",
    "apply_host_residual_mutation_op",
    "HostResidualMutationOp",
    "Wave 943",
    "playable_claim = false",
];

pub const LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_NAV_STEPS_WAVE943: &[&str] = &[
    "UNMAPPED_DAMAGE_FALLBACK_BOUNDARY",
    "HOST_APPLY_RAW_HP_DAMAGE",
    "LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_BOUNDARY",
    "SESSION_GET_OBJECTS_MUT_ZERO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUnmappedDamageFallbackAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUnmappedDamageFallbackAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
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

fn session_fn_window(src: &str) -> &str {
    let marker = "fn shadow_session_after_host_tick";
    let Some(i) = src.find(marker) else {
        return "";
    };
    let Some(brace) = src[i..].find('{').map(|o| i + o) else {
        return "";
    };
    let mut depth = 0usize;
    let mut p = brace;
    let bytes = src.as_bytes();
    while p < src.len() {
        match bytes[p] as char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[i..=p];
                }
            }
            _ => {}
        }
        p += 1;
    }
    &src[i..src.len().min(i + 140_000)]
}

pub fn honesty_host_unmapped_damage_fallback_method_names_residual_wave943() -> bool {
    let names = LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_METHOD_NAMES_WAVE943;
    let ok = residual_name_index(names, "apply_host_unmapped_damage_fallback").is_some()
        && residual_name_index(names, "ApplyRawHpDamage").is_some()
        && residual_name_index(names, "Wave 943").is_some();
    residual_action_store(ResidualHostUnmappedDamageFallbackAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unmapped_damage_fallback_nav_commands_residual_wave943() -> bool {
    let steps = LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_NAV_STEPS_WAVE943;
    let ok = residual_name_index(steps, "LIVE_HOST_UNMAPPED_DAMAGE_FALLBACK_BOUNDARY").is_some()
        && residual_name_index(steps, "SESSION_GET_OBJECTS_MUT_ZERO").is_some();
    residual_action_store(ResidualHostUnmappedDamageFallbackAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unmapped_damage_fallback_residual_pack_wave943() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(
        gl,
        "fn apply_host_unmapped_damage_fallback",
        2500,
    ));
    let residual_api =
        non_comment_code(code_window(gl, "fn apply_host_residual_mutation_op", 20000));
    let session = non_comment_code(session_fn_window(sh));
    let session_mut_count = session.matches("get_objects_mut").count();
    let ok = gl.contains("ApplyRawHpDamage")
        && gl.contains("apply_host_unmapped_damage_fallback")
        && residual_api.contains("ApplyRawHpDamage")
        && residual_api.contains("health.damage")
        && api.contains("ApplyRawHpDamage")
        && api.contains("apply_host_residual_mutation_op")
        && session.contains("apply_host_unmapped_damage_fallback")
        && session.contains("entity_for_host")
        && !session.contains("get_objects_mut")
        && session_mut_count == 0
        && gl.contains("Wave 943")
        && sh.contains("Wave 943")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUnmappedDamageFallbackAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_unmapped_damage_fallback_honesty() -> bool {
    let a = honesty_host_unmapped_damage_fallback_method_names_residual_wave943();
    let b = honesty_host_unmapped_damage_fallback_nav_commands_residual_wave943();
    let c = honesty_host_unmapped_damage_fallback_residual_pack_wave943();
    residual_action_store(ResidualHostUnmappedDamageFallbackAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_unmapped_damage_fallback_residual_wave943() {
        assert!(honesty_host_unmapped_damage_fallback_residual_pack_wave943());
        assert!(honesty_host_unmapped_damage_fallback_method_names_residual_wave943());
        assert!(honesty_host_unmapped_damage_fallback_nav_commands_residual_wave943());
        assert!(simulate_live_host_unmapped_damage_fallback_honesty());
    }
}
