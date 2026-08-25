//! Wave 192 residual peels: PresentationFrame append_missing_from_gameworld
//! (sparse RenderableObject from live Entity store for host-missing entities).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 191 enriched GameWorldEntityView residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `PresentationFrame::renderable_from_gameworld_entity`
//! - `PresentationFrame::append_missing_from_gameworld`
//! - engine seeds call append after overlay
//! - runtime-host exports `gameworld_appended`
//!
//! Fail-closed:
//! - Not full `build_from_gameworld` cutover
//! - Sparse objects omit weapons/FOW/FX host detail
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Append-missing residual method names.
pub const LIVE_PRESENTATION_APPEND_MISSING_METHOD_NAMES_WAVE192: &[&str] = &[
    "renderable_from_gameworld_entity",
    "append_missing_from_gameworld",
    "gameworld_appended",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_APPEND_MISSING_NAV_STEPS_WAVE192: &[&str] = &[
    "REQUIRE_RENDERABLE_FROM_ENTITY",
    "REQUIRE_APPEND_MISSING_API",
    "REQUIRE_ENGINE_APPEND_AFTER_OVERLAY",
    "LIVE_APPEND_CREATES_OBJECT",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_APPEND_MISSING_CMD_NAMES_WAVE192: &[&str] = &[
    "click_live_presentation_append_missing_ok_prepare",
    "click_live_presentation_append_missing_ok_live",
    "click_live_presentation_append_missing_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_append_missing_method_names_residual_wave192() -> bool {
    LIVE_PRESENTATION_APPEND_MISSING_METHOD_NAMES_WAVE192.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_APPEND_MISSING_METHOD_NAMES_WAVE192,
            "renderable_from_gameworld_entity",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_APPEND_MISSING_METHOD_NAMES_WAVE192,
            "gameworld_appended",
        ) == Some(2)
        && residual_name_index(
            LIVE_PRESENTATION_APPEND_MISSING_METHOD_NAMES_WAVE192,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_append_missing_nav_commands_residual_wave192() -> bool {
    LIVE_PRESENTATION_APPEND_MISSING_NAV_STEPS_WAVE192.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_APPEND_MISSING_NAV_STEPS_WAVE192,
            "REQUIRE_RENDERABLE_FROM_ENTITY",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_APPEND_MISSING_NAV_STEPS_WAVE192,
            "LIVE_APPEND_CREATES_OBJECT",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_PRESENTATION_APPEND_MISSING_CMD_NAMES_WAVE192.len() == 3
}

/// Wave 192 composite residual honesty pack.
pub fn honesty_live_presentation_append_missing_residual_pack_wave192() -> bool {
    honesty_live_presentation_append_missing_method_names_residual_wave192()
        && honesty_live_presentation_append_missing_nav_commands_residual_wave192()
}

/// Source residual: sparse builder + append API exist on PresentationFrame.
pub fn honesty_append_missing_api_source() -> bool {
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    src.contains("pub fn renderable_from_gameworld_entity")
        && src.contains("pub fn append_missing_from_gameworld")
        && src.contains("pub gameworld_appended: usize")
        && src.contains("0x8000_0000")
}

/// Source residual: engine calls append after overlay on seed paths.
pub fn honesty_engine_append_after_overlay_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 195: append/rebuild folded into build_for_engine helpers.
    (src.contains("build_for_engine(")
        || src.contains("build_with_victory_for_engine(")
        || src.contains("build_for_engine_with_runtime_heightmap(")
        || src.contains("build_with_victory_for_engine_with_runtime_heightmap("))
        && src.contains("gameworld_appended")
        && crate::presentation_frame::PRESENTATION_FRAME_SRC
            .contains("append_missing_from_gameworld")
}

/// Live residual: append creates a frame object for a shadow-only entity.
pub fn simulate_live_presentation_append_missing_honesty() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::world::PlayerId;
    use gamelogic::world::entities::TemplateRef;
    use glam::Vec3;

    if !honesty_live_presentation_append_missing_residual_pack_wave192() {
        return false;
    }
    if !honesty_append_missing_api_source() {
        return false;
    }
    if !honesty_engine_append_after_overlay_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AppendMissing192");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("AppendOnly192") {
        let mut t = ThingTemplate::new("AppendOnly192");
        t.set_health(120.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("AppendOnly192".into(), t);
    }
    // Host object so build_from_logic has baseline roster.
    let Some(_host_oid) =
        logic.create_object("AppendOnly192", Team::USA, Vec3::new(10.0, 0.0, 12.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    // Spawn a GameWorld-only entity with no host map entry.
    let gw_id = {
        let tmpl = TemplateRef {
            name: "GwOnlyAppend192".into(),
            source: None,
        };
        // GameWorld::spawn_entity is the public facade (Transform is crate-private).
        shadow.world_mut().spawn_entity(
            tmpl,
            Some(PlayerId::from_index(0)),
            gamelogic::world::entities::Transform::new([40.0, 0.0, 41.0], 0.5),
            88.0,
        )
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
        e.max_health = 88.0;
        e.team_ordinal = 0;
        e.selected = true;
        e.velocity = [0.0, 0.0, 1.25];
        e.move_max_speed = 18.0;
        e.team_color = [0.1, 0.2, 0.9, 1.0];
        e.body_damage_state = 0;
    }

    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    let before = pres.objects.len();
    let _ = pres.overlay_gameworld_shadow(&shadow);
    let appended = pres.append_missing_from_gameworld(&shadow);
    if appended == 0 {
        return false;
    }
    if pres.gameworld_appended == 0 {
        return false;
    }
    if pres.objects.len() <= before {
        return false;
    }
    let synth_id = 0x8000_0000 | gw_id.get();
    let Some(obj) = pres.objects.iter().find(|o| o.id.0 == synth_id) else {
        return false;
    };
    if obj.template_name != "GwOnlyAppend192" {
        return false;
    }
    if (obj.position.x - 40.0).abs() > 1e-3 || (obj.position.z - 41.0).abs() > 1e-3 {
        return false;
    }
    if (obj.health_current - 88.0).abs() > 1e-3 {
        return false;
    }
    if !obj.selected || (obj.velocity.z - 1.25).abs() > 1e-3 {
        return false;
    }
    if obj.team_color != [0.1, 0.2, 0.9, 1.0] {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_append_missing_method_names_residual_wave192());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_append_missing_nav_commands_residual_wave192());
    }

    #[test]
    fn wave192_composite_pack() {
        assert!(honesty_live_presentation_append_missing_residual_pack_wave192());
    }

    #[test]
    fn append_missing_sources() {
        assert!(honesty_append_missing_api_source());
        assert!(honesty_engine_append_after_overlay_source());
    }

    #[test]
    fn simulate_live_presentation_append_missing_honesty_residual_live() {
        assert!(
            simulate_live_presentation_append_missing_honesty(),
            "append_missing_from_gameworld residual must latch"
        );
    }
}
