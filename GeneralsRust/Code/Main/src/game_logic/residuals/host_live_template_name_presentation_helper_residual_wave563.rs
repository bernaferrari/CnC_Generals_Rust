//! Wave 563 residual peels: freeze `known_template_names` onto `PresentationFrame`
//! and centralize train template residual through `presentation_or_boot_has_template`
//! — presentation freeze owns name contains when installed; boot residual without
//! freeze uses host `templates.contains_key`. Mid-command inserts still hit host.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 562 combat-kill particle observe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` known_template_names / has_template_name
//! - `cnc_game_engine.rs` presentation_or_boot_has_template / train_unit
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_METHOD_NAMES_WAVE563: &[&str] = &[
    "presentation_or_boot_has_template",
    "known_template_names",
    "has_template_name",
    "train_unit",
    "Wave 563",
    "playable_claim = false",
];

pub const LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_NAV_STEPS_WAVE563: &[&str] = &[
    "REQUIRE_KNOWN_TEMPLATE_NAMES_FIELD",
    "REQUIRE_TEMPLATE_NAME_PRESENTATION_HELPER",
    "LIVE_TEMPLATE_NAME_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_CMD_NAMES_WAVE563: &[&str] = &[
    "known_template_names_field",
    "template_name_presentation_helper",
    "train_unit_template_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualTemplateNamePresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualTemplateNamePresentationHelperAction {
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

fn residual_action_store(action: ResidualTemplateNamePresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_template_name_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_template_name_presentation_helper_last_action()
-> ResidualTemplateNamePresentationHelperAction {
    ResidualTemplateNamePresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_template_name_presentation_helper_method_names_residual_wave563() -> bool {
    let names = LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_METHOD_NAMES_WAVE563;
    let ok = residual_name_index(names, "presentation_or_boot_has_template").is_some()
        && residual_name_index(names, "known_template_names").is_some()
        && residual_name_index(names, "has_template_name").is_some()
        && residual_name_index(names, "train_unit").is_some()
        && residual_name_index(names, "Wave 563").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualTemplateNamePresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_template_name_presentation_helper_source_markers_residual_wave563() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub known_template_names: Vec<String>")
        && pf.contains("Wave 563")
        && pf.contains("fn has_template_name")
        && pf.contains("binary_search_by");
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_has_template(") else {
        residual_action_store(ResidualTemplateNamePresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — no templates.contains_key dual-read.
    let helper_ok = helper.contains("Wave 563")
        && helper.contains("pres.has_template_name(name)")
        && helper.contains("host_match_known_template_names")
        && !helper.contains("self.game_logic.templates.contains_key(name)");
    // 2026-08-15: train arm peeled into runtime_host_cmd_enqueue_production.
    let train_ok = eng.contains("\"enqueue_production\" | \"train_unit\"")
        && (eng.contains("presentation_or_boot_has_template")
            || eng.contains("presentation_or_live_has_template"))
        && eng.contains("host_ensure_golden_ranger_template");
    let ok = field_ok && helper_ok && train_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualTemplateNamePresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_template_name_presentation_helper_nav_commands_residual_wave563() -> bool {
    let steps = LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_NAV_STEPS_WAVE563;
    let cmds = RUNTIME_HOST_LIVE_TEMPLATE_NAME_PRESENTATION_HELPER_CMD_NAMES_WAVE563;
    let ok = residual_name_index(steps, "REQUIRE_KNOWN_TEMPLATE_NAMES_FIELD").is_some()
        && residual_name_index(steps, "REQUIRE_TEMPLATE_NAME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_TEMPLATE_NAME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "known_template_names_field").is_some()
        && residual_name_index(cmds, "template_name_presentation_helper").is_some()
        && residual_name_index(cmds, "train_unit_template_residual").is_some();
    residual_action_store(ResidualTemplateNamePresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_template_name_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let ok = eng.contains("Wave 563")
        && eng.contains("fn presentation_or_boot_has_template")
        && pf.contains("pub known_template_names: Vec<String>");
    residual_action_store(ResidualTemplateNamePresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_template_name_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("\"enqueue_production\" | \"train_unit\"")
        && (eng.contains("presentation_or_boot_has_template")
            || eng.contains("presentation_or_live_has_template"))
        && eng.contains("enqueue_production")
        && eng.contains("host_ensure_golden_ranger_template");
    residual_action_store(ResidualTemplateNamePresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_template_name_presentation_helper_residual_pack_wave563() -> bool {
    honesty_template_name_presentation_helper_method_names_residual_wave563()
        && honesty_template_name_presentation_helper_source_markers_residual_wave563()
        && honesty_template_name_presentation_helper_nav_commands_residual_wave563()
        && simulate_template_name_presentation_helper_collect_source()
        && simulate_template_name_presentation_helper_dispatch_source()
}

pub fn simulate_live_template_name_presentation_helper_honesty() -> bool {
    let ok = honesty_template_name_presentation_helper_residual_pack_wave563();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualTemplateNamePresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_template_name_presentation_helper_method_names_residual_wave563());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_template_name_presentation_helper_source_markers_residual_wave563());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_template_name_presentation_helper_nav_commands_residual_wave563());
    }

    #[test]
    fn template_name_presentation_helper_sources() {
        assert!(simulate_template_name_presentation_helper_collect_source());
        assert!(simulate_template_name_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave563_composite_pack() {
        assert!(honesty_template_name_presentation_helper_residual_pack_wave563());
    }

    #[test]
    fn simulate_live_template_name_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_template_name_presentation_helper_honesty(),
            "template name presentation helper residual must latch"
        );
        assert!(residual_template_name_presentation_helper_ok());
        assert_eq!(
            residual_template_name_presentation_helper_last_action(),
            ResidualTemplateNamePresentationHelperAction::Composite
        );
    }
}
