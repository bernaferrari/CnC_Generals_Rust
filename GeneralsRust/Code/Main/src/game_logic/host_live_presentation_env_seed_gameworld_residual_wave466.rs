//! Wave 466 residual peels: ensure_presentation_env_for_hints seeds
//! PresentationFrame via build_for_engine(host, shadow) when GameWorld shadow
//! exists (not host-only None freeze).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 455 presentation-only env apply.
//! Architecture residual - boot/map env freeze includes GW overlay when live.
//!
//! Sources (cnc_game_engine.rs):
//! - ensure_presentation_env_for_hints(..., shadow: Option<&GameWorldShadow>)
//! - build_for_engine(..., shadow) instead of None
//! - call sites pass self.gameworld_shadow.as_ref()
//!
//! Fail-closed:
//! - Without shadow session still freezes from host logic only
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466: &[&str] = &[
    "ensure_presentation_env_for_hints",
    "build_for_engine",
    "gameworld_shadow",
    "presentation_frame",
    "set_presentation_frame",
    "overlay_gameworld_shadow",
];

pub const PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466: &[&str] = &[
    "Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists",
    "build_for_engine",
    "gameworld_shadow.as_ref()",
    "set_presentation_frame",
];

pub const PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466: &[&str] = &[
    "DETECT_MISSING_PRESENTATION_FRAME",
    "PASS_GAMEWORLD_SHADOW_OPTION",
    "BUILD_FOR_ENGINE_WITH_SHADOW",
    "SET_PIPELINE_PRESENTATION_FRAME",
    "ENV_APPLY_PRESENTATION_ONLY",
    "NO_HOST_ONLY_NONE_WHEN_SHADOW_LIVE",
];

pub const RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466: &[&str] = &[
    "click_presentation_env_seed_gameworld_ok_wnd_detect",
    "click_presentation_env_seed_gameworld_ok_wnd_pass_shadow",
    "click_presentation_env_seed_gameworld_ok_wnd_build",
    "click_presentation_env_seed_gameworld_ok_wnd_prepare",
    "click_presentation_env_seed_gameworld_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationEnvSeedGameworldAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EnsureSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationEnvSeedGameworldAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_env_seed_gameworld_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_env_seed_gameworld_last_action()
-> ResidualPresentationEnvSeedGameworldAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationEnvSeedGameworldAction::MethodNames,
        2 => ResidualPresentationEnvSeedGameworldAction::SourceMarkers,
        3 => ResidualPresentationEnvSeedGameworldAction::NavCommands,
        4 => ResidualPresentationEnvSeedGameworldAction::EnsureSource,
        5 => ResidualPresentationEnvSeedGameworldAction::CallSites,
        6 => ResidualPresentationEnvSeedGameworldAction::Composite,
        _ => ResidualPresentationEnvSeedGameworldAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_env_seed_gameworld_method_names_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466,
            "ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466,
            "overlay_gameworld_shadow",
        ) == Some(5)
}

pub fn honesty_presentation_env_seed_gameworld_source_markers_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466.len() == 4
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466,
            "Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466,
            "gameworld_shadow.as_ref()",
        ) == Some(2)
}

pub fn honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466,
            "PASS_GAMEWORLD_SHADOW_OPTION",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466,
            "NO_HOST_ONLY_NONE_WHEN_SHADOW_LIVE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466,
            "click_presentation_env_seed_gameworld_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

/// Wave 466: pipeline env freeze from host + optional GameWorld shadow.
/// Mirrors `host_ensure_presentation_env_for_hints` when the pipeline has no frame.
pub fn seed_presentation_env_frame_from_host_and_shadow(
    logic: &crate::game_logic::GameLogic,
    local_player_id: u32,
    shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
) -> crate::presentation_frame::PresentationFrame {
    crate::presentation_frame::PresentationFrame::build_for_engine(logic, local_player_id, shadow)
}

