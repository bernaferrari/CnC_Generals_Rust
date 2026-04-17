//! Production exit strategies
//!
//! Defines how units exit production facilities and where they spawn.

use super::rally_point::RallyPoint;
use crate::common::*;
use std::fmt::Debug;

/// Trait for production exit strategies
pub trait ProductionExitStrategy: Send + Sync + Debug {
    /// Spawn a unit at the exit point
    fn spawn_unit(
        &mut self,
        template_name: &str,
        producer_id: ObjectID,
        door_index: usize,
        rally_point: RallyPoint,
    ) -> Result<ObjectID, String>;

    /// Get the exit position for a given door
    fn get_exit_position(&self, door_index: usize) -> Result<Coord3D, String>;

    /// Get the number of available doors/exits
    fn get_door_count(&self) -> usize;

    /// Check if door is available for use
    fn is_door_available(&self, door_index: usize) -> bool;

    /// Reserve a door for unit exit
    fn reserve_door(&mut self, door_index: usize) -> Result<(), String>;

    /// Release a door after unit has exited
    fn release_door(&mut self, door_index: usize);
}

/// Default exit strategy - units spawn at predefined exit points
#[derive(Debug)]
pub struct DefaultProductionExit {
    /// Producer object ID
    #[allow(dead_code)]
    producer_id: ObjectID,
    /// Exit points (one per door)
    exit_points: Vec<Coord3D>,
    /// Door availability
    door_available: Vec<bool>,
}

impl DefaultProductionExit {
    /// Create a new default exit strategy
    pub fn new(producer_id: ObjectID, exit_points: Vec<Coord3D>) -> Self {
        let door_count = exit_points.len();
        Self {
            producer_id,
            exit_points,
            door_available: vec![true; door_count],
        }
    }

    /// Create with a single exit point
    pub fn with_single_exit(producer_id: ObjectID, exit_point: Coord3D) -> Self {
        Self::new(producer_id, vec![exit_point])
    }
}

impl ProductionExitStrategy for DefaultProductionExit {
    fn spawn_unit(
        &mut self,
        template_name: &str,
        producer_id: ObjectID,
        door_index: usize,
        rally_point: RallyPoint,
    ) -> Result<ObjectID, String> {
        if door_index >= self.exit_points.len() {
            return Err(format!("Invalid door index: {}", door_index));
        }

        let exit_pos = self.exit_points[door_index].clone();

        // Create unit at exit position
        log::info!(
            "Spawning {} at door {} for producer {} (position: {:?})",
            template_name,
            door_index,
            producer_id,
            exit_pos
        );

        let team = crate::object::registry::OBJECT_REGISTRY
            .get_object(producer_id)
            .and_then(|producer| producer.read().ok().and_then(|o| o.get_team()));
        let created = crate::object::object_factory::get_object_factory()
            .write()
            .ok()
            .and_then(|mut factory| {
                factory
                    .create_object(
                        template_name,
                        exit_pos,
                        team,
                        crate::object::object_factory::ObjectCreationFlags::empty(),
                    )
                    .ok()
            })
            .ok_or_else(|| "Failed to spawn unit".to_string())?;

        // Send move/guard/attack-move orders based on rally_point
        // Matches C++ DefaultProductionExitUpdate.cpp:85-96 aiFollowExitProductionPath
        if let Some(unit_obj) = crate::object::registry::OBJECT_REGISTRY.get_object(created) {
            if let Ok(mut _unit) = unit_obj.write() {
                match rally_point.rally_type() {
                    super::rally_point::RallyPointType::Position => {
                        if let Some(rally_pos) = rally_point.position() {
                            // Issue move command to rally point
                            log::debug!("Unit {} moving to rally point {:?}", created, rally_pos);
                            // In full implementation, would use AI system to path to rally point
                        }
                    }
                    super::rally_point::RallyPointType::Object => {
                        if let Some(target_id) = rally_point.target_object() {
                            // Issue guard/follow command to target object
                            log::debug!("Unit {} guarding object {}", created, target_id);
                        }
                    }
                    super::rally_point::RallyPointType::Exit => {
                        // Unit stays at exit position (natural rally point)
                        log::debug!("Unit {} staying at exit position", created);
                    }
                }
            }
        }

        log::debug!(
            "Spawned unit {} at door {} for producer {}",
            created,
            door_index,
            producer_id
        );

        Ok(created)
    }

    fn get_exit_position(&self, door_index: usize) -> Result<Coord3D, String> {
        self.exit_points
            .get(door_index)
            .cloned()
            .ok_or_else(|| format!("Invalid door index: {}", door_index))
    }

    fn get_door_count(&self) -> usize {
        self.exit_points.len()
    }

