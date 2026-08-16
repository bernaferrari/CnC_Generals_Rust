// Move/damage/delete and trigger enter/exit tests
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

impl Team {
    /// Move team to destination
    pub fn move_team_to(&mut self, _destination: Coord3D) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        // C++ Team::moveTeamTo currently performs no command issue.
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                let _ = object_guard.is_effectively_dead() || object_guard.is_destroyed();
            });
        }
    }

    /// Damage all team members
    pub fn damage_team_members(&mut self, amount: Real) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                    return;
                }

                if amount < 0.0 {
                    object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                } else {
                    let mut damage_info = DamageInfo::with_simple(
                        amount,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    let _ = object_guard.attempt_damage(&mut damage_info);
                }
            });
        }

        // C++ Team::damageTeamMembers returns FALSE.
        false
    }

    /// Delete team (mark for destruction)
    pub fn delete_team(&mut self, ignore_dead: Bool) {
        // Wave 256: empty dual-world → no factory member walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.is_default_team_for_controller() {
            self.evacuate_team_containers();
        }

        let members = self.members.clone();
        if let Ok(mut manager) = get_object_manager().write() {
            for object_id in members {
                let should_destroy = OBJECT_REGISTRY
                    .with_object(object_id, |object_guard| {
                        !(ignore_dead && object_guard.is_effectively_dead())
                    })
                    .unwrap_or(false);
                if !should_destroy {
                    continue;
                }
                manager.destroy_object(object_id);
            }
        }
    }

    /// Get estimated team position (first member position)
    pub fn get_estimate_team_position(&self) -> Option<Coord3D> {
        // Wave 256: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let object_id = *self.members.first()?;
        OBJECT_REGISTRY.with_object(object_id, |object_guard| *object_guard.get_position())
    }

    fn locomotor_surface_matches(
        ai: Option<Arc<Mutex<dyn crate::modules::AIUpdateInterface>>>,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> bool {
        // C++ parity (Team.cpp static locoSetMatches):
        // script condition bits are remapped before comparing against locomotor surface bits.
        let mut surface_bits = which_to_consider as UnsignedInt;
        surface_bits = (surface_bits & 0x01) | ((surface_bits & 0x02) << 2);
        let considered = surface_bits as LocomotorSurfaceTypeMask;

        if let Some(ai_arc) = ai {
            if let Ok(ai_guard) = ai_arc.lock() {
                if let Some(loco_arc) = ai_guard.get_cur_locomotor() {
                    if let Ok(loco_guard) = loco_arc.lock() {
                        return (loco_guard.get_legal_surfaces() & considered) != 0;
                    }
                }
            }
        }

        (SURFACE_GROUND & considered) != 0
    }

    fn object_in_trigger(object: &crate::object::Object, trigger: &PolygonTrigger) -> bool {
        object.is_inside_trigger(trigger)
    }

    fn object_did_enter(object_id: ObjectID, trigger: &PolygonTrigger) -> bool {
        OBJECT_REGISTRY
            .with_object(object_id, |object| object.did_enter(trigger))
            .unwrap_or(false)
    }

    fn object_did_exit(object_id: ObjectID, trigger: &PolygonTrigger) -> bool {
        OBJECT_REGISTRY
            .with_object(object_id, |object| object.did_exit(trigger))
            .unwrap_or(false)
    }

    pub fn did_all_enter(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.entered_or_exited {
            return false;
        }

        let mut entered = false;
        let mut outside = false;

        for &object_id in &self.members {
            let Some((did_enter, is_outside)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }

                    let did_enter = Self::object_did_enter(object_id, trigger);
                    let is_outside = !did_enter && !Self::object_in_trigger(object_guard, trigger);
                    Some((did_enter, is_outside))
                })
                .flatten()
            else {
                continue;
            };

            if did_enter {
                entered = true;
            } else if is_outside {
                outside = true;
            }
        }

        entered && !outside
    }

    pub fn did_partial_enter(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.entered_or_exited {
            return false;
        }

        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return false;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return false;
                    }
                    Self::object_did_enter(object_id, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    pub fn did_partial_exit(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.entered_or_exited {
            return false;
        }

        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return false;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return false;
                    }
                    Self::object_did_exit(object_id, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    pub fn did_all_exit(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.entered_or_exited {
            return false;
        }

        let mut exited = false;
        let mut inside = false;
        let mut any_considered = false;

        for &object_id in &self.members {
            let Some((did_exit, is_inside)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }

                    let did_exit = Self::object_did_exit(object_id, trigger);
                    let is_inside = !did_exit && Self::object_in_trigger(object_guard, trigger);
                    Some((did_exit, is_inside))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if did_exit {
                exited = true;
            } else if is_inside {
                inside = true;
            }
        }

        any_considered && exited && !inside
    }

    pub fn all_inside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.has_any_objects() {
            return false;
        }

        let mut any_considered = false;
        let mut any_outside = false;

        for &object_id in &self.members {
            let Some(is_outside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(!Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_outside {
                any_outside = true;
                break;
            }
        }

        any_considered && !any_outside
    }

    pub fn none_inside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        let mut any_considered = false;
        let mut any_inside = false;

        for &object_id in &self.members {
            let Some(is_inside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_inside {
                any_inside = true;
            }
        }

        any_considered && !any_inside
    }

    pub fn some_inside_some_outside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        // Wave 256: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        let mut any_considered = false;
        let mut any_inside = false;
        let mut any_outside = false;

        for &object_id in &self.members {
            let Some(is_inside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_inside {
                any_inside = true;
            } else {
                any_outside = true;
            }
        }

        any_considered && any_inside && any_outside
    }
}
