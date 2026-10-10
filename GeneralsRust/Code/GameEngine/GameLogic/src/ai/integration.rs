//! AI Integration Module
//!
//! This module provides the integration layer between the enhanced AI systems
//! and the existing GameLogic framework, ensuring seamless interoperation
//! between modern Rust AI components and legacy systems.
//!
//! Author: Created by Claude for AI system integration

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use crate::common::KindOf;
use crate::common::ObjectID;
use crate::common::Snapshot;
use crate::common::types::{Coord3D, Real};
use crate::common::xfer::Xfer;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{GameDifficulty, player_list};
use crate::system::game_logic::get_game_logic;
use crate::terrain::get_terrain_logic;

use super::ai_player::{AIPlayer, AiPlayerTrait};
use super::skirmish_player::AISkirmishPlayer;
use super::{AiError, the_ai};

/// Wave 285: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub enum IntegratedAiPlayer {
    Standard(AIPlayer),
    Skirmish(AISkirmishPlayer),
}

impl IntegratedAiPlayer {
    pub fn update(&mut self) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.update(),
            IntegratedAiPlayer::Skirmish(player) => {
                player.update();
                Ok(())
            }
        }
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.set_ai_difficulty(difficulty),
            IntegratedAiPlayer::Skirmish(player) => player.set_ai_difficulty(difficulty),
        }
    }

    pub fn select_skillset(&mut self, skillset: i32) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.select_skillset(skillset),
            IntegratedAiPlayer::Skirmish(player) => player.select_skillset(skillset),
        }
    }

    pub fn set_team_delay_seconds(&mut self, delay_seconds: f32) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.set_team_delay_seconds(delay_seconds),
            IntegratedAiPlayer::Skirmish(player) => player.set_team_delay_seconds(delay_seconds),
        }
    }

    pub fn build_base_defense(&mut self, flank: bool) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.build_ai_base_defense(flank),
            IntegratedAiPlayer::Skirmish(player) => player.build_base_defense(flank),
        }
    }

    pub fn build_base_defense_structure(
        &mut self,
        structure_name: &str,
        flank: bool,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_ai_base_defense_structure(structure_name, flank)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_base_defense_structure(structure_name, flank)
            }
        }
    }

    pub fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_ai_building(building_name)
            }
            IntegratedAiPlayer::Skirmish(player) => player.build_specific_building(building_name),
        }
    }

    pub fn build_specific_ai_team(
        &mut self,
        team_name: &str,
        priority_build: bool,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_ai_team(team_name, priority_build)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_ai_team_by_name(team_name, priority_build);
                Ok(())
            }
        }
    }

    pub fn recruit_specific_ai_team(
        &mut self,
        team_name: &str,
        recruit_radius: Real,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.recruit_specific_ai_team(team_name, recruit_radius)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.recruit_specific_ai_team_by_name(team_name, recruit_radius);
                Ok(())
            }
        }
    }

    pub fn build_by_supplies(
        &mut self,
        minimum_cash: i32,
        building_name: &str,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_by_supplies(minimum_cash, building_name)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_by_supplies(minimum_cash, building_name)
            }
        }
    }

    pub fn build_upgrade(&mut self, upgrade_name: &str) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.build_upgrade(upgrade_name),
            IntegratedAiPlayer::Skirmish(player) => player.build_upgrade(upgrade_name),
        }
    }

    pub fn build_specific_building_near_location(
        &mut self,
        building_name: &str,
        location: Coord3D,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_building_near_location(building_name, location)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_building_near_location(building_name, location)
            }
        }
    }

    pub fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => player.repair_structure(structure_id),
            IntegratedAiPlayer::Skirmish(player) => player.repair_structure(structure_id),
        }
    }

    /// C++ Player::checkBridges → AISkirmishPlayer::checkBridges (solo AI returns false).
    pub fn check_bridges(
        &mut self,
        unit: &Arc<RwLock<crate::object::Object>>,
        start_waypoint_id: crate::common::WaypointID,
    ) -> bool {
        match self {
            IntegratedAiPlayer::Standard(_) => false,
            IntegratedAiPlayer::Skirmish(player) => player.check_bridges(unit, start_waypoint_id),
        }
    }

    pub fn on_structure_produced(
        &mut self,
        factory_id: ObjectID,
        structure_id: ObjectID,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.on_structure_produced(factory_id, structure_id)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.on_structure_produced(factory_id, structure_id)
            }
        }
    }

    pub fn is_supply_source_safe(&self, min_supplies: i32) -> bool {
        match self {
            IntegratedAiPlayer::Standard(player) => player.is_supply_source_safe(min_supplies),
            IntegratedAiPlayer::Skirmish(player) => player.is_supply_source_safe(min_supplies),
        }
    }

    pub fn is_supply_source_attacked(&mut self) -> bool {
        match self {
            IntegratedAiPlayer::Standard(player) => player.is_supply_source_attacked(),
            IntegratedAiPlayer::Skirmish(player) => player.is_supply_source_attacked(),
        }
    }

    /// C++ `Player::guardSupplyCenter` → `AIPlayer::guardSupplyCenter`.
    pub fn guard_supply_center(
        &mut self,
        team_name: &str,
        min_supplies: i32,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.guard_supply_center(team_name, min_supplies)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.guard_supply_center(team_name, min_supplies)
            }
        }
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.xfer(xfer),
            IntegratedAiPlayer::Skirmish(player) => player.xfer(xfer),
        }
    }

    pub fn load_post_process(&mut self) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.load_post_process(),
            IntegratedAiPlayer::Skirmish(player) => player.load_post_process(),
        }
    }

    pub fn new_map(&mut self) {
        match self {
            IntegratedAiPlayer::Standard(player) => player.new_map(),
            IntegratedAiPlayer::Skirmish(player) => player.new_map(),
        }
    }

    pub fn is_skirmish_ai(&self) -> bool {
        matches!(self, IntegratedAiPlayer::Skirmish(_))
    }

    pub fn get_ai_difficulty(&self) -> GameDifficulty {
        match self {
            IntegratedAiPlayer::Standard(player) => player.get_ai_difficulty(),
            IntegratedAiPlayer::Skirmish(player) => player.get_ai_difficulty(),
        }
    }

    pub fn get_ai_enemy_index(&mut self) -> Option<i32> {
        match self {
            IntegratedAiPlayer::Standard(_) => None,
            IntegratedAiPlayer::Skirmish(player) => player
                .get_ai_enemy()
                .and_then(|arc| arc.read().ok().map(|guard| guard.get_player_index() as i32)),
        }
    }

    pub fn build_specific_building_nearest_team(
        &mut self,
        thing_name: &str,
        team_id: i32,
    ) -> Result<(), AiError> {
        let team_name = team_id.to_string();
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_building_nearest_team(thing_name, &team_name)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_building_nearest_team(thing_name, &team_name)
            }
        }
    }

    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam` by team instance name.
    pub fn build_specific_building_nearest_team_by_name(
        &mut self,
        thing_name: &str,
        team_name: &str,
    ) -> Result<(), AiError> {
        match self {
            IntegratedAiPlayer::Standard(player) => {
                player.build_specific_building_nearest_team(thing_name, team_name)
            }
            IntegratedAiPlayer::Skirmish(player) => {
                player.build_specific_building_nearest_team(thing_name, team_name)
            }
        }
    }

    pub fn calc_closest_construction_zone(&self, template_name: &str) -> Option<Coord3D> {
        match self {
            IntegratedAiPlayer::Standard(player) => player
                .calc_closest_construction_zone_near_base(template_name)
                .ok()
                .flatten(),
            IntegratedAiPlayer::Skirmish(player) => player
                .calc_closest_construction_zone_near_base(template_name)
                .ok()
                .flatten(),
        }
    }

    /// C++ `Player::calcClosestConstructionZoneLocation(template, &location)`.
    pub fn calc_closest_construction_zone_at(
        &self,
        template_name: &str,
        location: &Coord3D,
    ) -> Option<Coord3D> {
        match self {
            IntegratedAiPlayer::Standard(player) => player
                .calc_closest_construction_zone_location(template_name, location)
                .ok()
                .flatten(),
            IntegratedAiPlayer::Skirmish(player) => player
                .calc_closest_construction_zone_location(template_name, location)
                .ok()
                .flatten(),
        }
    }
}

/// Per-player AI controllers (C++ `Player::m_ai`) plus the Common
/// `AIPlayerInterface` bridges handed to `Player::set_ai`.
pub struct AiIntegrationManager {
    /// AI players by player ID (C++-faithful player controllers)
    ai_players: BTreeMap<u32, IntegratedAiPlayer>,
    /// Common `AIPlayerInterface` handles kept alive for Player::set_ai.
    player_interfaces: HashMap<u32, Arc<AiPlayerBridge>>,
}

impl AiIntegrationManager {
    pub fn new() -> Self {
        Self {
            ai_players: BTreeMap::new(),
            player_interfaces: HashMap::new(),
        }
    }

    /// Drop every AI player and bridge.
    pub fn initialize(&mut self) -> Result<(), AiError> {
        self.ai_players.clear();
        self.player_interfaces.clear();
        log::info!("AI Integration Manager initialized");
        Ok(())
    }

    /// Create enhanced AI player for given player
    pub fn create_ai_player(&mut self, player_id: u32) -> Result<(), AiError> {
        player_list()
            .read()
            .ok()
            .and_then(|list| {
                list.get_player(player_id as crate::player::PlayerIndex)
                    .cloned()
            })
            .ok_or(AiError::InvalidObject)?;

        let is_skirmish = get_game_logic()
            .lock()
            .map(|logic| logic.is_in_skirmish_game())
            .unwrap_or(false);

        let ai_player = if is_skirmish {
            IntegratedAiPlayer::Skirmish(AISkirmishPlayer::new(player_id))
        } else {
            IntegratedAiPlayer::Standard(AIPlayer::new(player_id))
        };
        self.ai_players.insert(player_id, ai_player);
        self.attach_player_interface(player_id);

        log::info!("Created AI player for player {}", player_id);
        Ok(())
    }

    fn insert_ai_player(&mut self, player_id: u32, is_skirmish: bool) {
        let ai_player = if is_skirmish {
            IntegratedAiPlayer::Skirmish(AISkirmishPlayer::new(player_id))
        } else {
            IntegratedAiPlayer::Standard(AIPlayer::new(player_id))
        };
        self.ai_players.insert(player_id, ai_player);
        self.attach_player_interface(player_id);
    }

    pub fn ensure_ai_player(&mut self, player_id: u32, is_skirmish: bool) {
        if !self.ai_players.contains_key(&player_id) {
            self.insert_ai_player(player_id, is_skirmish);
        }
    }

    pub fn has_ai_player(&self, player_id: u32) -> bool {
        self.ai_players.contains_key(&player_id)
    }

    fn attach_player_interface(&mut self, player_id: u32) {
        let bridge = Arc::new(AiPlayerBridge { player_id });
        self.player_interfaces.insert(player_id, bridge.clone());
    }

    pub fn player_interface(&self, player_id: u32) -> Option<Arc<AiPlayerBridge>> {
        self.player_interfaces.get(&player_id).cloned()
    }

    fn with_player_mut<R>(
        &mut self,
        player_id: u32,
        f: impl FnOnce(&mut IntegratedAiPlayer) -> R,
    ) -> Option<R> {
        self.ai_players.get_mut(&player_id).map(f)
    }

    fn with_player<R>(
        &self,
        player_id: u32,
        f: impl FnOnce(&IntegratedAiPlayer) -> R,
    ) -> Option<R> {
        self.ai_players.get(&player_id).map(f)
    }

    pub fn xfer_ai_player(
        &mut self,
        player_id: u32,
        is_skirmish: bool,
        xfer: &mut dyn Xfer,
    ) -> Result<(), String> {
        self.ensure_ai_player(player_id, is_skirmish);
        let ai_player = self
            .ai_players
            .get_mut(&player_id)
            .ok_or_else(|| format!("AI player {} is not available for xfer", player_id))?;
        ai_player.xfer(xfer);
        Ok(())
    }

    pub fn load_post_process_ai_player(&mut self, player_id: u32) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.load_post_process();
        }
    }

    /// Remove AI player
    pub fn remove_ai_player(&mut self, player_id: u32) -> Result<(), AiError> {
        self.player_interfaces.remove(&player_id);
        if self.ai_players.remove(&player_id).is_some() {
            log::info!("Removed AI player {}", player_id);
            Ok(())
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Set AI player difficulty
    pub fn set_ai_player_difficulty(
        &mut self,
        player_id: u32,
        difficulty: GameDifficulty,
    ) -> Result<(), AiError> {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.set_difficulty(difficulty);
            Ok(())
        } else {
            Err(AiError::InvalidObject)
        }
    }

    pub fn with_ai_player_mut<F, R>(&mut self, player_id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut IntegratedAiPlayer) -> R,
    {
        let ai_player = self.ai_players.get_mut(&player_id)?;
        Some(f(ai_player))
    }

    pub fn with_ai_player<F, R>(&self, player_id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&IntegratedAiPlayer) -> R,
    {
        let ai_player = self.ai_players.get(&player_id)?;
        Some(f(ai_player))
    }

    /// Notify AI players about a new map load.
    pub fn new_map(&mut self) -> Result<(), AiError> {
        for ai_player in self.ai_players.values_mut() {
            match ai_player {
                IntegratedAiPlayer::Standard(player) => player.new_map(),
                IntegratedAiPlayer::Skirmish(player) => player.new_map(),
            }
        }

        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if let Ok(terrain) = get_terrain_logic().read() {
                        pf.rebuild_from_terrain(&terrain);
                    }

                    // Host path: empty dual-world registry residual.
                    if OBJECT_REGISTRY.is_empty() {
                        return Ok(());
                    }
                    for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
                        let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                            Some(v) => v,
                            None => continue,
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        if obj_guard.is_kind_of(KindOf::Structure)
                            || obj_guard.is_kind_of(KindOf::Building)
                            || obj_guard.is_kind_of(KindOf::Bridge)
                            || obj_guard.is_kind_of(KindOf::Barrier)
                        {
                            pf.create_wall_from_object(&obj_guard);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Add object to pathfinding map as obstacle
    pub fn add_pathfinding_obstacle(
        &self,
        object_id: ObjectID,
        positions: &[Coord3D],
        is_fence: bool,
    ) -> Result<(), AiError> {
        let ai_store = the_ai();
        let Some(ai) = ai_store.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.write();
        if let Ok(mut pf) = pathfinder_lock {
            pf.add_object_to_map(object_id, positions, is_fence);
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Remove object from pathfinding map
    pub fn remove_pathfinding_obstacle(
        &self,
        object_id: ObjectID,
        positions: &[Coord3D],
    ) -> Result<(), AiError> {
        let ai_store = the_ai();
        let Some(ai) = ai_store.read().ok() else {
            return Err(AiError::NoPathfinder);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Err(AiError::NoPathfinder);
        };
        let pathfinder_lock = pathfinder.write();
        if let Ok(mut pf) = pathfinder_lock {
            pf.remove_object_from_map(object_id, positions);
            Ok(())
        } else {
            Err(AiError::NoPathfinder)
        }
    }

    /// Register a newly created obstacle with the pathfinder.
    ///
    /// Preserves the former adapter's admission check: an AI-controlled
    /// object that is not in the shared object registry is rejected before
    /// any pathfinder work (that check used to be a side effect of building
    /// a per-object state machine nothing ever ran or read).
    pub fn notify_object_created(
        &mut self,
        object_id: ObjectID,
        position: Coord3D,
        is_ai_controlled: bool,
        is_obstacle: bool,
    ) -> Result<(), AiError> {
        if is_ai_controlled
            && (dual_world_registry_unavailable()
                || OBJECT_REGISTRY.get_object(object_id).is_none())
        {
            return Err(AiError::InvalidObject);
        }

        if is_obstacle {
            let ai_store = the_ai();
            if let Ok(ai_guard) = ai_store.read() {
                if let Some(pathfinder) = ai_guard.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        if OBJECT_REGISTRY
                            .with_object(object_id, |obj_guard| {
                                pf.create_wall_from_object(obj_guard);
                            })
                            .is_some()
                        {
                            return Ok(());
                        }
                    }
                }
            }
            let positions = vec![position];
            self.add_pathfinding_obstacle(object_id, &positions, false)?;
        }

        Ok(())
    }

    /// Remove a destroyed object's footprint from the pathfinder.
    pub fn notify_object_destroyed(
        &mut self,
        object_id: ObjectID,
        positions: &[Coord3D],
    ) -> Result<(), AiError> {
        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if OBJECT_REGISTRY
                        .with_object(object_id, |obj_guard| {
                            pf.remove_wall_from_object(obj_guard);
                        })
                        .is_some()
                    {
                        return Ok(());
                    }
                }
            }
        }
        self.remove_pathfinding_obstacle(object_id, positions)
    }

    // Internal helper methods

    /// Update all AI players in player-index order (C++ `PlayerList::update`
    /// walks `m_players[0..count]`, each `Player::update` driving its `m_ai`).
    fn update_ai_players(&mut self) -> Result<(), AiError> {
        for (player_id, ai_player) in &mut self.ai_players {
            if let Err(e) = ai_player.update() {
                log::warn!("Failed to update AI player {}: {:?}", player_id, e);
            }
        }
        Ok(())
    }

    /// Update AI players without running sensing/decision pipelines.
    pub fn update_ai_players_only(&mut self) -> Result<(), AiError> {
        self.update_ai_players()
    }

    // Helper methods removed - functionality moved to public interface

    /// Get number of active AI players
    pub fn get_ai_player_count(&self) -> usize {
        self.ai_players.len()
    }
}

// Global AI integration manager instance
lazy_static::lazy_static! {
    static ref AI_INTEGRATION_MANAGER: Arc<RwLock<Option<AiIntegrationManager>>> =
        Arc::new(RwLock::new(None));
}

/// Move the integration manager contents out for a whole-world restore
/// transaction without replacing the stable global `Arc`/`RwLock` wrapper.
///
/// A staged snapshot calls [`initialize_ai_integration`], which otherwise
/// replaces the active match's manager before the staged world is known to be
/// valid.  This raw boundary is deliberately crate-private; the runtime world
/// transaction is the only caller allowed to exchange global contents.
pub(crate) fn take_ai_integration_for_world_boundary() -> Option<AiIntegrationManager> {
    let mut manager = AI_INTEGRATION_MANAGER
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut *manager)
}

/// Install integration-manager contents at a whole-world restore boundary and
/// return the contents they replaced.  See
/// [`take_ai_integration_for_world_boundary`].
pub(crate) fn replace_ai_integration_for_world_boundary(
    next: Option<AiIntegrationManager>,
) -> Option<AiIntegrationManager> {
    let mut manager = AI_INTEGRATION_MANAGER
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *manager, next)
}

/// Initialize global AI integration manager
pub fn initialize_ai_integration() -> Result<(), AiError> {
    let mut manager_guard = AI_INTEGRATION_MANAGER.write().unwrap();
    let mut manager = AiIntegrationManager::new();
    manager.initialize()?;
    *manager_guard = Some(manager);

    log::info!("AI Integration Manager initialized globally");
    Ok(())
}

/// Get reference to global AI integration manager
pub fn with_ai_integration<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AiIntegrationManager) -> R,
{
    AI_INTEGRATION_MANAGER.read().unwrap().as_ref().map(f)
}

/// Get mutable reference to global AI integration manager
pub fn with_ai_integration_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AiIntegrationManager) -> R,
{
    AI_INTEGRATION_MANAGER.write().unwrap().as_mut().map(f)
}

/// Handle that implements Common `AIPlayerInterface` by forwarding into the
/// live GameLogic AI player owned by `AiIntegrationManager`.
#[derive(Debug)]
pub struct AiPlayerBridge {
    player_id: u32,
}

fn to_common_difficulty(
    difficulty: GameDifficulty,
) -> game_engine::common::rts::player::GameDifficulty {
    use game_engine::common::rts::player::GameDifficulty as CommonDifficulty;
    match difficulty {
        GameDifficulty::Easy => CommonDifficulty::Easy,
        GameDifficulty::Normal => CommonDifficulty::Normal,
        GameDifficulty::Hard => CommonDifficulty::Hard,
        GameDifficulty::Brutal => CommonDifficulty::Brutal,
    }
}

impl game_engine::common::rts::player::AIPlayerInterface for AiPlayerBridge {
    fn new_map(&self) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| player.new_map())
        });
    }

    fn is_skirmish_ai(&self) -> bool {
        with_ai_integration(|mgr| {
            mgr.with_player(self.player_id, |player| player.is_skirmish_ai())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    fn get_ai_enemy(&self) -> Option<i32> {
        with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| player.get_ai_enemy_index())
                .flatten()
        })
        .flatten()
    }

    fn get_ai_difficulty(&self) -> game_engine::common::rts::player::GameDifficulty {
        with_ai_integration(|mgr| {
            mgr.with_player(self.player_id, |player| {
                to_common_difficulty(player.get_ai_difficulty())
            })
            .unwrap_or(game_engine::common::rts::player::GameDifficulty::Normal)
        })
        .unwrap_or(game_engine::common::rts::player::GameDifficulty::Normal)
    }

    fn build_specific_ai_team(&self, team_name: &str, priority: bool) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_specific_ai_team(team_name, priority);
            })
        });
    }

    fn build_ai_base_defense(&self, flank: bool) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_base_defense(flank);
            })
        });
    }

    fn build_ai_base_defense_structure(&self, thing_name: &str, flank: bool) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_base_defense_structure(thing_name, flank);
            })
        });
    }

    fn build_specific_ai_building(&self, thing_name: &str) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_specific_building(thing_name);
            })
        });
    }

    fn build_by_supplies(&self, minimum_cash: i32, thing_name: &str) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_by_supplies(minimum_cash, thing_name);
            })
        });
    }

    fn build_specific_building_nearest_team(&self, thing_name: &str, team_id: i32) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_specific_building_nearest_team(thing_name, team_id);
            })
        });
    }

    fn build_upgrade(&self, upgrade_name: &str) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.build_upgrade(upgrade_name);
            })
        });
    }

    fn recruit_specific_ai_team(&self, team_name: &str, recruit_radius: f32) {
        let _ = with_ai_integration_mut(|mgr| {
            mgr.with_player_mut(self.player_id, |player| {
                let _ = player.recruit_specific_ai_team(team_name, recruit_radius);
            })
        });
    }

    fn calc_closest_construction_zone(
        &self,
        template_name: &str,
    ) -> Option<game_engine::common::rts::player::Coord3D> {
        with_ai_integration(|mgr| {
            mgr.with_player(self.player_id, |player| {
                player
                    .calc_closest_construction_zone(template_name)
                    .map(|p| game_engine::common::rts::player::Coord3D {
                        x: p.x,
                        y: p.y,
                        z: p.z,
                    })
            })
            .flatten()
        })
        .flatten()
    }
}

/// Common-facing handle for a live GameLogic AI player.
pub fn ai_player_interface(
    player_id: u32,
) -> Option<Arc<dyn game_engine::common::rts::player::AIPlayerInterface>> {
    with_ai_integration(|mgr| {
        mgr.player_interface(player_id)
            .map(|bridge| bridge as Arc<dyn game_engine::common::rts::player::AIPlayerInterface>)
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;
    use crate::object::registry::OBJECT_REGISTRY;
    use game_engine::system::xfer_load::XferLoad;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;
    use std::sync::{Arc, RwLock};

    #[test]
    fn integration_manager_xfers_ai_player_snapshot_on_load() {
        let player_id = 5;
        let mut original = AiIntegrationManager::new();
        original.ensure_ai_player(player_id, false);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original
                .xfer_ai_player(player_id, false, &mut save)
                .expect("AI player should serialize");
        }

        let mut loaded = AiIntegrationManager::new();
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut load = XferLoad::new(cursor, 1);
            loaded
                .xfer_ai_player(player_id, false, &mut load)
                .expect("AI player should deserialize");
        }

        assert!(loaded.has_ai_player(player_id));
    }
}
