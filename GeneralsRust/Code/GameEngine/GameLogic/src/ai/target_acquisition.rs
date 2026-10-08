//! Shared C++ AI.cpp target selection, attack priorities and vision rules.
//! Native callers borrow the source; ID-based entrypoints are compatibility adapters.
use super::*;

pub fn resolve_attack_priority_info_for_object(owner_id: ObjectID) -> Option<AttackPriorityInfo> {
    resolve_attack_priority_info(owner_id, || {
        OBJECT_REGISTRY
            .with_object(owner_id, team_name_for_object)
            .flatten()
    })
}

pub(super) fn resolve_attack_priority_info_for_source(
    owner: &Object,
) -> Option<AttackPriorityInfo> {
    resolve_attack_priority_info(owner.get_id(), || team_name_for_object(owner))
}

fn team_name_for_object(owner: &Object) -> Option<String> {
    let team = owner.get_team()?;
    let team = team.read().ok()?;
    Some(team.get_name().to_string())
}

fn resolve_attack_priority_info(
    owner_id: ObjectID,
    team_name: impl FnOnce() -> Option<String>,
) -> Option<AttackPriorityInfo> {
    // Wave 263: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    let mut priority_set_name = String::new();

    if let Ok(engine_lock) = get_script_engine().read() {
        if let Some(engine) = engine_lock.as_ref() {
            if let Some(name) = engine.get_object_attack_priority_set(owner_id) {
                if !name.is_empty() {
                    priority_set_name = name.to_string();
                }
            }
        }
    }

    if priority_set_name.is_empty() {
        let team_name = team_name();

        if let Some(team_name) = team_name {
            if let Ok(factory) = get_team_factory().lock() {
                if let Some(prototype) = factory.find_team_prototype(&team_name) {
                    let name = prototype.get_attack_priority_name().as_str();
                    if !name.is_empty() {
                        priority_set_name = name.to_string();
                    }
                }
            }
        }
    }

    if priority_set_name.is_empty() {
        return None;
    }

    if let Ok(engine_lock) = get_script_engine().read() {
        if let Some(engine) = engine_lock.as_ref() {
            return engine.get_attack_info(&priority_set_name);
        }
    }

    None
}

impl AI {
    pub(crate) fn attack_priority_info_for_source(
        &self,
        source: &Object,
    ) -> Option<AttackPriorityInfo> {
        resolve_attack_priority_info_for_source(source)
    }
    pub fn find_closest_enemy(
        &self,
        me: ObjectId,
        range: Real,
        qualifiers: u32,
        info: Option<&AttackPriorityInfo>,
        optional_filter: Option<&dyn PartitionFilter>,
    ) -> Result<Option<ObjectId>, AiError> {
        // Wave 263: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        OBJECT_REGISTRY
            .with_object(me, |source| {
                self.find_closest_enemy_for_source(source, range, qualifiers, info, optional_filter)
            })
            .unwrap_or(Err(AiError::InvalidObject))
    }

    /// C++ AI::findClosestEnemy takes the source Object itself. The caller
    /// retains its source borrow; only candidate IDs use the active registry.
    pub(crate) fn find_closest_enemy_for_source(
        &self,
        me_guard: &Object,
        range: Real,
        qualifiers: u32,
        info: Option<&AttackPriorityInfo>,
        optional_filter: Option<&dyn PartitionFilter>,
    ) -> Result<Option<ObjectId>, AiError> {
        if (qualifiers & search_qualifiers::CAN_ATTACK) != 0 && !me_guard.is_able_to_attack() {
            return Ok(None);
        }

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(None);
        };

        let me = me_guard.get_id();
        let me_pos = *me_guard.get_position();
        let candidates =
            partition.get_objects_in_range_with_borrowed_positions(&me_pos, range, &[(me, me_pos)]);
        // C++ AI.cpp:651 — NULL or default AttackPriorityInfo is closest-first.
        let use_priority = info.is_some_and(|i| !i.get_name().is_empty());
        let mut closest_id = None;
        let mut closest_dist_sqr = range * range + 1.0;

