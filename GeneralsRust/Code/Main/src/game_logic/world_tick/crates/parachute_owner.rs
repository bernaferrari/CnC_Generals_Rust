//! Receiving ParachuteContain admission; no content-name or generic Vehicle fallback.
use crate::game_logic::*;
use glam::Vec3;
impl GameLogic {
    pub(in crate::game_logic) fn put_hijacker_in_airborne_parachute(
        &mut self,
        rider: ObjectId,
        eject: Vec3,
    ) {
        self.put_hijacker_in_airborne_parachute_with_effects(rider, eject, &mut Default::default())
            .expect("receiving Hijacker parachute admission failed");
    }
    pub(in crate::game_logic) fn put_hijacker_in_airborne_parachute_with_effects(
        &mut self,
        rider_id: ObjectId,
        eject: Vec3,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let rider = self
            .objects
            .get(&rider_id)
            .ok_or(gamelogic::GameLogicError::InvalidObject(rider_id.0))?;
        let name = rider
            .thing()
            .template
            .hijacker_parachute_name
            .clone()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "HijackerUpdate missing authored ParachuteName".into(),
                )
            })?;
        if !self.templates.contains_key(&name) {
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "HijackerUpdate ParachuteName {name} is unavailable"
            )));
        }
        let rider_birth = std::sync::Arc::downgrade(&rider.visual_draw_identity_token);
        let team = rider.team;
        let owner = rider.owner_player_id;
        // C++ newObject runs at initial pose, then explicit setPosition(saved).
        let chute_id = if let Some(player) = owner {
            self.create_object_for_player_with_effects_checked(
                &name,
                player,
                Vec3::ZERO,
                None,
                effects,
            )?
        } else {
            self.create_object_with_script_effects_checked(&name, team, Vec3::ZERO, None, effects)?
        }
        .ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(format!(
                "HijackerUpdate could not create {name}"
            ))
        })?;
        let chute_birth = std::sync::Arc::downgrade(
            &self
                .objects
                .get(&chute_id)
                .ok_or(gamelogic::GameLogicError::InvalidObject(chute_id.0))?
                .visual_draw_identity_token,
        );
        let original = |world: &Self| {
            world.objects.get(&rider_id).is_some_and(|object| {
                rider_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) && world.objects.get(&chute_id).is_some_and(|object| {
                chute_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            })
        };
        if !original(self) {
            return Ok(());
        }
        if !self.set_owned_object_position_with_effects(chute_id, eject, self.frame, effects)
            || !original(self)
        {
            return Ok(());
        }
        if !self.parachute_can_contain(chute_id, rider_id) {
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "HijackerUpdate PutInContainer {name} is invalid for receiving rider"
            )));
        }
        if self.should_cancel_containment_after_booby_trap_with_effects(chute_id, rider_id, effects)
            || !original(self)
        {
            return Ok(());
        }
        if self.objects.get(&rider_id).unwrap().contained_by.is_some() {
            return Ok(());
        }
        self.objects
            .get_mut(&chute_id)
            .unwrap()
            .occupants
            .push(rider_id);
        // ParachuteContain.h45 explicitly is NOT enclosing: the passenger
        // stays visible and registered throughout its redeploy callbacks.
        // redeployOccupants precedes onContaining (and its HELD mutation).
        let redeployed = if let Some(firepoint) =
            self.parachute_bone_matrix(chute_id, "FIREPOINT", 1, effects)?
        {
            let transform = self.objects.get(&chute_id).unwrap().get_transform_matrix() * firepoint;
            let reaction = self
                .objects
                .get_mut(&rider_id)
                .unwrap()
                .prepare_transform_tree_reaction(
                    transform,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
            self.complete_owned_position_reaction(rider_id, reaction, effects)
        } else {
            let position = self.objects.get(&chute_id).unwrap().get_position();
            self.set_owned_object_position_with_effects(rider_id, position, self.frame, effects)
        };
        if !redeployed || !original(self) {
            return Ok(());
        }
        self.play_container_on_containing_sound(chute_id, effects);
        {
            let rider = self.objects.get_mut(&rider_id).unwrap();
            rider.set_status_disabled_held(true);
            rider.set_status_parachuting(true);
            let para = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                "PARACHUTING",
            )
            .expect("source PARACHUTING bit");
            rider.model_condition_bits &= !(1u128 << para);
            rider.model_condition_bits |=
                1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
        }
        self.objects
            .get_mut(&chute_id)
            .unwrap()
            .parachute_attachments
            .as_mut()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain constructor state missing".into(),
                )
            })?
            .need_rider_bones = true;
        if !self.position_parachute_rider_owned(chute_id, rider_id, effects)? || !original(self) {
            return Ok(());
        }
        // Original onContainedBy happens after both position callbacks.
        Self::set_owned_containment_in_parts(
            &mut self.objects,
            &mut self.partition_manager,
            rider_id,
            Some(chute_id),
            false,
        );
        self.stamp_contained_by_frame(rider_id, self.frame);
        self.admit_object_drawable_deselection(rider_id);
        self.play_container_load_sound(chute_id, effects);
        self.car_bomb.record_airborne_parachute_put();
        self.usa_pilot.record_air_ejection();
        Ok(())
    }

    fn parachute_can_contain(&self, chute_id: ObjectId, rider_id: ObjectId) -> bool {
        let (Some(chute), Some(rider)) = (self.objects.get(&chute_id), self.objects.get(&rider_id))
        else {
            return false;
        };
        let data = &chute.thing().template.contain_module;
        if data.kind != ContainModuleKind::Parachute
            || data.parachute.is_none()
            || !chute.occupants.is_empty()
            || !data.leftover_kind_masks_admit(rider.kind_of_cpp_mask())
        {
            return false;
        }
        let relation = self.normal_enter_relationship(rider, chute);
        if !chute.allows_normal_enter_for_relationship(relation) {
            return false;
        }
        let parachutable =
            game_engine::common::system::kind_of::KindOfMask::from_string("PARACHUTABLE")
                .is_some_and(|mask| rider.kind_of_cpp_mask() & mask.bits() != 0);
        rider.transport_slot_count() != 0 || rider.is_kind_of(KindOf::Infantry) || parachutable
    }

    /// Original source local bones are Z-up; canonical transforms are Y-up.
    fn parachute_bone_matrix(
        &self,
        id: ObjectId,
        name: &str,
        start: i32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<glam::Mat4>> {
        let source = self
            .current_native_slave_source(id, self.local_player_id().unwrap_or(0))
            .map_err(|error| {
                gamelogic::GameLogicError::Configuration(format!(
                    "ParachuteContain bone source: {error}"
                ))
            })?;
        let bones = if let Some(assets) = effects.pristine_bones {
            assets.pristine_bone_transforms(&source, name, start, 1)
        } else {
            #[cfg(feature = "game_client")]
            {
                effects
                    .view
                    .as_deref_mut()
                    .ok_or_else(|| "ParachuteContain requires receiving pristine assets".to_owned())
                    .and_then(|view| view.pristine_client_bone_transforms(&source, name, start, 1))
            }
            #[cfg(not(feature = "game_client"))]
            {
                Err("ParachuteContain requires receiving pristine assets".to_owned())
            }
        }
        .map_err(gamelogic::GameLogicError::Configuration)?;
        let basis =
            glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
        Ok(bones.first().map(|matrix| basis * (*matrix) * basis))
    }
    fn parachute_bone(
        &self,
        id: ObjectId,
        name: &str,
        start: i32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<Vec3>> {
        Ok(self
            .parachute_bone_matrix(id, name, start, effects)?
            .map(|matrix| matrix.w_axis.truncate()))
    }

    pub(in crate::game_logic) fn position_parachute_rider_owned(
        &mut self,
        chute_id: ObjectId,
        rider_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let chute_birth = std::sync::Arc::downgrade(
            &self
                .objects
                .get(&chute_id)
                .ok_or(gamelogic::GameLogicError::InvalidObject(chute_id.0))?
                .visual_draw_identity_token,
        );
        let rider_birth = std::sync::Arc::downgrade(
            &self
                .objects
                .get(&rider_id)
                .ok_or(gamelogic::GameLogicError::InvalidObject(rider_id.0))?
                .visual_draw_identity_token,
        );
        let original = |world: &Self| {
            world.objects.get(&chute_id).is_some_and(|object| {
                chute_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) && world.objects.get(&rider_id).is_some_and(|object| {
                rider_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            })
        };
        // Source updateBonePositions/updateOffsetsFromBones observe the literal
        // current roster, not the onRemoving parameter after it was erased.
        let attachment_rider = self
            .objects
            .get(&chute_id)
            .unwrap()
            .occupants
            .first()
            .copied();
        self.ensure_parachute_attachment_bones(chute_id, attachment_rider, effects)?;
        if !original(self) {
            return Ok(false);
        }
        self.update_parachute_attachment_offsets(chute_id, attachment_rider);
        let chute = self.objects.get(&chute_id).unwrap();
        let mut position = chute.get_position()
            + chute
                .parachute_attachments
                .as_ref()
                .unwrap()
                .rider_attach_offset;
        if !self.set_owned_object_position_with_effects(rider_id, position, self.frame, effects)
            || !original(self)
        {
            return Ok(false);
        }
        let rider = self.objects.get(&rider_id).unwrap();
        let terrain = super::super::flight_terrain::FlightTerrainView {
            grid: &self.pathfinding_system.grid,
            logical: Some(self.world_services.terrain().as_ref()),
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        let altitude =
            rider.get_position().y - terrain.raw_ground(rider.get_position(), rider.ground_height);
        if altitude < 0. {
            position.y -= altitude;
            if !self.set_owned_object_position_with_effects(rider_id, position, self.frame, effects)
                || !original(self)
            {
                return Ok(false);
            }
        }
        let yaw = self.objects.get(&chute_id).unwrap().get_orientation();
        self.objects
            .get_mut(&rider_id)
            .unwrap()
            .set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
        if effects.view.is_some() {
            let chute = self.objects.get(&chute_id).unwrap();
            let cache = chute.parachute_attachments.as_ref().unwrap();
            let matrix = crate::game_logic::object::parachute_contain::sway_matrix(
                cache.rider_sway_offset,
                chute.status.parachute_pitch,
                chute.status.parachute_roll,
            );
            let source = self
                .current_native_slave_source(
                    rider_id,
                    effects.view.as_ref().unwrap().local_player_id(),
                )
                .map_err(|error| gamelogic::GameLogicError::Configuration(error.to_owned()))?;
            // Source positionRider uses current HELD after callbacks, not a saved flag.
            let instance = self
                .objects
                .get(&rider_id)
                .unwrap()
                .is_held_disabled()
                .then_some(matrix);
            effects
                .view
                .as_deref_mut()
                .unwrap()
                .set_drawable_instance_transform(&source, instance)
                .map_err(gamelogic::GameLogicError::Configuration)?;
        }
        Ok(true)
    }

    fn update_parachute_attachment_offsets(
        &mut self,
        chute_id: ObjectId,
        rider_id: Option<ObjectId>,
    ) {
        let chute = self.objects.get(&chute_id).expect("receiving parachute");
        let cache = chute
            .parachute_attachments
            .as_ref()
            .expect("source attachment state");
        let transform = chute.get_transform_matrix();
        let sway = transform.transform_vector3(cache.para_sway_bone);
        let attach = transform.transform_vector3(cache.para_attach_bone);
        let rider_offsets = rider_id.and_then(|id| self.objects.get(&id)).map(|rider| {
            let offset = attach
                - rider
                    .get_transform_matrix()
                    .transform_vector3(cache.rider_attach_bone);
            (offset, sway - offset)
        });
        let cache = self
            .objects
            .get_mut(&chute_id)
            .unwrap()
            .parachute_attachments
            .as_mut()
            .unwrap();
        cache.para_sway_offset = sway;
        cache.para_attach_offset = attach;
        if let Some((attach, sway)) = rider_offsets {
            cache.rider_attach_offset = attach;
            cache.rider_sway_offset = sway;
        }
    }

    fn ensure_parachute_attachment_bones(
        &mut self,
        chute: ObjectId,
        rider: Option<ObjectId>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let cache = self
            .objects
            .get(&chute)
            .and_then(|object| object.parachute_attachments.as_ref())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain source attachment state unavailable".into(),
                )
            })?;
        let (para_needed, rider_needed) = (cache.need_para_bones, cache.need_rider_bones);
        if para_needed {
            let sway = self
                .parachute_bone(chute, "PARA_COG", 0, effects)?
                .unwrap_or(Vec3::ZERO);
            let attach = self
                .parachute_bone(chute, "PARA_ATTCH", 0, effects)?
                .unwrap_or(Vec3::ZERO);
            let cache = self
                .objects
                .get_mut(&chute)
                .unwrap()
                .parachute_attachments
                .as_mut()
                .unwrap();
            cache.para_sway_bone = sway;
            cache.para_attach_bone = attach;
            cache.need_para_bones = false;
        }
        if let Some(rider) = rider.filter(|_| rider_needed) {
            let attach = self
                .parachute_bone(rider, "PARA_MAN", 0, effects)?
                .unwrap_or_else(|| {
                    Vec3::Y
                        * self
                            .objects
                            .get(&rider)
                            .unwrap()
                            .geometry_info()
                            .max_height_above_position()
                });
            let cache = self
                .objects
                .get_mut(&chute)
                .unwrap()
                .parachute_attachments
                .as_mut()
                .unwrap();
            cache.rider_attach_bone = attach;
            cache.need_rider_bones = false;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "parachute_owner_tests.rs"]
mod tests;

#[path = "parachute_update.rs"]
mod update;

#[path = "parachute_removal.rs"]
mod removal;

#[path = "parachute_death.rs"]
mod death;
