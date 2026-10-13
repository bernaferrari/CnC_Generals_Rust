//! Fire spread data passed directly to receiving Main callbacks.

use super::ObjectId;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct FireSpreadTickEvent {
    pub id: ObjectId,
    pub state: u8,
    pub aflame_end_frame: u32,
    pub burned_end_frame: u32,
    pub next_spread_frame: u32,
    pub became_burned: bool,
    pub aflame: bool,
    pub try_spread: bool,
    pub spawn_embers: bool,
    pub ignite_target: Option<ObjectId>,
    pub pos: Vec3,
    pub spread_try_range: f32,
    pub flame_damage_accum: f32,
}
