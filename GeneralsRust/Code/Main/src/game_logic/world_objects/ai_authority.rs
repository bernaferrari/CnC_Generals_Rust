//! Host objects `impl GameLogic` — `ai_authority`.
//! AI behavior, stop_attack, health authority write. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Process AI behavior for a single object
    /// Enhanced with proper enemy detection, attack decisions, and movement
    pub(in super::super) fn process_ai_behavior(
        &mut self,
        object_id: ObjectId,
        ai_state: AIState,
        target_id: Option<ObjectId>,
        position: Vec3,
        team: Team,
        can_attack: bool,
        frame: u32,
        dt: f32,
    ) -> Option<AICommand> {
        let should_scan =
            |interval: u32| -> bool { interval > 0 && frame.is_multiple_of(interval) };
        let ai_auto_engage_paused = self.skirmish_ai_auto_engage_paused(team);

        if matches!(ai_state, AIState::AttackMoving) {
            if self
                .objects
                .get(&object_id)
                .is_some_and(|o| o.is_out_of_special_reload_ammo())
            {
                if let Some(o) = self.objects.get_mut(&object_id) {
                    o.return_to_base_requested = true;
                    o.is_attack_path = false;
                }
                return Some(AICommand::SetAIState {
                    object_id,
                    state: AIState::Idle,
                });
            }
            // C++ nested AIAttackMoveStateMachine is not idle while a victim
            // is held: do not getNextMoodTarget or re-issue AttackTarget.
            if target_id.is_some() {
                return None;
            }
            // C++ AIAttackMoveToState uses getNextMoodTarget, not a 200wu nearest scan.
            if can_attack && !ai_auto_engage_paused && should_scan(20) {
                let is_player = self
                    .objects
                    .get(&object_id)
                    .and_then(|o| o.owner_player_id)
                    .and_then(|pid| self.players.get(&pid).map(|p| p.is_local))
                    .unwrap_or(false);
                if let Some(enemy_id) = self.get_next_mood_target(object_id, true, false, is_player)
                {
                    // C++ AIAttackMoveToState::update: friend_endingMove +
                    // setGoalObject + AI_ATTACK_OBJECT. No Hold / FindNewTarget
                    // second opinion (hq-6p7c2).
                    return Some(AICommand::AttackTarget {
                        object_id,
                        target_id: enemy_id,
                    });
                }
            }
            return self.tick_attack_move_blocked_progress(object_id, frame);
        }

        if self.tick_persisted_face(object_id, target_id, dt, frame) {
            if matches!(ai_state, AIState::FacingObject | AIState::FacingPosition) {
                // C++ AIFaceState SUCCESS → AI_IDLE. Apply host-immediate so
                // decision-authority log-only SetAIState cannot leave FACE stuck.
                if let Some(u) = self.objects.get_mut(&object_id) {
                    u.set_ai_state(AIState::Idle);
                }
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(object_id, 0);
                }
                return None;
            }
        } else if matches!(ai_state, AIState::FacingObject | AIState::FacingPosition) {
            return None;
        }

        // C++ AIAttackState / AIIdleState never auto-retreat on low HP.
        // Mood targeting (getNextMoodTarget) only issues aiAttackObject.

        match ai_state {
            AIState::Idle => {
                // C++ AIIdleState::update: mood acquire only (`try_mood_auto_acquire`).
                // Do not invent a 200-radius scan that bypasses Sleep/Passive/AutoAcquire.
                let _ = (can_attack, ai_auto_engage_paused, should_scan);
                None
            }

            AIState::GuardRetaliating | AIState::Attacking => {
                use crate::ai_decisions::{AIDecisionSystem, AttackDecision};

                let Some(current_target_id) = target_id else {
                    return Some(AICommand::StopAttack { object_id });
                };

                // Paused skirmish AI: do not chase new targets; drop combat.
                if ai_auto_engage_paused {
                    return Some(AICommand::StopAttack { object_id });
                }

                match AIDecisionSystem::should_attack(self, object_id, current_target_id) {
                    AttackDecision::Attack | AttackDecision::Hold | AttackDecision::Retreat => None,
                    AttackDecision::FindNewTarget => {
                        if !can_attack {
                            return Some(AICommand::StopAttack { object_id });
                        }
                        AIDecisionSystem::find_best_target(
                            self, object_id, position, team, 200.0, true, true, false,
                        )
                        .map(|target_id| AICommand::AttackTarget {
                            object_id,
                            target_id,
                        })
                        .or(Some(AICommand::StopAttack { object_id }))
                    }
                }
            }

            AIState::AttackMoving => None,

            AIState::Moving => None,

            AIState::Patrolling => {
                // C++ AIHuntState::update — map-wide seek-and-destroy.
                // Leftover hunt.rs:257-259 / C++ AIHuntState::update: empty
                // clip (Raptor/MiG ReturnToBase) is STATE_FAILURE → RTB.
                if self.objects.get(&object_id).is_some_and(|o| {
                    o.is_out_of_ammo() && !o.is_kind_of(crate::game_logic::KindOf::Projectile)
                }) {
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        // C++ JetAIUpdate intercepts hunt STATE_FAILURE.
                        if o.is_out_of_special_reload_ammo()
                            || o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        {
                            o.return_to_base_requested = true;
                            o.is_attack_path = false;
                        }
                        o.hunting = false;
                        o.release_weapon_lock(crate::game_logic::WeaponLockType::LockedTemporarily);
                    }
                    self.hunt_next_enemy_scan.remove(&object_id);
                    return Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Idle,
                    });
                }

                // C++ AIHuntState::update scans with no isAbleToAttack gate.
                // Scan clock is per-unit jitter, not global frame%30.
                if !ai_auto_engage_paused && self.hunt_acquire_scan_due(object_id, frame) {
                    let units_should_hunt = self.object_units_should_hunt(object_id);
                    let has_priority = self.attack_priority_info_for(object_id).is_some();
                    let team_victim = self.host_team_common_target(object_id);
                    let victim = if team_victim.is_some() && !has_priority {
                        team_victim
                    } else {
                        let mut scanned = self.find_closest_enemy(
                            object_id,
                            9999.9,
                            crate::game_logic::find_enemy_flags::CAN_ATTACK,
                        );
                        if scanned.is_none() && units_should_hunt {
                            scanned = self.find_closest_enemy_ignoring_priority(
                                object_id,
                                9999.9,
                                crate::game_logic::find_enemy_flags::CAN_ATTACK,
                            );
                        }
                        if let (Some(team_id), Some(scanned_id)) = (team_victim, scanned) {
                            if has_priority {
                                let team_pri = self
                                    .objects
                                    .get(&team_id)
                                    .and_then(|t| {
                                        self.attack_priority_info_for(object_id)
                                            .map(|info| self.attack_priority_for_target(info, t))
                                    })
                                    .unwrap_or(0);
                                let scan_pri = self
                                    .objects
                                    .get(&scanned_id)
                                    .and_then(|t| {
                                        self.attack_priority_info_for(object_id)
                                            .map(|info| self.attack_priority_for_target(info, t))
                                    })
                                    .unwrap_or(0);
                                if team_pri >= scan_pri {
                                    Some(team_id)
                                } else {
                                    Some(scanned_id)
                                }
                            } else {
                                scanned
                            }
                        } else if scanned.is_none() {
                            team_victim
                        } else {
                            scanned
                        }
                    };
                    if team_victim.is_none() || has_priority {
                        // C++ writes setTeamTargetObject every hunt scan when
                        // attackCommonTarget (refresh or clear).
                        if self.team_wants_common_attack(object_id) {
                            self.set_host_team_common_target(object_id, victim);
                        }
                    }
                    // C++ AIHuntState::update: setGoalObject + AI_ATTACK_OBJECT.
                    // No Hold / should_attack wrap (hq-q7xo1). Attack-object paths
                    // to the map-wide victim the same way attack-move does (hq-6p7c2).
                    if let Some(enemy_id) = victim {
                        return Some(AICommand::AttackTarget {
                            object_id,
                            target_id: enemy_id,
                        });
                    }
                    if !units_should_hunt && target_id.is_none() {
                        if let Some(o) = self.objects.get_mut(&object_id) {
                            o.hunting = false;
                            o.release_weapon_lock(
                                crate::game_logic::WeaponLockType::LockedTemporarily,
                            );
                        }
                        self.hunt_next_enemy_scan.remove(&object_id);
                        return Some(AICommand::SetAIState {
                            object_id,
                            state: AIState::Idle,
                        });
                    }
                }
                None
            }

            AIState::GuardingArea | AIState::GuardingObject => {
                // Guarding states are resolved in update_support_states() where guard anchors/radii
                // and target legality checks are available.
                None
            }

            AIState::Gathering => {
                // Resource gathering behavior: move to supply pile, collect, return to refinery.
                // This autonomous behavior just monitors state — actual resource accumulation
                // happens in the update loop via a separate phase.
                let gather_target_id = target_id;

                if let Some(source_id) = gather_target_id {
                    if let Some(source_obj) = self.objects.get(&source_id) {
                        let dist_to_source = position.distance(source_obj.get_position());
                        if dist_to_source > 15.0 {
                            // Still moving toward the resource — keep going
                            return Some(AICommand::MoveTo {
                                object_id,
                                position: source_obj.get_position(),
                            });
                        }
                        // Close enough — the update loop handles accumulation.
                        // Check if full (stored_resources checked in update phase).
                        None
                    } else {
                        // Resource source no longer exists — go idle
                        Some(AICommand::SetAIState {
                            object_id,
                            state: AIState::Idle,
                        })
                    }
                } else {
                    Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Idle,
                    })
                }
            }

            AIState::Constructing | AIState::Repairing => {
                // Building or repairing - continue current task
                None
            }

            AIState::Docked | AIState::Garrisoned => {
                // Unit is inside another structure - no autonomous behavior
                None
            }

            AIState::AttackingGround => {
                // Artillery-style ground attack
                // Continue until command is cancelled
                None
            }

            AIState::SpecialAbility => {
                // Unit is using special ability
                // Continue until ability completes
                None
            }

            AIState::SeekingRepair => {
                // Unit is looking for repair facility
                // Would pathfind to nearest repair bay
                None
            }

            AIState::SeekingHealing => {
                // Unit is looking for medical facility
                // Would pathfind to nearest medical center
                None
            }

            AIState::Entering => {
                // Unit is entering a transport or garrison
                None
            }

            AIState::Docking => {
                // Unit is docking with a structure (harvester to refinery, etc)
                None
            }

            AIState::ReturningResources => {
                // C++ SupplyTruckWantsToPickUpOrDeliverBoxesState::update
                // (SupplyTruckAIUpdate.cpp:487-530): a loaded truck does NOT
                // issue a raw aiMoveToPosition at the center.  It resolves the
                // depot through ResourceGatheringManager::findBestSupplyCenter
                // — m_preferredDock wins when remotely okay (computeRelativeCost
                // != FLT_MAX), otherwise nearest — then enters AIDock, whose
                // DockUpdate drives the approach.  Mirror that here:
                // resolve the depot (persisted dock first) and steer toward its
                // exact position; the support-states deposit arm owns claim +
                // deposit once within range.  The old raw MoveTo{center} from
                // this layer preempted that arm every tick and snapped the goal
                // off the exact center position (hq-mik7s).
                let owner_player_id = self
                    .objects
                    .get(&object_id)
                    .and_then(|object| self.player_owner_for_host_object(object));
                if let Some(center_id) = self.preferred_supply_center_or_nearest(
                    object_id,
                    team,
                    owner_player_id,
                    position,
                ) {
                    if let Some(dest) =
                        self.objects.get(&center_id).map(|c| c.get_position())
                    {
                        if position.distance(dest)
                            > crate::game_logic::host_repair::DOZER_MIN_ACTION_TOLERANCE
                        {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                    }
                } else {
                    // No reachable friendly center — C++ WantingState returns
                    // STATE_FAILURE and the truck regroups; go idle.
                    return Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Idle,
                    });
                }
                None
            }

            AIState::Capturing => {
                // Unit is capturing enemy structure
                // Continue until capture completes
                None
            }

            AIState::FacingObject | AIState::FacingPosition => None,
        }
    }

    /// Leftover `AIFaceState::update` + ANGLE `locoUpdate_moveTowardsAngle`.
    /// Returns true when Face just succeeded or the goal vanished.
    fn tick_persisted_face(
        &mut self,
        object_id: ObjectId,
        target_id: Option<ObjectId>,
        dt: f32,
        frame: u32,
    ) -> bool {
        let (active, facing_object, face_pos, ai) = match self.objects.get(&object_id) {
            Some(o) => (
                o.face_active,
                matches!(o.ai_state, AIState::FacingObject),
                o.face_goal_pos,
                o.ai_state.clone(),
            ),
            None => return false,
        };
        if !active && !matches!(ai, AIState::FacingObject | AIState::FacingPosition) {
            return false;
        }
        let target_pos = if facing_object || (active && face_pos.is_none()) {
            let Some(tid) = target_id else {
                if let Some(u) = self.objects.get_mut(&object_id) {
                    u.face_active = false;
                    u.set_locomotor_goal_none();
                }
                return true;
            };
            match self.objects.get(&tid).map(|o| o.get_position()) {
                Some(p) => p,
                None => {
                    if let Some(u) = self.objects.get_mut(&object_id) {
                        u.face_active = false;
                        u.set_locomotor_goal_none();
                    }
                    return true;
                }
            }
        } else if let Some(p) = face_pos {
            p
        } else {
            if let Some(u) = self.objects.get_mut(&object_id) {
                u.face_active = false;
                u.set_locomotor_goal_none();
            }
            return true;
        };
        let Some(u) = self.objects.get_mut(&object_id) else {
            return false;
        };
        !u.tick_face_towards(target_pos, dt, frame)
    }

    /// Apply AI command to the game state
    /// Engage a target, honoring AI decision authority (log-only when GameWorld applies).
    ///
    /// Player command paths should call [`Object::attack_target`] directly so orders
    /// apply same-frame without waiting for shadow writeback.
    /// Clear engagement, honoring AI decision authority (log-only when GameWorld applies).
    ///
    /// Player `command_stop` should call [`Object::stop_attack`] directly for same-frame UX.
    fn object_units_should_hunt(&self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        obj.owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|p| p.units_should_hunt)
            .unwrap_or(false)
    }

    fn find_closest_enemy_ignoring_priority(
        &mut self,
        unit_id: ObjectId,
        range: f32,
        qualifiers: u32,
    ) -> Option<ObjectId> {
        let saved = self
            .objects
            .get_mut(&unit_id)
            .and_then(|o| o.attack_priority_set.take());
        let found = self.find_closest_enemy(unit_id, range, qualifiers);
        if let Some(o) = self.objects.get_mut(&unit_id) {
            o.attack_priority_set = saved;
        }
        found
    }

    pub(in super::super) fn stop_attack_decision_aware(&mut self, unit_id: ObjectId) {
        // Always clear host combat engagement immediately so mid-frame fire stops.
        // Log under decision authority for GameWorld last-write parity.
        self.drop_jet_targeters_on_attack_exit(unit_id);
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.stop_attack();
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
        }
    }

    /// Clear engagement target, honoring AI decision authority (StopAttack channel).
    ///
    /// Prefer this over raw `set_target(None)` when the target is combat engagement
    /// (not harvest/heal/construction non-combat associations that stay host-owned).
    pub(in super::super) fn clear_target_decision_aware(&mut self, unit_id: ObjectId) {
        // Combat engagement clear is host-immediate; non-combat associations should
        // call set_target(None) directly without this helper.
        self.remove_self_as_jet_targeter_from_current_victim(unit_id);
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.set_target(None);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
        }
    }

    /// Absolute HP write honoring damage authority (heal channel last-writer).
    pub(in super::super) fn set_health_absolute_authority_aware(
        &mut self,
        object_id: ObjectId,
        health: f32,
    ) {
        let hp = health.max(0.0);
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_heal_log::record(object_id, hp);
            return;
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.health.current = hp.min(obj.health.maximum.max(hp));
            crate::game_logic::host_heal_log::record(object_id, obj.health.current);
        }
    }

    /// Absolute HP write while holding `&mut Object` (avoid re-borrow).
    pub(in super::super) fn write_object_health_authority_aware(
        obj: &mut crate::game_logic::Object,
        health: f32,
    ) {
        let hp = health.max(0.0);
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_heal_log::record(obj.id, hp);
        } else {
            obj.health.current = hp.min(obj.health.maximum.max(hp));
            crate::game_logic::host_heal_log::record(obj.id, obj.health.current);
        }
    }

    /// Consume/suicide destroy residual: log lethal HP under damage authority.
    /// Destroy flag stays host for process_destroy_list bookkeeping.
    pub(in super::super) fn mark_destroyed_authority_aware(
        &mut self,
        object_id: ObjectId,
        source: Option<ObjectId>,
    ) {
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                let hp = obj.health.current.max(1.0);
                crate::game_logic::host_damage_log::record(object_id, hp, source, true);
            } else if obj.health.current > 0.0 {
                obj.health.current = 0.0;
            }
            obj.status.destroyed = true;
        }
    }

    /// Same as mark_destroyed_authority_aware while holding `&mut Object`.
    pub(in super::super) fn mark_object_destroyed_authority_aware(
        obj: &mut crate::game_logic::Object,
        source: Option<ObjectId>,
    ) {
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = obj.health.current.max(1.0);
            crate::game_logic::host_damage_log::record(obj.id, hp, source, true);
        } else if obj.health.current > 0.0 {
            obj.health.current = 0.0;
        }
        obj.status.destroyed = true;
    }

    /// Residual auto-fire damage/spawn dual path:
    /// - FIRE_SPAWN_AUTHORITY on (default): queue projectile (shadow owns spawn;
    ///   same-frame drain in update_simulation applies damage). Skip host hitscan.
    /// - off: keep host hitscan take_damage_from for frame-local residual honesty.
    pub(in super::super) fn residual_auto_fire_apply_damage(
        &mut self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        damage: f32,
        shooter_pos: glam::Vec3,
        weapon: Option<&crate::game_logic::Weapon>,
        slot: u8,
    ) -> (bool, f32) {
        use crate::game_logic::combat::{self, DamageType, PendingProjectile};
        use crate::game_logic::host_usa_pilot::HostDeathType;

        // Presentation AttackTargeted residual (HUD observe only — C++ plays no
        // SFX on an attack order; per-shot audio is the authored FireSound via
        // FiringTracker::shotFired).
        crate::game_logic::host_attack_log::record(attacker_id, Some(target_id));

        // Fire *decision* residual: under AI_DECISION_AUTHORITY, emit AttackTarget so
        // GameWorld last-writes host engagement target (parity with fire_at_ex).
        // Host still *chooses* the residual target; this peels the decision channel.
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(attacker_id, target_id);
            crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 2);
            // Attacking
        }

        // Fire-spawn channel residual: under FIRE_SPAWN_AUTHORITY, log a projectile
        // spawn carrying live damage (parity with fire_at). Hitscan below still
        // owns same-frame residual HP so update_combat-only honesty stays green
        // for both instant and ballistic residual weapons; residual-hitscan pairs
        // are marked so shadow zeroes spawn damage (no dual-tick double-dip).
        // Ballistic projectile-owned residual HP (no hitscan) remains deferred until
        // dual-tick tests drive shadow materialize+resolve after host combat.
        if crate::gameworld_shadow::gameworld_fire_spawn_authority_live() {
            // Even this alternate auto-fire path must retain the real
            // Weapon.ini presentation references. C++ carries them on the
            // WeaponTemplate selected for the firing object; do not infer an
            // effect from the weapon's category when the host shadow later
            // materializes this pending projectile.
            let (projectile_object_name, fire_fx_name, fire_ocl_name, detonation_fx_name, detonation_ocl_name, exhaust_name) = self
                .objects
                .get(&attacker_id)
                .and_then(|attacker| {
                    let veterancy = attacker.experience.level;
                    attacker.weapon_name_for_slot(slot).map(|weapon_name| {
                        (
                            crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name(
                                weapon_name,
                            ),
                            crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                            crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                            crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                            crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                            crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_weapon_name_at_veterancy(
                                weapon_name,
                                veterancy,
                            ),
                        )
                    })
                })
                .unwrap_or_default();
            let (speed, splash, homing, dtype, attack_range, min_attack_range) = match weapon {
                Some(w) => {
                    let speed = if w.projectile_speed <= 0.0 {
                        999_000.0
                    } else {
                        w.projectile_speed
                    };
                    let homing = w.can_target_air && !w.can_target_ground;
                    let dtype = if speed >= 999_000.0 {
                        DamageType::Laser
                    } else if w.splash_radius > 0.0 {
                        DamageType::Explosive
                    } else {
                        DamageType::Bullet
                    };
                    (speed, w.splash_radius, homing, dtype, w.range, w.min_range)
                }
                None => (999_000.0, 0.0, false, DamageType::Laser, 0.0, 0.0),
            };
            combat::queue_projectile(PendingProjectile {
                shooter_id: attacker_id,
                shooter_pos,
                source_context: self.objects.get(&attacker_id).map(|attacker| {
                    combat::ProjectileLaunchContext {
                        source_team: attacker.team,
                        source_owner_player_id: attacker.owner_player_id,
                        source_veterancy: attacker.experience.level,
                        source_orientation: attacker.get_orientation(),
                        source_velocity: attacker.movement.velocity,
                    }
                }),
                target_id: Some(target_id),
                target_pos: self.objects.get(&target_id).map(|t| t.get_position()),
                damage,
                speed,
                splash_radius: splash,
                is_homing: homing,
                damage_type: dtype,
                death_type: HostDeathType::Normal,
                projectile_object_name,
                projectile_lifecycle: None,
                fire_fx_name,
                fire_ocl_name,
                detonation_fx_name,
                detonation_ocl_name,
                exhaust_name,
                secondary_damage: 0.0,
                secondary_damage_radius: 0.0,
                shock_wave_amount: 0.0,
                shock_wave_radius: 0.0,
                shock_wave_taper_off: 0.0,
                radius_damage_affects: 0,
                projectile_collides: 0,
                scatter_radius: 0.0,
                scatter_table_offset: None,
                min_weapon_speed: 0.0,
                scale_weapon_speed: false,
                attack_range,
                min_attack_range,
                historic_weapon_key: String::new(),
                historic_bonus_time_frames: 0,
                historic_bonus_count: 0,
                historic_bonus_radius: 0.0,
                historic_bonus_weapon: String::new(),

                die_on_detonate: self
                    .objects
                    .get(&attacker_id)
                    .and_then(|attacker| attacker.weapon_name_for_slot(slot))
                    .map(crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name)
                    .unwrap_or(false),
            });
        }

        let mut destroyed = false;
        let mut kill_xp = 0.0;
        if let Some(target) = self.objects.get_mut(&target_id) {
            // Source-attributed residual: BodyModule last_damage_source + damage log.
            destroyed = target.take_damage_from(damage, Some(attacker_id));
            if crate::gameworld_shadow::gameworld_fire_spawn_authority_live() {
                crate::game_logic::host_fire_spawn_log::record_residual_hitscan(
                    attacker_id,
                    target_id,
                );
            }
            if destroyed {
                kill_xp = target.kill_experience_value();
            }
        }
        (destroyed, kill_xp)
    }

    #[cfg(test)]
    pub fn stop_attack_decision_aware_for_test(&mut self, unit_id: ObjectId) {
        self.stop_attack_decision_aware(unit_id);
    }

    /// Set engagement (target + Attacking), honoring AI decision authority.
    ///
    /// Residual fire paths should use this instead of raw `target = Some` so
    /// GameWorld apply/writeback is last-writer when authority is on. Does not
    /// invoke full [`Object::attack_target`] (avoids takeoff/force-attack side effects).
    /// Set AI state, honoring AI decision authority (log-only when GameWorld applies).
    pub(in super::super) fn set_ai_state_decision_aware(
        &mut self,
        unit_id: ObjectId,
        state: AIState,
    ) {
        // Host applies immediately so residual FSM/combat sees the new state
        // same-frame. Decision authority still logs for GameWorld last-write.
        let ordinal = crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_ai_state(state);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, ordinal);
        }
    }

    #[cfg(test)]
    pub fn set_ai_state_decision_aware_for_test(&mut self, unit_id: ObjectId, state: AIState) {
        self.set_ai_state_decision_aware(unit_id, state);
    }

    /// C++ `AIAttackMoveToState::update` retry / 3s-sleep / close-enough
    /// (AIStates.cpp:3631-3656). Mood engage already ran this frame.
    fn tick_attack_move_blocked_progress(
        &mut self,
        object_id: ObjectId,
        frame: u32,
    ) -> Option<AICommand> {
        // C++ ATTACK_RETRY_COUNT=5, ATTACK_CLOSE_ENOUGH_CELLS=8,
        // PATHFIND_CELL_SIZE_F=10, 3*LOGICFRAMES_PER_SECOND=90.
        const CLOSE_ENOUGH: f32 = 8.0 * 10.0;
        const SLEEP_FRAMES: u32 = 90;

        let Some(obj) = self.objects.get(&object_id) else {
            return None;
        };
        let sleep_until = obj.attack_move_sleep_until;
        let dest = obj.requested_destination;
        let pos = obj.get_position();
        let retry = obj.attack_move_retry_count;
        let waiting = obj.waiting_for_path;
        let has_move_goal = obj.movement.target_position.is_some();

        if sleep_until > frame {
            if let Some(o) = self.objects.get_mut(&object_id) {
                o.movement.velocity = Vec3::ZERO;
                o.set_status_moving(false);
            }
            return None;
        }

        let mut moving = waiting || has_move_goal;
        if sleep_until == frame {
            if let Some(goal) = dest {
                moving = self.assign_unit_path(object_id, goal, &[]);
                if let Some(o) = self.objects.get_mut(&object_id) {
                    if o.is_attack_path {
                        o.set_ai_state(AIState::AttackMoving);
                    }
                }
            }
        }

        if moving {
            return None;
        }
        let Some(goal) = dest else {
            return None;
        };
        let dx = pos.x - goal.x;
        let dz = pos.z - goal.z;
        let dist_sqr = dx * dx + dz * dz;
        if dist_sqr < CLOSE_ENOUGH * CLOSE_ENOUGH || retry < 1 {
            return Some(AICommand::SetAIState {
                object_id,
                state: AIState::Idle,
            });
        }
        if let Some(o) = self.objects.get_mut(&object_id) {
            o.attack_move_retry_count = retry - 1;
            o.attack_move_sleep_until = frame.saturating_add(SLEEP_FRAMES);
            o.movement.velocity = Vec3::ZERO;
            o.movement.target_position = None;
            o.set_status_moving(false);
        }
        None
    }

    /// C++ AIAttackApproachTargetState: computer players do not chase an
    /// airborne aircraft unless the parent state is AI_HUNT.
    pub(crate) fn computer_refuses_non_hunt_airborne_chase(
        &self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get(&unit_id) else {
            return false;
        };
        if unit.hunting {
            return false;
        }
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        if !target.is_kind_of(KindOf::Aircraft) || !target.status.airborne_target {
            return false;
        }
        unit.owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|p| !p.is_local)
            .unwrap_or(false)
    }

    pub(in super::super) fn apply_engagement_decision_aware(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        // C++ AIAttackState / AIGuard do not make an arbitrary visible object
        // a goal.  They validate the concrete WeaponSet first.  This is the
        // final authority boundary for all host AI engagement producers,
        // including guards and skirmish decisions.
        if !matches!(
            self.get_able_to_attack_specific_object(
                unit_id,
                target_id,
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        if self.computer_refuses_non_hunt_airborne_chase(unit_id, target_id) {
            return false;
        }
        // Host engagement is same-frame so residual auto-fire / continue-after-kill
        // can shoot without waiting for shadow writeback.
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_target(Some(target_id));
            // C++ Hunt stays in AI_HUNT and Attack-Move stays in
            // AI_ATTACK_MOVE_TO while the nested attack machine runs.
            // Combat already fires from both parent states.
            if !matches!(u.ai_state, AIState::Patrolling | AIState::AttackMoving) {
                u.set_ai_state(AIState::Attacking);
            }
            u.set_status_attacking(true);
            if matches!(u.ai_state, AIState::AttackMoving) {
                // C++ friend_endingMove + setLocomotorGoalNone. Dest/path stay.
                u.movement.velocity = Vec3::ZERO;
                u.set_status_moving(false);
            }
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
            if !matches!(
                self.objects.get(&unit_id).map(|o| o.ai_state.clone()),
                Some(AIState::Patrolling | AIState::AttackMoving)
            ) {
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            }
        }
        true
    }

    #[cfg(test)]
    pub fn apply_engagement_decision_aware_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) {
        let _ = self.apply_engagement_decision_aware(unit_id, target_id);
    }

    /// AI / skirmish manager entry: host-immediate engagement + decision log.
    pub fn apply_engagement_decision_aware_for_ai(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        self.apply_engagement_decision_aware(unit_id, target_id)
    }

    /// AI / skirmish manager entry: host-immediate AI state + decision log.
    pub fn set_ai_state_decision_aware_for_ai(&mut self, unit_id: ObjectId, state: AIState) {
        self.set_ai_state_decision_aware(unit_id, state);
    }

    pub(in super::super) fn engage_target_decision_aware(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        if !matches!(
            self.get_able_to_attack_specific_object(
                unit_id,
                target_id,
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        // Full host attack_target residual (weapon arming / force-attack clear).
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.set_force_attack(false);
            obj.attack_target(target_id);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        true
    }

    #[cfg(test)]
    pub fn engage_target_decision_aware_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) {
        let _ = self.engage_target_decision_aware(unit_id, target_id);
    }

    pub(in super::super) fn apply_ai_command(&mut self, command: AICommand) {
        // Host applies immediately so AI aggression/combat is same-frame.
        // Decision authority still logs every command for GameWorld last-write.
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        match command {
            AICommand::AttackTarget {
                object_id,
                target_id,
            } => {
                let _ = self.apply_engagement_decision_aware(object_id, target_id);
                self.set_host_team_common_target(object_id, Some(target_id));
            }
            AICommand::StopAttack { object_id } => {
                // stop_attack_decision_aware clears host + logs.
                self.stop_attack_decision_aware(object_id);
            }
            AICommand::MoveTo {
                object_id,
                position,
            } => {
                if decision_auth {
                    crate::game_logic::host_ai_decision_log::record_move_to(object_id, position);
                }
                // Pathfinding stays host-side (movement authority peels integrate separately).
                self.move_object_with_pathfinding(object_id, position, None);
            }
            AICommand::SetAIState { object_id, state } => {
                // Player Hunt (`hunting`) must survive transient Idle so the
                // parent Hunt machine can rescan. Stop/Guard clear the flag.
                self.set_ai_state_decision_aware(object_id, state);
            }
        }
    }

    /// C++ AIHuntState `m_nextEnemyScanTime`. First visit matches onEnter
    /// `now + GameLogicRandomValue(0, ENEMY_SCAN_RATE)`; later scans add 30.
    fn hunt_acquire_scan_due(&mut self, object_id: ObjectId, now: u32) -> bool {
        const RATE: u32 = 30;
        match self.hunt_next_enemy_scan.get(&object_id).copied() {
            Some(next) if now < next => return false,
            None => {
                let offset = gamelogic::helpers::game_logic_random_value(0, RATE);
                let next = now.saturating_add(offset);
                if now < next {
                    self.hunt_next_enemy_scan.insert(object_id, next);
                    return false;
                }
            }
            Some(_) => {}
        }
        self.hunt_next_enemy_scan
            .insert(object_id, now.saturating_add(RATE));
        true
    }

    #[cfg(test)]
    pub fn apply_ai_command_for_test(&mut self, command: AICommand) {
        self.apply_ai_command(command);
    }
}

