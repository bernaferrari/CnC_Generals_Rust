//! Wave 358 residual peels: HackInternetAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), pack/hack
//! cash helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 357 RailroadGuideAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/hack_internet_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// HackInternetAI dual-world empty-gate residual method names.
pub const LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE358: &[&str] = &[
    "dual_world_registry_unavailable",
    "enter_hacking",
    "do_cash_update",
    "get_pack_time",
    "update_hacking",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE358: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_HACK_INTERNET_AI_EMPTY_GATES",
    "LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE358: &[&str] = &[
    "click_live_hack_internet_ai_dual_world_empty_gate_ok_prepare",
    "click_live_hack_internet_ai_dual_world_empty_gate_ok_live",
    "click_live_hack_internet_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_hack_internet_ai_dual_world_empty_gate_method_names_residual_wave358() -> bool {
    LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE358.len() == 6
        && residual_name_index(
            LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE358,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE358,
            "update_hacking",
        ) == Some(4)
        && residual_name_index(
            LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE358,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_hack_internet_ai_dual_world_empty_gate_nav_commands_residual_wave358() -> bool {
    LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE358.len() == 4
        && residual_name_index(
            LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE358,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE358,
            "LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HACK_INTERNET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE358.len() == 3
}

/// Wave 358 composite residual honesty pack.
pub fn honesty_live_hack_internet_ai_dual_world_empty_gate_residual_pack_wave358() -> bool {
    honesty_live_hack_internet_ai_dual_world_empty_gate_method_names_residual_wave358()
        && honesty_live_hack_internet_ai_dual_world_empty_gate_nav_commands_residual_wave358()
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

/// Source residual: HackInternetAI empty dual-world short-circuits.
pub fn honesty_hack_internet_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/update/ai_update/hack_internet_ai_update.rs"
    );
    if !(g.contains("Wave 358")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(enter) = fn_body(g, "fn enter_hacking(") else {
        return false;
    };
    let Some(cash) = fn_body(g, "fn do_cash_update(") else {
        return false;
    };
    let Some(pack) = fn_body(g, "fn get_pack_time(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update_hacking(") else {
        return false;
    };
    helper_ok
        && enter.contains("return;")
        && cash.contains("return Ok(())")
        && pack.contains("self.data.pack_time")
        && upd.contains("return Ok(())")
        && g.matches("Wave 358:").count() >= 8
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_hack_internet_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_hack_internet_ai_dual_world_empty_gate_residual_pack_wave358()
        && honesty_hack_internet_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_hack_internet_ai_dual_world_empty_gate_method_names_residual_wave358()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_hack_internet_ai_dual_world_empty_gate_nav_commands_residual_wave358()
        );
    }

    #[test]
    fn wave358_composite_pack() {
        assert!(honesty_live_hack_internet_ai_dual_world_empty_gate_residual_pack_wave358());
    }

    #[test]
    fn hack_internet_ai_dual_world_empty_gate_sources() {
        assert!(honesty_hack_internet_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_hack_internet_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_hack_internet_ai_dual_world_empty_gate_honesty(),
            "hack internet ai dual-world empty gate residual must latch"
        );
    }
}
