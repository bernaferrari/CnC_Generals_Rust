//! Shared source-scan helpers for residual honesty packs.
//!
//! Host GameLogic was split out of `game_logic.rs` into `world_*` modules.
//! Source-honesty tests must scan the live split, not the leftover facade.
//! Mirrors `gameworld_shadow/tests/harness.rs` `GAME_LOGIC_HOST_SRC` (2026-08-15).

/// Concatenated host GameLogic sources after the world_* split.
pub const GAME_LOGIC_HOST_SRC: &str = concat!(
    include_str!("../game_logic/crate_tick.rs"),
    include_str!("../game_logic/player.rs"),
    include_str!("../game_logic/host.rs"),
    include_str!("../game_logic/script_camera.rs"),
    include_str!("../game_logic/authority.rs"),
    include_str!("../game_logic/construct.rs"),
    include_str!("../game_logic/mod.rs"),
    include_str!("../buildings.rs"),
    include_str!("../world_save.rs"),
    include_str!("../world_save/world_subsystems.rs"),
    include_str!("../world_save/world_paths.rs"),
    include_str!("../world_save/world_runtime.rs"),
    include_str!("../world_save/world_players.rs"),
    include_str!("../world_save/world_load.rs"),
    include_str!("../world_save/world_tests.rs"),
    include_str!("../host_usa_pilot.rs"),
    include_str!("../host_ranger.rs"),
    include_str!("../world_combat/ocl_and_scud.rs"),
    include_str!("../world_combat/streams_and_rpg.rs"),
    include_str!("../world_combat/strategy_center.rs"),
    include_str!("../world_combat/infantry_weapons.rs"),
    include_str!("../world_combat/missile_defenders.rs"),
    include_str!("../world_combat/drones_and_garrison/mod.rs"),
    include_str!("../world_combat/drones_and_garrison/firepoints.rs"),
    include_str!("../world_combat/drones_and_garrison/neutron.rs"),
    include_str!("../world_combat/drones_and_garrison/transport.rs"),
    include_str!("../world_combat/drones_and_garrison/garrison.rs"),
    include_str!("../world_combat/drones_and_garrison/support_residuals.rs"),
    include_str!("../world_combat/drones_and_garrison/defector.rs"),
    include_str!("../world_combat/drones_and_garrison/production_and_power.rs"),
    include_str!("../world_combat/drones_and_garrison/special_powers.rs"),
    include_str!("../world_combat/drones_and_garrison/tests.rs"),
    include_str!("../world_combat/special_power_flights.rs"),
    include_str!("../world_combat/air_and_mig.rs"),
    include_str!("../world_combat/heroes_and_plans.rs"),
    include_str!("../resources.rs"),
    include_str!("../combat/mod.rs"),
    include_str!("../combat/damage.rs"),
    include_str!("../combat/projectile.rs"),
    include_str!("../combat/weapon_fire.rs"),
    include_str!("../combat/resolution.rs"),
    include_str!("../combat/tests.rs"),
    include_str!("../world_tick/step.rs"),
    include_str!("../world_objects/destroy_list_bounty.rs"),
    include_str!("../world_scripts/ambush_leaflet.rs"),
    include_str!("../world_objects/object_queries.rs"),
    include_str!("../world_objects/create_destroy_die.rs"),
    include_str!("../world_objects/host_ops_writeback.rs"),
    include_str!("../world_objects/ai_authority.rs"),
    include_str!("../world_objects/support_states/mod.rs"),
    include_str!("../world_objects/support_states/contain_states.rs"),
    include_str!("../world_objects/support_states/guard_states.rs"),
    include_str!("../world_objects/support_states/heal_contain_tunnel.rs"),
    include_str!("../world_objects/support_states/special_abilities.rs"),
    include_str!("../world_objects/support_states/supply_repair_docks.rs"),
    include_str!("../world_objects/support_states/update.rs"),
    include_str!("../world_objects/weapon_upgrades.rs"),
    include_str!("../world_objects/crates_radar_power.rs"),
    include_str!("../world_objects/ready_completions.rs"),
    include_str!("../world_objects/spawn_templates/mod.rs"),
    include_str!("../world_objects/spawn_templates/definition.rs"),
    include_str!("../world_objects/spawn_templates/metadata.rs"),
    include_str!("../world_objects/spawn_templates/seeding.rs"),
    include_str!("../world_objects/spawn_templates/vision.rs"),
    include_str!("../world_objects/spawn_templates/setup.rs"),
    include_str!("../world_objects/spawn_templates/tests.rs"),
    include_str!("../world_objects/resources_income.rs"),
    include_str!("../world_objects/object_ai_combat.rs"),
    include_str!("../world_tick/production.rs"),
    include_str!("../world_tick/physics.rs"),
    include_str!("../world_tick/airfield.rs"),
    include_str!("../world_tick/combat.rs"),
    include_str!("../world_tick/combat_fire_fx.rs"),
    include_str!("../world_tick/teams.rs"),
    include_str!("../world_tick/crates.rs"),
    include_str!("../world_tick/attack.rs"),
    include_str!("../world_tick/shock.rs"),
    include_str!("../world_tick/mood.rs"),
    include_str!("../world_tick/ai.rs"),
    include_str!("../world_tick/movement.rs"),
    include_str!("../world_tick/presence.rs"),
    include_str!("../world_scripts/angry_mob_aurora.rs"),
    include_str!("../world_scripts/saboteur_car_bomb.rs"),
    include_str!("../world_scripts/helix_radar.rs"),
    include_str!("../world_scripts/production_eva.rs"),
    include_str!("../world_scripts/rebuild_dozer.rs"),
    include_str!("../world_scripts/add_object_selection.rs"),
    include_str!("../world_scripts/special_power_strikes.rs"),
    include_str!("../world_scripts/stealth_mines.rs"),
    include_str!("../object/update.rs"),
    include_str!("../object/attack.rs"),
);

