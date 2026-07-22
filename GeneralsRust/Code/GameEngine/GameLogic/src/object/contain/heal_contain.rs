//! Heal Contain Module
//!
//! Objects that are contained inside a heal contain get healed over time.
//! This is used for medical facilities and healing structures.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, OpenContain};
use crate::common::{GameResult, ObjectID, PlayerMaskType};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, ExitDoorType, UpdateSleepTime};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Configuration data for HealContain module
#[derive(Debug, Clone)]
pub struct HealContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Time in frames for something to become fully healed
    pub frames_for_full_heal: u32,
}

impl Default for HealContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            frames_for_full_heal: 0,
        }
    }
}

impl HealContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, HEAL_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, HEAL_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for HealContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        HealContainModuleData::parse_from_config(self, config)
    }
}

fn parse_time_for_full_heal(
    _ini: &mut INI,
    data: &mut HealContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.frames_for_full_heal = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const HEAL_CONTAIN_FIELDS: &[FieldParse<HealContainModuleData>] = &[FieldParse {
    token: "TimeForFullHeal",
    parse: parse_time_for_full_heal,
}];

/// Heal contain module - heals contained units over time
#[derive(Debug)]
pub struct HealContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Reference to the owning object
    object_id: ObjectID,
    module_data: HealContainModuleData,
}

