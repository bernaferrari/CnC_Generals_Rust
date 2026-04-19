//! POWTruckAIUpdate - AI update logic for POW trucks.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/POWTruckAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::TheActionManager;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{ObjectID, Real, UnsignedInt, INVALID_ID, LOGICFRAMES_PER_SECOND};
use crate::helpers::{TheGameLogic, TheGlobalData, TheInGameUI, ThePartitionManager};
use crate::modules::{AIUpdateInterface, POWTruckAIUpdateInterface};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct POWTruckAIUpdateData {
    /// After this long we seek out targets in AUTOMATIC mode.
    pub bored_time_in_frames: UnsignedInt,
    /// This close is considered "at the prison" for purposes of waiting.
    pub hang_around_prison_distance: Real,
}

#[cfg(feature = "allow_surrender")]
impl Default for POWTruckAIUpdateData {
    fn default() -> Self {
        Self {
            bored_time_in_frames: 0,
            hang_around_prison_distance: 0.0,
        }
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct POWTruckAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: POWTruckAIUpdateData,
}

#[cfg(feature = "allow_surrender")]
impl Default for POWTruckAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: POWTruckAIUpdateData::default(),
        }
    }
}

#[cfg(feature = "allow_surrender")]
impl POWTruckAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, POW_TRUCK_AI_UPDATE_FIELDS)
    }
}

#[cfg(feature = "allow_surrender")]
impl ModuleData for POWTruckAIUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn is_ai_module_data(&self) -> bool {
        true
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for POWTruckAIUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(feature = "allow_surrender")]
fn parse_bored_time(
    _ini: &mut INI,
    data: &mut POWTruckAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    data.base.bored_time_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

#[cfg(feature = "allow_surrender")]
fn parse_hang_around_distance(
    _ini: &mut INI,
    data: &mut POWTruckAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    data.base.hang_around_prison_distance = INI::parse_real(token)?;
    Ok(())
}

#[cfg(feature = "allow_surrender")]
const POW_TRUCK_AI_UPDATE_FIELDS: &[FieldParse<POWTruckAIUpdateModuleData>] = &[
    FieldParse {
        token: "BoredTime",
        parse: parse_bored_time,
    },
    FieldParse {
        token: "AtPrisonDistance",
        parse: parse_hang_around_distance,
    },
];

/// Module wrapper for POWTruckAIUpdate to align with module system expectations.
#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct POWTruckAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<POWTruckAIUpdateModuleData>,
}

#[cfg(feature = "allow_surrender")]
impl POWTruckAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<POWTruckAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

