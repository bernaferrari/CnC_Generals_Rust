//! Native32 source-identified requested/entered task continuation.
use super::*;
use crate::game_logic::{GameLogic, Object, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DozerTaskObjectSnapshot {
    pub source: Option<(u32, String)>,
    pub requests: [(u32, u32); 3],
    pub current: i32,
    pub primary: gamelogic::object::update::ai_update::dozer_ai_update::owned_state::PrimaryMachine,
    pub build_sub_task: i32,
    pub docks: [[(bool, [f32; 3]); 3]; 3],
}
fn bad(s: &str) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("Dozer task owner: {s}"))
}
// The receiving host uses XZ ground coordinates; original Core/C++ uses XY.
// Keep this conversion at the serialized module boundary, not in the owner.
type Primary = gamelogic::object::update::ai_update::dozer_ai_update::owned_state::PrimaryMachine;
fn save_primary(value: &Primary) -> SaveLoadResult<Vec<u8>> {
    let mut native = value.clone();
    let p = native.control.goal_position;
    native.control.goal_position = glam::Vec3::new(p.x, p.z, p.y);
    native.save_original_control_bytes().map_err(|e| bad(&e))
}
fn load_primary(bytes: &[u8]) -> SaveLoadResult<Primary> {
    let mut value = Primary::load_original_control_bytes(bytes).map_err(|e| bad(&e))?;
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
        let mut primary =
            if self.source.is_some() && x.get_mode() != crate::save_load::XferMode::Load {
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
fn payload(o: &ObjectSnapshot) -> SaveLoadResult<Option<&DozerTaskObjectSnapshot>> {
    let mut found = None;
    for (name, module) in &o.modules {
        if let ModuleSnapshot::DozerTasks(data) = module {
            if name != "ReceivingDozerTasks" || found.is_some() {
                return Err(bad("duplicate/foreign source payload"));
            }
            found = Some(data);
        }
    }
    Ok(found)
}
pub(super) fn validate(o: &ObjectSnapshot, w: &GameLogic) -> SaveLoadResult<()> {
    let expected = w
        .templates
        .get(&o.template_name)
        .and_then(|t| t.receiving_client_template.as_ref())
        .and_then(|d| d.dozer_ai_identity());
    let Some(saved) = payload(o)? else {
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
    use gamelogic::object::update::ai_update::dozer_ai_update::owned_state::PrimaryState;
    let entered = match saved.primary.state {
        PrimaryState::Build => 0,
        PrimaryState::Repair => 1,
        PrimaryState::Fortify => 2,
        _ => -1,
    };
    if entered != saved.current {
        return Err(bad("primary/current task mismatch"));
    }
    if !(-1..=2).contains(&saved.current) {
        return Err(bad("invalid entered task"));
    }
    Ok(())
}
pub(super) fn apply_after_admission(o: &ObjectSnapshot, w: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(saved) = payload(o)? else {
        return Ok(());
    };
    // Decode against fresh receiving action definitions before touching the
    // admitted actor, including direct in-memory snapshot callers.
    let primary = if saved.source.is_some() {
        load_primary(&save_primary(&saved.primary)?)?
    } else {
        saved.primary.clone()
    };
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
    use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;
    object.dozer_tasks.primary = primary;
    use gamelogic::object::update::ai_update::dozer_ai_update::DozerBuildSubTask;
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
