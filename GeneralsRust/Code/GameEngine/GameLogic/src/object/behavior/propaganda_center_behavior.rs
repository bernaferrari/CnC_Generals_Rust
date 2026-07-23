#![allow(unexpected_cfgs)]

//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaCenterBehavior.cpp`.
//!
//! Propaganda Center Behavior Module
//!
//! Extends prison behavior with brainwashing logic for contained units.

#[cfg(feature = "allow_surrender")]
use std::any::Any;
#[cfg(feature = "allow_surrender")]
use std::sync::{Arc, Mutex, RwLock, Weak};

#[cfg(feature = "allow_surrender")]
use game_engine::common::ini::{FieldParse, INIError, INI};
#[cfg(feature = "allow_surrender")]
use game_engine::common::name_key_generator::NameKeyGenerator;
#[cfg(feature = "allow_surrender")]
use game_engine::common::rts::AsciiString;
#[cfg(feature = "allow_surrender")]
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
#[cfg(feature = "allow_surrender")]
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

#[cfg(feature = "allow_surrender")]
use crate::common::xfer::XferExt;
#[cfg(feature = "allow_surrender")]
use crate::common::{ObjectID, UnsignedInt, INVALID_ID};
#[cfg(feature = "allow_surrender")]
use crate::helpers::TheGameLogic;
#[cfg(feature = "allow_surrender")]
use crate::modules::{
    BehaviorModuleInterface, ContainModuleInterface, ContainWant, ExitDoorType,
    UpdateModuleInterface, UpdateSleepTime,
};
#[cfg(feature = "allow_surrender")]
use crate::object::behavior::prison_behavior::{PrisonBehavior, PrisonBehaviorModuleData};
#[cfg(feature = "allow_surrender")]
use crate::object::Object;

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct PropagandaCenterBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub base: PrisonBehaviorModuleData,
    pub brainwash_duration: UnsignedInt,
}

#[cfg(feature = "allow_surrender")]
impl Default for PropagandaCenterBehaviorModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: PrisonBehaviorModuleData::default(),
            brainwash_duration: 0,
        }
    }
}

#[cfg(feature = "allow_surrender")]
fn parse_brainwash_duration(
    _ini: &mut INI,
    data: &mut PropagandaCenterBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.brainwash_duration = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

#[cfg(feature = "allow_surrender")]
const PROPAGANDA_CENTER_FIELDS: &[FieldParse<PropagandaCenterBehaviorModuleData>] = &[FieldParse {
    token: "BrainwashDuration",
    parse: parse_brainwash_duration,
}];

#[cfg(feature = "allow_surrender")]
impl PropagandaCenterBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, PROPAGANDA_CENTER_FIELDS)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for PropagandaCenterBehaviorModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.base.crc(xfer)?;
        let mut brainwash_duration = self.brainwash_duration;
        xfer.xfer_unsigned_int(&mut brainwash_duration)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)?;
        xfer.xfer_unsigned_int(&mut self.brainwash_duration)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
crate::impl_legacy_module_data_with_key_field!(
    PropagandaCenterBehaviorModuleData,
    module_tag_name_key
);

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct PropagandaCenterBehavior {
    object_id: ObjectID,
    module_data: Arc<PropagandaCenterBehaviorModuleData>,
    prison_behavior: PrisonBehavior,
    brainwashing_subject_id: ObjectID,
    brainwashing_subject_start_frame: UnsignedInt,
    brainwashed_list: Vec<ObjectID>,
}

