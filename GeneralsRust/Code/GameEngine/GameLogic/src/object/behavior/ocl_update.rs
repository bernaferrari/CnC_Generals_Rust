//! OCLUpdate - Object Creation List update module
//!
//! Handles spawning objects in various patterns for special powers and effects.
//! Used by major superweapons and special abilities.
//!
//! Spawn patterns:
//! - PARALLEL: Objects spawn simultaneously
//! - LINE: Objects spawn in a line
//! - CIRCLE: Objects spawn in a circular pattern
//! - STAR: Objects spawn in a star pattern
//! - SPIRAL: Objects spawn in a spiral
//!
//! Used by:
//! - Particle Cannon (creates damage objects in line)
//! - Nuclear Missile (creates explosion objects in circle)
//! - Scud Storm (creates toxic clouds in area)
//! - Cluster mines (creates mine objects)
//! - Carpet bombing (creates bomb objects in line)
//!
//! Original C++ Author: EA Developers
//! Rust conversion: 2025

use serde::{Deserialize, Serialize};
use crate::common::{ObjectID, Real, Coord3D, UnsignedInt, Bool};
use crate::helpers::{TheTerrainLogic, TheThingFactory};
use crate::player::ThePlayerList;
use crate::team::get_team_factory;
use crate::modules::OCLUpdateInterface;
use std::f32::consts::PI;

/// Object creation disposition (spawn pattern)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OCLDisposition {
    /// All objects created simultaneously
    UseMembersWeapon,
    /// Objects created in a line
    SendItToFirstAvailableUnit,
    /// Objects created in a circle
    SendItToEachMemberOfTheTeam,
    /// Objects created in a star pattern
    DoParallelCreate,
    /// Objects created in a spiral
    DoLineCreate,
    /// Custom pattern
    DoCircleCreate,
}

impl Default for OCLDisposition {
    fn default() -> Self {
        OCLDisposition::DoParallelCreate
    }
}

/// Object to create in the OCL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OCLCreateObject {
    /// Name of object template to create
    pub object_name: String,
    /// Number of this object to create
    pub count: u32,
    /// Offset from center position
    pub offset: Coord3D,
    /// Disposition/pattern
    pub disposition: OCLDisposition,
}

impl OCLCreateObject {
    pub fn new(name: String, count: u32) -> Self {
        Self {
            object_name: name,
            count,
            offset: [0.0, 0.0, 0.0],
            disposition: OCLDisposition::DoParallelCreate,
        }
    }
}

/// OCL update module configuration (from INI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OCLUpdateModuleData {
    /// Name of the OCL definition
    pub ocl_name: String,

    /// Objects to create
    #[serde(default)]
    pub create_objects: Vec<OCLCreateObject>,

    /// Minimum number of objects to create
    #[serde(default)]
    pub min_count: u32,

    /// Maximum number of objects to create
    #[serde(default)]
    pub max_count: u32,

    /// Spread radius for random placement
    #[serde(default)]
    pub spread_radius: Real,

    /// Delay between object creations (frames)
    #[serde(default)]
    pub creation_delay: UnsignedInt,

    /// Should objects be placed on ground?
    #[serde(default = "default_true")]
    pub place_on_ground: Bool,

    /// Initial velocity for created objects
    #[serde(default)]
    pub initial_velocity: Option<Coord3D>,

    /// Should created objects inherit owner's team?
    #[serde(default = "default_true")]
    pub inherit_team: Bool,

    /// Should created objects inherit owner's player?
    #[serde(default = "default_true")]
    pub inherit_player: Bool,

    /// Angle spread for line/circle patterns (radians)
    #[serde(default = "default_full_circle")]
    pub angle_spread: Real,

    /// Number of objects in circle/star patterns
    #[serde(default = "default_circle_count")]
    pub pattern_count: u32,

    /// Radius for circle patterns
    #[serde(default = "default_radius")]
    pub pattern_radius: Real,
}

fn default_true() -> Bool {
    true
}

fn default_full_circle() -> Real {
    2.0 * PI
}

fn default_circle_count() -> u32 {
    8
}

fn default_radius() -> Real {
    50.0
}

impl Default for OCLUpdateModuleData {
    fn default() -> Self {
        Self {
            ocl_name: String::new(),
            create_objects: Vec::new(),
            min_count: 1,
            max_count: 1,
            spread_radius: 0.0,
            creation_delay: 0,
            place_on_ground: true,
            initial_velocity: None,
            inherit_team: true,
            inherit_player: true,
            angle_spread: 2.0 * PI,
            pattern_count: 8,
            pattern_radius: 50.0,
        }
    }
}

/// OCL creation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OCLState {
    /// Idle, no creation in progress
    Idle,
    /// Creating objects
    Creating,
    /// Creation complete
    Complete,
}

/// OCL update behavior module
pub struct OCLUpdate {
    /// Configuration data
    data: OCLUpdateModuleData,

