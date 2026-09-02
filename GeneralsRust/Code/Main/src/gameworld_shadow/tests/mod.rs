//! GameWorldShadow tests split by theme from `gameworld_shadow_tests.rs`.

use super::*;
pub(super) use crate::game_logic::{KindOf, Team, ThingTemplate};
pub(super) use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub(super) use glam::Vec3;

/// Live concatenated gameworld_shadow sources (tick/*.rs + writeback/apply modules).
pub(super) use super::GAMEWORLD_SHADOW_SRC;

/// Live concatenated object sources.
pub(super) use crate::game_logic::object::OBJECT_SRC as GAME_LOGIC_OBJECT_SRC;

mod harness;
pub(super) use harness::{AuthorityEnvGuard, GAME_LOGIC_HOST_SRC, last_rust_fn_body, rust_fn_body};

pub(super) fn authority_env_lock() -> std::sync::MutexGuard<'static, ()> {
    super::authority_env_lock()
}

pub(super) fn ensure_template(logic: &mut GameLogic, name: &str, hp: f32) {
    if logic.templates.contains_key(name) {
        return;
    }
    let mut t = ThingTemplate::new(name);
    t.set_health(hp);
    t.add_kind_of(KindOf::Selectable);
    t.add_kind_of(KindOf::Attackable);
    logic.templates.insert(name.into(), t);
}

mod authority_dry_run;
mod authority_writeback;
mod radar_coupled;
mod combat_status;
mod command_authority;
mod continue_attack;
mod deferred_destroy;
mod economy_construction;
mod entity_channels;
mod entity_modules;
mod factory_contain_commands;
mod fire_damage;
mod host_log_combat;
mod presentation;
mod sciences_upgrades;
mod sell_heal;
mod sync_ids;
mod weapon_movement_authority;