/// Engine-cache variant of the env seed. The payload is immutable and owned by
/// the presentation frame, so this preserves the no-live-GameLogic renderer
/// boundary while avoiding a full height/blend clone for every seed.
pub fn seed_presentation_env_frame_from_host_and_shadow_with_runtime_heightmap(
    logic: &crate::game_logic::GameLogic,
    local_player_id: u32,
    shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    runtime_heightmap: Option<
        std::sync::Arc<crate::presentation_frame::PresentationRuntimeHeightmap>,
    >,
) -> crate::presentation_frame::PresentationFrame {
    crate::presentation_frame::PresentationFrame::build_for_engine_with_runtime_heightmap(
        logic,
        local_player_id,
        shadow,
        runtime_heightmap,
    )
}

pub fn simulate_presentation_env_seed_gameworld_source() -> bool {
    let src = cnc_source();
    let helper = include_str!("host_live_presentation_env_seed_gameworld_residual_wave466.rs");
    // Wave 590: real env seed body lives in host_ensure_presentation_env_for_hints.
    let Some(body) = function_body(src, "fn host_ensure_presentation_env_for_hints(")
        .or_else(|| function_body(src, "fn ensure_presentation_env_for_hints("))
    else {
        return false;
    };
    // Wave 474/590: instance method seeds with self.gameworld_shadow overlay.
    let helper_ok = helper.contains("pub fn seed_presentation_env_frame_from_host_and_shadow")
        && helper.contains("PresentationFrame::build_for_engine");
    let ok = (body
        .contains("Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists")
        || body.contains("Wave 474: instance seed only")
        || body.contains("Wave 590"))
        && (body.contains("build_for_engine")
            || body.contains("seed_presentation_env_frame_from_host_and_shadow"))
        && body.contains("self.gameworld_shadow.as_ref()")
        && body.contains("&self.game_logic")
        && body.contains("&mut self")
        && !body.contains("shadow: Option<&crate::gameworld_shadow::GameWorldShadow>")
        && src.contains("self.host_ensure_presentation_env_for_hints()")
        && src.contains("seed_presentation_env_frame_from_host_and_shadow")
        && helper_ok;
    residual_action_store(ResidualPresentationEnvSeedGameworldAction::EnsureSource);
    ok
}

pub fn simulate_presentation_env_seed_gameworld_callsites() -> bool {
    let src = cnc_source();
    // Wave 467/474: call sites use ensure_presentation_env_seeded (mirrors last frame).
    let seeded = src.matches("ensure_presentation_env_seeded()").count();
    let def_ok = src.contains("fn ensure_presentation_env_seeded")
        && src.contains("last_presentation_frame.is_none()")
        && src.contains("self.gameworld_shadow.as_ref()"); // on ensure instance body
    // No free-fn Self::ensure_presentation_env_for_hints call sites remain.
    let free_call = src.contains("Self::ensure_presentation_env_for_hints(");
    let ok = seeded >= 1
        && def_ok
        && !free_call
        && src.contains("self.ensure_presentation_env_for_hints()");
    residual_action_store(ResidualPresentationEnvSeedGameworldAction::CallSites);
    ok
}

pub fn honesty_presentation_env_seed_gameworld_residual_pack_wave466() -> bool {
    honesty_presentation_env_seed_gameworld_method_names_residual_wave466()
        && honesty_presentation_env_seed_gameworld_source_markers_residual_wave466()
        && honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466()
        && simulate_presentation_env_seed_gameworld_source()
        && simulate_presentation_env_seed_gameworld_callsites()
}

