//! Build Placement Mode
//!
//! UI state machine for placing buildings on the map.
//! Matches C++ BuildPlacingUpdate and InGameUI placement logic.
//!
//! Flow:
//! 1. Player clicks build button → enters placement mode
//! 2. Building outline follows cursor, shows valid/invalid placement
//! 3. Player clicks to place → creates foundation (ghost building), assigns dozer
//! 4. Construction begins with dozer
//! 5. On complete: building becomes active, unlocks new units/abilities

use crate::common::*;
use crate::economy::IncomeSource;
use crate::helpers::{TheGameLogic, TheInGameUI, ThePartitionManager, TheThingFactory};
use crate::object::production::construction::{
    get_construction_manager, ConstructionState, FoundationValidator,
};
use crate::object::production::prerequisite_checker::PrerequisiteChecker;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

/// Placement mode state.
/// Matches C++ BuildPlacingState from InGameUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPlacementState {
    /// Not in placement mode
    Inactive,
    /// Building outline follows cursor
    Placing,
    /// Waiting for a valid placement location
    WaitingForValidLocation,
    /// Placement confirmed, creating building
    Confirming,
}

/// Result of a placement validation check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementValidity {
    /// Location is valid for building
    Valid,
    /// Location is outside the map bounds
    OutsideMap,
    /// Terrain is not suitable (cliff, water, etc.)
    BadTerrain,
    /// Location is blocked by another object
    Blocked,
    /// Location is not revealed (in shroud/fog)
    NotRevealed,
    /// Player doesn't have enough money
    InsufficientFunds,
    /// Missing prerequisite building
    MissingPrerequisite,
    /// Location is too far from existing structures
    TooFarFromBase,
    /// No dozer available to build
    NoDozerAvailable,
}

impl PlacementValidity {
    /// Returns true if the placement is valid
    pub fn is_valid(self) -> bool {
        self == PlacementValidity::Valid
    }
}

/// Build placement mode manager
/// Manages the state of placing a building on the map.
/// Matches C++ InGameUI build placement behavior.
pub struct BuildPlacementMode {
    /// Current state of the placement mode
    state: BuildPlacementState,
    /// Template name of the building being placed
    template_name: String,
    /// Player ID placing the building
    player_id: ObjectID,
    /// Current cursor position in world coordinates
    cursor_position: Coord3D,
    /// Current building angle
    angle: f32,
    /// Build cost of the structure
    cost: i32,
    /// Build time in frames
    build_time_frames: u32,
    /// Foundation validator
    validator: FoundationValidator,
    /// Last validation result (cached for UI feedback)
    last_validity: PlacementValidity,
    /// Whether to snap to grid
    snap_to_grid: bool,
}

impl BuildPlacementMode {
    /// Create a new build placement mode (initially inactive)
    pub fn new() -> Self {
        Self {
            state: BuildPlacementState::Inactive,
            template_name: String::new(),
            player_id: INVALID_OBJECT_ID,
            cursor_position: Coord3D::ZERO,
            angle: 0.0,
            cost: 0,
            build_time_frames: 0,
            validator: FoundationValidator::new_strict(),
            last_validity: PlacementValidity::Valid,
            snap_to_grid: true,
        }
    }

    /// Enter placement mode for a specific building template.
    /// Matches C++ InGameUI::startBuildPlacement.
    pub fn enter_placement_mode(
        &mut self,
        template_name: String,
        player_id: ObjectID,
    ) -> Result<(), String> {
        // Look up template
        let template = TheThingFactory::find_template(&template_name)
            .ok_or_else(|| format!("Template '{}' not found", template_name))?;

        // Get build cost and time
        let cost = template.get_build_cost();
        let build_time = template.calc_time_to_build(None).max(1) as u32;

        // Check if player can afford it
        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(player_id as i32) {
                if let Ok(player_guard) = player.read() {
                    let money = player_guard.get_money().get_money();
                    if (money as i32) < cost {
                        return Err("Insufficient funds".to_string());
                    }
                }
            }
        }

        self.state = BuildPlacementState::Placing;
        self.template_name = template_name;
        self.player_id = player_id;
        self.cost = cost;
        self.build_time_frames = build_time;
        self.last_validity = PlacementValidity::Valid;

