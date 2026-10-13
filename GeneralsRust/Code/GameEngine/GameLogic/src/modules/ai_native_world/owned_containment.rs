//! Ordered cargo and carrier observations from this exact session's modules.
use super::*;

pub(crate) struct PhysicsCargoObservation {
    pub(crate) contained_mass: Real,
    pub(crate) carrier_deck_height: Real,
}

pub(super) fn depart_supplied_group_owned(
    groups: &mut Vec<Arc<RwLock<crate::ai::AiGroup>>>,
    owner: &mut Object,
    clear_before_remove: bool,
) {
    let Some(group_id) = owner.get_group_id() else {
        return;
    };
    let index = groups
        .iter()
        .position(|group| group.read().expect("admitted group poisoned").get_id() == group_id)
        .expect("admitted supplied group retired");
    if clear_before_remove {
        owner.leave_group();
    }
    let empty = groups[index]
        .write()
        .expect("admitted group poisoned")
        .remove_owned_member(owner);
    if empty {
        groups.remove(index);
    }
}

impl BorrowedAiWorld<'_> {
    pub(super) fn observe_contained_weapon_bonus_owned(
        &self,
        owner: &Object,
    ) -> Result<crate::common::types::WeaponBonusConditionFlags, String> {
        let Some(parent) = owner.get_contained_by() else {
            return Ok(crate::common::types::WeaponBonusConditionFlags::empty());
        };
        if parent == owner.get_id() {
            return Err("cyclic contained weapon bonus owner".into());
        }
        let parent = self
            .object(parent)
            .ok_or("contained weapon bonus owner absent")?;
        let parent = parent
            .read()
            .map_err(|_| "contained weapon bonus owner poisoned")?;
        let Some(contain) = parent.get_contain() else {
            return Ok(crate::common::types::WeaponBonusConditionFlags::empty());
        };
        let pass = contain
            .try_lock()
            .map_err(|_| "contained weapon bonus module borrowed")?
            .passes_weapon_bonus_to_passengers();
        Ok(if pass {
            parent.get_weapon_bonus_condition()
        } else {
            crate::common::types::WeaponBonusConditionFlags::empty()
        })
    }

    pub(super) fn validate_containment_attachment_owned(
        &self,
        passenger: &Arc<RwLock<Object>>,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let passenger_id = passenger
            .read()
            .map_err(|_| "attach passenger poisoned")?
            .get_id();
        let actual = self.object(passenger_id).ok_or("attach passenger absent")?;
        if !Arc::ptr_eq(passenger, &actual) {
            return Err("attach passenger belongs to another session".into());
        }
        let mut current = Arc::clone(owner);
        let mut visited = std::collections::BTreeSet::new();
        loop {
            let object = current.read().map_err(|_| "attach ancestor poisoned")?;
            let id = object.get_id();
            if id == passenger_id {
                return Err("cyclic containment attachment".into());
            }
            if !visited.insert(id) {
                return Err("cyclic existing containment ancestors".into());
            }
            let actual = self.object(id).ok_or("attach ancestor absent")?;
            if !Arc::ptr_eq(&current, &actual) {
                return Err("attach ancestor belongs to another session".into());
            }
            let parent = object.get_contained_by();
            drop(object);
            let Some(parent) = parent else {
                return Ok(());
            };
            current = self.object(parent).ok_or("attach ancestor owner absent")?;
        }
    }

    pub(super) fn validate_group_departure_owned(&self, owner: &Object) -> Result<(), String> {
        let Some(group_id) = owner.get_group_id() else {
            return Ok(());
        };
        let mut matched = None;
        for (index, group) in self.services.groups.iter().enumerate() {
            let group = group.try_read().map_err(|_| "containment group borrowed")?;
            if group.get_id() == group_id {
                if matched.replace(index).is_some() {
                    return Err("duplicate supplied group identity".into());
                }
                if !group.is_member(owner.get_id()) {
                    return Err("group/backlink membership disagree".into());
                }
            }
        }
        let index = matched.ok_or("containment group absent from this owner")?;
        drop(
            self.services.groups[index]
                .try_write()
                .map_err(|_| "containment group already borrowed")?,
        );
        Ok(())
    }

    pub(super) fn leave_group_owned(&mut self, owner: &mut Object) {
        depart_supplied_group_owned(self.services.groups, owner, true);
    }

    fn cargo_roster_owned(&self, owner: &Arc<RwLock<Object>>) -> Result<Vec<ObjectID>, String> {
        let object = owner.read().map_err(|_| "cargo owner poisoned")?;
        let actual = self.object(object.get_id()).ok_or("cargo owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("cargo belongs to another session".into());
        }
        let contain = object.get_contain();
        drop(object);
        let Some(contain) = contain else {
            return Ok(Vec::new());
        };
        let roster = contain
            .lock()
            .map_err(|_| "cargo module poisoned")?
            .get_contained_objects()
            .into_owned();
        Ok(roster)
    }

    fn contained_mass_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        active: &mut std::collections::BTreeSet<ObjectID>,
    ) -> Result<Real, String> {
        let id = owner.read().map_err(|_| "cargo owner poisoned")?.get_id();
        let actual = self.object(id).ok_or("cargo owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("cargo belongs to another session".into());
        }
        if !active.insert(id) {
            return Err("cyclic physical cargo ownership".into());
        }
        let mut mass = 0.0;
        let mut members = std::collections::BTreeSet::new();
        // Preserve the C++ physical list order; no contain/Object/module loan
        // spans a nested mass query.
        for passenger in self.cargo_roster_owned(owner)? {
            if !members.insert(passenger) {
                return Err("duplicate physical cargo member".into());
            }
            let passenger = self
                .object(passenger)
                .ok_or("physical cargo member absent")?;
            let physics = {
                let passenger = passenger.read().map_err(|_| "cargo passenger poisoned")?;
                if passenger.get_contained_by() != Some(id) {
                    return Err("cargo membership/backlink disagree".into());
                }
                passenger.get_physics()
            };
            if let Some(physics) = physics {
                let base = physics.owned_base_mass()?;
                let nested = self.contained_mass_owned(&passenger, active)?;
                mass += base + nested;
                if !mass.is_finite() {
                    return Err("physical cargo mass is nonfinite".into());
                }
            }
        }
        active.remove(&id);
        Ok(mass)
    }

    pub(crate) fn observe_physics_cargo_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<PhysicsCargoObservation, String> {
        let (id, producer, on_deck) = {
            let object = owner.read().map_err(|_| "cargo owner poisoned")?;
            (
                object.get_id(),
                object.get_producer_id(),
                object.test_status(ObjectStatusTypes::DeckHeightOffset),
            )
        };
        let actual = self.object(id).ok_or("cargo owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("cargo observation belongs to another session".into());
        }
        let mass = self.contained_mass_owned(owner, &mut std::collections::BTreeSet::new())?;
        let carrier_deck_height = if on_deck {
            match self.object(producer) {
                Some(producer) => producer
                    .read()
                    .map_err(|_| "carrier producer poisoned")?
                    .with_parking_place_behavior(|parking| parking.get_landing_deck_height_offset())
                    .unwrap_or(0.0),
                None => 0.0,
            }
        } else {
            0.0
        };
        if !carrier_deck_height.is_finite() {
            return Err("carrier deck height is nonfinite".into());
        }
        Ok(PhysicsCargoObservation {
            contained_mass: mass,
            carrier_deck_height,
        })
    }
}