        let mut best_enemy = None;
        let mut effective_priority = 0;
        let mut actual_priority = 0;
        let attack_priority_modifier = self.ai_data.attack_priority_distance_modifier;

        for target_id in candidates {
            if target_id == me {
                continue;
            }

            // Collect candidate evaluation without retaining Arc handles.
            let Some(eval) = OBJECT_REGISTRY.with_object(target_id, |target| {
                if target.is_effectively_dead() {
                    return None;
                }
                if me_guard.is_off_map() != target.is_off_map() {
                    return None;
                }
                if me_guard.relationship_to(&target) != Relationship::Enemies {
                    return None;
                }
                if (qualifiers & search_qualifiers::ATTACK_BUILDINGS) == 0
                    && target.is_kind_of(KindOf::Structure)
                    && !target.is_able_to_attack()
                {
                    return None;
                }
                if (qualifiers & search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS) != 0
                    && target.is_kind_of(KindOf::Structure)
                    && !target.is_kind_of(KindOf::CountsForVictory)
                {
                    return None;
                }
                if (qualifiers & search_qualifiers::CAN_SEE) != 0 {
                    let target_pos = *target.get_position();
                    let me_eye = elevated_eye(me_guard, &me_pos);
                    let target_eye = elevated_eye(target, &target_pos);
                    if !crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
                        None,
                        &to_collide_coord(&me_eye),
                        None,
                        &to_collide_coord(&target_eye),
                    ) {
                        return None;
                    }
                }
                if (qualifiers & search_qualifiers::UNFOGGED) != 0 {
                    let player_index = me_guard
                        .with_controlling_player(|guard| guard.get_player_index())
                        .unwrap_or(-1);
                    if target.get_shrouded_status(player_index)
                        != crate::common::ObjectShroudStatus::Clear
                    {
                        return None;
                    }
                }
                if target.is_stealthed() && !target.is_detected() {
                    return None;
                }

                let attack_result = if (qualifiers
                    & (search_qualifiers::CAN_ATTACK
                        | search_qualifiers::WITHIN_ATTACK_RANGE))
                    != 0
                {
                    Some(me_guard.get_able_to_attack_specific_object_for_objects(
                        AbleToAttackType::NewTarget,
                        &target,
                        CommandSourceType::FromAi,
                    ))
                } else {
                    None
                };

                if (qualifiers & search_qualifiers::CAN_ATTACK) != 0 {
                    if matches!(
                        attack_result,
                        Some(CanAttackResult::NotPossible | CanAttackResult::InvalidShot)
                    ) {
                        return None;
                    }
                }
                if (qualifiers & search_qualifiers::WITHIN_ATTACK_RANGE) != 0 {
                    if !matches!(attack_result, Some(CanAttackResult::Possible)) {
                        return None;
                    }
                }

                let dist_sqr = ThePartitionManager::get_distance_squared(
                    &me_guard,
                    &target,
                    FROM_BOUNDING_SPHERE_2D,
                );

                let template_name = target.get_template().get_name().to_string();
                let contained_ids = target
                    .get_contain()
                    .and_then(|contain| {
                        contain
                            .lock()
                            .ok()
                            .map(|cg| cg.get_contained_objects().into_owned())
                    })
                    .unwrap_or_default();

                Some((dist_sqr, template_name, contained_ids))
                }).flatten()
            else {
                continue;
            };

            let (dist_sqr, template_name, contained_ids) = eval;

            if let Some(filter) = optional_filter {
                if !filter.allow(target_id) {
                    continue;
                }
            }

            if !use_priority {
                if dist_sqr < closest_dist_sqr {
                    closest_dist_sqr = dist_sqr;
                    closest_id = Some(target_id);
                }
                continue;
            }

