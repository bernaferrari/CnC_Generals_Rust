//! CPP TransportContain.cpp:306-415. Physical removal of an admitted live
//! ground leaf; richer physics, death-scatter and render-bone families reject
//! before unlink instead of silently dropping their original callbacks.
use super::*;
use crate::modules::ai_world::AiWorld;

impl TransportContain {
    pub(crate) fn remove_with_world(
        &mut self,
        id: ObjectID,
        expose: bool,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let Some(prepared) = self.base.prepare_remove_with_world(id, expose, world)? else {
            return Ok(());
        };
        let owner = prepared
            .owner
            .read()
            .map_err(|_| "remove transport poisoned")?;
        let rider = prepared
            .passenger
            .read()
            .map_err(|_| "remove rider poisoned")?;
        let orientation = owner.get_orientation();
        let exit_position = if !self.module_data.exit_bone.is_empty() {
            match owner.get_drawable() {
                Some(drawable) => {
                    let drawable = drawable
                        .read()
                        .map_err(|_| "remove bone drawable poisoned")?;
                    let positions = drawable.get_pristine_bone_positions(
                        self.module_data.exit_bone.as_str(),
                        0,
                        1,
                    );
                    positions.first().map(|local| {
                        let transform = owner.convert_bone_pos_to_world_pos(Some(local), None);
                        Coord3D::new(transform.w_axis.x, transform.w_axis.y, transform.w_axis.z)
                    })
                }
                None => None,
            }
        } else {
            None
        };
        let parent_velocity = if self.module_data.keep_container_velocity_on_exit {
            owner
                .get_physics()
                .map(|physics| {
                    physics
                        .access()
                        .map(|physics| physics.get_velocity())
                        .map_err(|_| "remove parent physics poisoned")
                })
                .transpose()?
        } else {
            None
        };
        let rider_physics = rider.get_physics();
        if let Some(physics) = &rider_physics {
            let physics = physics
                .access()
                .map_err(|_| "remove rider physics poisoned")?;
            if !physics.supports_motive_force_at() {
                return Err("remove physics does not admit supplied owner/frame".into());
            }
        }
        let owner_above = world.above_terrain(&owner)?;
        let owner_dead = owner.is_effectively_dead();
        let slots = rider.get_transport_slot_count();
        if slots == 0 || self.extra_slots_in_use < slots as i32 - 1 {
            return Err("invalid transport removal slot count".into());
        }
        let ai = rider.get_ai_update_interface();
        drop(rider);
        drop(owner);
        if parent_velocity.is_some() && rider_physics.is_some() {
            world.physics_mass_for_owner(&prepared.passenger)?;
        }
        if !orientation.is_finite()
            || exit_position.is_some_and(|point| !point.is_finite())
            || parent_velocity.is_some_and(|velocity| !velocity.is_finite())
        {
            return Err("transport exit pose/velocity is nonfinite".into());
        }
        // No recoverable store/guard/service failure follows this unlink.
        self.base.detach_remove_with_world(&prepared, world);
        self.base.removing_sounds_with_world(&prepared, world);
        {
            let mut rider = prepared
                .passenger
                .write()
                .expect("admitted remove rider poisoned");
            world.clear_contained_held(&mut rider);
        }
        if let Some(position) = exit_position {
            world
                .set_object_position_handle(&prepared.passenger, &position)
                .expect("admitted exit bone position");
        }
        if self.module_data.orient_like_container_on_exit {
            world
                .set_object_orientation_handle(&prepared.passenger, orientation)
                .expect("admitted exit orientation");
        }
        let rider_mass = if parent_velocity.is_some() && rider_physics.is_some() {
            Some(
                world
                    .physics_mass_for_owner(&prepared.passenger)
                    .expect("admitted cargo Physics mass"),
            )
        } else {
            None
        };
        {
            let rider = prepared
                .passenger
                .read()
                .expect("admitted remove rider poisoned");
            if let (Some(velocity), Some(physics)) = (parent_velocity, &rider_physics) {
                let mut physics = physics.access().expect("admitted rider physics poisoned");
                let force = velocity * rider_mass.expect("observed cargo Physics mass");
                let pitch = physics.get_center_of_mass_offset() * self.module_data.exit_pitch_rate;
                let wake = physics.apply_motive_force_with_object_at(&force, &rider, world.frame());
                physics.set_pitch_rate(pitch);
                drop(physics);
                if wake {
                    world.wake_physics_after_force(&rider);
                }
            }
        }
        self.extra_slots_in_use -= slots as i32 - 1;
        if self.base.get_contain_count() == 0 {
            let mut owner = prepared
                .owner
                .write()
                .expect("admitted remove owner poisoned");
            owner.clear_model_condition_state(crate::common::ModelConditionFlags::LOADED);
        }
        if owner_above {
            if let Some(physics) = rider_physics {
                physics
                    .access()
                    .expect("admitted rider physics poisoned")
                    .set_allow_to_fall(true);
            }
        }
        // C++ callback point: no passenger Object loan survives AI operations.
        if let Some(ai) = &ai {
            if self.module_data.go_aggressive_on_exit {
                ai.lock()
                    .expect("admitted removal AI poisoned")
                    .set_attitude(AIAttitudeType::Aggressive)
                    .expect("actual removal attitude");
            }
        }
        if owner_dead {
            world.scatter_removed(&prepared.passenger, &prepared.owner);
        }
        if let Some(ai) = ai {
            if self.module_data.reset_mood_check_time_on_exit {
                ai.lock()
                    .expect("admitted removal AI poisoned")
                    .wake_up_and_attempt_to_target_at(world.frame())
                    .expect("admitted removal wake");
            }
        }
        self.frame_exit_not_busy = world.frame().wrapping_add(self.module_data.exit_delay);
        self.refresh_armed_riders_with_world(&prepared.owner, &prepared.members, world);
        {
            let mut owner = prepared
                .owner
                .write()
                .expect("admitted remove owner poisoned");
            owner.set_is_transporting(self.base.get_contain_count() > 0);
            let timers = if owner.is_kind_of(KindOf::CliffJumper) {
                owner
                    .get_weapon_in_slot(WeaponSlotType::Secondary.into())
                    .map(|weapon| {
                        (
                            weapon.when_we_can_fire_again,
                            weapon.when_pre_attack_finished,
                            weapon.when_last_reload_started,
                        )
                    })
            } else {
                None
            };
            drop(owner);
            let mut rider = prepared
                .passenger
                .write()
                .expect("admitted remove rider poisoned");
            if rider.is_kind_of(KindOf::Hero) && rider.is_kind_of(KindOf::Salvager) {
                if let (Some((shot, preattack, reload)), Some(weapon)) = (
                    timers,
                    rider.get_weapon_in_slot_mut(WeaponSlotType::Secondary.into()),
                ) {
                    weapon.when_we_can_fire_again = shot;
                    weapon.when_pre_attack_finished = preattack;
                    weapon.when_last_reload_started = reload;
                }
            }
        }
        self.base.finish_remove_with_world(&prepared, world);
        Ok(())
    }
}
