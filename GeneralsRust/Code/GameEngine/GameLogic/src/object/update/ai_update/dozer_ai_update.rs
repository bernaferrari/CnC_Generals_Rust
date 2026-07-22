//! DozerAIUpdate module data + AI update logic.
//!
//! Ported from GameLogic/Module/DozerAIUpdate.h and
//! GameLogic/Object/Update/AIUpdate/DozerAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::ActionManager;
use crate::ai::{CommandSourceType, THE_AI};
use crate::common::audio::AudioEventRts;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, KindOf, ModelConditionFlags, ObjectID, Real, UnsignedInt,
    INVALID_ID, MODELCONDITION_ACTIVELY_CONSTRUCTING, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::helpers::{FindPositionOptions, TheAudio, TheGameLogic, ThePartitionManager};
use crate::modules::AIUpdateInterface;
use crate::object::behavior::behavior_module::{
    BridgeBehaviorInterface, BridgeTowerBehaviorInterface, BridgeTowerType,
};
use crate::object::production::get_construction_manager;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::player_list;
use crate::state_machine::StateReturnType;
use game_engine::common::ini::{FieldParse, INIError, INILoadType, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const MIN_ACTION_TOLERANCE: Real = 70.0;

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

/// Dozer task types (matches C++ DozerTask).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DozerTask {
    Invalid = -1,
    Build = 0,
    Repair = 1,
    Fortify = 2,
}

impl DozerTask {
    fn as_index(self) -> usize {
        self as i32 as usize
    }
}

const DOZER_NUM_TASKS: usize = 3;

/// Dock points for dozer tasks (matches C++ DozerDockPoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DozerDockPoint {
    Start = 0,
    Action = 1,
    End = 2,
}

impl DozerDockPoint {
    fn as_index(self) -> usize {
        self as i32 as usize
    }
}

const DOZER_NUM_DOCK_POINTS: usize = 3;

/// Build subtask (matches C++ DozerBuildSubTask).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DozerBuildSubTask {
    SelectBuildDockLocation = 0,
    MovingToBuildDockLocation = 1,
    DoBuildAtDock = 2,
}

/// Module data for DozerAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct DozerAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub repair_health_percent_per_second: Real,
    pub bored_time: Real,
    pub bored_range: Real,
}

impl Default for DozerAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
        }
    }
}

impl DozerAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DOZER_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for DozerAIUpdateModuleData {
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

impl Snapshotable for DozerAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_real(&mut self.repair_health_percent_per_second))?;
        xfer_io(xfer.xfer_real(&mut self.bored_time))?;
        xfer_io(xfer.xfer_real(&mut self.bored_range))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut DozerAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a str>, INIError> {
    let values: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if values.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(values)
}

fn parse_duration_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_duration_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_real(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_percent_to_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

fn parse_locomotor_set_field(
    ini: &mut INI,
    data: &mut DozerAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    if values.len() < 2 {
        return Err(INIError::InvalidData);
    }

    let set = match values[0] {
        "SET_NORMAL" => crate::common::LocomotorSetType::Normal,
        "SET_NORMAL_UPGRADED" => crate::common::LocomotorSetType::NormalUpgraded,
        "SET_FREEFALL" => crate::common::LocomotorSetType::Freefall,
        "SET_WANDER" => crate::common::LocomotorSetType::Wander,
        "SET_PANIC" => crate::common::LocomotorSetType::Panic,
        "SET_TAXIING" => crate::common::LocomotorSetType::Taxiing,
        "SET_SUPERSONIC" => crate::common::LocomotorSetType::Supersonic,
        "SET_SLUGGISH" => crate::common::LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };

    if data.base.has_locomotor_set(set) && ini.get_load_type() != INILoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }

    let mut entries = Vec::new();
    for token in values.iter().skip(1) {
        if token.is_empty() || token.eq_ignore_ascii_case("None") {
            continue;
        }
        entries.push(AsciiString::from(*token));
    }
    if entries.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.base.set_locomotor_set_entries(set, entries);
    Ok(())
}

fn parse_turret_field(
    ini: &mut INI,
    data: &mut DozerAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_primary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_primary(turret);
    Ok(())
}

fn parse_alt_turret_field(
    ini: &mut INI,
    data: &mut DozerAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_secondary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_secondary(turret);
    Ok(())
}

