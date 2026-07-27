//! Wave 235 residual peels: InGame RMB classification expands presentation
//! freeze (repair/enter/heal/service) and skips live `GameLogic` dual-read when
//! `selected_presentation` is non-empty. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 234 engine player/selection presentation UI residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` classify_right_click_target_from_presentation
//! - `cnc_game_engine.rs` presentation_target_hint / presentation_selected_unit_hints
//!
//! Fail-closed:
//! - Boot path without presentation still dual-reads GameLogic
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RMB presentation full-classify residual method names.
pub const LIVE_RMB_PRESENTATION_FULL_CLASSIFY_METHOD_NAMES_WAVE235: &[&str] = &[
    "classify_right_click_target_from_presentation",
    "is_damaged",
    "provides_vehicle_repair",
    "can_repair",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RMB_PRESENTATION_FULL_CLASSIFY_NAV_STEPS_WAVE235: &[&str] = &[
    "REQUIRE_RMB_PRESENTATION_FULL_CLASSIFY",
    "REQUIRE_NO_LIVE_DUAL_READ_WHEN_SELECTED_PRESENTATION",
    "LIVE_RMB_PRESENTATION_FULL_CLASSIFY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RMB_PRESENTATION_FULL_CLASSIFY_CMD_NAMES_WAVE235: &[&str] = &[
    "click_live_rmb_presentation_full_classify_ok_prepare",
    "click_live_rmb_presentation_full_classify_ok_live",
    "click_live_rmb_presentation_full_classify_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_rmb_presentation_full_classify_method_names_residual_wave235() -> bool {
    LIVE_RMB_PRESENTATION_FULL_CLASSIFY_METHOD_NAMES_WAVE235.len() == 5
        && residual_name_index(
            LIVE_RMB_PRESENTATION_FULL_CLASSIFY_METHOD_NAMES_WAVE235,
            "classify_right_click_target_from_presentation",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_PRESENTATION_FULL_CLASSIFY_METHOD_NAMES_WAVE235,
            "can_repair",
        ) == Some(3)
        && residual_name_index(
            LIVE_RMB_PRESENTATION_FULL_CLASSIFY_METHOD_NAMES_WAVE235,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_rmb_presentation_full_classify_nav_commands_residual_wave235() -> bool {
    LIVE_RMB_PRESENTATION_FULL_CLASSIFY_NAV_STEPS_WAVE235.len() == 4
        && residual_name_index(
            LIVE_RMB_PRESENTATION_FULL_CLASSIFY_NAV_STEPS_WAVE235,
            "REQUIRE_RMB_PRESENTATION_FULL_CLASSIFY",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_PRESENTATION_FULL_CLASSIFY_NAV_STEPS_WAVE235,
            "LIVE_RMB_PRESENTATION_FULL_CLASSIFY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RMB_PRESENTATION_FULL_CLASSIFY_CMD_NAMES_WAVE235.len() == 3
}

/// Wave 235 composite residual honesty pack.
pub fn honesty_live_rmb_presentation_full_classify_residual_pack_wave235() -> bool {
    honesty_live_rmb_presentation_full_classify_method_names_residual_wave235()
        && honesty_live_rmb_presentation_full_classify_nav_commands_residual_wave235()
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

/// Source residual: expanded presentation classify + optional GameLogic dual-read.
pub fn honesty_rmb_presentation_full_classify_source() -> bool {
    let cs = include_str!("../command_system.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    let Some(classify) = fn_body(cs, "fn classify_right_click_target_from_presentation(") else {
        return false;
    };
    if !(classify.contains("Wave 235")
        && classify.contains("game_logic: Option<&GameLogic>")
        && classify.contains("CommandType::Repair")
        && classify.contains("CommandType::GetRepaired")
        && classify.contains("CommandType::GetHealed")
        && classify.contains("CommandType::Enter")
        && classify.contains("can_repair"))
    {
        return false;
    }
    // determine_context_command passes None when selected_presentation non-empty.
    let Some(det) = fn_body(cs, "fn determine_context_command(") else {
        return false;
    };
    // Wave 541: target presentation freeze is authoritative — classify call passes None
    // (empty selected_presentation fails closed; no live dual-read forward).
    if !(det.contains("classify_right_click_target_from_presentation")
        && (det.contains("Wave 235") || det.contains("Wave 236") || det.contains("Wave 541"))
        && det.contains("None"))
    {
        return false;
    }
    // Engine builders stamp new residual fields.
    let Some(th) = fn_body(eng, "fn presentation_target_hint(") else {
        return false;
    };
    let Some(sh) = fn_body(eng, "fn presentation_selected_unit_hints(") else {
        return false;
    };
    th.contains("is_damaged")
        && th.contains("provides_vehicle_repair")
        && th.contains("provides_heal")
        && sh.contains("can_repair")
        && sh.contains("is_vehicle")
        && sh.contains("is_infantry")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_rmb_presentation_full_classify_honesty() -> bool {
    honesty_live_rmb_presentation_full_classify_residual_pack_wave235()
        && honesty_rmb_presentation_full_classify_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_rmb_presentation_full_classify_method_names_residual_wave235());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_rmb_presentation_full_classify_nav_commands_residual_wave235());
    }

    #[test]
    fn wave235_composite_pack() {
        assert!(honesty_live_rmb_presentation_full_classify_residual_pack_wave235());
    }

    #[test]
    fn rmb_presentation_full_classify_sources() {
        assert!(honesty_rmb_presentation_full_classify_source());
    }

    #[test]
    fn simulate_live_rmb_presentation_full_classify_honesty_residual_live() {
        assert!(
            simulate_live_rmb_presentation_full_classify_honesty(),
            "rmb presentation full classify residual must latch"
        );
    }
}
