//! Original per-builder BUILD contribution and synchronous completion.
use super::*;
impl GameLogic {
    pub(in crate::game_logic) fn dozer_build_action_frame_owned(
        &mut self,
        builder: ObjectId,
        site: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.dozer_build_action_frame_observed(builder, site, effects, |_, _, _| {})
    }
    fn dozer_build_action_frame_observed(
        &mut self,
        builder: ObjectId,
        site: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        mut after_completion: impl FnMut(&GameLogic, ObjectId, ObjectId),
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(actor) = self.objects.get(&builder) else {
            return Ok(false);
        };
        let builder_birth = std::sync::Arc::clone(&actor.visual_draw_identity_token);
        let builder_owner = actor.owner_player_id;
        let Some(target) = self.objects.get(&site) else {
            return Ok(false);
        };
        if !target.status.under_construction
            || target.builder_id.is_some_and(|owner| owner != builder)
        {
            return Ok(false);
        }
        let site_owner = target.owner_player_id;
        if builder_owner != site_owner {
            return Ok(false);
        } // CPP478 controller equality, not alliance
        let site_birth = std::sync::Arc::clone(&target.visual_draw_identity_token);
        let handicap = builder_owner
            .and_then(|owner| self.players.get(&owner))
            .map(|owner| owner.handicap_build_time_multiplier(target.is_kind_of(KindOf::Structure)))
            .unwrap_or(1.);
        let template_factor = builder_owner
            .map(|owner| self.player_template_production_time_factor(owner, &target.template_name))
            .unwrap_or(1.);
        let authored = Self::cpp_build_time_frames_from_modifiers(
            target.thing().template.build_time,
            handicap,
            template_factor,
        );
        let power = builder_owner
            .and_then(|owner| self.compute_player_power_factors().get(&owner).copied())
            .unwrap_or(1.);
        let frames = Self::cpp_build_time_frames_after_power(authored, power).max(1) as f32;
        let target = self.objects.get_mut(&site).expect("borrowed site");
        target.set_under_construction_model_conditions(true);
        self.construction_model_condition_updates =
            self.construction_model_condition_updates.saturating_add(1);
        // CPP510-536: percent, raw health, then producer and builder, one logic frame.
        target.construction_percent = (target.construction_percent + 1. / frames).min(1.);
        target.internal_change_health(target.health.maximum / frames);
        self.health_events
            .record_applied_heal(site, target.health.current);
        target.producer_id = Some(builder);
        target.builder_id = Some(builder);
        if target.construction_percent < 1. {
            return Ok(false);
        }
        self.construction_events
            .record(site, target.template_name.clone());
        self.notify_structure_construction_complete_from_builder_with_effects(
            site, builder, effects,
        )?;
        let stable = |world: &GameLogic| {
            world.objects.get(&builder).is_some_and(|o| {
                std::sync::Arc::ptr_eq(&builder_birth, &o.visual_draw_identity_token)
            }) && world
                .objects
                .get(&site)
                .is_some_and(|o| std::sync::Arc::ptr_eq(&site_birth, &o.visual_draw_identity_token))
        };
        if !stable(self) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Dozer BUILD lifetime changed during completion".into(),
            ));
        }
        after_completion(self, builder, site);
        // CPP628-641 reads the live end dock AFTER every completion callback.
        let actor = self.objects.get(&builder).unwrap();
        let mut end = actor.dozer_tasks.docks[0][2]
            .point()
            .unwrap_or_else(|| actor.get_position());
        self.adjust_to_possible_destination(builder, &mut end);
        self.dozer_clear_actively_constructing_on_command(builder);
        let _ = self.unit_command_ai_move_to(builder, end); // original void command may refuse
        if !stable(self) {
            return Err(gamelogic::GameLogicError::Configuration(
                "Dozer BUILD lifetime changed during end move".into(),
            ));
        }
        self.dozer_internal_task_complete(builder, false);
        if let Some(actor) = self.objects.get_mut(&builder) {
            actor.dozer_tasks.primary.actions[0].control.goal_object = 0; // CPP751, before success MineClearing
        }
        self.dozer_primary_reset_owned(builder);
        Ok(true)
    }
    #[cfg(test)]
    pub(in crate::game_logic) fn fixture_dozer_build_frame_with_completion_observer(
        &mut self,
        builder: ObjectId,
        site: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        observer: impl FnMut(&GameLogic, ObjectId, ObjectId),
    ) -> gamelogic::GameLogicResult<bool> {
        self.dozer_build_action_frame_observed(builder, site, effects, observer)
    }
    /// Direct contribution fixture adapter, using the same per-frame algorithm.
    /// It does not infer current task from AIState or install a synthetic runner.
    #[cfg(test)]
    pub(in crate::game_logic) fn fixture_build_action_frames_with_effects(
        &mut self,
        site_ids: &[ObjectId],
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.update_construction_with_effects(site_ids, dt, effects)?;
        for _ in 0..(dt.max(0.) * 30.).round() as u32 {
            for &site in site_ids {
                let builder = self
                    .objects
                    .get(&site)
                    .filter(|o| o.status.under_construction)
                    .and_then(|o| o.builder_id);
                if let Some(builder) = builder {
                    self.dozer_build_action_frame_owned(builder, site, effects)?;
                }
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(in crate::game_logic) fn fixture_build_action_frames(
        &mut self,
        site_ids: &[ObjectId],
        dt: f32,
    ) -> gamelogic::GameLogicResult<()> {
        self.fixture_build_action_frames_with_effects(site_ids, dt, &mut Default::default())
    }
    #[cfg(test)]
    pub(in crate::game_logic) fn fixture_build_action_frames_with_ui(
        &mut self,
        sites: &[ObjectId],
        dt: f32,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> gamelogic::GameLogicResult<()> {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            script_events: None,
            partition: None,
            audio: None,
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            view: None,
        };
        self.fixture_build_action_frames_with_effects(sites, dt, &mut effects)
    }
}