const DOZER_AI_UPDATE_FIELDS: &[FieldParse<DozerAIUpdateModuleData>] = &[
    FieldParse {
        token: "Turret",
        parse: parse_turret_field,
    },
    FieldParse {
        token: "AltTurret",
        parse: parse_alt_turret_field,
    },
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
    },
    FieldParse {
        token: "Locomotor",
        parse: parse_locomotor_set_field,
    },
    FieldParse {
        token: "MoodAttackCheckRate",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(
                &mut |value| data.base.set_surrender_duration_frames(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.base.set_forbid_player_commands(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.base.set_turrets_linked(value), tokens)
        },
    },
    FieldParse {
        token: "RepairHealthPercentPerSecond",
        parse: |_, data, tokens| {
            parse_percent_to_real_field(
                &mut |value| data.repair_health_percent_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "BoredTime",
        parse: |_, data, tokens| {
            parse_duration_real_field(&mut |value| data.bored_time = value, tokens)
        },
    },
    FieldParse {
        token: "BoredRange",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.bored_range = value, tokens),
    },
];

#[derive(Debug, Clone)]
struct DozerTaskEntry {
    target_id: ObjectID,
    order_frame: u32,
}

impl Default for DozerTaskEntry {
    fn default() -> Self {
        Self {
            target_id: INVALID_ID,
            order_frame: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DozerDockPointInfo {
    valid: bool,
    location: Coord3D,
}

impl Default for DozerDockPointInfo {
    fn default() -> Self {
        Self {
            valid: false,
            location: Coord3D::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DozerActionState {
    PickActionPos,
    MoveToActionPos,
    DoAction,
}

#[derive(Debug, Clone)]
struct DozerActionTask {
    task_type: DozerTask,
    target_id: ObjectID,
    dock_point: Option<Coord3D>,
    #[allow(dead_code)]
    failed_attempts: u32,
    build_total_frames: u32,
    build_max_health: f32,
    is_rebuild: bool,
    started_construction: bool,
}

/// Runtime data for dozer AI.
#[derive(Debug, Clone)]
pub struct DozerAIUpdateData {
    pub repair_health_percent_per_second: Real,
    pub bored_time: Real,
    pub bored_range: Real,
}

impl Default for DozerAIUpdateData {
    fn default() -> Self {
        Self {
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
        }
    }
}

/// Dozer AI Update module (matches C++ DozerAIUpdate).
pub struct DozerAIUpdate {
    data: DozerAIUpdateData,
    object_id: ObjectID,
    dozer_task: Option<DozerActionTask>,
    action_state: DozerActionState,
    tasks: [DozerTaskEntry; DOZER_NUM_TASKS],
    dock_points: [[DozerDockPointInfo; DOZER_NUM_DOCK_POINTS]; DOZER_NUM_TASKS],
    current_task: DozerTask,
    build_sub_task: DozerBuildSubTask,
    is_rebuild: bool,
    building_sound: Option<AudioEventRts>,
    building_sound_target: ObjectID,
}

impl std::fmt::Debug for DozerAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DozerAIUpdate")
            .field("data", &self.data)
            .field("object_id", &self.object_id)
            .field("dozer_task", &self.dozer_task)
            .field("action_state", &self.action_state)
            .field("current_task", &self.current_task)
            .field("build_sub_task", &self.build_sub_task)
            .field("is_rebuild", &self.is_rebuild)
            .finish()
    }
}

impl DozerAIUpdate {
    pub fn new(data: DozerAIUpdateData, object_id: ObjectID) -> Self {
        Self {
            data,
            object_id,
            dozer_task: None,
            action_state: DozerActionState::PickActionPos,
            tasks: [
                DozerTaskEntry::default(),
                DozerTaskEntry::default(),
                DozerTaskEntry::default(),
            ],
            dock_points: [
                [
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                ],
                [
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                ],
                [
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                    DozerDockPointInfo::default(),
                ],
            ],
            current_task: DozerTask::Invalid,
            build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
            is_rebuild: false,
            building_sound: None,
            building_sound_target: INVALID_ID,
        }
    }

    fn owner_object(&self) -> Option<Arc<RwLock<Object>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    pub fn get_repair_health_per_second(&self) -> Real {
        self.data.repair_health_percent_per_second
    }

    pub fn get_bored_time(&self) -> Real {
        self.data.bored_time
    }

    pub fn get_bored_range(&self) -> Real {
        let mut range = self.data.bored_range;
        let player_id = self.owner_object().and_then(|obj| {
            let guard = obj.read().ok()?;
            guard.get_controlling_player_id()
        });
        let is_ai = if let Some(player_id) = player_id {
            if let Ok(list) = player_list().read() {
                if let Some(player) = list.get_player(player_id as i32) {
                    if let Ok(player_guard) = player.read() {
                        player_guard.is_skirmish_ai()
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if is_ai {
            if let Ok(ai) = THE_AI.read() {
                range *= ai
                    .get_ai_data()
                    .read()
                    .map(|d| d.ai_dozer_bored_radius_modifier)
                    .unwrap_or(1.0);
            }
        }
        range
    }

    pub fn get_current_task(&self) -> DozerTask {
        self.current_task
    }

    pub fn set_current_task(&mut self, task: DozerTask) {
        self.current_task = task;
    }

    pub fn get_is_rebuild(&self) -> bool {
        self.is_rebuild
    }

    pub fn set_build_sub_task(&mut self, sub: DozerBuildSubTask) {
        self.build_sub_task = sub;
    }

    pub fn get_build_sub_task(&self) -> DozerBuildSubTask {
        self.build_sub_task
    }

    pub fn is_task_pending(&self, task: DozerTask) -> bool {
        if task == DozerTask::Invalid {
            return false;
        }
        self.tasks[task.as_index()].target_id != INVALID_ID
    }

    pub fn is_any_task_pending(&self) -> bool {
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            if self.is_task_pending(task) {
                return true;
            }
        }
        false
    }

    pub fn get_task_target(&self, task: DozerTask) -> ObjectID {
        if task == DozerTask::Invalid {
            return INVALID_ID;
        }
        self.tasks[task.as_index()].target_id
    }

    pub fn get_most_recent_command(&self) -> DozerTask {
        let mut most_recent = DozerTask::Invalid;
        let mut most_recent_frame = 0;
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            if self.is_task_pending(task) {
                let entry = &self.tasks[task.as_index()];
                if entry.order_frame > most_recent_frame {
                    most_recent = task;
                    most_recent_frame = entry.order_frame;
                }
            }
        }
        most_recent
    }

    pub fn get_dock_point(&self, task: DozerTask, point: DozerDockPoint) -> Option<Coord3D> {
        if task == DozerTask::Invalid {
            return None;
        }
        let info = self.dock_points[task.as_index()][point.as_index()];
        if info.valid {
            Some(info.location)
        } else {
            None
        }
    }

    pub fn new_task(&mut self, task: DozerTask, target_id: ObjectID) {
        if task == DozerTask::Invalid {
            return;
        }
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };

        if task == DozerTask::Build || task == DozerTask::Repair {
            let mut position = Coord3D::ZERO;
            let target_id = Self::find_good_build_or_repair_position_and_target(
                &owner_guard,
                &target_guard,
                &mut position,
            )
            .unwrap_or(target_id);
            let idx = task.as_index();
            self.dock_points[idx][DozerDockPoint::Start.as_index()] = DozerDockPointInfo {
                valid: true,
                location: position,
            };
            self.dock_points[idx][DozerDockPoint::Action.as_index()] = DozerDockPointInfo {
                valid: true,
                location: position,
            };
            let target_pos = if target_id != target_guard.get_id() {
                if let Some(obj) = TheGameLogic::find_object_by_id(target_id) {
                    if let Ok(guard) = obj.read() {
                        *guard.get_position()
                    } else {
                        *target_guard.get_position()
                    }
                } else {
                    *target_guard.get_position()
                }
            } else {
                *target_guard.get_position()
            };
            let mut offset = position - target_pos;
            offset.z = 0.0;
            let len = (offset.x * offset.x + offset.y * offset.y).sqrt();
            if len > 0.0 {
                offset.x /= len;
                offset.y /= len;
            }
            offset.x *= 5.0 * PATHFIND_CELL_SIZE_F;
            offset.y *= 5.0 * PATHFIND_CELL_SIZE_F;
            let mut end_pos = position;
            end_pos.x += offset.x;
            end_pos.y += offset.y;
            self.dock_points[idx][DozerDockPoint::End.as_index()] = DozerDockPointInfo {
                valid: true,
                location: end_pos,
            };
            if task == DozerTask::Build {
                if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                    if let Ok(mut target_write) = target_obj.write() {
                        target_write.set_builder(Some(&owner_guard));
                    }
                }
            }
            self.tasks[task.as_index()].target_id = target_id;
        }

        if self.tasks[task.as_index()].target_id == INVALID_ID {
            self.tasks[task.as_index()].target_id = target_id;
        }
        self.tasks[task.as_index()].order_frame = TheGameLogic::get_frame();
        self.current_task = task;
    }

    pub fn cancel_task(&mut self, task: DozerTask) {
        self.internal_cancel_task(task);
    }

    pub fn internal_task_complete(&mut self, task: DozerTask) {
        self.internal_task_complete_or_cancelled(task);
        if task != DozerTask::Invalid {
            self.tasks[task.as_index()] = DozerTaskEntry::default();
            for point in &mut self.dock_points[task.as_index()] {
                point.valid = false;
            }
        }
    }

    pub fn internal_cancel_task(&mut self, task: DozerTask) {
        self.internal_task_complete_or_cancelled(task);
        if task != DozerTask::Invalid {
            self.tasks[task.as_index()] = DozerTaskEntry::default();
            for point in &mut self.dock_points[task.as_index()] {
                point.valid = false;
            }
        }
        if let Some(owner) = self.owner_object() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.ai_idle();
                    }
                }
            }
        }
    }

    fn internal_task_complete_or_cancelled(&mut self, task: DozerTask) {
        if let Some(owner) = self.owner_object() {
            if let Ok(mut owner_guard) = owner.write() {
                if task == DozerTask::Build || task == DozerTask::Repair {
                    owner_guard.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                }
            }
        }
    }

    pub fn on_delete(&mut self) {
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            if self.is_task_pending(task) {
                self.cancel_task(task);
            }
        }
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            let target_id = self.get_task_target(task);
            if target_id != INVALID_ID {
                if let Some(target) = TheGameLogic::find_object_by_id(target_id) {
                    if let Ok(mut guard) = target.write() {
                        guard.clear_model_condition_state(
                            ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
                        );
                    }
                }
            }
        }
    }

    pub fn can_accept_new_repair(&self, target: &Object) -> bool {
        if self.current_task != DozerTask::Repair {
            return true;
        }
        let current_id = self.tasks[DozerTask::Repair.as_index()].target_id;
        if current_id == INVALID_ID {
            return true;
        }
        let Some(current_obj) = TheGameLogic::find_object_by_id(current_id) else {
            return true;
        };
        let Ok(current_guard) = current_obj.read() else {
            return true;
        };
        if current_guard.get_id() == target.get_id() {
            return false;
        }
        if current_guard.is_kind_of(KindOf::BridgeTower) && target.is_kind_of(KindOf::BridgeTower) {
            let current_bridge = Self::get_bridge_id_for_tower(&current_guard);
            let new_bridge = Self::get_bridge_id_for_tower(target);
            if current_bridge != INVALID_ID && current_bridge == new_bridge {
                return false;
            }
        }
        true
    }

    pub fn set_repair_target(&mut self, target_id: ObjectID, cmd_source: CommandSourceType) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_repair_object(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }
        self.new_task(DozerTask::Repair, target_id);
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Repair,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn set_resume_construction_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_resume_construction_of(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }
        self.new_task(DozerTask::Build, target_id);
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Build,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
    ) {
        self.is_rebuild = is_rebuild;
        self.new_task(DozerTask::Build, building_id);
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Build,
            target_id: building_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            is_rebuild,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn start_building_sound(&mut self, sound: &AudioEventRts, construction_site_id: ObjectID) {
        let Some(audio) = TheAudio::get() else {
            return;
        };
        let mut event = sound.clone();
        event.set_object_id(self.object_id);
        let handle = audio.add_audio_event(&event);
        event.set_playing_handle(handle);
        self.building_sound = Some(event);
        self.building_sound_target = construction_site_id;
    }

    pub fn finish_building_sound(&mut self) {
        if let Some(sound) = self.building_sound.take() {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(sound.get_playing_handle());
            }
        }
        self.building_sound_target = INVALID_ID;
    }

    fn handle_build_completion(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
    ) {
        self.finish_building_sound();

        let mut target_display_name: Option<String> = None;
        let mut target_pos: Option<Coord3D> = None;
        let mut controlling_player: Option<Arc<RwLock<crate::player::Player>>> = None;

        if let Ok(mut target_guard) = target.write() {
            target_guard.clear_status(
                crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::UnderConstruction,
                ) | crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::Reconstructing,
                ),
            );

            let _ = target_guard.clear_model_condition_flags(
                ModelConditionFlags::AWAITING_CONSTRUCTION
                    | ModelConditionFlags::PARTIALLY_CONSTRUCTED
                    | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
            );
            target_guard.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);

            if let Some(body) = target_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let _ = body_guard.evaluate_visual_condition();
                }
            }

            target_guard.handle_partition_cell_maintenance();
            target_guard.update_upgrade_modules_from_player();
            target_guard.on_build_complete();

            let template = target_guard.get_template();
            let display_name = template.get_name();
            if display_name.is_empty() {
                let fallback = crate::helpers::TheGameText::fetch("INI:MissingDisplayName");
                let template_name = template.get_name().as_str();
                target_display_name = Some(fallback.replace("%s", template_name));
            } else {
                target_display_name = Some(display_name.as_str().to_string());
            }
            target_pos = Some(*target_guard.get_position());
            controlling_player = target_guard.get_controlling_player();
        }

        if let Some(player) = controlling_player {
            if let Ok(mut player_guard) = player.write() {
                let builder_id = owner.read().ok().map(|g| g.get_id());
                let structure_id = target
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                player_guard.on_structure_construction_complete_id(
                    builder_id,
                    structure_id,
                    is_rebuild,
                );
            }
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_locally_controlled() {
                if let Some(display_name) = target_display_name.as_ref() {
                    let format = crate::helpers::TheGameText::fetch("DOZER:ConstructionComplete");
                    let message = if format.contains("%s") {
                        format.replace("%s", display_name)
                    } else {
                        format!("{} {}", format, display_name)
                    };
                    crate::helpers::TheInGameUI::display_message(&message);
                }

                if let Some(voice) = owner_guard
                    .get_template()
                    .get_per_unit_sound("VoiceTaskComplete")
                {
                    if let Some(audio) = TheAudio::get() {
                        let mut event = voice.clone();
                        event.set_object_id(owner_guard.get_id());
                        audio.add_audio_event(&event);
                    }
                }

                if let (Some(radar), Some(pos)) = (crate::helpers::TheRadar::get(), target_pos) {
                    radar.create_event(
                        &pos,
                        game_engine::common::system::radar::RadarEventType::Construction,
                        4.0,
                    );
                }
            }
        }

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let end_pos = self
                        .get_dock_point(self.current_task, DozerDockPoint::End)
                        .unwrap_or(*owner_guard.get_position());
                    let _ = ai_guard.ai_move_to_position(&end_pos);
                }
            }
        }
    }

    pub fn update(&mut self) -> StateReturnType {
        // validate current task if repair
        if self.current_task == DozerTask::Repair {
            let target_id = self.get_task_target(DozerTask::Repair);
            if let (Some(owner), Some(target)) = (
                self.owner_object(),
                TheGameLogic::find_object_by_id(target_id),
            ) {
                if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                    if !ActionManager::can_repair_object(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.cancel_task(DozerTask::Repair);
                    }
                }
            }
        }

        if self.dozer_task.is_none() {
            if self.current_task == DozerTask::Invalid {
                self.current_task = self.get_most_recent_command();
            }
            if self.current_task != DozerTask::Invalid {
                self.spawn_dozer_task_from_current();
                if self.dozer_task.is_some() {
                    self.action_state = DozerActionState::PickActionPos;
                }
            }
        }

        if self.dozer_task.is_some() {
            self.update_dozer_task();
        }

        StateReturnType::Continue
    }

    fn spawn_dozer_task_from_current(&mut self) {
        let task = self.current_task;
        if task == DozerTask::Invalid {
            return;
        }
        let target_id = self.get_task_target(task);
        if target_id == INVALID_ID {
            return;
        }
        self.dozer_task = Some(DozerActionTask {
            task_type: task,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            is_rebuild: false,
            started_construction: false,
        });
    }

    fn update_dozer_task(&mut self) {
        let Some(mut task) = self.dozer_task.take() else {
            return;
        };

        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(task.target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            self.dozer_task = Some(task);
            return;
        };
        let owner_pos = *owner_guard.get_position();
        let owner_airborne = owner_guard.is_using_airborne_locomotor();
        let owner_ai_update = owner_guard.get_ai_update_interface();

        let target_pos = *target_guard.get_position();
        let target_radius = target_guard
            .get_geometry_info()
            .get_bounding_sphere_radius();
        let target_builder_id = target_guard.get_builder_id();
        let target_is_bridge_tower = target_guard.is_kind_of(KindOf::BridgeTower);
        let target_body = target_guard.get_body_module();
        let target_body_max = target_body
            .as_ref()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_max_health()))
            .unwrap_or(0.0);
        drop(target_guard);
        drop(owner_guard);

        if task.dock_point.is_none() && self.action_state == DozerActionState::PickActionPos {
            if self.current_task != DozerTask::Invalid {
                if let Some(point) = self.get_dock_point(self.current_task, DozerDockPoint::Start) {
                    task.dock_point = Some(point);
                }
            }
        }

        if task.dock_point.is_none() && self.action_state == DozerActionState::PickActionPos {
            let mut dock_pos = target_pos;
            let start_angle = (owner_pos.y - target_pos.y).atan2(owner_pos.x - target_pos.x);
            let mut options = FindPositionOptions::default();
            options.min_radius = target_radius;
            options.max_radius = 100.0;
            options.start_angle = Some(start_angle);
            options.source_to_path_to_dest_id = Some(self.object_id);
            if !owner_airborne {
                options.max_z_delta = 10.0;
            } else {
                options.ignore_object_id = Some(task.target_id);
            }
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&target_pos, &options, &mut dock_pos)
                {
                    task.dock_point = Some(dock_pos);
                }
            }
            if task.dock_point.is_none() {
                task.dock_point = Some(dock_pos);
            }
            self.action_state = DozerActionState::MoveToActionPos;
        }

        let dock_pos = task.dock_point.unwrap_or(target_pos);
        let delta = owner_pos - dock_pos;
        let dist_sq = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;

        match self.action_state {
            DozerActionState::MoveToActionPos => {
                if dist_sq <= MIN_ACTION_TOLERANCE * MIN_ACTION_TOLERANCE {
                    self.action_state = DozerActionState::DoAction;
                } else if let Some(ai) = owner_ai_update.as_ref() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_movement_target(&dock_pos);
                    }
                }
            }
            DozerActionState::DoAction => match task.task_type {
                DozerTask::Repair => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.internal_task_complete(DozerTask::Repair);
                        return;
                    }
                    if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                        if !ActionManager::can_repair_object(
                            &*owner_guard,
                            &*target_guard,
                            CommandSourceType::FromAi,
                        ) {
                            self.internal_task_complete(DozerTask::Repair);
                            return;
                        }
                    }
                    if let Some(body) = target_body.as_ref() {
                        if let Ok(mut body_guard) = body.lock() {
                            let max_health = body_guard.get_max_health();
                            let current = body_guard.get_health();
                            if max_health > 0.0 {
                                let delta = max_health
                                    * self.get_repair_health_per_second()
                                    * SECONDS_PER_LOGICFRAME_REAL;
                                let new_health = (current + delta).min(max_health);
                                let _ = body_guard.set_health(new_health);
                                if new_health >= max_health {
                                    if target_is_bridge_tower {
                                        self.remove_bridge_scaffolding(task.target_id);
                                    }
                                    self.internal_task_complete(DozerTask::Repair);
                                    return;
                                }
                            }
                        } else {
                            self.internal_task_complete(DozerTask::Repair);
                            return;
                        }
                    } else {
                        self.internal_task_complete(DozerTask::Repair);
                        return;
                    }
                }
                DozerTask::Build => {
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.internal_task_complete(DozerTask::Build);
                        return;
                    }
                    let manager_handle = get_construction_manager();
                    let mut manager = match manager_handle.write() {
                        Ok(manager) => manager,
                        Err(_) => {
                            return;
                        }
                    };
                    if !task.started_construction {
                        let max_health = if task.build_max_health > 0.0 {
                            task.build_max_health
                        } else {
                            target_body_max
                        };
                        let _ = manager.start_construction(
                            task.target_id,
                            self.object_id,
                            max_health,
                            task.build_total_frames.max(1),
                            task.is_rebuild,
                        );
                        task.started_construction = true;
                        if let Ok(mut owner_write) = owner.write() {
                            owner_write
                                .set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                        }
                    }
                    let completed = manager.update_for_dozer(self.object_id);
                    let progress = manager.get_progress(task.target_id).unwrap_or(0.0);
                    let current_health = manager.get_current_health(task.target_id);
                    if let Ok(mut target_write) = target.write() {
                        target_write.set_construction_percent(progress);
                        if let Some(health) = current_health {
                            let _ = target_write.set_health(health);
                        }
                    }
                    if completed.contains(&task.target_id) {
                        self.handle_build_completion(&owner, &target, task.is_rebuild);
                        self.internal_task_complete(DozerTask::Build);
                        return;
                    }
                }
                DozerTask::Fortify | DozerTask::Invalid => {
                    self.internal_task_complete(task.task_type);
                    return;
                }
            },
            DozerActionState::PickActionPos => {}
        }

        self.dozer_task = Some(task);
    }

    fn get_bridge_id_for_tower(tower: &Object) -> ObjectID {
        for module_handle in tower.behavior_modules() {
            let bridge_id = module_handle.with_module(|module| {
                module
                    .get_bridge_tower_control_interface()
                    .map(|tower| tower.bridge_id())
            });
            if let Some(bridge_id) = bridge_id {
                return bridge_id;
            }
        }

        for behavior in tower.get_behavior_modules() {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(interface) = guard.get_bridge_tower_behavior_interface() {
                return interface.get_bridge_id();
            }
        }
        INVALID_ID
    }

    fn remove_bridge_scaffolding(&self, bridge_tower_id: ObjectID) {
        let Some(tower_obj) = TheGameLogic::find_object_by_id(bridge_tower_id) else {
            return;
        };
        let Ok(tower_guard) = tower_obj.read() else {
            return;
        };
        let mut bridge_id: Option<ObjectID> = None;
        for module_handle in tower_guard.behavior_modules() {
            bridge_id = module_handle.with_module(|module| {
                module
                    .get_bridge_tower_control_interface()
                    .map(|tower| tower.bridge_id())
            });
            if bridge_id.is_some() {
                break;
            }
        }

        if bridge_id.is_none() {
            for behavior in tower_guard.get_behavior_modules() {
                let Ok(mut behavior_guard) = behavior.lock() else {
                    continue;
                };
                if let Some(interface) = behavior_guard.get_bridge_tower_behavior_interface() {
                    bridge_id = Some(interface.get_bridge_id());
                    break;
                }
            }
        }
        let Some(bridge_id) = bridge_id else {
            return;
        };
        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return;
        };
        let mut removed = false;
        for module_handle in bridge_guard.behavior_modules() {
            let matched = module_handle.with_module(|module| {
                if let Some(bridge) = module.get_bridge_control_interface() {
                    if let Err(err) = bridge.remove_scaffolding() {
                        log::debug!(
                            "DozerAIUpdate::remove_bridge_scaffolding failed for bridge {}: {}",
                            bridge_id,
                            err
                        );
                    }
                    true
                } else {
                    false
                }
            });
            if matched {
                removed = true;
                break;
            }
        }
        if !removed {
            for behavior in bridge_guard.get_behavior_modules() {
                let Ok(mut behavior_guard) = behavior.lock() else {
                    continue;
                };
                if let Some(interface) = behavior_guard.get_bridge_behavior_interface() {
                    interface.remove_scaffolding();
                    break;
                }
            }
        }
    }

    pub fn find_good_build_or_repair_position(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
    ) -> bool {
        let mut working = *target.get_position();
        let mut best = working;
        let mut offset = *me.get_position() - *target.get_position();
        let len = (offset.x * offset.x + offset.y * offset.y + offset.z * offset.z).sqrt();
        if len > 0.0 {
            offset.x /= len;
            offset.y /= len;
            offset.z /= len;
        }
        let scale = target.get_geometry_info().get_major_radius() / 2.0;
        working.x += offset.x * scale;
        working.y += offset.y * scale;
        working.z += offset.z * scale;

        let mut options = FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = 100.0;
        options.source_to_path_to_dest_id = Some(me.get_id());
        if !me.is_using_airborne_locomotor() {
            options.max_z_delta = 10.0;
        }
        if me.is_using_airborne_locomotor() {
            options.ignore_object_id = Some(target.get_id());
        }

        let found = if let Some(partition) = ThePartitionManager::get() {
            partition.find_position_around_with_options(&working, &options, &mut best)
        } else {
            false
        };
        *position_out = if found { best } else { working };
        found
    }

    pub fn find_good_build_or_repair_position_and_target(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
    ) -> Option<ObjectID> {
        if target.is_kind_of(KindOf::Bridge) {
            let mut best_dist_sq = f32::MAX;
            let mut best_tower: Option<ObjectID> = None;
            for tower_id in Self::get_bridge_tower_ids(target) {
                let Some(tower) = TheGameLogic::find_object_by_id(tower_id) else {
                    continue;
                };
                let Ok(tower_guard) = tower.read() else {
                    continue;
                };
                let mut tmp = Coord3D::ZERO;
                if Self::find_good_build_or_repair_position(me, &tower_guard, &mut tmp) {
                    let dx = me.get_position().x - tmp.x;
                    let dy = me.get_position().y - tmp.y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < best_dist_sq {
                        best_dist_sq = dist_sq;
                        *position_out = tmp;
                        best_tower = Some(tower_guard.get_id());
                    }
                }
            }
            if let Some(tower_id) = best_tower {
                return Some(tower_id);
            }
        }
        let _ = Self::find_good_build_or_repair_position(me, target, position_out);
        Some(target.get_id())
    }

    fn get_bridge_tower_ids(target: &Object) -> Vec<ObjectID> {
        for module_handle in target.behavior_modules() {
            let ids = module_handle.with_module(|module| {
                module.get_bridge_control_interface().map(|bridge| {
                    bridge
                        .tower_ids()
                        .into_iter()
                        .filter(|id| *id != INVALID_ID)
                        .collect::<Vec<_>>()
                })
            });
            if let Some(ids) = ids {
                return ids;
            }
        }

        for behavior in target.get_behavior_modules() {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if let Some(interface) = guard.get_bridge_behavior_interface() {
                let mut ids = Vec::new();
                for tower_type in [
                    BridgeTowerType::North,
                    BridgeTowerType::South,
                    BridgeTowerType::East,
                    BridgeTowerType::West,
                ] {
                    let id = interface.get_tower_id(tower_type);
                    if id != INVALID_ID {
                        ids.push(id);
                    }
                }
                return ids;
            }
        }
        Vec::new()
    }
}

