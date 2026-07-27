//! Wave 581 residual peels: template residual for mid-command host inserts is
//! centralized through `presentation_or_live_has_template`, spawn through
//! `host_create_object`, and GoldenRanger ensure through
//! `host_ensure_golden_ranger_template`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 580 host cancel/selection helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_live_has_template /
//!   host_create_object / host_ensure_golden_ranger_template
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TEMPLATE_SPAWN_HELPER_METHOD_NAMES_WAVE581: &[&str] = &[
    "presentation_or_live_has_template",
    "host_create_object",
    "host_ensure_golden_ranger_template",
    "presentation_or_boot_has_template",
    "Wave 581",
    "playable_claim = false",
];

pub const LIVE_HOST_TEMPLATE_SPAWN_HELPER_NAV_STEPS_WAVE581: &[&str] = &[
    "REQUIRE_PRESENTATION_OR_LIVE_TEMPLATE",
    "REQUIRE_HOST_CREATE_OBJECT",
    "REQUIRE_GOLDEN_RANGER_ENSURE",
    "LIVE_HOST_TEMPLATE_SPAWN_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_TEMPLATE_SPAWN_HELPER_CMD_NAMES_WAVE581: &[&str] = &[
    "presentation_or_live_template",
    "host_create_object_helper",
    "golden_ranger_ensure",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTemplateSpawnHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostTemplateSpawnHelperAction {
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

fn residual_action_store(action: ResidualHostTemplateSpawnHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_template_spawn_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_template_spawn_helper_last_action() -> ResidualHostTemplateSpawnHelperAction {
    ResidualHostTemplateSpawnHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
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

pub fn honesty_host_template_spawn_helper_method_names_residual_wave581() -> bool {
    let names = LIVE_HOST_TEMPLATE_SPAWN_HELPER_METHOD_NAMES_WAVE581;
    let ok = residual_name_index(names, "presentation_or_live_has_template").is_some()
        && residual_name_index(names, "host_create_object").is_some()
        && residual_name_index(names, "host_ensure_golden_ranger_template").is_some()
        && residual_name_index(names, "presentation_or_boot_has_template").is_some()
        && residual_name_index(names, "Wave 581").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostTemplateSpawnHelperAction::MethodNames);
    ok
}

pub fn honesty_host_template_spawn_helper_source_markers_residual_wave581() -> bool {
    let eng = eng_source();
    let Some(live) = fn_body(eng, "fn host_presentation_or_live_has_template(")
        .or_else(|| fn_body(eng, "fn presentation_or_live_has_template("))
    else {
        residual_action_store(ResidualHostTemplateSpawnHelperAction::SourceMarkers);
        return false;
    };
    let Some(create) = fn_body(eng, "fn host_create_object(") else {
        residual_action_store(ResidualHostTemplateSpawnHelperAction::SourceMarkers);
        return false;
    };
    let Some(golden) = fn_body(eng, "fn host_ensure_golden_ranger_template(") else {
        residual_action_store(ResidualHostTemplateSpawnHelperAction::SourceMarkers);
        return false;
    };
    let live_ok = live.contains("Wave 581")
        && live.contains("has_template_name(name)")
        && live.contains("templates.contains_key(name)");
    let create_ok = create.contains("Wave 581")
        && create.contains("self.game_logic.create_object(name, team, spawn_at)")
        && !create.contains("self.host_create_object(name");
    let golden_ok = golden.contains("Wave 581")
        && golden.contains("GoldenRanger")
        && golden.contains("templates.insert");
    let call_ok = eng.contains("self.presentation_or_live_has_template(")
        && eng.contains("self.host_create_object(")
        && eng.contains("self.host_ensure_golden_ranger_template()");
    let raw_create = eng.matches("self.game_logic.create_object").count();
    // no leftover double-check pattern
    let double = eng.contains("presentation_or_boot_has_template(name)\n                                && !self.game_logic.templates.contains_key(name)");
    let ok = live_ok
        && create_ok
        && golden_ok
        && call_ok
        && raw_create == 1
        && !double
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostTemplateSpawnHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_template_spawn_helper_nav_commands_residual_wave581() -> bool {
    let steps = LIVE_HOST_TEMPLATE_SPAWN_HELPER_NAV_STEPS_WAVE581;
    let cmds = RUNTIME_HOST_LIVE_HOST_TEMPLATE_SPAWN_HELPER_CMD_NAMES_WAVE581;
    let ok = residual_name_index(steps, "REQUIRE_PRESENTATION_OR_LIVE_TEMPLATE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_CREATE_OBJECT").is_some()
        && residual_name_index(steps, "REQUIRE_GOLDEN_RANGER_ENSURE").is_some()
        && residual_name_index(steps, "LIVE_HOST_TEMPLATE_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "presentation_or_live_template").is_some()
        && residual_name_index(cmds, "host_create_object_helper").is_some()
        && residual_name_index(cmds, "golden_ranger_ensure").is_some();
    residual_action_store(ResidualHostTemplateSpawnHelperAction::NavCommands);
    ok
}

pub fn simulate_host_template_spawn_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 581")
        && eng.contains("fn presentation_or_live_has_template")
        && eng.contains("fn host_create_object")
        && eng.contains("fn host_ensure_golden_ranger_template");
    residual_action_store(ResidualHostTemplateSpawnHelperAction::CollectSource);
    ok
}

pub fn simulate_host_template_spawn_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.presentation_or_live_has_template(name)")
        && eng.contains("self.host_create_object(")
        && eng.contains("self.host_ensure_golden_ranger_template()")
        && eng.contains("USA_Dozer");
    residual_action_store(ResidualHostTemplateSpawnHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_template_spawn_helper_residual_pack_wave581() -> bool {
    honesty_host_template_spawn_helper_method_names_residual_wave581()
        && honesty_host_template_spawn_helper_source_markers_residual_wave581()
        && honesty_host_template_spawn_helper_nav_commands_residual_wave581()
        && simulate_host_template_spawn_helper_collect_source()
        && simulate_host_template_spawn_helper_dispatch_source()
}

pub fn simulate_live_host_template_spawn_helper_honesty() -> bool {
    let ok = honesty_host_template_spawn_helper_residual_pack_wave581();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostTemplateSpawnHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_template_spawn_helper_method_names_residual_wave581());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_template_spawn_helper_source_markers_residual_wave581());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_template_spawn_helper_nav_commands_residual_wave581());
    }

    #[test]
    fn host_template_spawn_helper_sources() {
        assert!(simulate_host_template_spawn_helper_collect_source());
        assert!(simulate_host_template_spawn_helper_dispatch_source());
    }

    #[test]
    fn wave581_composite_pack() {
        assert!(honesty_host_template_spawn_helper_residual_pack_wave581());
    }

    #[test]
    fn simulate_live_host_template_spawn_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_template_spawn_helper_honesty(),
            "host template/spawn helper residual must latch"
        );
        assert!(residual_host_template_spawn_helper_ok());
        assert_eq!(
            residual_host_template_spawn_helper_last_action(),
            ResidualHostTemplateSpawnHelperAction::Composite
        );
    }
}