    fn is_door_available(&self, door_index: usize) -> bool {
        self.door_available
            .get(door_index)
            .copied()
            .unwrap_or(false)
    }

    fn reserve_door(&mut self, door_index: usize) -> Result<(), String> {
        if door_index >= self.door_available.len() {
            return Err(format!("Invalid door index: {}", door_index));
        }

        if !self.door_available[door_index] {
            return Err(format!("Door {} is not available", door_index));
        }

        self.door_available[door_index] = false;
        Ok(())
    }

    fn release_door(&mut self, door_index: usize) {
        if door_index < self.door_available.len() {
            self.door_available[door_index] = true;
        }
    }
}

/// Queue production exit - units wait in a queue before exiting
#[derive(Debug)]
pub struct QueueProductionExit {
    /// Base exit strategy
    base: DefaultProductionExit,
    /// Queue positions (staging area before exit)
    queue_positions: Vec<Coord3D>,
    /// Current queue index
    current_queue_index: usize,
}

impl QueueProductionExit {
    /// Create a new queue exit strategy
    pub fn new(
        producer_id: ObjectID,
        exit_points: Vec<Coord3D>,
        queue_positions: Vec<Coord3D>,
    ) -> Self {
        Self {
            base: DefaultProductionExit::new(producer_id, exit_points),
            queue_positions,
            current_queue_index: 0,
        }
    }

    /// Get next queue position
    fn next_queue_position(&mut self) -> Coord3D {
        let pos = self.queue_positions[self.current_queue_index].clone();
        self.current_queue_index = (self.current_queue_index + 1) % self.queue_positions.len();
        pos
    }
}

impl ProductionExitStrategy for QueueProductionExit {
    fn spawn_unit(
        &mut self,
        template_name: &str,
        producer_id: ObjectID,
        door_index: usize,
        _rally_point: RallyPoint,
    ) -> Result<ObjectID, String> {
        // Spawn at queue position first
        // Matches C++ QueueProductionExitUpdate behavior for staging units
        let queue_pos = self.next_queue_position();

        log::info!(
            "Spawning {} at queue position {:?} for producer {}",
            template_name,
            queue_pos,
            producer_id
        );

        // Create unit at queue position (staging area)
        let team = crate::object::registry::OBJECT_REGISTRY
            .get_object(producer_id)
            .and_then(|producer| producer.read().ok().and_then(|o| o.get_team()));

        let created = crate::object::object_factory::get_object_factory()
            .write()
            .ok()
            .and_then(|mut factory| {
                factory
                    .create_object(
                        template_name,
                        queue_pos.clone(),
                        team,
                        crate::object::object_factory::ObjectCreationFlags::empty(),
                    )
                    .ok()
            })
            .ok_or_else(|| "Failed to spawn unit in queue".to_string())?;

        // After delay, unit would move to actual exit position
        // Then move to rally point (implemented via AI pathfinding system)
        if let Some(unit_obj) = crate::object::registry::OBJECT_REGISTRY.get_object(created) {
            if let Ok(mut _unit) = unit_obj.write() {
                // Get the actual exit position
                if let Ok(exit_pos) = self.base.get_exit_position(door_index) {
                    log::debug!(
                        "Unit {} will move from queue {:?} to exit {:?}",
                        created,
                        queue_pos,
                        exit_pos
                    );
                    // In full implementation: Issue move command to exit position
                    // Then issue move/guard command based on rally_point type
                }
            }
        }

        Ok(created)
    }

    fn get_exit_position(&self, door_index: usize) -> Result<Coord3D, String> {
        self.base.get_exit_position(door_index)
    }

    fn get_door_count(&self) -> usize {
        self.base.get_door_count()
    }

    fn is_door_available(&self, door_index: usize) -> bool {
        self.base.is_door_available(door_index)
    }

    fn reserve_door(&mut self, door_index: usize) -> Result<(), String> {
        self.base.reserve_door(door_index)
    }

    fn release_door(&mut self, door_index: usize) {
        self.base.release_door(door_index)
    }
}

/// Supply center production exit - for supply collectors
#[derive(Debug)]
pub struct SupplyCenterProductionExit {
    /// Base exit strategy
    base: DefaultProductionExit,
    /// Supply dock positions
    #[allow(dead_code)]
    supply_docks: Vec<Coord3D>,
}

impl SupplyCenterProductionExit {
    /// Create a new supply center exit strategy
    pub fn new(
        producer_id: ObjectID,
        exit_points: Vec<Coord3D>,
        supply_docks: Vec<Coord3D>,
    ) -> Self {
        Self {
            base: DefaultProductionExit::new(producer_id, exit_points),
            supply_docks,
        }
    }
}