impl HealContain {
    /// Create a new HealContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &HealContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;
        Ok(Self {
            base,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
            module_data: module_data.clone(),
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
    }

    /// Update method called once per frame
    pub fn update(&mut self, module_data: &HealContainModuleData) -> GameResult<UpdateSleepTime> {
        // Extend base functionality
        self.base.update()?;

        // Get contained objects list (need to collect to avoid borrow issues)
        let contained_objects: Vec<_> = self
            .base
            .get_contained_items_list()?
            .iter()
            .cloned()
            .collect();

        // Process each contained object for healing
        for obj in contained_objects {
            let patient_id = obj
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            let done_healing = self.do_heal(patient_id, module_data.frames_for_full_heal)?;

            if done_healing {
                // Reserve door for exit
                let obj_clone = obj.clone();
                if let Ok(object) = obj.read() {
                    if let Ok(exit_door) = self
                        .base
                        .reserve_door_for_exit(&super::open_contain::ObjectTemplate {}, &object)
                    {
                        if exit_door != ExitDoorType::NoneAvailable {
                            drop(object); // Release lock before calling exit
                            self.base.exit_object_via_door(
                                obj_clone.read().map(|g| g.get_id()).unwrap_or(0),
                                exit_door,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(UpdateSleepTime::None)
    }

    /// Check if this is a heal container
    pub fn is_heal_contain(&self) -> bool {
        true // This container only contains units while healing (not a transport!)
    }

    /// Check if this is a tunnel container
    pub fn is_tunnel_contain(&self) -> bool {
        false
    }

    /// Perform healing on a single object for a single frame
    pub fn do_heal(&mut self, obj_id: ObjectID, frames_for_full_heal: u32) -> GameResult<bool> {
        let mut done_healing = false;

        let obj = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("HealContain patient not found")?;

        // Setup healing damage info structure
        let source_id = if self.object_id == crate::common::INVALID_ID {
            0
        } else {
            self.object_id
        };

        let mut heal_info = DamageInfo::new();
        heal_info.input.damage_type = DamageType::Healing;
        heal_info.input.death_type = DeathType::None;
        heal_info.input.source_id = source_id;
        heal_info.input.amount = 0.0;
        heal_info.sync_from_input();

        // Get current frame and contained frame
        let current_frame = self.get_current_frame();

        if let Ok(object) = obj.read() {
            let contained_by_frame = object.get_contained_by_frame();

            // Get body module for healing
            if let Some(body) = object.get_body_module() {
                if let Ok(body_module) = body.lock() {
                    let max_health = body_module.get_max_health();

                    // C++ compares elapsed logic frames:
                    // TheGameLogic->getFrame() - obj->getContainedByFrame()
                    let frames_contained = current_frame.saturating_sub(contained_by_frame);
                    if frames_contained >= frames_for_full_heal {
                        // Set amount to max health to ensure full healing
                        heal_info.input.amount = max_health;
                        heal_info.sync_from_input();

                        // Apply full healing
                        drop(body_module); // Release lock before mutable operation
                        if let Ok(mut body_mut) = body.lock() {
                            body_mut.attempt_healing(&mut heal_info)?;
                        }

                        done_healing = true;
                    } else {
                        // Give incremental healing over time
                        // Calculate healing amount as if object started at 0 health
                        // and would be fully healed at frames_for_full_heal
                        heal_info.input.amount = max_health / frames_for_full_heal as f32;
                        heal_info.sync_from_input();

                        // Apply incremental healing
                        drop(body_module); // Release lock before mutable operation
                        if let Ok(mut body_mut) = body.lock() {
                            body_mut.attempt_healing(&mut heal_info)?;
                        }
                    }
                }
            }
        }

        Ok(done_healing)
    }

    /// Get current frame from game logic singleton (C++ parity path).
    fn get_current_frame(&self) -> u32 {
        TheGameLogic::get_frame()
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save base state
        let base_state = self.base.save_state()?;
        for (key, value) in base_state {
            state.insert(format!("base_{}", key), value);
        }

        // HealContain doesn't have additional state beyond base class

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        // Extract base state
        let mut base_state = HashMap::new();
        for (key, value) in state {
            if let Some(base_key) = key.strip_prefix("base_") {
                base_state.insert(base_key.to_string(), value.clone());
            }
        }

        // Load base state
        self.base.load_state(&base_state)?;

        Ok(())
    }

    /// Perform CRC calculation for network synchronization
    pub fn calculate_crc(&self) -> u32 {
        // Implementation would calculate CRC of relevant state
        // For now, delegate to base class
        self.base.calculate_crc()
    }

    /// Post-process after loading from save file
    pub fn load_post_process(&mut self) -> GameResult<()> {
        // Extend base class post-processing
        self.base.load_post_process()?;

        // HealContain doesn't need additional post-processing

        Ok(())
    }
}

impl Snapshotable for HealContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        Snapshotable::xfer(&mut self.base, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for HealContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.base.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.base
            .add_to_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
            )
            .map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.base
            .remove_from_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
                false,
            )
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let module_data = self.module_data.clone();
        HealContain::update(self, &module_data).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_heal_contain(&self) -> bool {
        Self::is_heal_contain(self)
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.base.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }
}

impl ContainerInterface for HealContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.base.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
        let max = match self.base.get_contain_max() {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};
    use game_engine::common::system::{XferBlockSize, XferMode, XferStatus};
    use std::io;
    use std::sync::Mutex;

    struct RecordingXfer {
        bytes: Vec<u8>,
    }

    impl RecordingXfer {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }
    }

    impl Xfer for RecordingXfer {
        fn get_xfer_mode(&self) -> XferMode {
            XferMode::Save
        }

        fn get_identifier(&self) -> &str {
            "heal-contain-test"
        }

        fn set_options(&mut self, _options: u32) {}

        fn clear_options(&mut self, _options: u32) {}

        fn get_options(&self) -> u32 {
            0
        }

        fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
            Ok(0)
        }

        fn end_block(&mut self) -> Result<(), XferStatus> {
            Ok(())
        }

        fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_snapshot(
            &mut self,
            _snapshot: &mut game_engine::system::Snapshot,
        ) -> Result<(), XferStatus> {
            Ok(())
        }

        fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
            Ok(())
        }

        unsafe fn xfer_implementation(
            &mut self,
            data: *mut u8,
            data_size: usize,
        ) -> io::Result<()> {
            let bytes = unsafe { std::slice::from_raw_parts(data, data_size) };
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
    }

    fn test_object(name: &str, id: ObjectID) -> Arc<RwLock<Object>> {
        Object::new_with_id(
            Arc::new(DefaultThingTemplate::new(name.to_string())),
            id,
            ObjectStatusMaskType::none(),
            None,
        )
        .expect("test object")
    }

    fn attach_active_body(obj: &Arc<RwLock<Object>>, max_health: f32, initial_health: f32) {
        let id = obj.read().expect("object read").get_id();
        let body = ActiveBody::new_with_owner(
            ActiveBodyModuleData {
                max_health,
                initial_health,
                ..Default::default()
            },
            id,
        );
        obj.write()
            .expect("object write")
            .set_body_module(Some(Arc::new(Mutex::new(body))));
    }

    #[test]
    fn test_heal_contain_creation() {
        let module_data = HealContainModuleData {
            frames_for_full_heal: 100,
            ..Default::default()
        };

        // Test would create objects and verify healing functionality
        assert_eq!(module_data.frames_for_full_heal, 100);
    }

    #[test]
    fn test_heal_contain_properties() {
        let module_data = HealContainModuleData::default();

        // Create weak reference for deferred object wiring.
        // In real implementation, this would be a proper object reference

        // Test basic properties
        assert_eq!(module_data.frames_for_full_heal, 0);
    }

    #[test]
    fn test_healing_calculation() {
        // Test healing amount calculation
        let max_health = 100.0;
        let frames_for_full_heal = 50;
        let heal_per_frame = max_health / frames_for_full_heal as f32;

        assert_eq!(heal_per_frame, 2.0);
    }

    #[test]
    fn trait_snapshot_xfer_writes_heal_version_and_open_contain_state_like_cpp() {
        let mut contain =
            HealContain::new(Weak::new(), &HealContainModuleData::default()).expect("heal contain");

        let mut xfer = RecordingXfer::new();
        ContainModuleInterface::snapshot_xfer(&mut contain, &mut xfer).expect("heal snapshot xfer");

        assert_eq!(xfer.bytes[0], 1, "HealContain xfer version");
        assert_eq!(xfer.bytes[1], 2, "delegated OpenContain xfer version");
        assert!(
            xfer.bytes.len() > 1,
            "trait snapshot hook must not fall back to no-op"
        );
    }

    #[test]
    fn zero_frame_full_heal_matches_cpp_default_path() {
        let owner = test_object("HealOwner", 10_100);
        let patient = test_object("HealPatient", 10_101);
        attach_active_body(&patient, 100.0, 25.0);

        let mut contain =
            HealContain::new(Arc::downgrade(&owner), &HealContainModuleData::default())
                .expect("heal contain");

        assert!(
            contain
                .do_heal(
                    patient
                        .read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                    0,
                )
                .expect("heal succeeds"),
            "TimeForFullHeal=0 should immediately finish healing"
        );

        let body = patient
            .read()
            .expect("patient read")
            .get_body_module()
            .expect("body module");
        assert_eq!(body.lock().expect("body lock").get_health(), 100.0);
    }

    #[test]
    fn incremental_heal_uses_max_health_divided_by_full_heal_frames() {
        let owner = test_object("HealOwner", 10_200);
        let patient = test_object("HealPatient", 10_201);
        attach_active_body(&patient, 100.0, 25.0);

        let mut contain =
            HealContain::new(Arc::downgrade(&owner), &HealContainModuleData::default())
                .expect("heal contain");

        assert!(
            !contain
                .do_heal(
                    patient
                        .read()
                        .ok()
                        .map(|g| g.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                    50,
                )
                .expect("heal succeeds"),
            "patient should remain contained until the full-heal frame"
        );

        let body = patient
            .read()
            .expect("patient read")
            .get_body_module()
            .expect("body module");
        assert_eq!(body.lock().expect("body lock").get_health(), 27.0);
    }
}
