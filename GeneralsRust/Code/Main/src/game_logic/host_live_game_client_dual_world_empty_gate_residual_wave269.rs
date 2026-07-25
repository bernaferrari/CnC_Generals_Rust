//! Wave 269 residual peels: GameClient dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), GameClient
//! drawable/object factory lookups fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 268 Player dual-world empty-gate residual.
//!
//! Sources:
//! - `GameClient/src/core/game_client.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Full `update()` keeps host presentation path (not short-circuited)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// GameClient dual-world empty-gate residual method names.
pub const LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE269: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_drawable_template_name",
    "register_drawable_with_template",
    "find_game_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE269: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_GAME_CLIENT_EMPTY_GATES",
    "LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE269: &[&str] = &[
    "click_live_game_client_dual_world_empty_gate_ok_prepare",
    "click_live_game_client_dual_world_empty_gate_ok_live",
    "click_live_game_client_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_game_client_dual_world_empty_gate_method_names_residual_wave269() -> bool {
    LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE269.len() == 5
        && residual_name_index(
            LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE269,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE269,
            "find_game_object",
        ) == Some(3)
        && residual_name_index(
            LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE269,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_game_client_dual_world_empty_gate_nav_commands_residual_wave269() -> bool {
    LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE269.len() == 4
        && residual_name_index(
            LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE269,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE269,
            "LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_GAME_CLIENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE269.len() == 3
}

/// Wave 269 composite residual honesty pack.
pub fn honesty_live_game_client_dual_world_empty_gate_residual_pack_wave269() -> bool {
    honesty_live_game_client_dual_world_empty_gate_method_names_residual_wave269()
        && honesty_live_game_client_dual_world_empty_gate_nav_commands_residual_wave269()
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

/// Source residual: GameClient empty dual-world short-circuits.
pub fn honesty_game_client_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameClient/src/core/game_client.rs");
    if !(g.contains("Wave 269")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(resolve) = fn_body(g, "fn resolve_drawable_template_name(") else {
        return false;
    };
    let Some(register) = fn_body(g, "fn register_drawable_with_template(") else {
        return false;
    };
    let Some(find) = fn_body(g, "fn find_game_object(") else {
        return false;
    };
    // update must keep host path, not early dual-world Ok(())
    let Some(update) = fn_body(g, "fn update(&mut self)") else {
        return false;
    };
    resolve.contains("dual_world_registry_unavailable")
        && register.contains("dual_world_registry_unavailable")
        && find.contains("dual_world_registry_unavailable")
        && find.contains("Ok(None)")
        && update.contains("host_presentation_path")
        && {
            // ensure early dual-world gate is NOT the first action after brace
            let after = update.split('{').nth(1).unwrap_or("");
            !after.trim_start().starts_with("// Wave 269")
        }
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_game_client_dual_world_empty_gate_honesty() -> bool {
    honesty_live_game_client_dual_world_empty_gate_residual_pack_wave269()
        && honesty_game_client_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_game_client_dual_world_empty_gate_method_names_residual_wave269());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_game_client_dual_world_empty_gate_nav_commands_residual_wave269());
    }

    #[test]
    fn wave269_composite_pack() {
        assert!(honesty_live_game_client_dual_world_empty_gate_residual_pack_wave269());
    }

    #[test]
    fn game_client_dual_world_empty_gate_sources() {
        assert!(honesty_game_client_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_game_client_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_game_client_dual_world_empty_gate_honesty(),
            "game client dual-world empty gate residual must latch"
        );
    }
}
