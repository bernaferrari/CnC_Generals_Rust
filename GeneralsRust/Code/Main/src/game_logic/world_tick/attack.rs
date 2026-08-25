//! Host tick `impl GameLogic` — `attack`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ TurretAIAimTurretState REL_THRESH (~2°).
const TURRET_REL_THRESH_RAD: f32 = 0.035;
/// C++ TurretAI::updateTurretAI ENABLE_SWEEP_FRAME_COUNT.
const ENABLE_SWEEP_FRAME_COUNT: u32 = 3;
/// C++ TurretAIData default Min/MaxIdleScanInterval.
const DEFAULT_IDLE_SCAN_INTERVAL: u32 = 9_999_999;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HostTurretTargetKind {
    #[default]
    None,
    Object,
    Position,
}

#[derive(Clone)]
struct HostTurretExtra {
    kind: HostTurretTargetKind,
    target_pos: Option<glam::Vec3>,
    victim_team: Option<Team>,
    enable_sweep_until: u32,
    positive_sweep: bool,
    fire_angle_sweep: [f32; 3],
    sweep_speed_mod: [f32; 3],
    allows_pitch: bool,
    fire_pitch: f32,
    min_pitch: f32,
    ground_unit_pitch: f32,
    pitch_rate: f32,
    min_idle_scan_angle: f32,
    max_idle_scan_angle: f32,
    min_idle_scan_interval: u32,
    max_idle_scan_interval: u32,
    play_rot_sound: bool,
    play_pitch_sound: bool,
    did_fire: bool,
    move_loop_playing: bool,
    idle_scan_desired_rad: f32,
    idle_scan_entered: bool,
    targeter_adds: u32,
    preventing_aim: bool,
    fires_while_turning: bool,
    sweep_sound_until: u32,
    seeded: bool,
}

impl Default for HostTurretExtra {
    fn default() -> Self {
        Self {
            kind: HostTurretTargetKind::None,
            target_pos: None,
            victim_team: None,
            enable_sweep_until: 0,
            positive_sweep: true,
            fire_angle_sweep: [0.0; 3],
            sweep_speed_mod: [1.0; 3],
            allows_pitch: false,
            fire_pitch: 0.0,
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            pitch_rate: crate::game_logic::object::default_turret_turn_rate(),
            min_idle_scan_angle: 0.0,
            max_idle_scan_angle: 0.0,
            min_idle_scan_interval: DEFAULT_IDLE_SCAN_INTERVAL,
            max_idle_scan_interval: DEFAULT_IDLE_SCAN_INTERVAL,
            play_rot_sound: false,
            play_pitch_sound: false,
            did_fire: false,
            move_loop_playing: false,
            idle_scan_desired_rad: 0.0,
            idle_scan_entered: false,
            targeter_adds: 0,
            preventing_aim: false,
            fires_while_turning: false,
            sweep_sound_until: 0,
            seeded: false,
        }
    }
}

