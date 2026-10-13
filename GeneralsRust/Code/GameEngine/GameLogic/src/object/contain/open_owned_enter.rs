//! Original-point OpenContain boarding using a supplied object/service owner.
use super::*;
use crate::modules::ai_world::AiWorld;

/// Retained C++ pointer-equivalent identities for one admitted synchronous add.
/// All recoverable store/guard failures are checked before the literal roster push.
pub(in crate::object::contain) struct PreparedContainEnter {
    pub owner: Arc<RwLock<Object>>,
    pub passenger: Arc<RwLock<Object>>,
    /// Existing live members in literal roster order, then the new passenger;
    /// unresolved entries retain the prior skip behavior. Identities stay fixed
    /// through the synchronous attach/onContaining/onContainedBy sequence.
    pub members: Vec<Arc<RwLock<Object>>>,
}

impl OpenContain {
    pub(crate) fn begin_enter_with_world(
        &mut self,
        id: ObjectID,
        world: &mut dyn AiWorld,
    ) -> Result<Option<PreparedContainEnter>, String> {
        let passenger = world.object(id).ok_or("contain passenger missing")?;
        let owner = world
            .object(self.object_id)
            .ok_or("contain owner missing")?;
        let rider = passenger.read().map_err(|_| "contain passenger poisoned")?;
        let owner_guard = owner.read().map_err(|_| "contain owner poisoned")?;
        if rider.get_contained_by().is_some() {
            return Ok(None);
        }
        debug_assert!(
            self.is_valid_container_for_with_world(&rider, &owner_guard, false, world),
            "forced boarding passenger invalid for supplied container"
        );
        drop(owner_guard);
        drop(rider);
        world.validate_containment_attachment(&passenger, &owner)?;
        // Poisoned participants are rejected before the first canonical mutation.
        // The synchronous owner forbids replacement/removal until this original
        // callback chain completes; callbacks may still mutate their fields.
        drop(
            passenger
                .write()
                .map_err(|_| "contain passenger poisoned")?,
        );
        drop(owner.write().map_err(|_| "contain owner poisoned")?);
        let mut members = Vec::with_capacity(self.contained_object_ids.len() + 1);
        for member_id in &self.contained_object_ids {
            if let Some(member) = world.object(*member_id) {
                drop(member.write().map_err(|_| "contained rider poisoned")?);
                members.push(member);
            }
        }
        members.push(Arc::clone(&passenger));
        for member in &members {
            world.validate_containment_member(member)?;
            let drawable = member
                .read()
                .map_err(|_| "contained rider poisoned")?
                .get_drawable();
            if let Some(drawable) = drawable {
                drop(
                    drawable
                        .write()
                        .map_err(|_| "contained drawable poisoned")?,
                );
            }
        }

        if let Some(drawable) = owner
            .read()
            .map_err(|_| "contain owner poisoned")?
            .get_drawable()
        {
            drop(drawable.write().map_err(|_| "contain drawable poisoned")?);
        }
        Ok(Some(PreparedContainEnter {
            owner,
            passenger,
            members,
        }))
    }

    pub(crate) fn attach_enter_with_world(
        &mut self,
        prepared: &PreparedContainEnter,
        world: &mut dyn AiWorld,
    ) {
        let passenger = &prepared.passenger;
        let rider = passenger
            .read()
            .expect("admitted contain passenger poisoned");
        let id = rider.get_id();
        let stealth = rider.is_kind_of(KindOf::StealthGarrison);
        let enclosing = self.is_enclosing_container_for(&rider);
        let mask = world.passenger_player_mask(&rider);
        drop(rider);
        self.add_to_contain_list_id(id, stealth)
            .expect("literal roster insertion is infallible");
        if let Some(mask) = mask {
            self.player_who_entered = mask;
        }
        if enclosing {
            let mut rider = passenger
                .write()
                .expect("admitted contain passenger poisoned");
            world.remove_from_world(&mut rider);
        }
        self.redeploy_with_world(prepared, world);
        if self.load_sounds_enabled {
            let owner = prepared
                .owner
                .read()
                .expect("admitted contain owner poisoned");
            let mut sound = owner.get_template().get_sound_enter();
            sound.set_object_id(self.object_id);
            drop(owner);
            world.play_containment_sound(&sound);
        }
    }

    pub(crate) fn finish_enter_with_world(
        &mut self,
        prepared: &PreparedContainEnter,
        world: &mut dyn AiWorld,
    ) {
        let passenger = &prepared.passenger;
        let mut rider = passenger
            .write()
            .expect("admitted contain passenger poisoned");
        let enclosing = self.is_enclosing_container_for(&rider);
        rider.admit_contained_by_at(self.object_id, enclosing, world.frame());
        world.maintain_contained_partition(&mut rider, self.is_garrisonable());
        drop(rider);
        if self.load_sounds_enabled && self.last_load_sound_frame != world.frame() {
            if let Some(sound) = self.module_data.enter_sound.as_ref() {
                let name = sound.get_event_name();
                if leftover_contain_sound_name_is_playable(name) {
                    let mut event = AudioEventRts::new(name);
                    event.set_object_id(self.object_id);
                    world.play_containment_sound(&event);
                }
            }
            self.last_load_sound_frame = world.frame();
        }
    }

    pub(crate) fn enter_with_world(
        &mut self,
        id: ObjectID,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let Some(passenger) = self.begin_enter_with_world(id, world)? else {
            return Ok(());
        };
        self.attach_enter_with_world(&passenger, world);
        self.finish_enter_with_world(&passenger, world);
        Ok(())
    }

    fn redeploy_with_world(&mut self, prepared: &PreparedContainEnter, world: &mut dyn AiWorld) {
        self.begin_owned_redeployment();
        let (points, position) = {
            let owner = prepared
                .owner
                .read()
                .expect("admitted contain owner poisoned");
            (
                owner
                    .containment_firepoint_matrices(false)
                    .expect("admitted contain FIREPOINT matrices"),
                *owner.get_position(),
            )
        };
        for rider in prepared.members.iter().rev() {
            let point = self.owned_next_firepoint(&points);
            let matrix = match point {
                Some(_) if self.module_data.passengers_in_turret => prepared
                    .owner
                    .read()
                    .expect("admitted contain owner poisoned")
                    .containment_turret_firepoint(self.owned_firepoint_index())
                    .expect("admitted turret FIREPOINT"),
                Some(matrix) => matrix,
                None => {
                    let mut matrix = rider
                        .read()
                        .expect("admitted contained rider poisoned")
                        .get_transform_matrix();
                    matrix.w_axis = position.extend(1.0);
                    matrix
                }
            };
            let enclosing = {
                let rider = rider.read().expect("admitted contained rider poisoned");
                self.is_enclosing_container_for(&rider)
            };
            world.place_contained_handle(rider, matrix, enclosing);
            self.advance_owned_firepoint();
        }
    }
}
