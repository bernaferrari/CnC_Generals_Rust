//! The original synchronous StructureTopple effect/owner boundaries.
//! CPP StructureToppleUpdate.cpp194–296: Delay → Angle → crush → Done →
//! Delay → local rotation → completion, releasing the host loan per callback.
use super::*;
use crate::game_logic::host_structure_topple::HostStructureToppleState;

impl GameLogic {
    pub(crate) fn dispatch_owned_structure_topple_fx(&mut self, id: ObjectId) {
        self.dispatch_owned_structure_topple_fx_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn dispatch_owned_structure_topple_fx_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let effects = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.structure_topple_data.as_mut())
            .map(|data| data.take_pending_fx())
            .unwrap_or_default();
        self.execute_owned_structure_topple_fx_with_effects(id, effects, presentation_effects);
    }
    fn execute_owned_structure_topple_fx(
        &mut self,
        id: ObjectId,
        effects: Vec<crate::game_logic::host_structure_topple::StructureToppleFxEvent>,
    ) {
        self.execute_owned_structure_topple_fx_with_effects(
            id,
            effects,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn execute_owned_structure_topple_fx_with_effects(
        &mut self,
        id: ObjectId,
        effects: Vec<crate::game_logic::host_structure_topple::StructureToppleFxEvent>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        for effect in effects {
            if !self
                .objects
                .get(&id)
                .is_some_and(|object| object.structure_topple_fx_is_allowed())
            {
                continue;
            }
            if effect.at_object {
                self.dispatch_fx_list_at_host_object_with_effects(
                    &effect.name,
                    id,
                    None,
                    presentation_effects,
                );
            } else {
                self.dispatch_fx_list_at_host_position_with_effects(
                    &effect.name,
                    glam::Vec3::new(effect.x, effect.y, effect.z),
                    presentation_effects,
                );
            }
        }
    }
    pub(crate) fn update_owned_structure_topple(&mut self, id: ObjectId) -> Result<bool, String> {
        self.update_owned_structure_topple_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_owned_structure_topple_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        if self
            .objects
            .get(&id)
            .and_then(|object| object.structure_topple_data.as_ref())
            .is_none_or(|data| {
                matches!(
                    data.state,
                    HostStructureToppleState::Standing | HostStructureToppleState::Done
                )
            })
        {
            return Ok(false);
        }
        // Reject unsupported firing families before any real waiting/angle
        // phase or sample-cursor mutation. An absent weapon matches CPP's
        // findWeaponTemplate(NULL) no-crushing branch.
        self.preflight_owned_structure_topple_crushing(id)?;
        let waiting_burst = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.structure_topple_data.as_mut())
            .is_some_and(|data| data.prepare_owned_waiting_fx(self.frame));
        self.dispatch_owned_structure_topple_fx_with_effects(id, presentation_effects);
        if let Some(data) = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.structure_topple_data.as_mut())
        {
            data.finish_owned_waiting(self.frame, waiting_burst);
        }
        let Some((toppling, effects)) = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.structure_topple_data.as_mut())
            .map(|data| data.prepare_owned_angle_fx())
        else {
            return Ok(false);
        };
        self.execute_owned_structure_topple_fx_with_effects(id, effects, presentation_effects);
        if toppling {
            if let Some(object) = self.objects.get_mut(&id) {
                if let Some(data) = object.structure_topple_data.as_mut() {
                    data.advance_owned_angle();
                }
            }
            self.apply_owned_structure_topple_crushing_with_effects(id, presentation_effects)?;
            // Every subsequent observation resolves this owner's current
            // object after the actual damage/death callbacks.
            let reached_ground = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.structure_topple_data.as_mut())
                .filter(|data| data.state == HostStructureToppleState::Toppling)
                .is_some_and(|data| data.finish_owned_angle(self.frame));
            if reached_ground {
                self.apply_owned_structure_topple_crushing_with_effects(id, presentation_effects)?;
            }
            self.dispatch_owned_structure_topple_fx_with_effects(id, presentation_effects);
            let burst_due = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.structure_topple_data.as_mut())
                .is_some_and(|data| data.queue_owned_burst_fx(self.frame));
            self.dispatch_owned_structure_topple_fx_with_effects(id, presentation_effects);
            if burst_due {
                if let Some(data) = self
                    .objects
                    .get_mut(&id)
                    .and_then(|object| object.structure_topple_data.as_mut())
                {
                    data.finish_owned_burst(self.frame);
                }
            }
            if let Some(object) = self.objects.get_mut(&id) {
                object.apply_owned_structure_topple_rotation(
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
        }
        let completed = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.structure_topple_data.as_ref())
            .is_some_and(|data| data.owned_completion_is_due(self.frame));
        if completed {
            if let Some(object) = self.objects.get_mut(&id) {
                object.finish_owned_structure_topple_pose(
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            if let Some(data) = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.structure_topple_data.as_mut())
            {
                data.complete_owned_update(self.frame);
            }
        }
        Ok(completed)
    }
    fn apply_owned_structure_topple_crushing(&mut self, id: ObjectId) -> Result<(), String> {
        self.apply_owned_structure_topple_crushing_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn apply_owned_structure_topple_crushing_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let Some(weapon) = self.preflight_owned_structure_topple_crushing(id)? else {
            return Ok(());
        };
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let Some(data) = object.structure_topple_data.as_ref() else {
            return Ok(());
        };
        // This clone is an owned point plan, never a runtime writeback. CPP
        // updates m_lastCrushedLocation only after every weapon/FX callback.
        let mut plan = data.clone();
        let position = object.get_position();
        let samples = plan.take_crush_sweep_samples(position.x, position.z);
        self.validate_structure_topple_crush_points(id, &samples, &weapon)?;
        if !samples.is_empty() {
            self.apply_prepared_structure_topple_crush_samples_with_effects(
                id,
                samples,
                &weapon,
                presentation_effects,
            )?;
            if let Some(data) = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.structure_topple_data.as_mut())
            {
                data.last_crushed_location = plan.last_crushed_location;
            }
        }
        Ok(())
    }
}
