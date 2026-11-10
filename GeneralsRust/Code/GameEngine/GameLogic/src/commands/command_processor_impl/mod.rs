////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Command-processor interfaces.
//!
//! The GameLogic-crate CommandProcessor (handler_*.rs) was never driven:
//! Main's command executor owns C++ `GameLogic::logicMessageDispatcher`
//! for every receiving world. Only the shared manager interfaces remain.

use std::sync::Arc;

use crate::common::{Coord3D, IRegion2D, Int, ObjectID};

/// Trait for object management interface
pub trait ObjectManager: Send + Sync {
    fn get_object(&self, id: ObjectID) -> Option<Arc<dyn GameObject>>;
    fn get_objects_in_region(&self, region: &IRegion2D) -> Vec<ObjectID>;
    fn create_object(
        &mut self,
        template: &str,
        position: Coord3D,
        player_id: Int,
    ) -> Option<ObjectID>;
    fn destroy_object(&mut self, id: ObjectID) -> bool;
}

/// Trait for player management interface  
pub trait PlayerManager: Send + Sync {
    fn get_player_resources(&self, player_id: Int) -> Option<PlayerResources>;
    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int);
    fn can_player_afford(&self, player_id: Int, cost: &ResourceCost) -> bool;
}

/// Trait for AI management interface
pub trait AIManager: Send + Sync {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool;
    fn issue_waypoint_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        self.issue_move_order(objects, destination)
    }
    fn issue_attack_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        self.issue_move_order(objects, destination)
    }
    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool;
    fn issue_build_order(&mut self, builder: ObjectID, template: &str, position: Coord3D) -> bool;
    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool;
    fn issue_targeted_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        command: crate::ai::AiCommandType,
    ) -> bool;
    fn issue_guard_position_order(
        &mut self,
        objects: &[ObjectID],
        position: Coord3D,
        guard_mode: crate::ai::GuardMode,
    ) -> bool;
    fn issue_guard_object_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        guard_mode: crate::ai::GuardMode,
    ) -> bool;
    /// C++ parity: AIUpdateInterface::queueWaypoint() — queue a waypoint without starting execution
    fn queue_waypoint_for_object(&mut self, _object_id: ObjectID, _pos: Coord3D) {}
    /// C++ parity: AIUpdateInterface::executeWaypointQueue() — start executing queued waypoints
    fn execute_waypoint_queue_for_object(&mut self, _object_id: ObjectID) {}
}

/// Trait for game object interface
pub trait GameObject: Send + Sync {
    fn get_id(&self) -> ObjectID;
    fn get_position(&self) -> Coord3D;
    fn get_owner(&self) -> Int;
    fn is_alive(&self) -> bool;
    fn can_be_controlled_by(&self, player_id: Int) -> bool;
}

/// Resource cost structure
#[derive(Debug, Clone)]
pub struct ResourceCost {
    pub supplies: Int,
    pub power: Int,
}

/// Player resources
#[derive(Debug, Clone)]
pub struct PlayerResources {
    pub supplies: Int,
    pub power_available: Int,
    pub power_used: Int,
}
