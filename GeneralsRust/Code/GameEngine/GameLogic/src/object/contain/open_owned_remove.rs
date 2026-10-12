//! OpenContain.cpp:607-660 physical removal through the driving owner.
use super::*;
use crate::modules::ai_world::AiWorld;

pub(in crate::object::contain) struct PreparedContainRemove {
    pub owner: Arc<RwLock<Object>>,
    pub passenger: Arc<RwLock<Object>>,
    pub members: Vec<Arc<RwLock<Object>>>,
    stealth: bool,
    expose: bool,
    enclosing: bool,
}
impl OpenContain {
    pub(in crate::object::contain) fn prepare_remove_with_world(
        &self,
        id: ObjectID,
        expose: bool,
        world: &dyn AiWorld,
    ) -> Result<Option<PreparedContainRemove>, String> {
        if !self.contained_object_ids.contains(&id) {
            return Ok(None);
        }
        let owner = world.object(self.object_id).ok_or("remove owner missing")?;
        let passenger = world.object(id).ok_or("remove passenger missing")?;
        if Arc::ptr_eq(&owner, &passenger) {
            return Err("self containment removal".into());
        }
        world.validate_remove_operation(&passenger, &owner, expose)?;
        let rider = passenger.read().map_err(|_| "remove passenger poisoned")?;
        if rider.get_contained_by() != Some(self.object_id) {
            return Ok(None);
        }
        let stealth = rider.is_kind_of(KindOf::StealthGarrison);
        let enclosing = self.is_enclosing_container_for(&rider);
        if stealth && self.stealth_units_contained == 0 {
            return Err("invalid contained stealth count".into());
        }
        drop(rider);
        drop(passenger.write().map_err(|_| "remove passenger poisoned")?);
        drop(owner.write().map_err(|_| "remove owner poisoned")?);
        let mut members = Vec::with_capacity(self.contained_object_ids.len().saturating_sub(1));
        for member_id in &self.contained_object_ids {
            if *member_id == id {
                continue;
            }
            let member = world
                .object(*member_id)
                .ok_or("remaining contain member missing")?;
            let member_guard = member
                .read()
                .map_err(|_| "remaining contain member poisoned")?;
            member_guard.validate_native_leaf_enter_effects()?;
            drop(member_guard);
            drop(
                member
                    .write()
                    .map_err(|_| "remaining contain member poisoned")?,
            );
            members.push(member);
        }
        Ok(Some(PreparedContainRemove {
            owner,
            passenger,
            members,
            stealth,
            expose,
            enclosing,
        }))
    }
    pub(in crate::object::contain) fn detach_remove_with_world(
        &mut self,
        prepared: &PreparedContainRemove,
        world: &mut dyn AiWorld,
    ) {
        let id = prepared
            .passenger
            .read()
            .expect("admitted remove passenger poisoned")
            .get_id();
        self.contained_object_ids
            .retain(|candidate| *candidate != id);
        if prepared.stealth {
            self.stealth_units_contained -= 1;
            if prepared.expose {
                world.expose_removed(&prepared.passenger);
            }
        }
        let owner = prepared
            .owner
            .read()
            .expect("admitted remove owner poisoned");
        let position = *owner.get_position();
        let layer = owner.get_layer();
        drop(owner);
        let mut rider = prepared
            .passenger
            .write()
            .expect("admitted remove passenger poisoned");
        if prepared.enclosing {
            world.add_to_world(&mut rider);
            world
                .set_object_position(&mut rider, &position)
                .expect("admitted remove position effects");
        }
        world.set_object_layer(&mut rider, layer);
        drop(rider);
        if self.last_unload_sound_frame != world.frame() {
            if let Some(sound) = &self.module_data.exit_sound {
                let mut event = AudioEventRts::new(sound.get_event_name());
                event.set_object_id(self.object_id);
                world.play_containment_sound(&event);
            }
            self.last_unload_sound_frame = world.frame();
        }
    }
    pub(in crate::object::contain) fn removing_sounds_with_world(
        &self,
        prepared: &PreparedContainRemove,
        world: &mut dyn AiWorld,
    ) {
        let owner = prepared
            .owner
            .read()
            .expect("admitted remove owner poisoned");
        let mut exit = owner.get_template().get_sound_exit();
        exit.set_object_id(owner.get_id());
        drop(owner);
        world.play_containment_sound(&exit);
        let rider = prepared
            .passenger
            .read()
            .expect("admitted remove passenger poisoned");
        let mut falling = rider.get_template().get_sound_falling();
        falling.set_object_id(rider.get_id());
        drop(rider);
        world.play_containment_sound(&falling);
    }
    pub(in crate::object::contain) fn finish_remove_with_world(
        &self,
        prepared: &PreparedContainRemove,
        world: &mut dyn AiWorld,
    ) {
        let mut rider = prepared
            .passenger
            .write()
            .expect("admitted remove passenger poisoned");
        rider.finish_removed_from_local();
        world.finish_removed_partition(&mut rider);
    }
    pub(crate) fn remove_with_world(
        &mut self,
        id: ObjectID,
        expose: bool,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let Some(prepared) = self.prepare_remove_with_world(id, expose, world)? else {
            return Ok(());
        };
        self.detach_remove_with_world(&prepared, world);
        self.removing_sounds_with_world(&prepared, world);
        self.finish_remove_with_world(&prepared, world);
        Ok(())
    }
}

