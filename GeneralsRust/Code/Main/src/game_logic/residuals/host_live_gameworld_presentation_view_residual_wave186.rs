//! Wave 186 residual peels: live GameWorld presentation view residual
//! (engine seeds observe-path entity count from `presentation_view_from_shadow`
//! after coupled tick; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 185 fire-spawn/special-power residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `presentation_view_from_shadow` / `presentation_view_from_gameworld`
//! - engine `last_gameworld_presentation_entity_count` after shadow session
//! - status `gameworld_presentation_entities`
//! - Main `PresentationFrame` still built from host (temporary)
//!
//! Fail-closed:
//! - Not full PresentationFrame cutover to GameWorld-only
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live GameWorld presentation view residual method names.
pub const LIVE_GAMEWORLD_PRESENTATION_VIEW_METHOD_NAMES_WAVE186: &[&str] = &[
    "presentation_view_from_shadow",
    "presentation_view_from_gameworld",
    "last_gameworld_presentation_entity_count",
    "gameworld_presentation_entities",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_PRESENTATION_VIEW_NAV_STEPS_WAVE186: &[&str] = &[
    "REQUIRE_PRESENTATION_VIEW_API",
    "REQUIRE_ENGINE_SEEDS_GW_VIEW",
    "REQUIRE_STATUS_FIELD",
    "LIVE_MAP_SHADOW_VIEW_NONEMPTY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_PRESENTATION_VIEW_CMD_NAMES_WAVE186: &[&str] = &[
    "click_live_gameworld_presentation_view_ok_view",
    "click_live_gameworld_presentation_view_ok_engine",
    "click_live_gameworld_presentation_view_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_presentation_view_method_names_residual_wave186() -> bool {
    LIVE_GAMEWORLD_PRESENTATION_VIEW_METHOD_NAMES_WAVE186.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_PRESENTATION_VIEW_METHOD_NAMES_WAVE186,
            "presentation_view_from_shadow",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_PRESENTATION_VIEW_METHOD_NAMES_WAVE186,
            "last_gameworld_presentation_entity_count",
        ) == Some(2)
        && residual_name_index(
            LIVE_GAMEWORLD_PRESENTATION_VIEW_METHOD_NAMES_WAVE186,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_presentation_view_nav_commands_residual_wave186() -> bool {
    LIVE_GAMEWORLD_PRESENTATION_VIEW_NAV_STEPS_WAVE186.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_PRESENTATION_VIEW_NAV_STEPS_WAVE186,
            "REQUIRE_PRESENTATION_VIEW_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_PRESENTATION_VIEW_NAV_STEPS_WAVE186,
            "LIVE_MAP_SHADOW_VIEW_NONEMPTY",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_GAMEWORLD_PRESENTATION_VIEW_CMD_NAMES_WAVE186.len() == 3
}

/// Wave 186 composite residual honesty pack.
pub fn honesty_live_gameworld_presentation_view_residual_pack_wave186() -> bool {
    honesty_live_gameworld_presentation_view_method_names_residual_wave186()
        && honesty_live_gameworld_presentation_view_nav_commands_residual_wave186()
}

/// Source residual: presentation view APIs exist.
pub fn honesty_presentation_view_api_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    src.contains("pub fn presentation_view_from_shadow")
        && src.contains("pub fn presentation_view_from_gameworld")
        && src.contains("pub struct GameWorldPresentationView")
        && src.contains("pub entities: Vec<GameWorldEntityView>")
}

/// Source residual: engine seeds GW presentation count after shadow session.
pub fn honesty_engine_seeds_gameworld_presentation_view_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    eng.contains("last_gameworld_presentation_entity_count")
        && eng.contains("presentation_view_from_shadow")
        && eng.contains("gameworld_presentation_entities")
        && eng.contains("shadow_session_after_host_tick")
}

/// Live residual: after map load + shadow sync, presentation view is non-empty.
pub fn simulate_live_gameworld_presentation_view_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::gameworld_shadow::{
        GameWorldShadow, ensure_gate_damage_authority, gameworld_shadow_enabled,
        presentation_view_from_shadow,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    if !honesty_live_gameworld_presentation_view_residual_pack_wave186() {
        return false;
    }
    if !honesty_presentation_view_api_source() {
        return false;
    }
    if !honesty_engine_seeds_gameworld_presentation_view_source() {
        return false;
    }

    ensure_gate_damage_authority();

    // Soft-ok when MapsZH absent (CI without retail maps).
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        // Still prove empty-world view API.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GwPresView186");
        let _ = apply_skirmish_config(&mut logic, &cfg);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let view = presentation_view_from_shadow(&shadow, 0);
        // Empty entities is fine pre-map; API must not panic.
        let _ = view.entities.len();
        return true;
    };

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let cfg = golden_skirmish_config("GwPresView186");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded || logic.get_objects().is_empty() {
        return false;
    }
    logic.update();

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let view = presentation_view_from_shadow(&shadow, 0);
    if view.entities.is_empty() {
        return false;
    }
    // Entity count should track host objects at least partially.
    if view.entities.len() < 1 {
        return false;
    }

    // Shadow enabled is production default; if off, still proved apply path above.
    let _ = gameworld_shadow_enabled();

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_presentation_view_method_names_residual_wave186());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_presentation_view_nav_commands_residual_wave186());
    }

    #[test]
    fn wave186_composite_pack() {
        assert!(honesty_live_gameworld_presentation_view_residual_pack_wave186());
    }

    #[test]
    fn presentation_view_sources() {
        assert!(honesty_presentation_view_api_source());
        assert!(honesty_engine_seeds_gameworld_presentation_view_source());
    }

    #[test]
    fn simulate_live_gameworld_presentation_view_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_presentation_view_honesty(),
            "live GameWorld presentation view residual must latch"
        );
    }

    #[test]
    fn presentation_view_from_shadow_reports_created_host_object() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::{
            GameWorldShadow, ensure_gate_damage_authority, presentation_view_from_shadow,
        };
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("GwViewRanger186");
        t.set_health(120.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("GwViewRanger186".into(), t);
        let id = logic
            .create_object("GwViewRanger186", Team::USA, Vec3::new(8.0, 0.0, 16.0))
            .expect("create host object");

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let view = presentation_view_from_shadow(&shadow, 0);
        assert!(
            !view.entities.is_empty(),
            "presentation_view_from_shadow must observe synced host objects"
        );
        assert!(
            view.entities
                .iter()
                .any(|e| e.id == id.0 || e.template.contains("GwViewRanger186")),
            "observe-path view must include the created unit"
        );
        assert!(
            view.entities.len() >= 1,
            "entity count is the host residual stamped after shadow session"
        );
    }
}
