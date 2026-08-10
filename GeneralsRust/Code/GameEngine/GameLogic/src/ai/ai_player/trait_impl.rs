//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AiPlayerTrait for AIPlayer {
    fn update(&mut self) -> Result<(), AiError> {
        // C++ AIPlayer::update (AIPlayer.cpp): base → ready → queued → team →
        // upgrades → bridge. No strategy residual in C++.
        self.do_base_building()?;
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;

        Ok(())
    }

    fn update_economy(&mut self) -> Result<(), AiError> {
        self.analyze_economic_situation()?;

        // Queue supply trucks if needed
        if self.economic_state.supply_shortage {
            self.queue_supply_truck()?;
        }

        // Build economic structures
        if self.economic_state.economic_pressure > 0.7 {
            self.build_specific_ai_building("SupplyCenter")?;
        }

        Ok(())
    }

    fn update_construction(&mut self) -> Result<(), AiError> {
        self.process_base_building()?;
        self.update_construction_priorities()?;
        Ok(())
    }

    fn update_diplomacy(&mut self) -> Result<(), AiError> {
        // AI diplomacy hooks are limited in the current port; preserve no-op behavior.
        Ok(())
    }

    fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError> {
        self.build_specific_ai_building(building_name)
    }

    fn build_by_supplies(&mut self, minimum_cash: i32, building_name: &str) -> Result<(), AiError> {
        AIPlayer::build_by_supplies(self, minimum_cash, building_name)
    }

    fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        AIPlayer::build_upgrade(self, upgrade_name)
    }

    fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        AIPlayer::build_specific_building_near_location(self, building_name, location)
    }

    fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        AIPlayer::repair_structure(self, structure_id)
    }

    fn get_player_id(&self) -> u32 {
        self.player_id
    }

    fn get_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError> {
        self.build_ai_base_defense(flank)
    }

    fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError> {
        self.build_ai_base_defense_structure(structure_name, flank)
    }
}
