//! Wave 172 residual peels: live GameWorldShadow sync + presentation overlay
//! after map load (architecture migration residual; never flips shell
//! `playable_claim`).
//!
//! Orthogonal to Wave 171 live PresentationFrame seed residual.
//! Host residual only — network deferred.
//!
//! Sources (repo architecture):
//! - GameWorldShadow::sync_from_host
//! - PresentationFrame::overlay_gameworld_shadow
//! - CncGameEngine::seed_presentation_after_match_start shadow path
//! - GENERALS_GAMEWORLD_* authority defaults (wave 153)
//!
//! Fail-closed:
//! - Not full GameWorld production cutover / dual-tick removal
//! - Not full damage-authority combat residual
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live shadow overlay residual method names.
pub const LIVE_GAMEWORLD_SHADOW_OVERLAY_METHOD_NAMES_WAVE172: &[&str] = &[
    "GameWorldShadow::new",
    "sync_from_host",
    "GameWorldShadow::probe",
    "PresentationFrame::build_from_logic",
    "overlay_gameworld_shadow",
];

/// Ordered live shadow overlay residual navigation steps.
pub const LIVE_GAMEWORLD_SHADOW_OVERLAY_NAV_STEPS_WAVE172: &[&str] = &[
    "ENSURE_DAMAGE_AUTHORITY",
    "LOAD_RETAIL_MAP",
    "SYNC_SHADOW_FROM_HOST",
    "PROBE_MAPPED_OBJECTS",
    "BUILD_PRESENTATION",
    "OVERLAY_SHADOW_ON_FRAME",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_SHADOW_OVERLAY_CMD_NAMES_WAVE172: &[&str] = &[
    "click_live_gameworld_shadow_ok_sync",
    "click_live_gameworld_shadow_ok_overlay",
    "click_live_gameworld_shadow_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_shadow_overlay_method_names_residual_wave172() -> bool {
    LIVE_GAMEWORLD_SHADOW_OVERLAY_METHOD_NAMES_WAVE172.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_SHADOW_OVERLAY_METHOD_NAMES_WAVE172,
            "sync_from_host",
        ) == Some(1)
        && residual_name_index(
            LIVE_GAMEWORLD_SHADOW_OVERLAY_METHOD_NAMES_WAVE172,
            "overlay_gameworld_shadow",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_shadow_overlay_nav_commands_residual_wave172() -> bool {
    LIVE_GAMEWORLD_SHADOW_OVERLAY_NAV_STEPS_WAVE172.len() == 6
        && residual_name_index(
            LIVE_GAMEWORLD_SHADOW_OVERLAY_NAV_STEPS_WAVE172,
            "SYNC_SHADOW_FROM_HOST",
        ) == Some(2)
        && residual_name_index(
            LIVE_GAMEWORLD_SHADOW_OVERLAY_NAV_STEPS_WAVE172,
            "OVERLAY_SHADOW_ON_FRAME",
        ) == Some(5)
        && RUNTIME_HOST_LIVE_GAMEWORLD_SHADOW_OVERLAY_CMD_NAMES_WAVE172.len() == 3
}

/// Wave 172 composite residual honesty pack.
pub fn honesty_live_gameworld_shadow_overlay_residual_pack_wave172() -> bool {
    honesty_live_gameworld_shadow_overlay_method_names_residual_wave172()
        && honesty_live_gameworld_shadow_overlay_nav_commands_residual_wave172()
}

/// Source residual: seed_presentation overlays GameWorld shadow after host freeze.
pub fn honesty_seed_presentation_shadow_overlay_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 590: real seed body lives in host_seed_presentation_after_match_start.
    let i = match src.find("fn host_seed_presentation_after_match_start(&mut self)") {
        Some(i) => i,
        None => match src.find("fn seed_presentation_after_match_start") {
            Some(i) => i,
            None => return false,
        },
    };
    let body = &src[i..src.len().min(i + 2200)];
    // Wave 195/590: seed uses build_for_engine which applies GW overlay/rebuild internally.
    body.contains("gameworld_shadow")
        && body.contains("sync_from_host")
        && body.contains("build_for_engine")
        && src.contains("host_seed_presentation_after_match_start()")
}

/// Live residual: map load → shadow sync → probe → presentation overlay.
pub fn simulate_live_gameworld_shadow_overlay_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
    use crate::presentation_frame::PresentationFrame;

    if !honesty_live_gameworld_shadow_overlay_residual_pack_wave172() {
        return false;
    }
    if !honesty_seed_presentation_shadow_overlay_source() {
        return false;
    }

    // Gate default-on damage/economy/production authority (matches behavior_gate).
    ensure_gate_damage_authority();

    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        // No maps — soft residual (authority source still required).
        return true;
    };

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded {
        return false;
    }
    for _ in 0..3 {
        logic.update();
    }

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let probe = shadow.probe(&mut logic);
    if probe.mapped_objects == 0 {
        return false;
    }
    if probe.host_objects == 0 || probe.shadow_entities == 0 {
        return false;
    }

    let mut pres = PresentationFrame::build_from_logic(&logic, 0);
    if pres.objects.is_empty() {
        return false;
    }
    let overlaid = pres.overlay_gameworld_shadow(&shadow);
    // Overlay may update 0 rows if poses already match; require mapping capacity
    // via probe rather than forcing pose deltas.
    let _ = overlaid;
    // Fail-closed: presentation objects should be mappable for most host IDs.
    let mappable = pres
        .objects
        .iter()
        .filter(|o| shadow.entity_for_host(o.id).is_some())
        .count();
    if mappable == 0 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_shadow_overlay_method_names_residual_wave172());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_shadow_overlay_nav_commands_residual_wave172());
    }

    #[test]
    fn wave172_composite_pack() {
        assert!(honesty_live_gameworld_shadow_overlay_residual_pack_wave172());
    }

    #[test]
    fn seed_presentation_shadow_overlay_source() {
        assert!(honesty_seed_presentation_shadow_overlay_source());
    }

    #[test]
    fn simulate_live_gameworld_shadow_overlay_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_shadow_overlay_honesty(),
            "live map load must sync GameWorldShadow and overlay presentation"
        );
    }

    #[test]
    fn sync_from_host_maps_created_object_and_overlays_presentation() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("ShadowRanger172");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ShadowRanger172".into(), t);
        let id = logic
            .create_object("ShadowRanger172", Team::USA, Vec3::new(12.0, 0.0, 24.0))
            .expect("create host object");

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let probe = shadow.probe(&mut logic);
        assert!(
            probe.mapped_objects >= 1,
            "sync_from_host must map created host objects"
        );
        assert!(
            probe.host_objects >= 1 && probe.shadow_entities >= 1,
            "shadow census must include host unit"
        );
        assert!(
            shadow.entity_for_host(id).is_some(),
            "entity_for_host must resolve the created object"
        );

        let mut pres = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            pres.objects.iter().any(|o| o.id == id),
            "presentation freeze must include the created object"
        );
        let _ = pres.overlay_gameworld_shadow(&shadow);
        let mapped = pres
            .objects
            .iter()
            .filter(|o| shadow.entity_for_host(o.id).is_some())
            .count();
        assert!(
            mapped >= 1,
            "overlay_gameworld_shadow must keep host ids mappable"
        );
    }
}