        log::debug!(
            "Entered placement mode for '{}' (cost: {}, build_time: {} frames)",
            self.template_name,
            cost,
            self.build_time_frames
        );

        Ok(())
    }

    /// Exit placement mode, cancelling the build.
    /// Matches C++ InGameUI::cancelBuildPlacement.
    pub fn exit_placement_mode(&mut self) {
        self.state = BuildPlacementState::Inactive;
        self.template_name.clear();
        self.last_validity = PlacementValidity::Valid;
    }

    /// Check if currently in placement mode
    pub fn is_active(&self) -> bool {
        self.state != BuildPlacementState::Inactive
    }

    /// Get current placement state
    pub fn state(&self) -> BuildPlacementState {
        self.state
    }

    /// Get the template name being placed
    pub fn template_name(&self) -> &str {
        &self.template_name
    }

    /// Get the player ID
    pub fn player_id(&self) -> ObjectID {
        self.player_id
    }

    /// Update cursor position (called each frame from UI).
    /// Matches C++ InGameUI::setBuildPosition.
    pub fn update_cursor_position(&mut self, pos: Coord3D) {
        self.cursor_position = pos;

        // Validate the new position
        if self.is_active() {
            self.last_validity = self.validate_placement_internal();
        }
    }

    /// Get current cursor position
    pub fn cursor_position(&self) -> &Coord3D {
        &self.cursor_position
    }

    /// Set the building angle (rotation).
    /// Matches C++ placement rotation.
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
    }

    /// Get the building angle
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Get the last validation result
    pub fn last_validity(&self) -> PlacementValidity {
        self.last_validity
    }

    /// Rotate the building 90 degrees.
    /// Matches C++ InGameUI::rotateBuildPlacement.
    pub fn rotate_90(&mut self) {
        self.angle += std::f32::consts::FRAC_PI_2;
        // Normalize to [0, 2π)
        while self.angle >= 2.0 * std::f32::consts::PI {
            self.angle -= 2.0 * std::f32::consts::PI;
        }
        // Re-validate after rotation
        if self.is_active() {
            self.last_validity = self.validate_placement_internal();
        }
    }

    /// Validate current placement position.
    /// Matches C++ BuildAssistant::isLocationLegalToBuild.
    pub fn validate_placement(&mut self) -> PlacementValidity {
        self.last_validity = self.validate_placement_internal();
        self.last_validity
    }

    /// Internal validation logic.
    fn validate_placement_internal(&self) -> PlacementValidity {
        // Check funds
        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(player_guard) = player.read() {
                    let money = player_guard.get_money().get_money();
                    if (money as i32) < self.cost {
                        return PlacementValidity::InsufficientFunds;
                    }
                }
            }
        }

        // Check terrain, overlap, shroud via FoundationValidator
        if let Err(reason) = self.validator.validate_placement(
            &self.cursor_position,
            &self.template_name,
            self.angle,
            self.player_id,
        ) {
            return match reason.as_str() {
                "Location outside playable area" | "Location on bridge" => {
                    PlacementValidity::OutsideMap
                }
                "Location underwater" | "Location on cliff" | "Location not flat enough" => {
                    PlacementValidity::BadTerrain
                }
                "Location blocked by immobile object"
                | "Location blocked by enemy object"
                | "Location blocked by object" => PlacementValidity::Blocked,
                "Location not visible" => PlacementValidity::NotRevealed,
                "Location blocked by stealth" => PlacementValidity::Blocked,
                _ => PlacementValidity::BadTerrain,
            };
        }

        PlacementValidity::Valid
    }

    /// Attempt to place the building at the current cursor position.
    /// Returns Ok(object_id) if placement was successful.
    /// Matches C++ InGameUI::placeBuild.
    pub fn place_building(&mut self) -> Result<ObjectID, PlacementValidity> {
        // Validate first
        let validity = self.validate_placement();
        if !validity.is_valid() {
            return Err(validity);
        }

        // Find an available dozer
        let dozer_id = self.find_available_dozer();
        if dozer_id == INVALID_OBJECT_ID {
            return Err(PlacementValidity::NoDozerAvailable);
        }

        // Create the building object at the placement position
        let building_id = self.create_building_at_placement()?;

        // Deduct cost from player
        self.deduct_cost();

        // Start construction via the construction manager
        self.start_construction(building_id, dozer_id);

        // Exit placement mode
        let placed_id = building_id;
        self.exit_placement_mode();

        Ok(placed_id)
    }

    /// Find an available dozer for the player.
    /// Matches C++ logic for finding idle dozers.
    fn find_available_dozer(&self) -> ObjectID {
        let Some(partition) = ThePartitionManager::get() else {
            return INVALID_OBJECT_ID;
        };

        let scan_radius = 500.0; // Search radius for dozers

        for obj_id in partition.get_objects_in_range(&self.cursor_position, scan_radius) {
            let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            // Check if it's a dozer/worker
            if !obj_guard.is_kind_of(KindOf::Dozer) {
                continue;
            }

            // Check ownership
            let Some(controller_id) = obj_guard.get_controlling_player_id() else {
                continue;
            };
            if controller_id as ObjectID != self.player_id {
                continue;
            }

            // Check if dozer is idle (not currently constructing)
            let manager = get_construction_manager();
            if let Ok(mgr) = manager.read() {
                if mgr.is_dozer_busy(obj_id) {
                    continue;
                }
            }

            // Check if dozer is not destroyed and not under construction
            if obj_guard.is_destroyed() || obj_guard.is_under_construction() {
                continue;
            }

            return obj_id;
        }

        // Also check all player objects (dozer might be far away)
        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(player_guard) = player.read() {
                    if let Some(team) = player_guard.get_default_team() {
                        if let Ok(team_guard) = team.read() {
                            for member_id in team_guard.get_members() {
                                let Some(member_arc) =
                                    crate::object::registry::OBJECT_REGISTRY.get_object(member_id)
                                else {
                                    continue;
                                };
                                let Ok(member_guard) = member_arc.read() else {
                                    continue;
                                };

                                if !member_guard.is_kind_of(KindOf::Dozer) {
                                    continue;
                                }
                                if member_guard.is_destroyed()
                                    || member_guard.is_under_construction()
                                {
                                    continue;
                                }

                                let manager = get_construction_manager();
                                if let Ok(mgr) = manager.read() {
                                    if mgr.is_dozer_busy(member_id) {
                                        continue;
                                    }
                                }

                                return member_id;
                            }
                        }
                    }
                }
            }
        }

        INVALID_OBJECT_ID
    }

    /// Create the building object at the placement position.
    fn create_building_at_placement(&self) -> Result<ObjectID, PlacementValidity> {
        let template = TheThingFactory::find_template(&self.template_name)
            .ok_or(PlacementValidity::BadTerrain)?;

        // Get player's team
        let team = if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(player_guard) = player.read() {
                    player_guard.get_default_team()
                } else {
                    return Err(PlacementValidity::MissingPrerequisite);
                }
            } else {
                return Err(PlacementValidity::MissingPrerequisite);
            }
        } else {
            return Err(PlacementValidity::MissingPrerequisite);
        };

        let Some(team_arc) = team else {
            return Err(PlacementValidity::MissingPrerequisite);
        };
        let team_guard = team_arc
            .read()
            .map_err(|_| PlacementValidity::MissingPrerequisite)?;

        // Create the object
        let factory = TheThingFactory::get().map_err(|_| PlacementValidity::BadTerrain)?;
        let new_object = factory
            .new_object(template, &*team_guard)
            .map_err(|_| PlacementValidity::Blocked)?;

        // Set position and orientation
        if let Ok(mut obj_guard) = new_object.write() {
            obj_guard.set_position(&self.cursor_position);
            obj_guard.set_angle(self.angle);

            // Mark as under construction
            obj_guard.set_construction_percent(0.0);
            obj_guard.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::UnderConstruction),
                true,
            );

            // Set model condition to awaiting construction
            obj_guard.set_model_condition_state(ModelConditionFlags::AWAITING_CONSTRUCTION);

            // Set health to 1 (like C++ Object construction start)
            if let Some(body) = obj_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    body_guard.set_health(1.0);
                }
            }

            Ok(obj_guard.get_id())
        } else {
            Err(PlacementValidity::Blocked)
        }
    }

    /// Deduct the build cost from the player.
    fn deduct_cost(&self) {
        if self.cost <= 0 {
            return;
        }

        if let Ok(list) = player_list().read() {
            if let Some(player) = list.get_player(self.player_id as i32) {
                if let Ok(mut player_guard) = player.write() {
                    match player_guard.get_money_mut().withdraw(self.cost as u32) {
                        Ok(_) => {
                            player_guard
                                .get_score_keeper_mut()
                                .add_money_spent(self.cost as u32);
                            log::debug!(
                                "Deducted {} credits from player {} for building '{}'",
                                self.cost,
                                self.player_id,
                                self.template_name
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to deduct {} credits from player {}: {}",
                                self.cost,
                                self.player_id,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Start construction via the construction manager and assign the dozer.
    fn start_construction(&self, building_id: ObjectID, dozer_id: ObjectID) {
        // Get max health from the building template
        let max_health = TheThingFactory::find_template(&self.template_name)
            .and_then(|template| {
                TheGameLogic::find_object_by_id(building_id).and_then(|obj| {
                    obj.read().ok().and_then(|guard| {
                        guard
                            .get_body_module()
                            .and_then(|body| body.lock().ok().map(|b| b.get_max_health()))
                    })
                })
            })
            .unwrap_or(1000.0);

        // Start construction via the global construction manager
        let manager = get_construction_manager();
        if let Ok(mut mgr) = manager.write() {
            if let Err(e) = mgr.start_construction(
                building_id,
                dozer_id,
                max_health,
                self.build_time_frames.max(1),
                false, // Not a rebuild
            ) {
                log::warn!(
                    "Failed to start construction for building {}: {}",
                    building_id,
                    e
                );
            }
        }

        // Set model condition on the building
        if let Some(building) = TheGameLogic::find_object_by_id(building_id) {
            if let Ok(mut guard) = building.write() {
                guard.clear_model_condition_state(ModelConditionFlags::AWAITING_CONSTRUCTION);
                guard.set_model_condition_state(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED);
            }
        }

        // Tell the dozer to build
        if let Some(dozer) = TheGameLogic::find_object_by_id(dozer_id) {
            if let Ok(mut guard) = dozer.write() {
                guard.set_model_condition_state(ModelConditionFlags::ACTIVELY_CONSTRUCTING);
                // The DozerAIUpdate will pick up the construction via the construction manager
            }
        }

        log::info!(
            "Started construction of '{}' (id={}) by dozer (id={}) at {:?}",
            self.template_name,
            building_id,
            dozer_id,
            self.cursor_position
        );
    }
}

impl Default for BuildPlacementMode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placement_mode_lifecycle() {
        let mut mode = BuildPlacementMode::new();

        assert!(!mode.is_active());
        assert_eq!(mode.state(), BuildPlacementState::Inactive);

        // We can't test enter_placement_mode without templates registered,
        // but we can test the state transitions
        mode.state = BuildPlacementState::Placing;
        assert!(mode.is_active());
        assert_eq!(mode.state(), BuildPlacementState::Placing);

        mode.exit_placement_mode();
        assert!(!mode.is_active());
        assert_eq!(mode.state(), BuildPlacementState::Inactive);
    }

    #[test]
    fn test_rotation() {
        let mut mode = BuildPlacementMode::new();

        assert!((mode.angle() - 0.0).abs() < 0.001);

        mode.rotate_90();
        assert!((mode.angle() - std::f32::consts::FRAC_PI_2).abs() < 0.001);

        mode.rotate_90();
        assert!((mode.angle() - std::f32::consts::PI).abs() < 0.001);

        mode.rotate_90();
        mode.rotate_90();
        // Should wrap back to ~0
        assert!(mode.angle() < 0.01 || (mode.angle() - 2.0 * std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    fn test_cursor_update() {
        let mut mode = BuildPlacementMode::new();
        mode.state = BuildPlacementState::Placing;
        mode.template_name = "TestBuilding".to_string();

        let pos = Coord3D::new(100.0, 200.0, 0.0);
        mode.update_cursor_position(pos.clone());

        assert_eq!(mode.cursor_position(), &pos);
    }

    #[test]
    fn test_validity_check() {
        let validity = PlacementValidity::Valid;
        assert!(validity.is_valid());

        assert!(!PlacementValidity::Blocked.is_valid());
        assert!(!PlacementValidity::InsufficientFunds.is_valid());
        assert!(!PlacementValidity::BadTerrain.is_valid());
    }
}
