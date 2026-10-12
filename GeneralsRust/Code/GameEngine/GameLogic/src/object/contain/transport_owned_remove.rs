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
        if self.module_data.orient_like_container_on_exit {
            return Err("native remove orientation service not admitted".into());
        }
        if !self.module_data.exit_bone.is_empty() && owner.get_drawable().is_some() {
            return Err("native remove render bone service not admitted".into());
        }
        if self.module_data.keep_container_velocity_on_exit
            && owner.get_physics().is_some()
            && rider.get_physics().is_some()
        {
            return Err("native remove motive force service not admitted".into());
        }
        if world.above_terrain(&owner)? && rider.get_physics().is_some() {
            return Err("native remove allow-to-fall physics not admitted".into());
        }
        let slots = rider.get_transport_slot_count();
        if slots == 0 || self.extra_slots_in_use < slots as i32 - 1 {
            return Err("invalid transport removal slot count".into());
        }
        let ai = rider.get_ai_update_interface();
        drop(rider);
        drop(owner);
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
        // A configured ExitBone without a Drawable does nothing in C++.
        // No missing-bone fallback moves the rider to (0,0,0).
        self.extra_slots_in_use -= slots as i32 - 1;
        if self.base.get_contain_count() == 0 {
            let mut owner = prepared
                .owner
                .write()
                .expect("admitted remove owner poisoned");
            owner.clear_model_condition_state(crate::common::ModelConditionFlags::LOADED);
        }
        // C++ callback point: no passenger Object loan survives AI operations.
        if let Some(ai) = ai {
            let mut ai = ai.lock().expect("admitted removal AI poisoned");
            if self.module_data.go_aggressive_on_exit {
                ai.set_attitude(AIAttitudeType::Aggressive)
                    .expect("actual removal attitude");
            }
            if self.module_data.reset_mood_check_time_on_exit {
                ai.wake_up_and_attempt_to_target_at(world.frame())
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
