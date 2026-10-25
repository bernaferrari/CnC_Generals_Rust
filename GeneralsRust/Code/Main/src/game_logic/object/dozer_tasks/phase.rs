//! Receiving phases of the shared native Dozer machine. Each synchronous
//! self-command runs with the actor and its authoritative owner installed.
use super::*;
use gamelogic::object::update::ai_update::dozer_ai_update::{
    DozerBuildSubTask,
    owned_state::{ActionState, PrimaryState},
};

impl GameLogic {
    /// Exact REPAIR/RESUME aiDoCommand preamble (CPP2357/2371). A busy
    /// entered task is not idled; a request itself never unconditionally idles.
    pub(crate) fn dozer_prepare_repair_or_resume_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.dozer_tasks.current == DozerTask::Invalid)
        {
            let _ = self.dozer_inherited_ai_idle_owned(id)?;
        }
        Ok(())
    }

    /// DozerAIUpdate::aiDoCommand default -> AIUpdateInterface::aiDoCommand.
    /// This is individual inherited Idle, not group recursion or another AI class.
    pub(crate) fn dozer_inherited_ai_idle_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_standard_ai_idle_owned(id)
    }

    fn dozer_repair_target_admitted_owned(&self, id: ObjectId) -> bool {
        let Some(actor) = self.objects.get(&id) else {
            return false;
        };
        let Some(target) = actor.dozer_tasks.requests[1]
            .target
            .and_then(|t| self.objects.get(&t))
        else {
            return false;
        };
        actor.is_kind_of(KindOf::Dozer)
            && !actor.status.under_construction
            && actor.contained_by.is_none()
            && !target.status.effectively_dead
            && target.is_kind_of(KindOf::Structure)
            && !target.is_kind_of(KindOf::Bridge)
            && !target.is_kind_of(KindOf::BridgeTower)
            && !target.is_rebuild_hole
            && !target.status.under_construction
            && target.health.current != target.health.maximum
            && self.repair_relationship_is_not_enemy(id, target.id)
            && (actor.last_command_source
                == crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
                || !self.is_enter_target_shrouded_for_action(id, target.id))
    }

    pub(crate) fn dozer_primary_reset_owned(&mut self, id: ObjectId) {
        if let Some(actor) = self.objects.get_mut(&id) {
            actor.set_weapon_set_mine_clearing_detail_at_frame(true, self.frame);
            actor.dozer_tasks.finish_primary_reset(self.frame);
        }
    }

    /// One original derived-update phase, after the inherited AI update. Entry
    /// only changes the primary; Pick executes on its next update, as in CPP.
    pub(crate) fn dozer_update_primary_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.dozer_update_primary_owned_with_effects(id, &mut Default::default())
    }
    pub(in crate::game_logic) fn dozer_update_primary_owned_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((state, current, idle, dead, unmanned)) = self.objects.get(&id).and_then(|o| {
            (o.dozer_tasks.primary.control.initialized
                || o.dozer_tasks.requests.iter().any(|r| r.target.is_some()))
            .then_some((
                o.dozer_tasks.primary.state,
                o.dozer_tasks.current,
                o.ai_state() == AIState::Idle,
                o.status.effectively_dead,
                o.status.disabled_unmanned,
            ))
        }) else {
            return Ok(());
        };
        if dead {
            return Ok(());
        }
        // GameLogic.cpp3718 uses ANY intersection, including HELD combined
        // with another disable; AIUpdate.h282 processes DISABLED_HELD only.
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.has_disabled_type() && !o.status.disabled_held)
        {
            return Ok(());
        }
        // DozerAIUpdate.cpp1585 validates active Repair before its primary.
        if current == DozerTask::Repair && !self.dozer_repair_target_admitted_owned(id) {
            let _ = self.dozer_cancel_entered_task_owned(id)?;
            return self.dozer_update_primary_owned_with_effects(id, effects);
        }
        if state == PrimaryState::Idle {
            if !idle {
                return Ok(());
            }
            let selected = self
                .objects
                .get(&id)
                .map(|o| {
                    o.dozer_tasks
                        .most_recent(|r| r.target.map(|_| r.order_frame))
                })
                .unwrap_or(DozerTask::Invalid);
            if selected != DozerTask::Invalid {
                if let Some(actor) = self.objects.get_mut(&id) {
                    actor.dozer_tasks.enter_primary_task(selected);
                }
            }
            return Ok(());
        }
        if current == DozerTask::Invalid {
            return Ok(());
        }
        use gamelogic::object::update::ai_update::dozer_ai_update::action_kernel::{
            ActionFacts, ActionStep,
        };
        let slot = current as usize;
        let Some((phase, goal, target, start, action, radius, position, birth)) =
            self.objects.get(&id).map(|o| {
                (
                    o.dozer_tasks.primary.actions[slot].state,
                    o.dozer_tasks.primary.actions[slot].control.goal_position,
                    if matches!(
                        o.dozer_tasks.primary.actions[slot].state,
                        ActionState::PickPosition { .. }
                    ) {
                        o.dozer_tasks.requests[slot].target
                    } else {
                        let raw = o.dozer_tasks.primary.actions[slot].control.goal_object;
                        (raw != 0).then_some(ObjectId(raw))
                    },
                    o.dozer_tasks.docks[slot][0].point(),
                    o.dozer_tasks.docks[slot][1].point(),
                    o.geometry_info().bounding_sphere_radius(),
                    o.get_position(),
                    std::sync::Arc::clone(&o.visual_draw_identity_token),
                )
            })
        else {
            return Ok(());
        };
        let mut facts = ActionFacts {
            frame: self.frame,
            regular_idle: idle,
            target: target.map_or(0, |t| t.0),
            target_exists: target.is_some_and(|t| self.objects.get(&t).is_some()),
            claim_valid: true,
            unmanned,
            picked_position: Vec3::ZERO,
            ignore_obstacle_count: 1,
            distance_squared: 0.0,
            actor_radius: radius,
        };
        match phase {
            ActionState::PickPosition { .. } => {
                if facts.target_exists {
                    facts.picked_position = match start {
                        Some(point) => point,
                        None => self.dozer_pick_action_position_owned(id, target.unwrap())?,
                    };
                    facts.ignore_obstacle_count = if start.is_some() { 1 } else { 2 };
                }
            }
            ActionState::MoveToPosition => {
                if let Some(target_object) = target.and_then(|t| self.objects.get(&t)) {
                    facts.claim_valid = match current {
                        DozerTask::Build => target_object.builder_id == Some(id),
                        DozerTask::Repair => {
                            target_object
                                .sole_healing_benefactor
                                .is_none_or(|healer| healer == id)
                                || self.frame
                                    > target_object.sole_healing_benefactor_expiration_frame
                        }
                        _ => true,
                    };
                    if facts.claim_valid {
                        let gap =
                            crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
                                position, radius, goal, 0.0,
                            );
                        facts.distance_squared = gap * gap;
                    }
                }
            }
            ActionState::DoAction { .. } => {}
        }
        let Some(step) = self
            .objects
            .get_mut(&id)
            .map(|o| o.dozer_tasks.prepare_action_phase(facts))
        else {
            return Ok(());
        };
        match step {
            ActionStep::Continue => {}
            ActionStep::CancelTask { .. } => {
                let _ = self.dozer_cancel_entered_task_owned(id)?;
            }
            ActionStep::ResetPrimary => self.dozer_primary_reset_owned(id),
            ActionStep::Move {
                task,
                target,
                position,
                ..
            } => {
                if let Some(actor) = self.objects.get_mut(&id) {
                    actor.target = Some(ObjectId(target));
                    actor.set_adjust_destinations(false);
                    actor.set_actively_constructing(false);
                    actor.last_command_source =
                        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                }
                self.path_approach_with_state_ignoring(
                    id,
                    position,
                    AIState::Moving,
                    Some(ObjectId(target)),
                );
                if let Some(actor) = self
                    .objects
                    .get_mut(&id)
                    .filter(|o| std::sync::Arc::ptr_eq(&birth, &o.visual_draw_identity_token))
                {
                    actor.dozer_tasks.finish_action_move(task, target, position);
                }
            }
            ActionStep::CompleteRequest { task, target } => {
                self.dozer_internal_task_complete(id, task == DozerTask::Repair);
                if let Some(actor) = self
                    .objects
                    .get_mut(&id)
                    .filter(|o| std::sync::Arc::ptr_eq(&birth, &o.visual_draw_identity_token))
                {
                    actor.dozer_tasks.finish_claim_failure(task, target);
                }
            }
            ActionStep::EnterAction {
                task,
                target,
                frame,
            } => {
                if task == DozerTask::Build {
                    if let Some(site) = self.objects.get_mut(&ObjectId(target)) {
                        site.set_under_construction_model_conditions(true);
                    }
                }
                if let Some(actor) = self
                    .objects
                    .get_mut(&id)
                    .filter(|o| std::sync::Arc::ptr_eq(&birth, &o.visual_draw_identity_token))
                {
                    actor.dozer_tasks.finish_enter_action(task, target, frame);
                }
            }
            ActionStep::RunAction { task, .. } => {
                if task == DozerTask::Build {
                    if target
                        .and_then(|target| self.objects.get(&target))
                        .is_some_and(|site| {
                            self.objects
                                .get(&id)
                                .is_some_and(|actor| actor.owner_player_id != site.owner_player_id)
                        })
                    {
                        self.dozer_primary_reset_owned(id); // original STATE_FAILURE retains the request
                        return Ok(());
                    }
                    let sub = self.objects.get(&id).unwrap().dozer_tasks.build_sub_task;
                    if sub == DozerBuildSubTask::SelectBuildDockLocation {
                        if let Some(destination) = action {
                            if let Some(actor) = self.objects.get_mut(&id) {
                                actor.set_actively_constructing(false);
                                actor.last_command_source =
                                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                            }
                            self.path_approach_with_state_ignoring(
                                id,
                                destination,
                                AIState::Moving,
                                target,
                            );
                        }
                        if let Some(actor) = self.objects.get_mut(&id) {
                            actor.dozer_tasks.build_sub_task =
                                DozerBuildSubTask::MovingToBuildDockLocation;
                        }
                    }
                    let arrived = self.objects.get(&id).is_some_and(|o| {
                        o.ai_state() == AIState::Idle
                            && o.dozer_tasks.build_sub_task
                                == DozerBuildSubTask::MovingToBuildDockLocation
                    });
                    if arrived {
                        if let Some(actor) = self.objects.get_mut(&id) {
                            actor.dozer_tasks.build_sub_task = DozerBuildSubTask::DoBuildAtDock;
                        }
                        if let Some(site) = target.and_then(|target| self.objects.get(&target)) {
                            let (site_id, name, pos) =
                                (site.id, site.template_name.clone(), site.get_position());
                            self.start_building_sound(site_id, &name, pos);
                        }
                    }
                    if let Some(actor) = self.objects.get_mut(&id) {
                        if actor.dozer_tasks.build_sub_task == DozerBuildSubTask::DoBuildAtDock {
                            // CPP DozerAIUpdate.cpp507: the builder's animation
                            // changes here; its inherited base AI remains Idle.
                            actor.set_actively_constructing(true);
                        }
                    }
                    if self.objects.get(&id).is_some_and(|o| {
                        o.dozer_tasks.build_sub_task == DozerBuildSubTask::DoBuildAtDock
                    }) {
                        if let Some(site) = target {
                            if self.dozer_build_action_frame_owned(id, site, effects)? {
                                return Ok(()); // success tail already set mine-clearing after internalTaskComplete
                            }
                        }
                    }
                }
                // CPP725 Fortify remains intentionally empty.
                if let Some(actor) = self.objects.get_mut(&id) {
                    actor.set_weapon_set_mine_clearing_detail_at_frame(false, self.frame);
                }
            }
        }
        Ok(())
    }
}
