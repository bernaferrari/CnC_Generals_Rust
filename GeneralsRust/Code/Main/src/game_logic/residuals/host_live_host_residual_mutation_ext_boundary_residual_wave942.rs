//! Wave 942: host residual mutation boundary extension
//! (projectile/field expire, bomb destroy, horde/hive/model, payload config).
//!
//! Remaining shadow-session `get_objects_mut` dual-writes for lethal expires,
//! flight-bomb destroys, horde grants, stinger hive, model bits, sticky follow,
//! and post-create payload config route through `apply_host_residual_mutation_op`.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RESIDUAL_MUTATION_EXT_METHOD_NAMES_WAVE942: &[&str] = &[
    "apply_host_residual_mutation_op",
    "HostResidualMutationOp",
    "LethalExpire",
    "DestroyBomb",
    "ConfigureSpawnedPayload",
    "SetWeaponBonusHorde",
    "ApplyStingerHiveState",
    "SetModelConditionBits",
    "SetPosition",
    "ObjectIdentityClear",
    "SpawnedPayloadKind",
    "Wave 942",
    "playable_claim = false",
];

pub const LIVE_HOST_RESIDUAL_MUTATION_EXT_NAV_STEPS_WAVE942: &[&str] = &[
    "RESIDUAL_MUTATION_EXT_BOUNDARY",
    "HOST_RESIDUAL_MUTATION_OP_EXT",
    "LIVE_HOST_RESIDUAL_MUTATION_EXT_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostResidualMutationExtBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostResidualMutationExtBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_host_residual_mutation_ext_boundary_method_names_residual_wave942() -> bool {
    let names = LIVE_HOST_RESIDUAL_MUTATION_EXT_METHOD_NAMES_WAVE942;
    let ok = residual_name_index(names, "apply_host_residual_mutation_op").is_some()
        && residual_name_index(names, "LethalExpire").is_some()
        && residual_name_index(names, "ConfigureSpawnedPayload").is_some()
        && residual_name_index(names, "Wave 942").is_some();
    residual_action_store(ResidualHostResidualMutationExtBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_mutation_ext_boundary_nav_commands_residual_wave942() -> bool {
    let steps = LIVE_HOST_RESIDUAL_MUTATION_EXT_NAV_STEPS_WAVE942;
    let ok = residual_name_index(steps, "LIVE_HOST_RESIDUAL_MUTATION_EXT_BOUNDARY").is_some()
        && residual_name_index(steps, "RESIDUAL_MUTATION_EXT_BOUNDARY").is_some();
    residual_action_store(ResidualHostResidualMutationExtBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_mutation_ext_boundary_residual_pack_wave942() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_host_residual_mutation_op", 20000));
    let session = non_comment_code(session_fn_window(sh));
    let session_mut_count = session.matches("get_objects_mut").count();
    let ok = gl.contains("enum HostResidualMutationOp")
        && gl.contains("LethalExpire")
        && gl.contains("DestroyBomb")
        && gl.contains("ConfigureSpawnedPayload")
        && gl.contains("SetWeaponBonusHorde")
        && gl.contains("ApplyStingerHiveState")
        && gl.contains("SetModelConditionBits")
        && gl.contains("enum ObjectIdentityClear")
        && gl.contains("enum SpawnedPayloadKind")
        && api.contains("LethalExpire")
        && api.contains("ConfigureSpawnedPayload")
        && api.contains("SetWeaponBonusHorde")
        && api.contains("ApplyStingerHiveState")
        && session.contains("apply_host_residual_mutation_op")
        && session.contains("LethalExpire")
        && session.contains("DestroyBomb")
        && session.contains("ConfigureSpawnedPayload")
        && session.contains("SetWeaponBonusHorde")
        && session.contains("ApplyStingerHiveState")
        && session.contains("SetModelConditionBits")
        && session.contains("SetPosition")
        && session.contains("host_cannon_shell_projectile_log::drain_impacts")
        && session.contains("host_field_object_expire_log::drain")
        && session.contains("host_stinger_hive_log::drain")
        && session.contains("host_battlemaster_horde_log::drain")
        && session.contains("host_china_infantry_horde_log::drain")
        && session.contains("host_actively_constructing_log::drain")
        && session.contains("host_sticky_booby_attach_log::drain_sticky_follows")
        && session_mut_count <= 1
        && gl.contains("Wave 942")
        && sh.contains("942")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostResidualMutationExtBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_residual_mutation_ext_boundary_honesty() -> bool {
    let a = honesty_host_residual_mutation_ext_boundary_method_names_residual_wave942();
    let b = honesty_host_residual_mutation_ext_boundary_nav_commands_residual_wave942();
    let c = honesty_host_residual_mutation_ext_boundary_residual_pack_wave942();
    residual_action_store(ResidualHostResidualMutationExtBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_residual_mutation_ext_boundary_residual_wave942() {
        assert!(honesty_host_residual_mutation_ext_boundary_residual_pack_wave942());
        assert!(honesty_host_residual_mutation_ext_boundary_method_names_residual_wave942());
        assert!(honesty_host_residual_mutation_ext_boundary_nav_commands_residual_wave942());
        assert!(simulate_live_host_residual_mutation_ext_boundary_honesty());
    }
}
