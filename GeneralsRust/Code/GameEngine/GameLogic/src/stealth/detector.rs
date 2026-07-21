//! Stealth Detector Update Module
//!
//! Implements detection scanning for units that can detect stealthed units

use super::{DetectionLevel, StealthDifficulty};
use crate::common::*;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use log::{debug, trace};
use std::sync::{Arc, Mutex, RwLock};

/// Stealth detector configuration
#[derive(Debug, Clone)]
pub struct StealthDetectorUpdateModuleData {
    module_tag_name_key: NameKeyType,
    /// Detection range in world units
    detection_range: Real,
    /// Detection level
    detection_level: u32,
    /// Frames between detection scans
    scan_interval_frames: UnsignedInt,
    /// Can detect specific stealth levels (bitmask)
    can_detect_stealth_mask: u32,
    /// Detection bonus from upgrades
    detection_bonus_percent: Real,
    /// EVA event when detecting enemy stealth
    detection_eva_event: Option<String>,
}

impl Default for StealthDetectorUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            detection_range: 200.0,
            detection_level: DetectionLevel::Basic as u32,
            scan_interval_frames: 10,
            can_detect_stealth_mask: 0xFFFFFFFF,
            detection_bonus_percent: 0.0,
            detection_eva_event: None,
        }
    }
}

impl StealthDetectorUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DETECTOR_UPDATE_FIELDS)
    }

    pub fn set_detection_range(&mut self, range: Real) {
        self.detection_range = range;
    }

    pub fn set_scan_interval_frames(&mut self, frames: UnsignedInt) {
        self.scan_interval_frames = frames;
    }

    pub fn detection_range(&self) -> Real {
        self.detection_range * (1.0 + self.detection_bonus_percent)
    }

    pub fn scan_interval_frames(&self) -> UnsignedInt {
        self.scan_interval_frames
    }

    pub fn can_detect_mask(&self) -> u32 {
        self.can_detect_stealth_mask
    }
}

impl ModuleData for StealthDetectorUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthDetectorUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Detector runtime state
#[derive(Debug)]
pub struct StealthDetectorController {
    data: Arc<StealthDetectorUpdateModuleData>,
    object_id: ObjectID,
    scan_cooldown_frames: UnsignedInt,
    detected_objects: Vec<ObjectID>,
    is_active: bool,
}

impl StealthDetectorController {
    pub fn new(data: Arc<StealthDetectorUpdateModuleData>, object_id: ObjectID) -> Self {
        Self {
            data,
            object_id,
            scan_cooldown_frames: 0,
            detected_objects: Vec::new(),
            is_active: true,
        }
    }

    /// Perform a detection scan for nearby stealthed units
    pub fn scan_for_stealth(&mut self, _current_frame: u32) {
        if self.scan_cooldown_frames > 0 {
            self.scan_cooldown_frames = self.scan_cooldown_frames.saturating_sub(1);
            return;
        }

        // Get detector position
        let detector_pos = match self.get_detector_position() {
            Some(pos) => pos,
            None => return,
        };

        let detection_range = self.data.detection_range();
        let mut newly_detected = Vec::new();

        // Scan all objects in range
        // Host path: dual-world factory empty — no stealth residual to detect.
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        let all_objects = OBJECT_REGISTRY.get_all_objects();

        for obj_ref in all_objects {
            if let Ok(obj_guard) = obj_ref.read() {
                let target_id = obj_guard.get_id();

                // Skip self
                if target_id == self.object_id {
                    continue;
                }

                // Check if target is stealthed
                if !obj_guard.is_stealthed() {
                    continue;
                }

                // Check if enemy
                if !self.is_enemy(&*obj_guard) {
                    continue;
                }

                // Check range
                let distance = (*obj_guard.get_position() - detector_pos).length();
                if distance > detection_range {
                    continue;
                }

                // Check stealth difficulty vs detection capability
                if self.can_detect_target(&*obj_guard, distance) {
                    newly_detected.push(target_id);

                    // Mark target as detected
                    if let Some(stealth_module) = obj_guard.get_stealth_module() {
                        if let Ok(mut stealth_guard) = stealth_module.lock() {
                            stealth_guard.mark_as_detected();
                        }
                    }

                    trace!(
                        "Detector {} detected stealthed unit {} at range {}",
                        self.object_id,
                        target_id,
                        distance
                    );
                }
            }
        }

        // Update detected list
        self.detected_objects = newly_detected;

        // Reset scan cooldown
        self.scan_cooldown_frames = self.data.scan_interval_frames();
    }

    /// Check if this detector can detect a specific target
    fn can_detect_target(&self, target: &Object, distance: f32) -> bool {
        // Get target stealth difficulty
        let stealth_difficulty = self.get_target_stealth_difficulty(target);

        // Calculate effective detection range
        let effective_range =
            self.data.detection_range() * stealth_difficulty.get_detection_modifier();

        // Check if within effective range
        distance <= effective_range
    }

