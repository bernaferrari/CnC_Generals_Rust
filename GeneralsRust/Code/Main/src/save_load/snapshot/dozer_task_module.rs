//! Native32 source-identified requested/entered task continuation.
use super::*;
use crate::game_logic::{GameLogic, Object, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use gamelogic::object::update::ai_update::dozer_ai_update::{
    DozerBuildSubTask, DozerTask,
    owned_state::{ActionMachine, ActionState, PrimaryMachine, PrimaryState},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DozerTaskObjectSnapshot {
    pub source: Option<(u32, String)>,
    pub requests: [(u32, u32); 3],
    pub current: i32,
    pub primary: PrimaryMachine,
    pub build_sub_task: i32,
    pub docks: [[(bool, [f32; 3]); 3]; 3],
}

/// DozerActionStateMachine::xfer transfers only the task (C++ 820–831).
/// Rust's native save also keeps the entered action so its next tick can run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDozerTaskSnapshot {
    tasks: DozerTaskObjectSnapshot,
    action: ActionMachine,
}

impl ActiveDozerTaskSnapshot {
    pub(super) fn capture_module(object: &Object) -> SaveLoadResult<ModuleSnapshot> {
        let tasks = DozerTaskObjectSnapshot::capture(object)?;
        let current = object.dozer_tasks.current;
        if tasks.source.is_none() || current == DozerTask::Invalid {
            return Ok(ModuleSnapshot::DozerTasks(tasks));
        }
        let action = object.dozer_tasks.primary.actions[current as usize].clone();
        tasks.validate_active_action(&action)?;
        Ok(ModuleSnapshot::ActiveDozerTask(Self { tasks, action }))
    }

    pub(super) fn write_xfer(&mut self, x: &mut dyn Xfer) -> SaveLoadResult<()> {
        self.tasks.validate_active_action(&self.action)?;
        let mut body = serde_json::to_string(&self.action).map_err(bad)?;
        self.tasks.xfer(x)?;
        super::xfer_helpers::xfer_ai_continuation_string(x, &mut body)?;
        Ok(())
    }

    pub(super) fn read_xfer(x: &mut dyn Xfer) -> SaveLoadResult<Self> {
        let mut tasks = DozerTaskObjectSnapshot::default();
        tasks.xfer(x)?;
        let mut body = String::new();
        super::xfer_helpers::xfer_ai_continuation_string(x, &mut body)?;
        let action = serde_json::from_str(&body).map_err(bad)?;
        tasks.validate_active_action(&action)?;
        Ok(Self { tasks, action })
    }
}

fn bad(s: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("Dozer task owner: {s}"))
}
// The receiving host uses XZ ground coordinates; original Core/C++ uses XY.
// Keep this conversion at the serialized module boundary, not in the owner.
fn save_primary(value: &PrimaryMachine) -> SaveLoadResult<Vec<u8>> {
    let mut native = value.clone();
    let p = native.control.goal_position;
    native.control.goal_position = glam::Vec3::new(p.x, p.z, p.y);
    native.save_original_control_bytes().map_err(bad)
}
fn load_primary(bytes: &[u8]) -> SaveLoadResult<PrimaryMachine> {
    let mut value = PrimaryMachine::load_original_control_bytes(bytes).map_err(bad)?;
    let p = value.control.goal_position;
    value.control.goal_position = glam::Vec3::new(p.x, p.z, p.y);
    Ok(value)
}
impl XferData for DozerTaskObjectSnapshot {
    fn xfer(&mut self, x: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut present = self.source.is_some();
        x.xfer_bool(&mut present)?;
        if present {
            let (mut ordinal, mut tag) = self.source.clone().unwrap_or_default();
            x.xfer_u32(&mut ordinal)?;
            x.xfer_string(&mut tag)?;
            self.source = Some((ordinal, tag));
        } else {
            self.source = None;
        }
        let mut task_count = 3;
        x.xfer_i32(&mut task_count)?;
        if task_count != 3 {
            return Err(bad("invalid task count"));
        }
        for (target, frame) in &mut self.requests {
            x.xfer_u32(target)?;
            x.xfer_u32(frame)?;
        }
        // CPP primary state/control Xfer, shared with the native module. Nested
        // action Xfer writes its task only and restarts Pick on detached decode.
        let mut primary = if self.source.is_some() && x.get_mode() != XferMode::Load {
            save_primary(&self.primary)?
        } else {
            Vec::new()
        };
        let mut len = primary.len() as u32;
        x.xfer_u32(&mut len)?;
        if len > 65536 {
            return Err(bad("oversized primary control"));
        }
        primary.resize(len as usize, 0);
        x.xfer_raw(&mut primary)?;
        if self.source.is_some() {
            self.primary = load_primary(&primary)?;
        } else if !primary.is_empty() {
            return Err(bad("primary control without admitted Dozer source"));
        }
        x.xfer_i32(&mut self.current)?;
        let mut dock_count = 3;
        x.xfer_i32(&mut dock_count)?;
        if dock_count != 3 {
            return Err(bad("invalid dock count"));
        }
        for task in &mut self.docks {
            for (valid, xyz) in task {
                x.xfer_bool(valid)?;
                // CPP2461–2467 writes location even when valid is false.
                for axis in xyz {
                    x.xfer_f32(axis)?;
                }
            }
        }
        x.xfer_i32(&mut self.build_sub_task)?;
        if !(0..=2).contains(&self.build_sub_task) {
            return Err(bad("invalid build subtask"));
        }
        if !(-1..=2).contains(&self.current) {
            return Err(bad("invalid entered task"));
        }
        Ok(())
    }
}
impl DozerTaskObjectSnapshot {
    fn entered_task(&self) -> DozerTask {
        match self.primary.state {
            PrimaryState::Build => DozerTask::Build,
            PrimaryState::Repair => DozerTask::Repair,
            PrimaryState::Fortify => DozerTask::Fortify,
            _ => DozerTask::Invalid,
        }
    }

