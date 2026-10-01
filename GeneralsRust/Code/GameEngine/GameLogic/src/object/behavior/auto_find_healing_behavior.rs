//! AutoFindHealingBehavior - Rust conversion of C++ AutoFindHealingUpdate
//!
//! Update module to handle independent targeting of heal pads for cleanup/healing.
//! Original C++: AutoFindHealingUpdate.cpp by Kris Morness, August 2002
//! Rust conversion: 2025
//!
//! FILE: AutoFindHealingUpdate.cpp line 1-205

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{ObjectID, Bool, Int, KindOf, ModuleData, Real, UnsignedInt, FROM_CENTER_2D};
use crate::helpers::ThePartitionManager;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// Wave 450: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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
    object_id: ObjectID,
    module_data: Arc<AutoFindHealingUpdateModuleData>,
    /// Countdown to next scan. Matches C++ line 58
    next_scan_frames: Int,
}

impl AutoFindHealingUpdate {
    /// Creates a new AutoFindHealingUpdate. Matches C++ lines 56-59
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
        .downcast_ref::<AutoFindHealingUpdateModuleData>()
            .ok_or("Invalid module data for AutoFindHealingUpdate")?;

        Ok(Self {
            object_id: object_id,
            module_data: Arc::new(specific_data.clone()),
            next_scan_frames: 0, // Matches C++ line 58
        })
    }

    /// Scan for closest heal pad target. Matches C++ lines 127-161
    fn scan_closest_target(&self, me: &GameObject) -> Option<crate::common::ObjectID> {
        // Wave 450: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

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
        if self.object_id == crate::common::INVALID_ID {
            return 0;
        }
        let proceed = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |obj_read| {
            if let Some(player) = obj_read.get_controlling_player() {
                let human = crate::player::with_player(player, |p| p.is_human()).unwrap_or(true);
                if human {
                    return false;
                }
            }
            true
        });
        let Some(true) = proceed else {
            return 0;
        };
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return 0;
        }
        self.next_scan_frames = self.module_data.scan_frames as Int;
        let should_scan = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |obj_read| {
            if obj_read.get_ai_update_interface().is_none() {
                return false;
            }
            if let Some(body) = obj_read.get_body_module() {
                let health = body.get_health();
                let max_health = body.get_max_health();
                if health > max_health * self.module_data.never_heal {
                    return false;
                }
            }
            true
        });
        if should_scan != Some(true) {
            return 0;
        }
        let heal_id = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |obj_ref| {
            self.scan_closest_target(obj_ref)
        }).flatten();
        if let Some(heal_id) = heal_id {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, |owner| {
                owner.ai_pending_heal = Some(heal_id);
            });
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
        let _ = xfer;
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
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AutoFindHealingUpdate::new(object_id, module_data)?))
    }
}

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