impl BorrowedAiWorld<'_> {
    fn validate_containment_pose_tree(
        &self,
        owner: &Arc<RwLock<Object>>,
        active: &mut std::collections::BTreeSet<ObjectID>,
        containment_effects: bool,
    ) -> Result<(), String> {
        let (id, contain, drawable) = {
            let object = owner
                .try_read()
                .map_err(|_| "pose owner is already borrowed")?;
            if containment_effects {
                object.controlling_player_id_from_pin()?;
                object.validate_native_containment_effects()?;
            }
            (object.get_id(), object.get_contain(), object.get_drawable())
        };
        let actual = self.object(id).ok_or("pose owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("pose owner belongs to another session".into());
        }
        if !active.insert(id) {
            return Err("cyclic containment pose graph".into());
        }
        if let Some(drawable) = drawable {
            drop(
                drawable
                    .try_write()
                    .map_err(|_| "pose Drawable is already borrowed")?,
            );
        }
        if let Some(contain) = contain {
            let roster = {
                let contain = contain
                    .try_lock()
                    .map_err(|_| "pose contain is already borrowed")?;
                if !contain.supports_owned_redeployment() {
                    return Err("container has no supplied transform callback".into());
                }
                if contain.passengers_in_turret() {
                    owner
                        .try_read()
                        .map_err(|_| "pose owner is borrowed")?
                        .containment_turret_firepoint(0)?;
                }
                contain.get_contained_objects().into_owned()
            };
            let mut members = std::collections::BTreeSet::new();
            for member in roster {
                if !members.insert(member) {
                    return Err("duplicate pose cargo member".into());
                }
                let member = self.object(member).ok_or("pose cargo member absent")?;
                if member
                    .try_read()
                    .map_err(|_| "pose cargo member is borrowed")?
                    .get_contained_by()
                    != Some(id)
                {
                    return Err("pose cargo member/backlink disagree".into());
                }
                // Only the entering/removing object receives HELD. Nested
                // children receive pose callbacks, not a new disabled edge.
                self.validate_containment_pose_tree(&member, active, false)?;
            }
        }
        active.remove(&id);
        Ok(())
    }

    pub(crate) fn validate_containment_transform_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_containment_pose_tree(owner, &mut std::collections::BTreeSet::new(), false)
    }

    pub(crate) fn validate_containment_member_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_containment_pose_tree(owner, &mut std::collections::BTreeSet::new(), true)
    }

    /// CPP Object.cpp1740 and OpenContain.cpp1174: no Physics/Object/Contain
    /// lease crosses the reverse-list virtual rider transform callback.
    pub(crate) fn set_physics_transform_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        matrix: &glam::Mat4,
    ) -> Result<(), String> {
        self.validate_containment_transform_owned(owner)?;
        self.apply_containment_transform_owned(owner, matrix)
    }

    fn apply_containment_transform_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        matrix: &glam::Mat4,
    ) -> Result<(), String> {
        let observation = owner
            .write()
            .map_err(|_| "pose owner poisoned")?
            .begin_owned_transform_with_services(
                matrix,
                self.services.terrain,
                self.services.collision,
            )?;
        if observation.pose_changed {
            let (position, contain, drawable) = {
                let object = owner.read().map_err(|_| "pose owner poisoned")?;
                (
                    *object.get_position(),
                    object.get_contain(),
                    object.get_drawable(),
                )
            };
            if let Some(contain) = contain {
                let (roster, hidden, turret) = {
                    let mut contain = contain.lock().map_err(|_| "pose contain poisoned")?;
                    (
                        contain.begin_owned_redeployment(),
                        contain.drawable_hidden_after_transform(),
                        contain.passengers_in_turret(),
                    )
                };
                if let (Some(drawable), Some(hidden)) = (drawable, hidden) {
                    drawable
                        .write()
                        .map_err(|_| "pose Drawable poisoned")?
                        .set_drawable_hidden(hidden)
                        .map_err(|error| error.to_string())?;
                }
                let base_bones = if roster.is_empty() {
                    Vec::new()
                } else {
                    owner
                        .read()
                        .map_err(|_| "pose owner poisoned")?
                        .containment_firepoint_matrices(false)?
                };
                for id in roster {
                    let member = self.object(id).ok_or("admitted pose rider retired")?;
                    let (bone, index, enclosing) = {
                        let rider = member.read().map_err(|_| "pose rider poisoned")?;
                        let mut contain = contain.lock().map_err(|_| "pose contain poisoned")?;
                        (
                            contain.owned_next_firepoint(&base_bones),
                            contain.owned_firepoint_index(),
                            contain.is_enclosing_container_for(&rider),
                        )
                    };
                    let point = match bone {
                        Some(_) if turret => Some(
                            owner
                                .read()
                                .map_err(|_| "pose owner poisoned")?
                                .containment_turret_firepoint(index)?,
                        ),
                        other => other,
                    };
                    let target = if let Some(point) = point {
                        if enclosing {
                            let mut matrix = member
                                .read()
                                .map_err(|_| "pose rider poisoned")?
                                .get_transform_matrix();
                            matrix.w_axis = point.w_axis;
                            matrix
                        } else {
                            point
                        }
                    } else {
                        let mut matrix = member
                            .read()
                            .map_err(|_| "pose rider poisoned")?
                            .get_transform_matrix();
                        matrix.w_axis = position.extend(1.0);
                        matrix
                    };
                    self.apply_containment_transform_owned(&member, &target)?;
                    contain
                        .lock()
                        .map_err(|_| "pose contain poisoned")?
                        .advance_owned_firepoint();
                }
            }
        }
        let changes = &mut self.services.changed_trigger_objects;
        let moved = &mut self.services.terrain_moved_objects;
        owner
            .write()
            .map_err(|_| "pose owner poisoned")?
            .finish_owned_transform_with_services(
                observation,
                self.services.terrain,
                self.services.frame,
                &mut |id| changes.push(id),
                &mut |object| moved.push(object.get_id()),
            )
    }
}
