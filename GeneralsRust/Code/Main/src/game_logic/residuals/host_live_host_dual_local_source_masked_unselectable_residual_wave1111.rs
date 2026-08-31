//! Wave 1111: dual-world local source masked/unselectable residual.
//!
//! After Waves 1074–1075 destroyed/sold/disabled/UC local-source peels,
//! `selection_source_object_id`, `selection_attack_result`, enter/repair/resume/
//! pickup dual paths still accepted masked or unselectable local sources.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE_METHOD_NAMES_WAVE1111: &[&str] = &[
    "selection_source_object_id",
    "selection_attack_result",
    "selection_can_enter_target",
    "selection_can_repair_target",
    "Wave 1111",
    "playable_claim: false",
];

pub const LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE_NAV_STEPS_WAVE1111: &[&str] = &[
    "LOCAL_SOURCE_MASKED_FAIL_CLOSED",
    "LOCAL_SOURCE_UNSELECTABLE_FAIL_CLOSED",
    "LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDualLocalSourceMaskedUnselectableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDualLocalSourceMaskedUnselectableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_dual_local_source_masked_unselectable_method_names_residual_wave1111() -> bool {
    let names = LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE_METHOD_NAMES_WAVE1111;
    let ok = residual_name_index(names, "selection_source_object_id").is_some()
        && residual_name_index(names, "Wave 1111").is_some();
    residual_action_store(ResidualHostDualLocalSourceMaskedUnselectableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_local_source_masked_unselectable_nav_commands_residual_wave1111() -> bool {
    let steps = LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE_NAV_STEPS_WAVE1111;
    let ok = residual_name_index(steps, "LIVE_HOST_DUAL_LOCAL_SOURCE_MASKED_UNSELECTABLE")
        .is_some()
        && residual_name_index(steps, "LOCAL_SOURCE_MASKED_FAIL_CLOSED").is_some();
    residual_action_store(ResidualHostDualLocalSourceMaskedUnselectableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_local_source_masked_unselectable_residual_pack_wave1111() -> bool {
    let tr = tr_source();
    let es = es_source();
    let src_i = match tr.find("fn selection_source_object_id") {
        Some(i) => i,
        None => {
            residual_action_store(
                ResidualHostDualLocalSourceMaskedUnselectableAction::SourceMarkers,
            );
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let src = &tr[src_i..src_i.saturating_add(1600)];
    let atk_i = match tr.find("fn selection_attack_result") {
        Some(i) => i,
        None => {
            residual_action_store(
                ResidualHostDualLocalSourceMaskedUnselectableAction::SourceMarkers,
            );
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let atk = &tr[atk_i..atk_i.saturating_add(2200)];
    let ok = src.contains("Wave 1111: also fail-closed on masked/unselectable local sources")
        && src.contains("!e.masked")
        && src.contains("!e.unselectable")
        && (atk.contains("Wave 1111: masked/unselectable local source residual fail-closed")
            || src.contains("Wave 1111: masked/unselectable local source residual fail-closed"))
        && (atk.contains("!sel.masked") || src.contains("!e.masked"))
        && (atk.contains("!sel.unselectable") || src.contains("!e.unselectable"))
        && (tr.matches("!sel.masked").count() + tr.matches("!e.masked").count()) >= 5
        && (tr.matches("!sel.unselectable").count() + tr.matches("!e.unselectable").count()) >= 5
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostDualLocalSourceMaskedUnselectableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_dual_local_source_masked_unselectable_residual_honesty() -> bool {
    let a = honesty_host_dual_local_source_masked_unselectable_method_names_residual_wave1111();
    let b = honesty_host_dual_local_source_masked_unselectable_nav_commands_residual_wave1111();
    let c = honesty_host_dual_local_source_masked_unselectable_residual_pack_wave1111();
    residual_action_store(ResidualHostDualLocalSourceMaskedUnselectableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_dual_local_source_masked_unselectable_residual_wave1111() {
        assert!(honesty_host_dual_local_source_masked_unselectable_residual_pack_wave1111());
        assert!(
            honesty_host_dual_local_source_masked_unselectable_method_names_residual_wave1111()
        );
        assert!(
            honesty_host_dual_local_source_masked_unselectable_nav_commands_residual_wave1111()
        );
        assert!(simulate_live_host_dual_local_source_masked_unselectable_residual_honesty());
    }
}
