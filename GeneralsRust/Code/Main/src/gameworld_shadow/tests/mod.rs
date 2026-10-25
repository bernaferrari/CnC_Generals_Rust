//! GameWorldShadow tests split by theme from `gameworld_shadow_tests.rs`.

use super::*;
pub(super) use crate::game_logic::{KindOf, Team, ThingTemplate};
pub(super) use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub(super) use glam::Vec3;

mod harness;
pub(super) use harness::{perform_owner_damage, perform_owner_health};

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
mod combat_status;
mod command_authority;
mod continue_attack;
mod deferred_destroy;
mod economy_construction;
mod entity_channels;
mod factory_contain_commands;
mod fire_damage;
mod host_log_combat;
mod presentation;
mod radar_coupled;
mod sciences_upgrades;
mod sell_heal;
mod sync_ids;
mod weapon_movement_authority;

mod upgrade_completion_owner;

mod ai_decision_owner;
mod health_event_owner;
mod health_owner_order;

mod direct_destroy_owner;

mod movement_event_owner;

/// Exercise the shipped observation boundary before inspecting its projection.
/// This fixture performs no gameplay tick, physical cleanup or mutation replay.
/// Tests offering an end-of-frame cleanup must call the checked owner explicitly.
pub(super) fn observe_completed_owner(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> GameWorldShadowProbe {
    super::run_owned_host_boundary(Some(shadow), logic);
    shadow.probe(logic)
}
