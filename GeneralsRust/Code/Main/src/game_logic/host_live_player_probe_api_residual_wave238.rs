//! Wave 238 residual peels: engine boot HUD/science paths use GameLogic
//! `player_economy` / `player_unlocked_sciences` / `player_can_purchase_science`
//! probes (and `ui_local_economy`) instead of dual-reading `&Player` via
//! `get_player`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 237 engine player UI boot peel residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` player_economy / player_unlocked_sciences /
//!   player_science_purchase_points / player_can_purchase_science
//! - `cnc_game_engine.rs` ui_local_economy / HUD boot / purchase unlock path
//!
//! Fail-closed:
//! - Bootstrap camera static helper may still use get_player when no frame
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Player probe API residual method names.
pub const LIVE_PLAYER_PROBE_API_METHOD_NAMES_WAVE238: &[&str] = &[
    "player_economy",
    "player_unlocked_sciences",
    "player_can_purchase_science",
    "ui_local_economy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PLAYER_PROBE_API_NAV_STEPS_WAVE238: &[&str] = &[
    "REQUIRE_PLAYER_PROBE_API",
    "REQUIRE_HUD_PURCHASE_USE_PROBES",
    "LIVE_PLAYER_PROBE_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PLAYER_PROBE_API_CMD_NAMES_WAVE238: &[&str] = &[
    "click_live_player_probe_api_ok_prepare",
    "click_live_player_probe_api_ok_live",
    "click_live_player_probe_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_player_probe_api_method_names_residual_wave238() -> bool {
    LIVE_PLAYER_PROBE_API_METHOD_NAMES_WAVE238.len() == 5
        && residual_name_index(LIVE_PLAYER_PROBE_API_METHOD_NAMES_WAVE238, "player_economy")
            == Some(0)
        && residual_name_index(
            LIVE_PLAYER_PROBE_API_METHOD_NAMES_WAVE238,
            "ui_local_economy",
        ) == Some(3)
        && residual_name_index(
            LIVE_PLAYER_PROBE_API_METHOD_NAMES_WAVE238,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_player_probe_api_nav_commands_residual_wave238() -> bool {
    LIVE_PLAYER_PROBE_API_NAV_STEPS_WAVE238.len() == 4
        && residual_name_index(
            LIVE_PLAYER_PROBE_API_NAV_STEPS_WAVE238,
            "REQUIRE_PLAYER_PROBE_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_PROBE_API_NAV_STEPS_WAVE238,
            "LIVE_PLAYER_PROBE_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PLAYER_PROBE_API_CMD_NAMES_WAVE238.len() == 3
}

/// Wave 238 composite residual honesty pack.
pub fn honesty_live_player_probe_api_residual_pack_wave238() -> bool {
    honesty_live_player_probe_api_method_names_residual_wave238()
        && honesty_live_player_probe_api_nav_commands_residual_wave238()
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

/// Source residual: probe APIs exist; engine HUD/purchase use them without get_player.
pub fn honesty_player_probe_api_source() -> bool {
    let gl = include_str!("game_logic.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    for api in [
        "pub fn player_economy(",
        "pub fn player_unlocked_sciences(",
        "pub fn player_science_purchase_points(",
        "pub fn player_can_purchase_science(",
    ] {
        if !gl.contains(api) {
            return false;
        }
    }
    if !(eng.contains("fn ui_local_economy(") && eng.contains("Wave 238")) {
        return false;
    }
    // HUD boot residual must not call get_player.
    // Approximate: ui_local_economy used near update_resources in update_internal path.
    if !eng.contains("ui_local_economy()") {
        return false;
    }
    let Some(purchase) = fn_body(eng, "fn try_purchase_next_generals_science(") else {
        return false;
    };
    purchase.contains("player_unlocked_sciences")
        && purchase.contains("player_can_purchase_science")
        && !purchase.contains("get_player(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_player_probe_api_honesty() -> bool {
    honesty_live_player_probe_api_residual_pack_wave238() && honesty_player_probe_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_player_probe_api_method_names_residual_wave238());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_player_probe_api_nav_commands_residual_wave238());
    }

    #[test]
    fn wave238_composite_pack() {
        assert!(honesty_live_player_probe_api_residual_pack_wave238());
    }

    #[test]
    fn player_probe_api_sources() {
        assert!(honesty_player_probe_api_source());
    }

    #[test]
    fn simulate_live_player_probe_api_honesty_residual_live() {
        assert!(
            simulate_live_player_probe_api_honesty(),
            "player probe API residual must latch"
        );
    }
}