thread_local! {
    static HOST_TURRET_EXTRA: std::cell::RefCell<std::collections::HashMap<u32, HostTurretExtra>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn with_host_turret_extra<R>(id: ObjectId, f: impl FnOnce(&mut HostTurretExtra) -> R) -> R {
    HOST_TURRET_EXTRA.with(|m| {
        let mut map = m.borrow_mut();
        f(map.entry(id.0).or_default())
    })
}

fn seed_host_turret_extra(id: ObjectId, template_name: &str, turn_rate: f32) {
    with_host_turret_extra(id, |e| {
        if e.seeded {
            return;
        }
        e.seeded = true;
        let spec = crate::game_logic::object::turret_spawn_for_template(template_name);
        if spec.has_turret {
            e.pitch_rate = spec.pitch_rate_rad.max(turn_rate);
            e.allows_pitch = spec.allows_pitch;
            e.fire_pitch = spec.fire_pitch_rad;
            e.min_pitch = spec.min_pitch_rad;
            e.ground_unit_pitch = spec.ground_unit_pitch_rad;
            e.min_idle_scan_angle = spec.min_idle_scan_angle_rad;
            e.max_idle_scan_angle = spec.max_idle_scan_angle_rad;
            e.min_idle_scan_interval = spec.min_idle_scan_interval;
            e.max_idle_scan_interval = spec.max_idle_scan_interval;
            e.fire_angle_sweep = spec.fire_angle_sweep;
            e.sweep_speed_mod = spec.sweep_speed_mod;
            e.fires_while_turning = spec.fires_while_turning;
        } else {
            e.pitch_rate = turn_rate.max(crate::game_logic::object::default_turret_turn_rate());
        }
    });
}

fn host_template_is_bridge(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("bridge") && !n.contains("tower") && !n.contains("scaffold")
}

fn nearer_bridge_attack_point(from: glam::Vec3, victim_pos: glam::Vec3, span: f32) -> glam::Vec3 {
    let half = span.max(20.0);
    let a = glam::Vec3::new(victim_pos.x - half, victim_pos.y, victim_pos.z);
    let b = glam::Vec3::new(victim_pos.x + half, victim_pos.y, victim_pos.z);
    if from.distance_squared(a) <= from.distance_squared(b) {
        a
    } else {
        b
    }
}

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

    /// C++ wantToSquishTarget state condition (AIStates.cpp:1140-1166).
    ///
    /// Computer crush/squish chase only: turreted weapon, not DONT_AUTO_CRUSH,
    /// and canCrushOrSquish (ALLIES / unmanned / levels).
    pub fn want_to_squish_target(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        use gamelogic::common::Relationship;
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if victim.contained_by.is_some() {
            return false;
        }
        // C++ getWhichTurretForCurWeapon() != TURRET_INVALID.
        if !obj.turret_enabled {
            return false;
        }
        // C++ PLAYER_COMPUTER only — human tanks do not auto-chase-squish.
        let is_computer = obj
            .owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|p| !p.is_local)
            .unwrap_or(false);
        if !is_computer {
            return false;
        }
        // C++ KINDOF_DONT_AUTO_CRUSH_INFANTRY (Tomahawk, dozers).
        if obj.is_kind_of(crate::game_logic::KindOf::Dozer)
            || obj.template_name.to_ascii_uppercase().contains("TOMAHAWK")
        {
            return false;
        }
        let is_ally = self.object_relationship(obj, victim) == Relationship::Allies;
        obj.can_crush_or_squish(victim, is_ally)
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
                // C++ AIM conditions: outOfWeaponRangeObject OR wantToSquishTarget → CHASE.
                if out_of_wr || self.want_to_squish_target(unit_id, victim_id) {
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
                // C++ FIRE conditions: outOfWeaponRangeObject OR wantToSquishTarget → CHASE.
                if out_of_wr || self.want_to_squish_target(unit_id, victim_id) {
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
                let (victim_spd, can_crush) = {
                    use gamelogic::common::Relationship;
                    let Some(victim) = self.objects.get(&victim_id) else {
                        return AttackMachineResult::Success;
                    };
                    let Some(attacker) = self.objects.get(&unit_id) else {
                        return AttackMachineResult::Failure;
                    };
                    let is_ally =
                        self.object_relationship(attacker, victim) == Relationship::Allies;
                    (
                        victim.forward_speed_2d().abs(),
                        attacker.can_crush_or_squish(victim, is_ally),
                    )
                };
                if in_range && !can_crush {
                    // Match speeds residual while goal still in range (victim * 0.95).
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
                // C++ AIStates.cpp:3058-3060: canCrushOrSquish → FAST_AS_POSSIBLE
                // so tanks run over fleeing infantry instead of pacing at victim*0.95.
                if can_crush {
                    if let Some(attacker) = self.objects.get_mut(&unit_id) {
                        attacker.group_speed_factor = 1.0;
                        attacker.bump_speed_limit = f32::MAX;
                    }
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

    /// C++ `TurretAI::removeSelfAsTargeter` (TurretAI.cpp:528-543).
    fn remove_self_as_jet_targeter(&mut self, unit_id: ObjectId) {
        let Some(prev) = self.objects.get(&unit_id).and_then(|u| u.turret_target_id) else {
            return;
        };
        if let Some(tgt) = self.objects.get_mut(&prev) {
            tgt.add_jet_targeter(unit_id, false, self.frame);
        }
    }

    /// C++ `AIUpdateInterface::setCurrentVictim(NULL)` (AIUpdate.cpp:4169-4186).
    pub(in super::super) fn remove_self_as_jet_targeter_from_current_victim(
        &mut self,
        unit_id: ObjectId,
    ) {
        let Some(prev) = self.objects.get(&unit_id).and_then(|u| u.target) else {
            return;
        };
        if let Some(tgt) = self.objects.get_mut(&prev) {
            tgt.add_jet_targeter(unit_id, false, self.frame);
        }
    }

    /// C++ attack-state exit: `setCurrentVictim(NULL)` + `setTurretTargetObject(NULL)`.
    pub(in super::super) fn drop_jet_targeters_on_attack_exit(&mut self, unit_id: ObjectId) {
        self.remove_self_as_jet_targeter(unit_id);
        self.remove_self_as_jet_targeter_from_current_victim(unit_id);
    }

    /// C++ AIUpdateInterface::setTurretTargetObject residual.
    pub fn set_turret_target_object(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        force_attacking: bool,
    ) {
        // C++ setTurretTargetObject: nuke victim → removeSelfAsTargeter first.
        let clearing = match victim_id {
            None => true,
            Some(vid) => self
                .objects
                .get(&vid)
                .map(|v| !(v.is_alive() && !v.status.destroyed))
                .unwrap_or(true),
        };
        if clearing {
            self.remove_self_as_jet_targeter(unit_id);
        }

        if let Some(vid) = victim_id {
            let (alive, team) = self
                .objects
                .get(&vid)
                .map(|v| (v.is_alive() && !v.status.destroyed, Some(v.team)))
                .unwrap_or((false, None));
            if !alive {
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.set_turret_target_object(None, false);
                    if u.turret_substate == crate::game_logic::object::TurretSubState::Hold {
                        u.turret_hold_until_frame =
                            self.frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                    }
                }
                with_host_turret_extra(unit_id, |e| {
                    e.kind = HostTurretTargetKind::None;
                    e.target_pos = None;
                    e.victim_team = None;
                });
                return;
            }
            with_host_turret_extra(unit_id, |e| {
                e.kind = HostTurretTargetKind::Object;
                e.target_pos = None;
                e.victim_team = team;
            });
        } else {
            with_host_turret_extra(unit_id, |e| {
                e.kind = HostTurretTargetKind::None;
                e.target_pos = None;
                e.victim_team = None;
            });
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

    /// C++ TurretAI::setTurretTargetPosition (TurretAI.cpp:589-626).
    pub fn set_turret_target_position(&mut self, unit_id: ObjectId, pos: Option<glam::Vec3>) {
        let enabled = self.objects.get(&unit_id).is_some_and(|u| u.turret_enabled);
        if !enabled {
            return;
        }
        // C++ always removeSelfAsTargeter before retargeting a position.
        self.remove_self_as_jet_targeter(unit_id);
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return;
        };
        u.turret_target_id = None;
        u.turret_force_attacking = false;
        u.turret_mood_target = false;
        match pos {
            Some(p) => {
                if !matches!(
                    u.turret_substate,
                    crate::game_logic::object::TurretSubState::Aim
                        | crate::game_logic::object::TurretSubState::Fire
                ) {
                    u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
                }
                with_host_turret_extra(unit_id, |e| {
                    e.kind = HostTurretTargetKind::Position;
                    e.target_pos = Some(p);
                    e.victim_team = None;
                });
            }
            None => {
                if matches!(
                    u.turret_substate,
                    crate::game_logic::object::TurretSubState::Aim
                        | crate::game_logic::object::TurretSubState::Fire
                ) {
                    u.turret_substate = crate::game_logic::object::TurretSubState::Hold;
                    u.turret_hold_until_frame =
                        self.frame.saturating_add(u.turret_recenter_frames.max(1));
                    u.turret_holding = true;
                }
                with_host_turret_extra(unit_id, |e| {
                    e.kind = HostTurretTargetKind::None;
                    e.target_pos = None;
                    e.victim_team = None;
                });
            }
        }
        u.record_host_turret();
    }

    /// C++ TurretAI::notifyFired → 3-frame sweep window (TurretAI.cpp:697-702).
    pub fn notify_turret_fired(&mut self, unit_id: ObjectId) {
        let now = self.frame;
        with_host_turret_extra(unit_id, |e| {
            e.did_fire = true;
            e.enable_sweep_until = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
            e.sweep_sound_until = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
        });
    }

    /// C++ TurretAIData TurretFireAngleSweep / TurretSweepSpeedModifier.
    pub fn set_turret_fire_angle_sweep(&mut self, unit_id: ObjectId, slot: u8, sweep_rad: f32) {
        with_host_turret_extra(unit_id, |e| {
            e.seeded = true;
            let i = (slot as usize).min(2);
            e.fire_angle_sweep[i] = sweep_rad.max(0.0);
        });
    }

    /// C++ TurretAIData AllowsPitch / FirePitch / MinPhysicalPitch / GroundUnitPitch.
    pub fn set_turret_pitch_params(
        &mut self,
        unit_id: ObjectId,
        allows_pitch: bool,
        fire_pitch_rad: f32,
        min_pitch_rad: f32,
        ground_unit_pitch_rad: f32,
        pitch_rate_rad: f32,
    ) {
        with_host_turret_extra(unit_id, |e| {
            e.seeded = true;
            e.allows_pitch = allows_pitch;
            e.fire_pitch = fire_pitch_rad;
            e.min_pitch = min_pitch_rad;
            e.ground_unit_pitch = ground_unit_pitch_rad;
            e.pitch_rate = pitch_rate_rad.max(0.0);
        });
    }

    /// C++ TurretAIData Min/MaxIdleScanAngle + Interval.
    pub fn set_turret_idle_scan_params(
        &mut self,
        unit_id: ObjectId,
        min_angle_rad: f32,
        max_angle_rad: f32,
        min_interval: u32,
        max_interval: u32,
    ) {
        with_host_turret_extra(unit_id, |e| {
            e.seeded = true;
            e.min_idle_scan_angle = min_angle_rad.max(0.0);
            e.max_idle_scan_angle = max_angle_rad.max(e.min_idle_scan_angle);
            e.min_idle_scan_interval = min_interval;
            e.max_idle_scan_interval = max_interval.max(min_interval);
        });
    }

    /// C++ TurretAI::friend_checkForIdleMoodTarget (TurretAI.cpp:855-876).
    /// Leftover `turret.rs` friend_check_for_idle_mood_target: PreferMostDamage FromAi.
    fn turret_check_for_idle_mood_target(&mut self, unit_id: ObjectId, current_time: f32) {
        use mood_action_adjust::AFFECT_RANGE_IGNORE_ALL;
        let adj = self.get_mood_matrix_action_adjustment(unit_id, MoodMatrixAction::Idle, false);
        if adj & AFFECT_RANGE_IGNORE_ALL != 0 {
            return;
        }
        let Some(enemy) = self.get_next_mood_target(unit_id, true, true, false) else {
            return;
        };
        // Leftover: choose_best_weapon_for_target PreferMostDamage FromAi, then
        // set idle-mood turret target. AIM/FIRE read selected_weapon_slot.
        let _ = self.choose_best_weapon_for_target(unit_id, Some(enemy), current_time);
        self.set_turret_target_object(unit_id, Some(enemy), false);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.turret_mood_target = true;
        }
    }

    fn apply_turret_rotate_fx(&mut self, unit_id: ObjectId, prev_angle_deg: f32) {
        let (rotating, angle_deg, pitch_sound, pos, occupants, keep_loop, template_name) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return;
            };
            let now = self.frame;
            let keep_loop = with_host_turret_extra(unit_id, |e| {
                e.fires_while_turning && e.sweep_sound_until != 0 && now < e.sweep_sound_until
            });
            (
                u.turret_rotating,
                u.turret_angle_deg,
                with_host_turret_extra(unit_id, |e| e.play_pitch_sound),
                u.get_position(),
                u.contained_units(),
                keep_loop,
                u.template_name.clone(),
            )
        };
        with_host_turret_extra(unit_id, |e| {
            e.play_rot_sound = rotating || keep_loop;
        });
        if let Some(u) = self.objects.get_mut(&unit_id) {
            let bit = crate::game_logic::host_enum_table_residual::turret_rotate_model_bit();
            let before = u.model_condition_bits;
            if rotating {
                u.model_condition_bits |= 1u128 << bit;
            } else {
                u.model_condition_bits &= !(1u128 << bit);
            }
            if u.model_condition_bits != before {
                u.record_host_model_condition();
            }
        }
        // C++ TurretAI ctor: getPerUnitSound("TurretMoveLoop") — INI value, not slot key.
        let authored = crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
            &template_name,
            "TurretMoveLoop",
        );
        let want_play = rotating || pitch_sound || keep_loop;
        let play = want_play && authored.is_some();
        let (started, stopped) = with_host_turret_extra(unit_id, |e| {
            if play && !e.move_loop_playing {
                e.move_loop_playing = true;
                (true, false)
            } else if !play && e.move_loop_playing {
                e.move_loop_playing = false;
                (false, true)
            } else {
                if !play {
                    e.move_loop_playing = false;
                }
                (false, false)
            }
        });
        if let Some(name) = authored {
            if started {
                self.queue_audio_event(
                    AudioEventRequest::new(&name)
                        .with_object(unit_id)
                        .with_position(pos)
                        .looping(),
                );
            } else if stopped {
                self.queue_audio_event(
                    AudioEventRequest::new(&name)
                        .with_object(unit_id)
                        .with_position(pos)
                        .stopping(),
                );
            }
        }
        // C++ Object::reactToTurretChange → Contain::containReactToTransformChange.
        if (angle_deg - prev_angle_deg).abs() > 0.01 {
            for pid in occupants {
                if let Some(p) = self.objects.get_mut(&pid) {
                    p.set_position(pos);
                }
            }
        }
    }

    /// C++ TurretAI state machine residual (AIM/FIRE/HOLD/RECENTER/IDLE/IDLESCAN).
    pub fn tick_turret_state_machine(
        &mut self,
        unit_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
    ) -> AttackAimResult {
        use crate::game_logic::object::TurretSubState;

        let (enabled, alive, sub, tid, under_construction, tmpl, turn_rate) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.is_alive() {
                return AttackAimResult::Failure;
            }
            (
                u.turret_enabled,
                true,
                u.turret_substate,
                u.turret_target_id,
                u.status.under_construction,
                u.template_name.clone(),
                u.turret_turn_rate_rad,
            )
        };
        let _ = alive;
        seed_host_turret_extra(unit_id, &tmpl, turn_rate);
        // C++ updateTurretAI: run when enabled OR current state is RECENTER.
        if !enabled && sub != TurretSubState::Recenter {
            return AttackAimResult::Failure;
        }
        with_host_turret_extra(unit_id, |e| {
            e.did_fire = false;
            if e.enable_sweep_until != 0 && logic_frame >= e.enable_sweep_until {
                // window expired; keep stored until next notifyFired
            }
        });
        let pos_kind = with_host_turret_extra(unit_id, |e| e.kind);
        let prev_angle = self
            .objects
            .get(&unit_id)
            .map(|u| u.turret_angle_deg)
            .unwrap_or(0.0);

        let result = match sub {
            TurretSubState::Idle => {
                let strategy_center =
                    crate::game_logic::host_strategy_center::is_strategy_center_template(&tmpl);
                // Strategy Center idle-scan is owned by
                // `tick_strategy_center_turret_idle_scan` (Bombardment ACTIVE).
                // Do not double-step the generic SM.
                if !strategy_center {
                    let (min_iv, max_iv, scan_authored) = with_host_turret_extra(unit_id, |e| {
                        (
                            e.min_idle_scan_interval,
                            e.max_idle_scan_interval,
                            e.min_idle_scan_angle != 0.0 || e.max_idle_scan_angle != 0.0,
                        )
                    });
                    if scan_authored {
                        {
                            let Some(u) = self.objects.get_mut(&unit_id) else {
                                return AttackAimResult::Failure;
                            };
                            if u.turret_idle_scan_next_frame == 0 {
                                // Leftover TurretAIIdleState::reset_idle_scan:
                                // GameLogicRandomValue(min, max), not index mix.
                                let max_iv = if max_iv < min_iv { min_iv } else { max_iv };
                                let interval = gamelogic::helpers::get_game_logic_random_value(
                                    min_iv as i32,
                                    max_iv as i32,
                                ) as u32;
                                u.turret_idle_scan_next_frame =
                                    logic_frame.saturating_add(interval);
                            }
                        }
                    }
                }
                self.turret_check_for_idle_mood_target(unit_id, current_time);
                if self
                    .objects
                    .get(&unit_id)
                    .map(|u| u.turret_substate == TurretSubState::Aim)
                    .unwrap_or(false)
                {
                    return AttackAimResult::Continue;
                }
                let due = !strategy_center
                    && self
                        .objects
                        .get(&unit_id)
                        .map(|u| {
                            u.turret_idle_scan_next_frame != 0
                                && logic_frame >= u.turret_idle_scan_next_frame
                        })
                        .unwrap_or(false);
                if due {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::IdleScan;
                        u.turret_idle_scanning = true;
                        u.turret_idle_scan_next_frame = 0;
                    }
                    with_host_turret_extra(unit_id, |e| {
                        e.idle_scan_entered = false;
                    });
                }
                AttackAimResult::Continue
            }
            TurretSubState::IdleScan => {
                if under_construction {
                    return AttackAimResult::Continue;
                }
                let (min_a, max_a, desired) = with_host_turret_extra(unit_id, |e| {
                    if !e.idle_scan_entered {
                        e.idle_scan_entered = true;
                        if e.min_idle_scan_angle == 0.0 && e.max_idle_scan_angle == 0.0 {
                            e.idle_scan_desired_rad = 0.0;
                            return (0.0, 0.0, None);
                        }
                        // Leftover TurretAIIdleScanState::classic_on_enter:
                        // GameLogicRandomValueReal(0, max-min) + GameLogicRandomValue(0,1) sign.
                        let span = (e.max_idle_scan_angle - e.min_idle_scan_angle).max(0.0);
                        let mut off = e.min_idle_scan_angle
                            + gamelogic::helpers::get_game_logic_random_value_real(0.0, span);
                        if gamelogic::helpers::get_game_logic_random_value(0, 1) == 0 {
                            off = -off;
                        }
                        e.idle_scan_desired_rad = off;
                    }
                    (
                        e.min_idle_scan_angle,
                        e.max_idle_scan_angle,
                        Some(e.idle_scan_desired_rad),
                    )
                });
                if min_a == 0.0 && max_a == 0.0 {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Hold;
                        u.turret_idle_scanning = false;
                        u.turret_holding = true;
                        u.turret_hold_until_frame =
                            logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_idle_scan_index = u.turret_idle_scan_index.saturating_add(1);
                    }
                    return AttackAimResult::Continue;
                }
                let Some(off) = desired else {
                    return AttackAimResult::Continue;
                };
                let (ang_ok, pitch_ok) = {
                    let Some(u) = self.objects.get_mut(&unit_id) else {
                        return AttackAimResult::Failure;
                    };
                    u.turret_idle_scan_desired_angle_deg =
                        (u.turret_natural_angle_deg.to_radians() + off).to_degrees();
                    let nat_a = u.turret_natural_angle_deg.to_radians() + off;
                    let nat_p = u.turret_natural_pitch_deg.to_radians();
                    let a = u.turn_turret_towards_angle_rad(nat_a, 0.5, 0.0);
                    let p = u.turn_turret_towards_pitch_rad(nat_p, 0.5);
                    (a, p)
                };
                if ang_ok && pitch_ok {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Hold;
                        u.turret_idle_scanning = false;
                        u.turret_holding = true;
                        u.turret_hold_until_frame =
                            logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_idle_scan_index = u.turret_idle_scan_index.saturating_add(1);
                    }
                }
                AttackAimResult::Continue
            }
            TurretSubState::Aim => {
                let has_pos = pos_kind == HostTurretTargetKind::Position
                    && with_host_turret_extra(unit_id, |e| e.target_pos.is_some());
                if tid.is_none() && !has_pos {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Hold;
                        u.turret_hold_until_frame =
                            logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                        u.record_host_turret();
                    }
                    return AttackAimResult::Continue;
                }
                if let Some(vid) = tid {
                    let alive = self
                        .objects
                        .get(&vid)
                        .map(|v| v.is_alive() && !v.status.destroyed)
                        .unwrap_or(false);
                    if !alive {
                        self.set_turret_target_object(unit_id, None, false);
                        return AttackAimResult::Continue;
                    }
                    let team_now = self.objects.get(&vid).map(|v| v.team);
                    let team0 = with_host_turret_extra(unit_id, |e| e.victim_team);
                    if team0.is_some() && team_now != team0 {
                        if self
                            .objects
                            .get(&unit_id)
                            .map(|u| u.turret_mood_target)
                            .unwrap_or(false)
                        {
                            self.set_turret_target_object(unit_id, None, false);
                        }
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Hold;
                            u.turret_hold_until_frame =
                                logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                            u.turret_holding = true;
                        }
                        return AttackAimResult::Continue;
                    }
                    if self.cannot_possibly_attack_object(
                        unit_id,
                        vid,
                        self.objects
                            .get(&unit_id)
                            .map(|u| u.turret_force_attacking)
                            .unwrap_or(false),
                    ) {
                        if self
                            .objects
                            .get(&unit_id)
                            .map(|u| u.turret_mood_target)
                            .unwrap_or(false)
                        {
                            self.set_turret_target_object(unit_id, None, false);
                        }
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Hold;
                            u.turret_hold_until_frame =
                                logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                            u.turret_holding = true;
                        }
                        return AttackAimResult::Continue;
                    }
                    with_host_turret_extra(unit_id, |e| {
                        e.targeter_adds = e.targeter_adds.saturating_add(1);
                    });
                }
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
                let has_pos = pos_kind == HostTurretTargetKind::Position
                    && with_host_turret_extra(unit_id, |e| e.target_pos.is_some());
                if tid.is_none() && !has_pos {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    return AttackAimResult::Continue;
                }
                if let Some(vid) = tid {
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
                            if matches!(fire, AttackFireResult::Success) {
                                self.notify_turret_fired(unit_id);
                            }
                            self.attack_fire_weapon_exit(unit_id);
                            if let Some(u) = self.objects.get_mut(&unit_id) {
                                u.turret_substate = TurretSubState::Aim;
                            }
                            AttackAimResult::Continue
                        }
                    }
                } else {
                    let pos = with_host_turret_extra(unit_id, |e| e.target_pos);
                    let Some(pos) = pos else {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Aim;
                        }
                        return AttackAimResult::Continue;
                    };
                    let (can, in_range) = {
                        let Some(u) = self.objects.get(&unit_id) else {
                            return AttackAimResult::Failure;
                        };
                        let Some(slot) = u.selected_weapon_slot() else {
                            return AttackAimResult::Failure;
                        };
                        (
                            u.can_fire(current_time),
                            u.is_within_attack_range_pos_for_slot(slot, pos),
                        )
                    };
                    if !in_range {
                        self.attack_fire_weapon_exit(unit_id);
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Aim;
                        }
                        return AttackAimResult::Continue;
                    }
                    if can {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            if let Some(slot) = u.selected_weapon_slot() {
                                if let Some(w) = u.weapon_slot_mut(slot) {
                                    w.last_fire_time = current_time;
                                }
                            }
                        }
                        self.notify_turret_fired(unit_id);
                    }
                    self.attack_fire_weapon_exit(unit_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    AttackAimResult::Continue
                }
            }
            TurretSubState::Hold => {
                self.turret_check_for_idle_mood_target(unit_id, current_time);
                if self
                    .objects
                    .get(&unit_id)
                    .map(|u| u.turret_substate == TurretSubState::Aim)
                    .unwrap_or(false)
                {
                    return AttackAimResult::Continue;
                }
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
                if under_construction {
                    return AttackAimResult::Continue;
                }
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
                        u.turret_idle_scan_next_frame = 0;
                    }
                    AttackAimResult::Success
                } else {
                    AttackAimResult::Continue
                }
            }
        };
        self.apply_turret_rotate_fx(unit_id, prev_angle);
        result
    }

    /// Drive TurretAI SM for all turret-enabled objects (and disabled Recenter).
    pub(crate) fn tick_all_turret_state_machines(
        &mut self,
        object_ids: &[ObjectId],
        current_time: f32,
        logic_frame: u32,
    ) {
        use crate::game_logic::object::TurretSubState;
        for &id in object_ids {
            let should = self
                .objects
                .get(&id)
                .map(|o| {
                    o.is_alive()
                        && (o.turret_enabled || o.turret_substate == TurretSubState::Recenter)
                })
                .unwrap_or(false);
            if should {
                let _ = self.tick_turret_state_machine(id, current_time, logic_frame);
            }
        }
    }

    pub fn tick_turret_aim(
        &mut self,
        unit_id: ObjectId,
        max_rate_modifier: f32,
    ) -> AttackAimResult {
        let (
            victim_pos,
            has_object,
            victim_immobile,
            victim_ground,
            under_construction,
            tmpl,
            turn_rate,
            geom_h,
            unit_pos,
        ) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.turret_enabled {
                return AttackAimResult::Failure;
            }
            seed_host_turret_extra(unit_id, &u.template_name, u.turret_turn_rate_rad);
            let kind = with_host_turret_extra(unit_id, |e| e.kind);
            let pos_tgt = with_host_turret_extra(unit_id, |e| e.target_pos);
            match (u.turret_target_id, kind, pos_tgt) {
                (Some(tid), _, _) => {
                    let Some(v) = self.objects.get(&tid) else {
                        return AttackAimResult::Failure;
                    };
                    if !v.is_alive() || v.status.destroyed {
                        return AttackAimResult::Failure;
                    }
                    let raw = if host_template_is_bridge(&v.template_name)
                        || v.template_name.eq_ignore_ascii_case("Bridge")
                    {
                        nearer_bridge_attack_point(
                            u.get_position(),
                            v.get_position(),
                            v.selection_radius,
                        )
                    } else {
                        v.get_position()
                    };
                    if v.is_temporarily_preventing_aim_success(self.frame) {
                        return AttackAimResult::Continue;
                    }
                    let aim = v.apply_sneaky_targeting_offset(raw, self.frame);

                    (
                        aim,
                        true,
                        v.is_kind_of(KindOf::Immobile) || v.is_kind_of(KindOf::Structure),
                        !v.is_kind_of(KindOf::Aircraft),
                        u.status.under_construction,
                        u.template_name.clone(),
                        u.turret_turn_rate_rad,
                        u.selection_radius.max(10.0),
                        u.get_position(),
                    )
                }
                (None, HostTurretTargetKind::Position, Some(p)) => (
                    p,
                    false,
                    true,
                    true,
                    u.status.under_construction,
                    u.template_name.clone(),
                    u.turret_turn_rate_rad,
                    u.selection_radius.max(10.0),
                    u.get_position(),
                ),
                _ => return AttackAimResult::Failure,
            }
        };
        let _ = (under_construction, tmpl, turn_rate);
        let now = self.frame;
        let (
            sweep,
            sweep_mod,
            sweep_on,
            allows_pitch,
            fire_pitch,
            min_pitch,
            ground_pitch,
            pitch_rate,
            preventing,
        ) = with_host_turret_extra(unit_id, |e| {
            let slot = self
                .objects
                .get(&unit_id)
                .and_then(|u| u.selected_weapon_slot())
                .unwrap_or(0) as usize;
            let i = slot.min(2);
            let sweep = e.fire_angle_sweep[i];
            let sweep_on = sweep > 0.0 && e.enable_sweep_until != 0 && now < e.enable_sweep_until;
            (
                sweep,
                e.sweep_speed_mod[i],
                sweep_on,
                e.allows_pitch,
                e.fire_pitch,
                e.min_pitch,
                e.ground_unit_pitch,
                e.pitch_rate,
                e.preventing_aim,
            )
        });

        let Some(u) = self.objects.get_mut(&unit_id) else {
            return AttackAimResult::Failure;
        };
        u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
        let rel_world = u.relative_angle_2d_to(victim_pos);
        let mut aim_angle = rel_world;
        let mut rate_mod = max_rate_modifier.max(0.01);
        if sweep > 0.0 && sweep_on {
            let pos_sweep = with_host_turret_extra(unit_id, |e| e.positive_sweep);
            if pos_sweep {
                aim_angle += sweep;
            } else {
                aim_angle -= sweep;
            }
            rate_mod *= sweep_mod.max(0.01);
        }
        let mut turn_aligned =
            u.turn_turret_towards_angle_rad(aim_angle, rate_mod, TURRET_REL_THRESH_RAD);
        if sweep > 0.0 {
            if turn_aligned {
                with_host_turret_extra(unit_id, |e| {
                    e.positive_sweep = !e.positive_sweep;
                });
            }
            let turret_rad = u.turret_angle_deg.to_radians();
            let angle_diff = Object::normalize_angle_rad(rel_world - turret_rad);
            turn_aligned = angle_diff.abs() < sweep;
        }

        let mut pitch_aligned = true;
        if allows_pitch {
            let desired = if fire_pitch > 0.0 {
                fire_pitch
            } else {
                let mut v = victim_pos - unit_pos;
                v.y -= geom_h * 0.5;
                let len = v.length();
                let actual = if len > 0.0 { (v.y / len).asin() } else { 0.0 };
                let mut desired = actual.max(min_pitch);
                if ground_pitch > 0.0 && (!has_object || victim_immobile || victim_ground) {
                    let range = u
                        .selected_weapon_slot()
                        .and_then(|s| u.weapon_slot(s).map(|w| w.range))
                        .unwrap_or(1.0)
                        .max(1.0);
                    let dist = v.length();
                    desired = (actual + ground_pitch * (dist / range)).max(min_pitch);
                }
                desired
            };
            let pitch_mod = if u.turret_turn_rate_rad > 1e-6 {
                (pitch_rate / u.turret_turn_rate_rad).max(0.01)
            } else {
                1.0
            };
            pitch_aligned = u.turn_turret_towards_pitch_rad(desired, pitch_mod);
            with_host_turret_extra(unit_id, |e| {
                e.play_pitch_sound = !pitch_aligned;
            });
        }

        let range_ok = {
            let Some(slot) = u.selected_weapon_slot() else {
                return AttackAimResult::Failure;
            };
            u.is_within_attack_range_pos_for_slot(slot, victim_pos)
        };

        if turn_aligned && pitch_aligned && range_ok && !preventing {
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
        let fired_slot = {
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
            u.fire_at_ex_defer_weapon_barrel_advance(
                victim_id,
                current_time,
                victim_infantry,
                victim_faerie,
            )
        };

        if let Some(slot) = fired_slot {
            // The shot above has consumed ammo and queued its projectile, but
            // deliberately has not advanced the concrete WeaponSet cursor.
            // Normalize its exact pre-advance barrel through the owning world
            // so Drawable presentation sees an actual discharge, never an AI
            // intent. A malformed transient object state still advances the
            // logical cursor fail-closed rather than replaying the shot.
            if self
                .record_accepted_weapon_discharge(unit_id, slot)
                .is_none()
            {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.advance_weapon_barrel_after_shot(slot);
                }
            }
            // max_shots_to_fire is decremented inside the accepted Object
            // shot (C++ Weapon::m_maxShotCount).
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
            let vic_r = victim.thing.template.geometry_info.bounding_circle_radius();
            let (min_range, max_range, src_r, is_contact) = self
                .objects
                .get(&unit_id)
                .map(|u| {
                    // C++ computeApproachTarget runs on the firing Weapon after chooseBest.
                    let slot = u.selected_weapon_slot();
                    let w = slot.and_then(|s| u.weapon_slot(s));
                    let (min_r, max_r) = w.map(|w| (w.min_range, w.range)).unwrap_or((0.0, 0.0));
                    let name = slot.and_then(|s| u.weapon_name_for_slot(s));
                    let contact = crate::game_logic::weapon_bootstrap::is_contact_effective_range(
                        max_r,
                    ) || name
                        .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
                        .unwrap_or(false);
                    (
                        min_r,
                        max_r,
                        u.thing.template.geometry_info.bounding_circle_radius(),
                        contact,
                    )
                })
                .unwrap_or((0.0, 0.0, 0.0, false));
            // C++ Weapon::computeApproachTarget: contact → target pos;
            // min-range back-off only when minAttackRange > PATHFIND_CELL_SIZE_F;
            // otherwise 0.9 * max.
            let dx = from.x - vic_pos.x;
            let dz = from.z - vic_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let (dir_x, dir_z) = if dist > 1e-3 {
                (dx / dist, dz / dist)
            } else {
                (1.0, 0.0)
            };
            let cell = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE;
            let standoff = if is_contact {
                0.0
            } else if min_range > cell && dist < min_range {
                (min_range + max_range) * 0.5 + src_r + vic_r
            } else if max_range > 0.0 {
                max_range * 0.9 + src_r + vic_r
            } else {
                0.0
            };
            let mut center = if standoff > 0.0 {
                glam::Vec3::new(
                    vic_pos.x + dir_x * standoff,
                    vic_pos.y,
                    vic_pos.z + dir_z * standoff,
                )
            } else {
                vic_pos
            };
            if !is_contact && max_range > 0.0 {
                center = self.adjust_aircraft_attack_approach(
                    unit_id, center, vic_pos, max_range, min_range,
                );
            }

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
    /// Uses host PathfindingSystem A*. A null path leaves the unit halted
    /// (AIStates.cpp:1771-1778) — never install a straight-line through walls.
    pub fn request_object_path(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        if obj.status.destroyed || !obj.is_alive() {
            return false;
        }
        let start = obj.get_position();
        let is_aircraft = obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft;
        let surfaces = if obj.locomotor_surfaces != 0 {
            obj.locomotor_surfaces
        } else {
            Object::default_locomotor_surfaces_for_template(&obj.thing.template)
        };
        let is_crusher = obj.crusher_level > 0;
        let unit_radius = obj.selection_radius;
        let loco = if is_aircraft {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let mut dest = destination;
        let waypoints = match self.pathfinding_system.find_path_ex_surfaces(
            start,
            dest,
            &self.objects,
            is_aircraft,
            loco,
            is_crusher,
            Some(id),
        ) {
            Some(w) => w,
            None => {
                // C++ AIUpdateInterface::doPathfind: adjustToPossibleDestination
                // then computePath (AIUpdate.cpp:434-438).
                if !self.pathfinding_system.adjust_to_possible_destination(
                    start,
                    &mut dest,
                    loco,
                    is_crusher,
                    unit_radius,
                ) {
                    return false;
                }
                let Some(w) = self.pathfinding_system.find_path_ex_surfaces(
                    start,
                    dest,
                    &self.objects,
                    is_aircraft,
                    loco,
                    is_crusher,
                    Some(id),
                ) else {
                    return false;
                };
                w
            }
        };
        if waypoints.is_empty() {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.request_path(dest, Some(waypoints));
            true
        } else {
            false
        }
    }
}