            let priority_info = match info {
                Some(info) => info,
                None => continue,
            };
            let mut current_priority = priority_info.get_priority(template_name.as_str());
            if current_priority == 0 {
                continue;
            }

            // C++ AI.cpp lines 669-679: garrisoned contents can raise priority.
            for contained_id in contained_ids {
                if let Some(contained_priority) =
                    OBJECT_REGISTRY.with_object(contained_id, |contained_obj| {
                        let contained_template_name =
                            contained_obj.get_template().get_name().as_str();
                        priority_info.get_priority(contained_template_name)
                    })
                {
                    if contained_priority > current_priority {
                        current_priority = contained_priority;
                    }
                }
            }

            let dist = dist_sqr.sqrt();
            let modifier = if attack_priority_modifier > 0.0 {
                (dist / attack_priority_modifier) as i32
            } else {
                0
            };
            let mut modified_priority = current_priority - modifier;
            if modified_priority < 1 {
                modified_priority = 1;
            }

            if modified_priority > effective_priority
                || (modified_priority == effective_priority && current_priority > actual_priority)
            {
                effective_priority = modified_priority;
                actual_priority = current_priority;
                best_enemy = Some(target_id);
            }
        }

        if use_priority {
            Ok(best_enemy)
        } else {
            Ok(closest_id)
        }
    }

    pub fn get_adjusted_vision_range_for_object(
        &self,
        object: ObjectId,
        factors_to_consider: u32,
    ) -> Result<Real, AiError> {
        // Wave 263: empty dual-world → Ok(0.0).
        if dual_world_registry_unavailable() {
            return Ok(0.0);
        }

        OBJECT_REGISTRY
            .with_object(object, |source| {
                let attitude = source
                    .get_ai_update_interface()
                    .and_then(|ai| ai.lock().ok().map(|ai| ai.get_attitude()));
                self.get_adjusted_vision_range_for_source(source, factors_to_consider, attitude)
            })
            .ok_or(AiError::InvalidObject)
    }
    /// The driving AI supplies its live attitude, so this query never locks
    /// the Object's cached AI handle already held by the scheduler.
    pub(crate) fn get_adjusted_vision_range_for_source(
        &self,
        source: &Object,
        factors_to_consider: u32,
        attitude: Option<AIAttitudeType>,
    ) -> Real {
        // AI.cpp:785-789 returns zero when the object has no AI interface.
        if attitude.is_none() {
            return 0.0;
        }
        let mut range = source.get_vision_range();
        let controller_is_human =
            source.with_controlling_player(|player| player.get_player_type() == PlayerType::Human);
        let player_is_human = controller_is_human == Some(true);
        let contained = source.get_contained_by().is_some();
        let weapon_range = source.get_largest_weapon_range();
        let ai_data = &self.ai_data;

        if (factors_to_consider & vision_factors::OWNER_TYPE) != 0 {
            if player_is_human {
                if (factors_to_consider & vision_factors::GUARD_INNER) != 0 {
                    range *= ai_data.guard_inner_modifier_human;
                } else {
                    range *= ai_data.guard_outer_modifier_human;
                }
            } else if (factors_to_consider & vision_factors::GUARD_INNER) != 0 {
                range *= ai_data.guard_inner_modifier_ai;
            } else {
                range *= ai_data.guard_outer_modifier_ai;
            }
        }

        // C++ AI.cpp:814-826 — contained uses weapon range; Sleep returns 0.
        if contained {
            range = weapon_range;
        } else if (factors_to_consider & vision_factors::MOOD) != 0
            && controller_is_human == Some(false)
        {
            if let Some(attitude) = attitude {
                match attitude {
                    AIAttitudeType::Sleep => return 0.0,
                    AIAttitudeType::Aggressive => range *= ai_data.aggressive_range_modifier,
                    AIAttitudeType::Defensive => range *= ai_data.alert_range_modifier,
                    AIAttitudeType::Passive | AIAttitudeType::Normal => {}
                }
            }
        }

        range
    }
}
