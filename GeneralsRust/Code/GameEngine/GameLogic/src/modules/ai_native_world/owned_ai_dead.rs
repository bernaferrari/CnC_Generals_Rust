//! CPP AIDeadState1453–1518 through the receiving script/radar/Physics owners.
use super::*;

impl BorrowedAiWorld<'_> {
    pub(super) fn validate_dead_ai_owned(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.services
            .disposal
            .as_deref()
            .ok_or("Dead AI script owner unavailable")?;
        let object = owner.try_read().map_err(|_| "Dead AI owner borrowed")?;
        let admitted = self
            .object(object.get_id())
            .ok_or("Dead AI owner retired")?;
        if !Arc::ptr_eq(&admitted, owner) {
            return Err("Dead AI owner belongs to another session".into());
        }
        if let Some(radar) = object.owned_radar_data() {
            if radar.object_id != object.get_id()
                || !self.services.radar.contains_object(object.get_id())
            {
                return Err("Dead AI radar link belongs to another session".into());
            }
        }
        if let Some(drawable) = object.get_drawable() {
            drop(
                drawable
                    .try_write()
                    .map_err(|_| "Dead AI drawable borrowed")?,
            );
        }
        if let Some(physics) = object.get_physics() {
            physics.installed_update_proxy(&object)?;
            drop(
                physics
                    .try_access()
                    .map_err(|_| "Dead AI Physics borrowed")?,
            );
        }
        Ok(())
    }

    pub(super) fn enter_dead_ai_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        clear: crate::common::ModelConditionFlags,
    ) -> Result<(), String> {
        self.validate_dead_ai_owned(owner)?;
        owner
            .write()
            .map_err(|_| "Dead AI owner poisoned")?
            .clear_and_set_model_condition_flags(
                clear,
                crate::common::ModelConditionFlags::DYING,
            )?;
        // Original notification records the driving frame; the ambient wrapper
        // reads TheGameLogic and must not participate in this receiving route.
        self.services
            .disposal
            .as_deref()
            .ok_or("Dead AI script owner unavailable")?
            .scripts
            .set_frame_object_count_changed(self.services.frame);
        Ok(())
    }

    pub(super) fn update_dead_ai_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_dead_ai_owned(owner)?;
        let physics = {
            let mut object = owner.write().map_err(|_| "Dead AI owner poisoned")?;
            object.set_effectively_dead_with_radar(true, self.services.radar)?;
            object
                .is_kind_of(KindOf::Infantry)
                .then(|| object.get_physics())
                .flatten()
        };
        if let Some(physics) = physics {
            let mut actual = physics.access().map_err(|_| "Dead AI Physics poisoned")?;
            let velocity = actual.get_velocity();
            // PhysicsBehavior.h getVelocityMagnitude is the full vector length.
            let speed =
                (velocity.x * velocity.x + velocity.y * velocity.y + velocity.z * velocity.z)
                    .sqrt();
            actual.scrub_velocity_2d(speed * 0.8);
        }
        Ok(())
    }

    pub(super) fn exit_dead_ai_owned(&mut self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.validate_dead_ai_owned(owner)?;
        owner
            .write()
            .map_err(|_| "Dead AI owner poisoned")?
            .clear_model_condition_state(crate::common::ModelConditionFlags::DYING);
        Ok(())
    }
}
