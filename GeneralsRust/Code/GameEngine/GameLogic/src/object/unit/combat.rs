//! Unit combat targeting, engagement, and auto-acquire.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;
use super::registry::dual_world_registry_unavailable;
use super::types::*;

impl Unit {
    /// Update combat behavior
    pub(super) fn update_combat(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.auto_acquire_enemies && self.attack_target.is_none() {
            return Ok(());
        }

        match self.combat_mode {
            CombatMode::Aggressive => {
                if self.attack_target.is_none() {
                    self.acquire_target()?;
                }
            }

            CombatMode::Defensive => {
                // Only attack if we're being attacked
                if self.is_under_attack() && self.attack_target.is_none() {
                    self.acquire_target()?;
                }
            }

            CombatMode::HoldPosition => {
                // Attack but don't move to engage
                if self.attack_target.is_none() {
                    self.acquire_target_in_range()?;
                }
            }

            CombatMode::HoldFire => {
                // Don't attack at all
                self.attack_target = None;
            }

            CombatMode::GuardArea => {
                // Only attack enemies in our guard area
                if self.attack_target.is_none() {
                    self.acquire_target_in_guard_area()?;
                    if self.attack_target.is_none() {
                        if let Some(guard_pos) = self.guard_position {
                            let current_pos = self.get_position();
                            let dx = guard_pos.x - current_pos.x;
                            let dy = guard_pos.y - current_pos.y;
                            let distance = (dx * dx + dy * dy).sqrt();
                            if distance > 1.0
                                && !self.is_movement_active()
                                && self.target_position.is_none()
                            {
                                self.move_to_position(guard_pos, false)?;
                            }
                        }
                    }
                }
            }
        }

        // Process attack if we have a target
        if let Some(target_id) = self.attack_target {
            self.engage_target(target_id, delta_time)?;
        } else if self.attack_move_active && self.is_movement_active() {
            self.acquire_target()?;
        }

        if self.attack_move_active && self.movement_state == MovementState::Attacking {
            const ATTACK_MOVE_SHOT_GRACE: u32 = 15;
            let current_frame = TheGameLogic::get_frame() as u32;
            let last_shot = self
                .base_arc()
                .read()
                .map(|guard| guard.get_last_shot_fired_frame())
                .unwrap_or(0);
            if current_frame >= self.attack_move_resume_frame
                && current_frame.saturating_sub(last_shot) > ATTACK_MOVE_SHOT_GRACE
            {
                self.movement_state = match self.current_order {
                    Some(UnitOrder::Patrol { .. }) => MovementState::Patrolling,
                    Some(UnitOrder::Follow { .. }) => MovementState::Following,
                    Some(UnitOrder::Guard { .. }) => MovementState::Guarding,
                    _ => MovementState::Moving,
                };
            }
        }

        if self.attack_move_active && self.movement_state == MovementState::Idle {
            let destination = match &self.current_order {
                Some(UnitOrder::AttackMove { destination, .. }) => *destination,
                Some(UnitOrder::Patrol { .. }) => {
                    return Ok(());
                }
                _ => {
                    self.attack_move_active = false;
                    return Ok(());
                }
            };

            let current_pos = self.get_position();
            let dx = destination.x - current_pos.x;
            let dy = destination.y - current_pos.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 1.0 {
                self.move_to_position(destination, false)?;
            } else {
                self.attack_move_active = false;
                self.movement_state = MovementState::Idle;
            }
        }

        if !self.attack_move_active
            && self.attack_target.is_none()
            && self.movement_state == MovementState::Attacking
        {
            let mut resume_state = match self.current_order {
                Some(UnitOrder::Follow { .. }) => MovementState::Following,
                Some(UnitOrder::Patrol { .. }) => MovementState::Patrolling,
                Some(UnitOrder::Retreat { .. }) => MovementState::Retreating,
                Some(UnitOrder::Guard { .. }) => MovementState::Idle,
                Some(UnitOrder::Move { .. }) => MovementState::Moving,
                _ => MovementState::Idle,
            };
            if matches!(
                resume_state,
                MovementState::Moving
                    | MovementState::Following
                    | MovementState::Patrolling
                    | MovementState::Retreating
            ) && self.target_position.is_none()
            {
                resume_state = MovementState::Idle;
            }
            self.movement_state = resume_state;
        }

        if matches!(self.current_order, Some(UnitOrder::Attack { .. }))
            && self.attack_target.is_none()
        {
            self.current_order = None;
            self.advance_order_queue();
        }

        Ok(())
    }
    pub(super) fn look_for_enemies(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.auto_acquire_enemies {
            return Ok(());
        }

        if self.auto_acquire_not_while_attacking && self.is_currently_attacking() {
            return Ok(());
        }

        if !self.auto_acquire_while_stealthed
            && self
                .base_arc()
                .read()
                .map(|guard| guard.is_stealthed())
                .unwrap_or(false)
        {
            return Ok(());
        }

        match self.combat_mode {
            CombatMode::GuardArea => self.acquire_target_in_guard_area()?,
            CombatMode::HoldPosition | CombatMode::HoldFire => self.acquire_target_in_range()?,
            CombatMode::Aggressive | CombatMode::Defensive => self.acquire_target()?,
        }

        Ok(())
    }
    pub(super) fn acquire_target(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        if !self.should_scan_for_targets(self.engagement_range) {
            return Ok(());
        }

        if let Some((target_id, _)) = self.find_closest_enemy_with_buildings(
            self.get_position(),
            self.engagement_range,
            self.engagement_range,
        ) {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    pub(super) fn acquire_target_in_range(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        if !self.should_scan_for_targets(self.engagement_range) {
            return Ok(());
        }

        if let Some((target_id, _)) = self.find_closest_enemy_with_buildings(
            self.get_position(),
            self.engagement_range,
            self.engagement_range,
        ) {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    pub(super) fn acquire_target_in_guard_area(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        let guard_pos = match self.guard_position {
            Some(pos) => pos,
            None => return Ok(()),
        };

        if !self.should_scan_for_targets(self.guard_radius) {
            return Ok(());
        }

        let guard_radius = if self.guard_radius > 0.0 {
            self.guard_radius
        } else {
            self.engagement_range
        };

        if let Some((target_id, _)) =
            self.find_closest_enemy_with_buildings(guard_pos, guard_radius, self.engagement_range)
        {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    pub(super) fn engage_target(
        &mut self,
        target_id: ObjectID,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 258: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let (target_pos, target_relationship, detected) = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |g| {
                let pos = Some(*g.get_position());
                let rel = self
                    .base_arc()
                    .read()
                    .ok()
                    .map(|me| me.relationship_to(g))
                    .unwrap_or(Relationship::Neutral);
                let detected = g.is_detected();
                (pos, rel, detected)
            })
            .unwrap_or((None, Relationship::Neutral, false));

        let target_pos = match target_pos {
            Some(pos) => pos,
            None => {
                self.attack_target = None;
                return Ok(());
            }
        };

        if !matches!(target_relationship, Relationship::Enemies) {
            self.attack_target = None;
            return Ok(());
        }

        let current_pos = self.get_position();
        let dx = target_pos.x - current_pos.x;
        let dy = target_pos.y - current_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if !detected && !self.can_detect_target_distance(distance) {
            self.attack_target = None;
            return Ok(());
        }

        if distance > self.engagement_range {
            if self.attack_move_active {
                self.attack_target = None;
                return Ok(());
            }

            if self.can_move() && !self.is_movement_active() {
                self.move_to_position(target_pos, false)?;
            }
        } else {
            if matches!(self.combat_mode, CombatMode::GuardArea) {
                if let Some(guard_pos) = self.guard_position {
                    let guard_radius = if self.guard_radius > 0.0 {
                        self.guard_radius
                    } else {
                        self.engagement_range
                    };
                    let dx_guard = target_pos.x - guard_pos.x;
                    let dy_guard = target_pos.y - guard_pos.y;
                    let dist_guard = (dx_guard * dx_guard + dy_guard * dy_guard).sqrt();
                    if dist_guard > guard_radius {
                        self.attack_target = None;
                        return Ok(());
                    }
                }
            }
            self.movement_state = MovementState::Attacking;
            if self.attack_move_active {
                const ATTACK_MOVE_PAUSE_FRAMES: u32 = 30;
                let current_frame = TheGameLogic::get_frame() as u32;
                self.attack_move_resume_frame =
                    current_frame.saturating_add(ATTACK_MOVE_PAUSE_FRAMES);
            }
            const TARGET_LOCK_FRAMES: u32 = 30;
            let current_frame = TheGameLogic::get_frame() as u32;
            self.attack_target_lock_until = current_frame.saturating_add(TARGET_LOCK_FRAMES);
        }

        Ok(())
    }
    pub(super) fn should_scan_for_targets(&mut self, max_distance: Real) -> bool {
        // Wave 258: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        const TARGET_LOCK_GRACE: u32 = 30;

        let current_frame = TheGameLogic::get_frame() as u32;
        if let Ok(guard) = self.base_arc().read() {
            let last_shot = guard.get_last_shot_fired_frame();
            if current_frame.saturating_sub(last_shot) < TARGET_LOCK_GRACE {
                return false;
            }
        }
        if current_frame < self.attack_target_lock_until {
            return false;
        }

        if let Some(target_id) = self.attack_target {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |target_guard| {
                    let is_enemy = self
                        .base_arc()
                        .read()
                        .ok()
                        .map(|guard| guard.relationship_to(target_guard))
                        == Some(Relationship::Enemies);
                    if !is_enemy {
                        return false;
                    }
                    let target_pos = *target_guard.get_position();
                    let self_pos = self.get_position();
                    let dx = target_pos.x - self_pos.x;
                    let dy = target_pos.y - self_pos.y;
                    let distance = (dx * dx + dy * dy).sqrt();
                    distance <= max_distance && self.can_detect_target(target_guard, distance)
                })
                .unwrap_or(false)
            {
                return false;
            }
        }
        let interval = self.mood_attack_check_rate_frames.max(1);
        if current_frame.saturating_sub(self.last_target_scan_frame) < interval {
            return false;
        }

        self.last_target_scan_frame = current_frame;
        true
    }
    pub(super) fn find_closest_enemy(
        &self,
        center: Coord3D,
        max_distance: Real,
        vision_distance: Real,
    ) -> Option<(ObjectID, Real)> {
        // Host path: empty dual-world registry residual.
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return None;
        }
        let all_object_ids = crate::object::registry::OBJECT_REGISTRY.get_all_object_ids();
        let self_id = self
            .base_arc()
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(0);
        let mut closest: Option<(ObjectID, Real)> = None;

        if let Some(current_target) = self.attack_target {
            if let Some(dist_to_self) = crate::object::registry::OBJECT_REGISTRY
                .with_object(current_target, |target_guard| {
                    let is_enemy = self
                        .base_arc()
                        .read()
                        .ok()
                        .map(|guard| guard.relationship_to(target_guard))
                        == Some(Relationship::Enemies);
                    if !is_enemy {
                        return None;
                    }
                    let target_pos = *target_guard.get_position();
                    let dx_center = target_pos.x - center.x;
                    let dy_center = target_pos.y - center.y;
                    let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();
                    let self_pos = self.get_position();
                    let dx_self = target_pos.x - self_pos.x;
                    let dy_self = target_pos.y - self_pos.y;
                    let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();
                    if dist_to_center <= max_distance
                        && dist_to_self <= vision_distance
                        && self.can_detect_target(target_guard, dist_to_self)
                    {
                        Some(dist_to_self)
                    } else {
                        None
                    }
                })
                .flatten()
            {
                closest = Some((current_target, dist_to_self * 1.1));
            }
        }

        for obj_id in &all_object_ids {
            let obj = match crate::object::registry::OBJECT_REGISTRY.get_object(*obj_id) {
                Some(v) => v,
                None => continue,
            };
            let obj_guard = match obj.read() {
                Ok(guard) => guard,
                Err(_) => continue,
            };

            let obj_id = obj_guard.get_id();
            if obj_id == self_id {
                continue;
            }

            if !obj_guard.is_kind_of(KindOf::Unit) {
                continue;
            }

            if !matches!(
                self.base_arc()
                    .read()
                    .ok()
                    .map(|guard| guard.relationship_to(&obj_guard)),
                Some(Relationship::Enemies)
            ) {
                continue;
            }

            let obj_pos = *obj_guard.get_position();
            let dx_center = obj_pos.x - center.x;
            let dy_center = obj_pos.y - center.y;
            let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();

            if dist_to_center > max_distance {
                continue;
            }

            let self_pos = self.get_position();
            let dx_self = obj_pos.x - self_pos.x;
            let dy_self = obj_pos.y - self_pos.y;
            let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();

            if dist_to_self > vision_distance {
                continue;
            }

            if !self.can_detect_target(&obj_guard, dist_to_self) {
                continue;
            }

            let mut weighted_dist = dist_to_self;
            if self.is_under_attack() {
                weighted_dist *= 0.9;
            }
            if dist_to_self <= self.engagement_range {
                weighted_dist *= 0.8;
            }

            match closest {
                Some((current_id, best_dist)) if weighted_dist >= best_dist => {
                    // Keep current target unless new target is meaningfully closer.
                    if current_id == obj_id {
                        closest = Some((obj_id, weighted_dist));
                    }
                }
                _ => closest = Some((obj_id, weighted_dist)),
            }
        }

        closest
    }
    pub(super) fn find_closest_enemy_with_buildings(
        &self,
        center: Coord3D,
        max_distance: Real,
        vision_distance: Real,
    ) -> Option<(ObjectID, Real)> {
        if !self.auto_acquire_attack_buildings {
            return self.find_closest_enemy(center, max_distance, vision_distance);
        }

        // Host path: empty dual-world registry residual.
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return None;
        }
        let all_object_ids = crate::object::registry::OBJECT_REGISTRY.get_all_object_ids();
        let self_id = self
            .base_arc()
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(0);
        let mut closest: Option<(ObjectID, Real)> = None;

        for obj_id in &all_object_ids {
            let obj = match crate::object::registry::OBJECT_REGISTRY.get_object(*obj_id) {
                Some(v) => v,
                None => continue,
            };
            let obj_guard = match obj.read() {
                Ok(guard) => guard,
                Err(_) => continue,
            };

            let obj_id = obj_guard.get_id();
            if obj_id == self_id {
                continue;
            }

            let is_unit = obj_guard.is_kind_of(KindOf::Unit);
            let is_structure = obj_guard.is_kind_of(KindOf::Structure);
            if !is_unit && !is_structure {
                continue;
            }

            if !matches!(
                self.base_arc()
                    .read()
                    .ok()
                    .map(|guard| guard.relationship_to(&obj_guard)),
                Some(Relationship::Enemies)
            ) {
                continue;
            }

            let obj_pos = *obj_guard.get_position();
            let dx_center = obj_pos.x - center.x;
            let dy_center = obj_pos.y - center.y;
            let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();

            if dist_to_center > max_distance {
                continue;
            }

            let self_pos = self.get_position();
            let dx_self = obj_pos.x - self_pos.x;
            let dy_self = obj_pos.y - self_pos.y;
            let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();

            if dist_to_self > vision_distance {
                continue;
            }

            if !self.can_detect_target(&obj_guard, dist_to_self) {
                continue;
            }

            let mut weighted_dist = dist_to_self;
            if self.is_under_attack() {
                weighted_dist *= 0.9;
            }
            if dist_to_self <= self.engagement_range {
                weighted_dist *= 0.8;
            }

            match closest {
                Some((current_id, best_dist)) if weighted_dist >= best_dist => {
                    if current_id == obj_id {
                        closest = Some((obj_id, weighted_dist));
                    }
                }
                _ => closest = Some((obj_id, weighted_dist)),
            }
        }

        closest
    }
    pub(super) fn can_detect_target(&self, target: &Object, distance: Real) -> bool {
        if target.is_detected() {
            return true;
        }

        self.can_detect_target_distance(distance)
    }
    pub(super) fn can_detect_target_distance(&self, distance: Real) -> bool {
        let base_range = self
            .base_arc()
            .read()
            .ok()
            .map(|guard| guard.get_stealth_detection_range() as Real)
            .unwrap_or(0.0);
        let detection_range = self.stealth_detection_range.max(base_range);

        if detection_range <= 0.0 {
            return false;
        }

        distance <= detection_range
    }
    pub(super) fn is_under_attack(&self) -> bool {
        let Some(body) = self
            .base_arc()
            .read()
            .ok()
            .and_then(|guard| guard.get_body_module())
        else {
            return false;
        };

        let Ok(body_guard) = body.lock() else {
            return false;
        };

        let Some(last) = body_guard.get_last_damage_info() else {
            return false;
        };

        if matches!(
            last.input.damage_type,
            DamageType::Healing | DamageType::Penalty
        ) {
            return false;
        }

        let last_frame = body_guard.get_last_damage_timestamp();
        if last_frame == u32::MAX {
            return false;
        }

        let current_frame = TheGameLogic::get_frame() as u32;
        current_frame.saturating_sub(last_frame) <= LOGICFRAMES_PER_SECOND
    }
    pub(super) fn is_currently_attacking(&self) -> bool {
        matches!(
            self.current_order,
            Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
        ) || self.movement_state == MovementState::Attacking
    }
    pub(super) fn can_auto_acquire_now(&self) -> bool {
        if !self.auto_acquire_enemies {
            return false;
        }

        if self.auto_acquire_not_while_attacking && self.is_currently_attacking() {
            return false;
        }

        if !self.auto_acquire_while_stealthed {
            let stealthed = self
                .base_arc()
                .read()
                .map(|guard| guard.is_stealthed())
                .unwrap_or(false);
            if stealthed {
                return false;
            }
        }

        true
    }
}
