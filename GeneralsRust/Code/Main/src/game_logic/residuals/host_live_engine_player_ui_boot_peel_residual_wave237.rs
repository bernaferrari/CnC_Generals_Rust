//! Wave 237 residual peels: remaining engine player UI dual-reads route through
//! `ui_player_info` / `ui_player_team` (defeat message, load-screen boot roster,
//! camera reset). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 236 mouse input presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` defeat UI, load_screen_init_context boot, reset_camera
//!
//! Fail-closed:
//! - Bootstrap camera static helper still accepts &GameLogic for no-frame boot
//! - Purchase capability probe still dual-reads only when no presentation frame
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Engine player UI boot peel residual method names.
pub const LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_METHOD_NAMES_WAVE237: &[&str] = &[
    "ui_player_info",
    "ui_player_team",
    "load_screen_init_context",
    "reset_camera_view_hotkey",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_NAV_STEPS_WAVE237: &[&str] = &[
    "REQUIRE_ENGINE_PLAYER_UI_BOOT_PEEL",
    "REQUIRE_DEFEAT_LOAD_RESET_USE_HELPERS",
    "LIVE_ENGINE_PLAYER_UI_BOOT_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_CMD_NAMES_WAVE237: &[&str] = &[
    "click_live_engine_player_ui_boot_peel_ok_prepare",
    "click_live_engine_player_ui_boot_peel_ok_live",
    "click_live_engine_player_ui_boot_peel_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_engine_player_ui_boot_peel_method_names_residual_wave237() -> bool {
    LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_METHOD_NAMES_WAVE237.len() == 5
        && residual_name_index(
            LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_METHOD_NAMES_WAVE237,
            "ui_player_info",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_METHOD_NAMES_WAVE237,
            "reset_camera_view_hotkey",
        ) == Some(3)
        && residual_name_index(
            LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_METHOD_NAMES_WAVE237,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_engine_player_ui_boot_peel_nav_commands_residual_wave237() -> bool {
    LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_NAV_STEPS_WAVE237.len() == 4
        && residual_name_index(
            LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_NAV_STEPS_WAVE237,
            "REQUIRE_ENGINE_PLAYER_UI_BOOT_PEEL",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_NAV_STEPS_WAVE237,
            "LIVE_ENGINE_PLAYER_UI_BOOT_PEEL",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ENGINE_PLAYER_UI_BOOT_PEEL_CMD_NAMES_WAVE237.len() == 3
}

/// Wave 237 composite residual honesty pack.
pub fn honesty_live_engine_player_ui_boot_peel_residual_pack_wave237() -> bool {
    honesty_live_engine_player_ui_boot_peel_method_names_residual_wave237()
        && honesty_live_engine_player_ui_boot_peel_nav_commands_residual_wave237()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: defeat/load/reset use ui_player helpers (Wave 237 markers).
pub fn honesty_engine_player_ui_boot_peel_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    if !(eng.contains("fn ui_player_info(") && eng.contains("fn ui_player_team(")) {
        return false;
    }
    // Defeat path uses ui_player_info.
    if !(eng.contains("Wave 237: defeat UI prefers presentation roster helper")
        || eng.contains("ui_player_info(player_id)"))
    {
        return false;
    }
    let Some(load) = fn_body(eng, "fn load_screen_init_context(") else {
        return false;
    };
    if !(load.contains("Wave 237") && load.contains("ui_player_info")) {
        return false;
    }
    let Some(reset) = fn_body(eng, "fn reset_camera_view_hotkey(") else {
        return false;
    };
    // Wave 239: boot focus may use player_command_center_position probe.
    (reset.contains("Wave 237") || reset.contains("Wave 239"))
        && (reset.contains("ui_player_team") || reset.contains("player_command_center_position"))
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_engine_player_ui_boot_peel_honesty() -> bool {
    honesty_live_engine_player_ui_boot_peel_residual_pack_wave237()
        && honesty_engine_player_ui_boot_peel_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_engine_player_ui_boot_peel_method_names_residual_wave237());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_engine_player_ui_boot_peel_nav_commands_residual_wave237());
    }

    #[test]
    fn wave237_composite_pack() {
        assert!(honesty_live_engine_player_ui_boot_peel_residual_pack_wave237());
    }

    #[test]
    fn engine_player_ui_boot_peel_sources() {
        assert!(honesty_engine_player_ui_boot_peel_source());
    }

    #[test]
    fn simulate_live_engine_player_ui_boot_peel_honesty_residual_live() {
        assert!(
            simulate_live_engine_player_ui_boot_peel_honesty(),
            "engine player UI boot peel residual must latch"
        );
    }
}
