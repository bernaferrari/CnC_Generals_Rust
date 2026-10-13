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
        let terrain = self.services.terrain;
        let prepared = {
            let object = owner.read().map_err(|_| "Physics owner poisoned")?;
            let layer = crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32);
            let position = object.get_position();
            let ground = terrain.get_layer_height(position.x, position.y, layer, None, false);
            physics.prepare_owned_at(&object, frame, self.services.gravity, ground, |x, y| {
                terrain.get_layer_height(x, y, layer, None, false)
            })?
        };
        if prepared.landing_expected {
            // CPP Object::onCollide(nullptr) visits every installed collider.
            // Admit the complete leaf set before Physics/pose changes; custom
            // collide, containment, projectile and disabled families remain errors.
            owner
                .read()
                .map_err(|_| "landing owner poisoned")?
                .validate_native_leaf_enter_effects()?;
            if let Some(damage) = &prepared.landing_damage {
                self.validate_damage_owned(owner, damage)?;
            }
        }
        if prepared.resting_kill_expected {
            let damage = Self::physics_resting_kill_damage(owner)?;
            self.validate_damage_owned(owner, &damage)?;
        }
        let (step, pose_result) = {
            let mut object = owner.write().map_err(|_| "Physics owner poisoned")?;
            let step = physics.commit_owned_at(&mut object, prepared)?;
            if step.enable_freefall {
                object.set_freefall_for_owned_leaf();
                object.set_model_condition_state(crate::common::MODELCONDITION_FREEFALL);
            }
            let result = if let Some(transform) = step.transform {
                object.set_transform_matrix_with_services(
                    &transform,
                    terrain,
                    &mut self.services.collision,
                    frame,
                    &mut |id| self.services.changed_trigger_objects.push(id),
                    &mut |object| self.services.terrain_moved_objects.push(object.get_id()),
                )
            } else {
                Ok(())
            };
            if let Some(height) = step.height_above_terrain {
                object.set_height_above_terrain(height);
            }
            (step, result)
        };
        let airborne_at_end =
            self.above_terrain(&*owner.read().map_err(|_| "landing owner poisoned")?)?;
        // Reset canonical acceleration/overlap before any landing callbacks.
        let landing = physics.begin_owned_landing_at(
            &*owner.read().map_err(|_| "landing owner poisoned")?,
            airborne_at_end,
        )?;
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
            {
                let object = owner
                    .read()
                    .map_err(|_| "landing collider owner poisoned")?;
                if !object.test_status(crate::common::ObjectStatusTypes::NoCollisions) {
                    physics.ground_collision_owned_at(&object)?;
                }
            }
            // Reread Physics after the synchronous collider, just as CPP does.
            let damage = physics.falling_damage_owned_at(
                &*owner.read().map_err(|_| "fall damage owner poisoned")?,
                step.active_velocity_z,
            )?;
            if let Some(mut damage) = damage {
                self.attempt_damage_owned(owner, &mut damage)?;
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

    fn physics_resting_kill_damage(
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
