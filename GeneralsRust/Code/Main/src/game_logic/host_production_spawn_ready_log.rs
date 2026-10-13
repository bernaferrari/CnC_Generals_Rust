//! Typed production completion carried by the receiving Main owner.
use crate::game_logic::ObjectId;

#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionSpawnReadyEvent {
    pub unit: ObjectId,
    pub producer: ObjectId,
    pub template: String,
    pub spawn_pos: [f32; 3],
    pub rally: Option<[f32; 3]>,
}