    /// Current state
    state: OCLState,

    /// Objects created so far
    objects_created: u32,

    /// Total objects to create this cycle
    target_count: u32,

    /// Frame when next object should be created
    next_creation_frame: UnsignedInt,

    /// Current frame
    current_frame: UnsignedInt,

    /// Center position for creation
    center_position: Coord3D,

    /// Owner team ID
    owner_team_id: Option<u32>,

    /// Owner player ID
    owner_player_id: Option<u32>,

    /// IDs of created objects
    created_object_ids: Vec<ObjectID>,
}

impl OCLUpdate {
    pub fn new(data: OCLUpdateModuleData) -> Self {
        Self {
            data,
            state: OCLState::Idle,
            objects_created: 0,
            target_count: 0,
            next_creation_frame: 0,
            current_frame: 0,
            center_position: [0.0, 0.0, 0.0],
            owner_team_id: None,
            owner_player_id: None,
            created_object_ids: Vec::new(),
        }
    }

    /// Start OCL creation at a position
    pub fn start_creation(
        &mut self,
        position: Coord3D,
        team_id: Option<u32>,
        player_id: Option<u32>,
    ) {
        self.state = OCLState::Creating;
        self.center_position = position;
        self.owner_team_id = team_id;
        self.owner_player_id = player_id;
        self.objects_created = 0;
        self.created_object_ids.clear();

        // Determine how many objects to create
        use rand::Rng;
        let mut rng = rand::thread_rng();

        if self.data.min_count == self.data.max_count {
            self.target_count = self.data.min_count;
        } else {
            let range = self.data.max_count - self.data.min_count;
            self.target_count = self.data.min_count + rng.gen_range(0..=range);
        }

        self.next_creation_frame = self.current_frame;
    }

    /// Calculate position for an object based on pattern
    fn calculate_spawn_position(
        &self,
        index: u32,
        total: u32,
        disposition: OCLDisposition,
        offset: Coord3D,
    ) -> Coord3D {
        let mut pos = self.center_position;

        // Apply base offset
        pos[0] += offset[0];
        pos[1] += offset[1];
        pos[2] += offset[2];

        match disposition {
            OCLDisposition::DoParallelCreate => {
                // All at same position with optional spread
                if self.data.spread_radius > 0.0 {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    let angle = rng.gen::<Real>() * 2.0 * PI;
                    let distance = rng.gen::<Real>() * self.data.spread_radius;
                    pos[0] += distance * angle.cos();
                    pos[1] += distance * angle.sin();
                }
            }
            OCLDisposition::DoLineCreate => {
                // Line pattern
                if total > 1 {
                    let spacing = self.data.pattern_radius * 2.0 / (total - 1) as Real;
                    let offset_along_line = (index as Real) * spacing - self.data.pattern_radius;
                    pos[0] += offset_along_line;
                }
            }
            OCLDisposition::DoCircleCreate => {
                // Circle pattern
                let angle = (index as Real) * self.data.angle_spread / (total as Real);
                pos[0] += self.data.pattern_radius * angle.cos();
                pos[1] += self.data.pattern_radius * angle.sin();
            }
            _ => {
                // Other dispositions use default (parallel)
            }
        }

        // Place on ground if configured
        if self.data.place_on_ground {
            pos[2] = 0.0; // Would query terrain height here
        }

        pos
    }

    /// Create a single object
    fn create_object(
        &mut self,
        object_def: &OCLCreateObject,
        index: u32,
        total: u32,
    ) -> Option<ObjectID> {
        // Calculate spawn position
        let position = self.calculate_spawn_position(
            index,
            total,
            object_def.disposition,
            object_def.offset,
        );

        let template = TheThingFactory::find_template(&object_def.object_name)?;
        let factory = TheThingFactory::get().ok()?;

        let mut spawn_team = None;
        if self.data.inherit_team {
            if let Some(team_id) = self.owner_team_id {
                spawn_team = get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|factory| factory.find_team_by_id(team_id));
            }
        }

