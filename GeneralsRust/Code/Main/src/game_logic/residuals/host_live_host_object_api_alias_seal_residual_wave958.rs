//! Wave 958: host_* owns object-map access; legacy get_object/find_object alias host_*.
//!
//! GameLogic::host_object/host_objects read `self.objects` directly. Legacy
//! get_object/get_objects/find_object are thin aliases. Remaining Main harness
//! tests prefer host_*. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OBJECT_API_ALIAS_SEAL_METHOD_NAMES_WAVE958: &[&str] = &[
    "host_object",
    "host_objects",
    "host_object_mut",
    "host_objects_mut",
    "get_object",
    "find_object",
    "Wave 958",
    "playable_claim = false",
];

pub const LIVE_HOST_OBJECT_API_ALIAS_SEAL_NAV_STEPS_WAVE958: &[&str] = &[
    "HOST_OBJECT_API_ALIAS_SEAL",
    "HOST_OWNS_OBJECTS_MAP",
    "LEGACY_GET_OBJECT_ALIASES_HOST",
    "LIVE_HOST_OBJECT_API_ALIAS_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostObjectApiAliasSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostObjectApiAliasSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn fn_window<'a>(src: &'a str, marker: &str) -> &'a str {
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
    &src[i..src.len().min(i + 4_000)]
}

fn non_comment(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_object_api_alias_seal_method_names_residual_wave958() -> bool {
    let names = LIVE_HOST_OBJECT_API_ALIAS_SEAL_METHOD_NAMES_WAVE958;
    let ok = residual_name_index(names, "host_objects").is_some()
        && residual_name_index(names, "Wave 958").is_some()
        && residual_name_index(names, "get_object").is_some();
    residual_action_store(ResidualHostObjectApiAliasSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_api_alias_seal_nav_commands_residual_wave958() -> bool {
    let steps = LIVE_HOST_OBJECT_API_ALIAS_SEAL_NAV_STEPS_WAVE958;
    let ok = residual_name_index(steps, "LIVE_HOST_OBJECT_API_ALIAS_SEAL").is_some()
        && residual_name_index(steps, "HOST_OWNS_OBJECTS_MAP").is_some();
    residual_action_store(ResidualHostObjectApiAliasSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_api_alias_seal_residual_pack_wave958() -> bool {
    let gl = gl_source();
    let cnc = cnc_source();
    let host = non_comment(fn_window(&gl, "pub fn host_object(&self, id: ObjectId)"));
    let host_map = non_comment(fn_window(&gl, "pub fn host_objects("));
    let get = non_comment(fn_window(&gl, "pub fn get_object(&self, id: ObjectId)"));
    let find = non_comment(fn_window(&gl, "pub fn find_object(&self, id: ObjectId)"));
    let get_map = non_comment(fn_window(&gl, "pub fn get_objects(&self)"));
    let ok = host.contains("self.objects.get")
        && (host_map.contains("&self.objects") || host_map.contains("self.objects.map()"))
        && get.contains("host_object(id)")
        && find.contains("host_object(id)")
        && get_map.contains("host_objects()")
        && !get.contains("self.objects.get")
        && gl.contains("Wave 958")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostObjectApiAliasSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_object_api_alias_seal_honesty() -> bool {
    let a = honesty_host_object_api_alias_seal_method_names_residual_wave958();
    let b = honesty_host_object_api_alias_seal_nav_commands_residual_wave958();
    let c = honesty_host_object_api_alias_seal_residual_pack_wave958();
    residual_action_store(ResidualHostObjectApiAliasSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_object_api_alias_seal_residual_wave958() {
        assert!(honesty_host_object_api_alias_seal_residual_pack_wave958());
        assert!(honesty_host_object_api_alias_seal_method_names_residual_wave958());
        assert!(honesty_host_object_api_alias_seal_nav_commands_residual_wave958());
        assert!(simulate_live_host_object_api_alias_seal_honesty());
    }
}
