//! GameWorldShadow tests split by theme from `gameworld_shadow_tests.rs`.

use super::*;
pub(super) use crate::game_logic::{KindOf, Team, ThingTemplate};
pub(super) use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub(super) use glam::Vec3;

/// Former monolithic `gameworld_shadow.rs` source, concatenated after the directory split.
pub(super) const GAMEWORLD_SHADOW_SRC: &str = concat!(
    include_str!("../mod.rs"),
    include_str!("../types.rs"),
    include_str!("../tick.rs"),
    include_str!("../construct.rs"),
    include_str!("../writeback_core.rs"),
    include_str!("../writeback_production.rs"),
    include_str!("../counts.rs"),
    include_str!("../apply_host_events.rs"),
    include_str!("../apply_host_combat.rs"),
    include_str!("../apply_host_weapon_set.rs"),
    include_str!("../apply_host_stealth.rs"),
    include_str!("../apply_host_misc.rs"),
    include_str!("../writeback_misc.rs"),
    include_str!("../writeback_combat_status.rs"),
    include_str!("../apply_host_damage.rs"),
    include_str!("../session.rs"),
    include_str!("../presentation.rs"),
    include_str!("../couple_guard.rs"),
);

/// Former monolithic `game_logic/object.rs` after the object/ directory split.
pub(super) const GAME_LOGIC_OBJECT_SRC: &str = concat!(
    include_str!("../../game_logic/object/mod.rs"),
    include_str!("../../game_logic/object/attack.rs"),
    include_str!("../../game_logic/object/bonuses.rs"),
    include_str!("../../game_logic/object/construct.rs"),
    include_str!("../../game_logic/object/damage.rs"),
    include_str!("../../game_logic/object/death.rs"),
    include_str!("../../game_logic/object/install.rs"),
    include_str!("../../game_logic/object/jets.rs"),
    include_str!("../../game_logic/object/orders.rs"),
    include_str!("../../game_logic/object/physics.rs"),
    include_str!("../../game_logic/object/physics_motion.rs"),
    include_str!("../../game_logic/object/pose.rs"),
    include_str!("../../game_logic/object/record.rs"),
    include_str!("../../game_logic/object/rtb.rs"),
    include_str!("../../game_logic/object/status_bits.rs"),
    include_str!("../../game_logic/object/stealth.rs"),
    include_str!("../../game_logic/object/update.rs"),
    include_str!("../../game_logic/object/visual.rs"),
    include_str!("../../game_logic/object/weapons.rs"),
);

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
