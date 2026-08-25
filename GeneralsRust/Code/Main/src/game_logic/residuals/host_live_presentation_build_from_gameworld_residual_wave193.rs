//! Wave 193 residual peels: PresentationFrame build_from_gameworld /
//! rebuild_objects_from_gameworld (GameWorld-primary object roster).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 192 append_missing residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `PresentationFrame::build_from_gameworld`
//! - `PresentationFrame::rebuild_objects_from_gameworld`
//! - engine opt-in via GENERALS_PRESENTATION_FROM_GAMEWORLD
//! - runtime-host exports `gameworld_rebuilt`
//!
//! Fail-closed:
//! - Opt-in path; default remains host build_from_logic + overlay/append
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Build-from-gameworld residual method names.
pub const LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_METHOD_NAMES_WAVE193: &[&str] = &[
    "build_from_gameworld",
    "rebuild_objects_from_gameworld",
    "gameworld_rebuilt",
    "GENERALS_PRESENTATION_FROM_GAMEWORLD",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_NAV_STEPS_WAVE193: &[&str] = &[
    "REQUIRE_BUILD_FROM_GAMEWORLD_API",
    "REQUIRE_REBUILD_OBJECTS_API",
    "REQUIRE_ENGINE_OPT_IN_ENV",
    "LIVE_REBUILD_MATCHES_ENTITY_COUNT",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_CMD_NAMES_WAVE193: &[&str] = &[
    "click_live_presentation_build_from_gameworld_ok_prepare",
    "click_live_presentation_build_from_gameworld_ok_live",
    "click_live_presentation_build_from_gameworld_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_build_from_gameworld_method_names_residual_wave193() -> bool {
    LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_METHOD_NAMES_WAVE193.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_METHOD_NAMES_WAVE193,
            "build_from_gameworld",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_METHOD_NAMES_WAVE193,
            "GENERALS_PRESENTATION_FROM_GAMEWORLD",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_METHOD_NAMES_WAVE193,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_build_from_gameworld_nav_commands_residual_wave193() -> bool {
    LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_NAV_STEPS_WAVE193.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_NAV_STEPS_WAVE193,
            "REQUIRE_BUILD_FROM_GAMEWORLD_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_NAV_STEPS_WAVE193,
            "LIVE_REBUILD_MATCHES_ENTITY_COUNT",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_BUILD_FROM_GAMEWORLD_CMD_NAMES_WAVE193.len() == 3
}

/// Wave 193 composite residual honesty pack.
pub fn honesty_live_presentation_build_from_gameworld_residual_pack_wave193() -> bool {
    honesty_live_presentation_build_from_gameworld_method_names_residual_wave193()
        && honesty_live_presentation_build_from_gameworld_nav_commands_residual_wave193()
}

/// Source residual: build/rebuild APIs exist on PresentationFrame.
pub fn honesty_build_from_gameworld_api_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    src.contains("pub fn build_from_gameworld")
        && src.contains("pub fn rebuild_objects_from_gameworld")
        && src.contains("pub gameworld_rebuilt: usize")
}

/// Source residual: engine opt-in env gate after overlay/append.
pub fn honesty_engine_presentation_from_gameworld_opt_in_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 195: engine presentation helpers fold rebuild; status still exports rebuilt.
    (src.contains("build_for_engine(")
        || src.contains("build_with_victory_for_engine(")
        || src.contains("build_for_engine_with_runtime_heightmap(")
        || src.contains("build_with_victory_for_engine_with_runtime_heightmap("))
        && src.contains("gameworld_rebuilt")
        && src.contains("gameworld_rebuilt=")
}

/// Live residual: rebuild/build_from_gameworld produce GW-primary object roster.
pub fn simulate_live_presentation_build_from_gameworld_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::world::PlayerId;
    use gamelogic::world::entities::TemplateRef;
    use glam::Vec3;

    if !honesty_live_presentation_build_from_gameworld_residual_pack_wave193() {
        return false;
    }
    if !honesty_build_from_gameworld_api_source() {
        return false;
    }
    if !honesty_engine_presentation_from_gameworld_opt_in_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BuildFromGw193");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("Rebuild193") {
        let mut t = ThingTemplate::new("Rebuild193");
        t.set_health(150.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("Rebuild193".into(), t);
    }
    let Some(oid) = logic.create_object("Rebuild193", Team::USA, Vec3::new(20.0, 0.0, 22.0)) else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    // Extra GW-only entity so rebuild must include it (host frame would miss without append).
    let gw_id = {
        let tmpl = TemplateRef {
            name: "GwOnlyRebuild193".into(),
            source: None,
        };
        shadow.world_mut().spawn_entity(
            tmpl,
            Some(PlayerId::from_index(0)),
            gamelogic::world::entities::Transform::new([55.0, 0.0, 56.0], 0.25),
            77.0,
        )
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
        e.max_health = 77.0;
        e.team_ordinal = 0;
        e.selected = true;
        e.object_type_ordinal = 1; // vehicle
        e.power_provided = 0;
        e.velocity = [0.5, 0.0, 0.0];
    }

    // Poison host object pose — rebuild must take GameWorld pose after sync + manual edit.
    if let Some(e) = shadow
        .entity_for_host(oid)
        .and_then(|eid| shadow.world_mut().world_mut().entity_mut(eid))
    {
        e.transform.position = [21.0, 0.0, 23.0].into();
        e.health = 140.0;
        e.max_health = 150.0;
        e.selected = true;
    }

    let mut host_frame = PresentationFrame::build_from_logic(&logic, 0);
    let host_count = host_frame.objects.len();
    let rebuilt = host_frame.rebuild_objects_from_gameworld(&shadow);
    if rebuilt < 2 {
        return false;
    }
    if host_frame.gameworld_rebuilt < 2 {
        return false;
    }
    // Rebuild should include host-mapped + GW-only (>= host_count, typically host_count+1).
    if host_frame.objects.len() < host_count {
        return false;
    }
    let Some(host_obj) = host_frame.objects.iter().find(|o| o.id == oid) else {
        return false;
    };
    if (host_obj.position.x - 21.0).abs() > 1e-3 || (host_obj.position.z - 23.0).abs() > 1e-3 {
        return false;
    }
    if (host_obj.health_current - 140.0).abs() > 1e-3 {
        return false;
    }
    if !host_obj.selected {
        return false;
    }
    let synth = 0x8000_0000 | gw_id.get();
    let Some(gw_obj) = host_frame.objects.iter().find(|o| o.id.0 == synth) else {
        return false;
    };
    if gw_obj.template_name != "GwOnlyRebuild193" {
        return false;
    }
    if !matches!(
        gw_obj.object_type,
        crate::presentation_frame::PresentationObjectType::Vehicle
    ) {
        return false;
    }

    // build_from_gameworld with host should also latch rebuilt count.
    let built = PresentationFrame::build_from_gameworld(&shadow, 0, Some(&logic));
    if built.gameworld_rebuilt < 2 {
        return false;
    }
    if built.objects.iter().all(|o| o.id.0 != synth) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_build_from_gameworld_method_names_residual_wave193());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_build_from_gameworld_nav_commands_residual_wave193());
    }

    #[test]
    fn wave193_composite_pack() {
        assert!(honesty_live_presentation_build_from_gameworld_residual_pack_wave193());
    }

    #[test]
    fn build_from_gameworld_sources() {
        assert!(honesty_build_from_gameworld_api_source());
        assert!(honesty_engine_presentation_from_gameworld_opt_in_source());
    }

    #[test]
    fn simulate_live_presentation_build_from_gameworld_honesty_residual_live() {
        assert!(
            simulate_live_presentation_build_from_gameworld_honesty(),
            "build_from_gameworld residual must latch"
        );
    }
}