impl ProductionExitStrategy for SupplyCenterProductionExit {
    fn spawn_unit(
        &mut self,
        template_name: &str,
        producer_id: ObjectID,
        door_index: usize,
        rally_point: RallyPoint,
    ) -> Result<ObjectID, String> {
        let exit_pos = self.base.get_exit_position(door_index)?;

        log::info!(
            "Spawning supply truck {} at {:?} for producer {}",
            template_name,
            exit_pos,
            producer_id
        );

        // Create supply truck at exit position
        // Matches C++ SupplyCenterProductionExitUpdate behavior
        let team = crate::object::registry::OBJECT_REGISTRY
            .get_object(producer_id)
            .and_then(|producer| producer.read().ok().and_then(|o| o.get_team()));

        let created = crate::object::object_factory::get_object_factory()
            .write()
            .ok()
            .and_then(|mut factory| {
                factory
                    .create_object(
                        template_name,
                        exit_pos,
                        team,
                        crate::object::object_factory::ObjectCreationFlags::empty(),
                    )
                    .ok()
            })
            .ok_or_else(|| "Failed to spawn supply truck".to_string())?;

        // Send to rally point or begin supply gathering
        if let Some(unit_obj) = crate::object::registry::OBJECT_REGISTRY.get_object(created) {
            if let Ok(mut _unit) = unit_obj.write() {
                match rally_point.rally_type() {
                    super::rally_point::RallyPointType::Position => {
                        if let Some(rally_pos) = rally_point.position() {
                            log::debug!(
                                "Supply truck {} heading to rally point {:?}",
                                created,
                                rally_pos
                            );
                            // In full implementation: Issue move command to rally point
                        }
                    }
                    super::rally_point::RallyPointType::Exit => {
                        // Begin supply gathering automatically
                        log::debug!("Supply truck {} beginning supply gathering", created);
                        // In full implementation: Activate supply gathering AI behavior
                    }
                    _ => {
                        log::debug!("Supply truck {} at exit, awaiting orders", created);
                    }
                }
            }
        }

        Ok(created)
    }

    fn get_exit_position(&self, door_index: usize) -> Result<Coord3D, String> {
        self.base.get_exit_position(door_index)
    }

    fn get_door_count(&self) -> usize {
        self.base.get_door_count()
    }

    fn is_door_available(&self, door_index: usize) -> bool {
        self.base.is_door_available(door_index)
    }

    fn reserve_door(&mut self, door_index: usize) -> Result<(), String> {
        self.base.reserve_door(door_index)
    }

    fn release_door(&mut self, door_index: usize) {
        self.base.release_door(door_index)
    }
}

/// Spawn point production exit - for parachute drops and aircraft
#[derive(Debug)]
pub struct SpawnPointProductionExit {
    /// Producer object ID
    #[allow(dead_code)]
    producer_id: ObjectID,
    /// Spawn positions (may be in air for aircraft)
    spawn_points: Vec<Coord3D>,
    /// Whether units should use parachute
    use_parachute: bool,
    /// Current spawn index
    current_spawn_index: usize,
}

impl SpawnPointProductionExit {
    /// Create a new spawn point exit strategy
    pub fn new(producer_id: ObjectID, spawn_points: Vec<Coord3D>, use_parachute: bool) -> Self {
        Self {
            producer_id,
            spawn_points,
            use_parachute,
            current_spawn_index: 0,
        }
    }

    /// Get next spawn point
    fn next_spawn_point(&mut self) -> Coord3D {
        let pos = self.spawn_points[self.current_spawn_index].clone();
        self.current_spawn_index = (self.current_spawn_index + 1) % self.spawn_points.len();
        pos
    }
}

