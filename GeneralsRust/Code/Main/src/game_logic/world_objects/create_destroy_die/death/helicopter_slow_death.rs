//! Authored rotorcraft death callbacks from HelicopterSlowDeathUpdate.cpp.
use super::*;
use crate::game_logic::ScriptPresentationEffects;
use crate::game_logic::combat_particles::CombatParticleKind;
use crate::game_logic::game_logic::{AudioEventRequest, FlightTerrainView};
use crate::game_logic::native_die_definition::{
    NativeHelicopterFlight, NativeHelicopterSlowDeath, NativeSlowFamily,
};
use generals_sim::GameLogicResult;

impl GameLogic {
    pub(super) fn begin_authored_helicopter_slow_death(
        &mut self,
        id: ObjectId,
        definition: &NativeHelicopterSlowDeath,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let object = &self.objects[&id];
        if object.has_drawable {
            self.stop_native_drawable_ambient_playback(id, effects)?;
        }
        let position = object.get_position();
        let forward_angle = object.get_orientation();
        if let Some(sound) = &definition.effects.death_loop_sound {
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_object(id)
                    .with_position(position)
                    .looping(),
            );
        }
        let (min, max) = definition.blade_delay;
        let flight = NativeHelicopterFlight {
            forward_angle,
            forward_speed: definition.forward_speed,
            self_spin: definition.min_spin,
            spin_towards_maximum: true,
            last_spin_update: 0,
            blade_frames: self.logic_random.next_real(min, max) as u32,
            hit_ground: None,
            retired: false,
        };
        let object = self.objects.get_mut(&id).unwrap();
        object
            .native_die_runtime
            .slow_death
            .as_mut()
            .unwrap()
            .helicopter = Some(flight);
        if !object.has_ai_update_interface() {
            return Ok(());
        }
        object.max_lift = -object.physics_gravity() * (1.0 - definition.fall_how_fast);
        object.braking = definition.braking;
        if let Some(particle) = &definition.effects.attach_particle {
            let object = &self.objects[&id];
            let offset = if let Some(bone) = &definition.particle_bone {
                self.helicopter_local_bone_offset(object, bone, effects)?
                    .unwrap_or_default()
            } else {
                definition.particle_location
            };
            let is_shrouded = self.host_fx_object_is_shrouded(id.0);
            self.combat_particles
                .attach_named_to_object_local_with_visibility(
                    id,
                    position,
                    forward_angle,
                    offset,
                    self.frame,
                    particle,
                    CombatParticleKind::DeathSmoke,
                    None,
                    is_shrouded,
                    effects,
                );
        }
        Ok(())
    }

    fn helicopter_local_bone_offset(
        &self,
        object: &Object,
        bone: &str,
        effects: &ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<Option<Vec3>> {
        if effects.pristine_bones.is_none() {
            return Ok(None);
        }
        let transforms = self
            .query_pristine_local_bone_transforms(object, effects.pristine_bones, bone, 0, 1, 0, 0)
            .map_err(generals_sim::GameLogicError::Configuration)?;
        Ok(transforms.first().map(|local| {
            let p = local.w_axis;
            Vec3::new(p.x, p.z, p.y)
        }))
    }

    pub(in crate::game_logic) fn tick_authored_helicopter_slow_death(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        let object = &self.objects[&id];
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let receipt = object.native_die_runtime.slow_death.unwrap();
        if receipt.helicopter.is_some_and(|flight| flight.retired) {
            return Ok(true);
        }
        let definition = object
            .thing()
            .template
            .native_die_declarations
            .as_deref()
            .and_then(|slots| receipt.family_in(slots))
            .and_then(|family| match family {
                NativeSlowFamily::Helicopter(helicopter) => Some(helicopter.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                generals_sim::GameLogicError::Configuration(
                    "authored HelicopterSlowDeath slot is missing".into(),
                )
            })?;

        let base_done = self.tick_authored_slow_death_base(id, effects)?;
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(base_done);
        }

        let flight = self.objects[&id]
            .native_die_runtime
            .slow_death
            .unwrap()
            .helicopter
            .unwrap();
        if let Some(ground_frame) = flight.hit_ground {
            if self.frame.wrapping_sub(ground_frame) as f32
                > definition.effects.delay_from_ground_frames
            {
                self.finish_authored_helicopter_crash(id, &definition, effects)?;
                return Ok(true);
            }
            return Ok(base_done);
        }

        if !self.advance_authored_helicopter_spin(id, flight, &definition, effects)?
            || !self.spawn_identity_matches(id, birth.0, birth.1)
        {
            return Ok(base_done);
        }
        self.check_authored_helicopter_contact(id, &definition, effects)?;
        Ok(base_done)
    }

    fn advance_authored_helicopter_spin(
        &mut self,
        id: ObjectId,
        mut flight: NativeHelicopterFlight,
        definition: &NativeHelicopterSlowDeath,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        let object = &self.objects[&id];
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let transform = object.get_transform_matrix();
        // C++ In_Place_Pre_Rotate_Z pre-multiplies only the orientation.
        // A positive original Z-up turn is negative about host Y.
        let mut rotated = glam::Mat4::from_rotation_y(-flight.self_spin) * transform;
        rotated.w_axis = transform.w_axis;
        if !self.set_owned_object_transform_with_effects(id, rotated, self.frame, effects)
            || !self.spawn_identity_matches(id, birth.0, birth.1)
        {
            return Ok(false);
        }
        if definition.spin_update_delay != 0.0
            && self.frame.wrapping_sub(flight.last_spin_update) as f32
                > definition.spin_update_delay
        {
            if flight.spin_towards_maximum {
                flight.self_spin += definition.spin_update_amount;
                if flight.self_spin > definition.max_spin {
                    flight.self_spin = definition.max_spin;
                    flight.spin_towards_maximum = false;
                }
            } else {
                flight.self_spin -= definition.spin_update_amount;
                if flight.self_spin < definition.min_spin {
                    flight.self_spin = definition.min_spin;
                    flight.spin_towards_maximum = true;
                }
            }
            flight.last_spin_update = self.frame;
        }
        let (sin, cos) = f64::from(flight.forward_angle).sin_cos();
        let force = Vec3::new(cos as f32, 0.0, sin as f32) * flight.forward_speed;
        let object = self.objects.get_mut(&id).unwrap();
        if object.authors_behavior_module("PhysicsBehavior") {
            object.apply_motive_force(force, self.frame);
        }
        flight.forward_angle += definition.spiral_turn_rate;
        flight.forward_speed *= definition.forward_damping;
        let blade_due = flight.blade_frames == 1;
        flight.blade_frames = flight.blade_frames.saturating_sub(1);
        object
            .native_die_runtime
            .slow_death
            .as_mut()
            .unwrap()
            .helicopter = Some(flight);
        if blade_due {
            self.run_authored_helicopter_blade(id, definition, effects)?;
        }
        Ok(self.spawn_identity_matches(id, birth.0, birth.1))
    }

    fn run_authored_helicopter_blade(
        &mut self,
        id: ObjectId,
        definition: &NativeHelicopterSlowDeath,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let object = &self.objects[&id];
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let offset = match &definition.blade_bone {
            Some(bone) => self.helicopter_local_bone_offset(object, bone, effects)?,
            None => None,
        };
        let position = offset.map_or_else(
            || object.get_position(),
            |offset| object.get_transform_matrix().transform_point3(offset),
        );
        if let Some(name) = &definition.effects.fx_blade {
            self.dispatch_fx_list_at_host_position_with_effects(name, position, effects);
        }
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(());
        }
        if let Some(name) = &definition.effects.ocl_blade {
            let outcome =
                self.execute_receiving_positional_ocl_with_births(name, id, position, effects);
            if let Some(error) = outcome.error {
                return Err(error);
            }
        }
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(());
        }
        if let Some(name) = &definition.eject_pilot
            && self.objects[&id].experience.level != VeterancyLevel::Rookie
        {
            self.execute_receiving_eject_pilot_list(id, name, None, None, effects)?;
        }
        Ok(())
    }

    fn check_authored_helicopter_contact(
        &mut self,
        id: ObjectId,
        definition: &NativeHelicopterSlowDeath,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let object = &self.objects[&id];
        let position = object.get_position();
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        let hit_tree = object
            .last_collidee
            .and_then(|other| self.objects.get(&other))
            .is_some_and(|object| object.is_kind_of(KindOf::Shrubbery));
        let view = FlightTerrainView {
            logical: Some(self.world_services.terrain().as_ref()),
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        let ground = view.raw_ground(position, object.ground_height);
        let layer = view.highest_layer(Vec3::new(position.x, 99_999.0, position.z), ground, false);
        let support = view.grid.layer_height_unclipped(position, layer, ground);
        self.set_owned_pathfinder_layer(id, layer);
        if position.y > support + 1.0 && !hit_tree {
            return Ok(());
        }
        self.objects
            .get_mut(&id)
            .unwrap()
            .native_die_runtime
            .slow_death
            .as_mut()
            .unwrap()
            .helicopter
            .as_mut()
            .unwrap()
            .hit_ground = Some(self.frame);
        if !self.run_authored_death_fx_and_ocl(
            id,
            definition.effects.fx_hit_ground.as_deref(),
            definition.effects.ocl_hit_ground.as_deref(),
            None,
            effects,
        )? {
            return Ok(());
        }
        self.set_owned_passenger_held(id, true, effects);
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Ok(());
        }
        let object = self.objects.get_mut(&id).unwrap();
        if object.has_drawable {
            let bit = crate::game_logic::host_enum_table_residual::special_damaged_model_bit();
            object.model_condition_bits |= 1 << bit;
        }
        if let Some(name) = &definition.effects.death_loop_sound {
            self.queue_audio_event(
                AudioEventRequest::new(name)
                    .with_object(id)
                    .stopping_object_loop(),
            );
        }
        Ok(())
    }

    fn finish_authored_helicopter_crash(
        &mut self,
        id: ObjectId,
        definition: &NativeHelicopterSlowDeath,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let object = self.objects.get_mut(&id).unwrap();
        let birth = (object.visual_object_generation, self.visual_world_epoch);
        object
            .native_die_runtime
            .slow_death
            .as_mut()
            .unwrap()
            .helicopter
            .as_mut()
            .unwrap()
            .retired = true;
        if !self.run_authored_death_fx_and_ocl(
            id,
            definition.effects.fx_final_blow_up.as_deref(),
            definition.effects.ocl_final_blow_up.as_deref(),
            None,
            effects,
        )? {
            return Ok(());
        }
        if let Some(template) = &definition.effects.final_rubble_object {
            let object = &self.objects[&id];
            let (team, owner, position) =
                (object.team, object.owner_player_id, object.get_position());
            let created = if let Some(team_id) = object.team_instance_id {
                let team_name = object.team_instance_name.clone();
                self.create_object_for_exact_team_with_birth_checked(
                    template, team, owner, &team_name, team_id, position, None, effects,
                )
            } else {
                self.create_object_for_owner_or_team_with_birth_checked(
                    template, team, owner, position, None, effects,
                )
            }
            .map_err(|error| error.error)?;
            if let Some((rubble, rubble_birth)) = created
                && self.spawn_identity_matches(rubble, rubble_birth.0, rubble_birth.1)
                && self.spawn_identity_matches(id, birth.0, birth.1)
            {
                let transform = self.objects[&id].get_transform_matrix();
                self.set_owned_object_transform_with_effects(
                    rubble, transform, self.frame, effects,
                );
            }
        }
        if self.spawn_identity_matches(id, birth.0, birth.1) {
            let credit = self.objects[&id].deferred_destruction_credit;
            self.request_object_destruction_with_effects(id, credit, effects)?;
        }
        Ok(())
    }
}
