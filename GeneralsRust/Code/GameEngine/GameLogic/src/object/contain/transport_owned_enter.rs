//! TransportContain::onContaining at the original forced boarding point.
use super::super::open_contain::owned_enter::PreparedContainEnter;
use super::*;
use crate::modules::ai_world::AiWorld;

impl TransportContain {
    pub(super) fn enter_with_world(
        &mut self,
        id: ObjectID,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let Some(passenger) = self.base.begin_enter_with_world(id, world)? else {
            return Ok(());
        };
        self.base.attach_enter_with_world(&passenger, world);
        self.on_containing_with_world(&passenger, world);
        self.base.finish_enter_with_world(&passenger, world);
        Ok(())
    }

    pub(in crate::object::contain) fn on_containing_with_world(
        &mut self,
        prepared: &PreparedContainEnter,
        world: &mut dyn AiWorld,
    ) {
        let passenger = &prepared.passenger;
        self.last_extra_slots_delta = 0;
        let timers = {
            let mut rider = passenger
                .write()
                .expect("admitted transport passenger poisoned");
            world.set_contained_held(&mut rider);
            let slots = rider.get_transport_slot_count();
            debug_assert!(
                slots > 0,
                "TransportContain contained a non-transportable rider"
            );
            let extra = slots as i32 - 1;
            self.extra_slots_in_use += extra;
            self.last_extra_slots_delta = extra;
            if rider.is_kind_of(KindOf::Hero) && rider.is_kind_of(KindOf::Salvager) {
                rider
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
            }
        };
        debug_assert!(
            self.extra_slots_in_use >= 0
                && self.extra_slots_in_use + self.base.get_contain_count() as i32
                    <= self.get_contain_max()
        );
        if self.base.get_contain_count() == 1 {
            let owner = &prepared.owner;
            let drawable = owner
                .read()
                .expect("admitted transport owner poisoned")
                .get_drawable();
            if let Some(drawable) = drawable {
                drawable
                    .write()
                    .expect("admitted transport drawable poisoned")
                    .set_model_condition_state(ModelConditionState::Loaded);
            }
        }
        self.refresh_armed_riders_with_world(&prepared.owner, &prepared.members, world);
        {
            let owner = &prepared.owner;
            let mut owner = owner.write().expect("admitted transport owner poisoned");
            owner.set_is_transporting(true);
            if owner.is_kind_of(KindOf::CliffJumper) {
                if let (Some((shot, preattack, reload)), Some(weapon)) = (
                    timers,
                    owner.get_weapon_in_slot_mut(WeaponSlotType::Secondary.into()),
                ) {
                    weapon.when_we_can_fire_again = shot;
                    weapon.when_pre_attack_finished = preattack;
                    weapon.when_last_reload_started = reload;
                }
            }
        }
    }
    pub(in crate::object::contain) fn refresh_armed_riders_with_world(
        &self,
        owner: &Arc<RwLock<Object>>,
        members: &[Arc<RwLock<Object>>],
        world: &mut dyn AiWorld,
    ) {
        if !self.module_data.armed_riders_upgrade_weapon_set {
            return;
        }
        let mut armed = false;
        // Retained identities preserve physical roster order, with one read
        // per examined rider. The admitted operation forbids replacement.
        for rider in members {
            let rider = rider.read().expect("admitted transport rider poisoned");
            armed |= transport_rider_has_viable_weapon(&rider);
            if armed {
                break;
            }
        }
        let mut owner = owner.write().expect("admitted transport owner poisoned");
        world.set_transport_weapon_upgrade(&mut owner, armed);
    }
}