/// Live unit-command split (2026-08-15). Kept out of `GAME_LOGIC_HOST_SRC`
/// so exact `.matches().count()` honesty packs do not shift.
pub const GAME_LOGIC_UNIT_COMMANDS_SRC: &str = include_str!("../world_scripts/unit_commands.rs");

/// Object order / construct splits scanned by command-authority residuals.
pub const GAME_LOGIC_OBJECT_ORDERS_SRC: &str = include_str!("../object/orders.rs");
pub const GAME_LOGIC_OBJECT_CONSTRUCT_SRC: &str = include_str!("../object/construct.rs");

/// Extra host splits kept out of `GAME_LOGIC_HOST_SRC` so exact
/// `.matches().count()` honesty packs do not shift (2026-08-15).
pub const GAME_LOGIC_EVA_CAMERA_SRC: &str = include_str!("../world_scripts/eva_camera.rs");
pub const GAME_LOGIC_UI_PRODUCTION_SRC: &str = include_str!("../world_scripts/ui_production.rs");
pub const GAME_LOGIC_SCRIPTS_CAMERA_SRC: &str = concat!(
    include_str!("../world_scripts/scripts_camera/mod.rs"),
    include_str!("../world_scripts/scripts_camera/script_state.rs"),
    include_str!("../world_scripts/scripts_camera/script_unit_actions.rs"),
    include_str!("../world_scripts/scripts_camera/script_team_actions.rs"),
    include_str!("../world_scripts/scripts_camera/script_runtime_camera.rs"),
);
pub const GAME_LOGIC_OBJECT_WEAPONS_SRC: &str = include_str!("../object/weapons.rs");
pub const GAME_LOGIC_TANKS_SRC: &str = include_str!("../world_combat/tanks_and_upgrades.rs");

/// Host GameLogic plus the extra splits. Use this when looking up symbols
/// that moved out of the original god-file; do not use for exact counts.
pub fn host_logic_scan_src() -> &'static str {
    static SRC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        format!(
            "{}{}{}{}{}{}{}{}{}",
            GAME_LOGIC_HOST_SRC,
            GAME_LOGIC_UNIT_COMMANDS_SRC,
            GAME_LOGIC_OBJECT_ORDERS_SRC,
            GAME_LOGIC_OBJECT_CONSTRUCT_SRC,
            GAME_LOGIC_EVA_CAMERA_SRC,
            GAME_LOGIC_UI_PRODUCTION_SRC,
            GAME_LOGIC_SCRIPTS_CAMERA_SRC,
            GAME_LOGIC_OBJECT_WEAPONS_SRC,
            GAME_LOGIC_TANKS_SRC,
        )
    })
    .as_str()
}

/// Engine plus presentation_frame. Presentation honesty packs that used to
/// scan the god-file `cnc_game_engine.rs` must look here after the split.
pub fn engine_scan_src() -> &'static str {
    static SRC: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SRC.get_or_init(|| {
        format!(
            "{}{}",
            crate::cnc_game_engine::ENGINE_SRC,
            crate::presentation_frame::PRESENTATION_FRAME_SRC,
        )
    })
    .as_str()
}

/// Weapon crate after the god-file split (2026-08-15). Dual-world residuals
/// must scan these seams, not the leftover `weapon/mod.rs` facade.
pub const WEAPON_SRC: &str = concat!(
    include_str!("../../../../GameEngine/GameLogic/src/weapon/mod.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/helpers.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon_instance.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon_instance_combat.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/damage_application.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/weapon/template.rs"),
);

/// Object crate split scanned by Wave 264 dual-world residuals (2026-08-15).
pub const OBJECT_SPLIT_SRC: &str = concat!(
    include_str!("../../../../GameEngine/GameLogic/src/object/mod.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/object/object_combat.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/object/object_queries.rs"),
    include_str!("../../../../GameEngine/GameLogic/src/object/object_vision.rs"),
);

/// Brace-matched Rust function body for the last `fn name` in `src`.
pub fn last_rust_fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut last = None;
    let mut from = 0usize;
    let needle = format!("fn {name}");
    while let Some(rel) = src[from..].find(&needle) {
        let at = from + rel;
        let after = at + needle.len();
        let next = src.as_bytes().get(after).copied().unwrap_or(b'(');
        if next.is_ascii_alphanumeric() || next == b'_' {
            from = after;
            continue;
        }
        if let Some(body) = rust_fn_body(&src[at..], name) {
            last = Some(&src[at..at + body.len()]);
        }
        from = at + needle.len();
    }
    last
}

/// Brace-matched Rust function body starting at the first `fn name` in `src`.
pub fn rust_fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let needles = [
        format!("fn {name}"),
        format!("pub fn {name}"),
        format!("pub(super) fn {name}"),
        format!("pub(crate) fn {name}"),
    ];
    let start = needles.iter().filter_map(|n| src.find(n.as_str())).min()?;
    let brace = src[start..].find('{')? + start;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut i = brace;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..=i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
