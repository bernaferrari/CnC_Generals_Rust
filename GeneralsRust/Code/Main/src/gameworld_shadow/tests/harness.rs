//! Shared isolation + source-scan helpers for gameworld_shadow tests.
//!
//! Host GameLogic was split out of `game_logic.rs` into `world_*` modules.
//! Source-honesty tests must scan the live split, not the leftover facade.

use super::authority_env_lock;
use crate::gameworld_shadow::{
    CoupledTickGuard, GAMEWORLD_AUTHORITY_ENV_NAMES, refresh_gameworld_authority_env_caches,
};

/// Concatenated host GameLogic sources after the world_* split and the
/// game_logic/ directory split (2026-08-15).
pub const GAME_LOGIC_HOST_SRC: &str = concat!(
    include_str!("../../game_logic/game_logic/crate_tick.rs"),
    include_str!("../../game_logic/game_logic/player.rs"),
    include_str!("../../game_logic/game_logic/host.rs"),
    include_str!("../../game_logic/game_logic/script_camera.rs"),
    include_str!("../../game_logic/game_logic/authority.rs"),
    include_str!("../../game_logic/game_logic/construct.rs"),
    include_str!("../../game_logic/game_logic/mod.rs"),
    include_str!("../../game_logic/buildings.rs"),
    include_str!("../../game_logic/world_save.rs"),
    include_str!("../../game_logic/world_save/world_subsystems.rs"),
    include_str!("../../game_logic/world_save/world_paths.rs"),
    include_str!("../../game_logic/world_save/world_runtime.rs"),
    include_str!("../../game_logic/world_save/world_players.rs"),
    include_str!("../../game_logic/world_save/world_load.rs"),
    include_str!("../../game_logic/world_save/world_tests.rs"),
    include_str!("../../game_logic/host_usa_pilot.rs"),
    include_str!("../../game_logic/host_ranger.rs"),
    include_str!("../../game_logic/world_combat/ocl_and_scud.rs"),
    include_str!("../../game_logic/world_combat/streams_and_rpg.rs"),
    include_str!("../../game_logic/world_combat/strategy_center.rs"),
    include_str!("../../game_logic/world_combat/infantry_weapons.rs"),
    include_str!("../../game_logic/world_combat/missile_defenders.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/mod.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/firepoints.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/neutron.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/transport.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/garrison.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/support_residuals.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/defector.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/production_and_power.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/special_powers.rs"),
    include_str!("../../game_logic/world_combat/drones_and_garrison/tests.rs"),
    include_str!("../../game_logic/world_combat/special_power_flights.rs"),
    include_str!("../../game_logic/world_combat/air_and_mig.rs"),
    include_str!("../../game_logic/world_combat/heroes_and_plans.rs"),
    include_str!("../../game_logic/resources.rs"),
    include_str!("../../game_logic/combat/mod.rs"),
    include_str!("../../game_logic/combat/damage.rs"),
    include_str!("../../game_logic/combat/projectile.rs"),
    include_str!("../../game_logic/combat/weapon_fire.rs"),
    include_str!("../../game_logic/combat/resolution.rs"),
    include_str!("../../game_logic/combat/tests.rs"),
    include_str!("../../game_logic/world_tick/step.rs"),
    include_str!("../../game_logic/world_objects/destroy_list_bounty.rs"),
    include_str!("../../game_logic/world_scripts/ambush_leaflet.rs"),
    include_str!("../../game_logic/world_objects/object_queries.rs"),
    include_str!("../../game_logic/world_objects/create_destroy_die.rs"),
    include_str!("../../game_logic/world_objects/host_ops_writeback.rs"),
    include_str!("../../game_logic/world_objects/ai_authority.rs"),
    include_str!("../../game_logic/world_objects/support_states/mod.rs"),
    include_str!("../../game_logic/world_objects/support_states/contain_states.rs"),
    include_str!("../../game_logic/world_objects/support_states/guard_states.rs"),
    include_str!("../../game_logic/world_objects/support_states/heal_contain_tunnel.rs"),
    include_str!("../../game_logic/world_objects/support_states/special_abilities.rs"),
    include_str!("../../game_logic/world_objects/support_states/supply_repair_docks.rs"),
    include_str!("../../game_logic/world_objects/support_states/update.rs"),
    include_str!("../../game_logic/world_objects/weapon_upgrades.rs"),
    include_str!("../../game_logic/world_objects/crates_radar_power.rs"),
    include_str!("../../game_logic/world_tick/production.rs"),
    include_str!("../../game_logic/world_tick/physics.rs"),
    include_str!("../../game_logic/world_tick/airfield.rs"),
    include_str!("../../game_logic/world_tick/combat.rs"),
    include_str!("../../game_logic/world_tick/teams.rs"),
    include_str!("../../game_logic/world_tick/crates.rs"),
    include_str!("../../game_logic/world_tick/attack.rs"),
    include_str!("../../game_logic/world_tick/shock.rs"),
    include_str!("../../game_logic/world_tick/mood.rs"),
    include_str!("../../game_logic/world_tick/ai.rs"),
    include_str!("../../game_logic/world_scripts/angry_mob_aurora.rs"),
    include_str!("../../game_logic/world_scripts/saboteur_car_bomb.rs"),
    include_str!("../../game_logic/world_scripts/helix_radar.rs"),
    include_str!("../../game_logic/world_scripts/production_eva.rs"),
    include_str!("../../game_logic/world_scripts/rebuild_dozer.rs"),
    include_str!("../../game_logic/world_scripts/add_object_selection.rs"),
    include_str!("../../game_logic/world_scripts/special_power_strikes.rs"),
    include_str!("../../game_logic/world_scripts/stealth_mines.rs"),
    include_str!("../../game_logic/object/update.rs"),
    include_str!("../../game_logic/object/attack.rs"),
);

const EXTRA_ENV_KEYS: &[&str] = &["GENERALS_GAMEWORLD_DEFERRED_DESTROY"];

/// Serializes GENERALS_GAMEWORLD_* mutation and restores every key on drop.
pub struct AuthorityEnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _couple: Option<CoupledTickGuard>,
    saved: Vec<(String, Option<String>)>,
}

impl AuthorityEnvGuard {
    pub fn lock() -> Self {
        let lock = authority_env_lock();
        let mut keys: Vec<&str> = GAMEWORLD_AUTHORITY_ENV_NAMES.to_vec();
        keys.extend_from_slice(EXTRA_ENV_KEYS);
        let saved = keys
            .into_iter()
            .map(|k| (k.to_string(), std::env::var(k).ok()))
            .collect();
        Self {
            _lock: lock,
            _couple: None,
            saved,
        }
    }

    pub fn set(self, key: &str, value: &str) -> Self {
        crate::env_compat::set_var(key, value);
        refresh_gameworld_authority_env_caches();
        self
    }

    pub fn couple(mut self) -> Self {
        if self._couple.is_none() {
            self._couple = Some(CoupledTickGuard::enter());
        }
        self
    }
}

impl Drop for AuthorityEnvGuard {
    fn drop(&mut self) {
        for (key, prev) in self.saved.drain(..) {
            match prev {
                Some(v) => crate::env_compat::set_var(&key, v),
                None => crate::env_compat::remove_var(&key),
            }
        }
        refresh_gameworld_authority_env_caches();
    }
}

/// Brace-matched Rust function body starting at `fn name` / `pub fn name`.
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
