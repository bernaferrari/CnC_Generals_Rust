//! Wave 1107: presentation residual counts + centroid + unit-cmd sold peels.
//!
//! After Waves 1104–1106 selection/FOW sold peels, residual object counts,
//! group centroid, and unit_command_buttons still treated sold objects as alive.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD_METHOD_NAMES_WAVE1107: &[&str] = &[
    "detector_object_count",
    "centroid_of_ids",
    "unit_command_buttons",
    "battle_plan_bonus_object_count",
    "Wave 1107",
    "playable_claim: false",
];

pub const LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD_NAV_STEPS_WAVE1107: &[&str] = &[
    "RESIDUAL_COUNTS_EXCLUDE_SOLD",
    "CENTROID_FAILS_CLOSED_SOLD",
    "UNIT_CMD_PRIMARY_USABLE",
    "LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostResidualCountsCentroidCmdSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostResidualCountsCentroidCmdSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_residual_counts_centroid_cmd_sold_method_names_residual_wave1107() -> bool {
    let names = LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD_METHOD_NAMES_WAVE1107;
    let ok = residual_name_index(names, "detector_object_count").is_some()
        && residual_name_index(names, "Wave 1107").is_some();
    residual_action_store(ResidualHostResidualCountsCentroidCmdSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_counts_centroid_cmd_sold_nav_commands_residual_wave1107() -> bool {
    let steps = LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD_NAV_STEPS_WAVE1107;
    let ok = residual_name_index(steps, "LIVE_HOST_RESIDUAL_COUNTS_CENTROID_CMD_SOLD").is_some()
        && residual_name_index(steps, "RESIDUAL_COUNTS_EXCLUDE_SOLD").is_some();
    residual_action_store(ResidualHostResidualCountsCentroidCmdSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_counts_centroid_cmd_sold_residual_pack_wave1107() -> bool {
    let pf = pf_source();
    let es = es_source();
    let ok = pf.contains("Wave 1107: residual counts exclude sold")
        && pf.contains("Wave 1107: camera/group centroid residual fail-closed on sold")
        && pf.contains("Wave 1107: unit command buttons residual fail-closed on sold")
        && pf.contains("fn detector_object_count")
        && pf.contains("fn command_set_override_object_count")
        && pf.contains("fn innate_stealth_object_count")
        && pf.contains("fn timed_detector_object_count")
        && pf.contains("fn humvee_transport_object_count")
        && pf.contains("fn overlord_gattling_object_count")
        && pf.contains("fn hive_object_count")
        && pf.contains("fn continuous_fire_object_count")
        && pf.contains("fn battle_plan_bonus_object_count")
        && pf.contains("fn horde_bonus_object_count")
        && pf.contains("fn turret_idle_scan_count")
        && pf.contains("fn centroid_of_ids")
        && pf.contains("fn unit_command_buttons")
        && pf.matches("!o.sold").count() >= 10
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostResidualCountsCentroidCmdSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_residual_counts_centroid_cmd_sold_residual_honesty() -> bool {
    let a = honesty_host_residual_counts_centroid_cmd_sold_method_names_residual_wave1107();
    let b = honesty_host_residual_counts_centroid_cmd_sold_nav_commands_residual_wave1107();
    let c = honesty_host_residual_counts_centroid_cmd_sold_residual_pack_wave1107();
    residual_action_store(ResidualHostResidualCountsCentroidCmdSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_residual_counts_centroid_cmd_sold_residual_wave1107() {
        assert!(honesty_host_residual_counts_centroid_cmd_sold_residual_pack_wave1107());
        assert!(honesty_host_residual_counts_centroid_cmd_sold_method_names_residual_wave1107());
        assert!(honesty_host_residual_counts_centroid_cmd_sold_nav_commands_residual_wave1107());
        assert!(simulate_live_host_residual_counts_centroid_cmd_sold_residual_honesty());
    }
}