#[cfg(test)]
mod hq_m6gcj_tests {
    use super::*;

    /// C++ `AIIdleState::update` (AIStates.cpp) never transitions to `AI_HUNT`.
    /// Pre-fix: `frame % 300 == object_id % 300` flipped Idle → Patrolling.
    #[test]
    fn idle_units_hold_position_without_hunt_flip() {
        let mut logic = GameLogic::new();
        let object_id = ObjectId(300);
        let command = logic.process_ai_behavior(
            object_id,
            AIState::Idle,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            300,
            1.0 / 30.0,
        );
        assert!(
            !matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Patrolling,
                    ..
                })
            ),
            "AIIdleState must not flip Idle to Patrolling/hunt; got {command:?}"
        );
        assert!(
            command.is_none(),
            "idle with no mood target holds position; got {command:?}"
        );
    }

    /// C++ `AIAttackState` never auto-retreats at 30% HP.
    #[test]
    fn attacking_does_not_auto_retreat_without_target_change() {
        let mut logic = GameLogic::new();
        let object_id = ObjectId(1);
        let target_id = ObjectId(2);
        let command = logic.process_ai_behavior(
            object_id,
            AIState::Attacking,
            Some(target_id),
            Vec3::ZERO,
            Team::USA,
            true,
            0,
            1.0 / 30.0,
        );
        assert!(
            !matches!(command, Some(AICommand::MoveTo { .. })),
            "AIAttackState must not emit retreat MoveTo; got {command:?}"
        );
    }

    #[test]
    fn hunt_without_victim_exits_when_units_should_hunt_false() {
        let mut logic = GameLogic::new();
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 30);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            30,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "regular Hunt exits to Idle after map-clear; got {command:?}"
        );
    }

    #[test]
    fn weaponless_hunt_still_scans_and_exits() {
        // C++ AIHuntState::update has no isAbleToAttack gate (hq-grlpr).
        let mut logic = GameLogic::new();
        let mut worker = Object::new(ThingTemplate::new("Worker"), ObjectId(1), Team::USA);
        worker.hunting = true;
        worker.set_ai_state(AIState::Patrolling);
        logic.objects.insert(worker.id, worker);
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 30);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            false,
            30,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "weaponless Hunt must still scan and exit to Idle; got {command:?}"
        );
    }

    #[test]
    fn hunt_without_victim_stays_when_units_should_hunt() {
        let mut logic = GameLogic::new();
        let mut player = Player::new(1, Team::USA, "USA AI", false);
        player.units_should_hunt = true;
        logic.players.insert(1, player);
        let mut hunter = Object::new(ThingTemplate::new("Hunter"), ObjectId(1), Team::USA);
        hunter.owner_player_id = Some(1);
        hunter.hunting = true;
        hunter.set_ai_state(AIState::Patrolling);
        logic.objects.insert(hunter.id, hunter);
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 30);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            30,
            1.0 / 30.0,
        );
        assert!(
            !matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "PLAYER_HUNT stays in hunt with no victims; got {command:?}"
        );
    }

    #[test]
    fn attack_moving_jet_out_of_special_reload_ammo_returns_to_base() {
        let mut logic = GameLogic::new();
        let mut jet = Object::new(
            ThingTemplate::new("AmericaJetRaptor"),
            ObjectId(7),
            Team::USA,
        );
        jet.weapon = Some(Weapon {
            ammo: Some(0),
            clip_size: 4,
            ..Weapon::default()
        });
        jet.thing.template.primary_weapon_name = Some("RaptorMissileWeapon".to_string());
        jet.set_ai_state(AIState::AttackMoving);
        jet.is_attack_path = true;
        logic.objects.insert(jet.id, jet);
        let command = logic.process_ai_behavior(
            ObjectId(7),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            0,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "empty-clip jet must leave attack-move; got {command:?}"
        );
        let jet = logic.objects.get(&ObjectId(7)).expect("jet");
        assert!(jet.return_to_base_requested);
        assert!(!jet.is_attack_path);
    }

    #[test]
    fn idle_auto_acquire_requires_yes_bit() {
        use crate::game_logic::AbleToAttackType;
        let mut logic = GameLogic::new();
        let mut idle = Object::new(ThingTemplate::new("IdleScout"), ObjectId(1), Team::USA);
        idle.auto_acquire_idle_bits = 0;
        idle.auto_acquire_when_idle = false;
        idle.set_ai_state(AIState::Idle);
        idle.weapon = Some(Weapon {
            range: 200.0,
            damage: 10.0,
            ..Weapon::default()
        });
        let mut enemy = Object::new(ThingTemplate::new("Enemy"), ObjectId(2), Team::GLA);
        enemy.set_position(Vec3::new(50.0, 0.0, 0.0));
        logic.objects.insert(idle.id, idle);
        logic.objects.insert(enemy.id, enemy);
        assert!(
            logic
                .get_next_mood_target(ObjectId(1), true, true, true)
                .is_none()
        );
        if let Some(o) = logic.objects.get_mut(&ObjectId(1)) {
            o.auto_acquire_idle_bits =
                gamelogic::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE;
            o.auto_acquire_when_idle = true;
        }
        let _ = AbleToAttackType::NewTarget;
        let _ = logic.get_next_mood_target(ObjectId(1), true, true, true);
    }

    #[test]
    fn attack_move_with_victim_does_not_rescan() {
        let mut logic = GameLogic::new();
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            Some(ObjectId(2)),
            Vec3::ZERO,
            Team::USA,
            true,
            20,
            1.0 / 30.0,
        );
        assert!(
            command.is_none(),
            "nested attack-move must not re-issue AttackTarget; got {command:?}"
        );
    }

    #[test]
    fn attack_move_stop_attack_keeps_parent_and_dest() {
        let dest = Vec3::new(120.0, 0.0, 80.0);
        let mut unit = Object::new(ThingTemplate::new("Crusader"), ObjectId(1), Team::USA);
        unit.set_ai_state(AIState::AttackMoving);
        unit.is_attack_path = true;
        unit.requested_destination = Some(dest);
        unit.movement.target_position = Some(dest);
        unit.target = Some(ObjectId(2));
        unit.status.attacking = true;
        unit.stop_attack();
        assert_eq!(unit.ai_state, AIState::AttackMoving);
        assert!(unit.target.is_none());
        assert_eq!(unit.requested_destination, Some(dest));
        assert_eq!(unit.movement.target_position, Some(dest));
        assert!(unit.is_attack_path);
    }

    #[test]
    fn attack_move_engagement_does_not_peel_parent() {
        let dest = Vec3::new(200.0, 0.0, 0.0);
        let mut logic = GameLogic::new();
        let mut tank = Object::new(ThingTemplate::new("Crusader"), ObjectId(1), Team::USA);
        tank.weapon = Some(Weapon {
            range: 150.0,
            damage: 10.0,
            ..Weapon::default()
        });
        tank.set_ai_state(AIState::AttackMoving);
        tank.is_attack_path = true;
        tank.requested_destination = Some(dest);
        tank.movement.target_position = Some(dest);
        tank.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
        let mut enemy = Object::new(ThingTemplate::new("Enemy"), ObjectId(2), Team::GLA);
        enemy.set_position(Vec3::new(20.0, 0.0, 0.0));
        logic.objects.insert(tank.id, tank);
        logic.objects.insert(enemy.id, enemy);
        let engaged = logic.apply_engagement_decision_aware(ObjectId(1), ObjectId(2));
        let tank = logic.objects.get(&ObjectId(1)).expect("tank");
        if engaged {
            assert_eq!(tank.ai_state, AIState::AttackMoving);
            assert_eq!(tank.target, Some(ObjectId(2)));
            assert_eq!(tank.movement.target_position, Some(dest));
            assert_eq!(tank.requested_destination, Some(dest));
            assert_eq!(tank.movement.velocity, Vec3::ZERO);
            assert!(!tank.status.moving);
        } else {
            assert_eq!(tank.ai_state, AIState::AttackMoving);
            assert_eq!(tank.movement.target_position, Some(dest));
        }
    }

    fn attack_move_unit(id: u32, dest: Vec3) -> Object {
        let mut tmpl = ThingTemplate::new("AtkMv");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut unit = Object::new(tmpl, ObjectId(id), Team::USA);
        unit.set_position(Vec3::ZERO);
        unit.set_ai_state(AIState::AttackMoving);
        unit.is_attack_path = true;
        unit.requested_destination = Some(dest);
        unit.attack_move_retry_count = 5;
        unit.attack_move_sleep_until = 0;
        unit.ai_attitude = 0;
        unit.vision_range = 200.0;
        unit.next_mood_check_time = 0;
        unit.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        unit
    }

    fn attack_move_enemy(id: u32, pos: Vec3) -> Object {
        let mut tmpl = ThingTemplate::new("AtkMvEnemy");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut enemy = Object::new(tmpl, ObjectId(id), Team::GLA);
        enemy.set_position(pos);
        enemy
    }

    /// hq-6p7c2: mood victim is engaged even when should_attack would Hold
    /// (distance > weapon.range * 1.5).
    #[test]
    fn attack_move_mood_target_skips_should_attack_hold() {
        let mut logic = GameLogic::new();
        logic
            .objects
            .insert(ObjectId(1), attack_move_unit(1, Vec3::new(400.0, 0.0, 0.0)));
        logic
            .objects
            .insert(ObjectId(2), attack_move_enemy(2, Vec3::new(90.0, 0.0, 0.0)));
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            20,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::AttackTarget {
                    object_id: ObjectId(1),
                    target_id: ObjectId(2),
                })
            ),
            "attack-move must engage the mood victim without Hold wrap; got {command:?}"
        );
    }

    /// hq-65aus: blocked / unfinished dest sleeps 3s and decrements retry.
    #[test]
    fn attack_move_blocked_path_sleeps_three_seconds() {
        let dest = Vec3::new(200.0, 0.0, 0.0);
        let mut logic = GameLogic::new();
        logic.objects.insert(ObjectId(1), attack_move_unit(1, dest));
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            21,
            1.0 / 30.0,
        );
        assert!(
            command.is_none(),
            "sleep start is CONTINUE; got {command:?}"
        );
        let unit = logic.objects.get(&ObjectId(1)).expect("unit");
        assert_eq!(unit.attack_move_retry_count, 4);
        assert_eq!(unit.attack_move_sleep_until, 111);
        assert_eq!(unit.ai_state, AIState::AttackMoving);
        assert!(unit.movement.target_position.is_none());
        assert_eq!(unit.requested_destination, Some(dest));
    }

    /// hq-65aus: after ATTACK_RETRY_COUNT sleeps, still-far dest gives up.
    #[test]
    fn attack_move_blocked_path_gives_up_after_five_retries() {
        let mut logic = GameLogic::new();
        let mut unit = attack_move_unit(1, Vec3::new(200.0, 0.0, 0.0));
        unit.attack_move_retry_count = 0;
        logic.objects.insert(ObjectId(1), unit);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            21,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "exhausted retries must leave attack-move; got {command:?}"
        );
    }

    /// hq-65aus: within 8 pathfind cells, accept the move result.
    #[test]
    fn attack_move_close_enough_does_not_retry() {
        let mut logic = GameLogic::new();
        logic
            .objects
            .insert(ObjectId(1), attack_move_unit(1, Vec3::new(50.0, 0.0, 0.0)));
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            21,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::SetAIState {
                    state: AIState::Idle,
                    ..
                })
            ),
            "close-enough dest must not sleep/retry; got {command:?}"
        );
    }

    /// hq-65aus: during the 3s sleep the unit can still mood-attack.
    #[test]
    fn attack_move_sleep_still_mood_attacks() {
        let mut logic = GameLogic::new();
        let mut unit = attack_move_unit(1, Vec3::new(400.0, 0.0, 0.0));
        unit.attack_move_sleep_until = 200;
        unit.attack_move_retry_count = 3;
        logic.objects.insert(ObjectId(1), unit);
        logic
            .objects
            .insert(ObjectId(2), attack_move_enemy(2, Vec3::new(40.0, 0.0, 0.0)));
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::AttackMoving,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            20,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::AttackTarget {
                    object_id: ObjectId(1),
                    target_id: ObjectId(2),
                })
            ),
            "sleep must not block mood engage; got {command:?}"
        );
        let unit = logic.objects.get(&ObjectId(1)).expect("unit");
        assert_eq!(unit.attack_move_retry_count, 3);
        assert_eq!(unit.attack_move_sleep_until, 200);
    }

    #[test]
    fn hunt_scan_prefers_higher_priority_over_team_victim() {
        let mut logic = GameLogic::new();
        let mut info = AttackPriorityInfo::new("HuntPrio");
        info.default_priority = 1;
        info.set_priority_template("Dozer", 5);
        info.set_priority_template("Tank", 80);
        logic.register_attack_priority_set(info);

        let mut hunter = Object::new(ThingTemplate::new("Hunter"), ObjectId(1), Team::USA);
        hunter.team_instance_name = "teamUSA".into();
        hunter.attack_priority_set = Some("HuntPrio".into());
        hunter.weapon = Some(Weapon {
            range: 150.0,
            damage: 10.0,
            ..Weapon::default()
        });
        hunter.hunting = true;
        hunter.set_ai_state(AIState::Patrolling);
        hunter.set_position(Vec3::ZERO);

        let mut dozer = Object::new(ThingTemplate::new("Dozer"), ObjectId(2), Team::GLA);
        dozer.set_position(Vec3::new(10.0, 0.0, 0.0));
        let mut tank = Object::new(ThingTemplate::new("Tank"), ObjectId(3), Team::GLA);
        tank.set_position(Vec3::new(40.0, 0.0, 0.0));
        logic.objects.insert(hunter.id, hunter);
        logic.objects.insert(dozer.id, dozer);
        logic.objects.insert(tank.id, tank);
        logic.set_host_team_common_target(ObjectId(1), Some(ObjectId(2)));
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 30);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            30,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::AttackTarget {
                    target_id: ObjectId(3),
                    ..
                })
            ),
            "hunt must retarget higher-priority tank; got {command:?}"
        );
        assert_eq!(
            logic.host_team_common_target(ObjectId(1)),
            Some(ObjectId(3))
        );
    }

    /// hq-q7xo1: hunt engages the map-wide victim even when should_attack
    /// would Hold (distance > weapon.range * 1.5).
    #[test]
    fn hunt_map_wide_victim_skips_should_attack_hold() {
        let mut logic = GameLogic::new();
        let mut hunter = Object::new(ThingTemplate::new("Hunter"), ObjectId(1), Team::USA);
        hunter.hunting = true;
        hunter.set_ai_state(AIState::Patrolling);
        hunter.set_position(Vec3::ZERO);
        hunter.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        logic.objects.insert(hunter.id, hunter);
        logic
            .objects
            .insert(ObjectId(2), attack_move_enemy(2, Vec3::new(90.0, 0.0, 0.0)));
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 30);
        let command = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            30,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                command,
                Some(AICommand::AttackTarget {
                    object_id: ObjectId(1),
                    target_id: ObjectId(2),
                })
            ),
            "hunt must chase the map-wide victim without Hold wrap; got {command:?}"
        );
    }

    /// hq-qqw8d: Hunt scan is per-unit m_nextEnemyScanTime, not frame%30.
    #[test]
    fn hunt_scan_uses_per_unit_clock_not_frame_mod_30() {
        let mut logic = GameLogic::new();
        let mut due = Object::new(ThingTemplate::new("DueHunter"), ObjectId(1), Team::USA);
        due.hunting = true;
        due.set_ai_state(AIState::Patrolling);
        due.set_position(Vec3::ZERO);
        due.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        let mut waiting = Object::new(ThingTemplate::new("WaitHunter"), ObjectId(2), Team::USA);
        waiting.hunting = true;
        waiting.set_ai_state(AIState::Patrolling);
        waiting.set_position(Vec3::new(5.0, 0.0, 0.0));
        waiting.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        logic.objects.insert(due.id, due);
        logic.objects.insert(waiting.id, waiting);
        logic
            .objects
            .insert(ObjectId(3), attack_move_enemy(3, Vec3::new(80.0, 0.0, 0.0)));
        // Frame 31 is not a multiple of 30. Old lockstep would skip both.
        logic.hunt_next_enemy_scan.insert(ObjectId(1), 31);
        logic.hunt_next_enemy_scan.insert(ObjectId(2), 50);
        let due_cmd = logic.process_ai_behavior(
            ObjectId(1),
            AIState::Patrolling,
            None,
            Vec3::ZERO,
            Team::USA,
            true,
            31,
            1.0 / 30.0,
        );
        let wait_cmd = logic.process_ai_behavior(
            ObjectId(2),
            AIState::Patrolling,
            None,
            Vec3::new(5.0, 0.0, 0.0),
            Team::USA,
            true,
            31,
            1.0 / 30.0,
        );
        assert!(
            matches!(
                due_cmd,
                Some(AICommand::AttackTarget {
                    object_id: ObjectId(1),
                    target_id: ObjectId(3),
                })
            ),
            "due hunter must scan off the global 30-boundary; got {due_cmd:?}"
        );
        assert!(
            wait_cmd.is_none(),
            "waiting hunter must keep its own clock; got {wait_cmd:?}"
        );
        assert_eq!(
            logic.hunt_next_enemy_scan.get(&ObjectId(1)).copied(),
            Some(61),
            "after a scan, next time is now + ENEMY_SCAN_RATE"
        );
    }
}
