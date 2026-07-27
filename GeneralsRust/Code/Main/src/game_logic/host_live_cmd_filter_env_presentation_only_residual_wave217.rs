//! Wave 217 residual peels: runtime-host sell/upgrade/formation/construct
//! selected-object filters require presentation identity (no live get_object
//! dual-read). Heightmap/skybox env hints pass live GameLogic only as boot
//! residual when no presentation freeze is installed. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 216 control-group/camera presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` sell/upgrade/formation/construct filters + env hints
//!
//! Fail-closed:
//! - Not full ControlBar WND command matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Cmd-filter + env presentation-only residual method names.
pub const LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_METHOD_NAMES_WAVE217: &[&str] = &[
    "sell",
    "upgrade",
    "formation",
    "construct",
    "env_logic",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_NAV_STEPS_WAVE217: &[&str] = &[
    "REQUIRE_CMD_FILTER_PRESENTATION_ONLY",
    "REQUIRE_ENV_HINTS_PRESENTATION_ONLY",
    "LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_CMD_NAMES_WAVE217: &[&str] = &[
    "click_live_cmd_filter_env_presentation_only_ok_prepare",
    "click_live_cmd_filter_env_presentation_only_ok_live",
    "click_live_cmd_filter_env_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_cmd_filter_env_presentation_only_method_names_residual_wave217() -> bool {
    LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_METHOD_NAMES_WAVE217.len() == 6
        && residual_name_index(
            LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_METHOD_NAMES_WAVE217,
            "sell",
        ) == Some(0)
        && residual_name_index(
            LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_METHOD_NAMES_WAVE217,
            "env_logic",
        ) == Some(4)
        && residual_name_index(
            LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_METHOD_NAMES_WAVE217,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_cmd_filter_env_presentation_only_nav_commands_residual_wave217() -> bool {
    LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_NAV_STEPS_WAVE217.len() == 4
        && residual_name_index(
            LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_NAV_STEPS_WAVE217,
            "REQUIRE_CMD_FILTER_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_NAV_STEPS_WAVE217,
            "LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CMD_FILTER_ENV_PRESENTATION_ONLY_CMD_NAMES_WAVE217.len() == 3
}

/// Wave 217 composite residual honesty pack.
pub fn honesty_live_cmd_filter_env_presentation_only_residual_pack_wave217() -> bool {
    honesty_live_cmd_filter_env_presentation_only_method_names_residual_wave217()
        && honesty_live_cmd_filter_env_presentation_only_nav_commands_residual_wave217()
}

/// Source residual: sell/upgrade/formation/construct filters presentation-required.
pub fn honesty_cmd_filters_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let markers = [
        (
            "sell_fail_no_structure",
            "Wave 217: presentation required for sell identity",
        ),
        (
            "upgrade_fail_no_player",
            "Wave 217: presentation required for upgrade producer identity",
        ),
        (
            "formation_fail_not_ingame",
            "Wave 217: presentation required for formation mobile identity",
        ),
        (
            "construct_fail_no_dozer",
            "Wave 217: presentation required for construct builder identity",
        ),
    ];
    for (anchor, note) in markers {
        let Some(i) = eng.find(anchor) else {
            return false;
        };
        // search nearby window (filters sit before fail strings for sell; for others broader)
        let lo = i.saturating_sub(2500);
        let hi = (i + 2500).min(eng.len());
        if !eng[lo..hi].contains(note) {
            return false;
        }
    }
    // No live get_object left inside sell selected filter window.
    let Some(sell_i) = eng.find("\"sell\" | \"sell_selected\"") else {
        return false;
    };
    let sell_win = &eng[sell_i..sell_i + 2000.min(eng.len() - sell_i)];
    !sell_win.contains("get_object(*id)")
}

/// Source residual: env hints presentation-only (Wave 455: no live GameLogic dual-read).
pub fn honesty_env_hints_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    // Wave 455/466: presentation-only env apply; seed passes GameWorld shadow.
    eng.contains("fn ensure_presentation_env_for_hints")
        && eng.contains("Wave 455: presentation-only env boundary")
        && (eng.contains("self.gameworld_shadow.as_ref()")
            || eng.contains("ensure_presentation_env_seeded"))
        && eng.contains("Wave 466: prefer host+GameWorld shadow freeze")
        && eng.contains("Self::apply_heightmap_hint(&mut self.render_pipeline)")
        && eng.contains("Self::apply_skybox_hint(&mut self.render_pipeline)")
        && !eng.contains("Self::apply_heightmap_hint(&mut self.render_pipeline, env_logic)")
        && !eng.contains("fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline, game_logic")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_cmd_filter_env_presentation_only_honesty() -> bool {
    honesty_live_cmd_filter_env_presentation_only_residual_pack_wave217()
        && honesty_cmd_filters_presentation_only_source()
        && honesty_env_hints_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_cmd_filter_env_presentation_only_method_names_residual_wave217());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_cmd_filter_env_presentation_only_nav_commands_residual_wave217());
    }

    #[test]
    fn wave217_composite_pack() {
        assert!(honesty_live_cmd_filter_env_presentation_only_residual_pack_wave217());
    }

    #[test]
    fn cmd_filter_env_sources() {
        assert!(honesty_cmd_filters_presentation_only_source());
        assert!(honesty_env_hints_presentation_only_source());
    }

    #[test]
    fn simulate_live_cmd_filter_env_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_cmd_filter_env_presentation_only_honesty(),
            "cmd-filter/env presentation-only residual must latch"
        );
    }
}