#[cfg(feature = "allow_surrender")]
impl Module for POWTruckAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for POWTruckAIUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Current POW truck task (stored in save files in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum POWTruckTask {
    Waiting = 0,
    FindTarget = 1,
    CollectingTarget = 2,
    ReturningPrisoners = 3,
}

/// POW truck AI mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum POWTruckAIMode {
    Automatic = 0,
    Manual = 1,
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct POWTruckAIUpdate {
    data: POWTruckAIUpdateData,
    owner_id: ObjectID,
    ai_mode: POWTruckAIMode,
    current_task: POWTruckTask,
    target_id: ObjectID,
    prison_id: ObjectID,
    entered_waiting_frame: UnsignedInt,
    last_find_frame: UnsignedInt,
}

#[cfg(feature = "allow_surrender")]
impl POWTruckAIUpdate {
    pub fn new(data: POWTruckAIUpdateData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            ai_mode: POWTruckAIMode::Automatic,
            current_task: POWTruckTask::Waiting,
            target_id: INVALID_ID,
            prison_id: INVALID_ID,
            entered_waiting_frame: 0,
            last_find_frame: 0,
        }
    }

    pub fn get_current_task(&self) -> POWTruckTask {
        self.current_task
    }

    pub fn update(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ai.set_ultra_accurate(true)?;

        match self.current_task {
            POWTruckTask::Waiting => self.update_waiting(owner_id, ai)?,
            POWTruckTask::FindTarget => self.update_find_target(owner_id, ai)?,
            POWTruckTask::CollectingTarget => self.update_collecting_target(owner_id, ai)?,
            POWTruckTask::ReturningPrisoners => self.update_return_prisoners(owner_id, ai)?,
        }

        Ok(())
    }

    pub fn handle_pick_up_prisoner(
        &mut self,
        owner_id: ObjectID,
        prisoner_id: ObjectID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prisoner = TheGameLogic::find_object_by_id(prisoner_id);
        if self
            .validate_target(owner_id, prisoner.as_ref(), cmd_source)
            .is_err()
        {
            return Ok(());
        }
        self.private_pick_up_prisoner(owner_id, prisoner_id, cmd_source, ai)
    }

    pub fn handle_return_prisoners(
        &mut self,
        owner_id: ObjectID,
        prison_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.private_return_prisoners(owner_id, prison_id, cmd_source, ai)
    }

    fn load_prisoner_internal(
        &mut self,
        prisoner_id: ObjectID,
        cmd_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let owner_arc = TheGameLogic::find_object_by_id(self.owner_id);
        let prisoner_arc = TheGameLogic::find_object_by_id(prisoner_id);
        let (Some(owner_arc), Some(prisoner_arc)) = (owner_arc, prisoner_arc) else {
            return Ok(());
        };

        if self
            .validate_target(self.owner_id, Some(&prisoner_arc), cmd_source)
            .is_err()
        {
            return Ok(());
        }

        let contain = owner_arc.read().ok().and_then(|guard| guard.get_contain());
        let Some(contain) = contain else {
            return Ok(());
        };

        if let Ok(contain_guard) = contain.lock() {
            if contain_guard.get_contained_count() == contain_guard.get_max_capacity() {
                drop(contain_guard);
                if let Some(prison_id) = self.find_best_prison(self.owner_id) {
                    self.set_task(POWTruckTask::ReturningPrisoners, Some(prison_id));
                } else {
                    self.set_task(POWTruckTask::Waiting, None);
                }
                return Ok(());
            }
        }

        let _ = contain
            .lock()
            .map(|mut guard| guard.contain_object(prisoner_id));

        if let Ok(prisoner_guard) = prisoner_arc.read() {
            if let Some(prisoner_ai) = prisoner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = prisoner_ai.lock() {
                    ai_guard.set_surrendered(None, false);
                }
            }
        }

        if self.ai_mode == POWTruckAIMode::Automatic {
            self.set_task(POWTruckTask::FindTarget, None);
        } else {
            self.set_task(POWTruckTask::Waiting, None);
        }

        Ok(())
    }

    fn set_task(&mut self, task: POWTruckTask, task_object: Option<ObjectID>) {
        let old_task = self.current_task;

        if matches!(
            task,
            POWTruckTask::CollectingTarget | POWTruckTask::ReturningPrisoners
        ) && task_object.is_none()
        {
            self.set_task(POWTruckTask::Waiting, None);
            return;
        }

        if old_task == POWTruckTask::CollectingTarget {
            self.target_id = INVALID_ID;
        }

        if old_task == POWTruckTask::ReturningPrisoners {
            self.prison_id = INVALID_ID;
        }

        match task {
            POWTruckTask::CollectingTarget => {
                self.target_id = task_object.unwrap_or(INVALID_ID);
            }
            POWTruckTask::ReturningPrisoners => {
                self.prison_id = task_object.unwrap_or(INVALID_ID);
            }
            POWTruckTask::Waiting => {
                self.entered_waiting_frame = TheGameLogic::get_frame();
            }
            _ => {}
        }

        self.current_task = task;
    }

    fn set_ai_mode(&mut self, mode: POWTruckAIMode) {
        self.ai_mode = mode;
    }

    fn private_pick_up_prisoner(
        &mut self,
        owner_id: ObjectID,
        prisoner_id: ObjectID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prisoner = TheGameLogic::find_object_by_id(prisoner_id);
        if self
            .validate_target(owner_id, prisoner.as_ref(), cmd_source)
            .is_err()
        {
            return Ok(());
        }

        if matches!(cmd_source, CommandSourceType::FromPlayer) {
            self.set_ai_mode(POWTruckAIMode::Automatic);
        } else {
            self.set_ai_mode(POWTruckAIMode::Automatic);
        }

        self.set_task(POWTruckTask::CollectingTarget, Some(prisoner_id));

        if let Some(prisoner_legacy) = get_legacy_object(prisoner_id) {
            let _ = ai.ignore_obstacle(Some(&prisoner_legacy));
        }

        ai.set_ultra_accurate(true)?;
        self.issue_move_to_object(ai, prisoner_id, cmd_source);

        Ok(())
    }

    fn private_return_prisoners(
        &mut self,
        owner_id: ObjectID,
        prison_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if matches!(cmd_source, CommandSourceType::FromPlayer) {
            self.set_ai_mode(POWTruckAIMode::Automatic);
        }

        let prison_id = prison_id.or_else(|| self.find_best_prison(owner_id));
        let Some(prison_id) = prison_id else {
            return Ok(());
        };

        self.set_task(POWTruckTask::ReturningPrisoners, Some(prison_id));
        ai.set_ultra_accurate(true)?;
        self.issue_dock(ai, prison_id, cmd_source);

        Ok(())
    }

    fn update_waiting(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.ai_mode == POWTruckAIMode::Manual {
            return Ok(());
        }

        let owner = TheGameLogic::find_object_by_id(owner_id);
        let Some(owner_arc) = owner else {
            return Ok(());
        };

        if !ai.is_idle() {
            self.entered_waiting_frame = TheGameLogic::get_frame();
        }

        if TheGameLogic::get_frame().saturating_sub(self.entered_waiting_frame)
            > self.data.bored_time_in_frames
        {
            self.set_task(POWTruckTask::FindTarget, None);
        }

        drop(owner_arc);
        Ok(())
    }

    fn update_find_target(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        const FIND_DELAY: UnsignedInt = LOGICFRAMES_PER_SECOND;

        if self.ai_mode == POWTruckAIMode::Manual {
            return Ok(());
        }

        if TheGameLogic::get_frame().saturating_sub(self.last_find_frame) < FIND_DELAY {
            return Ok(());
        }

        self.last_find_frame = TheGameLogic::get_frame();

        let owner = TheGameLogic::find_object_by_id(owner_id);
        let Some(owner_arc) = owner else {
            return Ok(());
        };
        let owner_guard = owner_arc.read().ok();
        let Some(owner_guard) = owner_guard else {
            return Ok(());
        };

        if let Some(contain) = owner_guard.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                if contain_guard.get_contained_count() == contain_guard.get_max_capacity() {
                    drop(contain_guard);
                    drop(owner_guard);
                    self.do_return_prisoners(owner_id, ai)?;
                    return Ok(());
                }
            }
        }

        drop(owner_guard);

        if let Some(target_id) = self.find_best_target(owner_id, ai.get_last_command_source()) {
            self.private_pick_up_prisoner(owner_id, target_id, CommandSourceType::FromAi, ai)?;
        } else {
            let has_prisoners = owner_arc
                .read()
                .ok()
                .and_then(|guard| guard.get_contain())
                .and_then(|contain| contain.lock().ok().map(|c| c.get_contained_count() > 0))
                .unwrap_or(false);
            if has_prisoners {
                self.do_return_prisoners(owner_id, ai)?;
            } else {
                self.do_return_to_prison(owner_id, None, ai)?;
            }
        }

        Ok(())
    }

    fn update_collecting_target(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let target = TheGameLogic::find_object_by_id(self.target_id);
        if self
            .validate_target(owner_id, target.as_ref(), ai.get_last_command_source())
            .is_err()
        {
            if self.ai_mode == POWTruckAIMode::Automatic {
                self.set_task(POWTruckTask::FindTarget, None);
            } else {
                self.set_task(POWTruckTask::Waiting, None);
            }
            return Ok(());
        }

        if ai.is_idle() {
            if self.ai_mode == POWTruckAIMode::Automatic {
                self.set_task(POWTruckTask::FindTarget, None);
            } else {
                self.set_task(POWTruckTask::Waiting, None);
            }
        }

        Ok(())
    }

    fn update_return_prisoners(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prison = TheGameLogic::find_object_by_id(self.prison_id);
        if prison.is_none() {
            self.do_return_prisoners(owner_id, ai)?;
            return Ok(());
        }

        if ai.is_idle() {
            self.do_return_prisoners(owner_id, ai)?;
        }

        Ok(())
    }

    fn validate_target(
        &self,
        owner_id: ObjectID,
        target: Option<&Arc<RwLock<Object>>>,
        cmd_source: CommandSourceType,
    ) -> Result<(), String> {
        let Some(target_arc) = target else {
            return Err("missing target".into());
        };

        let owner_arc = TheGameLogic::find_object_by_id(owner_id).ok_or("missing owner")?;

        let owner_guard = owner_arc.read().map_err(|_| "owner lock")?;
        let target_guard = target_arc.read().map_err(|_| "target lock")?;

        if !TheActionManager::can_pick_up_prisoner(&owner_guard, &target_guard, cmd_source) {
            return Err("cannot pick up prisoner".into());
        }

        Ok(())
    }

    fn do_return_prisoners(
        &mut self,
        owner_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prison_id = self.find_best_prison(owner_id);
        if prison_id.is_none() {
            self.set_task(POWTruckTask::Waiting, None);
            return Ok(());
        }

        self.private_return_prisoners(owner_id, prison_id, CommandSourceType::FromAi, ai)
    }

    fn do_return_to_prison(
        &mut self,
        owner_id: ObjectID,
        prison_id: Option<ObjectID>,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_task(POWTruckTask::Waiting, None);

        let prison_id = prison_id.or_else(|| self.find_best_prison(owner_id));
        let Some(prison_id) = prison_id else {
            return Ok(());
        };

        let owner = TheGameLogic::find_object_by_id(owner_id);
        let prison = TheGameLogic::find_object_by_id(prison_id);
        let (Some(owner_arc), Some(prison_arc)) = (owner, prison) else {
            return Ok(());
        };

        let dist_sq = {
            let owner_guard = owner_arc.read().ok();
            let prison_guard = prison_arc.read().ok();
            if let (Some(owner_guard), Some(prison_guard)) = (owner_guard, prison_guard) {
                ThePartitionManager::get_distance_squared(
                    &owner_guard,
                    &prison_guard,
                    crate::common::FROM_CENTER_2D,
                )
            } else {
                return Ok(());
            }
        };

        let hang_dist = self.data.hang_around_prison_distance;
        if dist_sq <= hang_dist * hang_dist {
            return Ok(());
        }

        self.issue_dock(ai, prison_id, CommandSourceType::FromAi);
        Ok(())
    }

    fn find_best_prison(&self, owner_id: ObjectID) -> Option<ObjectID> {
        let owner_arc = TheGameLogic::find_object_by_id(owner_id)?;
        let owner_guard = owner_arc.read().ok()?;
        let prison_id = owner_guard.get_producer_id();
        if prison_id == INVALID_ID {
            return None;
        }
        Some(prison_id)
    }

    fn find_best_target(
        &self,
        owner_id: ObjectID,
        cmd_source: CommandSourceType,
    ) -> Option<ObjectID> {
        let owner_arc = TheGameLogic::find_object_by_id(owner_id)?;
        let owner_guard = owner_arc.read().ok()?;

        let mut closest_target: Option<ObjectID> = None;
        let mut closest_dist_sq: Real = Real::MAX;
        for obj in crate::object::registry::OBJECT_REGISTRY.get_all_objects() {
            let obj_id = obj
                .read()
                .ok()
                .map(|guard| guard.get_id())
                .unwrap_or(INVALID_ID);
            if obj_id == owner_id || obj_id == INVALID_ID {
                continue;
            }
            if self
                .validate_target(owner_id, Some(&obj), cmd_source)
                .is_err()
            {
                continue;
            }

            if let Ok(obj_guard) = obj.read() {
                let dist_sq = ThePartitionManager::get_distance_squared(
                    &owner_guard,
                    &obj_guard,
                    crate::common::FROM_CENTER_2D,
                );
                if closest_target.is_none() || dist_sq < closest_dist_sq {
                    closest_target = Some(obj_id);
                    closest_dist_sq = dist_sq;
                }
            }
        }

        closest_target
    }

    fn issue_move_to_object(
        &self,
        ai: &mut dyn AIUpdateInterface,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        let mut params = AiCommandParams::new(AiCommandType::MoveToObject, cmd_source);
        params.obj = Some(target_id);
        let _ = ai.execute_command(&params);
    }

    fn issue_dock(
        &self,
        ai: &mut dyn AIUpdateInterface,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        let mut params = AiCommandParams::new(AiCommandType::Dock, cmd_source);
        params.obj = Some(target_id);
        let _ = ai.execute_command(&params);
    }
}