    /// Get stealth difficulty of a target
    fn get_target_stealth_difficulty(&self, target: &Object) -> StealthDifficulty {
        // Query target's stealth module for difficulty
        if let Some(stealth) = target.get_stealth() {
            if let Ok(guard) = stealth.lock() {
                // If the target has a high stealth level or specific upgrades, return Hard
                // For now, we use a simple heuristic: if stealth level > 0, consider it Hard
                // This matches the intent of checking stealth capability magnitude
                if guard.get_stealth_level() > 1 {
                    return StealthDifficulty::Hard;
                }
            }
        }
        StealthDifficulty::Normal
    }

    /// Get detector position
    fn get_detector_position(&self) -> Option<Coord3D> {
        OBJECT_REGISTRY
            .get_object(self.object_id)
            .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
    }

    /// Check if target is an enemy
    fn is_enemy(&self, target: &Object) -> bool {
        // Get both objects' team info
        let Some(detector_obj) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return false;
        };

        let Ok(detector_guard) = detector_obj.read() else {
            return false;
        };

        let detector_team_id = detector_guard.get_team_id();
        let target_team_id = target.get_team_id();
        detector_team_id != target_team_id
    }

    /// Get list of currently detected objects
    pub fn get_detected_objects(&self) -> &[ObjectID] {
        &self.detected_objects
    }

    /// Set detector active state
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        if !active {
            self.detected_objects.clear();
        }
    }

    /// Check if detector is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn scan_cooldown_frames(&self) -> UnsignedInt {
        self.scan_cooldown_frames
    }

    #[cfg(test)]
    pub(crate) fn set_scan_cooldown_frames_for_testing(&mut self, frames: UnsignedInt) {
        self.scan_cooldown_frames = frames;
    }
}

/// Detector update module
pub struct StealthDetectorUpdate {
    module_name_key: NameKeyType,
    data: Arc<StealthDetectorUpdateModuleData>,
    controller: Arc<Mutex<StealthDetectorController>>,
    object_id: ObjectID,
    current_frame: u32,
}

impl StealthDetectorUpdate {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthDetectorUpdateModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let controller = Arc::new(Mutex::new(StealthDetectorController::new(
            data.clone(),
            object_id,
        )));

        Self {
            module_name_key,
            data,
            controller,
            object_id,
            current_frame: 0,
        }
    }

    pub fn get_controller(&self) -> Arc<Mutex<StealthDetectorController>> {
        self.controller.clone()
    }
}

impl Module for StealthDetectorUpdate {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        debug!(
            "Stealth detector initialized for object {} with range {}",
            self.object_id,
            self.data.detection_range()
        );
    }
}

impl Snapshotable for StealthDetectorUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        xfer.xfer_object_id(&mut self.object_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.current_frame)
            .map_err(|e| e.to_string())?;

        let mut controller = self
            .controller
            .lock()
            .map_err(|_| "StealthDetectorUpdate: controller lock poisoned".to_string())?;

        xfer.xfer_unsigned_int(&mut controller.scan_cooldown_frames)
            .map_err(|e| e.to_string())?;

        let mut detected = if xfer.is_reading() {
            Vec::new()
        } else {
            controller.detected_objects.clone()
        };
        xfer.xfer_stl_object_id_list(&mut detected)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            controller.detected_objects = detected;
        }

        xfer.xfer_bool(&mut controller.is_active)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// INI parsing
fn parse_detection_range(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.detection_range = INI::parse_real(value)?;
    Ok(())
}

fn parse_scan_interval(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.scan_interval_frames = INI::parse_unsigned_int(value)?;
    Ok(())
}

const DETECTOR_UPDATE_FIELDS: &[FieldParse<StealthDetectorUpdateModuleData>] = &[
    FieldParse {
        token: "DetectionRange",
        parse: parse_detection_range,
    },
    FieldParse {
        token: "ScanInterval",
        parse: parse_scan_interval,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_creation() {
        let data = Arc::new(StealthDetectorUpdateModuleData::default());
        let controller = StealthDetectorController::new(data.clone(), 1);
        assert!(controller.is_active());
        assert_eq!(controller.get_detected_objects().len(), 0);
    }

    #[test]
    fn test_detection_range_with_bonus() {
        let mut data = StealthDetectorUpdateModuleData::default();
        data.detection_range = 200.0;
        data.detection_bonus_percent = 0.5;
        assert_eq!(data.detection_range(), 300.0);
    }

    #[test]
    fn test_scan_cooldown() {
        let data = Arc::new(StealthDetectorUpdateModuleData {
            scan_interval_frames: 10,
            ..Default::default()
        });
        let mut controller = StealthDetectorController::new(data, 1);

        controller.scan_cooldown_frames = 10;
        controller.scan_for_stealth(0);
        assert_eq!(controller.scan_cooldown_frames, 9);
    }
}
