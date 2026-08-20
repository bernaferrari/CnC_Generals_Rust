// Member count, recruit, iterate, and occupancy queries
//
// Split from `team.rs` for module-size parity.
// Observable behavior is unchanged.

impl Team {
    /// Count buildings in team
    pub fn count_buildings(&self) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count = 0;
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_kind_of(KindOf::Structure) {
                    count += 1;
                }
            });
        }
        count
    }

    /// Count team members matching each template entry.
    /// Matches C++ Team::countObjectsByThingTemplate.
    pub fn count_objects_by_thing_template(
        &self,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
    ) {
        if OBJECT_REGISTRY.is_empty() {
            counts.fill(0);
            return;
        }
        counts.fill(0);
        let max_templates = templates.len().min(counts.len());
        if max_templates == 0 {
            return;
        }

        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if ignore_dead && object_guard.is_effectively_dead() {
                    return;
                }
                if ignore_under_construction
                    && object_guard.test_status(ObjectStatusTypes::UnderConstruction)
                {
                    return;
                }

                let obj_template = object_guard.get_template();
                for i in 0..max_templates {
                    if !obj_template.is_equivalent_to(templates[i].as_ref()) {
                        continue;
                    }
                    counts[i] += 1;
                    break;
                }
            });
        }
    }

    /// Heal all team members completely.
    /// Matches C++ Team::healAllObjects.
    pub fn heal_all_objects(&mut self) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                let _ = object_guard.heal_completely();
            });
        }
    }

    /// Iterate all live team member objects and invoke callback for each.
    /// Matches C++ Team::iterateObjects.
    /// Host/presentation path: OBJECT_REGISTRY empty → no dual-world members to visit.
    /// Iterate live member IDs only (no Arc retention).
    fn for_each_live_member_id<F>(&self, mut func: F)
    where
        F: FnMut(ObjectID),
    {
        if self.members.is_empty() {
            return;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY.get_object(object_id).is_some() {
                func(object_id);
            }
        }
    }

    /// Borrow-first live member access for the duration of `func`.
    fn for_each_live_member_with<F>(&self, mut func: F)
    where
        F: FnMut(ObjectID, &crate::object::Object),
    {
        if self.members.is_empty() {
            return;
        }
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object| {
                func(object_id, object);
            });
        }
    }

    fn for_each_live_member<F>(&self, mut func: F)
    where
        F: FnMut(Arc<RwLock<crate::object::Object>>),
    {
        // Legacy Arc callback path for callers that still need handles.
        self.for_each_live_member_id(|object_id| {
            if let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) {
                func(object_arc);
            }
        });
    }

    pub fn iterate_objects<F>(&self, mut func: F)
    where
        F: FnMut(Arc<RwLock<crate::object::Object>>),
    {
        self.for_each_live_member(func);
    }

    /// Add this team's members to an AIGroup.
    /// Matches C++ Team::getTeamAsAIGroup.
    pub fn get_team_as_ai_group(&self, ai_group: &mut AIGroup) {
        // ID-first: AIGroup stores ObjectIDs; resolve only inside add_by_id.
        for &object_id in &self.members {
            let _ = ai_group.add_by_id(object_id);
        }
    }

    /// Try to recruit a matching unit from other teams of this controller.
    /// Matches C++ Team::tryToRecruit.
    pub fn try_to_recruit(
        &self,
        template: &Arc<dyn ThingTemplate>,
        team_home: &Coord3D,
        max_dist: Real,
    ) -> Option<Arc<RwLock<crate::object::Object>>> {
        // Wave 256: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let controller_id = self.controlling_player_id?;
        let default_team_id = player_list()
            .read()
            .ok()
            .and_then(|players| players.get_player(controller_id as Int).cloned())
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .and_then(|player| player.get_default_team())
            })
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_id()));

        let my_priority = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(self.name.as_str())
                    .map(|prototype| prototype.get_production_priority())
            })
            .unwrap_or(Int::MAX);

        let mut dist_sqr = max_dist * max_dist;
        let mut recruit_id: Option<ObjectID> = None;

        for object_id in OBJECT_REGISTRY.get_all_object_ids() {
            let Some(decision) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    let obj_template = object_guard.get_template().clone();
                    let candidate_name = object_guard.get_template().get_name().to_string();
                    let template_matches = obj_template.is_equivalent_to(template.as_ref())
                        || TheThingFactory::has_build_variation_name(template, &candidate_name);
                    if !template_matches {
                        return None;
                    }
                    if object_guard.get_controlling_player_id() != Some(controller_id) {
                        return None;
                    }
                    if object_guard.is_effectively_dead()
                        || object_guard.is_disabled_by_type(DisabledType::Held)
                    {
                        return None;
                    }
                    let source_team_arc = object_guard.get_team()?;
                    let Ok(source_team_guard) = source_team_arc.read() else {
                        return None;
                    };
                    if !source_team_guard.is_active() {
                        return None;
                    }

                    let source_priority = get_team_factory()
                        .lock()
                        .ok()
                        .and_then(|factory| {
                            factory
                                .find_team_prototype(source_team_guard.get_name().as_str())
                                .map(|prototype| prototype.get_production_priority())
                        })
                        .unwrap_or(Int::MAX);
                    if source_priority >= my_priority {
                        return None;
                    }

                    let is_default_team = default_team_id == Some(source_team_guard.get_id());
                    let mut team_is_recruitable = is_default_team;
                    if source_team_guard.is_recruitable() {
                        team_is_recruitable = true;
                    }
                    if source_team_guard.is_recruitability_set() {
                        team_is_recruitable = source_team_guard.is_recruitable();
                    }
                    if !team_is_recruitable {
                        return None;
                    }

                    // C++ Team.cpp:2350-2352 — per-unit AI isRecruitable override.
                    if let Some(ai) = object_guard.get_ai_update_interface() {
                        let recruitable = ai
                            .lock()
                            .ok()
                            .map(|ai_guard| ai_guard.is_recruitable())
                            .unwrap_or(true);
                        if !recruitable {
                            return None;
                        }
                    }

                    let pos = *object_guard.get_position();
                    let dx = team_home.x - pos.x;
                    let dy = team_home.y - pos.y;
                    let this_dist_sqr = dx * dx + dy * dy;
                    Some((is_default_team, this_dist_sqr))
                })
                .flatten()
            else {
                continue;
            };

            let (is_default_team, this_dist_sqr) = decision;

            if is_default_team && recruit_id.is_none() {
                recruit_id = Some(object_id);
                dist_sqr = this_dist_sqr;
            }

            if this_dist_sqr > dist_sqr {
                continue;
            }

            dist_sqr = this_dist_sqr;
            recruit_id = Some(object_id);
        }

        recruit_id.and_then(|id| OBJECT_REGISTRY.get_object(id))
    }

    /// Count objects with specific kind flags
    pub fn count_objects(&self, set_mask: u32, clear_mask: u32) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let required = set_mask as KindOfMaskType;
        let forbidden = clear_mask as KindOfMaskType;
        let mut count = 0;

        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_kind_of_multi(required, forbidden) {
                    count += 1;
                }
            });
        }

        count
    }

    /// Check if team has any buildings
    pub fn has_any_buildings(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    object_guard.is_kind_of(KindOf::Structure)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has buildings of specific kind
    pub fn has_any_buildings_of_kind(&self, kind_of: u32) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        let mask = (kind_of as KindOfMaskType) | (KindOf::Structure.cpp_mask());
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    object_guard.is_kind_of_multi(mask, crate::common::KIND_OF_MASK_NONE)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any units
    pub fn has_any_units(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    if object_guard.is_kind_of(KindOf::Structure)
                        || object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any objects
    pub fn has_any_objects(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead()
                        || object_guard.is_destroyed()
                        || object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Inert)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if all team units are idle
    pub fn is_idle(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            let Some(idle) = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_effectively_dead() {
                    return true; // skip dead
                }
                let Some(ai_arc) = object_guard.get_ai_update_interface() else {
                    return true; // skip non-AI
                };
                let Ok(ai_guard) = ai_arc.lock() else {
                    return false; // lock fail => not idle
                };
                ai_guard.is_idle()
            }) else {
                continue;
            };
            if !idle {
                return false;
            }
        }
        true
    }

    /// Returns true when any live team member is inside the trigger area.
    /// Matches C++ Team::unitsEntered.
    pub fn units_entered(&self, trigger: &PolygonTrigger) -> Bool {
        // Wave 256: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() {
                        return false;
                    }
                    Self::object_in_trigger(object_guard, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any build facilities
    pub fn has_any_build_facility(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    object_guard.get_template().is_build_facility()
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }
}
