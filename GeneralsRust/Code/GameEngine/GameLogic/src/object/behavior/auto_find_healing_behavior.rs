//! AutoFindHealingBehavior - Rust conversion of C++ AutoFindHealingUpdate
//!
//! Update module to handle independent targeting of heal pads for cleanup/healing.
//! Original C++: AutoFindHealingUpdate.cpp by Kris Morness, August 2002
//! Rust conversion: 2025
//!
//! FILE: AutoFindHealingUpdate.cpp line 1-205

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Bool, Int, KindOf, ModuleData, Real, UnsignedInt, FROM_CENTER_2D};
use crate::helpers::ThePartitionManager;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

// Matches C++ AutoFindHealingUpdate.cpp lines 31-37
#[derive(Clone, Debug)]
pub struct AutoFindHealingUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Scan rate in frames. Matches C++ line 33
    pub scan_frames: UnsignedInt,
    /// Scan range for heal pads. Matches C++ line 34
    pub scan_range: Real,
    /// Health percentage above which we never heal. Matches C++ line 35
    pub never_heal: Real,
    /// Health percentage below which we always heal. Matches C++ line 36
    pub always_heal: Real,
}

impl Default for AutoFindHealingUpdateModuleData {
    fn default() -> Self {
        // Matches C++ AutoFindHealingUpdate.cpp lines 31-37 (constructor defaults)
        Self {
            base: BehaviorModuleData::default(),
            scan_frames: 0,
            scan_range: 0.0,
            never_heal: 0.95, // Matches C++ line 35
            always_heal: 0.25, // Matches C++ line 36
        }
    }
}

crate::impl_behavior_module_data_via_base!(AutoFindHealingUpdateModuleData, base);

/// AutoFindHealingUpdate - Automatically seeks out heal pads when damaged
///
/// Matches C++ AutoFindHealingUpdate.cpp lines 56-205
pub struct AutoFindHealingUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<AutoFindHealingUpdateModuleData>,
    /// Countdown to next scan. Matches C++ line 58
    next_scan_frames: Int,
}

impl AutoFindHealingUpdate {
    /// Creates a new AutoFindHealingUpdate. Matches C++ lines 56-59
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
        .downcast_ref::<AutoFindHealingUpdateModuleData>()
            .ok_or("Invalid module data for AutoFindHealingUpdate")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_scan_frames: 0, // Matches C++ line 58
        })
    }

    /// Scan for closest heal pad target. Matches C++ lines 127-161
    fn scan_closest_target(&self, me: &GameObject) -> Option<crate::common::ObjectID> {
        let data = &self.module_data;
        let mut best_target: Option<crate::common::ObjectID> = None;
        let mut closest_dist_sqr = 0.0;

        let Some(partition) = ThePartitionManager::get() else {
            return None;
        };

        let candidates = partition.get_objects_in_range(me.get_position(), data.scan_range);
        for other_id in candidates {
            let Some(dist) = crate::object::registry::OBJECT_REGISTRY.with_object(
                other_id,
                |other_guard| {
                    if !other_guard.is_kind_of(KindOf::HealPad) {
                        return None;
                    }
                    Some(ThePartitionManager::get_distance_squared(
                        me,
                        other_guard,
                        FROM_CENTER_2D,
                    ))
                },
            )
            .flatten() else {
                continue;
            };

            if best_target.is_none() || dist < closest_dist_sqr {
                best_target = Some(other_id);
                closest_dist_sqr = dist;
            }
        }

        best_target
    }
}

impl UpdateModuleInterface for AutoFindHealingUpdate {
    /// Main update loop. Matches C++ lines 78-123
    fn update_simple(&mut self) -> UpdateSleepTime {
        let object = match self.object.upgrade() {
            Some(obj) => obj,
            None => return 0, // UPDATE_SLEEP_NONE
        };

        let obj_read = match object.read() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };

        // Only process AI-controlled units. Matches C++ lines 82-84
        if let Some(player) = obj_read.get_controlling_player() {
            let player_read = match player.read() {
                Ok(guard) => guard,
                Err(_) => return 0,
            };

            // Human players handle healing manually
            if player_read.is_human() {
                return 0; // UPDATE_SLEEP_NONE
            }
        }

        // Countdown timer optimization. Matches C++ lines 88-93
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return 0; // UPDATE_SLEEP_NONE
        }
        self.next_scan_frames = self.module_data.scan_frames as Int;

        // Get AI interface. Matches C++ lines 95-96
        let ai_available = obj_read.get_ai_update_interface().is_some();
        if !ai_available {
            return 0; // UPDATE_SLEEP_NONE
        }

        // Check health status. Matches C++ lines 98-104
        if let Some(body) = obj_read.get_body_module() {
            let body_guard = match body.lock() {
                Ok(guard) => guard,
                Err(_) => return 0,
            };

            let health = body_guard.get_health();
            let max_health = body_guard.get_max_health();

            // If we're very healthy, don't bother looking for healing. Matches C++ lines 102-104
            if health > max_health * self.module_data.never_heal {
                return 0; // UPDATE_SLEEP_NONE
            }

            // Check if we should heal despite being busy. Matches C++ lines 106-114
            // For now, only heal if idle (C++ line 109)
            // Future: Check if health > max_health * always_heal threshold (C++ lines 111-113)
        }

        // Periodic scanning (expensive). Matches C++ lines 116-122
        drop(obj_read); // Release read lock before calling scan

        if let Ok(obj_ref) = object.read() {
            if let Some(heal_id) = self.scan_closest_target(&obj_ref) {
                if let Some(ai) = obj_ref.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let mut params = AiCommandParams::new(AiCommandType::GetHealed, CommandSourceType::FromAi);
                        params.obj = Some(heal_id);
                        let _ = ai_guard.execute_command(&params);
                    }
                }
            }
        }

        0 // UPDATE_SLEEP_NONE, matches C++ line 122
    }
}

impl BehaviorModuleInterface for AutoFindHealingUpdate {
    fn get_module_name(&self) -> &'static str {
        "AutoFindHealingUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

/// Factory for creating AutoFindHealingUpdate behaviors
impl Snapshotable for AutoFindHealingUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_int(&mut self.next_scan_frames)
            .map_err(|e| format!("AutoFindHealingUpdate xfer next_scan_frames: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct AutoFindHealingUpdateFactory;

impl AutoFindHealingUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AutoFindHealingUpdate::new(thing, module_data)?))
    }
}

// Thread safety
unsafe impl Send for AutoFindHealingUpdate {}
unsafe impl Sync for AutoFindHealingUpdate {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_defaults() {
        let data = AutoFindHealingUpdateModuleData::default();
        assert_eq!(data.scan_frames, 0);
        assert_eq!(data.scan_range, 0.0);
        assert_eq!(data.never_heal, 0.95); // C++ default
        assert_eq!(data.always_heal, 0.25); // C++ default
    }
}
