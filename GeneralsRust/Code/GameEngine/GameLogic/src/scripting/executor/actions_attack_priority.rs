//! Attack-priority sets, object lists, unmanned cleanup, and related world flags
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use crate::object::registry::OBJECT_REGISTRY;
use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // ATTACK PRIORITY ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_set_attack_priority_thing(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let type_or_list = self.get_string_param(action, 1)?;
        let priority = self.get_int_param(action, 2)?;
        let _ = with_script_engine_mut(|engine| {
            engine.set_priority_thing(&priority_set, &type_or_list, priority)
        });
        log::debug!(
            "Setting attack priority '{}' on '{}' to {}",
            priority_set,
            type_or_list,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_attack_priority_kindof(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let kind_name = self.get_string_param(action, 1)?;
        let priority = self.get_int_param(action, 2)?;
        if let Some(kind) = parse_kind_of(&kind_name) {
            let _ = with_script_engine_mut(|engine| {
                engine.set_priority_kind(&priority_set, kind, priority)
            });
        }
        log::debug!(
            "Setting attack priority '{}' for kindof '{}' to {}",
            priority_set,
            kind_name,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_default_attack_priority(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let priority_set = self.get_string_param(action, 0)?;
        let priority = self.get_int_param(action, 1)?;
        let _ =
            with_script_engine_mut(|engine| engine.set_priority_default(&priority_set, priority));
        log::debug!(
            "Setting default attack priority '{}' to {}",
            priority_set,
            priority
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_stopping_distance(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let distance = self.get_real_param(action, 1)?;
        log::debug!(
            "Setting team '{}' stopping distance to {}",
            team_name,
            distance
        );
        if super::dual_world_registry_unavailable() {
            super::request_host_script_stopping_distance(
                super::HostScriptStoppingDistanceRequest::Team {
                    team: team_name,
                    distance,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        if distance < 0.5 {
            return Ok(ScriptActionResult::Success);
        }

        // C++ walks the team's members and adjusts each current locomotor.
        let members = self
            .get_team_by_name(&team_name)
            .ok()
            .and_then(|team_id| {
                crate::team::with_team(team_id, |team| team.get_members().to_vec())
            })
            .unwrap_or_default();

        for member_id in members {
            let mut has_loco = false;
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface_mut() else {
                    return;
                };
                ai.with_cur_locomotor(&mut |loco| {
                    has_loco = true;
                    loco.set_close_enough_dist(distance);
                });
            });
            if !has_loco {
                return Ok(ScriptActionResult::Success);
            }
        }

        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // OBJECT LIST ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_object_list_add_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let list_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let _ = with_script_engine_mut(|engine| {
            let list_key = list_name.to_string();
            let mut list = engine
                .get_object_types(&list_key)
                .unwrap_or_else(|| ObjectTypes::with_list_name(AsciiString::from(&list_key)));
            list.add_object_type(AsciiString::from(object_type.as_str()));
            engine.set_object_types(list_key, list);
        });
        log::debug!("Adding '{}' to object list '{}'", object_type, list_name);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_object_list_remove_object_type(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let list_name = self.get_string_param(action, 0)?;
        let object_type = self.get_string_param(action, 1)?;
        let _ = with_script_engine_mut(|engine| {
            if let Some(mut list) = engine.get_object_types(&list_name) {
                list.remove_object_type(&AsciiString::from(object_type.as_str()));
                engine.set_object_types(list_name.to_string(), list);
            }
        });
        log::debug!(
            "Removing '{}' from object list '{}'",
            object_type,
            list_name
        );
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_object_allow_bonuses(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let allow = self.get_int_param(action, 0)? != 0;
        let _ = with_script_engine_mut(|engine| {
            engine.set_objects_should_receive_difficulty_bonus(allow);
        });
        log::debug!("Object allow bonuses: {}", allow);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_delete_all_unmanned(
        &mut self,
        _action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // Wave 284: empty dual-world → live host drain.
        if dual_world_registry_unavailable() {
            super::request_host_script_unmanned(super::HostScriptUnmannedRequest::DeleteAll);
            return Ok(ScriptActionResult::Success);
        }

        // Host path: empty dual-world registry → nothing unmanned to delete.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(ScriptActionResult::Success);
        }
        let mut to_destroy = Vec::new();
        OBJECT_REGISTRY.with_each(|id, guard| {
            if guard.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                to_destroy.push(id);
            }
        });
        if !to_destroy.is_empty() {
            if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
                for obj_id in to_destroy {
                    logic.destroy_object(obj_id);
                }
            }
        }
        log::debug!("Deleting all unmanned");
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_choose_victim_always_uses_normal(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let use_normal = self.get_int_param(action, 0)? != 0;
        let _ = with_script_engine_mut(|engine| {
            engine.set_choose_victim_always_uses_normal(use_normal);
        });
        log::debug!("Choose victim always uses normal: {}", use_normal);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_scripting_override_hulk_lifetime(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // C++ ScriptActions::doOverrideHulkLifetime(Real seconds):
        // seconds < 0 → -1 (off); else frames = seconds * LOGICFRAMES_PER_SECOND.
        let seconds = match self.get_real_param(action, 0) {
            Ok(real) if real != 0.0 => real,
            _ => self.get_int_param(action, 0)? as f32,
        };
        let frames = if seconds < 0.0 {
            -1
        } else {
            (seconds * crate::common::LOGICFRAMES_PER_SECOND as f32) as i32
        };
        log::debug!(
            "Scripting override hulk lifetime to {}s ({} frames)",
            seconds,
            frames
        );
        TheGameLogic::set_hulk_max_lifetime_override(frames);
        Ok(ScriptActionResult::Success)
    }
}