#[cfg(feature = "allow_surrender")]
impl POWTruckAIUpdateInterface for POWTruckAIUpdate {
    fn set_task(&mut self, task: POWTruckTask, task_object: Option<ObjectID>) {
        POWTruckAIUpdate::set_task(self, task, task_object);
    }

    fn get_current_task(&self) -> POWTruckTask {
        self.current_task
    }

    fn load_prisoner(&mut self, prisoner: ObjectID) {
        if let Err(err) = self.load_prisoner_internal(prisoner, CommandSourceType::FromAi) {
            log::warn!(
                "POWTruckAIUpdate: failed to load prisoner {}: {}",
                prisoner,
                err
            );
        }
    }

    fn unload_prisoners_to_prison(&mut self, prison: &Arc<RwLock<Object>>) {
        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let owner_guard = owner_arc
            .read()
            .map_err(|_| "pow truck owner lock poisoned");
        let Ok(owner_guard) = owner_guard else {
            return;
        };
        let Some(truck_contain) = owner_guard.get_contain() else {
            return;
        };

        let mut bounty: u32 = 0;
        let prisoner_ids: Vec<ObjectID> = truck_contain
            .lock()
            .map(|contain| contain.get_contained_objects().to_vec())
            .unwrap_or_default();

        for prisoner_id in prisoner_ids {
            let prisoner_arc = TheGameLogic::find_object_by_id(prisoner_id);
            let Some(prisoner_arc) = prisoner_arc else {
                continue;
            };

            let _ = truck_contain
                .lock()
                .map(|mut contain| contain.release_object(prisoner_id));

            if let Ok(mut prison_guard) = prison.write() {
                if let Some(prison_contain) = prison_guard.get_contain() {
                    let _ = prison_contain
                        .lock()
                        .map(|mut contain| contain.contain_object(prisoner_id));
                }
            }

            if let Ok(prisoner_guard) = prisoner_arc.read() {
                let cost = prisoner_guard.get_build_cost().max(0) as f32;
                let multiplier = TheGlobalData::get()
                    .map(|gd| gd.get_prison_bounty_multiplier())
                    .unwrap_or(0.0);
                bounty = bounty.saturating_add((multiplier * cost) as u32);
            }
        }

        if let Ok(prison_guard) = prison.read() {
            if prison_guard.is_kind_of(crate::common::KindOf::CollectsPrisonBounty) && bounty > 0 {
                if let Some(player) = owner_guard.get_controlling_player() {
                    if let Ok(mut player_guard) = player.write() {
                        let _ = player_guard.get_money_mut().deposit(bounty);
                        player_guard.get_money_mut().add_money_earned(bounty as i32);
                    }
                }

                let mut pos = *prison_guard.get_position();
                pos.z += prison_guard
                    .get_geometry_info()
                    .get_max_height_above_position();
                let color = TheGlobalData::get()
                    .map(|gd| gd.get_prison_bounty_text_color())
                    .unwrap_or(crate::common::Color::WHITE);
                let text = format!("+{}", bounty);
                let _ = TheInGameUI::add_floating_text(&text, &pos, color);
            }
        }

        self.set_task(POWTruckTask::Waiting, None);
    }
}
