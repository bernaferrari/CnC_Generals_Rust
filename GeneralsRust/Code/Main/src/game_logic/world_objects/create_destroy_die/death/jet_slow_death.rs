//! Authored fixed-wing death callbacks from JetSlowDeathBehavior.cpp.
use super::*;
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::host_jet_slow_death::HostJetSlowDeathData;
use crate::game_logic::native_die_definition::{
    NativeDieKind, NativeJetSlowDeath, NativeSlowFamily,
};
use gamelogic::common::Coord3D;
use gamelogic::path::PathfindLayerEnum;

impl GameLogic {
    pub(super) fn authored_jet_dies_on_ground(&self, object: &Object) -> bool {
        if object.status.deck_height_offset {
            return true;
        }
        // Thing::getHeightAboveTerrain samples raw ground. Host Physics caches
        // support height, which can include a bridge or a flight deck.
        let position = object.get_position();
        let ground = self
            .terrain_ground_height_at(position)
            .unwrap_or(object.ground_height);
        !object.height_treats_as_airborne(position.y - ground)
    }

    /// The grounded onDie override bypasses the inherited DieMux and timer.
    pub(super) fn run_authored_jet_ground_death(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
        jet: &NativeJetSlowDeath,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let birth = (
            self.objects[&id].visual_object_generation,
            self.visual_world_epoch,
        );
        if self.run_authored_death_fx_and_ocl(
            id,
            jet.effects.fx_on_ground_death.as_deref(),
            jet.effects.ocl_on_ground_death.as_deref(),
            interpreter,
            effects,
        )? {
            self.request_object_destruction_with_effects(id, killer, effects)?;
        }
        if self.spawn_identity_matches(id, birth.0, birth.1) {
            self.objects.get_mut(&id).unwrap().status.deck_height_offset = false;
        }
        Ok(())
    }

    pub(super) fn begin_authored_jet_slow_death(
        &mut self,
        id: ObjectId,
        jet: &NativeJetSlowDeath,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.objects.get_mut(&id).unwrap().jet_slow_death = Some(HostJetSlowDeathData {
            active: true,
            death_frame: self.frame,
            roll_rate: jet.roll_rate,
            roll_rate_delta: jet.roll_rate_delta,
            fall_how_fast: jet.fall_how_fast,
            ..Default::default()
        });
        if !self.run_authored_death_fx_and_ocl(
            id,
            jet.effects.fx_initial_death.as_deref(),
            jet.effects.ocl_initial_death.as_deref(),
            interpreter,
            effects,
        )? {
            return Ok(());
        }
        let object = self.objects.get_mut(&id).unwrap();
        let position = object.get_position();
        object.max_lift = -object.physics_gravity() * (1.0 - jet.fall_how_fast);
        object.movement.turn_rate = 0.0;
        if let Some(name) = &jet.effects.death_loop_sound {
            self.queue_audio_event(
                AudioEventRequest::new(name)
                    .with_object(id)
                    .with_position(position)
                    .looping(),
            );
        }
        Ok(())
    }

    pub(in crate::game_logic) fn tick_authored_jet_slow_death(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<bool> {
        let object = &self.objects[&id];
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let state = object.jet_slow_death.as_ref().unwrap();
        if !state.is_active() {
            return Ok(state.done);
        }
        let definition = object
            .native_die_runtime
            .slow_death
            .zip(object.thing().template.native_die_declarations.as_deref())
            .and_then(|(receipt, slots)| receipt.family_in(slots))
            .and_then(|family| match family {
                NativeSlowFamily::Jet(jet) => Some(jet.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                generals_sim::GameLogicError::Configuration(
                    "authored JetSlowDeath slot is missing".into(),
                )
            })?;

        // The derived slot still runs when the base queues deletion this frame.
        let base_done = self.tick_authored_slow_death_base(id, effects)?;
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(base_done);
        }
        let object = self.objects.get_mut(&id).unwrap();
        let state = object.jet_slow_death.as_mut().unwrap();
        object.shock_roll_rate = state.roll_rate;
        state.roll_rate *= definition.roll_rate_delta;

        let fx = &definition.effects;
        if state.hit_ground {
            if self.frame.wrapping_sub(state.hit_ground_frame) >= fx.delay_final_frames {
                state.done = true;
                state.active = false;
                let credit = object.deferred_destruction_credit;
                if self.run_authored_death_fx_and_ocl(
                    id,
                    fx.fx_final_blow_up.as_deref(),
                    fx.ocl_final_blow_up.as_deref(),
                    None,
                    effects,
                )? {
                    self.request_object_destruction_with_effects(id, credit, effects)?;
                }
                return Ok(true);
            }
            return Ok(base_done);
        }

        let secondary_due = state.death_frame != 0
            && self.frame.wrapping_sub(state.death_frame) >= fx.delay_secondary_frames;
        let position = object.get_position();
        let collidee = object.last_collidee;
        let ground_height = object.ground_height;
        let (layer, height) = {
            let owner = self.world_services.terrain();
            let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
            let layer = terrain
                .get_layer_for_destination(&Coord3D::new(position.x, position.z, position.y));
            let height = if layer == PathfindLayerEnum::Ground {
                let ground = if terrain.has_height_map() {
                    terrain.get_ground_height(position.x, position.z, None)
                } else {
                    ground_height
                };
                position.y - ground
            } else {
                let clearance = position.y
                    - terrain.get_layer_height(position.x, position.z, layer, None, true);
                if (0.0..=1.0).contains(&clearance) {
                    0.0
                } else {
                    clearance
                }
            };
            (layer, height)
        };
        self.set_owned_pathfinder_layer(id, layer as u8);

        let hit_tree = collidee
            .and_then(|other| self.objects.get(&other))
            .is_some_and(|object| object.is_kind_of(KindOf::Shrubbery));
        if height <= 0.0 || hit_tree {
            if let Some(name) = &fx.death_loop_sound {
                self.queue_audio_event(
                    AudioEventRequest::new(name)
                        .with_object(id)
                        .stopping_object_loop(),
                );
            }
            if !self.run_authored_death_fx_and_ocl(
                id,
                fx.fx_hit_ground.as_deref(),
                fx.ocl_hit_ground.as_deref(),
                None,
                effects,
            )? {
                return Ok(base_done);
            }
            let object = self.objects.get_mut(&id).unwrap();
            let state = object.jet_slow_death.as_mut().unwrap();
            state.hit_ground = true;
            state.hit_ground_frame = self.frame;
            object.shock_pitch_rate = definition.pitch_rate;
        }
        // C++ evaluates this even on the first ground contact frame.
        if secondary_due
            && self.run_authored_death_fx_and_ocl(
                id,
                fx.fx_secondary.as_deref(),
                fx.ocl_secondary.as_deref(),
                None,
                effects,
            )?
        {
            let state = self
                .objects
                .get_mut(&id)
                .unwrap()
                .jet_slow_death
                .as_mut()
                .unwrap();
            state.death_frame = 0;
            state.secondary_fired = true;
        }
        Ok(base_done)
    }
}
