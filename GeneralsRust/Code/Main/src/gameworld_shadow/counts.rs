//! Shadow entity/player residual counts.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn frenzy_until_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.weapon_bonus_frenzy_until_frame > 0 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with battle-plan sight scalar residual != 1.0.
    pub fn battle_plan_sight_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| (e.battle_plan_sight_scalar_applied - 1.0).abs() > 0.001 && !e.destroyed)
            .count()
    }

    pub fn horde_bonus_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.weapon_bonus_horde && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host humvee transport residual.
    pub fn humvee_transport_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_humvee_transport && !e.destroyed)
            .count()
    }

    pub fn detector_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_detector && !e.destroyed)
            .count()
    }

    /// Count shadow entities with special power ready residual.
    pub fn special_power_ready_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.special_power_ready && !e.destroyed)
            .count()
    }

    pub fn battle_bus_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_battle_bus_transport && !e.destroyed)
            .count()
    }

    /// Count shadow entities currently contained (host contained_by residual).
    pub fn contained_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.contained_by_host != 0 && !e.destroyed)
            .count()
    }

    pub fn armed_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.has_weapon && !e.destroyed)
            .count()
    }

    /// Count shadow entities with non-empty host movement path residual.
    pub fn pathing_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.path_len > 0 && !e.destroyed)
            .count()
    }

    pub fn producing_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.production_queue_len > 0 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host building residual.
    pub fn building_data_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_building && !e.destroyed)
            .count()
    }

    pub fn elite_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.veterancy_ordinal >= 2 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host stealthed residual.
    pub fn stealthed_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.stealthed && !e.destroyed)
            .count()
    }

    pub fn force_attack_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.force_attack && !e.destroyed)
            .count()
    }

    /// Count shadow entities with non-idle host AI state residual.
    pub fn non_idle_ai_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.ai_state_ordinal != 0 && !e.destroyed)
            .count()
    }

    pub fn building_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.object_type_ordinal == 3 && !e.destroyed)
            .count()
    }

    /// Sum of host power_provided residual on shadow entities.
    pub fn total_entity_power_provided(&self) -> i32 {
        self.world
            .world()
            .entities()
            .filter(|e| !e.destroyed)
            .map(|e| e.power_provided)
            .sum()
    }

    pub fn moving_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.moving && !e.destroyed)
            .count()
    }

    pub fn attacking_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.attacking && !e.destroyed)
            .count()
    }

    pub fn entity_count_for_team_ordinal(&self, team_ordinal: u8) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.team_ordinal == team_ordinal && !e.destroyed)
            .count()
    }

    pub fn selected_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.selected && !e.destroyed)
            .count()
    }

    /// Count shadow entities still under construction residual.
    pub fn under_construction_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.construction_percent < 0.999 && !e.destroyed)
            .count()
    }

    pub fn alive_player_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .filter(|(_, p)| p.is_alive)
            .count()
    }

    /// Max cash bounty percent residual across shadow players.
    pub fn max_cash_bounty_percent(&self) -> f32 {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.cash_bounty_percent)
            .fold(0.0_f32, f32::max)
    }

    pub fn radar_residual_present(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.radar_count != 0 || p.radar_disabled)
    }

    /// C++ Player::hasRadar residual on any shadow player.
    pub fn any_player_has_radar(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.radar_count > 0 && !p.radar_disabled)
    }

    pub fn power_bar_residual_present(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.power_produced != 0 || p.power_consumed != 0)
    }

    pub fn unlocked_science_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.unlocked_sciences.len())
            .sum()
    }

    pub fn completed_upgrade_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.completed_upgrades.len())
            .sum()
    }
}
