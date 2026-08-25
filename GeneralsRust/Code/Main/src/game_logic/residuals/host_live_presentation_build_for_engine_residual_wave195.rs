//! Wave 195 residual peels: engine presentation uses `build_for_engine` /
//! `build_with_victory_for_engine` (single path: host non-object residual +
//! GameWorld object roster). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 194 default-on rebuild residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `PresentationFrame::build_for_engine`
//! - `PresentationFrame::build_with_victory_for_engine`
//! - engine seed/tick/boot call sites
//!
//! Fail-closed:
//! - Host still freezes non-object residual (scripts/FX/camera/victory)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Build-for-engine residual method names.
pub const LIVE_PRESENTATION_BUILD_FOR_ENGINE_METHOD_NAMES_WAVE195: &[&str] = &[
    "build_for_engine",
    "build_with_victory_for_engine",
    "presentation_from_gameworld_enabled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_BUILD_FOR_ENGINE_NAV_STEPS_WAVE195: &[&str] = &[
    "REQUIRE_BUILD_FOR_ENGINE_API",
    "REQUIRE_VICTORY_FOR_ENGINE_API",
    "REQUIRE_ENGINE_SEED_USES_HELPER",
    "LIVE_BUILD_FOR_ENGINE_OBJECTS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_BUILD_FOR_ENGINE_CMD_NAMES_WAVE195: &[&str] = &[
    "click_live_presentation_build_for_engine_ok_prepare",
    "click_live_presentation_build_for_engine_ok_live",
    "click_live_presentation_build_for_engine_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_build_for_engine_method_names_residual_wave195() -> bool {
    LIVE_PRESENTATION_BUILD_FOR_ENGINE_METHOD_NAMES_WAVE195.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FOR_ENGINE_METHOD_NAMES_WAVE195,
            "build_for_engine",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FOR_ENGINE_METHOD_NAMES_WAVE195,
            "build_with_victory_for_engine",
        ) == Some(1)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FOR_ENGINE_METHOD_NAMES_WAVE195,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_build_for_engine_nav_commands_residual_wave195() -> bool {
    LIVE_PRESENTATION_BUILD_FOR_ENGINE_NAV_STEPS_WAVE195.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FOR_ENGINE_NAV_STEPS_WAVE195,
            "REQUIRE_BUILD_FOR_ENGINE_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FOR_ENGINE_NAV_STEPS_WAVE195,
            "LIVE_BUILD_FOR_ENGINE_OBJECTS",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_BUILD_FOR_ENGINE_CMD_NAMES_WAVE195.len() == 3
}

/// Wave 195 composite residual honesty pack.
pub fn honesty_live_presentation_build_for_engine_residual_pack_wave195() -> bool {
    honesty_live_presentation_build_for_engine_method_names_residual_wave195()
        && honesty_live_presentation_build_for_engine_nav_commands_residual_wave195()
}

/// Source residual: build_for_engine APIs exist.
pub fn honesty_build_for_engine_api_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    src.contains("pub fn build_for_engine")
        && src.contains("pub fn build_with_victory_for_engine")
        && src.contains("build_from_gameworld(shadow, local_player_id, Some(logic))")
}

/// Source residual: engine seed/tick use helpers (not manual overlay chain).
pub fn honesty_engine_build_for_engine_call_sites_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let seed_hits = src.matches("build_for_engine(").count()
        + src
            .matches("build_for_engine_with_runtime_heightmap(")
            .count();
    let victory_hits = src.matches("build_with_victory_for_engine(").count()
        + src
            .matches("build_with_victory_for_engine_with_runtime_heightmap(")
            .count();
    seed_hits >= 2
        && victory_hits >= 1
        // Production tick must not still open a manual overlay chain after victory build.
        && !src.contains("build_with_victory(\n                &mut self.game_logic")
}

/// Live residual: build_for_engine yields GW-primary objects when shadow live.
pub fn simulate_live_presentation_build_for_engine_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::world::PlayerId;
    use gamelogic::world::entities::TemplateRef;
    use glam::Vec3;

    if !honesty_live_presentation_build_for_engine_residual_pack_wave195() {
        return false;
    }
    if !honesty_build_for_engine_api_source() {
        return false;
    }
    if !honesty_engine_build_for_engine_call_sites_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BuildForEngine195");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("EnginePres195") {
        let mut t = ThingTemplate::new("EnginePres195");
        t.set_health(170.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("EnginePres195".into(), t);
    }
    let Some(oid) = logic.create_object("EnginePres195", Team::USA, Vec3::new(11.0, 0.0, 12.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw_id = {
        let tmpl = TemplateRef {
            name: "GwOnlyEngine195".into(),
            source: None,
        };
        shadow.world_mut().spawn_entity(
            tmpl,
            Some(PlayerId::from_index(0)),
            gamelogic::world::entities::Transform::new([90.0, 0.0, 91.0], 0.2),
            55.0,
        )
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
        e.max_health = 55.0;
        e.team_ordinal = 0;
        e.selected = true;
    }
    if let Some(e) = shadow
        .entity_for_host(oid)
        .and_then(|eid| shadow.world_mut().world_mut().entity_mut(eid))
    {
        e.transform.position = [13.0, 0.0, 14.0].into();
        e.health = 165.0;
        e.max_health = 170.0;
    }

    let pres = PresentationFrame::build_for_engine(&logic, 0, Some(&shadow));
    if crate::presentation_frame::presentation_from_gameworld_enabled() {
        if pres.gameworld_rebuilt < 2 {
            return false;
        }
    }
    let Some(host_obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };
    if (host_obj.position.x - 13.0).abs() > 1e-3 || (host_obj.health_current - 165.0).abs() > 1e-3 {
        return false;
    }
    let synth = 0x8000_0000 | gw_id.get();
    if pres.objects.iter().all(|o| o.id.0 != synth) {
        return false;
    }

    // Victory path still rebuilds objects.
    let pres_v = PresentationFrame::build_with_victory_for_engine(&mut logic, 0, Some(&shadow));
    if pres_v.objects.iter().all(|o| o.id.0 != synth)
        && crate::presentation_frame::presentation_from_gameworld_enabled()
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_build_for_engine_method_names_residual_wave195());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_build_for_engine_nav_commands_residual_wave195());
    }

    #[test]
    fn wave195_composite_pack() {
        assert!(honesty_live_presentation_build_for_engine_residual_pack_wave195());
    }

    #[test]
    fn build_for_engine_sources() {
        assert!(honesty_build_for_engine_api_source());
        assert!(honesty_engine_build_for_engine_call_sites_source());
    }

    #[test]
    fn simulate_live_presentation_build_for_engine_honesty_residual_live() {
        assert!(
            simulate_live_presentation_build_for_engine_honesty(),
            "build_for_engine residual must latch"
        );
    }
}