#[cfg(feature = "allow_surrender")]
impl PropagandaCenterBehavior {
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<PropagandaCenterBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let prison_behavior =
            PrisonBehavior::new(Arc::clone(&object), Arc::new(module_data.base.clone()))?;
        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data,
            prison_behavior,
            brainwashing_subject_id: INVALID_ID,
            brainwashing_subject_start_frame: 0,
            brainwashed_list: Vec::new(),
        })
    }

    fn get_object_id(&self) -> crate::common::ObjectID {
        self.object_id
    }

    fn with_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    fn clear_brainwashing_subject_if_match(&mut self, object_id: ObjectID) {
        if self.brainwashing_subject_id == object_id {
            self.brainwashing_subject_id = INVALID_ID;
            self.brainwashing_subject_start_frame = 0;
        }
    }

    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = self.prison_behavior.on_delete();
        for &brainwashed_id in &self.brainwashed_list {
            if let Some(object) = TheGameLogic::find_object_by_id(brainwashed_id) {
                if let Ok(mut guard) = object.write() {
                    guard.restore_original_team()?;
                }
            }
        }
        self.brainwashed_list.clear();
        Ok(())
    }

    fn process_brainwashing(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = TheGameLogic::get_frame();

        if self.brainwashing_subject_id != INVALID_ID {
            let Some(subject_arc) = TheGameLogic::find_object_by_id(self.brainwashing_subject_id)
            else {
                self.clear_brainwashing_subject_if_match(self.brainwashing_subject_id);
                return Ok(());
            };

            if current_frame.saturating_sub(self.brainwashing_subject_start_frame)
                >= self.module_data.brainwash_duration
            {
                let Some(exit_interface) = self
                    .with_object(|guard| guard.get_object_exit_interface())
                    .flatten()
                else {
                    return Ok(());
                };

                let Ok(subject_guard) = subject_arc.read() else {
                    return Ok(());
                };
                let Some((exit_door, controlling_player)) = self
                    .with_object(|owner_guard| {
                        let Ok(mut exit_guard) = exit_interface.lock() else {
                            return None;
                        };
                        Some((
                            exit_guard
                                .reserve_door_for_exit(Some(owner_guard), Some(&*subject_guard)),
                            owner_guard.get_controlling_player(),
                        ))
                    })
                    .flatten()
                else {
                    return Ok(());
                };
                drop(subject_guard);

                if matches!(exit_door, ExitDoorType::None | ExitDoorType::NoneAvailable) {
                    return Ok(());
                }

                if let Some(player_arc) = controlling_player {
                    if let Ok(player_guard) = player_arc.read() {
                        if let Some(default_team) = player_guard.get_default_team() {
                            if let Ok(mut subject_guard) = subject_arc.write() {
                                subject_guard.set_temporary_team(Some(default_team))?;
                            }
                        }
                    }
                }

                if let Ok(subject_guard) = subject_arc.read() {
                    if let Some(ai) = subject_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard.set_surrendered(None, false);
                        }
                    }
                }

                let subject_id = subject_arc.read().map(|guard| guard.get_id()).unwrap_or(0);
                if subject_id != INVALID_ID && !self.brainwashed_list.contains(&subject_id) {
                    self.brainwashed_list.push(subject_id);
                }

                if let Ok(mut exit_guard) = exit_interface.lock() {
                    let _ = exit_guard.exit_object_via_door(
                        subject_arc.read().map(|g| g.get_id()).unwrap_or(0),
                        exit_door,
                    );
                };
            }
        }

        if self.brainwashing_subject_id == INVALID_ID {
            if let Some(&first_contained_id) = self.prison_behavior.get_contained_objects().first()
            {
                if TheGameLogic::find_object_by_id(first_contained_id).is_some() {
                    self.brainwashing_subject_id = first_contained_id;
                    self.brainwashing_subject_start_frame = current_frame;
                }
            }
        }

        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
impl UpdateModuleInterface for PropagandaCenterBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let _ = UpdateModuleInterface::update(&mut self.prison_behavior)?;
        self.process_brainwashing()?;
        Ok(UpdateSleepTime::None)
    }
}

#[cfg(feature = "allow_surrender")]
impl ContainModuleInterface for PropagandaCenterBehavior {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.prison_behavior.can_contain(object_id)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.prison_behavior.contain_object(object_id)
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.prison_behavior.release_object(object_id)
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        self.prison_behavior.get_contained_objects()
    }

    fn get_contained_count(&self) -> usize {
        self.prison_behavior.get_contained_count()
    }

    fn get_max_capacity(&self) -> usize {
        self.prison_behavior.get_max_capacity()
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        self.prison_behavior.is_enclosing_container_for(obj)
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.prison_behavior
            .is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prison_behavior.add_to_contain(obj)
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prison_behavior.enable_load_sounds(enabled)
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prison_behavior
            .on_object_wants_to_enter_or_exit(obj, want)
    }

    fn is_garrisonable(&self) -> bool {
        self.prison_behavior.is_garrisonable()
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        self.prison_behavior.is_passenger_allowed_to_fire(id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.prison_behavior.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.prison_behavior.set_passenger_allowed_to_fire(allowed);
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.prison_behavior.on_containing(obj_id, was_selected)
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Ok(guard) = obj.read() {
            self.clear_brainwashing_subject_if_match(guard.get_id());
        }
        self.prison_behavior.on_removing(obj_id)
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prison_behavior.remove_all_contained(expose_stealth)
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.prison_behavior
            .client_visible_contained_flash_as_selected()
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        self.prison_behavior.friend_get_rider()
    }
}

