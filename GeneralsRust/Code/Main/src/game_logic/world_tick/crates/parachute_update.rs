//! ParachuteContain.cpp update: source module phase, one receiving Object owner.
use crate::game_logic::*;
use glam::Vec3;

impl GameLogic {
    pub(in crate::game_logic) fn uses_authored_parachute_path(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|object| {
            object.thing().template.contain_module.kind == ContainModuleKind::Parachute
                || object
                    .contained_by
                    .and_then(|parent| self.objects.get(&parent))
                    .is_some_and(|parent| {
                        parent.thing().template.contain_module.kind == ContainModuleKind::Parachute
                    })
        })
    }

    /// true also excludes a passenger held by this genuine module from the
    /// separately tracked synthetic OCL pilot residual.
    pub(in crate::game_logic) fn tick_parachute_contain_owned(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(false);
        };
        if object
            .contained_by
            .and_then(|parent| self.objects.get(&parent))
            .is_some_and(|parent| {
                parent.thing().template.contain_module.kind == ContainModuleKind::Parachute
            })
        {
            return Ok(true);
        }
        if object.thing().template.contain_module.kind != ContainModuleKind::Parachute {
            return Ok(false);
        }
        let data = object
            .thing()
            .template
            .contain_module
            .parachute
            .clone()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration("ParachuteContain data missing".into())
            })?;
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let original = |world: &Self| {
            world.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            })
        };
        self.update_parachute_open_contain_owned(id, effects)?;
        if !original(self) {
            return Ok(true);
        }
        if self.objects.get(&id).unwrap().is_held_disabled() {
            return Ok(true);
        }
        // Source captures the first rider once for opening/collision/layer tail;
        // positionContainedObjects below resolves the current roster again.
        let rider = self.objects.get(&id).unwrap().occupants.first().copied();
        let rider_birth = rider
            .and_then(|rider| self.objects.get(&rider))
            .map(|rider| std::sync::Arc::downgrade(&rider.visual_draw_identity_token));
        let original_rider = |world: &Self| {
            rider
                .zip(rider_birth.as_ref())
                .filter(|(id, birth)| {
                    world.objects.get(id).is_some_and(|object| {
                        birth.ptr_eq(&std::sync::Arc::downgrade(
                            &object.visual_draw_identity_token,
                        ))
                    })
                })
                .map(|(id, _)| id)
        };
        if self.objects.get(&id).unwrap().status.parachute_start_height == 1e10 {
            let object = self.objects.get(&id).unwrap();
            let position = object.get_position();
            let ground = self.parachute_raw_ground(position, object.ground_height);
            let start = if position.y - ground < 2. * data.open_distance {
                ground + 2. * data.open_distance
            } else {
                position.y
            };
            self.objects
                .get_mut(&id)
                .unwrap()
                .status
                .parachute_start_height = start;
        }
        let object = self.objects.get(&id).unwrap();
        if !object.status.parachute_open {
            if (object.status.parachute_start_height - object.get_position().y).abs()
                >= data.open_distance
            {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .set_status_parachute_open(true);
                self.set_parachute_condition_owned(id, true);
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .parachute_attachments
                    .as_mut()
                    .ok_or_else(|| {
                        gamelogic::GameLogicError::Configuration(
                            "ParachuteContain cache missing".into(),
                        )
                    })?
                    .need_para_bones = true;
                if let Some(rider) = original_rider(self) {
                    self.set_parachute_condition_owned(rider, true);
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .parachute_attachments
                        .as_mut()
                        .unwrap()
                        .need_rider_bones = true;
                    if !data.open_sound.is_empty() {
                        let mut event = game_engine::common::audio::AudioEventRts::new();
                        event.set_event_name(data.open_sound.clone());
                        self.admit_containment_audio(rider, event, effects);
                    }
                }
                if self.objects.get(&id).unwrap().has_ai_update_interface() {
                    let object = self.objects.get(&id).unwrap();
                    let target = if let Some(target) = object.parachute_landing_override() {
                        // Source modifies the currently selected FreeFall locomotor,
                        // then NORMAL is selected later in this same update.
                        if object.selected_locomotor_definition.is_some() {
                            self.objects.get_mut(&id).unwrap().set_ultra_accurate(true);
                        }
                        target
                    } else {
                        self.find_owned_parachute_landing_position(object.get_position())?
                    };
                    self.unit_command_ai_move_to(id, target);
                }
            } else if let Some(rider) = original_rider(self) {
                self.set_parachute_condition_owned(rider, false);
            }
        }
        if !original(self) {
            return Ok(true);
        }
        let opened = self.objects.get(&id).unwrap().status.parachute_open;
        let no_collisions = !opened || self.objects.get(&id).unwrap().occupants.is_empty();
        self.objects
            .get_mut(&id)
            .unwrap()
            .set_drawable_hidden(!opened);
        self.objects
            .get_mut(&id)
            .unwrap()
            .set_status_no_collisions(no_collisions);
        if let Some(rider) = original_rider(self) {
            self.objects
                .get_mut(&rider)
                .unwrap()
                .set_status_no_collisions(no_collisions);
        }
        let object = self.objects.get(&id).unwrap();
        if object.has_ai_update_interface() && object.is_alive() {
            use crate::game_logic::host_upgrade_module_residuals::{
                HostLocomotorSetKind, apply_locomotor_set_kind_for_surfaces_in,
            };
            let kind = if opened {
                HostLocomotorSetKind::Normal
            } else {
                HostLocomotorSetKind::Freefall
            };
            let object = self.objects.get(&id).unwrap();
            let requested_kind = if kind == HostLocomotorSetKind::Normal && object.locomotor_upgrade
            {
                HostLocomotorSetKind::NormalUpgraded
            } else {
                kind
            };
            let requested_exists = object
                .thing()
                .template
                .authored_locomotor_sets
                .as_ref()
                .is_some_and(|sets| sets.iter().any(|row| row.kind == requested_kind));
            // AIUpdate.cpp813: an absent vector returns FALSE and leaves the
            // current locomotor intact. ParachuteContain ignores that Bool.
            // A present vector that cannot resolve is failed content admission.
            if !apply_locomotor_set_kind_for_surfaces_in(
                self.objects.get_mut(&id).unwrap(),
                kind,
                0,
                self.locomotor_definitions.as_ref(),
            ) && requested_exists
            {
                return Err(gamelogic::GameLogicError::Configuration(
                    "ParachuteContain selected locomotor set unavailable".into(),
                ));
            }
            if let Some(definition) = self.objects.get(&id).unwrap().selected_locomotor_definition {
                let current_rider = self.objects.get(&id).unwrap().occupants.first().copied();
                let altitude_damping = current_rider
                    .and_then(|rider| self.objects.get(&rider))
                    .map(|rider| {
                        let position = rider.get_position();
                        if position.y - self.parachute_raw_ground(position, rider.ground_height)
                            <= 20.
                        {
                            data.low_altitude_damping
                        } else {
                            0.
                        }
                    })
                    .unwrap_or(0.);
                if opened {
                    let object = self.objects.get_mut(&id).unwrap();
                    let visual = definition.visual_physics;
                    let state = &mut object.status;
                    state.parachute_pitch_rate += -visual.pitch_stiffness * state.parachute_pitch
                        - (visual.pitch_damping + altitude_damping) * state.parachute_pitch_rate;
                    state.parachute_roll_rate += -visual.roll_stiffness * state.parachute_roll
                        - (visual.roll_damping + altitude_damping) * state.parachute_roll_rate;
                    state.parachute_pitch += state.parachute_pitch_rate;
                    state.parachute_roll += state.parachute_roll_rate;
                    if object.has_parachute_landing_override() {
                        object.close_enough_dist = Some(10.);
                        object.close_enough_dist_3d = false;
                    }
                }
                if effects.view.is_some() {
                    self.ensure_parachute_attachment_bones(id, current_rider, effects)?;
                    self.update_parachute_attachment_offsets(id, current_rider);
                    let object = self.objects.get(&id).unwrap();
                    let pivot = object
                        .parachute_attachments
                        .as_ref()
                        .unwrap()
                        .para_sway_offset;
                    let matrix = crate::game_logic::object::parachute_contain::sway_matrix(
                        pivot,
                        object.status.parachute_pitch,
                        object.status.parachute_roll,
                    );
                    let source = self
                        .current_native_slave_source(
                            id,
                            effects.view.as_ref().unwrap().local_player_id(),
                        )
                        .map_err(|error| {
                            gamelogic::GameLogicError::Configuration(error.to_owned())
                        })?;
                    effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .set_drawable_instance_transform(&source, Some(matrix))
                        .map_err(gamelogic::GameLogicError::Configuration)?;
                }
                if let Some(rider) = self.objects.get(&id).unwrap().occupants.first().copied() {
                    if !self.position_parachute_rider_owned(id, rider, effects)? || !original(self)
                    {
                        return Ok(true);
                    }
                }
            }
        }
        if !original(self) {
            return Ok(true);
        }
        let object = self.objects.get(&id).unwrap();
        let position = object.get_position();
        let ground = self.parachute_raw_ground(position, object.ground_height);
        let layer = self
            .pathfinding_system
            .grid
            .highest_layer_at_or_below(position, ground, false);
        self.set_parachute_pathfind_layer_owned(id, layer);
        if let Some(rider) = original_rider(self) {
            self.set_parachute_pathfind_layer_owned(rider, layer)
        }
        if self.objects.get(&id).unwrap().occupants.is_empty() {
            self.apply_owned_kill_with_effects(
                id,
                combat::DamageType::Unresistable,
                host_usa_pilot::HostDeathType::Normal,
                effects,
            );
        }
        if !original(self) {
            return Ok(true);
        }
        let object = self.objects.get(&id).unwrap();
        let position = object.get_position();
        if object.is_alive()
            && object.pathfind_layer == gamelogic::path::PathfindLayerEnum::Ground as u8
            && self.water_surface_at(position).is_some_and(|water| {
                self.is_underwater_at(position)
                    && position.y - water < data.kill_when_landing_in_water_slop
            })
        {
            self.apply_owned_kill_with_effects(
                id,
                combat::DamageType::Unresistable,
                host_usa_pilot::HostDeathType::Normal,
                effects,
            );
        }
        Ok(true)
    }

    fn parachute_raw_ground(&self, position: Vec3, fallback: f32) -> f32 {
        super::super::super::FlightTerrainView {
            grid: &self.pathfinding_system.grid,
            logical: Some(self.world_services.terrain().as_ref()),
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        }
        .raw_ground(position, fallback)
    }
    fn set_parachute_condition_owned(&mut self, id: ObjectId, opened: bool) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        let para = host_enum_table_residual::parachuting_model_bit();
        let free = host_enum_table_residual::freefall_model_bit();
        object.model_condition_bits &= !((1u128 << para) | (1u128 << free));
        object.model_condition_bits |= 1u128 << if opened { para } else { free };
        object.set_status_parachute_open(opened);
    }
    /// setLayer's occupancy mutation is synchronous. Reuse the existing exact
    /// receiving occupancy stamper rather than another per-object grid mirror.
    pub(in crate::game_logic) fn set_parachute_pathfind_layer_owned(
        &mut self,
        id: ObjectId,
        layer: u8,
    ) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        if object.pathfind_layer == layer {
            return;
        }
        object.pathfind_layer = layer;
        let terrain_owner = self.world_services.terrain();
        let terrain = terrain_owner
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let ground = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &terrain,
            self.map_loaded,
            self.terrain.as_ref(),
            self.pathfinding_height_samples.as_ref(),
            &self.pathfinding_system.grid,
        );
        self.pathfinding_system
            .grid
            .update_dynamic_obstacles(self.objects.read(), &ground);
    }

    fn update_parachute_open_contain_owned(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // OpenContain::update clears the entrant before monitoring its art.
        self.objects.get_mut(&id).unwrap().player_who_entered = None;
        let bits = if let Some(view) = effects.view.as_ref() {
            let source = self
                .current_native_slave_source(id, view.local_player_id())
                .map_err(|error| gamelogic::GameLogicError::Configuration(error.to_owned()))?;
            effects
                .view
                .as_deref_mut()
                .unwrap()
                .drawable_model_condition_bits(&source)
                .map_err(gamelogic::GameLogicError::Configuration)?
        } else {
            None
        };
        let object = self.objects.get(&id).unwrap();
        let previous = object
            .parachute_attachments
            .as_ref()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain source cache missing".into(),
                )
            })?
            .observed_condition_bits;
        if let Some(bits) = bits.filter(|bits| *bits != previous) {
            let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
            for rider in object.occupants.clone() {
                let Some(child) = self.objects.get(&rider) else {
                    continue;
                };
                let child_birth = std::sync::Arc::downgrade(&child.visual_draw_identity_token);
                let complete = if let Some(firepoint) =
                    self.parachute_bone_matrix(id, "FIREPOINT", 1, effects)?
                {
                    let transform =
                        self.objects.get(&id).unwrap().get_transform_matrix() * firepoint;
                    let reaction = self
                        .objects
                        .get_mut(&rider)
                        .unwrap()
                        .prepare_transform_tree_reaction(
                            transform,
                            self.frame,
                            &mut effects.reborrow_for_object(&mut self.partition_manager),
                        );
                    self.complete_owned_position_reaction(rider, reaction, effects)
                } else {
                    let position = self.objects.get(&id).unwrap().get_position();
                    self.set_owned_object_position_with_effects(
                        rider, position, self.frame, effects,
                    )
                };
                if !self.objects.get(&id).is_some_and(|object| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                }) {
                    return Ok(());
                }
                if !complete
                    || !self.objects.get(&rider).is_some_and(|object| {
                        child_birth.ptr_eq(&std::sync::Arc::downgrade(
                            &object.visual_draw_identity_token,
                        ))
                    })
                {
                    continue;
                }
            }
            self.objects
                .get_mut(&id)
                .unwrap()
                .parachute_attachments
                .as_mut()
                .unwrap()
                .observed_condition_bits = bits;
        }
        Self::update_open_contain_door(self.objects.get_mut(&id).unwrap());
        Ok(())
    }
}
