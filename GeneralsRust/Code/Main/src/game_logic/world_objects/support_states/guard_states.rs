//! C++ AIGuard state machine and guard-area scan behavior.
use super::super::super::*;
/// C++ AIGuardInnerState residual.
pub(super) const GUARD_CHASE_PHASE_INNER: u8 = 1;
/// C++ AIGuardOuterState residual.
const GUARD_CHASE_PHASE_OUTER: u8 = 2;
/// C++ AIGuardAttackAggressorState residual.
const GUARD_CHASE_PHASE_AGGRESSOR: u8 = 3;
/// C++ AIGuardReturnState InternalMoveTo close-enough residual.
const GUARD_RETURN_CLOSE: f32 = 25.0;
pub(super) const GUARD_RETURN_CLOSE_SQ: f32 = GUARD_RETURN_CLOSE * GUARD_RETURN_CLOSE;

pub(crate) fn host_guard_xy_dist_sq(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

pub(crate) fn host_same_map_status_off(
    pos: glam::Vec3,
    world_min: glam::Vec3,
    world_max: glam::Vec3,
) -> bool {
    // C++ Object::isOffMap — playable extent, not cargo-plane residual 0..500.
    crate::game_logic::host_deliver_payload::is_off_map_residual(
        pos,
        world_min.x,
        world_min.z,
        world_max.x,
        world_max.z,
    )
}

fn host_area_occupancy(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, std::collections::BTreeSet<u32>>> {
    static SESSIONS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<String, std::collections::BTreeSet<u32>>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    &SESSIONS
}

static HOST_AREA_STAMP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn note_host_guard_area_occupancy(
    frame: u32,
    trigger: &gamelogic::polygon_trigger::PolygonTrigger,
    occupants: impl Iterator<Item = (ObjectId, glam::Vec3)>,
) {
    let name = trigger.get_trigger_name().as_str().to_string();
    let mut current = std::collections::BTreeSet::new();
    for (id, pos) in occupants {
        if GameLogic::host_point_in_guard_area(trigger, pos) {
            current.insert(id.0);
        }
    }
    let mut sessions = host_area_occupancy()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if sessions.get(&name) != Some(&current) {
        sessions.insert(name, current);
        HOST_AREA_STAMP.store(frame, std::sync::atomic::Ordering::Relaxed);
        gamelogic::ai::set_frame_objects_changed_trigger_areas(frame);
    }
}

/// C++ `AIGuardIdleState::update` per-axis 2-cell (`delta*delta > 4*cell^2`).
pub(crate) fn host_guardee_moved_beyond_return_threshold(
    prev: glam::Vec3,
    now: glam::Vec3,
) -> bool {
    let cell = crate::game_logic::host_repair::PATHFIND_CELL_SIZE_F;
    let limit_sqr = 4.0 * cell * cell;
    let dx = prev.x - now.x;
    if dx * dx > limit_sqr {
        return true;
    }
    let dz = prev.z - now.z;
    dz * dz > limit_sqr
}

fn host_guard_area_stamp_expired(frame: u32, scan_rate: u32) -> bool {
    let leftover_atomic = gamelogic::ai::get_frame_objects_changed_trigger_areas();
    let leftover_gl = gamelogic::helpers::TheGameLogic::get_frame_objects_changed_trigger_areas();
    let host = HOST_AREA_STAMP.load(std::sync::atomic::Ordering::Relaxed);
    let changed = leftover_atomic.max(leftover_gl).max(host);
    changed != 0 && frame > changed.saturating_add(scan_rate)
}

impl GameLogic {
    pub(crate) fn host_named_guard_area_polygon(
        name: &str,
    ) -> Option<(glam::Vec3, f32, gamelogic::polygon_trigger::PolygonTrigger)> {
        let trigger = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| terrain.get_trigger_area_by_name(name).cloned())?;
        let c = trigger.get_center_point();
        let center = glam::Vec3::new(c.x, c.z, c.y);
        let radius = trigger.get_radius();
        Some((center, radius, trigger))
    }

    fn host_point_in_guard_area(
        trigger: &gamelogic::polygon_trigger::PolygonTrigger,
        pos: glam::Vec3,
    ) -> bool {
        trigger.point_in_trigger(&gamelogic::common::Coord2D::new(pos.x, pos.z))
    }

    /// Leftover `Object::relationship_to` / C++ `PartitionFilterRelationship`.
    /// Owner ids use leftover `object_relationship`. Missing owners keep the
    /// faction residual, but Neutral is never Enemies (`ALLOW_ENEMIES`).
    /// C++ `Object::getRelationship` undetected-defector overrides apply first.
    pub(crate) fn host_guard_leftover_relationship(
        &self,
        owner_player: Option<u32>,
        owner_inst: &str,
        owner_team: Team,
        owner_undetected_defector: bool,
        cand: &Object,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;
        if owner_undetected_defector {
            return Relationship::Neutral;
        }
        if cand.is_undetected_defector() {
            return Relationship::Allies;
        }
        match (owner_player, cand.owner_player_id) {
            (Some(_), Some(_)) => Self::object_relationship_from_owners(
                &self.players,
                owner_player,
                owner_inst,
                cand.owner_player_id,
                &cand.team_instance_name,
            ),
            _ => {
                if owner_team == cand.team && owner_team != Team::Neutral {
                    Relationship::Allies
                } else if owner_team == Team::Neutral || cand.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
        }
    }

    /// C++ `lookForInnerTarget` residual: polygon + flying + `ALLOW_ENEMIES`
    /// (EnterGuard: `ALLOW_NEUTRAL` + can-enter). Leftover `relationship_to`.
    pub(super) fn scan_guard_inner_target(
        &self,
        object_id: ObjectId,
        team: Team,
        scan_anchor: glam::Vec3,
        acquire_radius: f32,
        flying_only: bool,
        enter_guard: bool,
        hijack_guard: bool,
        polygon: Option<&gamelogic::polygon_trigger::PolygonTrigger>,
    ) -> Option<ObjectId> {
        if acquire_radius <= 0.0 {
            return None;
        }
        if let Some(trigger) = polygon {
            note_host_guard_area_occupancy(
                self.frame,
                trigger,
                self.objects
                    .iter()
                    .map(|(id, obj)| (*id, obj.get_position())),
            );
            if host_guard_area_stamp_expired(self.frame, self.host_guard_enemy_scan_rate()) {
                return None;
            }
        }
        let (world_min, world_max) = self.world_bounds();
        let (owner_off, owner_player, owner_inst, owner_undetected) = self
            .objects
            .get(&object_id)
            .map(|o| {
                (
                    host_same_map_status_off(o.get_position(), world_min, world_max),
                    o.owner_player_id,
                    o.team_instance_name.clone(),
                    o.is_undetected_defector(),
                )
            })
            .unwrap_or((false, None, String::new(), false));
        let radius_sq = acquire_radius * acquire_radius;
        let mut best: Option<(ObjectId, f32)> = None;
        for (cand_id, cand) in self.objects.iter() {
            if *cand_id == object_id || !cand.is_alive() {
                continue;
            }
            let cand_pos = cand.get_position();
            if owner_off != host_same_map_status_off(cand_pos, world_min, world_max) {
                continue;
            }
            if let Some(trigger) = polygon {
                if !Self::host_point_in_guard_area(trigger, cand_pos) {
                    continue;
                }
            }
            let d_sq = host_guard_xy_dist_sq(scan_anchor, cand_pos);
            if d_sq > radius_sq {
                continue;
            }
            if flying_only && !(cand.is_above_terrain() || cand.status.airborne_target) {
                continue;
            }
            use gamelogic::common::Relationship;
            let rel = self.host_guard_leftover_relationship(
                owner_player,
                &owner_inst,
                team,
                owner_undetected,
                cand,
            );
            if enter_guard {
                if hijack_guard {
                    if rel != Relationship::Enemies || !self.can_hijack_vehicle(object_id, cand) {
                        continue;
                    }
                } else if rel != Relationship::Neutral
                    || !self.can_unit_enter_normal_target(object_id, *cand_id)
                {
                    continue;
                }
            } else {
                if rel != Relationship::Enemies {
                    continue;
                }
                if !matches!(
                    self.get_able_to_attack_specific_object(
                        object_id,
                        *cand_id,
                        AbleToAttackType::NewTarget,
                        false,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                ) {
                    continue;
                }
            }
            if best.map(|(_, bd)| d_sq < bd).unwrap_or(true) {
                best = Some((*cand_id, d_sq));
            }
        }
        best.map(|(id, _)| id)
    }

    /// End an interrupted C++ capture SpecialAbilityUpdate.  An order may be
    /// cancelled while approaching, unpacking, or preparing; in all cases the
    /// source must not retain a stale channel or `IS_USING_ABILITY` bit.
    fn host_guard_inner_outer_for(&self, object_id: ObjectId, guard_radius: f32) -> (f32, f32) {
        const GUARD_MIN_RADIUS: f32 = 80.0;
        let (std_inner, std_outer) = self.host_std_guard_ranges(object_id);
        let mood = self
            .objects
            .get(&object_id)
            .map(|o| o.ai_attitude)
            .unwrap_or(0);
        let inner = if mood <= -2 {
            0.0
        } else if std_inner > 0.0 {
            std_inner
        } else if guard_radius > 0.0 {
            guard_radius
        } else {
            GUARD_MIN_RADIUS
        };
        let mut outer = if std_outer > 0.0 {
            std_outer
        } else {
            inner * 1.5
        };
        // C++ AIGuardOuterState::onEnter: range = max(vision, area->getRadius()).
        if let Some(name) = self
            .objects
            .get(&object_id)
            .and_then(|o| o.guard_area_trigger.as_deref())
            .filter(|n| !n.is_empty())
        {
            if let Some((_, poly_r, _)) = Self::host_named_guard_area_polygon(name) {
                if poly_r > outer {
                    outer = poly_r;
                }
            }
        }
        (inner, outer)
    }

    fn begin_guard_chase(&mut self, object_id: ObjectId, phase: u8) {
        let frames = self.host_guard_chase_unit_frames();
        let now = self.frame;
        if let Some(o) = self.objects.get_mut(&object_id) {
            o.guard_chase_phase = phase;
            o.guard_chase_give_up_frame = if phase == GUARD_CHASE_PHASE_INNER {
                0
            } else {
                now.saturating_add(frames)
            };
        }
    }

    fn begin_guard_chase_acquired(&mut self, object_id: ObjectId, target_id: ObjectId) {
        let (mut anchor, guard_radius) = {
            let Some(o) = self.objects.get(&object_id) else {
                return;
            };
            let anchor = if let Some(gid) = o.guard_target {
                self.objects
                    .get(&gid)
                    .filter(|g| g.is_alive())
                    .map(|g| g.get_position())
                    .or(o.guard_position)
                    .unwrap_or_else(|| o.get_position())
            } else {
                o.guard_position.unwrap_or_else(|| o.get_position())
            };
            (anchor, o.guard_radius)
        };
        if let Some(name) = self
            .objects
            .get(&object_id)
            .and_then(|o| o.guard_area_trigger.as_deref())
            .filter(|n| !n.is_empty())
        {
            if let Some((c, _, _)) = Self::host_named_guard_area_polygon(name) {
                anchor = c;
            }
        }
        let tgt_pos = self
            .objects
            .get(&target_id)
            .map(|t| t.get_position())
            .unwrap_or(anchor);
        let (inner, _) = self.host_guard_inner_outer_for(object_id, guard_radius);
        let phase = if inner > 0.0 && host_guard_xy_dist_sq(anchor, tgt_pos) > inner * inner {
            GUARD_CHASE_PHASE_OUTER
        } else {
            GUARD_CHASE_PHASE_INNER
        };
        self.begin_guard_chase(object_id, phase);
    }

    pub(super) fn engage_guard_target(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        aggressor: bool,
    ) -> bool {
        if aggressor {
            self.begin_guard_chase(object_id, GUARD_CHASE_PHASE_AGGRESSOR);
        } else {
            self.begin_guard_chase_acquired(object_id, target_id);
        }
        let ok = self.engage_target_decision_aware(object_id, target_id);
        if !ok {
            if let Some(o) = self.objects.get_mut(&object_id) {
                o.clear_guard_chase();
            }
        }
        ok
    }

    fn end_guard_chase_attack(&mut self, object_id: ObjectId) {
        if let Some(o) = self.objects.get_mut(&object_id) {
            o.clear_guard_chase();
        }
        self.stop_attack_decision_aware(object_id);
        // C++ Return/Idle onEnter re-seeds m_nextEnemyScanTime / m_nextReturnScanTime.
        self.guard_next_enemy_scan.remove(&object_id);
        // C++ AIGuardInnerState::onExit / AttackAggressor::onExit:
        // getTeam()->setTeamTargetObject(NULL).
        self.set_host_team_common_target(object_id, None);
        // C++ INNER→OUTER→GET_CRATE→RETURN: InternalMoveTo the post.
        self.return_guard_to_post(object_id);
    }

    /// C++ AIGuardReturnState::onEnter — walk to guardee / polygon center / post.
    fn return_guard_to_post(&mut self, object_id: ObjectId) {
        let Some(o) = self.objects.get(&object_id) else {
            return;
        };
        let (goal, state) = if let Some(gid) = o.guard_target {
            let pos = self
                .objects
                .get(&gid)
                .filter(|g| g.is_alive())
                .map(|g| g.get_position())
                .or(o.guard_position)
                .unwrap_or_else(|| o.get_position());
            (pos, AIState::GuardingObject)
        } else if let Some(name) = o.guard_area_trigger.as_deref().filter(|n| !n.is_empty()) {
            let center = Self::host_named_guard_area_polygon(name)
                .map(|(c, _, _)| c)
                .or(o.guard_position)
                .unwrap_or_else(|| o.get_position());
            (center, AIState::GuardingArea)
        } else if let Some(pos) = o.guard_position {
            (pos, AIState::GuardingArea)
        } else {
            return;
        };
        if !self.objects.get(&object_id).is_some_and(|o| o.can_move()) {
            if let Some(o) = self.objects.get_mut(&object_id) {
                o.set_ai_state(state);
            }
            return;
        }
        self.path_approach_with_state(object_id, goal, state);
    }

    /// C++ AIGuardInner / Outer / AttackAggressor ExitConditions while Attacking.
    pub(super) fn tick_guard_chase_exits(&mut self, object_id: ObjectId) -> bool {
        let snapshot = match self.objects.get(&object_id) {
            Some(o) if o.guard_chase_phase != 0 => (
                o.guard_chase_phase,
                o.guard_chase_give_up_frame,
                o.target,
                o.guard_position,
                o.guard_target,
                o.guard_radius,
                o.guard_mode,
            ),
            _ => return false,
        };
        let (phase, give_up, target_id, guard_pos, guard_tgt, guard_radius, guard_mode) = snapshot;
        let Some(tid) = target_id else {
            self.end_guard_chase_attack(object_id);
            return true;
        };
        let Some(tgt) = self.objects.get(&tid) else {
            self.end_guard_chase_attack(object_id);
            return true;
        };
        if !tgt.is_alive() || tgt.status.destroyed {
            self.end_guard_chase_attack(object_id);
            return true;
        }
        let tgt_pos = tgt.get_position();
        let mut anchor = if let Some(gid) = guard_tgt {
            self.objects
                .get(&gid)
                .filter(|g| g.is_alive())
                .map(|g| g.get_position())
                .or(guard_pos)
                .unwrap_or(tgt_pos)
        } else {
            guard_pos.unwrap_or(tgt_pos)
        };
        // C++ AIGuardOuterState::onEnter uses polygon center as the chase leash.
        if let Some(name) = self
            .objects
            .get(&object_id)
            .and_then(|o| o.guard_area_trigger.as_deref())
            .filter(|n| !n.is_empty())
        {
            if let Some((c, _, _)) = Self::host_named_guard_area_polygon(name) {
                anchor = c;
            }
        }
        let (inner, outer) = self.host_guard_inner_outer_for(object_id, guard_radius);
        let dist_sq = host_guard_xy_dist_sq(anchor, tgt_pos);
        let without_pursuit = matches!(guard_mode, crate::game_logic::GuardMode::WithoutPursuit);
        let now = self.frame;
        match phase {
            GUARD_CHASE_PHASE_INNER => {
                if inner > 0.0 && dist_sq > inner * inner {
                    if without_pursuit {
                        // C++ AIGuardOuterState::onEnter GUARDMODE_GUARD_WITHOUT_PURSUIT → SUCCESS.
                        self.end_guard_chase_attack(object_id);
                        return true;
                    }
                    self.begin_guard_chase(object_id, GUARD_CHASE_PHASE_OUTER);
                    if outer > 0.0 && dist_sq > outer * outer {
                        self.end_guard_chase_attack(object_id);
                        return true;
                    }
                }
                false
            }
            GUARD_CHASE_PHASE_OUTER => {
                let mut give_up = give_up;
                if inner > 0.0 && dist_sq <= inner * inner {
                    let frames = self.host_guard_chase_unit_frames();
                    give_up = now.saturating_add(frames);
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        o.guard_chase_give_up_frame = give_up;
                    }
                }
                if now >= give_up {
                    self.end_guard_chase_attack(object_id);
                    return true;
                }

                if outer > 0.0 && dist_sq > outer * outer {
                    self.end_guard_chase_attack(object_id);
                    return true;
                }
                false
            }
            GUARD_CHASE_PHASE_AGGRESSOR => {
                if now >= give_up {
                    self.end_guard_chase_attack(object_id);
                    return true;
                }
                if inner > 0.0 && dist_sq > inner * inner {
                    self.end_guard_chase_attack(object_id);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// C++ hasAttackedMeAndICanReturnFire — consume last attacker and engage.
    pub(super) fn try_guard_last_attacker(&mut self, object_id: ObjectId, team: Team) -> bool {
        let last = self
            .objects
            .get_mut(&object_id)
            .and_then(|o| o.last_damage_source.take());
        let Some(aid) = last else {
            return false;
        };
        if aid == object_id {
            return false;
        }
        let Some(atk) = self.objects.get(&aid) else {
            return false;
        };
        if !atk.is_alive() || atk.status.destroyed || !atk.is_targetable_by_enemy_of(team) {
            return false;
        }
        if !matches!(
            self.get_able_to_attack_specific_object(
                object_id,
                aid,
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        self.engage_guard_target(object_id, aid, true)
    }

    /// C++ Idle/Return `m_nextEnemyScanTime`. First visit matches onEnter
    /// `now + GameLogicRandomValue(0, rate)`; later looks add the AIData rate.
    pub(crate) fn guard_acquire_scan_due(&mut self, object_id: ObjectId, returning: bool) -> bool {
        let rate = if returning {
            self.host_guard_enemy_return_scan_rate()
        } else {
            self.host_guard_enemy_scan_rate()
        }
        .max(1);
        let now = self.frame;
        match self.guard_next_enemy_scan.get(&object_id).copied() {
            Some(next) if now < next => return false,
            None => {
                let offset = gamelogic::helpers::game_logic_random_value(0, rate);
                let next = now.saturating_add(offset);
                if now < next {
                    self.guard_next_enemy_scan.insert(object_id, next);
                    return false;
                }
            }
            Some(_) => {}
        }
        self.guard_next_enemy_scan
            .insert(object_id, now.saturating_add(rate));
        true
    }

    /// C++ EnterGuard / HijackGuard: board instead of shooting.
    pub(super) fn try_guard_enter_or_hijack(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        hijack: bool,
        team: Team,
    ) -> bool {
        if hijack {
            let legal = self.objects.get(&target_id).is_some_and(|t| {
                t.is_alive()
                    && t.is_targetable_by_enemy_of(team)
                    && t.is_kind_of(KindOf::Vehicle)
                    && !t.is_hijacked()
            });
            if !legal {
                return false;
            }
            let pos = self.objects.get(&target_id).map(|t| t.get_position());
            if let Some(o) = self.objects.get_mut(&object_id) {
                o.target = Some(target_id);
                o.set_ai_state(AIState::SpecialAbility);
            }
            self.queue_pending_special_ability(
                object_id,
                crate::game_logic::PendingSpecialAbility::Hijack { target_id },
            );
            if let Some(pos) = pos {
                self.path_approach_with_state(object_id, pos, AIState::SpecialAbility);
            }
            true
        } else if self.can_unit_enter_normal_target(object_id, target_id) {
            if let Some(o) = self.objects.get_mut(&object_id) {
                o.target = Some(target_id);
                o.set_order_target(Some(target_id));
                o.set_ai_state(AIState::Entering);
            }
            if let Some(pos) = self.objects.get(&target_id).map(|t| t.get_position()) {
                self.path_approach_with_state(object_id, pos, AIState::Entering);
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod guard_follow_threshold_tests {
    use super::host_guardee_moved_beyond_return_threshold;
    use crate::game_logic::host_repair::PATHFIND_CELL_SIZE_F;

    #[test]
    fn idle_follows_guardee_per_axis_two_cells_not_euclidean_four() {
        let post = glam::Vec3::ZERO;
        let x_only = glam::Vec3::new(PATHFIND_CELL_SIZE_F * 2.5, 0.0, 0.0);
        assert!(
            host_guardee_moved_beyond_return_threshold(post, x_only),
            "2.5 cells on X alone must return-to-post"
        );
        let diagonal_under =
            glam::Vec3::new(PATHFIND_CELL_SIZE_F * 1.5, 0.0, PATHFIND_CELL_SIZE_F * 1.5);
        assert!(
            !host_guardee_moved_beyond_return_threshold(post, diagonal_under),
            "1.5 cells on both ground axes stays idle"
        );
        let exactly_two = glam::Vec3::new(PATHFIND_CELL_SIZE_F * 2.0, 0.0, 0.0);
        assert!(
            !host_guardee_moved_beyond_return_threshold(post, exactly_two),
            "exactly 2 cells is not greater than 4*cell*cell"
        );
    }
}
