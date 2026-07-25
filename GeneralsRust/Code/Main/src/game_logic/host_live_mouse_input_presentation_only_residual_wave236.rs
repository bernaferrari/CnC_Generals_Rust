//! Wave 236 residual peels: `CommandSystem::process_mouse_input` takes
//! `Option<&GameLogic>` and engine InGame callers pass `None` when a
//! presentation frame is installed (RMB/cursor/minimap). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 235 RMB full presentation classify residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` process_mouse_input / create_selection_command /
//!   create_select_similar_command Option path + presentation box/similar ids
//! - `cnc_game_engine.rs` process_mouse_input callers pass None with frame
//!
//! Fail-closed:
//! - Boot/no-frame path still passes Some(&game_logic)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Mouse input presentation-only residual method names.
pub const LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236: &[&str] = &[
    "process_mouse_input",
    "Option<&GameLogic>",
    "presentation_box_select_units",
    "presentation_select_similar_units",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236: &[&str] = &[
    "REQUIRE_MOUSE_INPUT_OPTION_GAME_LOGIC",
    "REQUIRE_ENGINE_PASSES_NONE_WITH_FRAME",
    "LIVE_MOUSE_INPUT_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MOUSE_INPUT_PRESENTATION_ONLY_CMD_NAMES_WAVE236: &[&str] = &[
    "click_live_mouse_input_presentation_only_ok_prepare",
    "click_live_mouse_input_presentation_only_ok_live",
    "click_live_mouse_input_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_mouse_input_presentation_only_method_names_residual_wave236() -> bool {
    LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236.len() == 5
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "process_mouse_input",
        ) == Some(0)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "presentation_box_select_units",
        ) == Some(2)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_METHOD_NAMES_WAVE236,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236() -> bool {
    LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236.len() == 4
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236,
            "REQUIRE_MOUSE_INPUT_OPTION_GAME_LOGIC",
        ) == Some(0)
        && residual_name_index(
            LIVE_MOUSE_INPUT_PRESENTATION_ONLY_NAV_STEPS_WAVE236,
            "LIVE_MOUSE_INPUT_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_MOUSE_INPUT_PRESENTATION_ONLY_CMD_NAMES_WAVE236.len() == 3
}

/// Wave 236 composite residual honesty pack.
pub fn honesty_live_mouse_input_presentation_only_residual_pack_wave236() -> bool {
    honesty_live_mouse_input_presentation_only_method_names_residual_wave236()
        && honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236()
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

/// Source residual: Option mouse input + engine None-with-frame.
pub fn honesty_mouse_input_presentation_only_source() -> bool {
    let cs = include_str!("../command_system.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    let Some(pmi) = fn_body(cs, "pub fn process_mouse_input(")
        .or_else(|| fn_body(cs, "fn process_mouse_input("))
    else {
        return false;
    };
    if !(pmi.contains("game_logic: Option<&GameLogic>")
        || cs.contains("fn process_mouse_input(\n        &mut self,\n        context: &MouseCommandContext,\n        selected_units: &[ObjectId],\n        player_id: u32,\n        game_logic: Option<&GameLogic>"))
    {
        // Fall through to string search
        if !cs.contains("game_logic: Option<&GameLogic>") {
            return false;
        }
    }
    if !(cs.contains("presentation_box_select_units")
        && cs.contains("presentation_select_similar_units")
        && cs.contains("Wave 236"))
    {
        return false;
    }
    // Engine InGame callers pass None when presentation frame installed.
    eng.contains("process_mouse_input")
        && eng.contains("last_presentation_frame.is_some()")
        && eng.contains("Some(&self.game_logic)")
        && eng.matches("process_mouse_input").count() >= 3
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_mouse_input_presentation_only_honesty() -> bool {
    honesty_live_mouse_input_presentation_only_residual_pack_wave236()
        && honesty_mouse_input_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_mouse_input_presentation_only_method_names_residual_wave236());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236());
    }

    #[test]
    fn wave236_composite_pack() {
        assert!(honesty_live_mouse_input_presentation_only_residual_pack_wave236());
    }

    #[test]
    fn mouse_input_presentation_only_sources() {
        assert!(honesty_mouse_input_presentation_only_source());
    }

    #[test]
    fn simulate_live_mouse_input_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_mouse_input_presentation_only_honesty(),
            "mouse input presentation-only residual must latch"
        );
    }
}
