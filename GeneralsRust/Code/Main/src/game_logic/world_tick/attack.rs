//! Host tick `impl GameLogic` — `attack`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub fn cannot_possibly_attack_object(
        &self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        force_attacking: bool,
    ) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return true;
        };
        // C++ callers check isAbleToAttack before getAbleToAttackSpecificObject.
        if !obj.can_attack() {
            return true;
        }
        let attack_type = if force_attacking {
            AbleToAttackType::ContinuedTargetForced
        } else {
            AbleToAttackType::ContinuedTarget
        };
        // AI command residual (not player click).
        let result =
            self.get_able_to_attack_specific_object(unit_id, victim_id, attack_type, false);
        !matches!(
            result,
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        )
    }

    pub fn attack_state_enter(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
    ) -> AttackMachineResult {
        {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            if !u.is_alive() || u.status.under_construction {
                return AttackMachineResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() && u.tertiary_weapon.is_none() {
                return AttackMachineResult::Failure;
            }
            if u.is_kind_of(crate::game_logic::KindOf::Projectile) {
                return AttackMachineResult::Failure;
            }
        }
        {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackMachineResult::Failure;
            }
        }
        // C++ getMoodMatrixActionAdjustment(MM_Action_Attack) residual.
        // AI-controlled sleep mood refuses attack (→ idle).
        if !self.mood_allows_attack(unit_id, false) {
            return AttackMachineResult::Failure;
        }
        // C++ cannotPossiblyAttackObject residual on enter.
        if self.cannot_possibly_attack_object(unit_id, victim_id, false) {
            return AttackMachineResult::Failure;
        }
        // C++ AIAttackState::chooseWeapon residual (PreferMostDamage).
        let t = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        if !self.choose_best_weapon_for_target(unit_id, Some(victim_id), t) {
            return AttackMachineResult::Failure;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            // Nested SM bookkeeping stays host; engagement authority is log-only when on.
            u.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
            if u.max_shots_to_fire == 0 {
                u.max_shots_to_fire = -1;
            }
            if !decision_auth {
                u.target = Some(victim_id);
                u.set_status_attacking(true);
                u.set_ai_state(AIState::Attacking);
            }
        }
        if decision_auth {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        let _ = self.attack_aim_at_target_enter(unit_id);
        AttackMachineResult::Continue
    }

    /// C++ AIAttackState::onExit residual.
    pub fn attack_state_exit(&mut self, unit_id: ObjectId) {
        self.attack_aim_at_target_exit(unit_id);
        self.attack_fire_weapon_exit(unit_id);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            // C++ AIAttackState::onExit releases only temporary locks.  This
            // includes an interrupted explicit TERTIARY attack, while a UI
            // weapon toggle's permanent lock remains authoritative.
            u.release_weapon_lock(WeaponLockType::LockedTemporarily);
            u.set_status_attacking(false);
            u.status.is_aiming_weapon = false;
            u.status.is_firing_weapon = false;
            u.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
        }
    }

    /// Drive nested AttackStateMachine for units that entered via attack_state_enter.
    ///
    /// Units owned by the SM have is_aiming_weapon / is_firing_weapon or a non-Aim
    /// substate after approach. Legacy update_combat still owns plain Attacking units.
    pub(crate) fn tick_nested_attack_machines(
        &mut self,
        object_ids: &[ObjectId],
        current_time: f32,
        logic_frame: u32,
    ) {
        let mut stop: Vec<ObjectId> = Vec::new();
        for &id in object_ids {
            let (drive, victim) = {
                let Some(u) = self.objects.get(&id) else {
                    continue;
                };
                if u.ai_state != AIState::Attacking {
                    continue;
                }
                let sm_owned = u.status.is_aiming_weapon
                    || u.status.is_firing_weapon
                    || !matches!(
                        u.attack_substate,
                        crate::game_logic::AttackSubState::AimAtTarget
                    );
                if !sm_owned {
                    continue;
                }
                let Some(vid) = u.target else {
                    stop.push(id);
                    continue;
                };
                (true, vid)
            };
            if !drive {
                continue;
            }
            match self.tick_attack_state_machine(id, victim, current_time, logic_frame, 0.35) {
                AttackMachineResult::Continue => {}
                AttackMachineResult::Success | AttackMachineResult::Failure => {
                    stop.push(id);
                }
            }
        }
        for id in stop {
            self.attack_state_exit(id);
            self.stop_attack_decision_aware(id);
        }
    }

    /// C++ AttackStateMachine + AIAttackState::update residual (object attack).

    /// C++ canPursue residual for AttackStateMachine CHASE vs APPROACH.

    /// C++ outOfWeaponRangeObject state condition residual.
    ///
    /// True when view is LOS-blocked or target is outside attack range
    /// (unless leech-range is active).
    pub fn out_of_weapon_range_object(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return true;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return true;
        };
        if obj.weapon.is_none() && obj.secondary_weapon.is_none() && obj.tertiary_weapon.is_none() {
            return true;
        }
        // C++ AIAttackState queries the *current* weapon, not whichever
        // weapon happens to have the longest range.  A lock owns that
        // identity, including TERTIARY, and invalid restored slots fail
        // closed instead of becoming PRIMARY.
        let Some(slot) = obj.selected_weapon_slot() else {
            return true;
        };
        let Some(weapon) = obj.weapon_slot(slot) else {
            return true;
        };
        // Leech range residual: temporarily unlimited while engaged, but
        // only for the selected concrete slot.
        if obj.leech_range_active_for_slot(slot) {
            return false;
        }
        // Contact weapon residual: skip LOS false positives at tiny ranges.
        let contact =
            weapon.range <= 5.0 || weapon.min_range > 0.0 && weapon.range <= weapon.min_range * 2.0;
        // Ground LOS residual.
        let on_ground = !obj.status.airborne_target
            || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            || obj.contained_by.is_some()
            || !obj.can_move();
        let victim_air = victim.status.airborne_target;
        if !contact && on_ground && !victim_air {
            let from = obj.get_position();
            let to = victim.get_position();
            if self.attack_view_blocked(unit_id, Some(victim_id), to)
                || self.pathfinding_system.is_attack_view_blocked(from, to)
            {
                return true;
            }
        }
        !obj.is_within_attack_range_for_slot(slot, victim)
    }

    /// C++ wantToSquishTarget state condition residual.
    ///
    /// AI computer crush/squish chase preference (not player).
    pub fn want_to_squish_target(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if victim.contained_by.is_some() {
            return false;
        }
        // Fail-closed: host does not track player type on every object; use team AI residual.
        // Prefer vehicles crushing infantry.
        // DontAutoCrushInfantry residual: skip if template name hints "dozer" without crush.
        obj.can_crush_only(victim, false)
    }

    pub fn should_chase_attack_target(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        let Some(v) = self.objects.get(&victim_id) else {
            return false;
        };
        if !u.is_alive() || !v.is_alive() {
            return false;
        }
        // Immobile / projectile residual: never chase.
        if !u.can_move() || u.is_kind_of(crate::game_logic::KindOf::Projectile) {
            return false;
        }
        // Stealthed undetected residual.
        if v.status.stealthed && !v.status.detected && !v.status.disguised {
            return false;
        }
        // C++ wantToSquishTarget → CHASE_TARGET condition residual.
        if self.want_to_squish_target(unit_id, victim_id) {
            return true;
        }
        u.can_pursue_target(v)
    }

    pub fn tick_attack_state_machine(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
        max_turn_rad: f32,
    ) -> AttackMachineResult {
        {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            if !u.is_alive() || u.status.under_construction {
                return AttackMachineResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() && u.tertiary_weapon.is_none() {
                return AttackMachineResult::Failure;
            }
            if u.max_shots_to_fire == 0 {
                return AttackMachineResult::Failure;
            }
        }
        {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Success;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackMachineResult::Success;
            }
        }
        // C++ cannotPossiblyAttackObject condition → EXIT_MACHINE_WITH_FAILURE.
        if self.cannot_possibly_attack_object(unit_id, victim_id, false) {
            return AttackMachineResult::Failure;
        }
        // Re-evaluate weapon choice every frame (C++ AIAttackState::update).
        let _ = self.choose_best_weapon_for_target(unit_id, Some(victim_id), current_time);

        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_target(Some(victim_id));
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
        }

        let in_range = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Success;
            };
            u.selected_weapon_slot()
                .is_some_and(|slot| u.is_within_attack_range_for_slot(slot, v))
        };
        // C++ outOfWeaponRangeObject condition (range + LOS, leech-aware).
        let out_of_wr = self.out_of_weapon_range_object(unit_id, victim_id);

        let sub = self
            .objects
            .get(&unit_id)
            .map(|u| u.attack_substate)
            .unwrap_or(crate::game_logic::AttackSubState::AimAtTarget);

        use crate::game_logic::AttackSubState;
        match sub {
            AttackSubState::AimAtTarget => {
                if out_of_wr {
                    let chase = self.should_chase_attack_target(unit_id, victim_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = if chase {
                            AttackSubState::ChaseTarget
                        } else {
                            AttackSubState::ApproachTarget
                        };
                        u.status.is_aiming_weapon = false;
                    }
                    let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                    return AttackMachineResult::Continue;
                }
                match self.attack_aim_at_target_update(unit_id, victim_id, max_turn_rad) {
                    AttackAimResult::Success => {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::FireWeapon;
                            u.status.is_aiming_weapon = false;
                        }
                        let _ = self.attack_fire_weapon_enter(unit_id);
                    }
                    AttackAimResult::Continue => {}
                    AttackAimResult::Failure => return AttackMachineResult::Failure,
                }
                AttackMachineResult::Continue
            }
            AttackSubState::FireWeapon => {
                if out_of_wr {
                    let chase = self.should_chase_attack_target(unit_id, victim_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = if chase {
                            AttackSubState::ChaseTarget
                        } else {
                            AttackSubState::ApproachTarget
                        };
                        u.status.is_firing_weapon = false;
                    }
                    let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                    return AttackMachineResult::Continue;
                }
                let fire = self.attack_fire_weapon_update(unit_id, victim_id, current_time);
                match fire {
                    AttackFireResult::Continue => {
                        // PRE_ATTACK wind-up — stay in FIRE.
                    }
                    AttackFireResult::Success | AttackFireResult::Failure => {
                        // C++ both edges return to AIM_AT_TARGET.
                        self.attack_fire_weapon_exit(unit_id);
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::AimAtTarget;
                        }
                        let _ = self.attack_aim_at_target_enter(unit_id);
                    }
                }
                AttackMachineResult::Continue
            }
            AttackSubState::ApproachTarget => {
                if in_range {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::AimAtTarget;
                        u.set_status_moving(false);
                    }
                    let _ = self.attack_aim_at_target_enter(unit_id);
                    return AttackMachineResult::Continue;
                }
                // Fleeing victim may upgrade Approach → Chase mid-path.
                if self.should_chase_attack_target(unit_id, victim_id) {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::ChaseTarget;
                    }
                }
                let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.approach_timestamp = logic_frame;
                    u.set_status_moving(true);
                }
                AttackMachineResult::Continue
            }
            AttackSubState::ChaseTarget => {
                // C++ AIAttackPursueTargetState residual.
                // Stealthed undetected victim: drop to approach.
                {
                    let stealth_bail = self
                        .objects
                        .get(&victim_id)
                        .map(|v| v.status.stealthed && !v.status.detected && !v.status.disguised)
                        .unwrap_or(false);
                    if stealth_bail {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::ApproachTarget;
                        }
                        return AttackMachineResult::Continue;
                    }
                }
                if in_range {
                    // Match speeds residual while goal still in range (victim * 0.95).
                    let victim_spd = self
                        .objects
                        .get(&victim_id)
                        .map(|v| v.forward_speed_2d().abs())
                        .unwrap_or(0.0);
                    if let Some(attacker) = self.objects.get_mut(&unit_id) {
                        let desired = victim_spd * 0.95;
                        let vel = attacker.movement.velocity;
                        let speed = (vel.x * vel.x + vel.z * vel.z).sqrt();
                        if speed > 1e-3 && desired > 0.0 {
                            let scale = (desired / speed).min(1.0);
                            attacker.movement.velocity.x *= scale;
                            attacker.movement.velocity.z *= scale;
                        }
                        attacker.attack_substate = AttackSubState::AimAtTarget;
                        attacker.set_status_moving(false);
                    }
                    let _ = self.attack_aim_at_target_enter(unit_id);
                    return AttackMachineResult::Continue;
                }
                // canPursue false → drop to Approach (C++ onEnter SUCCESS → approach).
                if !self.should_chase_attack_target(unit_id, victim_id) {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::ApproachTarget;
                    }
                }
                let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.approach_timestamp = logic_frame;
                    u.set_status_moving(true);
                }
                AttackMachineResult::Continue
            }
        }
    }

    /// C++ AIUpdateInterface::setTurretTargetObject residual.
    pub fn set_turret_target_object(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        force_attacking: bool,
    ) {
        if let Some(vid) = victim_id {
            let alive = self
                .objects
                .get(&vid)
                .map(|v| v.is_alive() && !v.status.destroyed)
                .unwrap_or(false);
            if !alive {
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.set_turret_target_object(None, false);
                    if u.turret_substate == crate::game_logic::object::TurretSubState::Hold {
                        u.turret_hold_until_frame =
                            self.frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                    }
                }
                return;
            }
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_turret_target_object(victim_id, force_attacking);
            if victim_id.is_none()
                && u.turret_substate == crate::game_logic::object::TurretSubState::Hold
            {
                u.turret_hold_until_frame =
                    self.frame.saturating_add(u.turret_recenter_frames.max(1));
                u.turret_holding = true;
            }
        }
    }

    /// C++ TurretAIAimTurretState::update residual — rotate turret toward victim.
    ///
    /// Returns Success when within REL_THRESH (~2°), Continue while turning,
    /// Failure if no target / dead.

    /// C++ TurretAI state machine residual (AIM/FIRE/HOLD/RECENTER).
    ///
    /// Call once per frame for turret-enabled units.
    pub fn tick_turret_state_machine(
        &mut self,
        unit_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
    ) -> AttackAimResult {
        use crate::game_logic::object::TurretSubState;

        let (enabled, sub, tid) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.turret_enabled || !u.is_alive() {
                return AttackAimResult::Failure;
            }
            (true, u.turret_substate, u.turret_target_id)
        };
        let _ = enabled;

        match sub {
            TurretSubState::Idle | TurretSubState::IdleScan => {
                // Idle residual owned by strategy-center host; no-op here.
                AttackAimResult::Continue
            }
            TurretSubState::Aim => {
                let Some(vid) = tid else {
                    // No target → HOLD
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Hold;
                        u.turret_hold_until_frame =
                            logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                        u.record_host_turret();
                    }
                    return AttackAimResult::Continue;
                };
                // Dead victim → HOLD
                let alive = self
                    .objects
                    .get(&vid)
                    .map(|v| v.is_alive() && !v.status.destroyed)
                    .unwrap_or(false);
                if !alive {
                    self.set_turret_target_object(unit_id, None, false);
                    return AttackAimResult::Continue;
                }
                // OOR while aiming stays in AIM (body approach elsewhere).
                match self.tick_turret_aim(unit_id, 1.0) {
                    AttackAimResult::Success => {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Fire;
                        }
                        let _ = self.attack_fire_weapon_enter(unit_id);
                        AttackAimResult::Success
                    }
                    other => other,
                }
            }
            TurretSubState::Fire => {
                let Some(vid) = tid else {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    return AttackAimResult::Continue;
                };
                // C++ fireConditions: outOfWeaponRangeObject → AIM
                if self.out_of_weapon_range_object(unit_id, vid) {
                    self.attack_fire_weapon_exit(unit_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    return AttackAimResult::Continue;
                }
                let fire = self.attack_fire_weapon_update(unit_id, vid, current_time);
                match fire {
                    AttackFireResult::Continue => AttackAimResult::Continue,
                    AttackFireResult::Success | AttackFireResult::Failure => {
                        // C++ FIRE success and failure both return to AIM.
                        self.attack_fire_weapon_exit(unit_id);
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Aim;
                        }
                        AttackAimResult::Continue
                    }
                }
            }
            TurretSubState::Hold => {
                let done = {
                    let Some(u) = self.objects.get(&unit_id) else {
                        return AttackAimResult::Failure;
                    };
                    logic_frame >= u.turret_hold_until_frame && u.turret_hold_until_frame > 0
                };
                if done {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_holding = false;
                        u.record_host_turret();
                        u.turret_hold_until_frame = 0;
                        u.turret_substate = TurretSubState::Recenter;
                        u.turret_idle_recentering = true;
                    }
                }
                AttackAimResult::Continue
            }
            TurretSubState::Recenter => {
                // C++ rate modifier 0.5 toward natural angle/pitch.
                let (ang_ok, pitch_ok) = {
                    let Some(u) = self.objects.get_mut(&unit_id) else {
                        return AttackAimResult::Failure;
                    };
                    let nat_a = u.turret_natural_angle_deg.to_radians();
                    let nat_p = u.turret_natural_pitch_deg.to_radians();
                    let a = u.turn_turret_towards_angle_rad(nat_a, 0.5, 0.0);
                    let p = u.turn_turret_towards_pitch_rad(nat_p, 0.5);
                    (a, p)
                };
                if ang_ok && pitch_ok {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Idle;
                        u.turret_idle_recentering = false;
                        u.turret_rotating = false;
                    }
                    AttackAimResult::Success
                } else {
                    AttackAimResult::Continue
                }
            }
        }
    }

    /// Drive TurretAI SM for all turret-enabled objects.
    pub(crate) fn tick_all_turret_state_machines(
        &mut self,
        object_ids: &[ObjectId],
        current_time: f32,
        logic_frame: u32,
    ) {
        for &id in object_ids {
            let enabled = self
                .objects
                .get(&id)
                .map(|o| o.turret_enabled && o.is_alive())
                .unwrap_or(false);
            if enabled {
                let _ = self.tick_turret_state_machine(id, current_time, logic_frame);
            }
        }
    }

    pub fn tick_turret_aim(
        &mut self,
        unit_id: ObjectId,
        max_rate_modifier: f32,
    ) -> AttackAimResult {
        const REL_THRESH: f32 = 0.035; // ~2 degrees
        let (victim_pos, has_target) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.turret_enabled {
                return AttackAimResult::Failure;
            }
            let Some(tid) = u.turret_target_id else {
                return AttackAimResult::Failure;
            };
            let Some(v) = self.objects.get(&tid) else {
                return AttackAimResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackAimResult::Failure;
            }
            (v.get_position(), true)
        };
        let _ = has_target;
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return AttackAimResult::Failure;
        };
        u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
        // Relative angle body→target in world, convert to body-relative turret aim.
        // Host residual: body orientation + turret angle compose aim direction.
        let body_ori = u.get_orientation();
        let rel_world = u.relative_angle_2d_to(victim_pos); // already body-relative
                                                            // Desired turret angle = current body-relative target heading.
                                                            // relative_angle_2d_to returns how much body must turn; for turret,
                                                            // desired turret yaw (body-relative) = current turret + (rel - 0) wait:
                                                            // C++: relAngle = getRelativeAngle2D(obj, enemyPos) which is target bearing
                                                            // relative to object orientation. Turret angle is also relative to parent.
                                                            // So desired turret angle = relAngle (if turret 0 faces body forward).
        let desired_turret = rel_world;
        let aligned = u.turn_turret_towards_angle_rad(
            desired_turret,
            max_rate_modifier.max(0.01),
            REL_THRESH,
        );
        // Clear unused body_ori warning path: used for future pitch.
        let _ = body_ori;
        if aligned {
            AttackAimResult::Success
        } else {
            AttackAimResult::Continue
        }
    }

    pub fn attack_aim_at_target_enter(&mut self, unit_id: ObjectId) -> bool {
        let (alive, has_wpn, tid) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            (
                u.is_alive(),
                u.weapon.is_some() || u.secondary_weapon.is_some() || u.tertiary_weapon.is_some(),
                u.target,
            )
        };
        if !alive || !has_wpn {
            return false;
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_status_aiming_weapon(true);
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        // C++ AIAttackAimAtTargetState sets turret target when tur != INVALID.
        if let Some(vid) = tid {
            self.set_turret_target_object(unit_id, Some(vid), false);
        }
        true
    }

    /// C++ AIAttackAimAtTargetState::onExit residual.
    pub fn attack_aim_at_target_exit(&mut self, unit_id: ObjectId) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.status.is_aiming_weapon = false;
        }
    }

    /// C++ AIAttackAimAtTargetState::update residual (body turn, no turret).
    ///
    /// Turns in place toward victim using AcceptableAimDelta; returns Success
    /// when |relAngle| < aimDelta.
    pub fn attack_aim_at_target_update(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_turn_rad: f32,
    ) -> AttackAimResult {
        // Snapshot victim position + liveness.
        let victim_pos = {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackAimResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackAimResult::Failure;
            }
            v.get_position()
        };

        // Ensure turret tracks the same victim (C++ setTurretTargetObject).
        self.set_turret_target_object(unit_id, Some(victim_id), false);

        let (body_aimed, range_ok, turret_enabled) = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.is_alive() {
                return AttackAimResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() && u.tertiary_weapon.is_none() {
                return AttackAimResult::Failure;
            }
            u.set_status_aiming_weapon(true);
            u.set_target(Some(victim_id));
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            }
            let Some(slot) = u.selected_weapon_slot() else {
                return AttackAimResult::Failure;
            };
            let body_aimed = u.turn_toward_position(victim_pos, slot, max_turn_rad.max(0.05));
            let range_ok = u.is_within_attack_range_pos_for_slot(slot, victim_pos);
            (body_aimed, range_ok, u.turret_enabled)
        };

        // C++ body-aim path: if no turret turn rate, body alignment is enough.
        // When turret enabled, both body (or immobile) and turret must align.
        let turret_res = if turret_enabled {
            self.tick_turret_aim(unit_id, 1.0)
        } else {
            AttackAimResult::Success
        };

        let turret_ok = matches!(turret_res, AttackAimResult::Success);
        if body_aimed && turret_ok {
            return AttackAimResult::Success;
        }
        if !range_ok && !body_aimed {
            return AttackAimResult::Failure;
        }
        // Still turning body or turret.
        if matches!(turret_res, AttackAimResult::Failure) && !body_aimed {
            return AttackAimResult::Failure;
        }
        AttackAimResult::Continue
    }

    pub fn attack_fire_weapon_enter(&mut self, unit_id: ObjectId) -> bool {
        {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.is_alive() {
                return false;
            }
            u.set_status_firing_weapon(true);
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        true
    }

    /// C++ AIAttackFireWeaponState::onExit residual — clear IS_FIRING_WEAPON.
    pub fn attack_fire_weapon_exit(&mut self, unit_id: ObjectId) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.status.is_firing_weapon = false;
        }
    }

    /// C++ AIAttackFireWeaponState::update residual — fire once at victim.
    ///
    /// Object path only (ground attack uses fire_at with a dummy/position path later).
    pub fn attack_fire_weapon_update(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
    ) -> AttackFireResult {
        // Snapshot checks without holding mut.
        let (alive, can_fire_now, pre_continue, in_range) = {
            let Some(atk) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if !atk.is_alive() {
                return AttackFireResult::Failure;
            }
            let Some(vic) = self.objects.get(&victim_id) else {
                return AttackFireResult::Failure;
            };
            if !vic.is_alive() || vic.status.destroyed {
                return AttackFireResult::Failure;
            }
            let pre_continue = atk.pre_attack_target == Some(victim_id)
                && atk.pre_attack_ready_at > current_time + 1e-6;
            let can = atk.can_fire(current_time);
            let Some(slot) = atk.selected_weapon_slot() else {
                return AttackFireResult::Failure;
            };
            let range_ok = atk.is_within_attack_range_for_slot(slot, vic);
            (true, can, pre_continue, range_ok)
        };
        let _ = alive;
        if pre_continue {
            return AttackFireResult::Continue;
        }
        if !can_fire_now {
            // Try fire_at anyway — it may arm pre-attack.
        }
        if !in_range && !pre_continue {
            // Still allow fire_at to arm pre-attack if can_fire and target set.
            // Out of range without wind-up => failure.
            let Some(atk) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if atk.pre_attack_ready_at <= 0.0 {
                return AttackFireResult::Failure;
            }
        }

        // The nested AttackStateMachine is an active weapon path separate
        // from update_combat.  Gate the actual discharge here as well so a
        // DeployStyle unit that reached FIRE before unpacking cannot bypass
        // the parsed DeployStyleAIUpdate state machine.  Do not start an
        // unpack merely for an out-of-range approach.
        if in_range && !self.ensure_deploy_style_ready_to_fire(unit_id) {
            return AttackFireResult::Continue;
        }

        let (victim_infantry, victim_faerie) = self
            .objects
            .get(&victim_id)
            .map(|v| (v.is_kind_of(KindOf::Infantry), v.is_faerie_fire()))
            .unwrap_or((false, false));
        let fired = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackFireResult::Failure;
            };
            u.set_status_firing_weapon(true);
            u.set_target(Some(victim_id));
            u.set_ai_state(AIState::Attacking);
            u.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            }
            // TARGET_FAERIE_FIRE ROF residual when victim is painted.
            u.fire_at_ex(victim_id, current_time, victim_infantry, victim_faerie)
        };

        if fired {
            // max_shots_to_fire decremented inside Object::fire_at_ex (Weapon::m_maxShotCount).
            AttackFireResult::Success
        } else {
            // fire_at false: either pre-attack armed or cannot fire.
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if u.pre_attack_ready_at > current_time + 1e-6 {
                AttackFireResult::Continue
            } else {
                AttackFireResult::Failure
            }
        }
    }

    pub fn attack_can_fire_at(
        &self,
        attacker_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
        require_los: bool,
    ) -> bool {
        let Some(atk) = self.objects.get(&attacker_id) else {
            return false;
        };
        let Some(vic) = self.objects.get(&victim_id) else {
            return false;
        };
        if !atk.is_alive() || !vic.is_alive() {
            return false;
        }
        if !atk.can_fire(current_time) {
            return false;
        }
        if !atk.has_max_shots_remaining() {
            return false;
        }
        // This read-only query cannot begin an unpack transition, but it must
        // still reject a weapon whose exact parsed DeployStyleAIUpdate module
        // is not ReadyToAttack. The mutating fire paths start that transition
        // only after their target/range checks.
        let source_has_deploy_style = atk.get_template().deploy_style_metadata.is_some();
        let runtime_deploy_ready = atk
            .deploy_style
            .as_ref()
            .is_some_and(|deploy| deploy.is_ready_to_attack());
        if (source_has_deploy_style && !runtime_deploy_ready)
            || (!source_has_deploy_style && atk.deploy_style.is_some())
        {
            return false;
        }
        let Some(slot) = atk.selected_weapon_slot() else {
            return false;
        };
        if !atk.is_within_attack_range_for_slot(slot, vic) {
            return false;
        }
        if require_los {
            let from = atk.get_position();
            let to = vic.get_position();
            if self.pathfinding_system.is_attack_view_blocked(from, to) {
                return false;
            }
            let eye_a = atk.selection_radius.max(5.0) * 0.5;
            let eye_b = vic.selection_radius.max(5.0) * 0.5;
            let a = glam::Vec3::new(from.x, from.y + eye_a, from.z);
            let b = glam::Vec3::new(to.x, to.y + eye_b, to.z);
            if !self.is_clear_line_of_sight_terrain(a, b) {
                return false;
            }
        }
        true
    }

    /// C++ AIUpdateInterface::privateMoveToPosition residual.
    pub fn private_move_to_position(&mut self, unit_id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if !u.can_move() || !u.is_alive() {
            return false;
        }
        let was_idle = matches!(u.ai_state, AIState::Idle);
        // Clear blocked residual on new move order.
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.is_blocked = false;
            u.is_blocked_and_stuck = false;
            u.num_frames_blocked = 0;
            if !was_idle {
                // C++ temporary AI_MOVE_TO for 20 seconds when non-idle AI command.
                u.temporary_move_frames = 20 * 30;
            }
        }
        self.request_object_path(unit_id, pos)
    }

    /// C++ AIUpdateInterface::privateStop residual.
    pub fn private_stop(&mut self, unit_id: ObjectId) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.stop_moving();
        u.set_status_attacking(false);
        // Always clear host engagement same-frame (player Stop / privateStop residual).
        // Decision authority still logs so GameWorld can last-write idle/stop.
        u.target = None;
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
        }
        true
    }

    /// C++ AIAttackApproachTargetState::computePath residual.
    ///
    /// Returns true if approach path is valid/in-progress (stay in approach).
    /// Returns false if should leave approach (no weapon / should pursue).
    pub fn attack_approach_compute_path(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        fixed_pos: Option<glam::Vec3>,
    ) -> bool {
        let frame = self.frame;
        let (mobile, stuck, waiting, path_empty, approach_ts, prev_vic, has_weapon, from) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.can_move() {
                return false;
            }
            (
                true,
                u.is_blocked_and_stuck,
                u.waiting_for_path,
                u.movement.path.is_empty(),
                u.approach_timestamp,
                u.prev_victim_pos,
                u.weapon.is_some() || u.secondary_weapon.is_some() || u.tertiary_weapon.is_some(),
                u.get_position(),
            )
        };
        let _ = mobile;

        if waiting {
            return true;
        }

        let mut force_repath = stuck;
        if !force_repath && path_empty && !waiting {
            force_repath = true;
        }
        if !force_repath
            && frame.saturating_sub(approach_ts) < crate::game_logic::MIN_RECOMPUTE_TIME_RESIDUAL
        {
            return true;
        }

        if let Some(vid) = victim_id {
            let Some(victim) = self.objects.get(&vid) else {
                return false;
            };
            if !victim.is_alive() {
                return false;
            }
            if !has_weapon {
                return false;
            }
            let vic_pos = victim.get_position();
            // Center position residual (geometry center ≈ position for host).
            let center = vic_pos;
            if !force_repath {
                if let Some(prev) = prev_vic {
                    if crate::game_logic::is_same_position_residual(from, prev, center) {
                        return true;
                    }
                }
            }
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.prev_victim_pos = Some(center);
                u.approach_timestamp = frame;
            }
            // Contact weapon: ignore obstacle + path into target residual handled by assign.
            let _ = self.request_attack_path(unit_id, Some(vid), center);
            true
        } else if let Some(pos) = fixed_pos {
            if !force_repath {
                return true; // fixed positions don't move
            }
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.approach_timestamp = frame;
            }
            let _ = self.request_attack_path(unit_id, None, pos);
            true
        } else {
            false
        }
    }

    /// C++ AIUpdateInterface::requestAttackPath residual.
    ///
    /// Rate-limits repath (<3 frames → queue 2s). On accept, runs findAttackPath.
    pub fn request_attack_path(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        victim_pos: glam::Vec3,
    ) -> bool {
        let frame = self.frame;
        let can_compute = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.can_move() || !u.is_alive() {
                return false;
            }
            u.begin_request_attack_path(victim_id, victim_pos, frame)
        };
        if !can_compute {
            return false; // deferred
        }
        let ok = self.assign_unit_attack_path(unit_id, victim_id, victim_pos);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.waiting_for_path = false;
            if !ok {
                u.is_attack_path = false;
            }
        }
        ok
    }

    /// C++ AIUpdateInterface::privateAttackObject residual.
    pub fn private_attack_object(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_shots: i32,
    ) -> bool {
        let victim_pos = match self.objects.get(&victim_id) {
            Some(v) if v.is_alive() => v.get_position(),
            _ => return false,
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_max_shots_to_fire(max_shots);
        } else {
            return false;
        }
        // C++ AIAttackState::onEnter residual via nested AttackStateMachine.
        if self.attack_state_enter(unit_id, victim_id) == AttackMachineResult::Failure {
            return false;
        }
        // Prefer attack path if out of range / LOS blocked residual handled by assign.
        let _ = self.request_attack_path(unit_id, Some(victim_id), victim_pos);
        true
    }

    #[cfg(test)]
    pub fn private_attack_object_for_test(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_shots: i32,
    ) -> bool {
        self.private_attack_object(unit_id, victim_id, max_shots)
    }

    /// C++ AIUpdateInterface::requestPath residual.
    ///
    /// Uses host PathfindingSystem A* when available; fail-closed straight path.
    pub fn request_object_path(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        if obj.status.destroyed || !obj.is_alive() {
            return false;
        }
        let start = obj.get_position();
        // Snapshot objects for pathfinder dynamic obstacles.
        let waypoints = self
            .pathfinding_system
            .find_path(start, destination, &self.objects)
            .filter(|p| p.len() >= 2)
            .unwrap_or_else(|| vec![destination]);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.request_path(destination, Some(waypoints));
            true
        } else {
            false
        }
    }
}
