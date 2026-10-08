//! Shared C++ AI.cpp target selection, attack priorities and vision rules.
//! Native callers borrow the source; ID-based entrypoints are compatibility adapters.
use super::*;
use crate::weapon::WeaponSlotType;

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
        source: &Object,
        range: Real,
        qualifiers: u32,
        info: Option<&AttackPriorityInfo>,
        optional_filter: Option<&dyn PartitionFilter>,
    ) -> Result<Option<ObjectId>, AiError> {
        if qualifiers & search_qualifiers::CAN_ATTACK != 0 && !source.is_able_to_attack() {
            return Ok(None);
        }
        let Some(partition) = ThePartitionManager::get() else {
            return Ok(None);
        };
        let source_id = source.get_id();
        let source_pos = *source.get_position();
        let candidates = partition.get_objects_in_range_with_borrowed_positions(
            &source_pos,
            range,
            &[(source_id, source_pos)],
        );
        // C++ AI.cpp:642-646: null/default uses closest selection, not priorities.
        let priority_info = info.filter(|info| !info.get_name().is_empty());
        let mut closest_id = None;
        let mut closest_distance = range * range + 1.0;
        let mut priority_candidates = Vec::new();

        for target_id in candidates {
            if target_id == source_id {
                continue;
            }
            let Some(distance) = OBJECT_REGISTRY
                .with_object(target_id, |target| {
                    // PartitionManager.cpp:3358-3370 computes distance before filters.
                    let distance = ThePartitionManager::get_distance_squared(
                        source,
                        target,
                        FROM_BOUNDING_SPHERE_2D,
                    );
                    target_passes_builtin_filters(source, target, qualifiers).then_some(distance)
                })
                .flatten()
            else {
                continue;
            };
            // No target guard spans the user callback; it may change containment.
            if optional_filter.is_some_and(|filter| !filter.allow(target_id)) {
                continue;
            }
            if priority_info.is_some() {
                priority_candidates.push((target_id, distance));
            } else if distance < closest_distance {
                closest_distance = distance;
                closest_id = Some(target_id);
            }
        }

        let Some(priority_info) = priority_info else {
            return Ok(closest_id);
        };
        // C++ iterateObjectsInRange completes every filter, then sorts the iterator.
        // Only afterwards does AI.cpp:660-700 inspect live priority/contained data.
        priority_candidates
            .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut best_enemy = None;
        let mut effective_priority = 0;
        let mut actual_priority = 0;
        let distance_modifier = self.ai_data.attack_priority_distance_modifier;
        for (target_id, _) in priority_candidates {
            let Some((mut priority, contained_ids, distance)) = OBJECT_REGISTRY
                .with_object(target_id, |target| {
                    let priority =
                        priority_info.get_priority(target.get_template().get_name().as_str());
                    if priority == 0 {
                        return None;
                    }
                    let contained_ids = target
                        .get_contain()
                        .and_then(|contain| {
                            contain
                                .lock()
                                .ok()
                                .map(|contain| contain.get_contained_objects().into_owned())
                        })
                        .unwrap_or_default();
                    // AI.cpp:685 recalculates distance after the filtered iterator.
                    let distance = ThePartitionManager::get_distance_squared(
                        source,
                        target,
                        FROM_BOUNDING_SPHERE_2D,
                    );
                    Some((priority, contained_ids, distance))
                })
                .flatten()
            else {
                continue;
            };
            for contained_id in contained_ids {
                if let Some(contained_priority) = OBJECT_REGISTRY
                    .with_object(contained_id, |member| {
                        priority_info.get_priority(member.get_template().get_name().as_str())
                    })
                {
                    priority = priority.max(contained_priority);
                }
            }
            let penalty = if distance_modifier > 0.0 {
                (distance.sqrt() / distance_modifier) as i32
            } else {
                0
            };
            let modified_priority = (priority - penalty).max(1);
            if modified_priority > effective_priority
                || (modified_priority == effective_priority && priority > actual_priority)
            {
                effective_priority = modified_priority;
                actual_priority = priority;
                best_enemy = Some(target_id);
            }
        }
        Ok(best_enemy)
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

/// Keep short-circuit calls in C++ AI.cpp:613-639 order. Detailed building,
/// garrison/disguise and obstacle predicates remain tracked in hq-wlrfy.
fn target_passes_builtin_filters(source: &Object, target: &Object, qualifiers: u32) -> bool {
    if target.is_effectively_dead()
        || source.is_off_map() != target.is_off_map()
        || source.relationship_to(target) != Relationship::Enemies
    {
        return false;
    }
    if qualifiers & search_qualifiers::ATTACK_BUILDINGS == 0
        && target.is_kind_of(KindOf::Structure)
        && !target.is_able_to_attack()
    {
        return false;
    }
    // AI.cpp:512-535 tests each real weapon's range, independently of eligibility.
    if qualifiers & search_qualifiers::WITHIN_ATTACK_RANGE != 0
        && ![
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ]
        .into_iter()
        .any(|slot| {
            source
                .get_weapon_in_weapon_slot(slot)
                .is_some_and(|weapon| {
                    weapon.is_within_attack_range_for_objects(source, Some(target), None)
                })
        })
    {
        return false;
    }
    if qualifiers & search_qualifiers::CAN_SEE != 0 {
        let source_eye = elevated_eye(source, source.get_position());
        let target_eye = elevated_eye(target, target.get_position());
        if !crate::object::collide::partition_manager::PartitionManager::is_clear_line_of_sight_terrain(
            None, &to_collide_coord(&source_eye), None, &to_collide_coord(&target_eye),
        ) {
            return false;
        }
    }
    if qualifiers & search_qualifiers::CAN_ATTACK != 0
        && !matches!(
            source.get_able_to_attack_specific_object_for_objects(
                AbleToAttackType::NewTarget,
                target,
                CommandSourceType::FromAi,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        )
    {
        return false;
    }
    if qualifiers & search_qualifiers::UNFOGGED != 0 {
        let player_index = source
            .with_controlling_player(|player| player.get_player_index())
            .unwrap_or(-1);
        if target.get_shrouded_status(player_index) != crate::common::ObjectShroudStatus::Clear {
            return false;
        }
    }
    if qualifiers & search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS != 0
        && target.is_kind_of(KindOf::Structure)
        && !target.is_kind_of(KindOf::CountsForVictory)
    {
        return false;
    }
    !(target.is_stealthed() && !target.is_detected())
}
