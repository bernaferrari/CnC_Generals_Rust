//! CheckpointUpdate - Rust conversion of C++ CheckpointUpdate
//!
//! Opens gates when allies are near and no enemies are within range.
//! Controls door model conditions and adjusts geometry for pathfinding.
//! Author: Matthew D. Campbell / Mark Lorenzen (C++ version)
//! Rust conversion: 2025

use crate::ai::THE_AI;
use crate::common::{GeometryInfo, ModelConditionFlag, ModuleData, Real};
use crate::helpers::get_game_logic_random_value;
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::drawable::DrawableArcExt;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// Frames per second constant (C++: LOGICFRAMES_PER_SECOND)
const LOGICFRAMES_PER_SECOND: u32 = 30;

#[derive(Clone, Debug)]
pub struct CheckpointUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Delay between enemy/ally scans in frames
    pub enemy_scan_delay_time: u32,
}

impl Default for CheckpointUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            enemy_scan_delay_time: LOGICFRAMES_PER_SECOND,
        }
    }
}

crate::impl_behavior_module_data_via_base!(CheckpointUpdateModuleData, base);

/// CheckpointUpdate module - Opens gates when allies nearby, closes when enemies near
pub struct CheckpointUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<CheckpointUpdateModuleData>,

    /// Is an enemy currently near?
    enemy_near: bool,
    /// Is an ally currently near?
    ally_near: bool,
    /// Maximum bounding radius for geometry (saved at creation)
    max_bounding_radius: Real,
    /// Countdown until next scan
    enemy_scan_delay: u32,
}

impl CheckpointUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<CheckpointUpdateModuleData>()
            .ok_or("Invalid module data")?;

        // Get max bounding radius from object geometry
        let max_bounding_radius = {
            let obj = object.read().map_err(|_| "Failed to read object")?;
            obj.get_geometry_info().get_minor_radius()
        };

        // Bias with random delay so all checkpoints don't spike at once
        let random_delay =
            get_game_logic_random_value(0, specific_data.enemy_scan_delay_time as i32) as u32;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            enemy_near: false,
            ally_near: false,
            max_bounding_radius,
            enemy_scan_delay: random_delay,
        })
    }

    fn set_geometry_minor_radius(geom: &mut GeometryInfo, new_radius: Real) {
        let center_x = (geom.bounds.min.x + geom.bounds.max.x) * 0.5;
        let center_y = (geom.bounds.min.y + geom.bounds.max.y) * 0.5;
        let half_x = (geom.bounds.max.x - geom.bounds.min.x).abs() * 0.5;
        let half_y = (geom.bounds.max.y - geom.bounds.min.y).abs() * 0.5;
        let radius = new_radius.max(0.0);

        if half_x <= half_y {
            geom.bounds.min.x = center_x - radius;
            geom.bounds.max.x = center_x + radius;
        } else {
            geom.bounds.min.y = center_y - radius;
            geom.bounds.max.y = center_y + radius;
        }
    }

    /// Check for nearby allies and enemies
    fn check_for_allies_and_enemies(&mut self) {
        // Always scan (C++ has `|| TRUE` which makes the delay check always pass)
        self.enemy_scan_delay = self.module_data.enemy_scan_delay_time;

        let Some(obj_arc) = self.object.upgrade() else {
            self.enemy_near = false;
            self.ally_near = false;
            return;
        };

        let (obj_id, vision_range, mut geometry) = {
            let obj = match obj_arc.read() {
                Ok(guard) => guard,
                Err(_) => {
                    self.enemy_near = false;
                    self.ally_near = false;
                    return;
                }
            };
            (
                obj.get_id(),
                obj.get_vision_range(),
                obj.get_geometry_info().clone(),
            )
        };

        let restore_radius = geometry.get_minor_radius();
        let mut scan_geometry = geometry.clone();
        Self::set_geometry_minor_radius(&mut scan_geometry, self.max_bounding_radius);

        {
            let Ok(mut obj) = obj_arc.write() else {
                self.enemy_near = false;
                self.ally_near = false;
                return;
            };
            obj.set_geometry_info(scan_geometry);
        }

        let enemy = THE_AI.read().ok().and_then(|ai| {
            ai.find_closest_enemy(obj_id, vision_range, 0, None, None)
                .ok()
                .flatten()
        });
        let ally = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.find_closest_ally(obj_id, vision_range, 0).ok().flatten());

        self.enemy_near = enemy.is_some();
        self.ally_near = ally.is_some();

        Self::set_geometry_minor_radius(&mut geometry, restore_radius);
        {
            let Ok(mut obj) = obj_arc.write() else {
                return;
            };
            obj.set_geometry_info(geometry);
        }
    }
}

impl UpdateModuleInterface for CheckpointUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let was_an_ally = self.ally_near;
        let was_an_enemy = self.enemy_near;

        self.check_for_allies_and_enemies();

        let change = (was_an_ally != self.ally_near) || (was_an_enemy != self.enemy_near);
        let open = !self.enemy_near && self.ally_near;

        let me_arc = match self.object.upgrade() {
            Some(arc) => arc,
            None => return UPDATE_SLEEP_NONE,
        };

        let mut me = match me_arc.write() {
            Ok(guard) => guard,
            Err(_) => return UPDATE_SLEEP_NONE,
        };

        if let Some(draw) = me.get_drawable() {
            if change {
                if open {
                    // Open the gate: clear CLOSING, set OPENING
                    draw.clear_and_set_model_condition_state(
                        ModelConditionFlag::Door1Closing,
                        ModelConditionFlag::Door1Opening,
                    );
                } else {
                    // Close the gate: clear OPENING, set CLOSING
                    draw.clear_and_set_model_condition_state(
                        ModelConditionFlag::Door1Opening,
                        ModelConditionFlag::Door1Closing,
                    );
                }
            }

            // Adjust radius for pathfinding based on door animation state.
            let mut geom = me.get_geometry_info().clone();
            let radius = geom.get_minor_radius();
            let mut new_radius = radius;

            if open {
                if radius > 0.0 {
                    new_radius = (radius - 0.333).max(0.0);
                }
            } else if radius < self.max_bounding_radius {
                new_radius = (radius + 0.333).min(self.max_bounding_radius);
            }

            if (new_radius - radius).abs() > f32::EPSILON {
                Self::set_geometry_minor_radius(&mut geom, new_radius);
                me.set_geometry_info(geom);
            }
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for CheckpointUpdate {
    fn get_module_name(&self) -> &'static str {
        "CheckpointUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for CheckpointUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_bool(&mut self.enemy_near)
            .map_err(|e| format!("CheckpointUpdate xfer enemy_near: {:?}", e))?;
        xfer.xfer_bool(&mut self.ally_near)
            .map_err(|e| format!("CheckpointUpdate xfer ally_near: {:?}", e))?;
        xfer.xfer_real(&mut self.max_bounding_radius)
            .map_err(|e| format!("CheckpointUpdate xfer max_bounding_radius: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.enemy_scan_delay)
            .map_err(|e| format!("CheckpointUpdate xfer enemy_scan_delay: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct CheckpointUpdateFactory;
impl CheckpointUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(CheckpointUpdate::new(thing, module_data)?))
    }
}
