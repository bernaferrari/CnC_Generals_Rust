//! HeightDieUpdate - Objects that will die when they are a certain height above the terrain or objects
//! Author: Colin Day, April 2002 (C++ version)
//! Rust conversion: 2025
#![allow(unexpected_cfgs)]
//!
//! Ported from GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HeightDieUpdate.cpp

use crate::common::xfer::XferExt;

use crate::common::{
    Bool, Coord3D, KindOf, ModuleData, PathfindLayerEnum, Real, UnsignedInt, XferVersion,
};
use crate::helpers::TheGameLogic;
use crate::helpers::TheParticleSystemManager;
use crate::helpers::ThePartitionManager;
use crate::helpers::TheTerrainLogic;
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// Module data for HeightDieUpdate.
/// Matches C++ HeightDieUpdateModuleData fields from HeightDieUpdate.h
#[derive(Clone, Debug)]
pub struct HeightDieUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Die at this height above terrain
    pub target_height_above_terrain: Real,
    /// Target height considers terrain AND structure height underneath us
    pub target_height_includes_structures: Bool,
    /// Don't detonate unless moving in downward z dir
    pub only_when_moving_down: Bool,
    /// Destroy any attached particle system when below this height (HACK)
    pub destroy_attached_particles_at_height: Real,
    /// Snap to the ground when killed
    pub snap_to_ground_on_death: Bool,
    /// Don't explode before this time (frames)
    pub initial_delay: UnsignedInt,
}

impl Default for HeightDieUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            target_height_above_terrain: 0.0,
            target_height_includes_structures: false,
            only_when_moving_down: false,
            destroy_attached_particles_at_height: -1.0,
            snap_to_ground_on_death: false,
            initial_delay: 0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(HeightDieUpdateModuleData, base);

impl HeightDieUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HEIGHT_DIE_UPDATE_FIELDS)
    }
}

fn first_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_target_height(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.target_height_above_terrain = INI::parse_real(first_value(tokens)?)?;
    Ok(())
}

fn parse_target_height_includes_structures(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.target_height_includes_structures = INI::parse_bool(first_value(tokens)?)?;
    Ok(())
}

fn parse_only_when_moving_down(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.only_when_moving_down = INI::parse_bool(first_value(tokens)?)?;
    Ok(())
}

fn parse_destroy_attached_particles_at_height(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.destroy_attached_particles_at_height = INI::parse_real(first_value(tokens)?)?;
    Ok(())
}

fn parse_snap_to_ground_on_death(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.snap_to_ground_on_death = INI::parse_bool(first_value(tokens)?)?;
    Ok(())
}

fn parse_initial_delay(
    _ini: &mut INI,
    data: &mut HeightDieUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.initial_delay = INI::parse_duration_unsigned_int(first_value(tokens)?)?;
    Ok(())
}

const HEIGHT_DIE_UPDATE_FIELDS: &[FieldParse<HeightDieUpdateModuleData>] = &[
    FieldParse {
        token: "TargetHeight",
        parse: parse_target_height,
    },
    FieldParse {
        token: "TargetHeightIncludesStructures",
        parse: parse_target_height_includes_structures,
    },
    FieldParse {
        token: "OnlyWhenMovingDown",
        parse: parse_only_when_moving_down,
    },
    FieldParse {
        token: "DestroyAttachedParticlesAtHeight",
        parse: parse_destroy_attached_particles_at_height,
    },
    FieldParse {
        token: "SnapToGroundOnDeath",
        parse: parse_snap_to_ground_on_death,
    },
    FieldParse {
        token: "InitialDelay",
        parse: parse_initial_delay,
    },
];

/// HeightDieUpdate - kills the object when it falls below a threshold height above terrain.
/// Matches C++ HeightDieUpdate::update() at HeightDieUpdate.cpp:92-246
pub struct HeightDieUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<HeightDieUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    /// TRUE once we have triggered death. C++ m_hasDied.
    has_died: Bool,
    /// TRUE once we destroy attached systems (so we do it only once). C++ m_particlesDestroyed.
    particles_destroyed: Bool,
    /// We record our last position for logic that needs to know our direction of travel. C++ m_lastPosition.
    last_position: Coord3D,
    /// Earliest we are allowed to think about dying. C++ m_earliestDeathFrame.
    earliest_death_frame: UnsignedInt,
}

