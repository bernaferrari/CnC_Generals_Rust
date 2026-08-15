//! GameWorldShadow tests split by theme from the former `gameworld_shadow_tests.rs` monolith.
//!
//! Mechanical extract from gameworld_shadow.rs `mod tests`.
//! Child module via `#[path]` / `mod gameworld_shadow_tests`.
//! `include_str!` paths are relative to this directory (`../` → `src/`).

use super::*;
pub(super) use crate::game_logic::{KindOf, Team, ThingTemplate};
pub(super) use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub(super) use glam::Vec3;
use std::sync::{Mutex, OnceLock};

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

mod sync_ids;
mod combat_status;
mod sciences_upgrades;
mod host_log_combat;
mod authority_writeback;
mod economy_construction;
mod entity_channels;
mod continue_attack;
mod fire_damage;
mod sell_heal;
mod presentation;
