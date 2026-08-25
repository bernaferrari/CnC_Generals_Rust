//! Host combat `impl GameLogic` — `heroes_and_plans`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Tick TurretAI idle-scan / HoldTurret / idle-recenter residual for
    /// Bombardment ACTIVE Strategy Centers.
    ///
    /// C++ TurretAI state machine residual:
    /// IDLE → IDLESCAN → HOLD → RECENTER → IDLE.
    ///
    /// Host residual:
    /// - After Min/MaxIdleScanInterval, rotate toward NaturalTurretAngle ± offset
    ///   (MaxIdleScanAngle **60**); pitch holds NaturalTurretPitch.
    /// - On scan complete: HoldTurret for RecenterTime (**60** frames default).
    /// - After hold: idle-recenter to natural angles, then schedule next scan.
    /// - Busy gun (attacking / target / pack recenter) cancels mid residual.
    pub(in super::super) fn tick_strategy_center_turret_idle_scan(&mut self) {
        use crate::game_logic::host_strategy_center::{
            HostBattlePlan, HostBattlePlanTransition, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
            hold_turret_elapsed, hold_turret_until_frame, idle_scan_desired_angle_deg,
            idle_scan_interval_frames, is_strategy_center_template, step_turret_toward_angles,
            step_turret_toward_natural, turret_angles_are_natural, turret_angles_at,
        };

        let frame = self.frame;
        // Bombardment ACTIVE centers (door residual WaitingToClose / Active).
        let centers: Vec<ObjectId> = self
            .battle_plans
            .door_states()
            .iter()
            .filter(|s| {
                s.status == HostBattlePlanTransition::Active
                    && s.door_plan == Some(HostBattlePlan::Bombardment)
                    && !s.centering_turret
            })
            .map(|s| s.center_id)
            .collect();

        let mut scan_started = 0u32;
        let mut scan_completed = 0u32;
        let mut hold_started = 0u32;
        let mut hold_completed = 0u32;
        let mut idle_recenter_started = 0u32;
        let mut idle_recenter_completed = 0u32;
        for cid in centers {
            let Some(obj) = self.objects.get(&cid) else {
                continue;
            };
            if !obj.is_alive() || !is_strategy_center_template(&obj.template_name) {
                continue;
            }
            if obj.weapon.is_none() {
                continue;
            }
            let busy = obj.status.attacking
                || obj.target.is_some()
                || matches!(
                    obj.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                );
            // Snapshot residual state.
            let mut scanning = obj.turret_idle_scanning;
            let mut holding = obj.turret_holding;
            let mut idle_recentering = obj.turret_idle_recentering;
            let mut hold_until = obj.turret_hold_until_frame;
            let mut desired = obj.turret_idle_scan_desired_angle_deg;
            let mut next_frame = obj.turret_idle_scan_next_frame;
            let mut scan_index = obj.turret_idle_scan_index;
            let mut angle = obj.turret_angle_deg;
            let mut pitch = obj.turret_pitch_deg;

            if busy {
                // Busy residual: cancel mid-scan / hold / idle-recenter.
                // Keep next_frame so scan can resume after coast when set by fire path.
                scanning = false;
                holding = false;
                idle_recentering = false;
                hold_until = 0;
            } else if idle_recentering {
                // HOLD → RECENTER residual: step toward natural pitch/yaw.
                let (a, p) = step_turret_toward_natural(angle, pitch);
                angle = a;
                pitch = p;
                if turret_angles_are_natural(angle, pitch) {
                    idle_recentering = false;
                    // Back to IDLE: schedule next idle-scan residual.
                    next_frame = frame.saturating_add(idle_scan_interval_frames(scan_index));
                    idle_recenter_completed = idle_recenter_completed.saturating_add(1);
                }
            } else if holding {
                // HoldTurret residual: freeze angles until RecenterTime elapses.
                if hold_turret_elapsed(frame, hold_until) {
                    holding = false;
                    hold_until = 0;
                    idle_recentering = true;
                    hold_completed = hold_completed.saturating_add(1);
                    idle_recenter_started = idle_recenter_started.saturating_add(1);
                    // First recenter step this frame.
                    let (a, p) = step_turret_toward_natural(angle, pitch);
                    angle = a;
                    pitch = p;
                    if turret_angles_are_natural(angle, pitch) {
                        idle_recentering = false;
                        next_frame = frame.saturating_add(idle_scan_interval_frames(scan_index));
                        idle_recenter_completed = idle_recenter_completed.saturating_add(1);
                    }
                }
            } else if scanning {
                let (a, p) = step_turret_toward_angles(
                    angle,
                    pitch,
                    desired,
                    STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
                );
                angle = a;
                pitch = p;
                if turret_angles_at(
                    angle,
                    pitch,
                    desired,
                    STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
                ) {
                    // Scan complete residual → HoldTurret (do not reschedule yet).
                    scanning = false;
                    scan_index = scan_index.saturating_add(1);
                    holding = true;
                    hold_until = hold_turret_until_frame(frame);
                    next_frame = 0;
                    scan_completed = scan_completed.saturating_add(1);
                    hold_started = hold_started.saturating_add(1);
                }
            } else if next_frame > 0 && frame >= next_frame {
                // Start idle-scan residual toward natural + offset.
                desired = idle_scan_desired_angle_deg(scan_index);
                scanning = true;
                scan_started = scan_started.saturating_add(1);
                // First step this frame.
                let (a, p) = step_turret_toward_angles(
                    angle,
                    pitch,
                    desired,
                    STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
                );
                angle = a;
                pitch = p;
                if turret_angles_at(
                    angle,
                    pitch,
                    desired,
                    STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
                ) {
                    scanning = false;
                    scan_index = scan_index.saturating_add(1);
                    holding = true;
                    hold_until = hold_turret_until_frame(frame);
                    next_frame = 0;
                    scan_completed = scan_completed.saturating_add(1);
                    hold_started = hold_started.saturating_add(1);
                }
            }

            if let Some(obj) = self.objects.get_mut(&cid) {
                obj.turret_idle_scanning = scanning;
                obj.record_host_turret();
                obj.turret_holding = holding;
                obj.record_host_turret();
                obj.turret_idle_recentering = idle_recentering;
                obj.turret_hold_until_frame = hold_until;
                obj.turret_idle_scan_desired_angle_deg = desired;
                obj.turret_idle_scan_next_frame = next_frame;
                obj.turret_idle_scan_index = scan_index;
                obj.turret_angle_deg = angle;
                obj.record_host_turret();
                obj.turret_pitch_deg = pitch;
                obj.record_host_turret();
            }
        }
        for _ in 0..scan_started {
            self.battle_plans.record_turret_idle_scan_start();
        }
        for _ in 0..scan_completed {
            self.battle_plans.record_turret_idle_scan_complete();
        }
        for _ in 0..hold_started {
            self.battle_plans.record_turret_hold_start();
        }
        for _ in 0..hold_completed {
            self.battle_plans.record_turret_hold_complete();
        }
        for _ in 0..idle_recenter_started {
            self.battle_plans.record_turret_idle_recenter_start();
        }
        for _ in 0..idle_recenter_completed {
            self.battle_plans.record_turret_idle_recenter_complete();
        }
    }

    /// Residual honesty: AmericaParachute low-altitude open fudge residual.
    pub fn honesty_pilot_parachute_open_fudge_ok(&self) -> bool {
        self.usa_pilot.honesty_parachute_open_fudge_ok()
    }

    /// Residual honesty: AmericaParachute FreeFallDamage residual.

    /// C++ SabotageInternetCenterCrateCollide residual deepen.
    ///
    /// - `disableInternetCenterSpyVision` on every team FSInternetCenter
    /// - `setDisabledUntil(DISABLED_HACKED)` on the sabotaged center
    /// - `disableHacker` on contained occupants
    ///
    /// Returns (internet_centers_spy_disabled, hackers_disabled).

    /// C++ SabotageSuperweaponCrateCollide::executeCrateBehavior
    /// (`SabotageSuperweaponCrateCollide.cpp:117-126`): walk every behavior
    /// module, `getSpecialPower()`, `startPowerRecharge()` on each.
    ///
    /// Leftover `start_power_recharge_at` SharedNSync is
    /// `player.reset_or_start_special_power_ready_frame` (now + ReloadTime).
    /// Fire gate and HUD read that player timer, not the object module frame.
    pub(crate) fn apply_superweapon_sabotage_recharge(&mut self, target_id: ObjectId) -> bool {
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let owner_id = self.player_owner_for_host_object(target);
        let modules = target.thing.template.special_power_modules.clone();

        let mut recharged: Vec<crate::command_system::SpecialPowerType> = Vec::new();
        let mut shared_resets: Vec<(crate::command_system::SpecialPowerType, f32)> = Vec::new();
        {
            let Some(target) = self.objects.get_mut(&target_id) else {
                return false;
            };
            // C++ walks every behavior module and startPowerRecharge()s each
            // SpecialPowerModuleInterface (Spy + Emergency Repair + CIA, etc.).
            for module in &modules {
                let Some(power) = module.command_power.as_ref() else {
                    continue;
                };
                let reload_seconds = if module.reload_time_frames > 0 {
                    target.start_power_recharge_with_frames(power, module.reload_time_frames);
                    module.reload_time_frames as f32 / 30.0
                } else {
                    target.start_power_recharge(power);
                    crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                        power,
                    )
                    .unwrap_or(target.special_power_cooldown)
                    .max(0.0)
                };
                if !recharged.contains(power) {
                    recharged.push(power.clone());
                }
                if module.shared_n_sync && !shared_resets.iter().any(|(p, _)| p == power) {
                    shared_resets.push((power.clone(), reload_seconds));
                }
            }
            let leftover: Vec<crate::command_system::SpecialPowerType> = target
                .special_power_cooldowns
                .keys()
                .filter(|p| !recharged.contains(p))
                .cloned()
                .collect();
            for power in leftover {
                target.start_power_recharge(&power);
                if crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
                    &power,
                ) && !shared_resets.iter().any(|(p, _)| p == &power)
                {
                    let reload = crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                        &power,
                    )
                    .unwrap_or(target.special_power_cooldown)
                    .max(0.0);
                    shared_resets.push((power.clone(), reload));
                }
                recharged.push(power);
            }
            if recharged.is_empty() {
                // Host residual single-slot when no modules / map entries exist.
                target.set_special_power_ready(false);
                if target.special_power_cooldown <= 0.0 {
                    target.special_power_cooldown = 10.0;
                }
                target.special_power_cooldown_remaining = target.special_power_cooldown;
            }
        }
        if let Some(pid) = owner_id {
            if let Some(player) = self.get_player_mut(pid) {
                for (power, reload) in shared_resets {
                    player.reset_shared_special_power_timer(&power, reload);
                }
            }
        }
        let _ = self
            .special_power_strikes
            .reset_timers_for_source_object(target_id);
        true
    }

    pub(crate) fn apply_internet_center_sabotage_residual(
        &mut self,
        center_id: ObjectId,
        owner_team: Team,
        until_frame: u32,
    ) -> (u32, u32) {
        // 1) All team internet centers: SpyVision disabled until frame.
        let center_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && o.team == owner_team
                    // C++ `disableInternetCenterSpyVision` queries the exact
                    // `KINDOF_FS_INTERNET_CENTER`; an InternetHackContain
                    // alone is not that sabotage target.
                    && o.is_kind_of(KindOf::FSInternetCenter)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut centers = 0u32;
        for cid in &center_ids {
            if let Some(c) = self.objects.get_mut(cid) {
                c.apply_spy_vision_disabled_until(until_frame);
                // C++ SpyVisionUpdate::setDisabledUntilFrame → m_resetTimersNextUpdate.
                c.status.spy_vision_reset_timers = true;
                centers = centers.saturating_add(1);
            }
        }

        // 2) Sabotaged center DISABLED_HACKED (visual fluff residual).
        let mut hackers = 0u32;
        let mut occupants: Vec<ObjectId> = self
            .objects
            .get(&center_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        // Also collect reverse contained_by residual (garrison / fail-open).
        for (id, o) in &self.objects {
            if o.contained_by == Some(center_id) && !occupants.contains(id) {
                occupants.push(*id);
            }
        }
        if let Some(target) = self.objects.get_mut(&center_id) {
            target.apply_disabled_hacked(until_frame);
            target.apply_spy_vision_disabled_until(until_frame);
        }

        // 3) Contained hackers DISABLED_HACKED residual.
        for hid in occupants {
            if let Some(h) = self.objects.get_mut(&hid) {
                h.apply_disabled_hacked(until_frame);
                hackers = hackers.saturating_add(1);
            }
        }
        (centers.max(1), hackers) // at least the primary center counted
    }

    pub fn honesty_pilot_free_fall_damage_ok(&self) -> bool {
        self.usa_pilot.honesty_free_fall_damage_ok()
    }

    /// AmericaParachute FreeFallDamage residual: destroy chute mid-air.
    ///
    /// C++ ParachuteContain::onDie while significantly above terrain applies
    /// FreeFallDamagePercent (**0.5**) max-health DAMAGE_FALLING residual and
    /// leaves the rider freefalling (chute closed). Fail-closed: not full
    /// physics fling / DEATH_SPLATTED SlowDeath matrix.
    ///
    /// `id` may be:
    /// - a parachuting pilot/rider (legacy host residual path), or
    /// - an AmericaParachute container (C++ onDie on the chute Object).
    ///
    /// Returns true when residual applied to at least one rider/pilot.
    pub fn destroy_eject_parachute_midair(&mut self, id: ObjectId) -> bool {
        use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
        use crate::game_logic::host_usa_pilot::{
            HostDeathType, PILOT_FREE_FALL_DAMAGE_AUDIO, free_fall_damage_amount,
            should_apply_parachute_free_fall_damage,
        };

        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        let is_chute = obj
            .template_name
            .eq_ignore_ascii_case(HIJACKER_PARACHUTE_NAME);
        let height = obj.get_position().y;
        let chute_parachuting = obj.is_parachuting() || is_chute;
        if !should_apply_parachute_free_fall_damage(chute_parachuting, height) {
            return false;
        }

        // Container path: removeAllContained + FreeFallDamage each rider.
        if is_chute {
            let riders = obj.contained_units();
            let chute_pos = obj.get_position();
            let mut any = false;
            for rid in riders {
                // Exit contain residual.
                if let Some(chute) = self.objects.get_mut(&id) {
                    let _ = chute.exit_transport(rid);
                }
                let applied = self.apply_rider_free_fall_damage(rid, chute_pos);
                any |= applied;
            }
            // Kill chute residual (if not already dying).
            if let Some(chute) = self.objects.get_mut(&id) {
                chute.clear_eject_parachuting();
                if chute.is_alive() {
                    let hp = chute.health.current.max(1.0);
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        crate::game_logic::host_damage_log::record(id, hp, None, true);
                    } else {
                        chute.health.current = 0.0;
                    }
                    chute.status.destroyed = true;
                    chute.status.death_type = HostDeathType::Normal;
                }
            }
            if any {
                self.car_bomb.record_airborne_parachute_free_fall();
            }
            return any;
        }

        // Legacy pilot/rider path (no separate chute Object).
        self.apply_rider_free_fall_damage(id, obj.get_position())
    }

    /// Apply FreeFallDamagePercent residual to one rider and leave freefalling.
    pub(in super::super) fn apply_rider_free_fall_damage(
        &mut self,
        rider_id: ObjectId,
        eject_pos: glam::Vec3,
    ) -> bool {
        use crate::game_logic::host_usa_pilot::{
            HostDeathType, PILOT_FREE_FALL_DAMAGE_AUDIO, free_fall_damage_amount,
            should_apply_parachute_free_fall_damage,
        };

        let Some(rider) = self.objects.get(&rider_id) else {
            return false;
        };
        // Rider may still be parachuting from chute; height from eject pos.
        let height = eject_pos.y.max(rider.get_position().y);
        if !should_apply_parachute_free_fall_damage(true, height) {
            return false;
        }
        let max_hp = rider.health.maximum.max(rider.max_health);
        let dmg = free_fall_damage_amount(max_hp);

        let destroyed = if let Some(r) = self.objects.get_mut(&rider_id) {
            r.set_contained_by(None);
            r.set_ai_state(AIState::Idle);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(rider_id, 0);
            }
            r.set_position(eject_pos);
            crate::game_logic::host_ground_height_log::record(rider_id, eject_pos.y, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    rider_id,
                    Some([eject_pos.x, eject_pos.y, eject_pos.z]),
                );
                r.record_host_movement();
            }
            // Chute destroyed → freefall residual (chute closed, still parachuting sink).
            r.set_status_parachute_open(false);
            r.set_status_parachuting(true);
            r.status.airborne_target = true;
            r.set_status_masked(false);
            r.set_status_unselectable(false);
            r.set_status_no_collisions(false);
            // DAMAGE_FALLING / DEATH_SPLATTED residual if this kill finishes them.
            let killed = r.take_damage_from_typed_death(
                dmg,
                None,
                crate::game_logic::combat::DamageType::Unresistable,
                HostDeathType::Splatted,
            );
            // Ensure freefall continues even if take_damage cleared parachuting on death.
            if !killed {
                r.set_status_parachuting(true);
                r.set_status_parachute_open(false);
                r.status.airborne_target = true;
            }
            killed
        } else {
            return false;
        };

        self.usa_pilot.record_free_fall_damage();
        self.queue_audio_event(
            AudioEventRequest::new(PILOT_FREE_FALL_DAMAGE_AUDIO)
                .with_object(rider_id)
                .with_position(eject_pos)
                .with_priority(160),
        );
        if destroyed {
            self.mark_object_for_destruction(rider_id, None);
        }
        true
    }

    /// C++ `BattlePlanUpdate::setStatus` SearchAndDestroy idle loop.
    /// Play on TRANSITIONSTATUS_ACTIVE; remove when leaving ACTIVE.
    fn queue_search_and_destroy_idle_audio(&mut self, center_id: ObjectId, stop: bool) {
        use crate::game_logic::host_strategy_center::BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO;
        let pos = self
            .objects
            .get(&center_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let mut req = AudioEventRequest::new(BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO)
            .with_object(center_id)
            .with_position(pos)
            .with_priority(170);
        if stop {
            req = req.stopping();
        } else {
            req = req.looping();
        }
        self.queue_audio_event(req);
    }

    /// Tick BattlePlanUpdate pack/unpack door residual (AnimationTime frames).
    ///
    /// Advances OPENING → WAITING_TO_CLOSE (BecameActive → setBattlePlan) and
    /// CLOSING → IDLE → UNPACKING. Packing start clears army effects
    /// (setBattlePlan NONE + paralyze). Recenter residual may delay pack.
    /// While recentering, steps Strategy Center turret pitch/yaw toward natural.
    /// Also ticks TurretAI idle-scan residual for Bombardment ACTIVE centers.
    pub(in super::super) fn tick_battle_plan_door_residuals(&mut self) {
        use crate::game_logic::host_strategy_center::{
            HostBattlePlanDoorEvent, step_turret_toward_natural,
        };

        // Step Bombardment turret angles toward natural while recenter residual runs.
        let centering: Vec<ObjectId> = self
            .battle_plans
            .door_states()
            .iter()
            .filter(|s| s.centering_turret)
            .map(|s| s.center_id)
            .collect();
        for cid in centering {
            if let Some(obj) = self.objects.get_mut(&cid) {
                // Pack recenter cancels idle-scan / Hold / idle-recenter residual.
                obj.turret_idle_scanning = false;
                obj.record_host_turret();
                obj.turret_holding = false;
                obj.record_host_turret();
                obj.turret_hold_until_frame = 0;
                obj.turret_idle_recentering = false;
                let (a, p) = step_turret_toward_natural(obj.turret_angle_deg, obj.turret_pitch_deg);
                obj.turret_angle_deg = a;
                obj.record_host_turret();
                obj.turret_pitch_deg = p;
                obj.record_host_turret();
            }
        }

        // TurretAI idle-scan residual (Bombardment ACTIVE, idle gun).
        self.tick_strategy_center_turret_idle_scan();
        // TurretAI idle mood-target residual (friend_checkForIdleMoodTarget).
        self.tick_strategy_center_turret_mood_target();

        let frame = self.frame;
        let events = self.battle_plans.tick_door_residuals(frame);
        for event in events {
            match event {
                HostBattlePlanDoorEvent::Audio { center_id, event } => {
                    let pos = self
                        .objects
                        .get(&center_id)
                        .map(|o| o.get_position())
                        .unwrap_or(Vec3::ZERO);
                    self.queue_audio_event(
                        AudioEventRequest::new(event)
                            .with_position(pos)
                            .with_priority(170),
                    );
                }
                HostBattlePlanDoorEvent::BecameActive {
                    center_id,
                    player_id,
                    plan,
                } => {
                    self.apply_battle_plan_set_battle_plan(
                        player_id,
                        Some(plan),
                        Some(center_id),
                        false, // paralyze only on NONE
                    );
                    self.battle_plans.record_delayed_active_apply();
                    if plan
                        == crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy
                    {
                        self.queue_search_and_destroy_idle_audio(center_id, false);
                    }
                }
                HostBattlePlanDoorEvent::BeganPacking {
                    center_id,
                    player_id,
                } => {
                    // C++ setStatus(PACKING) → setBattlePlan(NONE) + paralyzeTroop.
                    let stop_idle = self
                        .battle_plans
                        .door_state_for_center(center_id)
                        .and_then(|s| s.door_plan)
                        == Some(
                            crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy,
                        );
                    self.apply_battle_plan_set_battle_plan(player_id, None, Some(center_id), true);
                    self.battle_plans.record_pack_clear();
                    if stop_idle {
                        self.queue_search_and_destroy_idle_audio(center_id, true);
                    }
                }
                HostBattlePlanDoorEvent::BeganRecenter { .. } => {
                    // Counter already recorded in begin_door_residual.
                }
            }
        }
        self.stamp_battle_plan_door_model_conditions();
    }

    /// C++ BattlePlanUpdate::setStatus door OPENING/CLOSING/WAITING_TO_CLOSE.
    fn stamp_battle_plan_door_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_to_close_model_bit,
            door_2_closing_model_bit, door_2_opening_model_bit, door_2_waiting_to_close_model_bit,
            door_3_closing_model_bit, door_3_opening_model_bit, door_3_waiting_to_close_model_bit,
        };
        use crate::game_logic::host_strategy_center::HostBattlePlanDoor;
        let stamps: Vec<(ObjectId, HostBattlePlanDoor)> = self
            .battle_plans
            .door_states()
            .iter()
            .map(|s| (s.center_id, s.door))
            .collect();
        let clear = (1u128 << door_1_opening_model_bit())
            | (1u128 << door_1_closing_model_bit())
            | (1u128 << door_1_waiting_to_close_model_bit())
            | (1u128 << door_2_opening_model_bit())
            | (1u128 << door_2_closing_model_bit())
            | (1u128 << door_2_waiting_to_close_model_bit())
            | (1u128 << door_3_opening_model_bit())
            | (1u128 << door_3_closing_model_bit())
            | (1u128 << door_3_waiting_to_close_model_bit());
        for (id, door) in stamps {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.model_condition_bits &= !clear;
            let set_bit = match door {
                HostBattlePlanDoor::Door1Opening => Some(door_1_opening_model_bit()),
                HostBattlePlanDoor::Door1WaitingToClose => {
                    Some(door_1_waiting_to_close_model_bit())
                }
                HostBattlePlanDoor::Door1Closing => Some(door_1_closing_model_bit()),
                HostBattlePlanDoor::Door2Opening => Some(door_2_opening_model_bit()),
                HostBattlePlanDoor::Door2WaitingToClose => {
                    Some(door_2_waiting_to_close_model_bit())
                }
                HostBattlePlanDoor::Door2Closing => Some(door_2_closing_model_bit()),
                HostBattlePlanDoor::Door3Opening => Some(door_3_opening_model_bit()),
                HostBattlePlanDoor::Door3WaitingToClose => {
                    Some(door_3_waiting_to_close_model_bit())
                }
                HostBattlePlanDoor::Door3Closing => Some(door_3_closing_model_bit()),
                HostBattlePlanDoor::None => None,
            };
            if let Some(bit) = set_bit {
                obj.model_condition_bits |= 1u128 << bit;
            }
            obj.record_host_model_condition();
        }
    }

    /// C++ BattlePlanUpdate::setBattlePlan residual (army + building effects).
    ///
    /// `plan = None` → PLANSTATUS_NONE (clear previous + optional paralyze).
    /// `plan = Some(...)` → apply army/building residuals for that plan.
    pub(in super::super) fn apply_battle_plan_set_battle_plan(
        &mut self,
        player_id: u32,
        plan: Option<crate::game_logic::host_strategy_center::HostBattlePlan>,
        strategy_center_id: Option<ObjectId>,
        paralyze_on_none: bool,
    ) {
        use crate::game_logic::host_strategy_center::{
            HostBattlePlan, STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR,
            apply_strategy_center_search_and_destroy_sight, battle_plan_paralyze_until_frame,
            is_dozer_template_name, is_drone_template_name, is_legal_battle_plan_member,
            is_strategy_center_template, remove_strategy_center_search_and_destroy_sight,
            strategy_center_stealth_detection_range_when_enabled,
            strategy_center_stealth_detector_enabled_for_plan,
        };

        let frame = self.frame;
        let paralyze_until = battle_plan_paralyze_until_frame(frame);

        let team = strategy_center_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .unwrap_or_else(|| match player_id {
                0 => Team::USA,
                1 => Team::China,
                2 => Team::GLA,
                _ => Team::Neutral,
            });

        let prev_plan = self.battle_plans.active_plan_for_player(player_id);

        // --- Clear previous army residual bonuses ---
        let previous_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.team == team && obj.has_battle_plan_bonus() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in previous_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.clear_battle_plan_bonus();
            }
        }

        // Reverse previous Strategy Center building residual from active plan.
        let mut disabled_stealth_detector = false;
        if let (Some(prev), Some(center_id)) = (prev_plan, strategy_center_id) {
            if let Some(center) = self.objects.get_mut(&center_id) {
                match prev {
                    HostBattlePlan::HoldTheLine => {
                        let ratio = if center.max_health > 0.0 {
                            center.health.current / center.max_health
                        } else {
                            1.0
                        };
                        center.max_health /=
                            STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR.max(0.01);
                        center.health.maximum = center.max_health;
                        {
                            let new_hp = (center.max_health * ratio).clamp(0.0, center.max_health);
                            Self::write_object_health_authority_aware(center, new_hp);
                        }
                    }
                    HostBattlePlan::SearchAndDestroy => {
                        center.detection_range = 0.0;
                        center.record_host_detector();
                        center.is_detector = false;
                        center.record_host_detector();
                        center.detection_rate_frames = 0;
                        center.record_host_detector();
                        center.next_detection_scan_frame = 0;
                        disabled_stealth_detector = true;
                        // C++ BattlePlanUpdate.cpp:745-749 divide vision/shroud by scalar.
                        let (vision, shroud) = remove_strategy_center_search_and_destroy_sight(
                            center.vision_range,
                            center.shroud_clearing_range,
                        );
                        center.vision_range = vision;
                        center.shroud_clearing_range = shroud;
                    }
                    HostBattlePlan::Bombardment => {
                        let _ = center.replace_weapon_set_slot(0, None);
                        let _ = center.replace_weapon_set_slot(1, None);
                        center.stop_attack();
                        // C++ enableTurret(false) when leaving Bombardment.
                        center.turret_enabled = false;
                        // Cancel TurretAI idle-scan / Hold residual when gun unequips.
                        center.turret_idle_scanning = false;
                        center.turret_holding = false;
                        center.turret_hold_until_frame = 0;
                        center.turret_idle_recentering = false;
                        center.turret_idle_scan_next_frame = 0;
                    }
                }
            }
        }
        if disabled_stealth_detector {
            self.battle_plans.record_stealth_detector_disable();
        }

        // Clear plan-affecting-army bookkeeping.
        self.battle_plans.clear_active_plan(player_id);

        // PLANSTATUS_NONE: paralyze troops residual (C++ setBattlePlan NONE).
        if plan.is_none() {
            if paralyze_on_none {
                let candidates: Vec<ObjectId> = self
                    .objects
                    .iter()
                    .filter_map(|(id, obj)| {
                        if !obj.is_alive() {
                            return None;
                        }
                        let is_structure = obj.is_kind_of(KindOf::Structure)
                            || obj.object_type == ObjectType::Building;
                        let is_infantry = obj.is_kind_of(KindOf::Infantry)
                            || obj.object_type == ObjectType::Infantry;
                        let is_vehicle = obj.is_kind_of(KindOf::Vehicle)
                            || obj.object_type == ObjectType::Vehicle;
                        let is_aircraft = obj.is_kind_of(KindOf::Aircraft)
                            || obj.object_type == ObjectType::Aircraft;
                        let is_dozer =
                            is_dozer_template_name(&obj.template_name) || obj.is_worker();
                        let is_drone = is_drone_template_name(&obj.template_name);
                        let can_attack = obj.can_attack()
                            || obj.weapon.is_some()
                            || obj.secondary_weapon.is_some();
                        let under_construction =
                            obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                        let same_team = obj.team == team;
                        if !is_legal_battle_plan_member(
                            is_infantry,
                            is_vehicle,
                            can_attack,
                            is_structure,
                            is_aircraft,
                            is_dozer,
                            is_drone,
                            true,
                            same_team,
                            under_construction,
                        ) {
                            return None;
                        }
                        Some(*id)
                    })
                    .collect();
                let mut paralyzed: u32 = 0;
                for id in candidates {
                    if let Some(target) = self.objects.get_mut(&id) {
                        if target.is_alive() {
                            target.apply_disabled_paralyzed(paralyze_until);
                            paralyzed = paralyzed.saturating_add(1);
                        }
                    }
                }
                if paralyzed > 0 {
                    self.battle_plans
                        .record_effect_application(0, false, paralyzed);
                }
            }
            return;
        }

        let plan = match plan {
            Some(p) => p,
            None => return,
        };

        // Apply army residual bonuses.
        let candidates: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let is_structure =
                    obj.is_kind_of(KindOf::Structure) || obj.object_type == ObjectType::Building;
                let is_infantry =
                    obj.is_kind_of(KindOf::Infantry) || obj.object_type == ObjectType::Infantry;
                let is_vehicle =
                    obj.is_kind_of(KindOf::Vehicle) || obj.object_type == ObjectType::Vehicle;
                let is_aircraft =
                    obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft;
                let is_dozer = is_dozer_template_name(&obj.template_name) || obj.is_worker();
                let is_drone = is_drone_template_name(&obj.template_name);
                let can_attack = obj.can_attack()
                    || obj.weapon.is_some()
                    || obj.secondary_weapon.is_some()
                    || obj.tertiary_weapon.is_some();
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let same_team = obj.team == team;
                if !is_legal_battle_plan_member(
                    is_infantry,
                    is_vehicle,
                    can_attack,
                    is_structure,
                    is_aircraft,
                    is_dozer,
                    is_drone,
                    true,
                    same_team,
                    under_construction,
                ) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut buffs: u32 = 0;
        for id in candidates {
            if let Some(target) = self.objects.get_mut(&id) {
                if target.is_alive() {
                    target.apply_battle_plan_bonus(plan);
                    buffs = buffs.saturating_add(1);
                }
            }
        }

        // Strategy Center building residual bonuses.
        let mut building_bonus = false;
        let mut enabled_stealth_detector = false;
        if let Some(center_id) = strategy_center_id {
            if let Some(center) = self.objects.get_mut(&center_id) {
                let is_center = is_strategy_center_template(&center.template_name)
                    || center.is_kind_of(KindOf::FSStrategyCenter);
                if is_center && center.is_alive() {
                    match plan {
                        HostBattlePlan::HoldTheLine => {
                            let ratio = if center.max_health > 0.0 {
                                center.health.current / center.max_health
                            } else {
                                1.0
                            };
                            center.max_health *= STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR;
                            center.health.maximum = center.max_health;
                            {
                                let new_hp =
                                    (center.max_health * ratio).clamp(0.0, center.max_health);
                                Self::write_object_health_authority_aware(center, new_hp);
                            }
                            building_bonus = true;
                        }
                        HostBattlePlan::SearchAndDestroy => {
                            if strategy_center_stealth_detector_enabled_for_plan(plan) {
                                center.detection_range =
                                    strategy_center_stealth_detection_range_when_enabled();
                                center.is_detector = true;
                                center.record_host_detector();
                                // DetectionRate residual: 500ms → 15 frames.
                                // setSDEnabled(true) → first scan immediate (next=0).
                                center.detection_rate_frames =
                                    crate::game_logic::host_strategy_center::STRATEGY_CENTER_STEALTH_DETECTION_RATE_FRAMES;
                                center.next_detection_scan_frame = 0;
                                enabled_stealth_detector = true;
                            }
                            // C++ BattlePlanUpdate.cpp:804-810 multiply vision/shroud by 2.0.
                            let (vision, shroud) = apply_strategy_center_search_and_destroy_sight(
                                center.vision_range,
                                center.shroud_clearing_range,
                            );
                            center.vision_range = vision;
                            center.shroud_clearing_range = shroud;
                            building_bonus = true;
                        }
                        HostBattlePlan::Bombardment => {
                            // C++ enableTurret(true) after unpack ACTIVE.
                            center.turret_enabled = true;
                            let _ = center.replace_weapon_set_slot(0, Some(
                                crate::game_logic::host_strategy_center::strategy_center_gun_weapon(
                                ),
                            ));
                            let _ = center.replace_weapon_set_slot(1, None);
                            // TurretAI idle-scan residual: schedule first idle scan
                            // via leftover GameLogicRandomValue(min, max).
                            center.turret_idle_scan_index = 0;
                            center.turret_idle_scanning = false;
                            center.turret_holding = false;
                            center.turret_hold_until_frame = 0;
                            center.turret_idle_recentering = false;
                            center.turret_idle_scan_next_frame = frame.saturating_add(
                                crate::game_logic::host_strategy_center::idle_scan_interval_frames(
                                    0,
                                ),
                            );
                            building_bonus = true;
                        }
                    }
                }
            }
        }
        if enabled_stealth_detector {
            self.battle_plans.record_stealth_detector_enable();
        }

        self.battle_plans.set_active_plan(player_id, plan);
        self.battle_plans
            .record_effect_application(buffs, building_bonus, 0);
        let _ = frame;
    }

    /// Activate Frenzy / Rage residual: temporary ally attack buff in radius.
    ///
    /// Matches retail SuperweaponFrenzy → Frenzy_InvisibleMarker WeaponBonusUpdate:
    /// - Radius residual 200 (RadiusCursorRadius / BonusRange)
    /// - BonusDuration 10000/20000/30000 ms by level (FRENZY_ONE/TWO/THREE)
    /// - DAMAGE 110% / 120% / 130% while buffed
    /// - Allies (player relationship ALLOW_ALLIES), CAN_ATTACK residual, not STRUCTURE
    /// - iterateContained: garrison/transport passengers of an in-range ally
    ///   container get the buff even when the container is STRUCTURE
    ///
    /// Fail-closed: not full OCL marker object / science upgrade matrix / particle.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_frenzy(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
        level: crate::game_logic::host_frenzy::HostFrenzyLevel,
    ) -> bool {
        use crate::game_logic::host_frenzy::{
            FRENZY_ACTIVATE_AUDIO, HOST_FRENZY_RADIUS, HostFrenzy, in_frenzy_radius_2d,
            is_legal_frenzy_target,
        };
        use gamelogic::common::Relationship;
        use std::collections::HashSet;

        let frame = self.frame;
        let duration = level.duration_frames();
        let until = frame.saturating_add(duration);
        let center = (location.x, location.z);

        let caster_team = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .unwrap_or_else(|| match player_id {
                0 => Team::USA,
                1 => Team::China,
                2 => Team::GLA,
                _ => Team::Neutral,
            });
        let caster_owner = caster_id
            .and_then(|cid| self.objects.get(&cid))
            .and_then(|c| self.player_owner_for_host_object(c))
            .or(Some(player_id));
        let caster_team_instance = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team_instance_name.clone()))
            .unwrap_or_default();

        // Snapshot in-range objects (include STRUCTURE so contained walk can fire).
        let candidates: Vec<(ObjectId, bool, bool, bool, bool, Vec<ObjectId>)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                if !in_frenzy_radius_2d(center, (pos.x, pos.z), HOST_FRENZY_RADIUS) {
                    return None;
                }
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let obj_owner = self.player_owner_for_host_object(obj);
                let is_ally = match (caster_owner, obj_owner) {
                    (Some(_), Some(_)) => {
                        GameLogic::object_relationship_from_owners(
                            &self.players,
                            caster_owner,
                            &caster_team_instance,
                            obj.owner_player_id,
                            &obj.team_instance_name,
                        ) == Relationship::Allies
                    }
                    _ => obj.team == caster_team && caster_team != Team::Neutral,
                };
                let can_attack = obj.can_attack()
                    || obj.weapon.is_some()
                    || obj.secondary_weapon.is_some()
                    || obj.tertiary_weapon.is_some();
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                Some((
                    *id,
                    is_structure,
                    is_ally,
                    can_attack,
                    under_construction,
                    obj.contained_units(),
                ))
            })
            .collect();

        let mut buffs: u32 = 0;
        let mut applied: HashSet<ObjectId> = HashSet::new();
        let mut contained_ids: Vec<ObjectId> = Vec::new();
        for (id, is_structure, is_ally, can_attack, under_construction, occupants) in candidates {
            if is_ally {
                contained_ids.extend(occupants);
            }
            if !is_legal_frenzy_target(is_structure, true, is_ally, can_attack, under_construction)
            {
                continue;
            }
            if !applied.insert(id) {
                continue;
            }
            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let was_buffed = target.weapon_bonus_frenzy;
            target.apply_weapon_bonus_frenzy(level.as_u8(), until);
            if !was_buffed || target.weapon_bonus_frenzy {
                buffs = buffs.saturating_add(1);
            }
        }

        // C++ WeaponBonusUpdate iterateContained — KindOf only (container already Allies).
        for occ_id in contained_ids {
            if !applied.insert(occ_id) {
                continue;
            }
            let Some(occ) = self.objects.get(&occ_id) else {
                continue;
            };
            if !occ.is_alive() {
                continue;
            }
            let is_structure = occ.is_kind_of(KindOf::Structure);
            let can_attack = occ.can_attack()
                || occ.weapon.is_some()
                || occ.secondary_weapon.is_some()
                || occ.tertiary_weapon.is_some();
            let under_construction =
                occ.status.under_construction || occ.construction_percent + 0.001 < 1.0;
            if !is_legal_frenzy_target(is_structure, true, true, can_attack, under_construction) {
                continue;
            }
            let Some(target) = self.objects.get_mut(&occ_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let was_buffed = target.weapon_bonus_frenzy;
            target.apply_weapon_bonus_frenzy(level.as_u8(), until);
            if !was_buffed || target.weapon_bonus_frenzy {
                buffs = buffs.saturating_add(1);
            }
        }

        let frenzy_id = self.frenzies.alloc_id();
        self.frenzies.record_activation(HostFrenzy {
            id: frenzy_id,
            player_id,
            location,
            radius: HOST_FRENZY_RADIUS,
            level,
            activate_frame: frame,
            expire_frame: until,
            caster_id,
            buffs,
        });

        self.queue_audio_event(
            AudioEventRequest::new(FRENZY_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            location,
            frame,
            caster_id,
            None,
        );

        // C++ SUPERWEAPON_Frenzy* OCL Frenzy_InvisibleMarker + DeletionUpdate residual.
        let _ = self.spawn_frenzy_invisible_marker(caster_team, location, level);

        true
    }

    /// Select a USA Strategy Center battle plan residual (intent → door residual).
    ///
    /// C++ BattlePlanUpdate::initiateIntentToDoSpecialPower sets desired plan;
    /// army buffs / building bonuses / turret / StealthDetector apply only when
    /// door residual reaches ACTIVE (setBattlePlan after unpack). Plan switch
    /// packs first (setBattlePlan NONE + paralyze), then unpacks new plan.
    ///
    /// Residual slice:
    /// - Bombardment: DAMAGE 120% + StrategyCenterGun after ACTIVE
    /// - HoldTheLine: armor 0.9 + center max-health ×2 after ACTIVE
    /// - SearchAndDestroy: RANGE 120% + building vision/shroud ×2 + StealthDetector 500 after ACTIVE
    /// - BattlePlanChangeParalyzeTime: 150 frames on PACKING (NONE transition)
    /// - AnimationTime **7000**ms → **210** frames pack/unpack
    /// - Bombardment non-natural turret → recenter (angle-based or **30** frame coast)
    /// - Turret natural-position pitch/yaw residual (NaturalTurretAngle **-90** /
    ///   NaturalTurretPitch **45** / rates **60** deg/s)
    ///
    /// Fail-closed: not full TurretAI idle-scan state machine /
    /// VisionObjectName spawn (createVisionObject disabled retail).
    /// Returns true when the residual selection was recorded.
    pub fn activate_battle_plan(
        &mut self,
        player_id: u32,
        plan: crate::game_logic::host_strategy_center::HostBattlePlan,
        strategy_center_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_strategy_center::{
            BATTLE_PLAN_TURRET_RECENTER_FRAMES, HostBattlePlanDoorEvent, HostBattlePlanSelection,
            strategy_center_turret_is_natural_with_angles, strategy_center_turret_recenter_frames,
        };

        let frame = self.frame;
        let audio = plan.activate_audio();
        let audio_pos = strategy_center_id
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(Vec3::ZERO);

        // No Strategy Center object: fail-closed immediate apply (no door residual).
        let Some(center_id) = strategy_center_id else {
            self.apply_battle_plan_set_battle_plan(player_id, Some(plan), None, false);
            let selection_id = self.battle_plans.alloc_id();
            self.battle_plans.record_selection(
                HostBattlePlanSelection {
                    id: selection_id,
                    player_id,
                    plan,
                    activate_frame: frame,
                    strategy_center_id: None,
                    buffs: 0,
                    building_bonus: false,
                    paralyzed: 0,
                },
                true,
            );
            self.queue_audio_event(
                AudioEventRequest::new(audio)
                    .with_position(audio_pos)
                    .with_priority(180),
            );
            return true;
        };

        // Turret natural residual for Bombardment pack gate (pitch/yaw + busy).
        let (turret_natural, recenter_frames) = {
            let center = self.objects.get(&center_id);
            match center {
                Some(c) => {
                    let is_attacking =
                        c.status.attacking || matches!(c.ai_state, AIState::Attacking);
                    let has_target = c.target.is_some();
                    let now_secs = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                    let last_fire_age = c.weapon.as_ref().map(|w| {
                        // last_fire_time is seconds residual; convert to frames @ 30 FPS.
                        let age_secs = (now_secs - w.last_fire_time).max(0.0);
                        (age_secs * 30.0).floor() as u32
                    });
                    let natural = strategy_center_turret_is_natural_with_angles(
                        is_attacking,
                        has_target,
                        last_fire_age,
                        c.turret_angle_deg,
                        c.turret_pitch_deg,
                    );
                    let busy = is_attacking
                        || has_target
                        || last_fire_age.is_some_and(|a| a < BATTLE_PLAN_TURRET_RECENTER_FRAMES);
                    let frames = strategy_center_turret_recenter_frames(
                        busy,
                        c.turret_angle_deg,
                        c.turret_pitch_deg,
                    );
                    (natural, frames)
                }
                None => (true, BATTLE_PLAN_TURRET_RECENTER_FRAMES),
            }
        };

        // Record selection intent (buffs deferred until BecameActive).
        let selection_id = self.battle_plans.alloc_id();
        self.battle_plans.record_selection(
            HostBattlePlanSelection {
                id: selection_id,
                player_id,
                plan,
                activate_frame: frame,
                strategy_center_id: Some(center_id),
                buffs: 0,
                building_bonus: false,
                paralyzed: 0,
            },
            false, // not active until unpack complete
        );

        // Start door residual (UNPACKING or PACKING / recenter).
        let door_events = self.battle_plans.begin_door_residual(
            center_id,
            player_id,
            plan,
            frame,
            turret_natural,
            recenter_frames,
        );
        for event in door_events {
            match event {
                HostBattlePlanDoorEvent::Audio {
                    center_id: cid,
                    event: name,
                } => {
                    let pos = self
                        .objects
                        .get(&cid)
                        .map(|o| o.get_position())
                        .unwrap_or(audio_pos);
                    self.queue_audio_event(
                        AudioEventRequest::new(name)
                            .with_position(pos)
                            .with_priority(170),
                    );
                }
                HostBattlePlanDoorEvent::BeganPacking {
                    center_id: cid,
                    player_id: pid,
                } => {
                    // Immediate pack clear + paralyze (setBattlePlan NONE).
                    let stop_idle = self
                        .battle_plans
                        .door_state_for_center(cid)
                        .and_then(|s| s.door_plan)
                        == Some(
                            crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy,
                        );
                    self.apply_battle_plan_set_battle_plan(pid, None, Some(cid), true);
                    self.battle_plans.record_pack_clear();
                    if stop_idle {
                        self.queue_search_and_destroy_idle_audio(cid, true);
                    }
                }
                HostBattlePlanDoorEvent::BeganRecenter { .. } => {
                    // Counter recorded in begin_door_residual.
                }
                HostBattlePlanDoorEvent::BecameActive { .. } => {
                    // Not emitted from begin_door_residual.
                }
            }
        }
        self.stamp_battle_plan_door_model_conditions();

        // C++ announcement audio + radar event fire when UNPACKING starts
        // (and on first select). Host residual: always on select intent.
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_position(audio_pos)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            audio_pos,
            frame,
            Some(center_id),
            None,
        );
        self.queue_radar_message_at(
            format!("Battle plan: {:?}", plan),
            audio_pos,
            radar_notifications::RadarKind::Ally,
        );
        if let Some(player) = self.players.get(&player_id) {
            crate::game_logic::host_radar::host_create_player_radar_event(
                crate::game_logic::host_radar::pack_player_color_argb(player.color_rgb),
                audio_pos,
                game_engine::common::system::radar::RadarEventType::BattlePlan,
            );
        }

        true
    }

    /// Host Emergency Repair residual registry (activate + honesty).
    pub fn emergency_repairs(
        &self,
    ) -> &crate::game_logic::host_emergency_repair::HostEmergencyRepairRegistry {
        &self.emergency_repairs
    }

    /// Residual honesty: Emergency Repair activated at least once.
    pub fn honesty_emergency_repair_activate_ok(&self) -> bool {
        self.emergency_repairs.honesty_activate_ok()
    }

    /// Residual honesty: Emergency Repair healed at least one vehicle.
    pub fn honesty_emergency_repair_heal_ok(&self) -> bool {
        self.emergency_repairs.honesty_heal_ok()
    }

    /// Combined host path honesty for Emergency Repair residual.
    pub fn honesty_emergency_repair_ok(&self) -> bool {
        self.emergency_repairs.honesty_host_path_ok()
    }

    /// Host Cleanup Area residual registry (activate + honesty).
    pub fn cleanup_areas(&self) -> &crate::game_logic::host_cleanup_area::HostCleanupAreaRegistry {
        &self.cleanup_areas
    }

    /// Residual honesty: CleanupArea activated at least once.
    pub fn honesty_cleanup_area_activate_ok(&self) -> bool {
        self.cleanup_areas.honesty_activate_ok() || self.cleanup_stream_missiles_spawned > 0
    }

    /// Residual honesty: CleanupArea cleared at least one hazard/mine.
    pub fn honesty_cleanup_area_clear_ok(&self) -> bool {
        self.cleanup_areas.honesty_clear_ok()
    }

    /// Combined host path honesty for Cleanup Area residual.
    pub fn honesty_cleanup_area_ok(&self) -> bool {
        self.cleanup_areas.honesty_host_path_ok() || self.cleanup_stream_missiles_spawned > 0
    }

    /// Activate Cleanup Area: C++ CleanupAreaPower → setCleanupAreaParameters.
    /// Never recharges. Ambulance drives to the click, then sprays until clean.
    pub fn activate_cleanup_area(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_cleanup_area::{
            CLEANUP_AREA_ACTIVATE_AUDIO, HOST_CLEANUP_AREA_RADIUS, HOST_CLEANUP_MAX_MOVE_DISTANCE,
            HostCleanupArea, HostCleanupAreaOrder, is_cleanup_area_caster,
        };

        let Some(cid) = caster_id else {
            return false;
        };
        let Some(caster) = self.objects.get(&cid) else {
            return false;
        };
        if !caster.is_alive()
            || caster.is_disabled()
            || !is_cleanup_area_caster(&caster.template_name)
        {
            return false;
        }
        let from = caster.get_position();

        // C++ CleanupHazardUpdate::setCleanupAreaParameters: store pos/range, aiMoveToPosition.
        self.cleanup_areas
            .set_cleanup_area_parameters(HostCleanupAreaOrder {
                caster_id: cid,
                player_id,
                location,
                move_range: HOST_CLEANUP_MAX_MOVE_DISTANCE,
                next_shot_frame: self.frame,
            });
        let _ = self.unit_command_move_to(cid, location);

        let entry_id = self.cleanup_areas.alloc_id();
        self.cleanup_areas.record_activation(HostCleanupArea {
            id: entry_id,
            player_id,
            location,
            radius: HOST_CLEANUP_AREA_RADIUS,
            activate_frame: self.frame,
            caster_id: Some(cid),
            radiation_cleared: 0,
            toxin_cleared: 0,
            mines_cleared: 0,
        });
        self.queue_audio_event(
            AudioEventRequest::new(CLEANUP_AREA_ACTIVATE_AUDIO)
                .with_position(from)
                .with_priority(170),
        );
        true
    }

    /// C++ CleanupHazardUpdate::update — drive first, then scan/spray until clean.
    pub fn update_cleanup_area_orders(&mut self) {
        use crate::game_logic::host_cleanup_area::{
            HOST_CLEANUP_ARRIVE_DISTANCE, HOST_CLEANUP_SCAN_RANGE,
            HOST_CLEANUP_WEAPON_ATTACK_RANGE, HOST_CLEANUP_WEAPON_DELAY_FRAMES,
        };

        let frame = self.frame;
        let orders = self.cleanup_areas.take_orders();
        let mut keep = Vec::new();
        for mut order in orders {
            let Some(caster) = self.objects.get(&order.caster_id) else {
                continue;
            };
            if !caster.is_alive() {
                continue;
            }
            if caster.is_disabled() {
                keep.push(order);
                continue;
            }
            let cpos = caster.get_position();
            let dest = caster.movement.target_position;
            let ai = caster.ai_state.clone();
            let caster_team = caster.team;

            // Player cancel residual: a new move far from the click is not AI cleanup.
            if let Some(d) = dest {
                let dx = d.x - order.location.x;
                let dz = d.z - order.location.z;
                let max = order.move_range + HOST_CLEANUP_SCAN_RANGE;
                if dx * dx + dz * dz > max * max
                    && matches!(ai, AIState::Moving | AIState::AttackMoving)
                {
                    continue;
                }
            }

            let arriving = {
                let dx = cpos.x - order.location.x;
                let dz = cpos.z - order.location.z;
                dx * dx + dz * dz <= HOST_CLEANUP_ARRIVE_DISTANCE * HOST_CLEANUP_ARRIVE_DISTANCE
            };
            // C++ fireWhenReady only attacks when idle/busy. Drive first while Moving.
            let can_spray =
                arriving || matches!(ai, AIState::Idle | AIState::SpecialAbility) || dest.is_none();
            if !can_spray {
                keep.push(order);
                continue;
            }

            let scan_r = HOST_CLEANUP_SCAN_RANGE + order.move_range;
            let hazard = self.find_cleanup_hazard_near(order.location, scan_r, caster_team);
            if let Some(hpos) = hazard {
                let dx = cpos.x - hpos.x;
                let dz = cpos.z - hpos.z;
                let attack = HOST_CLEANUP_WEAPON_ATTACK_RANGE;
                if dx * dx + dz * dz > attack * attack {
                    let _ = self.unit_command_move_to(order.caster_id, hpos);
                    keep.push(order);
                    continue;
                }
                if frame >= order.next_shot_frame {
                    if self
                        .spawn_cleanup_stream_projectile(
                            order.caster_id,
                            cpos,
                            hpos,
                            order.player_id,
                        )
                        .is_none()
                    {
                        let _ = self.apply_cleanup_area_at(
                            order.player_id,
                            hpos,
                            Some(order.caster_id),
                        );
                    }
                    order.next_shot_frame = frame.saturating_add(HOST_CLEANUP_WEAPON_DELAY_FRAMES);
                }

                keep.push(order);
            } else if arriving {
                // Area is clean and we are at the click — C++ m_moveRange = 0.
            } else {
                let _ = self.unit_command_move_to(order.caster_id, order.location);
                keep.push(order);
            }
        }
        self.cleanup_areas.restore_orders(keep);
    }

    /// Closest toxin/radiation field or enemy/neutral mine around `center`.
    fn find_cleanup_hazard_near(
        &self,
        center: Vec3,
        radius: f32,
        caster_team: Team,
    ) -> Option<Vec3> {
        let r2 = radius * radius;
        let mut best: Option<(f32, Vec3)> = None;
        let consider = |pos: Vec3, best: &mut Option<(f32, Vec3)>| {
            let dx = pos.x - center.x;
            let dz = pos.z - center.z;
            let d2 = dx * dx + dz * dz;
            if d2 <= r2 && best.map(|(bd, _)| d2 < bd).unwrap_or(true) {
                *best = Some((d2, pos));
            }
        };
        for field in self.special_power_strikes.toxin_fields() {
            consider(field.position, &mut best);
        }
        for field in self.special_power_strikes.radiation_fields() {
            consider(field.position, &mut best);
        }
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let Some(mine) = obj.mine_data.as_ref() else {
                continue;
            };
            if mine.detonated || obj.team == caster_team {
                continue;
            }
            consider(obj.get_position(), &mut best);
        }
        best.map(|(_, p)| p)
    }

    /// Apply CleanupArea hazard/mine clear residual at impact (post-projectile or instant).
    pub fn apply_cleanup_area_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_cleanup_area::{
            CLEANUP_AREA_ACTIVATE_AUDIO, CLEANUP_AREA_HAZARD_AUDIO, CLEANUP_AREA_MINE_AUDIO,
            HOST_CLEANUP_AREA_RADIUS, HostCleanupArea,
        };

        let frame = self.frame;
        let radius = HOST_CLEANUP_AREA_RADIUS;

        // Clear residual radiation + toxin fields in radius.
        let radiation_cleared = self
            .special_power_strikes
            .clear_radiation_fields_in_radius(location, radius);
        let toxin_cleared = self
            .special_power_strikes
            .clear_toxin_fields_in_radius(location, radius);

        // Clear residual enemy/neutral mines in radius (safe disarm, no splash).
        let mut mines_to_clear: Vec<ObjectId> = Vec::new();
        for (id, obj) in &self.objects {
            if !obj.is_alive() {
                continue;
            }
            let Some(mine) = obj.mine_data.as_ref() else {
                continue;
            };
            if mine.detonated {
                continue;
            }
            // Never clear own/ally residual mines.
            if let Some(cid) = caster_id {
                if let Some(caster) = self.objects.get(&cid) {
                    if obj.team == caster.team {
                        continue;
                    }
                }
            }
            let pos = obj.get_position();
            let dx = pos.x - location.x;
            let dz = pos.z - location.z;
            if dx * dx + dz * dz <= radius * radius {
                mines_to_clear.push(*id);
            }
        }

        let mut mines_cleared = 0_u32;
        for mine_id in mines_to_clear {
            let clearer = caster_id.unwrap_or(ObjectId(0));
            if self.clear_mine_internal(mine_id, clearer) {
                mines_cleared = mines_cleared.saturating_add(1);
            }
        }

        // Bookkeeping entry (even if nothing cleared — activation honesty).
        let entry_id = self.cleanup_areas.alloc_id();
        self.cleanup_areas.record_activation(HostCleanupArea {
            id: entry_id,
            player_id,
            location,
            radius,
            activate_frame: frame,
            caster_id,
            radiation_cleared,
            toxin_cleared,
            mines_cleared,
        });

        self.queue_audio_event(
            AudioEventRequest::new(CLEANUP_AREA_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(170),
        );
        if radiation_cleared > 0 || toxin_cleared > 0 {
            self.queue_audio_event(
                AudioEventRequest::new(CLEANUP_AREA_HAZARD_AUDIO)
                    .with_position(location)
                    .with_priority(150),
            );
        }
        if mines_cleared > 0 {
            self.queue_audio_event(
                AudioEventRequest::new(CLEANUP_AREA_MINE_AUDIO)
                    .with_position(location)
                    .with_priority(150),
            );
        }

        true
    }

    /// Spawn CleanupStreamProjectile residual (MissileAI non-seek cleanup stream).
    pub fn spawn_cleanup_stream_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        player_id: u32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_cleanup_area::{
            CLEANUP_STREAM_MISSILE_FUEL_FRAMES, CLEANUP_STREAM_MISSILE_IGNITION_DELAY_FRAMES,
            CLEANUP_STREAM_MISSILE_MAX_HEALTH, HOST_CLEANUP_PROJECTILE,
            HOST_CLEANUP_PROJECTILE_STREAM,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(HOST_CLEANUP_PROJECTILE) {
            let mut t = ThingTemplate::new(HOST_CLEANUP_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(CLEANUP_STREAM_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(HOST_CLEANUP_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mut start = from;
        start.y = start.y.max(2.0);
        let pid = self.create_object(HOST_CLEANUP_PROJECTILE, team, start)?;
        let expires = self
            .frame
            .saturating_add(CLEANUP_STREAM_MISSILE_FUEL_FRAMES.max(1));
        let ignites = self
            .frame
            .saturating_add(CLEANUP_STREAM_MISSILE_IGNITION_DELAY_FRAMES);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.cleanup_stream_projectile = true;
            o.cleanup_stream_aim = Some([aim.x, aim.y, aim.z]);
            o.cleanup_stream_intended = None;
            o.cleanup_stream_travelled = 0.0;
            o.cleanup_stream_fuel_expires_frame = Some(expires);
            o.cleanup_stream_ignition_frame = Some(ignites);
            o.cleanup_stream_shooter = Some(source_id.0);
            o.cleanup_stream_player_id = player_id;
            o.note_producer(source_id);
            o.health.current = CLEANUP_STREAM_MISSILE_MAX_HEALTH;
            o.health.maximum = CLEANUP_STREAM_MISSILE_MAX_HEALTH;
        }
        self.projectile_streams.add_projectile(
            source_id,
            HOST_CLEANUP_PROJECTILE_STREAM,
            start,
            None,
            Some(aim),
            self.frame,
        );
        self.cleanup_stream_missiles_spawned =
            self.cleanup_stream_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_cleanup_stream_projectiles(&mut self) {
        use crate::game_logic::host_cleanup_area::{
            CLEANUP_STREAM_MISSILE_TURN_DISTANCE, HOST_CLEANUP_PROJECTILE_STREAM,
            cleanup_stream_missile_step_speed,
        };
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.cleanup_stream_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, u32)> = Vec::new();
        let mut stream_pts: Vec<(ObjectId, glam::Vec3, glam::Vec3)> = Vec::new();
        for id in flying {
            let (source, aim, pos, fuel_done, ignited, travelled, player_id, shooter) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .cleanup_stream_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let fuel_done = o
                    .cleanup_stream_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .cleanup_stream_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                let shooter = o.cleanup_stream_shooter.map(ObjectId).or(o.producer_id);
                (
                    o.producer_id,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
                    o.cleanup_stream_travelled,
                    o.cleanup_stream_player_id,
                    shooter,
                )
            };
            let can_steer = travelled >= CLEANUP_STREAM_MISSILE_TURN_DISTANCE;
            let speed = cleanup_stream_missile_step_speed(ignited && can_steer);
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let vel = if dist > 0.001 {
                to_aim.normalize() * step_speed
            } else {
                glam::Vec3::new(0.0, -step_speed, 0.0)
            };
            let step = vel.length().max(step_speed);
            let new_pos = pos + vel;
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(new_pos);
                o.cleanup_stream_travelled += step;
                o.cleanup_stream_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if let Some(sid) = shooter {
                stream_pts.push((sid, new_pos, aim));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 6.0;
            if fuel_done || near {
                impact.push((id, source, if near { aim } else { new_pos }, player_id));
            }
        }
        for (sid, pos, aim) in stream_pts {
            self.projectile_streams.add_projectile(
                sid,
                HOST_CLEANUP_PROJECTILE_STREAM,
                pos,
                None,
                Some(aim),
                frame,
            );
        }
        for (id, source, pos, player_id) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.cleanup_stream_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_cleanup_area_at(player_id, pos, source);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Spawn Angry Mob rock/molotov DumbProjectile Bezier residual.
    pub fn spawn_angry_mob_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        kind: crate::game_logic::host_angry_mob::AngryMobProjectileKind,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_angry_mob::{
            ANGRY_MOB_PROJ_MAX_HEALTH, angry_mob_projectile_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let name = kind.projectile_name();
        if !self.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Projectile)
                .set_health(ANGRY_MOB_PROJ_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(name.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mut start = from;
        start.y = start.y.max(2.0);
        let pid = self.create_object(name, team, start)?;
        let flight = angry_mob_projectile_flight_frames(start, aim, kind).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.angry_mob_projectile = true;
            o.angry_mob_projectile_kind = kind.as_u8();
            o.angry_mob_projectile_from = Some([start.x, start.y, start.z]);
            o.angry_mob_projectile_aim = Some([aim.x, aim.y, aim.z]);
            o.angry_mob_projectile_launch_frame = Some(self.frame);
            o.angry_mob_projectile_flight_frames = flight;
            o.angry_mob_projectile_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.current = ANGRY_MOB_PROJ_MAX_HEALTH;
            o.health.maximum = ANGRY_MOB_PROJ_MAX_HEALTH;
        }
        self.angry_mob_projectiles_spawned = self.angry_mob_projectiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_angry_mob_projectiles(&mut self) {
        use crate::game_logic::host_angry_mob::{
            AngryMobProjectileKind, angry_mob_projectile_bezier_point,
            angry_mob_projectile_damage_at,
        };
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.angry_mob_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(
            ObjectId,
            Option<ObjectId>,
            Option<ObjectId>,
            glam::Vec3,
            AngryMobProjectileKind,
        )> = Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, flight, kind) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .angry_mob_projectile_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .angry_mob_projectile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                let launch = o.angry_mob_projectile_launch_frame.unwrap_or(frame);
                let flight = o.angry_mob_projectile_flight_frames.max(1);
                let kind = AngryMobProjectileKind::from_u8(o.angry_mob_projectile_kind);
                (
                    o.producer_id,
                    o.angry_mob_projectile_intended.map(ObjectId),
                    from,
                    aim,
                    launch,
                    flight,
                    kind,
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / flight as f32).clamp(0.0, 1.0);
            let pos = angry_mob_projectile_bezier_point(from, aim, t, kind);
            if let Some(o) = self.objects.get_mut(&id) {
                o.set_position(pos);
            }
            if elapsed >= flight {
                impact.push((id, source, intended, aim, kind));
            }
        }
        for (id, source, intended, pos, kind) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.angry_mob_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_angry_mob_projectile_at(pos, source, intended, kind);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_angry_mob_projectile_ok(&self) -> bool {
        self.angry_mob_projectiles_spawned > 0
    }

    /// Apply rock/molotov splash residual at impact.
    pub fn apply_angry_mob_projectile_at(
        &mut self,
        impact: glam::Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        kind: crate::game_logic::host_angry_mob::AngryMobProjectileKind,
    ) -> (u32, bool) {
        use crate::game_logic::host_angry_mob::{
            ANGRY_MOB_MOLOTOV_DAMAGE_TYPE, ANGRY_MOB_MOLOTOV_DEATH_TYPE,
            ANGRY_MOB_ROCK_DAMAGE_TYPE, ANGRY_MOB_ROCK_DEATH_TYPE, AngryMobProjectileKind,
            angry_mob_possible_to_attack, angry_mob_projectile_damage_at,
            is_legal_angry_mob_damage_target,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);
        let radius = kind.radius();
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let victims: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source.map(|s| s == *id).unwrap_or(false) {
                    return None;
                }
                if obj.angry_mob_projectile {
                    return None;
                }
                if !angry_mob_possible_to_attack(
                    obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft,
                    obj.status.airborne_target,
                    obj.weapon_target_anti_mask(),
                ) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle);
                let same_team = obj.team == source_team;
                if !is_legal_angry_mob_damage_target(
                    obj.is_alive(),
                    same_team,
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                // RadiusDamageAffects ALLIES ENEMIES NEUTRALS residual — all teams.
                let _ = source_team;
                let _ = intended_target;
                let d = (obj.get_position() - impact).length();
                if d > radius + 0.001 {
                    return None;
                }
                Some((*id, d))
            })
            .collect();
        for (vid, dist) in victims {
            let dmg = angry_mob_projectile_damage_at(kind, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(v) = self.objects.get_mut(&vid) {
                let (dt_name, death_name) = match kind {
                    AngryMobProjectileKind::Molotov => {
                        (ANGRY_MOB_MOLOTOV_DAMAGE_TYPE, ANGRY_MOB_MOLOTOV_DEATH_TYPE)
                    }
                    AngryMobProjectileKind::Rock => {
                        (ANGRY_MOB_ROCK_DAMAGE_TYPE, ANGRY_MOB_ROCK_DEATH_TYPE)
                    }
                };
                let destroyed =
                    v.take_damage_from_immediate_residual(dmg, source, dt_name, death_name);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    let team = v.team;
                    self.mark_object_for_destruction(vid, Some(team));
                }
            }
        }
        (hits, any_destroyed)
    }

    pub fn honesty_cleanup_stream_projectile_ok(&self) -> bool {
        self.cleanup_stream_missiles_spawned > 0
    }

    /// Activate Emergency Repair residual: SingleBurst heal of ally vehicles in radius.
    ///
    /// Matches retail SuperweaponEmergencyRepair → RepairVehiclesInArea_InvisibleMarker:
    /// - Radius residual 100 (RadiusCursorRadius / AutoHealBehavior Radius)
    /// - HealingAmount 100/200/300 by level (Level1/2/3)
    /// - KindOf VEHICLE, PartitionFilterRelationship ALLOW_ALLIES, damaged only
    ///
    /// ALLOW_ALLIES is player relationship (same controller or allied players),
    /// not faction `Team` identity.
    ///
    /// Fail-closed: not full OCL marker / science upgrade matrix / RepairCloud particles.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_emergency_repair(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
        level: crate::game_logic::host_emergency_repair::HostEmergencyRepairLevel,
    ) -> bool {
        use crate::game_logic::host_emergency_repair::{
            EMERGENCY_REPAIR_ACTIVATE_AUDIO, HOST_EMERGENCY_REPAIR_RADIUS, HostEmergencyRepair,
            emergency_repair_is_ally, in_emergency_repair_radius_2d,
            is_legal_emergency_repair_target,
        };
        use gamelogic::common::Relationship;

        let frame = self.frame;
        let heal_amount = level.heal_amount();
        let center = (location.x, location.z);

        let caster_team = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .or_else(|| self.players.get(&player_id).map(|p| p.team))
            .unwrap_or(Team::Neutral);
        let caster_owner = caster_id
            .and_then(|cid| self.objects.get(&cid))
            .and_then(|c| self.player_owner_for_host_object(c))
            .or(Some(player_id));

        let candidates: Vec<(ObjectId, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                if !in_emergency_repair_radius_2d(
                    center,
                    (pos.x, pos.z),
                    HOST_EMERGENCY_REPAIR_RADIUS,
                ) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let player_allies = match (caster_owner, self.player_owner_for_host_object(obj)) {
                    (Some(a), Some(b)) => {
                        Some(self.player_relationship(a, b) == Relationship::Allies)
                    }
                    _ => None,
                };
                let is_ally = emergency_repair_is_ally(
                    obj.team == caster_team && caster_team != Team::Neutral,
                    player_allies,
                );
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let is_damaged = obj.health.current + 0.01 < obj.health.maximum;
                Some((*id, is_vehicle, is_ally, under_construction, is_damaged))
            })
            .collect();

        let mut heals: u32 = 0;
        let mut heal_amount_total: f32 = 0.0;
        for (id, is_vehicle, is_ally, under_construction, is_damaged) in candidates {
            if !is_legal_emergency_repair_target(
                is_vehicle,
                true,
                is_ally,
                under_construction,
                is_damaged,
            ) {
                continue;
            }
            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            let before = target.health.current;
            target.heal(heal_amount);
            let restored = (target.health.current - before).max(0.0);
            if restored > 0.01 {
                heals = heals.saturating_add(1);
                heal_amount_total += restored;
            }
        }

        let entry_id = self.emergency_repairs.alloc_id();
        self.emergency_repairs
            .record_activation(HostEmergencyRepair {
                id: entry_id,
                player_id,
                location,
                radius: HOST_EMERGENCY_REPAIR_RADIUS,
                level,
                activate_frame: frame,
                caster_id,
                heals,
                heal_amount_total,
            });

        self.queue_audio_event(
            AudioEventRequest::new(EMERGENCY_REPAIR_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            location,
            frame,
            caster_id,
            None,
        );

        // C++ OCL RepairVehiclesInArea_InvisibleMarker + DeletionUpdate 0 residual.
        // Marker team is visual/faction identity; heal filter used player relationship.
        let _ = self.spawn_emergency_repair_marker(caster_team, location, level);

        true
    }

    /// Host GPS Scrambler residual registry (activate + honesty).
    pub fn gps_scramblers(
        &self,
    ) -> &crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry {
        &self.gps_scramblers
    }

    /// Residual honesty: GPS Scrambler activated at least once.
    pub fn honesty_gps_scrambler_activate_ok(&self) -> bool {
        self.gps_scramblers.honesty_activate_ok()
    }

    /// Residual honesty: GPS Scrambler granted stealth at least once.
    pub fn honesty_gps_scrambler_grant_ok(&self) -> bool {
        self.gps_scramblers.honesty_grant_ok()
    }

    /// Combined host path honesty for GPS Scrambler residual.
    pub fn honesty_gps_scrambler_ok(&self) -> bool {
        self.gps_scramblers.honesty_host_path_ok()
    }
}