impl crate::modules::DozerAIUpdateInterface for DozerAIUpdate {
    fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
    ) {
        self.set_build_task(building_id, total_build_frames, max_health, is_rebuild);
    }

    fn cancel_task(&mut self, task: DozerTask) {
        self.cancel_task(task);
    }

    fn is_task_pending(&self, task: DozerTask) -> bool {
        self.is_task_pending(task)
    }

    fn is_any_task_pending(&self) -> bool {
        self.is_any_task_pending()
    }
}

/// Module wrapper for DozerAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct DozerAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<DozerAIUpdateModuleData>,
}

impl DozerAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<DozerAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for DozerAIUpdateModule {
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

impl Snapshotable for DozerAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LocomotorSetType;

    fn parse_field(data: &mut DozerAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = DOZER_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn dozer_fields_accept_ini_equals_token() {
        let mut data = DozerAIUpdateModuleData::default();

        parse_field(
            &mut data,
            "AutoAcquireEnemiesWhenIdle",
            &["=", "YES", "ATTACK_BUILDINGS"],
        );
        parse_field(
            &mut data,
            "Locomotor",
            &["=", "SET_NORMAL", "DozerLocomotor"],
        );
        parse_field(&mut data, "MoodAttackCheckRate", &["=", "2000"]);
        parse_field(&mut data, "SurrenderDuration", &["=", "3000"]);
        parse_field(&mut data, "ForbidPlayerCommands", &["=", "Yes"]);
        parse_field(&mut data, "TurretsLinked", &["=", "Yes"]);
        parse_field(&mut data, "RepairHealthPercentPerSecond", &["=", "25%"]);
        parse_field(&mut data, "BoredTime", &["=", "1500"]);
        parse_field(&mut data, "BoredRange", &["=", "125.5"]);

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.repair_health_percent_per_second, 0.25);
        assert_eq!(data.bored_time, 45.0);
        assert_eq!(data.bored_range, 125.5);
    }

    #[test]
    fn parse_duration_real_field_accepts_duration_suffixes() {
        let mut parsed = 0.0;
        parse_duration_real_field(&mut |value| parsed = value, &["1500ms"]).expect("duration");
        assert!((parsed - 45.0).abs() < f32::EPSILON);

        parse_duration_real_field(&mut |value| parsed = value, &["1.5s"]).expect("duration");
        assert!((parsed - 45.0).abs() < f32::EPSILON);
    }
}
