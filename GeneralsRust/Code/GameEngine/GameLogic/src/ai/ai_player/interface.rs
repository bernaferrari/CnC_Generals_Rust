//! Canonical Core AIPlayer dispatch used by its player and script integration.
//! The unused time-budget scheduler that formerly housed this trait is retired.
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID};
use crate::player::GameDifficulty;

pub trait AiPlayerTrait {
    fn update(&mut self) -> Result<(), AiError>;
    fn update_economy(&mut self) -> Result<(), AiError>;
    fn update_construction(&mut self) -> Result<(), AiError>;
    fn update_diplomacy(&mut self) -> Result<(), AiError>;
    fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError>;
    fn build_by_supplies(&mut self, minimum_cash: i32, building_name: &str) -> Result<(), AiError>;
    fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError>;
    fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError>;
    fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError>;
    fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError>;
    fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError>;
    fn get_player_id(&self) -> u32;
    fn get_difficulty(&self) -> GameDifficulty;
}
