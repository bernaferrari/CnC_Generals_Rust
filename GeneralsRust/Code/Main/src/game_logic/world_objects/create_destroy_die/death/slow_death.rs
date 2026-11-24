//! Dispatch authored ordinary SlowDeath phases while their Object is installed.
use super::*;
use crate::game_logic::native_die_definition::{
    NativeDieKind, NativeSlowFamily, SlowDeathEffectPhase,
};

impl GameLogic {
    pub(super) fn run_authored_slow_death_phase(
        &mut self,
        id: ObjectId,
        phase: SlowDeathEffectPhase,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let Some(runtime) = object.native_die_runtime.slow_death else {
            return Ok(());
        };
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let phases = object
            .thing()
            .template
            .native_die_declarations
            .as_deref()
            .and_then(|slots| {
                slots
                    .iter()
                    .find(|slot| slot.source_index == runtime.source_index)
            })
            .and_then(|slot| match &slot.kind {
                NativeDieKind::Slow {
                    family: NativeSlowFamily::Ordinary(phases),
                    ..
                } => Some(std::sync::Arc::clone(phases)),
                _ => None,
            });
        let Some(phases) = phases else {
            return Err(gamelogic::GameLogicError::Configuration(
                "authored SlowDeath slot is missing".into(),
            ));
        };
        let phase = &phases[phase as usize];
        if phase.fx.is_empty() && phase.ocls.is_empty() && phase.weapons.is_empty() {
            return Ok(());
        }
        // C++ doPhaseStuff skips all draws unless some phase has a loaded
        // effect. Each selected category still rolls over its null entries.
        let has_effect = phases.iter().any(|phase| {
            #[cfg(feature = "game_client")]
            if phase.fx.iter().any(|name| self.has_receiving_fx_list(name)) {
                return true;
            }
            phase
                .ocls
                .iter()
                .any(|name| self.weapon_effect_definitions.has_ocl(name))
                || self.weapon_definitions.as_ref().is_some_and(|definitions| {
                    phase
                        .weapons
                        .iter()
                        .any(|name| definitions.contains_key(&name.to_ascii_lowercase()))
                })
        });
        if !has_effect {
            return Ok(());
        }

        let index = |world: &Self, len: usize| {
            (len != 0).then(|| world.logic_random.next_int(0, len as i32 - 1) as usize)
        };
        if let Some(i) = index(self, phase.fx.len()) {
            self.dispatch_fx_list_at_host_object_with_effects(&phase.fx[i], id, None, effects);
        }
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(());
        }
        if let Some(i) = index(self, phase.ocls.len()) {
            let outcome = self.execute_receiving_object_ocl_with_births(
                &phase.ocls[i],
                id,
                interpreter,
                effects,
            );
            if let Some(error) = outcome.error {
                return Err(error);
            }
        }
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(());
        }
        if let Some(i) = index(self, phase.weapons.len()) {
            let position = self.objects[&id].get_position();
            self.fire_owned_temporary_weapon_at(id, &phase.weapons[i], position, effects)?;
        }
        Ok(())
    }

    pub(in crate::game_logic) fn tick_authored_ordinary_slow_death(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(object) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        let (Some(receipt), Some(timer)) = (
            object.native_die_runtime.slow_death,
            object.slow_death.as_mut(),
        ) else {
            return Ok(false);
        };
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let done = timer.tick_motion(self.frame);
        if !timer.midpoint_played && self.frame >= receipt.midpoint_frame {
            timer.midpoint_played = true;
            self.run_authored_slow_death_phase(id, SlowDeathEffectPhase::Midpoint, None, effects)?;
        }
        if done && self.spawn_identity_matches(id, birth.0, birth.1) {
            let timer = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.slow_death.as_mut());
            if let Some(timer) = timer.filter(|timer| !timer.final_played) {
                timer.final_played = true;
                self.run_authored_slow_death_phase(id, SlowDeathEffectPhase::Final, None, effects)?;
            }
            if self.spawn_identity_matches(id, birth.0, birth.1) {
                let credit = self.objects[&id].deferred_destruction_credit;
                self.request_object_destruction_with_effects(id, credit, effects)?;
            }
        }
        Ok(done)
    }
}