impl OpenContain {
    /// CPP OpenContain.cpp:1749-1802: resolve pointers, remove enclosed items
    /// from world, friend-set owner only. Do not replay onContaining or replace
    /// saved contained-by timestamps with this operation's current frame.
    pub(crate) fn load_post_process_with_world(
        &mut self,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        if !self.contained_object_ids.is_empty() {
            return Err("contain postload list not empty".into());
        }
        let owner = world
            .object(self.object_id)
            .ok_or("contain postload owner missing")?;
        let mut seen = std::collections::BTreeSet::new();
        let mut resolved = Vec::with_capacity(self.xfer_contain_id_list.len());
        for id in &self.xfer_contain_id_list {
            if !seen.insert(*id) {
                return Err("duplicate contained postload id".into());
            }
            let passenger = world
                .object(*id)
                .ok_or("contained postload object missing")?;
            if Arc::ptr_eq(&passenger, &owner) {
                return Err("self contained postload reference".into());
            }
            let rider = passenger
                .read()
                .map_err(|_| "contained postload object poisoned")?;
            rider.validate_native_leaf_enter_effects()?;
            rider.controlling_player_id_from_pin()?;
            let enclosing = self.is_enclosing_container_for(&rider);
            drop(rider);
            drop(
                passenger
                    .write()
                    .map_err(|_| "contained postload object poisoned")?,
            );
            resolved.push((*id, passenger, enclosing));
        }
        world.validate_containment_services()?;
        for (id, passenger, enclosing) in resolved {
            self.contained_object_ids.push(id);
            let mut rider = passenger
                .write()
                .expect("admitted postload passenger poisoned");
            if enclosing {
                world.remove_from_world(&mut rider);
            }
            rider.restore_contained_owner(self.object_id);
        }
        self.xfer_contain_id_list.clear();
        Ok(())
    }
}

impl OpenContain {
    /// OpenContain.cpp exitObjectViaDoor: door pulse precedes the zero-path
    /// branch. Native removal already admitted this object's map footprint.
    pub(in crate::object::contain) fn finish_zero_path_exit_with_world(
        &mut self,
        world: &mut dyn AiWorld,
    ) {
        assert!(self.module_data.number_of_exit_paths <= 0);
        self.door_close_countdown = self.module_data.door_open_time;
        if self.door_close_countdown != 0 {
            let owner = world
                .object(self.get_object_id())
                .expect("admitted exit owner retired");
            owner
                .write()
                .expect("admitted exit owner poisoned")
                .clear_and_set_model_condition_flags(
                    MODELCONDITION_DOOR_1_CLOSING,
                    MODELCONDITION_DOOR_1_OPENING,
                )
                .expect("admitted exit model flags");
        }
    }
}
