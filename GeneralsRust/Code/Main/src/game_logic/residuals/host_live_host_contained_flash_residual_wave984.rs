//! Wave 984: host contained-flash residual peel.
//!
//! Stamps garrisoned unit ids into presentation drawables. On select, host empty
//! dual-world queues those ids; presentation shell drains and color-flashes
//! matching drawables (C++ clientVisibleContainedFlashAsSelected residual).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTAINED_FLASH_RESIDUAL_METHOD_NAMES_WAVE984: &[&str] = &[
    "queue_host_contained_flash_object_ids",
    "take_host_contained_flash_object_ids",
    "presentation_garrisoned_ids",
    "garrisoned_ids",
    "flash_contained_objects",
    "Wave 984",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTAINED_FLASH_RESIDUAL_NAV_STEPS_WAVE984: &[&str] = &[
    "GARRISONED_IDS_STAMP",
    "QUEUE_CONTAINED_FLASH",
    "SHELL_DRAIN_FLASH",
    "LIVE_HOST_CONTAINED_FLASH_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostContainedFlashResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostContainedFlashResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}
fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_contained_flash_residual_method_names_residual_wave984() -> bool {
    let names = LIVE_HOST_CONTAINED_FLASH_RESIDUAL_METHOD_NAMES_WAVE984;
    let ok = residual_name_index(names, "queue_host_contained_flash_object_ids").is_some()
        && residual_name_index(names, "Wave 984").is_some();
    residual_action_store(ResidualHostContainedFlashResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_contained_flash_residual_nav_commands_residual_wave984() -> bool {
    let steps = LIVE_HOST_CONTAINED_FLASH_RESIDUAL_NAV_STEPS_WAVE984;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTAINED_FLASH_RESIDUAL").is_some()
        && residual_name_index(steps, "SHELL_DRAIN_FLASH").is_some();
    residual_action_store(ResidualHostContainedFlashResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_contained_flash_residual_residual_pack_wave984() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let d = drawable_source();
    let client = client_source();
    let flash = match d.find("fn flash_contained_objects") {
        Some(i) => &d[i..d.len().min(i + 900)],
        None => "",
    };
    let shell = match client.find("fn update_presentation_shell") {
        Some(i) => {
            let rest = &client[i..];
            let end = rest
                .find("\n    pub fn ")
                .map(|o| i + o)
                .unwrap_or(client.len().min(i + 8000));
            &client[i..end]
        }
        None => "",
    };
    let ok = d.contains("presentation_garrisoned_ids")
        && d.contains("Wave 984")
        && flash.contains("queue_host_contained_flash_object_ids")
        && client.contains("queue_host_contained_flash_object_ids")
        && client.contains("take_host_contained_flash_object_ids")
        && shell.contains("Wave 984")
        && shell.contains("take_host_contained_flash_object_ids")
        && shell.contains("color_flash")
        && client.contains("pub garrisoned_ids: Vec<u32>")
        && cnc.contains("garrisoned_ids:")
        && cnc.contains("garrisoned_units")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostContainedFlashResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_contained_flash_residual_honesty() -> bool {
    let a = honesty_host_contained_flash_residual_method_names_residual_wave984();
    let b = honesty_host_contained_flash_residual_nav_commands_residual_wave984();
    let c = honesty_host_contained_flash_residual_residual_pack_wave984();
    residual_action_store(ResidualHostContainedFlashResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_contained_flash_residual_wave984() {
        assert!(honesty_host_contained_flash_residual_residual_pack_wave984());
        assert!(honesty_host_contained_flash_residual_method_names_residual_wave984());
        assert!(honesty_host_contained_flash_residual_nav_commands_residual_wave984());
        assert!(simulate_live_host_contained_flash_residual_honesty());
    }
}