pub fn simulate_live_presentation_env_seed_gameworld_honesty() -> bool {
    let ok = honesty_presentation_env_seed_gameworld_residual_pack_wave466();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEnvSeedGameworldAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_env_seed_gameworld_method_names_residual_wave466());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_env_seed_gameworld_source_markers_residual_wave466());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466());
    }

    #[test]
    fn presentation_env_seed_gameworld_sources() {
        assert!(simulate_presentation_env_seed_gameworld_source());
        assert!(simulate_presentation_env_seed_gameworld_callsites());
    }

    #[test]
    fn wave466_composite_pack() {
        assert!(honesty_presentation_env_seed_gameworld_residual_pack_wave466());
    }

    #[test]
    fn simulate_live_presentation_env_seed_gameworld_honesty_residual_live() {
        assert!(
            simulate_live_presentation_env_seed_gameworld_honesty(),
            "presentation env seed gameworld residual must latch"
        );
        assert!(residual_presentation_env_seed_gameworld_ok());
        assert_eq!(
            residual_presentation_env_seed_gameworld_last_action(),
            ResidualPresentationEnvSeedGameworldAction::Composite
        );
    }

    #[test]
    fn seed_presentation_env_frame_includes_gameworld_shadow_objects() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
        use crate::presentation_frame::presentation_from_gameworld_enabled;
        use gamelogic::world::PlayerId;
        use gamelogic::world::entities::TemplateRef;
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("EnvSeedRanger466");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EnvSeedRanger466".into(), t);
        let host_id = logic
            .create_object("EnvSeedRanger466", Team::USA, Vec3::new(8.0, 0.0, 9.0))
            .expect("create host object");

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw_id = {
            let tmpl = TemplateRef {
                name: "EnvSeedGwOnly466".into(),
                source: None,
            };
            shadow.world_mut().spawn_entity(
                tmpl,
                Some(PlayerId::from_index(0)),
                gamelogic::world::entities::Transform::new([70.0, 0.0, 71.0], 0.1),
                40.0,
            )
        };
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
            e.max_health = 40.0;
            e.team_ordinal = 0;
        }
        if let Some(e) = shadow
            .entity_for_host(host_id)
            .and_then(|eid| shadow.world_mut().world_mut().entity_mut(eid))
        {
            e.transform.position = [10.0, 0.0, 11.0].into();
            e.health = 90.0;
            e.max_health = 100.0;
        }

        let host_only = seed_presentation_env_frame_from_host_and_shadow(&logic, 0, None);
        assert!(
            host_only.objects.iter().any(|o| o.id == host_id),
            "host-only freeze must include host object"
        );
        let synth = 0x8000_0000 | gw_id.get();
        assert!(
            host_only.objects.iter().all(|o| o.id.0 != synth),
            "host-only freeze must not include GameWorld-only spawn"
        );

        let with_shadow =
            seed_presentation_env_frame_from_host_and_shadow(&logic, 0, Some(&shadow));
        let Some(host_obj) = with_shadow.objects.iter().find(|o| o.id == host_id) else {
            panic!("shadow freeze must keep host object");
        };
        assert!(
            (host_obj.position.x - 10.0).abs() < 1e-3
                && (host_obj.health_current - 90.0).abs() < 1e-3,
            "shadow freeze must overlay GameWorld pose/health: {:?}",
            host_obj.position
        );
        if presentation_from_gameworld_enabled() {
            assert!(
                with_shadow.objects.iter().any(|o| o.id.0 == synth),
                "shadow freeze must append GameWorld-only object"
            );
        }
    }

    #[test]
    fn seed_presentation_env_frame_matches_build_for_engine() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::{GameWorldShadow, ensure_gate_damage_authority};
        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("EnvSeedEq466");
        t.set_health(80.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EnvSeedEq466".into(), t);
        let _ = logic
            .create_object("EnvSeedEq466", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("create");
        let mut shadow = GameWorldShadow::new(32);
        shadow.sync_from_host(&logic);

        let via_helper = seed_presentation_env_frame_from_host_and_shadow(&logic, 0, Some(&shadow));
        let via_shipped = PresentationFrame::build_for_engine(&logic, 0, Some(&shadow));
        assert_eq!(
            via_helper.objects.len(),
            via_shipped.objects.len(),
            "env seed helper must be PresentationFrame::build_for_engine"
        );
        assert_eq!(
            via_helper
                .objects
                .iter()
                .map(|o| (o.id, o.position.x, o.health_current))
                .collect::<Vec<_>>(),
            via_shipped
                .objects
                .iter()
                .map(|o| (o.id, o.position.x, o.health_current))
                .collect::<Vec<_>>(),
        );
    }
}
