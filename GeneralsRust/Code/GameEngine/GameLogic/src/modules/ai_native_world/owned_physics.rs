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
        let mut object = owner.write().map_err(|_| "Physics owner poisoned")?;
        let frame = self.services.frame;
        let terrain = self.services.terrain;
        let layer = crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32);
        let position = object.get_position();
        let ground = terrain.get_layer_height(position.x, position.y, layer, None, false);
        let step = physics.integrate_owned_at(
            &mut object,
            frame,
            self.services.gravity,
            ground,
            |x, y| terrain.get_layer_height(x, y, layer, None, false),
        )?;
        if step.enable_freefall {
            object.set_freefall_for_owned_leaf();
            object.set_model_condition_state(crate::common::MODELCONDITION_FREEFALL);
        }
        let pose_result = if let Some(transform) = step.transform {
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
        // The canonical module loan ended before pose notifications. Finish it
        // even if pose admission reports an error, so IS_IN_UPDATE never leaks.
        let airborne_at_end = self.above_terrain(&object)?;
        if !airborne_at_end {
            physics.prepare_freefall_cleanup_at(&object)?;
            if object.is_disabled_by_type(crate::common::DisabledType::DisabledFreefall) {
                object.clear_freefall_for_owned_leaf();
            }
            object.clear_model_condition_state(crate::common::MODELCONDITION_FREEFALL);
        }
        let sleep = physics.finish_owned_at(&mut object, frame, airborne_at_end)?;
        pose_result?;
        Ok(sleep)
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
