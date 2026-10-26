//! ParachuteContain::onRemoving, driven before Object::onRemovedFrom.
use crate::game_logic::*;
use glam::Vec3;
impl GameLogic {
    pub(in crate::game_logic) fn remove_parachute_rider_owned(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(());
        };
        let Some(rider) = self.objects.get(&rider_id) else {
            return Ok(());
        };
        let data = container
            .thing()
            .template
            .contain_module
            .parachute
            .clone()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain removal data missing".into(),
                )
            })?;
        let container_birth = std::sync::Arc::downgrade(&container.visual_draw_identity_token);
        let rider_birth = std::sync::Arc::downgrade(&rider.visual_draw_identity_token);
        let original = |world: &Self| {
            world.objects.get(&container_id).is_some_and(|object| {
                container_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) && world.objects.get(&rider_id).is_some_and(|object| {
                rider_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            })
        };
        let layer = container.pathfind_layer;
        if let Some((container, events, partition)) =
            self.host_object_ownership_partition_mut(container_id)
        {
            container.remove_occupant_with_partition_owner(rider_id, events, partition);
        }
        // ParachuteContain is NOT enclosing: do not re-register/reposition the
        // rider to the container origin before source onRemoving.
        self.set_parachute_pathfind_layer_owned(rider_id, layer);
        self.play_container_removing_template_sounds(container_id, rider_id, effects);
        if !original(self) {
            return Ok(());
        }
        let rider = self.objects.get_mut(&rider_id).unwrap();
        rider.set_status_disabled_held(false);
        rider.set_status_parachuting(false);
        self.objects
            .get_mut(&container_id)
            .unwrap()
            .set_status_no_collisions(true);
        if !self.position_parachute_rider_owned(container_id, rider_id, effects)? || !original(self)
        {
            return Ok(());
        }
        let rider = self.objects.get_mut(&rider_id).unwrap();
        let para = host_enum_table_residual::parachuting_model_bit();
        let free = host_enum_table_residual::freefall_model_bit();
        rider.model_condition_bits &= !((1u128 << para) | (1u128 << free));
        rider.set_status_parachute_open(false);
        // Source presence is immutable admitted module inventory; every Object
        // having physics fields is not evidence of an actual PhysicsBehavior.
        if rider
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .is_some_and(|template| template.has_behavior_module("PhysicsBehavior"))
        {
            rider.allow_to_fall = true;
            rider.apply_physics_force(Vec3::ZERO);
        }
        self.objects
            .get_mut(&container_id)
            .unwrap()
            .parachute_attachments
            .as_mut()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain removal cache missing".into(),
                )
            })?
            .need_rider_bones = true;
        self.apply_parachute_land_ai(rider_id, effects)?;
        if !original(self) {
            return Ok(());
        }
        let rider = self.objects.get(&rider_id).unwrap();
        let position = rider.get_position();
        if rider.pathfind_layer == gamelogic::path::PathfindLayerEnum::Ground as u8
            && self.water_surface_at(position).is_some_and(|water| {
                self.is_underwater_at(position)
                    && position.y <= water + data.kill_when_landing_in_water_slop
            })
        {
            self.apply_owned_damage_with_effects(
                rider_id,
                host_partition_collision_physics_residual::PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                None,
                combat::DamageType::Water,
                host_usa_pilot::HostDeathType::Flooded,
                None,
                &DamageHitContext::default(),
                effects,
            );
        }
        if !original(self) {
            return Ok(());
        }
        let rider = self.objects.get(&rider_id).unwrap();
        let position = rider.get_position();
        let offmap = host_deliver_payload::is_off_map_residual(
            position,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        );
        let grid = &self.pathfinding_system.grid;
        let cell = grid.world_to_grid(position);
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        let kind = if !grid.is_valid_pos(cell) {
            PathfindCellType::Impassable
        } else if rider.pathfind_layer == gamelogic::path::PathfindLayerEnum::Ground as u8 {
            grid.cell_type(cell)
        } else {
            grid.layer_cell_type(rider.pathfind_layer, cell)
                .unwrap_or(PathfindCellType::Impassable)
        };
        if offmap
            || matches!(
                kind,
                PathfindCellType::Cliff | PathfindCellType::Water | PathfindCellType::Impassable
            )
        {
            self.apply_owned_kill_with_effects(
                rider_id,
                combat::DamageType::Unresistable,
                host_usa_pilot::HostDeathType::Normal,
                effects,
            );
        }
        if !original(self) {
            return Ok(());
        }
        // Source Object::onRemovedFrom is deliberately late, after all position,
        // AI and damage callbacks. It clears the frame to zero, not this frame.
        Self::set_owned_containment_in_parts(
            &mut self.objects,
            &mut self.partition_manager,
            rider_id,
            None,
            false,
        );
        self.stamp_contained_by_frame(rider_id, 0);
        Ok(())
    }
}
