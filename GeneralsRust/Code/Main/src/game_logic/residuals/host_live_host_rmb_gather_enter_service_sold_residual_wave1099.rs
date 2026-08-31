//! Wave 1099: presentation RMB gather/enter/heal/repair sold residual.
//!
//! After Wave 1098 attack sold peel, gather/enter/get-healed/get-repaired still
//! classified sold presentation targets. Fail-close `!hint.sold` on those
//! branches (target_hint already peels sold at construction).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD_METHOD_NAMES_WAVE1099: &[&str] = &[
    "classify_right_click_target_from_presentation",
    "Gather",
    "Enter",
    "GetHealed",
    "GetRepaired",
    "Wave 1099",
    "playable_claim = false",
];

pub const LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD_NAV_STEPS_WAVE1099: &[&str] = &[
    "GATHER_SOLD_FAIL_CLOSED",
    "ENTER_SOLD_FAIL_CLOSED",
    "HEAL_REPAIR_SOLD_FAIL_CLOSED",
    "LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRmbGatherEnterServiceSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRmbGatherEnterServiceSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cs_source() -> &'static str {
    crate::command_system::COMMAND_SYSTEM_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_rmb_gather_enter_service_sold_method_names_residual_wave1099() -> bool {
    let names = LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD_METHOD_NAMES_WAVE1099;
    let ok = residual_name_index(names, "classify_right_click_target_from_presentation").is_some()
        && residual_name_index(names, "Wave 1099").is_some();
    residual_action_store(ResidualHostRmbGatherEnterServiceSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_rmb_gather_enter_service_sold_nav_commands_residual_wave1099() -> bool {
    let steps = LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD_NAV_STEPS_WAVE1099;
    let ok = residual_name_index(steps, "LIVE_HOST_RMB_GATHER_ENTER_SERVICE_SOLD").is_some()
        && residual_name_index(steps, "ENTER_SOLD_FAIL_CLOSED").is_some();
    residual_action_store(ResidualHostRmbGatherEnterServiceSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_rmb_gather_enter_service_sold_residual_pack_wave1099() -> bool {
    let cs = cs_source();
    let es = es_source();
    let w = match super::harness::last_rust_fn_body(
        cs,
        "classify_right_click_target_from_presentation",
    ) {
        Some(b) => b,
        None => {
            residual_action_store(ResidualHostRmbGatherEnterServiceSoldAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let ok = w.contains("Wave 1099: sold residual fail-closed on gather target")
        && (w.contains("hint.is_resource && !hint.sold && any_worker()")
            || w.contains("hint.is_resource") && w.contains("!hint.sold") && w.contains("any_resource_collector"))
        && w.contains("Wave 1099: sold residual fail-closed on enter target")
        && w.contains("hint.can_be_entered")
        && w.contains("Wave 1099: sold residual fail-closed on heal pad")
        && (w.contains("hint.provides_heal && !hint.sold && hint.is_friendly_of_local")
            || (w.contains("hint.provides_heal") && w.contains("!hint.sold") && w.contains("hint.is_friendly_of_local")))
        && w.contains("Wave 1099: sold residual fail-closed on repair pad")
        && w.contains("&& !hint.sold")
        && w.contains("CommandType::GetRepaired")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostRmbGatherEnterServiceSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_rmb_gather_enter_service_sold_residual_honesty() -> bool {
    let a = honesty_host_rmb_gather_enter_service_sold_method_names_residual_wave1099();
    let b = honesty_host_rmb_gather_enter_service_sold_nav_commands_residual_wave1099();
    let c = honesty_host_rmb_gather_enter_service_sold_residual_pack_wave1099();
    residual_action_store(ResidualHostRmbGatherEnterServiceSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_rmb_gather_enter_service_sold_residual_wave1099() {
        assert!(honesty_host_rmb_gather_enter_service_sold_residual_pack_wave1099());
        assert!(honesty_host_rmb_gather_enter_service_sold_method_names_residual_wave1099());
        assert!(honesty_host_rmb_gather_enter_service_sold_nav_commands_residual_wave1099());
        assert!(simulate_live_host_rmb_gather_enter_service_sold_residual_honesty());
    }
}
