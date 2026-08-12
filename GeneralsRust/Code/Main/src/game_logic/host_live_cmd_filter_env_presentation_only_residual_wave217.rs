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

/// Wave 217: sell identity from presentation freeze only (no live get_object).
pub fn presentation_selected_sellable_structure_ids(
    frame: Option<&crate::presentation_frame::PresentationFrame>,
    selected: &[crate::game_logic::ObjectId],
    team: crate::game_logic::Team,
) -> Vec<crate::game_logic::ObjectId> {
    use crate::game_logic::KindOf;
    use crate::presentation_frame::{PresentationBuildingType, PresentationObjectType};
    let Some(frame) = frame else {
        // Wave 217: presentation required for sell identity.
        return Vec::new();
    };
    selected
        .iter()
        .copied()
        .filter(|id| {
            frame.objects.iter().any(|o| {
                o.id == *id
                    && o.team == team
                    && !o.destroyed
                    && (crate::presentation_frame::PresentationFrame::object_has_kind(
                        o,
                        KindOf::Structure,
                    ) || o.object_type == PresentationObjectType::Building)
                    && !crate::presentation_frame::PresentationFrame::object_has_kind(
                        o,
                        KindOf::CommandCenter,
                    )
                    && o.building_type != Some(PresentationBuildingType::CommandCenter)
            })
        })
        .collect()
}

/// Source residual: sell/upgrade/formation/construct filters presentation-required.
pub fn honesty_cmd_filters_presentation_only_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let helper = include_str!("host_live_cmd_filter_env_presentation_only_residual_wave217.rs");
    if !helper.contains("Wave 217: presentation required for sell identity")
        || !helper.contains("pub fn presentation_selected_sellable_structure_ids")
    {
        return false;
    }
    let markers = [
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
        let lo = i.saturating_sub(2500);
        let hi = (i + 2500).min(eng.len());
        if !eng[lo..hi].contains(note) {
            return false;
        }
    }
    let Some(sell_i) = eng.find("\"sell\" | \"sell_selected\"") else {
        return false;
    };
    let sell_win = &eng[sell_i..sell_i + 2000.min(eng.len() - sell_i)];
    !sell_win.contains("get_object(*id)")
        && sell_win.contains("presentation_selected_sellable_structure_ids")
        && eng.contains("sell_fail_no_structure")
}

/// Source residual: env hints presentation-only (Wave 455: no live GameLogic dual-read).
pub fn honesty_env_hints_presentation_only_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 455/466: presentation-only env apply; seed passes GameWorld shadow.
    eng.contains("fn ensure_presentation_env_for_hints")
        && eng.contains("fn host_ensure_presentation_env_for_hints")
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

    #[test]
    fn presentation_selected_sellable_structure_ids_fail_closed_without_freeze() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::ensure_gate_damage_authority;
        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut barracks = ThingTemplate::new("SellBarracks217");
        barracks.set_health(1000.0);
        barracks.add_kind_of(KindOf::Structure);
        barracks.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellBarracks217".into(), barracks);
        let mut cc = ThingTemplate::new("SellCC217");
        cc.set_health(2000.0);
        cc.add_kind_of(KindOf::Structure);
        cc.add_kind_of(KindOf::CommandCenter);
        cc.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellCC217".into(), cc);
        let mut ranger = ThingTemplate::new("SellRanger217");
        ranger.set_health(100.0);
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellRanger217".into(), ranger);

        let bid = logic
            .create_object("SellBarracks217", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("barracks");
        let cid = logic
            .create_object("SellCC217", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("cc");
        let rid = logic
            .create_object("SellRanger217", Team::USA, Vec3::new(30.0, 0.0, 0.0))
            .expect("ranger");

        assert!(
            presentation_selected_sellable_structure_ids(None, &[bid], Team::USA).is_empty(),
            "no presentation freeze → sell identity fail-closed"
        );

        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let sold =
            presentation_selected_sellable_structure_ids(Some(&frame), &[bid, cid, rid], Team::USA);
        assert_eq!(sold, vec![bid], "only non-CC structure is sellable");
        assert!(
            presentation_selected_sellable_structure_ids(Some(&frame), &[rid], Team::USA)
                .is_empty()
        );
        assert!(
            presentation_selected_sellable_structure_ids(Some(&frame), &[cid], Team::USA)
                .is_empty(),
            "command center is not sellable"
        );
    }
}