    fn validate_active_action(&self, action: &ActionMachine) -> SaveLoadResult<()> {
        let task = self.entered_task();
        if self.source.is_none()
            || task == DozerTask::Invalid
            || self.current != task as i32
            || action.task != task
            || !action.control.initialized
            || action.control.default_id != 0
            || !action.control.goal_position.is_finite()
            || (!matches!(action.state, ActionState::PickPosition { .. })
                && action.control.goal_object == 0)
        {
            return Err(bad("invalid active action continuation"));
        }
        // The next action tick handles a goal actor that no longer exists.
        Ok(())
    }

    pub(super) fn capture(o: &Object) -> SaveLoadResult<Self> {
        let source = o
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|d| d.dozer_ai_identity());
        let primary = if source.is_some() {
            load_primary(&save_primary(&o.dozer_tasks.primary)?)?
        } else {
            Default::default()
        };
        Ok(Self {
            source,
            requests: std::array::from_fn(|i| {
                (
                    o.dozer_tasks.requests[i].target.map_or(0, |id| id.0),
                    o.dozer_tasks.requests[i].order_frame,
                )
            }),
            current: o.dozer_tasks.current as i32,
            primary,
            build_sub_task: o.dozer_tasks.build_sub_task as i32,
            docks: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let dock = o.dozer_tasks.docks[i][j];
                    (
                        dock.valid,
                        [dock.location.x, dock.location.z, dock.location.y],
                    )
                })
            }),
        })
    }
}
fn payload(
    o: &ObjectSnapshot,
) -> SaveLoadResult<Option<(&DozerTaskObjectSnapshot, Option<&ActionMachine>)>> {
    let mut found = None;
    for (name, module) in &o.modules {
        let data = match module {
            ModuleSnapshot::DozerTasks(tasks) => (tasks, None),
            ModuleSnapshot::ActiveDozerTask(active) => (&active.tasks, Some(&active.action)),
            _ => continue,
        };
        if name != "ReceivingDozerTasks" || found.is_some() {
            return Err(bad("duplicate/foreign source payload"));
        }
        found = Some(data);
    }
    Ok(found)
}
pub(super) fn validate(o: &ObjectSnapshot, w: &GameLogic) -> SaveLoadResult<()> {
    let expected = w
        .templates
        .get(&o.template_name)
        .and_then(|t| t.receiving_client_template.as_ref())
        .and_then(|d| d.dozer_ai_identity());
    let Some((saved, action)) = payload(o)? else {
        return if expected.is_none() {
            Ok(())
        } else {
            Err(bad("missing exact task continuation"))
        };
    };
    if saved.source != expected {
        return Err(bad("source ordinal/tag mismatch"));
    }
    if !(0..=2).contains(&saved.build_sub_task) {
        return Err(bad("invalid build subtask"));
    }
    if saved.source.is_some() {
        load_primary(&save_primary(&saved.primary)?)?;
    }
    if saved.entered_task() as i32 != saved.current {
        return Err(bad("primary/current task mismatch"));
    }
    if let Some(action) = action {
        saved.validate_active_action(action)?;
    }
    Ok(())
}
pub(super) fn apply_after_admission(o: &ObjectSnapshot, w: &mut GameLogic) -> SaveLoadResult<()> {
    let Some((saved, action)) = payload(o)? else {
        return Ok(());
    };
    // Decode against fresh receiving action definitions before touching the
    // admitted actor, including direct in-memory snapshot callers.
    let mut primary = if saved.source.is_some() {
        load_primary(&save_primary(&saved.primary)?)?
    } else {
        saved.primary.clone()
    };
    if let Some(action) = action {
        saved.validate_active_action(action)?;
        primary.actions[action.task as usize] = action.clone();
    }
    let object = w
        .host_object_mut(o.id)
        .ok_or_else(|| bad("admitted actor absent"))?;
    for i in 0..3 {
        object.dozer_tasks.requests[i].target =
            (saved.requests[i].0 != 0).then_some(ObjectId(saved.requests[i].0));
        object.dozer_tasks.requests[i].order_frame = saved.requests[i].1;
        for j in 0..3 {
            let (valid, v) = saved.docks[i][j];
            object.dozer_tasks.docks[i][j] =
                gamelogic::object::update::ai_update::dozer_ai_update::DozerDockPointInfo {
                    valid,
                    location: glam::Vec3::new(v[0], v[2], v[1]),
                };
        }
    }
    object.dozer_tasks.primary = primary;
    object.dozer_tasks.build_sub_task = match saved.build_sub_task {
        0 => DozerBuildSubTask::SelectBuildDockLocation,
        1 => DozerBuildSubTask::MovingToBuildDockLocation,
        2 => DozerBuildSubTask::DoBuildAtDock,
        _ => return Err(bad("invalid build subtask")),
    };
    object.dozer_tasks.current = match saved.current {
        -1 => DozerTask::Invalid,
        0 => DozerTask::Build,
        1 => DozerTask::Repair,
        2 => DozerTask::Fortify,
        _ => return Err(bad("invalid entered task")),
    };
    Ok(())
}

#[cfg(test)]
mod original_xfer_component_tests;

#[cfg(test)]
mod receiving_restore_tests;
