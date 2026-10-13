//! AIStates.cpp:1323-1446 and AI.cpp:572-826, using the receiving match's owners.
use super::*;
use crate::ai::{search_qualifiers as q, target_acquisition, vision_factors};
use crate::weapon::WeaponSlotType;

impl BorrowedAiWorld<'_> {
    fn mood_priority(
        &self,
        source: &Object,
    ) -> Result<Option<crate::ai::AttackPriorityInfo>, String> {
        let scripts = &self
            .services
            .disposal
            .as_deref()
            .ok_or("mood search requires its script owner")?
            .scripts;
        let mut name = scripts
            .get_object_attack_priority_set(source.get_id())
            .unwrap_or_default();
        if name.is_empty() {
            if let Some(team) = source.admitted_team()? {
                let team = team.try_read().map_err(|_| "mood source team borrowed")?;
                if let Some(prototype) = self
                    .services
                    .teams
                    .find_team_prototype(team.get_name().as_str())
                {
                    name = prototype.get_attack_priority_name().as_str().to_owned();
                }
            }
        }
        Ok((!name.is_empty())
            .then(|| scripts.get_attack_info(&name))
            .flatten())
    }

    fn mood_candidate_distance(
        &self,
        source: &Object,
        id: ObjectID,
        range: Real,
        qualifiers: u32,
    ) -> Result<Option<Real>, String> {
        if id == source.get_id() {
            return Ok(None);
        }
        let Some(target) = self.object(id) else {
            return Ok(None);
        };
        let target = target.try_read().map_err(|_| "mood target borrowed")?;
        target.admitted_team()?;
        // CPP PartitionManager computes distance before the ordered filters.
        let distance = crate::helpers::ThePartitionManager::get_distance_squared(
            source,
            &target,
            crate::common::FROM_BOUNDING_SPHERE_2D,
        );
        if distance >= range * range
            || target.is_effectively_dead()
            || source.is_off_map() != target.is_off_map()
            || source.relationship_to_with_players(&target, self.players) != Relationship::Enemies
        {
            return Ok(None);
        }
        if qualifiers & q::ATTACK_BUILDINGS == 0
            && target.is_kind_of(KindOf::Structure)
            && !target.is_able_to_attack_with_world(self)?
        {
            return Ok(None);
        }
        if qualifiers & q::WITHIN_ATTACK_RANGE != 0 {
            let mut in_range = false;
            for slot in [
                WeaponSlotType::Primary,
                WeaponSlotType::Secondary,
                WeaponSlotType::Tertiary,
            ] {
                if let Some(weapon) = source.get_weapon_in_weapon_slot(slot) {
                    if self.weapon_in_attack_range(source, weapon, Some(&target), None)? {
                        in_range = true;
                        break;
                    }
                }
            }
            if !in_range {
                return Ok(None);
            }
        }
        if qualifiers & q::CAN_SEE != 0 {
            let eye = |object: &Object| {
                let position = object.get_position();
                glam::Vec3::new(
                    position.x,
                    position.y,
                    position.z + object.get_geometry_info().get_max_height_above_position(),
                )
            };
            if !self
                .services
                .terrain
                .is_clear_line_of_sight(&eye(source), &eye(&target))
            {
                return Ok(None);
            }
        }
        if qualifiers & q::CAN_ATTACK != 0
            && !matches!(
                source.get_able_to_attack_specific_object_with_world(
                    crate::attack::AbleToAttackType::NewTarget,
                    &target,
                    crate::ai::CommandSourceType::FromAi,
                    self,
                )?,
                crate::attack::CanAttackResult::Possible
                    | crate::attack::CanAttackResult::PossibleAfterMoving
            )
        {
            return Ok(None);
        }
        if qualifiers & q::UNFOGGED != 0 {
            let index = source
                .controlling_player_id_from_pin()?
                .map_or(-1, |id| id as Int);
            if target.get_shrouded_status_with_native_partition(
                index,
                self.services.collision.partition_manager(),
                self.players,
            ) != crate::common::ObjectShroudStatus::Clear
            {
                return Ok(None);
            }
        }
        if qualifiers & q::IGNORE_INSIGNIFICANT_BUILDINGS != 0
            && target.is_kind_of(KindOf::Structure)
            && !target.is_kind_of(KindOf::CountsForVictory)
        {
            return Ok(None);
        }
        // CPP also exempts DISGUISED from rejection by STEALTHED.
        if target.is_stealthed()
            && !target.is_detected()
            && !target.test_status(ObjectStatusTypes::Disguised)
        {
            return Ok(None);
        }
        Ok(Some(distance))
    }

    fn mood_candidate_priority(
        &self,
        source: &Object,
        id: ObjectID,
        info: &crate::ai::AttackPriorityInfo,
    ) -> Result<Option<(i32, Real)>, String> {
        let Some(target) = self.object(id) else {
            return Ok(None);
        };
        let (mut priority, members, distance) = {
            let target = target.try_read().map_err(|_| "priority target borrowed")?;
            let priority = info.get_priority(target.get_template().get_name().as_str());
            if priority == 0 {
                return Ok(None);
            }
            let members = if let Some(contain) = target.get_contain() {
                contain
                    .try_lock()
                    .map_err(|_| "priority contain borrowed")?
                    .get_contained_objects()
                    .into_owned()
            } else {
                Vec::new()
            };
            (
                priority,
                members,
                crate::helpers::ThePartitionManager::get_distance_squared(
                    source,
                    &target,
                    crate::common::FROM_BOUNDING_SPHERE_2D,
                ),
            )
        };
        for id in members {
            let Some(member) = self.object(id) else {
                continue;
            };
            let member = member.try_read().map_err(|_| "priority rider borrowed")?;
            priority = priority.max(info.get_priority(member.get_template().get_name().as_str()));
        }
        Ok(Some((priority, distance)))
    }
}

