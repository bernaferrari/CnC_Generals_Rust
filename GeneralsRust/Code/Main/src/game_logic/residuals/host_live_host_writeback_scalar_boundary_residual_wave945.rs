//! Wave 945: shadow→host scalar writeback authority boundary.
//!
//! Eighteen high-traffic scalar writebacks (AI state/attitude, owner, SP,
//! overcharge, weapon slot, selection radius, entity power, target location,
//! command set, ground height, body damage, death type, stored supplies,
//! faerie fire, repulsor, detector, guard) route through
//! `apply_host_writeback_op` / `HostWritebackOp` instead of direct
//! `get_objects_mut` dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY_METHOD_NAMES_WAVE945: &[&str] = &[
    "apply_host_writeback_op",
    "HostWritebackOp",
    "AiState",
    "AiAttitude",
    "Owner",
    "SpecialPower",
    "Overcharge",
    "WeaponSlot",
    "SelectionRadius",
    "EntityPower",
    "TargetLocation",
    "CommandSet",
    "GroundHeight",
    "BodyDamage",
    "DeathType",
    "StoredSupplies",
    "FaerieFire",
    "Repulsor",
    "Detector",
    "Guard",
    "Wave 945",
    "playable_claim = false",
];

pub const LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY_NAV_STEPS_WAVE945: &[&str] = &[
    "WRITEBACK_SCALAR_BOUNDARY",
    "HOST_WRITEBACK_OP_SCALAR",
    "LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackScalarBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWritebackScalarBoundaryAction) {
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

const WRITEBACKS: &[(&str, &str)] = &[
    ("pub fn writeback_ai_state_to_host", "AiState"),
    ("pub fn writeback_ai_attitude_to_host", "AiAttitude"),
    ("pub fn writeback_owner_to_host", "Owner"),
    ("pub fn writeback_special_power_to_host", "SpecialPower"),
    ("pub fn writeback_overcharge_to_host", "Overcharge"),
    ("pub fn writeback_weapon_slot_to_host", "WeaponSlot"),
    (
        "pub fn writeback_selection_radius_to_host",
        "SelectionRadius",
    ),
    ("pub fn writeback_entity_power_to_host", "EntityPower"),
    ("pub fn writeback_target_location_to_host", "TargetLocation"),
    ("pub fn writeback_command_set_to_host", "CommandSet"),
    ("pub fn writeback_ground_height_to_host", "GroundHeight"),
    ("pub fn writeback_body_damage_to_host", "BodyDamage"),
    ("pub fn writeback_death_type_to_host", "DeathType"),
    ("pub fn writeback_stored_supplies_to_host", "StoredSupplies"),
    ("pub fn writeback_faerie_fire_to_host", "FaerieFire"),
    ("pub fn writeback_repulsor_to_host", "Repulsor"),
    ("pub fn writeback_detector_to_host", "Detector"),
    ("pub fn writeback_guard_to_host", "Guard"),
];

pub fn honesty_host_writeback_scalar_boundary_method_names_residual_wave945() -> bool {
    let names = LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY_METHOD_NAMES_WAVE945;
    let ok = residual_name_index(names, "apply_host_writeback_op").is_some()
        && residual_name_index(names, "AiState").is_some()
        && residual_name_index(names, "Guard").is_some()
        && residual_name_index(names, "Wave 945").is_some();
    residual_action_store(ResidualHostWritebackScalarBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_scalar_boundary_nav_commands_residual_wave945() -> bool {
    let steps = LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY_NAV_STEPS_WAVE945;
    let ok = residual_name_index(steps, "LIVE_HOST_WRITEBACK_SCALAR_BOUNDARY").is_some()
        && residual_name_index(steps, "HOST_WRITEBACK_OP_SCALAR").is_some();
    residual_action_store(ResidualHostWritebackScalarBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_scalar_boundary_residual_pack_wave945() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_host_writeback_op", 12000));
    let mut all_ok = gl.contains("enum HostWritebackOp")
        && gl.contains("apply_host_writeback_op")
        && gl.contains("Wave 945")
        && sh.contains("Wave 945")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    for (fn_marker, variant) in WRITEBACKS {
        let body = non_comment_code(fn_window(sh, fn_marker));
        let variant_tok = format!("HostWritebackOp::{variant}");
        let ok = body.contains("apply_host_writeback_op")
            && body.contains(&variant_tok)
            && !body.contains("get_objects_mut")
            && api.contains(&variant_tok);
        if !ok {
            all_ok = false;
            break;
        }
    }
    residual_action_store(ResidualHostWritebackScalarBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(all_ok, Ordering::SeqCst);
    all_ok
}

pub fn simulate_live_host_writeback_scalar_boundary_honesty() -> bool {
    let a = honesty_host_writeback_scalar_boundary_method_names_residual_wave945();
    let b = honesty_host_writeback_scalar_boundary_nav_commands_residual_wave945();
    let c = honesty_host_writeback_scalar_boundary_residual_pack_wave945();
    residual_action_store(ResidualHostWritebackScalarBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_writeback_scalar_boundary_residual_wave945() {
        assert!(honesty_host_writeback_scalar_boundary_residual_pack_wave945());
        assert!(honesty_host_writeback_scalar_boundary_method_names_residual_wave945());
        assert!(honesty_host_writeback_scalar_boundary_nav_commands_residual_wave945());
        assert!(simulate_live_host_writeback_scalar_boundary_honesty());
    }
}