impl HeightDieUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HeightDieUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            // Matches C++ HeightDieUpdate.cpp:73-78
            has_died: false,
            particles_destroyed: false,
            last_position: Coord3D {
                x: -1.0,
                y: -1.0,
                z: -1.0,
            },
            // C++ v2: initialized to UINT_MAX, set on first update
            earliest_death_frame: u32::MAX,
        })
    }
}

impl UpdateModuleInterface for HeightDieUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let obj_arc = match self.object.upgrade() {
            Some(arc) => arc,
            None => return UpdateSleepTime::Forever,
        };

        // Matches C++ HeightDieUpdate.cpp:94-96
        // Initialize earliest death frame on first call
        if self.earliest_death_frame == u32::MAX {
            let current_frame = TheGameLogic::get_frame();
            self.earliest_death_frame =
                current_frame.saturating_add(self.module_data.initial_delay);
        }

        // If at least a one frame delay has been set, then stop for a while
        // Matches C++ HeightDieUpdate.cpp:99-100
        let current_frame = TheGameLogic::get_frame();
        if self.earliest_death_frame > current_frame {
            return UPDATE_SLEEP_NONE;
        }

        // Do nothing if we're contained within other objects ... like a transport
        // Matches C++ HeightDieUpdate.cpp:103-112
        let me = match obj_arc.read() {
            Ok(guard) => guard,
            Err(_) => return UPDATE_SLEEP_NONE,
        };

        if me.get_contained_by().is_some() {
            // Keep track of our last position even though we're not doing anything yet
            self.last_position = *me.get_position();
            return UPDATE_SLEEP_NONE;
        }

        // Get the module data
        let d = &self.module_data;

        // Get our current position. C++ line 118
        let pos = *me.get_position();

        // Drop read lock before potentially taking write lock
        drop(me);

        let mut direction_ok = true;
        if !self.has_died {
            // Matches C++ HeightDieUpdate.cpp:124-130
            if d.only_when_moving_down {
                if pos.z >= self.last_position.z {
                    direction_ok = false;
                }
            }

            // Get the terrain height. C++ line 133
            let mut terrain_height_at_pos = 0.0;
            if let Some(terrain) = TheTerrainLogic::get() {
                terrain_height_at_pos = terrain.get_ground_height(pos.x, pos.y, None);
            }

            // If including structures, check for bridges and buildings
            // Matches C++ HeightDieUpdate.cpp:136-145
            if d.target_height_includes_structures {
                if let Some(terrain) = TheTerrainLogic::get() {
                    let layer = terrain.get_highest_layer_for_destination(&pos);
                    if layer != PathfindLayerEnum::Ground {
                        // LAYER_GROUND = 0
                        let layer_height = terrain.get_layer_height(pos.x, pos.y, layer);
                        if layer_height > terrain_height_at_pos {
                            terrain_height_at_pos = layer_height;
                        }
                    }
                }
            }

            // Our target height to die at is by default the height specified in the INI
            // entry above the terrain. C++ lines 148-152
            let mut target_height = terrain_height_at_pos + d.target_height_above_terrain;

            // If we consider objects under us, find the tallest structure in range
            // Matches C++ HeightDieUpdate.cpp:158-197
            if d.target_height_includes_structures {
                // Scan all objects in the radius of our extent and find the tallest height
                let me_ref = match obj_arc.read() {
                    Ok(guard) => guard,
                    Err(_) => return UPDATE_SLEEP_NONE,
                };

                let range = me_ref.get_geometry_info().get_bounding_circle_radius();
                let my_id = me_ref.id();

                // Find tallest structure height
                let mut tallest_height: Real = 0.0;

                if let Some(partition) = ThePartitionManager::get() {
                    // In C++, iterateObjectsInRange is used with a KINDOF_STRUCTURE filter
                    // to find ALL structures in range, then we pick the tallest.
                    let candidates = partition.get_objects_in_range(&pos, range);

                    for obj_id in candidates {
                        // Ignore ourselves. C++ line 178-179
                        if obj_id == my_id {
                            continue;
                        }
                        if let Some(this_height) = crate::object::registry::OBJECT_REGISTRY
                            .with_object(obj_id, |structure| {
                                if structure.is_kind_of(KindOf::Structure) {
                                    Some(
                                        structure
                                            .get_geometry_info()
                                            .get_max_height_above_position(),
                                    )
                                } else {
                                    None
                                }
                            })
                            .flatten()
                        {
                            if this_height > tallest_height {
                                tallest_height = this_height;
                            }
                        }
                    }
                }

                // C++ lines 194-195
                if tallest_height > d.target_height_above_terrain {
                    target_height = tallest_height + terrain_height_at_pos;
                }
            }

            // If we are below the target height ... DIE!
            // Matches C++ HeightDieUpdate.cpp:200-222
            if pos.z < target_height && direction_ok {
                // If we're supposed to snap us to the ground on death do so
                // AND: even if we're not snapping to ground, be sure we don't go BELOW ground
                if d.snap_to_ground_on_death || pos.z < terrain_height_at_pos {
                    let ground = Coord3D {
                        x: pos.x,
                        y: pos.y,
                        z: terrain_height_at_pos,
                    };
                    if let Ok(mut obj_write) = obj_arc.write() {
                        let _ = obj_write.set_position(&ground);
                    }
                }

                // Kill the object. C++ line 217
                if let Ok(mut obj_write) = obj_arc.write() {
                    obj_write.kill(None, None);
                }

                // We have died ... don't do this again. C++ line 220
                self.has_died = true;
            }
        }

        // If our height is below the destroy attached particles height above the terrain, clean them up
        // Matches C++ HeightDieUpdate.cpp:230-239
        if !self.particles_destroyed
            && pos.z < d.destroy_attached_particles_at_height
            && (self.has_died || direction_ok)
        {
            // C++ HeightDieUpdate.cpp:234 — TheParticleSystemManager->destroyAttachedSystems(getObject())
            if let Some(obj_guard) = obj_arc.read().ok() {
                let obj_id = obj_guard.id();
                drop(obj_guard);
                if let Some(ps_manager) = TheParticleSystemManager::get() {
                    ps_manager.destroy_attached_systems(obj_id);
                }
            }

            // Don't do this again. C++ line 237
            self.particles_destroyed = true;
        }

        // Save our current position as the last position we monitored. C++ line 242
        self.last_position = pos;

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for HeightDieUpdate {
    fn get_module_name(&self) -> &'static str {
        "HeightDieUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for HeightDieUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Matches C++ HeightDieUpdate::xfer() at HeightDieUpdate.cpp:266-291
    /// Version 2 includes m_earliestDeathFrame
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("HeightDieUpdate xfer version failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        // has died. C++ line 278
        xfer.xfer_bool(&mut self.has_died)
            .map_err(|e| format!("HeightDieUpdate xfer has_died: {:?}", e))?;

        // particles destroyed. C++ line 281
        xfer.xfer_bool(&mut self.particles_destroyed)
            .map_err(|e| format!("HeightDieUpdate xfer particles_destroyed: {:?}", e))?;

        // last position. C++ line 284
        xfer.xfer_coord3d(&mut self.last_position);

        // earliest death frame (version >= 2). C++ line 286-289
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.earliest_death_frame)
                .map_err(|e| format!("HeightDieUpdate xfer earliest_death_frame: {:?}", e))?;
        } else {
            self.earliest_death_frame = 0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct HeightDieUpdateFactory;
impl HeightDieUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HeightDieUpdate::new(thing, module_data)?))
    }
}