impl BorrowedAiWorld<'_> {
    pub(super) fn idle_goal_position_owned(
        &self,
        source: &Object,
        goal_cell: ICoord2D,
    ) -> Result<Option<Coord3D>, String> {
        let observations = crate::ai::pathfind_complete::PathfindObservations {
            factory: self.factory,
            players: self.players,
            terrain: self.services.terrain,
        };
        let mut position = Coord3D::default();
        Ok(self
            .services
            .pathfinder
            .goal_position_for_cell_with_observations(
                goal_cell,
                source.get_geometry_info().get_bounding_circle_radius(),
                &mut position,
                &observations,
            )
            .then_some(position))
    }

    pub(super) fn closest_repulsor_owned(
        &self,
        source: &Object,
        range: Real,
    ) -> Result<Option<ObjectID>, String> {
        if !self.services.rules.enable_repulsors {
            return Ok(None);
        }
        let candidates = self
            .services
            .collision
            .partition_manager()
            .ordered_search_candidates(source.get_position(), range);
        let mut closest = None;
        let mut closest_distance = range * range;
        for id in candidates {
            if id == source.get_id() {
                continue;
            }
            let Some(target) = self.object(id) else {
                continue;
            };
            let target = target.try_read().map_err(|_| "repulsor target borrowed")?;
            let distance = crate::helpers::ThePartitionManager::get_distance_squared(
                source,
                &target,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            );
            if distance >= closest_distance
                || !target.test_status(ObjectStatusTypes::Repulsor)
                || (target.is_stealthed()
                    && !target.is_detected()
                    && !target.test_status(ObjectStatusTypes::Disguised))
            {
                continue;
            }
            closest_distance = distance;
            closest = Some(id);
        }
        Ok(closest)
    }

    pub(super) fn mood_vision_range_owned(
        &self,
        source: &Object,
        attitude: crate::modules::AIAttitudeType,
    ) -> Result<Real, String> {
        source.admitted_team()?;
        let mut weapon_range: Real = 0.0;
        if source.get_contained_by().is_some() {
            for slot in [
                WeaponSlotType::Primary,
                WeaponSlotType::Secondary,
                WeaponSlotType::Tertiary,
            ] {
                if let Some(weapon) = source.get_weapon_in_weapon_slot(slot) {
                    let bonus = self.weapon_bonus(source, weapon)?;
                    weapon_range = weapon_range.max(weapon.get_template().get_attack_range(&bonus));
                }
            }
        }
        Ok(target_acquisition::adjusted_vision_range(
            self.services.rules,
            source,
            vision_factors::OWNER_TYPE | vision_factors::MOOD,
            Some(attitude),
            self.controller_is_human(source),
            weapon_range,
        ))
    }
    pub(super) fn mood_search_rules_owned(&self) -> Result<(bool, bool), String> {
        Ok((
            self.services.rules.attack_uses_line_of_sight,
            self.services.rules.attack_ignore_insignificant_buildings,
        ))
    }
    pub(super) fn closest_mood_enemy_owned(
        &self,
        source: &Object,
        range: Real,
        qualifiers: u32,
    ) -> Result<Option<ObjectID>, String> {
        source.admitted_team()?;
        if qualifiers & q::CAN_ATTACK != 0 && !source.is_able_to_attack_with_world(self)? {
            return Ok(None);
        }
        let priority = self.mood_priority(source)?;
        let candidates = self
            .services
            .collision
            .partition_manager()
            .ordered_search_candidates(source.get_position(), range);
        target_acquisition::select_enemy_candidates(
            candidates,
            range,
            priority
                .as_ref()
                .is_some_and(|info| !info.get_name().is_empty()),
            self.services.rules.attack_priority_distance_modifier,
            |id| self.mood_candidate_distance(source, id, range, qualifiers),
            |id| match priority.as_ref() {
                Some(info) => self.mood_candidate_priority(source, id, info),
                None => Ok(None),
            },
        )
    }
    pub(super) fn mood_random_offset_owned(
        &mut self,
        minimum: i32,
        maximum: i32,
    ) -> Result<i32, String> {
        if maximum < minimum {
            return Err("invalid mood offset range".into());
        }
        let width = i64::from(maximum) - i64::from(minimum) + 1;
        let draw = u64::from(self.services.random.next_u32()) % (width as u64);
        Ok((i64::from(minimum) + draw as i64) as i32)
    }
}