        if spawn_team.is_none() && self.data.inherit_player {
            if let Some(player_id) = self.owner_player_id {
                spawn_team = ThePlayerList()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id).cloned())
                    .and_then(|player| player.read().ok().and_then(|guard| guard.get_default_team()));
            }
        }

        let created = if let Some(team_arc) = spawn_team.as_ref() {
            let team_guard = team_arc.read().ok()?;
            factory.new_object(template, &*team_guard).ok()?
        } else {
            factory.new_object_optional_team(template, None).ok()?
        };

        if let Ok(mut created_guard) = created.write() {
            let mut spawn_pos = position;
            if self.data.place_on_ground {
                if let Some(terrain) = TheTerrainLogic::get() {
                    spawn_pos[2] = terrain.get_ground_height(spawn_pos[0], spawn_pos[1], None);
                }
            }
            let _ = created_guard.set_position(&spawn_pos);

            if let Some(team_arc) = spawn_team {
                let _ = created_guard.set_team(Some(team_arc));
            }

            if let Some(velocity) = self.data.initial_velocity {
                if let Some(phys) = created_guard.get_physics_mut() {
                    if let Ok(mut phys_guard) = phys.lock() {
                        phys_guard.set_velocity(&velocity);
                    }
                }
            }
        }

        Some(created.read().ok()?.get_id())
    }

    /// Update OCL creation
    pub fn update(&mut self, current_frame: UnsignedInt) {
        self.current_frame = current_frame;

        match self.state {
            OCLState::Idle => {
                // Do nothing
            }
            OCLState::Creating => {
                // Check if it's time to create next object
                if current_frame >= self.next_creation_frame {
                    // Create objects from definition list
                    for object_def in &self.data.create_objects.clone() {
                        for i in 0..object_def.count {
                            if self.objects_created >= self.target_count {
                                self.state = OCLState::Complete;
                                return;
                            }

                            if let Some(id) = self.create_object(
                                object_def,
                                i,
                                object_def.count,
                            ) {
                                self.created_object_ids.push(id);
                                self.objects_created += 1;
                            }
                        }
                    }

                    // Schedule next creation
                    self.next_creation_frame = current_frame + self.data.creation_delay;

                    // Check if done
                    if self.objects_created >= self.target_count {
                        self.state = OCLState::Complete;
                    }
                }
            }
            OCLState::Complete => {
                // Creation finished, can reset or stay complete
            }
        }
    }

    /// Reset OCL for next use
    pub fn reset(&mut self) {
        self.state = OCLState::Idle;
        self.objects_created = 0;
        self.target_count = 0;
        self.created_object_ids.clear();
    }

    /// Get list of created object IDs
    pub fn get_created_objects(&self) -> &[ObjectID] {
        &self.created_object_ids
    }

    /// Is creation in progress?
    pub fn is_creating(&self) -> Bool {
        self.state == OCLState::Creating
    }

    /// Is creation complete?
    pub fn is_complete(&self) -> Bool {
        self.state == OCLState::Complete
    }

    /// Get number of objects created
    pub fn get_objects_created(&self) -> u32 {
        self.objects_created
    }
}

impl OCLUpdateInterface for OCLUpdate {
    fn reset_timer(&mut self) -> Result<(), GameError> {
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocl_creation() {
        let mut data = OCLUpdateModuleData::default();
        data.min_count = 5;
        data.max_count = 5;
        data.create_objects.push(OCLCreateObject::new("TestObject".to_string(), 5));

        let mut ocl = OCLUpdate::new(data);
        assert_eq!(ocl.state, OCLState::Idle);

        // Start creation
        ocl.start_creation([100.0, 100.0, 0.0], Some(1), Some(1));
        assert_eq!(ocl.state, OCLState::Creating);
        assert_eq!(ocl.target_count, 5);
    }

    #[test]
    fn test_parallel_pattern() {
        let data = OCLUpdateModuleData {
            min_count: 3,
            max_count: 3,
            spread_radius: 10.0,
            ..Default::default()
        };

        let ocl = OCLUpdate::new(data);

        let pos1 = ocl.calculate_spawn_position(
            0,
            3,
            OCLDisposition::DoParallelCreate,
            [0.0, 0.0, 0.0],
        );

        // Positions should be near center (with random spread)
        assert!((pos1[0] - 0.0).abs() <= 10.0);
        assert!((pos1[1] - 0.0).abs() <= 10.0);
    }

    #[test]
    fn test_line_pattern() {
        let data = OCLUpdateModuleData {
            pattern_radius: 100.0,
            ..Default::default()
        };

        let ocl = OCLUpdate::new(data);

        let pos1 = ocl.calculate_spawn_position(
            0,
            5,
            OCLDisposition::DoLineCreate,
            [0.0, 0.0, 0.0],
        );
        let pos5 = ocl.calculate_spawn_position(
            4,
            5,
            OCLDisposition::DoLineCreate,
            [0.0, 0.0, 0.0],
        );

        // First and last should be at opposite ends
        assert!((pos1[0] - (-100.0)).abs() < 1.0);
        assert!((pos5[0] - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_circle_pattern() {
        let data = OCLUpdateModuleData {
            pattern_radius: 50.0,
            pattern_count: 8,
            ..Default::default()
        };

        let ocl = OCLUpdate::new(data);

        let pos = ocl.calculate_spawn_position(
            0,
            8,
            OCLDisposition::DoCircleCreate,
            [0.0, 0.0, 0.0],
        );

        // Should be on circle
        let distance = (pos[0] * pos[0] + pos[1] * pos[1]).sqrt();
        assert!((distance - 50.0).abs() < 1.0);
    }
}
