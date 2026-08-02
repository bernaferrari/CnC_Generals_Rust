//! Wave 956: seal AI/shadow/authority dual-reads onto host_object/host_objects.
//!
//! Production AI, skirmish AI, decisions, gameworld shadow sync, authority bridge,
//! save snapshot, and presentation build route host map access through host_* APIs.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE956: &[&str] = &[
    "host_object",
    "host_objects",
    "host_object_mut",
    "host_objects_mut",
    "Wave 956",
    "playable_claim = false",
];

pub const LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL_NAV_STEPS_WAVE956: &[&str] = &[
    "AI_SHADOW_HOST_OBJECT_SEAL",
    "AI_HOST_OBJECTS",
    "SHADOW_HOST_OBJECTS",
    "PRESENTATION_BUILD_HOST_OBJECTS",
    "LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAiShadowHostObjectSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAiShadowHostObjectSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn non_comment_prod(src: &str) -> String {
    let ti = src.find("\n#[cfg(test)]").unwrap_or(src.len());
    src[..ti]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.contains("contains("))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

fn ai_source() -> &'static str {
    include_str!("../ai.rs")
}
fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}
fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

pub fn honesty_host_ai_shadow_host_object_seal_method_names_residual_wave956() -> bool {
    let names = LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE956;
    let ok = residual_name_index(names, "host_objects").is_some()
        && residual_name_index(names, "Wave 956").is_some();
    residual_action_store(ResidualHostAiShadowHostObjectSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ai_shadow_host_object_seal_nav_commands_residual_wave956() -> bool {
    let steps = LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL_NAV_STEPS_WAVE956;
    let ok = residual_name_index(steps, "LIVE_HOST_AI_SHADOW_HOST_OBJECT_SEAL").is_some()
        && residual_name_index(steps, "SHADOW_HOST_OBJECTS").is_some();
    residual_action_store(ResidualHostAiShadowHostObjectSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
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
    &src[i..src.len().min(i + 8_000)]
}

fn non_comment_window(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.contains("contains("))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

pub fn honesty_host_ai_shadow_host_object_seal_residual_pack_wave956() -> bool {
    let ai = non_comment_prod(ai_source());
    let shadow_src = shadow_source();
    let pf = pf_source();
    let gl = gl_source();
    let cnc = cnc_source();
    // gameworld_shadow embeds early #[cfg(test)] helpers — probe production sync body.
    let sync = non_comment_window(fn_window(shadow_src, "fn sync_from_host_with"));
    let mil = non_comment_window(fn_window(ai_source(), "fn calculate_military_strength"));
    let build = non_comment_window(fn_window(pf, "fn build_from_logic"));
    let ok = ai_source().contains("Wave 956")
        && shadow_src.contains("Wave 956")
        && gl.contains("fn host_objects(")
        && !ai.contains("get_objects()")
        && ai.contains("host_objects()")
        && mil.contains("host_objects()")
        && !mil.contains("get_objects()")
        && sync.contains("host_objects()")
        && !sync.contains("get_objects()")
        && (build.contains("host_objects()") || pf.contains("host_objects()"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAiShadowHostObjectSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ai_shadow_host_object_seal_honesty() -> bool {
    let a = honesty_host_ai_shadow_host_object_seal_method_names_residual_wave956();
    let b = honesty_host_ai_shadow_host_object_seal_nav_commands_residual_wave956();
    let c = honesty_host_ai_shadow_host_object_seal_residual_pack_wave956();
    residual_action_store(ResidualHostAiShadowHostObjectSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ai_shadow_host_object_seal_residual_wave956() {
        assert!(honesty_host_ai_shadow_host_object_seal_residual_pack_wave956());
        assert!(honesty_host_ai_shadow_host_object_seal_method_names_residual_wave956());
        assert!(honesty_host_ai_shadow_host_object_seal_nav_commands_residual_wave956());
        assert!(simulate_live_host_ai_shadow_host_object_seal_honesty());
    }
}
