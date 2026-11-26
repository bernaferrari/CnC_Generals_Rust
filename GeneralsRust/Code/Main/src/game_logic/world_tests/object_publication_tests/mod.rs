//! Object-owned simulation state and its immutable publication.

use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::{PresentationEvent, PresentationFrame, RenderableObject};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

mod combat;
mod containment;
mod lifecycle;
mod movement;
mod production;

fn publish(world: &mut GameLogic) -> PresentationFrame {
    let local = world.local_player_id().unwrap_or(0);
    PresentationFrame::try_build_from_logic(world, local).expect("publish receiving world")
}

fn actor(frame: &PresentationFrame, id: ObjectId) -> &RenderableObject {
    frame
        .objects
        .iter()
        .find(|row| row.id == id)
        .expect("published object")
}

fn insert_fixture_template(world: &mut GameLogic, name: &str, hp: f32) {
    let mut template = ThingTemplate::new(name);
    template.set_health(hp);
    template.add_kind_of(KindOf::Selectable);
    template.add_kind_of(KindOf::Attackable);
    world.templates.insert(name.into(), template);
}
