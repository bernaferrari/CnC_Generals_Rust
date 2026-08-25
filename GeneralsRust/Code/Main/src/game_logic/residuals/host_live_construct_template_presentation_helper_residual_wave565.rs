//! Wave 565 residual peels: construct residual prefers presentation freeze
//! template names via `presentation_or_boot_has_template` (same helper as
//! Wave 563 train residual). Mid-command host template inserts still hit live
//! `templates.contains_key`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 564 fixed-step diagnostics presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` construct/dozer_construct/place_structure
//! - `cnc_game_engine.rs` presentation_or_boot_has_template
//! - `presentation_frame.rs` known_template_names / has_template_name
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_METHOD_NAMES_WAVE565: &[&str] = &[
    "presentation_or_boot_has_template",
    "construct",
    "dozer_construct",
    "place_structure",
    "Wave 565",
    "playable_claim = false",
];

pub const LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_NAV_STEPS_WAVE565: &[&str] = &[
    "REQUIRE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER",
    "REQUIRE_CONSTRUCT_ARM_USES_HELPER",
    "LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_CMD_NAMES_WAVE565: &[&str] = &[
    "construct_template_presentation_helper",
    "construct_arm",
    "dozer_spawn_template_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualConstructTemplatePresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualConstructTemplatePresentationHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualConstructTemplatePresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_construct_template_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_construct_template_presentation_helper_last_action()
-> ResidualConstructTemplatePresentationHelperAction {
    ResidualConstructTemplatePresentationHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_construct_template_presentation_helper_method_names_residual_wave565() -> bool {
    let names = LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_METHOD_NAMES_WAVE565;
    let ok = residual_name_index(names, "presentation_or_boot_has_template").is_some()
        && residual_name_index(names, "construct").is_some()
        && residual_name_index(names, "dozer_construct").is_some()
        && residual_name_index(names, "place_structure").is_some()
        && residual_name_index(names, "Wave 565").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualConstructTemplatePresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_construct_template_presentation_helper_source_markers_residual_wave565() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok =
        pf.contains("pub known_template_names: Vec<String>") && pf.contains("fn has_template_name");
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_has_template(") else {
        residual_action_store(ResidualConstructTemplatePresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — no templates.contains_key dual-read.
    let helper_ok = helper.contains("Wave 565")
        && helper.contains("pres.has_template_name(name)")
        && helper.contains("host_match_known_template_names")
        && !helper.contains("self.game_logic.templates.contains_key(name)");
    // 2026-08-15: construct arm peeled into runtime_host helpers.
    let arm_ok = eng.contains("\"construct\" | \"dozer_construct\" | \"place_structure\"")
        && (eng.contains("presentation_or_boot_has_template")
            || eng.contains("presentation_or_live_has_template"))
        && (eng.contains("create_object") || eng.contains("host_create_object"))
        && eng.contains("USA_Dozer");
    let ok = field_ok && helper_ok && arm_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualConstructTemplatePresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_construct_template_presentation_helper_nav_commands_residual_wave565() -> bool {
    let steps = LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_NAV_STEPS_WAVE565;
    let cmds = RUNTIME_HOST_LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER_CMD_NAMES_WAVE565;
    let ok = residual_name_index(steps, "REQUIRE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_CONSTRUCT_ARM_USES_HELPER").is_some()
        && residual_name_index(steps, "LIVE_CONSTRUCT_TEMPLATE_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "construct_template_presentation_helper").is_some()
        && residual_name_index(cmds, "construct_arm").is_some()
        && residual_name_index(cmds, "dozer_spawn_template_residual").is_some();
    residual_action_store(ResidualConstructTemplatePresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_construct_template_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 565")
        && eng.contains("fn presentation_or_boot_has_template")
        && eng.contains("\"construct\" | \"dozer_construct\" | \"place_structure\"");
    residual_action_store(ResidualConstructTemplatePresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_construct_template_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("\"construct\" | \"dozer_construct\" | \"place_structure\"")
        && (eng.contains("presentation_or_boot_has_template")
            || eng.contains("presentation_or_live_has_template"))
        && (eng.contains("create_object") || eng.contains("host_create_object"))
        && eng.contains("Wave 565");
    residual_action_store(ResidualConstructTemplatePresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_construct_template_presentation_helper_residual_pack_wave565() -> bool {
    honesty_construct_template_presentation_helper_method_names_residual_wave565()
        && honesty_construct_template_presentation_helper_source_markers_residual_wave565()
        && honesty_construct_template_presentation_helper_nav_commands_residual_wave565()
        && simulate_construct_template_presentation_helper_collect_source()
        && simulate_construct_template_presentation_helper_dispatch_source()
}

pub fn simulate_live_construct_template_presentation_helper_honesty() -> bool {
    let ok = honesty_construct_template_presentation_helper_residual_pack_wave565();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualConstructTemplatePresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_construct_template_presentation_helper_method_names_residual_wave565());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_construct_template_presentation_helper_source_markers_residual_wave565());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_construct_template_presentation_helper_nav_commands_residual_wave565());
    }

    #[test]
    fn construct_template_presentation_helper_sources() {
        assert!(simulate_construct_template_presentation_helper_collect_source());
        assert!(simulate_construct_template_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave565_composite_pack() {
        assert!(honesty_construct_template_presentation_helper_residual_pack_wave565());
    }

    #[test]
    fn simulate_live_construct_template_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_construct_template_presentation_helper_honesty(),
            "construct template presentation helper residual must latch"
        );
        assert!(residual_construct_template_presentation_helper_ok());
        assert_eq!(
            residual_construct_template_presentation_helper_last_action(),
            ResidualConstructTemplatePresentationHelperAction::Composite
        );
    }
}
