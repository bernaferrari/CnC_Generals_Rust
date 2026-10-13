//! Actual C++ destroyObject notification phases on the driving native owner.
use super::*;
use game_engine::common::thing::update_module::UpdateSleepTime;

impl BorrowedAiWorld<'_> {
    pub(crate) fn validate_owned_destroy(&self, id: ObjectID) -> Result<(), String> {
        self.validate_owned_destroy_impl(id, false)
    }
    pub(super) fn validate_destroy_after_tunnel_detach(&self, id: ObjectID) -> Result<(), String> {
        self.validate_owned_destroy_impl(id, true)
    }
    fn validate_owned_destroy_impl(
        &self,
        id: ObjectID,
        detached_by_tunnel: bool,
    ) -> Result<(), String> {
        let owner = self.object(id).ok_or("native destroy owner absent")?;
        let object = owner
            .try_read()
            .map_err(|_| "native destroy object already borrowed")?;
        object.native_destroy_plan_with_containment()?;
        self.validate_group_departure_owned(&object)?;
        if let Some(data) = object.owned_radar_data() {
            if data.object_id != id || !self.services.radar.contains_object(id) {
                return Err("native destroy radar link belongs to another owner".into());
            }
        }
        if let Some(controller) = object.controlling_player_id_from_pin()? {
            if self.players.get_player(controller as Int).is_none() {
                return Err("native destroy controller absent".into());
            }
        }
        for proxy in object.named_update_registrations("AutoHealBehavior") {
            self.services
                .update_scheduler
                .validate_owned_update_wake(&proxy)?;
        }
        let actual = self
            .services
            .update_scheduler
            .find_object_by_id(id)
            .ok_or("native destroy scheduler owner absent")?;
        if !Arc::ptr_eq(&actual, &owner) {
            return Err("foreign native destroy scheduler owner".into());
        }
        let controller = self.controller(&object);
        let ai = object.get_ai_update_interface();
        let parent = object.get_contained_by();
        drop(object);
        if !detached_by_tunnel {
            if let Some(parent) = parent {
                let parent = self.object(parent).ok_or("destroy container absent")?;
                self.validate_remove_operation(&owner, &parent, false)?;
            }
        }
        self.validate_containment_delete_owned(&owner)?;
        if let Some(controller) = controller {
            if controller
                .try_read()
                .map_err(|_| "native destroy controller already borrowed")?
                .get_num_battle_plans_active()
                != 0
            {
                return Err("native destroy battle-plan effects not admitted".into());
            }
        }
        if let Some(ai) = ai {
            ai.try_lock()
                .map_err(|_| "native destroy AI already borrowed")?
                .validate_owned_removal_callbacks_with_world(&owner, self)?;
        }
        Ok(())
    }

    pub(crate) fn destroy_object_owned(&mut self, id: ObjectID) -> Result<(), String> {
        let Some(owner) = self.object(id) else {
            return Ok(());
        };
        if owner
            .try_read()
            .map_err(|_| "native destroy owner unavailable")?
            .is_destroyed()
        {
            return Ok(());
        }
        self.validate_owned_destroy(id)?;
        let (plan, ai, proxies, wall) = {
            let object = owner.read().map_err(|_| "native destroy object poisoned")?;
            (
                object.native_destroy_plan_with_containment()?,
                object.get_ai_update_interface(),
                object.named_update_registrations("AutoHealBehavior"),
                object.is_kind_of(KindOf::WalkOnTopOfWall),
            )
        };
        // Admitted classes have no onDestroy interface. They still receive their
        // real onDelete at the later original point, without an Object loan.
        owner
            .write()
            .map_err(|_| "native destroy object poisoned")?
            .begin_native_destroy();
        if let Some(ai) = ai {
            let mut ai = ai.lock().map_err(|_| "native destroy AI poisoned")?;
            ai.set_locomotor_goal_none();
            ai.destroy_path();
        }
        self.services
            .update_scheduler
            .queue_native_destroy(id, &owner)?;
        owner
            .write()
            .map_err(|_| "native destroy object poisoned")?
            .mark_native_destroy_notified();
        // CPP Object::onDestroy removes this object from its actual container
        // before delivering onDelete to authored modules in literal tag order.
        let parent = owner
            .read()
            .map_err(|_| "destroy owner poisoned")?
            .get_contained_by();
        if let Some(parent) = parent {
            self.remove_contained(id, parent, false)?;
        }
        plan.notify_delete_with_containment(|entry| {
            self.finish_containment_delete_owned(&owner, entry)
        })?;
        for proxy in proxies {
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    self.services.current_update.as_ref(),
                    self.services.frame,
                    UpdateSleepTime::Forever.to_u32(),
                )?;
        }
        {
            let mut object = owner
                .write()
                .map_err(|_| "native destroy object poisoned")?;
            self.maintain_partition(&mut object, false);
        }
        if wall {
            self.services.pathfinder.remove_wall_piece(id);
        }
        Ok(())
    }
}
