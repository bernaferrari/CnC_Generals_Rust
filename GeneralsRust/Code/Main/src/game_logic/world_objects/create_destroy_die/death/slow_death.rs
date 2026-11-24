//! Dispatch authored ordinary SlowDeath phases while their Object is installed.
use super::*;
use crate::game_logic::host_slow_death::{HostSlowDeathIni, HostSlowDeathPhase};
use crate::game_logic::native_die_definition::{
    NativeDieDeclaration, NativeDieKind, NativeSlowFamily, SlowDeathEffectPhase, SlowDeathFlight,
    SlowDeathFling,
};
use crate::game_logic::object::OnDieInput;

impl GameLogic {
    /// SlowDeathBehavior::onDie rolls over eligible declarations in source order.
    pub(super) fn choose_authored_slow_death<'a>(
        &self,
        id: ObjectId,
        input: OnDieInput,
        declarations: &'a [NativeDieDeclaration],
    ) -> Option<(u32, &'a NativeSlowFamily, &'a HostSlowDeathIni)> {
        let object = self.objects.get(&id)?;
        let overkill = input.overkill_fraction(object.health.maximum);
        let mut applicable = declarations.iter().filter_map(|declaration| {
            let NativeDieKind::Slow { family, config } = &declaration.kind else {
                return None;
            };
            declaration.gate.allows(object, input.death_type).then(|| {
                (
                    (declaration.source_index, family, config),
                    config.probability_modifier_for(overkill),
                )
            })
        });
        let total = applicable
            .clone()
            .map(|(_, weight)| weight)
            .fold(0, i32::wrapping_add);
        if total <= 0 {
            return None;
        }

        let mut roll = self.logic_random.next_int(1, total);
        applicable.find_map(|(selected, weight)| {
            roll -= weight;
            (roll <= 0).then_some(selected)
        })
    }

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
        let data = object
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
                    family: NativeSlowFamily::Ordinary(data),
                    ..
                } => Some(std::sync::Arc::clone(data)),
                _ => None,
            });
        let Some(data) = data else {
            return Err(gamelogic::GameLogicError::Configuration(
                "authored SlowDeath slot is missing".into(),
            ));
        };
        let phases = &data.phases;
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

    /// SlowDeathBehavior::beginSlowDeath releases a held slave, lifts the
    /// owner slightly off terrain, then draws and applies the authored force.
    pub(super) fn begin_authored_slow_death_flight(
        &mut self,
        id: ObjectId,
        fling: SlowDeathFling,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        if object.status.disabled_held {
            self.stop_owned_slaved_effects(id, effects);
        }
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return false;
        }
        let object = &self.objects[&id];
        if !object.authors_behavior_module("PhysicsBehavior") {
            return true;
        }
        let position = object.get_position();
        const MIN_ALTITUDE: f32 = 1.0;
        if position.y - self.raw_terrain_ground_height_at(position) < MIN_ALTITUDE
            && (!self.set_owned_object_position_with_effects(
                id,
                position + Vec3::Y * MIN_ALTITUDE,
                self.frame,
                effects,
            ) || !self.spawn_identity_matches(id, birth.0, birth.1))
        {
            return false;
        }

        use std::f32::consts::PI;
        let azimuth = self.logic_random.next_real(-PI, PI);
        let pitch = self
            .logic_random
            .next_real(fling.min_pitch, fling.max_pitch);
        let magnitude = self
            .logic_random
            .next_real(fling.min_force, fling.max_force);
        let force =
            gamelogic::object_creation_list::calc_random_force_xyz(azimuth, pitch, magnitude);
        let object = self.objects.get_mut(&id).unwrap();
        object.allow_to_fall = true;
        object.apply_physics_force(Vec3::new(force.x, force.z, force.y), self.frame);
        object.set_extra_bounciness(-1.0);
        object.set_extra_friction(
            -3.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL,
        );
        object.set_allow_bouncing(true);
        let transform = glam::Mat4::from_translation(object.get_position())
            * glam::Mat4::from_rotation_y(force.y.atan2(force.x));
        if !self.set_owned_object_transform_with_effects(id, transform, self.frame, effects)
            || !self.spawn_identity_matches(id, birth.0, birth.1)
        {
            return false;
        }
        self.set_authored_slow_death_flight(id, SlowDeathFlight::Flailing);
        true
    }

    fn set_authored_slow_death_flight(&mut self, id: ObjectId, flight: SlowDeathFlight) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        if let Some(receipt) = object.native_die_runtime.slow_death.as_mut() {
            receipt.flight = flight;
            flight.apply_model(object);
        }
    }

    /// Keep all three deadlines suspended through the first ground contact.
    /// The shrub check still runs on that contact frame.
    fn advance_authored_slow_death_flight(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        flight: SlowDeathFlight,
        sink_rate: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let object = self.objects.get_mut(&id).unwrap();
        let timer = object.slow_death.as_mut().unwrap();
        let receipt = object.native_die_runtime.slow_death.as_mut().unwrap();
        timer.sink_at_frame = timer.sink_at_frame.wrapping_add(1);
        timer.destroy_at_frame = timer.destroy_at_frame.wrapping_add(1);
        receipt.midpoint_frame = receipt.midpoint_frame.wrapping_add(1);
        let position = object.get_position();
        let collidee = object.last_collidee;
        let landed = position.y <= self.raw_terrain_ground_height_at(position);
        if landed {
            self.set_authored_slow_death_flight(
                id,
                if flight == SlowDeathFlight::Snagged {
                    SlowDeathFlight::BouncingWithParachute
                } else {
                    SlowDeathFlight::Bouncing
                },
            );
        }
        let hit_tree = collidee
            .and_then(|other| self.objects.get(&other))
            .is_some_and(|other| other.is_kind_of(KindOf::Shrubbery));
        if !hit_tree {
            return Ok(false);
        }

        self.set_owned_passenger_held(id, true, effects);
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(false);
        }
        self.set_authored_slow_death_flight(
            id,
            if landed {
                SlowDeathFlight::LandedInTree
            } else {
                SlowDeathFlight::Snagged
            },
        );
        let position = self.objects[&id].get_position() - Vec3::Y * (sink_rate * 50.0);
        if !self.set_owned_object_position_with_effects(id, position, self.frame, effects)
            || !self.spawn_identity_matches(id, birth.0, birth.1)
        {
            return Ok(false);
        }
        let object = &self.objects[&id];
        let position = object.get_position();
        if position.y > self.raw_terrain_ground_height_at(position) {
            return Ok(false);
        }
        let credit = object.deferred_destruction_credit;
        self.request_object_destruction_with_effects(id, credit, effects)?;
        Ok(true)
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
            object.slow_death.as_ref(),
        ) else {
            return Ok(false);
        };
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let in_flight = receipt.flight.is_airborne();
        let initial_sink_rate = timer.sink_rate_per_frame;
        let sink_due = self.frame >= timer.sink_at_frame && initial_sink_rate > 0.0;
        let (object, expedited_retirement) = if in_flight || sink_due {
            let expedited_retirement = if in_flight {
                self.advance_authored_slow_death_flight(
                    id,
                    birth,
                    receipt.flight,
                    initial_sink_rate,
                    effects,
                )?
            } else {
                false
            };
            if !self.spawn_identity_matches(id, birth.0, birth.1) {
                return Ok(false);
            }
            // The flight update may have postponed the sink deadline.
            let timer = self.objects[&id].slow_death.as_ref().unwrap();
            let sink_rate = timer.sink_rate_per_frame;
            let sinking = self.frame >= timer.sink_at_frame && sink_rate > 0.0;
            if sinking && !self.sink_authored_slow_death(id, birth, sink_rate, effects) {
                return Ok(false);
            }
            let object = self.objects.get_mut(&id).unwrap();
            if sinking {
                object.slow_death.as_mut().unwrap().phase = HostSlowDeathPhase::Sinking;
            }
            (object, expedited_retirement)
        } else {
            (object, false)
        };
        let receipt = object.native_die_runtime.slow_death.unwrap();
        let timer = object.slow_death.as_mut().unwrap();
        let done = self.frame >= timer.destroy_at_frame;
        if done {
            timer.phase = HostSlowDeathPhase::Done;
        }
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
        Ok(done || expedited_retirement)
    }

    fn sink_authored_slow_death(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        sink_rate: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        self.set_owned_passenger_held(id, true, effects);
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return false;
        }
        let position = self.objects[&id].get_position() - Vec3::Y * sink_rate;
        self.set_owned_object_position_with_effects(id, position, self.frame, effects)
            && self.spawn_identity_matches(id, birth.0, birth.1)
    }
}