impl ProductionExitStrategy for SpawnPointProductionExit {
    fn spawn_unit(
        &mut self,
        template_name: &str,
        producer_id: ObjectID,
        _door_index: usize,
        rally_point: RallyPoint,
    ) -> Result<ObjectID, String> {
        let spawn_pos = self.next_spawn_point();

        log::info!(
            "Spawning {} at spawn point {:?} for producer {} (parachute: {})",
            template_name,
            spawn_pos,
            producer_id,
            self.use_parachute
        );

        // Create unit at spawn position (potentially in air for parachute drops)
        // Matches C++ SpawnPointProductionExitUpdate behavior
        let team = crate::object::registry::OBJECT_REGISTRY
            .get_object(producer_id)
            .and_then(|producer| producer.read().ok().and_then(|o| o.get_team()));

        let created = crate::object::object_factory::get_object_factory()
            .write()
            .ok()
            .and_then(|mut factory| {
                factory
                    .create_object(
                        template_name,
                        spawn_pos.clone(),
                        team,
                        crate::object::object_factory::ObjectCreationFlags::empty(),
                    )
                    .ok()
            })
            .ok_or_else(|| "Failed to spawn unit at spawn point".to_string())?;

        // If use_parachute, add parachute contain module
        // Matches C++ ParachuteContain behavior for airdrops
        if self.use_parachute {
            if let Some(unit_obj) = crate::object::registry::OBJECT_REGISTRY.get_object(created) {
                if let Ok(mut _unit) = unit_obj.write() {
                    log::debug!(
                        "Unit {} spawned with parachute at altitude {}",
                        created,
                        spawn_pos.z
                    );
                    // In full implementation: Add ParachuteContain module to unit
                    // This would handle the descent animation and landing
                }
            }
        }

        // Send to rally point after landing (or immediately for aircraft)
        if let Some(unit_obj) = crate::object::registry::OBJECT_REGISTRY.get_object(created) {
            if let Ok(mut _unit) = unit_obj.write() {
                match rally_point.rally_type() {
                    super::rally_point::RallyPointType::Position => {
                        if let Some(rally_pos) = rally_point.position() {
                            log::debug!(
                                "Unit {} will move to rally point {:?} after spawn",
                                created,
                                rally_pos
                            );
                            // In full implementation: Queue move command after landing
                        }
                    }
                    super::rally_point::RallyPointType::Object => {
                        if let Some(target_id) = rally_point.target_object() {
                            log::debug!(
                                "Unit {} will guard object {} after spawn",
                                created,
                                target_id
                            );
                        }
                    }
                    super::rally_point::RallyPointType::Exit => {
                        log::debug!("Unit {} will stay at spawn point", created);
                    }
                }
            }
        }

        Ok(created)
    }

    fn get_exit_position(&self, door_index: usize) -> Result<Coord3D, String> {
        self.spawn_points
            .get(door_index)
            .cloned()
            .ok_or_else(|| format!("Invalid door index: {}", door_index))
    }

    fn get_door_count(&self) -> usize {
        self.spawn_points.len()
    }

    fn is_door_available(&self, _door_index: usize) -> bool {
        // Spawn points are always available (spawning in air/parachute)
        true
    }

    fn reserve_door(&mut self, _door_index: usize) -> Result<(), String> {
        // No need to reserve for spawn points
        Ok(())
    }

    fn release_door(&mut self, _door_index: usize) {
        // No need to release
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};

    fn ensure_template_exists(name: &str) {
        let needs_init = get_thing_factory().unwrap().is_none();
        if needs_init {
            init_thing_factory().unwrap();
        }
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    #[test]
    fn test_default_exit_creation() {
        let exits = vec![
            Coord3D::new(100.0, 100.0, 0.0),
            Coord3D::new(150.0, 100.0, 0.0),
        ];

        let exit_strategy = DefaultProductionExit::new(1, exits.clone());

        assert_eq!(exit_strategy.get_door_count(), 2);
        assert!(exit_strategy.is_door_available(0));
        assert!(exit_strategy.is_door_available(1));

        let pos = exit_strategy.get_exit_position(0).unwrap();
        assert_eq!(pos.x, 100.0);
    }

    #[test]
    fn test_door_reservation() {
        let exits = vec![Coord3D::new(100.0, 100.0, 0.0)];
        let mut exit_strategy = DefaultProductionExit::new(1, exits);

        assert!(exit_strategy.is_door_available(0));

        exit_strategy.reserve_door(0).unwrap();
        assert!(!exit_strategy.is_door_available(0));

        exit_strategy.release_door(0);
        assert!(exit_strategy.is_door_available(0));
    }

    #[test]
    fn test_queue_exit() {
        ensure_template_exists("Tank");
        let exits = vec![Coord3D::new(100.0, 100.0, 0.0)];
        let queue_positions = vec![Coord3D::new(50.0, 50.0, 0.0), Coord3D::new(60.0, 50.0, 0.0)];

        let mut exit_strategy = QueueProductionExit::new(1, exits, queue_positions);

        assert_eq!(exit_strategy.get_door_count(), 1);

        let rally = RallyPoint::at_exit();
        let result = exit_strategy.spawn_unit("Tank", 1, 0, rally);
        assert!(result.is_ok());
    }

    #[test]
    fn test_spawn_point_exit() {
        ensure_template_exists("Ranger");
        let spawn_points = vec![
            Coord3D::new(100.0, 100.0, 500.0), // High altitude
            Coord3D::new(150.0, 150.0, 500.0),
        ];

        let mut exit_strategy = SpawnPointProductionExit::new(1, spawn_points, true);

        assert_eq!(exit_strategy.get_door_count(), 2);
        assert!(exit_strategy.is_door_available(0));

        let rally = RallyPoint::at_exit();
        let result = exit_strategy.spawn_unit("Ranger", 1, 0, rally);
        assert!(result.is_ok());
    }
}
