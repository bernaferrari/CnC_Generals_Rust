//! Canonical Physics integration driven by this match's scheduler and terrain.
use super::*;

impl BorrowedAiWorld<'_> {
    pub(crate) fn update_physics_phase(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<crate::modules::UpdateSleepTime, String> {
        let (physics, proxy) = {
            let object = owner.read().map_err(|_| "Physics owner poisoned")?;
            let admitted = self
                .object(object.get_id())
                .ok_or("Physics owner absent from factory")?;
            if !Arc::ptr_eq(&admitted, owner) {
                return Err("Physics owner belongs to another factory".into());
            }
            let physics = object
                .get_physics()
                .ok_or("installed Physics descriptor absent")?;
            let proxy = physics.installed_update_proxy(&object)?;
            (physics, proxy)
        };
        let current = self
            .services
            .current_update
            .as_ref()
            .ok_or("Physics phase requires its actual driving proxy")?;
        if !Arc::ptr_eq(&proxy, current) {
            return Err("Physics phase belongs to another update proxy".into());
        }
        self.services
            .update_scheduler
            .validate_owned_update_wake(&proxy)?;
        let frame = self.services.frame;
        self.validate_containment_transform_owned(owner)?;
        let cargo = self.observe_physics_cargo_owned(owner)?;
        let terrain = self.services.terrain;
        let prepared = {
            let object = owner.read().map_err(|_| "Physics owner poisoned")?;
            let layer = crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32);
            let position = object.get_position();
            let ground = terrain.get_layer_height(position.x, position.y, layer, None, false)
                + cargo.carrier_deck_height;
            physics.prepare_owned_at(
                &object,
                frame,
                self.services.gravity,
                ground,
                self.services.physics_rules,
                cargo.contained_mass,
                |x, y| {
                    terrain.get_layer_height(x, y, layer, None, false) + cargo.carrier_deck_height
                },
            )?
        };
        if prepared.landing_expected {
            // CPP Object::onCollide(nullptr) visits every installed collider.
            // Admit the complete leaf set before Physics/pose changes; custom
            // collide, containment, projectile and disabled families remain errors.
            self.validate_physics_ground_collision_owned(owner)?;
            if let Some(damage) = &prepared.landing_damage {
                self.validate_damage_owned(owner, damage)?;
            }
        }
        if prepared.bounce_stun_expected
            && self.physics_bounce_stun_should_kill(owner, prepared.bounce_first_transform)?
        {
            let damage = Self::physics_resting_kill_damage(owner)?;
            self.validate_damage_owned(owner, &damage)?;
        }
        if prepared.resting_kill_expected {
            let damage = Self::physics_resting_kill_damage(owner)?;
            self.validate_damage_owned(owner, &damage)?;
        }
        let mut step = {
            let mut object = owner.write().map_err(|_| "Physics owner poisoned")?;
            physics.commit_owned_at(&mut object, prepared)?
        };
        // C++ handleBounce immediately publishes setAngles before stun kill,
        // with canonical unclamped velocity visible to all intervening callbacks.
        if let Some(first) = step
            .bounce
            .as_ref()
            .and_then(|bounce| bounce.first_transform)
        {
            self.physics_pose_owned(owner, &first)?;
        }
        if step
            .bounce
            .as_ref()
            .is_some_and(|bounce| bounce.force.is_some())
        {
            if physics.bounce_stun_destruction_active()?
                && self.physics_bounce_stun_should_kill(owner, None)?
            {
                let mut damage = Self::physics_resting_kill_damage(owner)?;
                self.attempt_damage_owned(owner, &mut damage)?;
            }
            // Original second setAngles rereads the current yaw after callbacks.
            let object = owner.read().map_err(|_| "bounce owner poisoned")?;
            let matrix = object.get_transform_matrix();
            let yaw = matrix.x_axis.y.atan2(matrix.x_axis.x);
            let righted = glam::Mat4::from_rotation_translation(
                glam::Quat::from_rotation_z(yaw),
                *object.get_position(),
            );
            drop(object);
            // Ground clamp and stunned model bookkeeping precede this righting
            // callback, as CPP745-790, while the final translated matrix follows.
            physics.complete_owned_bounce_at(
                &mut *owner.write().map_err(|_| "bounce owner poisoned")?,
                &mut step,
            )?;
            self.physics_pose_owned(owner, &righted)?;
            let mut final_matrix = owner
                .read()
                .map_err(|_| "bounce owner poisoned")?
                .get_transform_matrix();
            final_matrix.w_axis = step.transform.ok_or("bounce transform absent")?.w_axis;
            step.transform = Some(final_matrix);
        } else if step.bounce.is_some() {
            physics.complete_owned_bounce_at(
                &mut *owner.write().map_err(|_| "bounce owner poisoned")?,
                &mut step,
            )?;
        }
        {
            let mut object = owner.write().map_err(|_| "Physics owner poisoned")?;
            if step.enable_freefall {
                object.set_freefall_for_owned_leaf();
                object.set_model_condition_state(crate::common::MODELCONDITION_FREEFALL);
            }
        }
        let pose_result = match step.transform {
            Some(transform) => self.set_physics_transform_owned(owner, &transform),
            None => Ok(()),
        };
        if let Some(height) = step.height_above_terrain {
            owner
                .write()
                .map_err(|_| "Physics owner poisoned")?
                .set_height_above_terrain(height);
        }
        let cargo_end = self.observe_physics_cargo_owned(owner)?;
        let airborne_at_end = {
            let object = owner.read().map_err(|_| "landing owner poisoned")?;
            let position = object.get_position();
            let layer = crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32);
            Object::is_above_terrain_height(
                position.z,
                terrain.get_layer_height(position.x, position.y, layer, None, false)
                    + cargo_end.carrier_deck_height,
            )
        };
        // Reset canonical acceleration/overlap before any landing callbacks.
        let landing = physics.begin_owned_landing_at(
            &*owner.read().map_err(|_| "landing owner poisoned")?,
            airborne_at_end,
        )?;
        if let Some(force) = step.bounce.as_ref().and_then(|bounce| bounce.force) {
            let cargo = self.observe_physics_cargo_owned(owner)?;
            let mass = physics.owned_base_mass()? + cargo.contained_mass;
            physics.apply_owned_bounce_force_at(
                &*owner.read().map_err(|_| "bounce force owner poisoned")?,
                &force,
                mass,
            )?;
        }
        let landing_result = (|| -> Result<(), String> {
            pose_result?;
            let Some(landing) = landing else {
                return Ok(());
            };
            // The retail CPP branch copies and emits the configured event before
            // Object::onCollide. FIX_AUDIO volume shaping is a separate build contract.
            if let Some(sound) = landing.sound {
                self.add_audio_event(&sound);
            }
            self.dispatch_physics_ground_collision_owned(owner)?;
            // Reread Physics after the synchronous collider, just as CPP does.
            let cargo = self.observe_physics_cargo_owned(owner)?;
            let damage = physics.falling_damage_owned_at(
                &*owner.read().map_err(|_| "fall damage owner poisoned")?,
                step.active_velocity_z,
                cargo.contained_mass,
            )?;
            if let Some(mut damage) = damage {
                // The synchronous stunned-bounce kill can already have killed
                // this owner. ActiveBody.cpp:338 then returns with zero damage;
                // falling damage has no shockwave, so Object::attemptDamage's
                // remaining shock/radar tail has no effect either. Still apply
                // the original post-attempt SPLATTED observation below.
                let alive = !owner
                    .read()
                    .map_err(|_| "fall damage owner poisoned")?
                    .is_effectively_dead();
                if alive {
                    self.attempt_damage_owned(owner, &mut damage)?;
                }
                let mut object = owner.write().map_err(|_| "fallen owner poisoned")?;
                if object.is_effectively_dead() {
                    object.set_model_condition_state(crate::common::MODELCONDITION_SPLATTED);
                }
            }
            Ok(())
        })();
        let mut object = owner.write().map_err(|_| "Physics finish owner poisoned")?;
        if !airborne_at_end {
            physics.prepare_freefall_cleanup_at(&object)?;
            if object.is_disabled_by_type(crate::common::DisabledType::DisabledFreefall) {
                object.clear_freefall_for_owned_leaf();
            }
            object.clear_model_condition_state(crate::common::MODELCONDITION_FREEFALL);
        }
        // CPP ActiveBody.cpp:338 returns immediately for an already-dead owner;
        // kill's shockwave fields are zero, so no later effect remains.
        let kill_resting = !object.is_effectively_dead()
            && physics.should_kill_resting_owned(&object, airborne_at_end)?;
        drop(object);
        let kill_result = if landing_result.is_ok() && kill_resting {
            let mut damage = Self::physics_resting_kill_damage(owner)?;
            self.attempt_damage_owned(owner, &mut damage).map(|_| ())
        } else {
            Ok(())
        };
        // Kill callbacks execute before the original final flags and sleep test.
        let sleep = physics.finish_owned_at(
            &mut *owner.write().map_err(|_| "Physics finish owner poisoned")?,
            frame,
            airborne_at_end,
        )?;
        landing_result?;
        kill_result?;
        Ok(sleep)
    }

    pub(crate) fn validate_physics_ground_collision_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let mut visited = std::collections::HashSet::new();
        let mut next = Some(owner.clone());
        while let Some(owner) = next {
            let object = owner
                .try_read()
                .map_err(|_| "ground collider owner borrowed")?;
            let id = object.get_id();
            if !visited.insert(id) {
                return Err("cyclic ground collision containment".into());
            }
            let actual = self
                .object(id)
                .ok_or("ground collider absent from receiving factory")?;
            if !Arc::ptr_eq(&actual, &owner) {
                return Err("ground collider belongs to another factory".into());
            }
            object.owned_physics_ground_colliders()?;
            if object.is_kind_of(KindOf::Projectile) {
                return Err("ground collision requires supplied projectile detonation".into());
            }
            next = object
                .get_contained_by()
                .map(|id| self.object(id).ok_or("ground collision container absent"))
                .transpose()?;
        }
        Ok(())
    }

    pub(crate) fn dispatch_physics_ground_collision_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_physics_ground_collision_owned(owner)?;
        self.dispatch_physics_ground_collision_chain(owner, &mut std::collections::HashSet::new())
    }
    fn dispatch_physics_ground_collision_chain(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        visited: &mut std::collections::HashSet<ObjectID>,
    ) -> Result<(), String> {
        let (id, colliders) = {
            let object = owner.read().map_err(|_| "ground collider poisoned")?;
            (object.get_id(), object.owned_physics_ground_colliders()?)
        };
        if !visited.insert(id) {
            return Err("cyclic ground collision callback".into());
        }
        for collider in colliders {
            let next = {
                let object = owner.read().map_err(|_| "ground collider poisoned")?;
                if object.test_status(crate::common::ObjectStatusTypes::NoCollisions) {
                    break;
                }
                collider.ground_collision_owned_at(&object)?
            };
            if let Some(id) = next {
                let container = self
                    .object(id)
                    .ok_or("ground collision container disappeared")?;
                self.dispatch_physics_ground_collision_chain(&container, visited)?;
            }
        }
        visited.remove(&id);
        Ok(())
    }

    fn physics_pose_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        matrix: &glam::Mat4,
    ) -> Result<(), String> {
        self.set_physics_transform_owned(owner, matrix)
    }

    fn physics_bounce_stun_should_kill(
        &self,
        owner: &Arc<RwLock<Object>>,
        proposed: Option<glam::Mat4>,
    ) -> Result<bool, String> {
        let object = owner.read().map_err(|_| "stunned bounce owner poisoned")?;
        let matrix = proposed.unwrap_or_else(|| object.get_transform_matrix());
        if matrix.z_axis.z < 0.0 {
            return Ok(true);
        }
        let pos = *object.get_position();
        let extent = self.services.terrain.get_maximum_pathfind_extent();
        if pos.x < extent.lo.x || pos.x > extent.hi.x || pos.y < extent.lo.y || pos.y > extent.hi.y
        {
            return Ok(true);
        }
        let ai = object.get_ai_update_interface();
        drop(object);
        let Some(ai) = ai else {
            return Ok(false);
        };
        let ai = ai.try_lock().map_err(|_| "stunned bounce AI borrowed")?;
        Ok((self.services.terrain.is_cliff_cell(pos.x, pos.y)
            && !ai.has_locomotor_for_surface(crate::path::SURFACE_CLIFF))
            || (self
                .services
                .terrain
                .is_underwater(pos.x, pos.y, None, None)
                && !ai.has_locomotor_for_surface(crate::path::SURFACE_WATER)))
    }

    pub(super) fn physics_resting_kill_damage(
        owner: &Arc<RwLock<Object>>,
    ) -> Result<crate::damage::DamageInfo, String> {
        let body = owner
            .try_read()
            .map_err(|_| "resting kill owner borrowed")?
            .get_body_module()
            .ok_or("resting kill body absent")?;
        let health = body
            .try_lock()
            .map_err(|_| "resting kill body borrowed")?
            .get_max_health();
        let mut damage = crate::damage::DamageInfo::with_simple(
            health,
            crate::common::INVALID_ID,
            crate::damage::DamageType::Unresistable,
            crate::damage::DeathType::Normal,
        );
        damage.input.kill = true;
        damage.sync_from_input();
        Ok(damage)
    }

    pub(crate) fn run_scheduled_physics_update(
        &mut self,
        update: crate::system::game_logic::NativeScheduledUpdate,
    ) -> Result<(), String> {
        if update.phase != game_engine::common::thing::update_module::SleepyUpdatePhase::Physics {
            return Err("scheduled operation is not the Physics phase".into());
        }
        let owner = self
            .object(update.object_id)
            .ok_or("scheduled Physics owner absent")?;
        let sleep = if update.should_process {
            self.update_physics_phase(&owner)?
        } else {
            crate::modules::UpdateSleepTime::None
        };
        self.services
            .update_scheduler
            .finish_owned_scheduled_update(self.services.frame, update, sleep);
        Ok(())
    }
}