#[cfg(feature = "allow_surrender")]
impl BehaviorModuleInterface for PropagandaCenterBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_contain(&mut self) -> Option<&mut dyn ContainModuleInterface> {
        Some(self)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for PropagandaCenterBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e:?}"))?;

        Snapshotable::crc(&self.prison_behavior, xfer).map_err(|e| e.to_string())?;

        let mut brainwashing_subject_id = self.brainwashing_subject_id;
        xfer.xfer_object_id(&mut brainwashing_subject_id)
            .map_err(|e| e.to_string())?;
        let mut brainwashing_subject_start_frame = self.brainwashing_subject_start_frame;
        xfer.xfer_unsigned_int(&mut brainwashing_subject_start_frame)
            .map_err(|e| e.to_string())?;

        let mut list_count: u16 = self.brainwashed_list.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut list_count)
            .map_err(|e| e.to_string())?;

        for id in self
            .brainwashed_list
            .iter()
            .copied()
            .take(list_count as usize)
        {
            let mut id_copy = id;
            xfer.xfer_object_id(&mut id_copy)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e:?}"))?;

        Snapshotable::xfer(&mut self.prison_behavior, xfer).map_err(|e| e.to_string())?;

        xfer.xfer_object_id(&mut self.brainwashing_subject_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.brainwashing_subject_start_frame)
            .map_err(|e| e.to_string())?;

        let mut list_count: u16 = self.brainwashed_list.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut list_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for id in self
                .brainwashed_list
                .iter()
                .copied()
                .take(list_count as usize)
            {
                let mut id_copy = id;
                xfer.xfer_object_id(&mut id_copy)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            self.brainwashed_list.clear();
            for _ in 0..list_count {
                let mut id: ObjectID = 0;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
                self.brainwashed_list.push(id);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.prison_behavior).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct PropagandaCenterBehaviorModule {
    behavior: Arc<Mutex<PropagandaCenterBehavior>>,
    module_name_key: NameKeyType,
    module_data: Arc<PropagandaCenterBehaviorModuleData>,
}

#[cfg(feature = "allow_surrender")]
impl PropagandaCenterBehaviorModule {
    pub fn new(
        behavior: PropagandaCenterBehavior,
        module_name: &AsciiString,
        module_data: Arc<PropagandaCenterBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> Option<std::sync::MutexGuard<'_, PropagandaCenterBehavior>> {
        self.behavior.lock().ok()
    }

    pub fn contain_handle(&self) -> Arc<Mutex<dyn ContainModuleInterface>> {
        Arc::new(Mutex::new(PropagandaCenterBehaviorContainHandle {
            behavior: Arc::clone(&self.behavior),
        }))
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
struct PropagandaCenterBehaviorContainHandle {
    behavior: Arc<Mutex<PropagandaCenterBehavior>>,
}

#[cfg(feature = "allow_surrender")]
impl ContainModuleInterface for PropagandaCenterBehaviorContainHandle {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.behavior
            .lock()
            .map(|guard| guard.can_contain(object_id))
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.behavior
            .lock()
            .map_err(|_| "PropagandaCenterBehaviorContainHandle lock poisoned".to_string())?
            .contain_object(object_id)
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.behavior
            .lock()
            .map_err(|_| "PropagandaCenterBehaviorContainHandle lock poisoned".to_string())?
            .release_object(object_id)
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        &[]
    }

    fn get_contained_count(&self) -> usize {
        self.behavior
            .lock()
            .map(|guard| guard.get_contained_count())
            .unwrap_or(0)
    }

    fn get_max_capacity(&self) -> usize {
        self.behavior
            .lock()
            .map(|guard| guard.get_max_capacity())
            .unwrap_or(0)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for PropagandaCenterBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(guard) = self.behavior.lock() {
            Snapshotable::crc(&*guard, xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut guard) = self.behavior.lock() {
            Snapshotable::xfer(&mut *guard, xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
impl Module for PropagandaCenterBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        ModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_delete(&mut self) {
        if let Ok(mut guard) = self.behavior.lock() {
            let _ = guard.on_delete();
        }
    }
}

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default, Clone)]
pub struct PropagandaCenterBehaviorModuleData;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct PropagandaCenterBehavior;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct PropagandaCenterBehaviorModule;

pub mod utils {
    use crate::common::FrameNumber;

    pub fn calculate_brainwashing_progress(
        start_frame: FrameNumber,
        current_frame: FrameNumber,
        duration: u32,
    ) -> f32 {
        if duration == 0 {
            return 0.0;
        }

        let elapsed = current_frame.saturating_sub(start_frame);
        (elapsed as f32 / duration as f32).min(1.0)
    }

    pub fn is_brainwashing_complete(
        start_frame: FrameNumber,
        current_frame: FrameNumber,
        duration: u32,
    ) -> bool {
        current_frame.saturating_sub(start_frame) >= duration
    }
}

#[cfg(test)]
mod tests {
    use super::utils;

    #[test]
    fn test_brainwashing_progress_calculation() {
        let progress = utils::calculate_brainwashing_progress(100, 150, 100);
        assert_eq!(progress, 0.5);

        let complete_progress = utils::calculate_brainwashing_progress(100, 250, 100);
        assert_eq!(complete_progress, 1.0);

        let zero_duration = utils::calculate_brainwashing_progress(100, 150, 0);
        assert_eq!(zero_duration, 0.0);
    }

    #[test]
    fn test_brainwashing_completion_check() {
        assert!(!utils::is_brainwashing_complete(100, 150, 100));
        assert!(utils::is_brainwashing_complete(100, 200, 100));
        assert!(utils::is_brainwashing_complete(100, 250, 100));
    }
}
