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
    include_str!("../host_usa_pilot.rs"),
    include_str!("../host_ranger.rs"),
    include_str!("../world_combat/ocl_and_scud.rs"),
    include_str!("../world_combat/streams_and_rpg.rs"),
    include_str!("../world_combat/strategy_center.rs"),
    include_str!("../world_combat/infantry_weapons.rs"),
    include_str!("../world_combat/missile_defenders.rs"),
    include_str!("../world_combat/drones_and_garrison.rs"),
    include_str!("../world_combat/special_power_flights.rs"),
    include_str!("../world_combat/air_and_mig.rs"),
    include_str!("../world_combat/heroes_and_plans.rs"),
    include_str!("../resources.rs"),
    include_str!("../combat.rs"),
    include_str!("../world_tick/step.rs"),
    include_str!("../world_objects/destroy_list_bounty.rs"),
    include_str!("../world_scripts/ambush_leaflet.rs"),
    include_str!("../world_objects/object_queries.rs"),
    include_str!("../world_objects/create_destroy_die.rs"),
    include_str!("../world_objects/host_ops_writeback.rs"),
    include_str!("../world_objects/ai_authority.rs"),
    include_str!("../world_objects/support_states.rs"),
    include_str!("../world_objects/weapon_upgrades.rs"),
    include_str!("../world_objects/crates_radar_power.rs"),
    include_str!("../world_objects/ready_completions.rs"),
    include_str!("../world_objects/spawn_templates.rs"),
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
