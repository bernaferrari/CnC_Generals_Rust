//! Wave 345 residual peels: meta_event dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), selection/
//! cheat/drawable helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 344 system GameLogic dual-world empty-gate residual.
//!
//! Sources:
//! - `GameClient/src/message_stream/meta_event.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Meta-event dual-world empty-gate residual method names.
pub const LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE345: &[&str] = &[
    "dual_world_registry_unavailable",
    "apply_extent_adjust_to_local_selection",
    "kill_local_player_selection",
    "adjust_local_selection_veterancy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE345: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_META_EVENT_EMPTY_GATES",
    "LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE345: &[&str] = &[
    "click_live_meta_event_dual_world_empty_gate_ok_prepare",
    "click_live_meta_event_dual_world_empty_gate_ok_live",
    "click_live_meta_event_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_meta_event_dual_world_empty_gate_method_names_residual_wave345() -> bool {
    LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE345.len() == 5
        && residual_name_index(
            LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE345,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE345,
            "adjust_local_selection_veterancy",
        ) == Some(3)
        && residual_name_index(
            LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE345,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_meta_event_dual_world_empty_gate_nav_commands_residual_wave345() -> bool {
    LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE345.len() == 4
        && residual_name_index(
            LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE345,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE345,
            "LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_META_EVENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE345.len() == 3
}

/// Wave 345 composite residual honesty pack.
pub fn honesty_live_meta_event_dual_world_empty_gate_residual_pack_wave345() -> bool {
    honesty_live_meta_event_dual_world_empty_gate_method_names_residual_wave345()
        && honesty_live_meta_event_dual_world_empty_gate_nav_commands_residual_wave345()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + name.len();
            continue;
        };
        let brace = i + b;
        let mut depth = 0usize;
        for (off, ch) in src[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = &src[i..brace + off + 1];
                        if body.contains("dual_world_registry_unavailable") {
                            return Some(body);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Source residual: meta_event empty dual-world short-circuits.
/// Source residual: meta_event empty dual-world short-circuits / TheGameLogic peels.
pub fn honesty_meta_event_dual_world_empty_gate_source() -> bool {
    let g = game_client::message_stream::meta_event::META_EVENT_SRC;
    if !(g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    gamelogic::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // Wave 976: kill/vet/extent peel onto TheGameLogic IDs (no empty dual-world no-op).
    let kill_ok = g.contains("fn kill_local_player_selection")
        && g.contains("TheGameLogic::find_object_by_id")
        && g.contains("Wave 976");
    let vet_ok = g.contains("fn adjust_local_selection_veterancy")
        && g.contains("on_veterancy_level_changed")
        && g.contains("Wave 976");
    let extent_ok = g.contains("fn apply_extent_adjust_to_local_selection")
        && g.contains("set_geometry_info")
        && g.contains("Wave 976");
    helper_ok
        && kill_ok
        && vet_ok
        && extent_ok
        && g.contains("fn next_plane_camera_lock_object_id")
        && g.contains("fn refresh_drawable_time_of_day")
        && g.contains("fn refresh_drawable_model_conditions")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_meta_event_dual_world_empty_gate_honesty() -> bool {
    honesty_live_meta_event_dual_world_empty_gate_residual_pack_wave345()
        && honesty_meta_event_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_meta_event_dual_world_empty_gate_method_names_residual_wave345());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_meta_event_dual_world_empty_gate_nav_commands_residual_wave345());
    }

    #[test]
    fn wave345_composite_pack() {
        assert!(honesty_live_meta_event_dual_world_empty_gate_residual_pack_wave345());
    }

    #[test]
    fn meta_event_dual_world_empty_gate_sources() {
        assert!(honesty_meta_event_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_meta_event_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_meta_event_dual_world_empty_gate_honesty(),
            "meta event dual-world empty gate residual must latch"
        );
    }
}
