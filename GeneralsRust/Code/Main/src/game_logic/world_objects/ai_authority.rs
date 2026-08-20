//! Host objects `impl GameLogic` — `ai_authority`.
//! AI behavior, stop_attack, health authority write. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Process AI behavior for a single object
    /// Enhanced with proper enemy detection, attack decisions, and movement
    pub(in super::super) fn process_ai_behavior(
        &self,
        object_id: ObjectId,
        ai_state: AIState,
        target_id: Option<ObjectId>,
        position: Vec3,
        team: Team,
        can_attack: bool,
        frame: u32,
        _dt: f32,
    ) -> Option<AICommand> {
        let should_scan =
            |interval: u32| -> bool { interval > 0 && frame.is_multiple_of(interval) };
        // C++ AIAttackState / AIIdleState never auto-retreat on low HP.
        // Mood targeting (getNextMoodTarget) only issues aiAttackObject.
        let evaluate_enemy = |enemy_id: ObjectId, search_radius: f32| -> Option<AICommand> {
            use crate::ai_decisions::{AIDecisionSystem, AttackDecision};

            match AIDecisionSystem::should_attack(self, object_id, enemy_id) {
                AttackDecision::Attack | AttackDecision::Retreat => Some(AICommand::AttackTarget {
                    object_id,
                    target_id: enemy_id,
                }),
                AttackDecision::FindNewTarget => AIDecisionSystem::find_best_target(
                    self,
                    object_id,
                    position,
                    team,
                    search_radius,
                    true,
                    true,
                    false,
                )
                .map(|better_target| AICommand::AttackTarget {
                    object_id,
                    target_id: better_target,
                }),
                AttackDecision::Hold => None,
            }
        };

        // When skirmish AI is paused for a player (non-local), do not open new
        // auto-engage / patrol scans. Existing explicit AttackObject orders still
        // fire via update_combat. Used by golden clear so paused AI does not
        // counterfire production rangers mid-march.
        let ai_auto_engage_paused = self.skirmish_ai_auto_engage_paused(team);

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
                            self,
                            object_id,
                            position,
                            team,
                            200.0,
                            true,
                            true,
                            false,
                        )
                        .map(|target_id| AICommand::AttackTarget {
                            object_id,
                            target_id,
                        })
                        .or(Some(AICommand::StopAttack { object_id }))
                    }
                }
            }

            AIState::AttackMoving => {
                if can_attack && !ai_auto_engage_paused && should_scan(20) {
                    let search_radius = 200.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy_for_attacker(
                            self,
                            object_id,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }
                None
            }

            AIState::Moving => {
                None
            }

            AIState::Patrolling => {
                // C++ AIHuntState::update — map-wide seek-and-destroy.
                // ENEMY_SCAN_RATE = LOGICFRAMES_PER_SECOND (~30).
                if can_attack && !ai_auto_engage_paused && should_scan(30) {
                    if let Some(enemy_id) = self.find_closest_enemy(
                        object_id,
                        9999.9,
                        crate::game_logic::find_enemy_flags::CAN_ATTACK,
                    ) {
                        return evaluate_enemy(enemy_id, 9999.9);
                    }
                    return Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Idle,
                    });
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
                // Worker heading back to supply center to deposit resources.
                // The actual deposit happens in the update loop when close enough.
                let owner_player_id = self
                    .objects
                    .get(&object_id)
                    .and_then(|object| self.player_owner_for_host_object(object));
                if let Some(refinery_id) = self.preferred_supply_center_or_nearest(
                    object_id,
                    team,
                    owner_player_id,
                    position,
                ) {
                    if let Some(refinery) = self.objects.get(&refinery_id) {
                        let dist_to_refinery = position.distance(refinery.get_position());
                        if dist_to_refinery > 20.0 {
                            // Still heading to refinery
                            return Some(AICommand::MoveTo {
                                object_id,
                                position: refinery.get_position(),
                            });
                        }
                    }
                }
                None
            }

            AIState::Capturing => {
                // Unit is capturing enemy structure
                // Continue until capture completes
                None
            }
        }
    }

    /// Apply AI command to the game state
    /// Engage a target, honoring AI decision authority (log-only when GameWorld applies).
    ///
    /// Player command paths should call [`Object::attack_target`] directly so orders
    /// apply same-frame without waiting for shadow writeback.
    /// Clear engagement, honoring AI decision authority (log-only when GameWorld applies).
    ///
    /// Player `command_stop` should call [`Object::stop_attack`] directly for same-frame UX.
    pub(in super::super) fn stop_attack_decision_aware(&mut self, unit_id: ObjectId) {
        // Always clear host combat engagement immediately so mid-frame fire stops.
        // Log under decision authority for GameWorld last-write parity.
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

        // Presentation AttackTargeted residual (WeaponFire audio / dual-tick observe).
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
                die_on_detonate: false,
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
        // Host engagement is same-frame so residual auto-fire / continue-after-kill
        // can shoot without waiting for shadow writeback.
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_target(Some(target_id));
            // C++ Hunt stays in AI_HUNT while attacking. Combat already fires
            // from Patrolling. Do not peel Hunt into Attacking.
            if !matches!(u.ai_state, AIState::Patrolling) {
                u.set_ai_state(AIState::Attacking);
            }
            u.set_status_attacking(true);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
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
                // Prefer engagement helper (sets target even without weapon residual).
                let _ = self.apply_engagement_decision_aware(object_id, target_id);
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
                if matches!(state, AIState::Idle) {
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        if o.hunting && matches!(o.ai_state, AIState::Patrolling) {
                            o.hunting = false;
                        }
                    }
                }
                self.set_ai_state_decision_aware(object_id, state);
            }
        }
    }

    /// Test hook: apply one AICommand through the production decision path.
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
        let logic = GameLogic::new();
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
        let logic = GameLogic::new();
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
}
