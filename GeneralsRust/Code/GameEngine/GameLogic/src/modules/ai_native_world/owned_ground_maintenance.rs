//! AIUpdate.cpp:2240–2294 and Locomotor.cpp:2412–2477 ground NONE goal.
use super::*;

impl BorrowedAiWorld<'_> {
    pub(super) fn update_inactive_ground_tail(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &crate::locomotor::Locomotor,
    ) -> Result<(), String> {
        let (physics, _, _) = self.locomotion_update_proxies_mode(owner, locomotor, true)?;
        {
            let mut actual = physics.access().map_err(|_| "inactive Physics borrowed")?;
            locomotor.apply_physics_options(&mut *actual);
        }
        let mut object = owner
            .write()
            .map_err(|_| "inactive locomotor owner poisoned")?;
        let height = crate::locomotor::LocomotorMovementObservations::observe_height(
            &object,
            self.services.terrain,
        );
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::AirborneTarget,
            ),
            height > locomotor.template.airborne_targeting_height as f32,
        );
        Ok(())
    }
    pub(crate) fn maintain_ground_step(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &mut crate::locomotor::Locomotor,
        final_position: Option<Coord3D>,
    ) -> Result<(Coord3D, bool, bool), String> {
        let (physics, _, _) = self.locomotion_update_proxies_mode(owner, locomotor, true)?;
        self.validate_containment_transform_owned(owner)?;
        if final_position.is_some_and(|position| !position.is_finite()) {
            return Err("native final position must be finite".into());
        }
        // Validate the explicit motive observation before any final-position pose.
        physics
            .access()
            .map_err(|_| "ground maintenance Physics borrowed")?
            .is_motive_at(self.services.frame)?;
        // CPP doLocomotor applies authored Physics options before every goal
        // branch, including NONE; final-position callbacks observe those flags.
        {
            let mut actual = physics
                .access()
                .map_err(|_| "ground maintenance Physics borrowed")?;
            locomotor.apply_physics_options(&mut *actual);
        }
        let (current, on_ground, orientation, stick_to_slope, mut matrix) = {
            let object = owner
                .read()
                .map_err(|_| "ground maintenance owner poisoned")?;
            (
                *object.get_position(),
                !self.above_terrain(&object)?
                    && object.get_layer() == crate::common::PathfindLayerEnum::Ground,
                object.get_orientation(),
                object.is_kind_of(KindOf::StickToTerrainSlope),
                object.get_transform_matrix(),
            )
        };
        let (position, still_final) = match final_position {
            Some(final_position) => crate::locomotor::Locomotor::settle_final_position_with_height(
                current,
                final_position,
                on_ground,
                |x, y| Some(self.services.terrain.get_ground_height(x, y, None)),
            ),
            None => (current, false),
        };
        if final_position.is_some() {
            if stick_to_slope {
                self.services
                    .terrain
                    .align_on_terrain(orientation, &position, true, &mut matrix);
            } else {
                matrix.w_axis = glam::Vec4::new(position.x, position.y, position.z, 1.0);
            }
            // No Object/Physics read loan crosses the actual pose callback.
            self.set_physics_transform_owned(owner, &matrix)?;
        }
        let actual_position = *owner
            .read()
            .map_err(|_| "ground maintenance owner poisoned")?
            .get_position();
        let requires_constant = {
            let mut actual = physics
                .access()
                .map_err(|_| "ground maintenance Physics borrowed")?;
            locomotor.maintain_ground_with_physics_at(
                actual_position,
                self.services.frame,
                &mut *actual,
            )?
        };
        let mut object = owner
            .write()
            .map_err(|_| "ground maintenance owner poisoned")?;
        let height = crate::locomotor::LocomotorMovementObservations::observe_height(
            &object,
            self.services.terrain,
        );
        object.set_status(
            crate::common::ObjectStatusMaskType::from_status(
                crate::common::ObjectStatusTypes::AirborneTarget,
            ),
            height > locomotor.template.airborne_targeting_height as f32,
        );
        Ok((position, still_final, requires_constant))
    }
}
