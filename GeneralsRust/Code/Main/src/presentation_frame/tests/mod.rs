//! Honesty / residual tests for the presentation snapshot.
//!
//! Split from the former monolithic `presentation_frame.rs` `mod tests`.

pub use super::*;
pub use crate::game_logic::{GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate};
pub use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
pub use glam::Vec3;

mod apply_honesty;
mod dual_tick_registry;
mod fow_own_team;
mod freeze_queries;
mod render_overlay;
mod runtime_heightmap;
mod shared_contract;
mod weapon_readiness;

mod shroud_ownership;

/// Admit the same real Main objects used by presentation health-event controls.
fn admit_health_fixture_objects(logic: &mut GameLogic, count: u32) -> ObjectId {
    let mut template = ThingTemplate::new("PresentationHealthFixture");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("PresentationHealthFixture".into(), template);
    let mut target = ObjectId(0);
    for _ in 0..count {
        target = logic
            .create_object("PresentationHealthFixture", Team::USA, Vec3::ZERO)
            .expect("health fixture object");
    }
    assert_eq!(target.0, count);
    target
}
