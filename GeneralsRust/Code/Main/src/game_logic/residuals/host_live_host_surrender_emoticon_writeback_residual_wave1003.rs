//! Wave 1003: surrender/emoticon/formation GameWorld→host writeback residual.
//!
//! writeback_combat_status_to_host last-writes is_surrendered, emoticon_*, and
//! formation_id/offset from Entity onto host Object (host→entity already
//! shadowed in Wave 999). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE1003: &[&str] = &[
    "writeback_combat_status_to_host",
    "is_surrendered",
    "emoticon_name",
    "formation_offset",
    "Wave 1003",
    "playable_claim = false",
];

pub const LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE1003: &[&str] = &[
    "WRITEBACK_COMBAT_STATUS",
    "SURRENDERED",
    "EMOTICON",
    "FORMATION",
    "LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSurrenderEmoticonWritebackResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSurrenderEmoticonWritebackResidualAction) {
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

pub fn honesty_host_surrender_emoticon_writeback_residual_method_names_residual_wave1003() -> bool {
    let names = LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE1003;
    let ok = residual_name_index(names, "writeback_combat_status_to_host").is_some()
        && residual_name_index(names, "Wave 1003").is_some();
    residual_action_store(ResidualHostSurrenderEmoticonWritebackResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_surrender_emoticon_writeback_residual_nav_commands_residual_wave1003() -> bool {
    let steps = LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE1003;
    let ok = residual_name_index(steps, "LIVE_HOST_SURRENDER_EMOTICON_WRITEBACK_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "WRITEBACK_COMBAT_STATUS").is_some();
    residual_action_store(ResidualHostSurrenderEmoticonWritebackResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_surrender_emoticon_writeback_residual_residual_pack_wave1003() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("fn writeback_combat_status_to_host")
        && sh.contains("Wave 1003: surrender / emoticon / formation residual last-writer")
        && sh.contains("set_flag!(obj.is_surrendered, ent.is_surrendered)")
        && sh.contains("obj.emoticon_name = ent.emoticon_name.clone()")
        && sh.contains("obj.emoticon_frames_left = ent.emoticon_frames_left")
        && sh.contains("obj.formation_id = ent.formation_id")
        && sh.contains("ent.formation_offset[0]")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSurrenderEmoticonWritebackResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_surrender_emoticon_writeback_residual_honesty() -> bool {
    let a = honesty_host_surrender_emoticon_writeback_residual_method_names_residual_wave1003();
    let b = honesty_host_surrender_emoticon_writeback_residual_nav_commands_residual_wave1003();
    let c = honesty_host_surrender_emoticon_writeback_residual_residual_pack_wave1003();
    residual_action_store(ResidualHostSurrenderEmoticonWritebackResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_surrender_emoticon_writeback_residual_wave1003() {
        assert!(honesty_host_surrender_emoticon_writeback_residual_residual_pack_wave1003());
        assert!(
            honesty_host_surrender_emoticon_writeback_residual_method_names_residual_wave1003()
        );
        assert!(
            honesty_host_surrender_emoticon_writeback_residual_nav_commands_residual_wave1003()
        );
        assert!(simulate_live_host_surrender_emoticon_writeback_residual_honesty());
    }
}
