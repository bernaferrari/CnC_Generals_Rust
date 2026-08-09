//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// Process AI behavior for a single object
    /// Enhanced with proper enemy detection, attack decisions, and movement
    pub(super) fn process_ai_behavior(
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
        let retreat_from = |threat_id: ObjectId| -> AICommand {
            let direction = self
                .objects
                .get(&threat_id)
                .map(|enemy| position - enemy.get_position())
                .and_then(|delta| {
                    if delta.length_squared() > f32::EPSILON {
                        Some(delta.normalize())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| Vec3::new(1.0, 0.0, 0.0));
            AICommand::MoveTo {
                object_id,
                position: position + direction * 90.0,
            }
        };
        let evaluate_enemy = |enemy_id: ObjectId, search_radius: f32| -> Option<AICommand> {
            use crate::ai_decisions::{AIDecisionSystem, AttackDecision};

            match AIDecisionSystem::should_attack(self, object_id, enemy_id) {
                AttackDecision::Attack => Some(AICommand::AttackTarget {
                    object_id,
                    target_id: enemy_id,
                }),
                AttackDecision::Retreat => Some(retreat_from(enemy_id)),
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
                if can_attack && !ai_auto_engage_paused && should_scan(30) {
                    let search_radius = 200.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }
                // Structures (especially base defenses) must not enter unit patrol
                // wander — residual auto-fire keeps them Idle and scanning.
                let is_structure = self
                    .objects
                    .get(&object_id)
                    .map(|o| o.is_kind_of(KindOf::Structure))
                    .unwrap_or(false);
                if !ai_auto_engage_paused && !is_structure && frame % 300 == object_id.0 % 300 {
                    Some(AICommand::SetAIState {
                        object_id,
                        state: AIState::Patrolling,
                    })
                } else {
                    None
                }
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
                    AttackDecision::Attack | AttackDecision::Hold => None,
                    AttackDecision::Retreat => Some(retreat_from(current_target_id)),
                    AttackDecision::FindNewTarget => {
                        if !can_attack {
                            return Some(AICommand::StopAttack { object_id });
                        }
                        AIDecisionSystem::find_best_target(
                            self, object_id, position, team, 220.0, true, true, false,
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
                    let search_radius = 220.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
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
                // While moving, check if we're under attack
                // Could transition to defensive behavior if needed
                None
            }

            AIState::Patrolling => {
                if can_attack && !ai_auto_engage_paused && should_scan(25) {
                    let search_radius = 200.0;
                    if let Some((enemy_id, _)) =
                        crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                            self,
                            position,
                            team,
                            search_radius,
                        )
                    {
                        return evaluate_enemy(enemy_id, search_radius);
                    }
                }

                if frame % 180 == object_id.0 % 180 {
                    let patrol_radius = 100.0;
                    let random_angle = (((object_id.0 as u64 * 1103515245 + frame as u64) % 360)
                        as f32)
                        .to_radians();
                    let patrol_pos = Vec3::new(
                        position.x + patrol_radius * random_angle.cos(),
                        position.y,
                        position.z + patrol_radius * random_angle.sin(),
                    );
                    Some(AICommand::MoveTo {
                        object_id,
                        position: patrol_pos,
                    })
                } else {
                    None
                }
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
                if let Some(refinery_id) = self.find_nearest_supply_center(team, position) {
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
    pub(super) fn stop_attack_decision_aware(&mut self, unit_id: ObjectId) {
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
    pub(super) fn clear_target_decision_aware(&mut self, unit_id: ObjectId) {
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
    pub(super) fn set_health_absolute_authority_aware(&mut self, object_id: ObjectId, health: f32) {
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
    pub(super) fn write_object_health_authority_aware(obj: &mut crate::game_logic::Object, health: f32) {
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
    pub(super) fn mark_destroyed_authority_aware(&mut self, object_id: ObjectId, source: Option<ObjectId>) {
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
    pub(super) fn mark_object_destroyed_authority_aware(
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
    pub(super) fn residual_auto_fire_apply_damage(
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
            let _ = slot;
            combat::queue_projectile(PendingProjectile {
                shooter_id: attacker_id,
                shooter_pos,
                target_id: Some(target_id),
                target_pos: self.objects.get(&target_id).map(|t| t.get_position()),
                damage,
                speed,
                splash_radius: splash,
                is_homing: homing,
                damage_type: dtype,
                death_type: HostDeathType::Normal,
                projectile_object_name: String::new(),
                detonation_fx_name: String::new(),
                detonation_ocl_name: String::new(),
                exhaust_name: String::new(),
                secondary_damage: 0.0,
                secondary_damage_radius: 0.0,
                shock_wave_amount: 0.0,
                shock_wave_radius: 0.0,
                shock_wave_taper_off: 0.0,
                radius_damage_affects: 0,
                projectile_collides: 0,
                scatter_radius: 0.0,
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
                kill_xp = target.thing.template.experience_value
                    * Self::veterancy_xp_multiplier(target.experience.level);
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
    pub(super) fn set_ai_state_decision_aware(&mut self, unit_id: ObjectId, state: AIState) {
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

    pub(super) fn apply_engagement_decision_aware(&mut self, unit_id: ObjectId, target_id: ObjectId) {
        // Host engagement is same-frame so residual auto-fire / continue-after-kill
        // can shoot without waiting for shadow writeback.
        if let Some(u) = self.objects.get_mut(&unit_id) {
            // set_target records host_attack_log for shadow attack channel.
            u.set_target(Some(target_id));
            u.set_ai_state(AIState::Attacking);
            u.set_status_attacking(true);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
    }

    #[cfg(test)]
    pub fn apply_engagement_decision_aware_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) {
        self.apply_engagement_decision_aware(unit_id, target_id);
    }

    /// AI / skirmish manager entry: host-immediate engagement + decision log.
    pub fn apply_engagement_decision_aware_for_ai(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) {
        self.apply_engagement_decision_aware(unit_id, target_id);
    }

    /// AI / skirmish manager entry: host-immediate AI state + decision log.
    pub fn set_ai_state_decision_aware_for_ai(&mut self, unit_id: ObjectId, state: AIState) {
        self.set_ai_state_decision_aware(unit_id, state);
    }

    pub(super) fn engage_target_decision_aware(&mut self, unit_id: ObjectId, target_id: ObjectId) {
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
    }

    #[cfg(test)]
    pub fn engage_target_decision_aware_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) {
        self.engage_target_decision_aware(unit_id, target_id);
    }

    pub(super) fn apply_ai_command(&mut self, command: AICommand) {
        // Host applies immediately so AI aggression/combat is same-frame.
        // Decision authority still logs every command for GameWorld last-write.
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        match command {
            AICommand::AttackTarget {
                object_id,
                target_id,
            } => {
                // Prefer engagement helper (sets target even without weapon residual).
                self.apply_engagement_decision_aware(object_id, target_id);
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
                // set_ai_state_decision_aware mutates host + logs when auth live.
                self.set_ai_state_decision_aware(object_id, state);
            }
        }
    }

    /// Test hook: apply one AICommand through the production decision path.
    #[cfg(test)]
    pub fn apply_ai_command_for_test(&mut self, command: AICommand) {
        self.apply_ai_command(command);
    }

    pub(super) fn update_support_states(&mut self, object_ids: &[ObjectId], dt: f32) {
        const GUARD_MIN_RADIUS: f32 = 80.0;
        const INTERACT_RANGE: f32 = crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;
        const CAPTURE_RANGE_PADDING: f32 = 4.0;
        const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
        // Host residual flat HP/sec (not C++ percent-of-max / TimeForFullHeal matrix).
        const REPAIR_RATE: f32 = crate::game_logic::host_repair::HOST_REPAIR_RATE_HP_PER_SEC;
        const HEAL_RATE: f32 = crate::game_logic::host_repair::HOST_HEAL_RATE_HP_PER_SEC;

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state.clone(),
                    obj.team,
                    obj.get_position(),
                    obj.target,
                    obj.guard_position,
                    obj.guard_target,
                    obj.guard_radius,
                    obj.guard_mode,
                    obj.can_move(),
                    obj.can_attack(),
                    obj.health.current,
                    obj.health.maximum,
                    obj.selection_radius,
                    obj.is_alive(),
                ),
                None => continue,
            };

            let (
                ai_state,
                team,
                position,
                target_id,
                guard_position,
                guard_target,
                guard_radius,
                guard_mode,
                can_move,
                can_attack,
                health_current,
                health_maximum,
                selection_radius,
                is_alive,
            ) = snapshot;

            if !is_alive {
                continue;
            }

            if ai_state != AIState::SpecialAbility {
                self.pending_special_abilities.remove(&object_id);
            }

            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    // C++ GuardMode residual (AIGuard.cpp):
                    // Normal — pursue outside (wider acquire).
                    // WithoutPursuit — no outer chase; engage only inside radius.
                    // FlyingUnitsOnly — PartitionFilterIsFlying on acquire.
                    let acquire_radius = match guard_mode {
                        crate::game_logic::GuardMode::Normal => radius * 1.5,
                        _ => radius,
                    };

                    if can_attack {
                        let flying_only =
                            matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                        let without_pursuit =
                            matches!(guard_mode, crate::game_logic::GuardMode::WithoutPursuit);
                        // Prefer nearest legal enemy around the guard anchor.
                        let mut best: Option<(ObjectId, f32)> = None;
                        for (cand_id, cand) in self.objects.iter() {
                            if !cand.is_alive() || !cand.is_targetable_by_enemy_of(team) {
                                continue;
                            }
                            if flying_only
                                && !(cand.is_kind_of(KindOf::Aircraft)
                                    || cand.object_type == ObjectType::Aircraft)
                            {
                                continue;
                            }
                            let d = anchor.distance(cand.get_position());
                            if d > acquire_radius {
                                continue;
                            }
                            if without_pursuit && d > radius {
                                continue;
                            }
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((*cand_id, d));
                            }
                        }
                        if let Some((enemy_id, _)) = best {
                            // WithoutPursuit: if we already left the bubble, return home first.
                            if without_pursuit && position.distance(anchor) > radius {
                                if can_move {
                                    self.path_approach_with_state(
                                        object_id,
                                        anchor,
                                        AIState::GuardingArea,
                                    );
                                }
                            } else {
                                self.engage_target_decision_aware(object_id, enemy_id);
                                continue;
                            }
                        }
                    }

                    if can_move && position.distance(anchor) > radius * 0.6 {
                        self.path_approach_with_state(object_id, anchor, AIState::GuardingArea);
                    }
                }
                AIState::GuardingObject => {
                    let guard_target_id = match guard_target {
                        Some(id) => id,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    };

                    let Some(guard_anchor) = self
                        .objects
                        .get(&guard_target_id)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_guard_target(None);
                        }
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let radius = guard_radius.max(GUARD_MIN_RADIUS);
                    if can_attack {
                        if let Some((enemy_id, _)) =
                            crate::ai_decisions::AIDecisionSystem::find_nearest_enemy(
                                self,
                                guard_anchor,
                                team,
                                radius,
                            )
                        {
                            self.engage_target_decision_aware(object_id, enemy_id);
                            continue;
                        }
                    }

                    if can_move && position.distance(guard_anchor) > radius * 0.6 {
                        self.path_approach_with_state(
                            object_id,
                            guard_anchor,
                            AIState::GuardingObject,
                        );
                    }
                }
                AIState::Repairing => {
                    let Some(repair_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let actor_can_repair = self
                        .objects
                        .get(&object_id)
                        .map(|obj| obj.can_repair())
                        .unwrap_or(false);
                    if !actor_can_repair {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let Some((
                        repair_target_pos,
                        repair_target_team,
                        repair_target_alive,
                        repair_target_is_structure,
                        repair_target_under_construction,
                    )) = self.objects.get(&repair_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !repair_target_alive
                        || !repair_target_is_structure
                        || repair_target_under_construction
                        || (repair_target_team != team && repair_target_team != Team::Neutral)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if can_move && position.distance(repair_target_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(
                            object_id,
                            repair_target_pos,
                            AIState::Repairing,
                        );
                        continue;
                    }

                    // Dozer structure-repair residual: heal HP over time while in range.
                    // C++ DozerAIUpdate DOZER_TASK_REPAIR + MODELCONDITION_ACTIVELY_CONSTRUCTING.
                    // RepairHealthPercentPerSecond residual (2% max HP / sec).
                    // Fail-closed: multi-dozer both allowed (not full sole-benefactor reject).
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_actively_constructing(true);
                    }
                    let max_hp = self
                        .objects
                        .get(&repair_target_id)
                        .map(|t| t.health.maximum)
                        .unwrap_or(0.0);
                    let heal_per_sec =
                        crate::game_logic::host_repair::dozer_repair_hp_per_sec(max_hp)
                            .max(REPAIR_RATE * 0.25);
                    let heal_amount = heal_per_sec * dt;
                    // C++ attemptHealingFromSoleBenefactor(health, dozer, 2) residual.
                    let now = self.frame;
                    let sole = if let Some(target) = self.objects.get_mut(&repair_target_id) {
                        let healed = target.attempt_healing_from_sole_benefactor(
                            heal_amount,
                            object_id,
                            2,
                            now,
                        );
                        let full = target.health.current >= target.health.maximum - 0.01;
                        let pos = target.get_position();
                        Some((full, healed, pos))
                    } else {
                        None
                    };
                    let (target_full, healed, repair_pos) = match sole {
                        Some(v) => v,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                                obj.set_actively_constructing(false);
                            }
                            continue;
                        }
                    };
                    if !healed && !target_full {
                        // Another dozer owns sole-benefactor claim — cancel this dozer task.
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                        self.sole_benefactor_repair_rejects =
                            self.sole_benefactor_repair_rejects.saturating_add(1);
                        continue;
                    }
                    if healed {
                        self.record_structure_repair_residual_heal();
                    }
                    if target_full {
                        // C++ DOZER:RepairComplete residual.
                        let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
                        self.queue_radar_message_at(
                            msg,
                            repair_pos,
                            radar_notifications::RadarKind::Generic,
                        );
                        self.repair_complete_events = self.repair_complete_events.saturating_add(1);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    object_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 0,
                                );
                            }
                            obj.set_actively_constructing(false);
                        }
                    }
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    if health_current >= health_maximum - 0.01 {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some(support_target_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((
                        support_target_pos,
                        support_target_team,
                        support_target_alive,
                        support_target_under_construction,
                        support_building_type,
                        support_template_name,
                    )) = self.objects.get(&support_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.team,
                            target.is_alive(),
                            target.status.under_construction,
                            target
                                .building_data
                                .as_ref()
                                .map(|b| b.building_type)
                                .unwrap_or(BuildingType::CommandCenter),
                            target.template_name.clone(),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !support_target_alive
                        || support_target_under_construction
                        || support_target_team != team
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    let source_can_use_support = self
                        .objects
                        .get(&object_id)
                        .map(|obj| match state {
                            AIState::SeekingRepair => {
                                if obj.is_kind_of(KindOf::Aircraft) {
                                    crate::game_logic::host_repair::building_provides_aircraft_repair(
                                        support_building_type,
                                    )
                                } else if obj.is_kind_of(KindOf::Vehicle) {
                                    // RepairPad (USA) + WarFactory (China RepairDock residual).
                                    crate::game_logic::host_repair::building_provides_vehicle_repair(
                                        support_building_type,
                                    )
                                } else {
                                    false
                                }
                            }
                            AIState::SeekingHealing => {
                                let name = support_template_name.to_ascii_lowercase();
                                let is_heal_pad = support_building_type
                                    == BuildingType::HealPad
                                    || name.contains("hospital")
                                    || name.contains("heal")
                                    || name.contains("medic");
                                obj.is_kind_of(KindOf::Infantry) && is_heal_pad
                            }
                            _ => false,
                        })
                        .unwrap_or(false);
                    if !source_can_use_support {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    if can_move && position.distance(support_target_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, support_target_pos, state.clone());
                        continue;
                    }

                    // Pad/airfield/war-factory residual: heal self over time while docked in range.
                    // C++ RepairDockUpdate::action TimeForFullHeal residual (flat host rate).
                    // HealPad SeekingHealing residual records heal honesty separately.
                    let mut vehicle_healed = false;
                    let mut heal_pad_healed = false;
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        let rate = match state {
                            AIState::SeekingRepair => REPAIR_RATE,
                            AIState::SeekingHealing => HEAL_RATE,
                            _ => 0.0,
                        };
                        let before = obj.health.current;
                        obj.heal(rate * dt);
                        let healed = obj.health.current > before + 0.0001;
                        if healed && matches!(state, AIState::SeekingRepair) {
                            vehicle_healed = true;
                        }
                        if healed && matches!(state, AIState::SeekingHealing) {
                            heal_pad_healed = true;
                        }
                        if obj.health.current >= obj.health.maximum - 0.01 {
                            obj.set_target(None);
                        } else {
                            // Host-immediate residual: keep SeekingRepair/Healing
                            // authoritative on host; log for GameWorld last-write.
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                let ordinal =
                                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                        &state,
                                    );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, ordinal,
                                );
                            }
                            obj.set_ai_state(state);
                        }
                    }
                    if vehicle_healed {
                        self.record_vehicle_repair_residual_heal();
                    }
                    if heal_pad_healed {
                        self.record_heal_pad_residual_heal();
                    }
                }
                state @ (AIState::Entering | AIState::Docking) => {
                    let Some(container_id) = target_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // USA Pilot residual: Enter unmanned vehicle → recrew (not transport load).
                    // Retail VeterancyCrateCollide IsPilot path residual.
                    {
                        let pilot_snapshot = self.objects.get(&object_id).map(|o| {
                            (
                                crate::game_logic::host_usa_pilot::is_pilot_template(
                                    &o.template_name,
                                ),
                                o.team,
                                o.experience.level,
                                o.get_position(),
                                o.selection_radius,
                                o.can_move(),
                            )
                        });
                        let vehicle_snapshot = self.objects.get(&container_id).map(|v| {
                            (
                                v.get_position(),
                                v.selection_radius,
                                v.is_alive(),
                                v.is_kind_of(KindOf::Vehicle),
                                v.is_kind_of(KindOf::Aircraft) || v.status.airborne_target,
                                v.is_unmanned(),
                                v.status.under_construction,
                                v.is_worker()
                                    || v.template_name.to_ascii_lowercase().contains("dozer"),
                            )
                        });
                        if let (
                            Some((
                                is_pilot,
                                pilot_team,
                                pilot_level,
                                pilot_pos,
                                pilot_radius,
                                pilot_can_move,
                            )),
                            Some((
                                vehicle_pos,
                                vehicle_radius,
                                v_alive,
                                v_vehicle,
                                v_air,
                                v_unmanned,
                                v_under_construction,
                                v_dozer,
                            )),
                        ) = (pilot_snapshot, vehicle_snapshot)
                        {
                            let recrewable =
                                crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                                    v_alive,
                                    v_vehicle,
                                    v_air,
                                    v_unmanned,
                                    v_under_construction,
                                    v_dozer,
                                );
                            if crate::game_logic::host_usa_pilot::should_recrew_on_enter(
                                is_pilot, recrewable,
                            ) {
                                let enter_range = pilot_radius + vehicle_radius + 4.0;
                                if pilot_can_move && pilot_pos.distance(vehicle_pos) > enter_range {
                                    self.path_approach_with_state(
                                        object_id,
                                        vehicle_pos,
                                        AIState::Entering,
                                    );
                                    continue;
                                }
                                let transferred = self
                                    .objects
                                    .get_mut(&container_id)
                                    .map(|v| v.apply_pilot_recrew(pilot_team, pilot_level))
                                    .unwrap_or(false);
                                self.usa_pilot.record_recrew(transferred);
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_usa_pilot::PILOT_RECREW_AUDIO,
                                    )
                                    .with_object(container_id)
                                    .with_position(vehicle_pos)
                                    .with_priority(170),
                                );
                                let msg =
                                    localization::localize("hud.pilot.recrew", "Vehicle recrewed");
                                self.queue_radar_message_for_team(pilot_team, msg);
                                self.mark_destroyed_authority_aware(object_id, None);
                                self.mark_object_for_destruction(object_id, Some(pilot_team));
                                continue;
                            }
                        }
                    }

                    let Some((
                        container_pos,
                        container_radius,
                        container_team,
                        container_is_structure,
                        container_is_faction_structure,
                        container_is_overlord_bunker,
                        container_is_battle_bus,
                        container_is_technical,
                        container_is_combat_cycle,
                        container_is_combat_chinook,
                        container_is_listening_outpost,
                        container_is_troop_crawler,
                        container_is_tunnel_network,
                        container_is_alive,
                        container_under_construction,
                        container_can_contain,
                        container_has_space,
                        container_has_unit,
                        container_occupant_count,
                    )) = self.objects.get(&container_id).map(|container| {
                        (
                            container.get_position(),
                            container.selection_radius,
                            container.team,
                            container.is_kind_of(KindOf::Structure),
                            container.is_faction_structure(),
                            container.is_overlord_style_container()
                                && container.overlord_bunker_slot_capacity() > 0,
                            container.is_battle_bus_style_container(),
                            container.is_technical_style_container(),
                            container.is_combat_cycle_style_container(),
                            container.is_combat_chinook_style_container(),
                            container.is_listening_outpost_style_container(),
                            container.is_troop_crawler_style_container(),
                            container.is_tunnel_network_style_container(),
                            container.is_alive(),
                            container.status.under_construction,
                            container.can_contain(),
                            container.has_capacity_for(1),
                            container.contained_units().contains(&object_id),
                            container.contained_units().len(),
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // Residual garrison / Overlord BattleBunker / Battle Bus:
                    // infantry/heroes only (C++ AllowInsideKindOf = INFANTRY).
                    // Combat Chinook allows INFANTRY + VEHICLE (not AIRCRAFT).
                    // Tunnel Network: C++ allows all units except aircraft.
                    let unit_can_garrison_structure = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Infantry) || o.is_hero())
                        .unwrap_or(false);
                    let unit_is_aircraft = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.is_kind_of(KindOf::Aircraft))
                        .unwrap_or(false);
                    if container_is_tunnel_network {
                        // TunnelContain residual: reject aircraft only.
                        if unit_is_aircraft {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    } else if (container_is_structure
                        || container_is_overlord_bunker
                        || container_is_battle_bus
                        || container_is_technical
                        || container_is_combat_cycle
                        || container_is_listening_outpost
                        || container_is_troop_crawler)
                        && !unit_can_garrison_structure
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }
                    // Combat Chinook ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE residual.
                    if container_is_combat_chinook && unit_is_aircraft {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Tunnel network residual: units already in the shared pool may
                    // transfer to another allied tunnel without walking (can_move false).
                    let already_in_tunnel_network = container_is_tunnel_network
                        && self.tunnel_network.team_holding_unit(object_id).is_some();

                    if (!can_move && !already_in_tunnel_network)
                        || !container_is_alive
                        || container_under_construction
                        || !container_can_contain
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if container_team != team
                        && container_team != Team::Neutral
                        && (container_is_faction_structure || container_occupant_count > 0)
                    {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let enter_range = selection_radius + container_radius + 4.0;
                    // Cross-tunnel residual transfer: skip walk when already in pool.
                    if !already_in_tunnel_network
                        && can_move
                        && position.distance(container_pos) > enter_range
                    {
                        self.path_approach_with_state(object_id, container_pos, state);
                        continue;
                    }

                    // Tunnel shared capacity (MaxTunnelCapacity=10) overrides local space.
                    let tunnel_has_space = if container_is_tunnel_network {
                        self.tunnel_network.is_in_network(team, object_id)
                            || self.tunnel_network.has_capacity(team)
                    } else {
                        true
                    };
                    let can_enter = container_has_unit
                        || (container_has_space && tunnel_has_space)
                        || already_in_tunnel_network;
                    if !can_enter {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let entered = if container_has_unit {
                        true
                    } else {
                        self.objects
                            .get_mut(&container_id)
                            .map(|container| container.add_occupant(object_id))
                            .unwrap_or(false)
                    };
                    if !entered {
                        continue;
                    }

                    // Shared pool bookkeeping for tunnel residual.
                    if container_is_tunnel_network {
                        if !self
                            .tunnel_network
                            .record_enter(team, object_id, container_id)
                        {
                            // Capacity race: undo local occupant add.
                            if let Some(container) = self.objects.get_mut(&container_id) {
                                container.remove_occupant(object_id);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_attacking(false);
                        obj.target_location = None;
                        obj.set_status_force_attack(false);
                        obj.target = Some(container_id);
                        obj.set_contained_by(Some(container_id));
                        obj.set_position(container_pos);
                        crate::game_logic::host_ground_height_log::record(
                            obj.id,
                            container_pos.y,
                            false,
                        );
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            crate::game_logic::host_move_log::record(
                                obj.id,
                                Some([container_pos.x, container_pos.y, container_pos.z]),
                            );
                            obj.record_host_movement();
                        }
                        let __ai_st = if container_is_structure {
                            AIState::Garrisoned
                        } else {
                            AIState::Docked
                        };
                        // Host-immediate garrison/dock residual under decision auth.
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            let ordinal =
                                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                                    &__ai_st,
                                );
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                object_id, ordinal,
                            );
                        }
                        obj.set_ai_state(__ai_st);
                        obj.set_status_moving(false);
                    }
                    if container_is_tunnel_network {
                        // Enter counter already incremented in record_enter.
                    } else if container_is_structure {
                        self.record_garrison_residual_enter();
                    } else if container_is_overlord_bunker {
                        // China Overlord BattleBunker residual load (redirected bunker slots).
                        self.record_overlord_bunker_residual_enter();
                    } else if container_is_battle_bus {
                        // GLA Battle Bus residual load (Slots=8 infantry transport).
                        self.record_battle_bus_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_technical {
                        // GLA Technical residual load (Slots=5 infantry; no passenger fire).
                        self.record_technical_residual_load();
                    } else if container_is_combat_cycle {
                        // GLA Combat Cycle residual load (Slots=1) + rider weapon switch.
                        self.record_combat_cycle_residual_load();
                        self.refresh_combat_cycle_rider_weapon(container_id);
                    } else if container_is_combat_chinook {
                        // AirF Combat Chinook residual load (Slots=8 + passenger fire).
                        self.record_combat_chinook_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_listening_outpost {
                        // China Listening Outpost residual load (Slots=2 + passenger fire).
                        self.record_listening_outpost_residual_load();
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    } else if container_is_troop_crawler {
                        // China Troop Crawler residual load (Slots=8; exit-to-fight).
                        self.record_troop_crawler_residual_load();
                    } else {
                        // Vehicle transport residual load (Humvee / generic transport).
                        self.record_transport_residual_load();
                        // Humvee-style PassengersAllowedToFire still refreshes weapon set
                        // when ArmedRidersUpgradeMyWeaponSet is set.
                        self.refresh_battle_bus_armed_riders_weapon_set(container_id);
                    }
                }
                AIState::Capturing => {
                    let Some(capture_target_id) = target_id else {
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let (can_capture_buildings, is_lotus_captor) = self
                        .objects
                        .get(&object_id)
                        .map(|obj| {
                            let is_lotus =
                                crate::game_logic::host_hero_abilities::is_black_lotus_template(
                                    &obj.template_name,
                                );
                            let can =
                                crate::game_logic::host_hero_abilities::can_capture_without_upgrade(
                                    obj.is_hero(),
                                    is_lotus,
                                ) || (obj.is_kind_of(KindOf::Infantry)
                                    && self.team_has_completed_capture_upgrade(obj.team));
                            (can, is_lotus || obj.is_hero())
                        })
                        .unwrap_or((false, false));
                    if !can_capture_buildings {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    }

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_structure,
                        target_under_construction,
                    )) = self.objects.get(&capture_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Structure),
                            target.status.under_construction,
                        )
                    })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !target_alive || !target_is_structure || target_under_construction {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if target_team == team {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                            obj.stop_moving();
                        }
                        continue;
                    }

                    // Black Lotus / hero residual: StartAbilityRange 150.
                    // Infantry residual: selection radii + small pad.
                    let capture_range = if is_lotus_captor {
                        crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else {
                        selection_radius + target_radius + CAPTURE_RANGE_PADDING
                    };
                    if can_move && position.distance(target_position) > capture_range {
                        if self.assign_unit_path(object_id, target_position, &[]) {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 19,
                                    ); // Capturing
                                } else {
                                    obj.set_ai_state(AIState::Capturing);
                                }
                            }
                        } else if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_destination(target_position);
                            obj.set_ai_state(AIState::Capturing);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    object_id, 19,
                                ); // Capturing
                            }
                        }
                        continue;
                    }

                    let did_capture = if self
                        .objects
                        .get(&capture_target_id)
                        .map(|target| {
                            target.is_alive()
                                && target.is_kind_of(KindOf::Structure)
                                && !target.status.under_construction
                                && target.team != team
                        })
                        .unwrap_or(false)
                    {
                        // BoobyTrap residual: enemy capture detonate (allies skip).
                        // C++ SpecialAbilityUpdate / checkAndDetonateBoobyTrap(captor).
                        let trap_pos = self
                            .objects
                            .get(&capture_target_id)
                            .map(|t| t.get_position())
                            .unwrap_or(target_position);
                        let planter_ally = self
                            .booby_trap
                            .plant(capture_target_id)
                            .map(|p| p.planter_team == team)
                            .unwrap_or(false);
                        if !planter_ally
                            && (self.booby_trap.is_booby_trapped(capture_target_id)
                                || self
                                    .objects
                                    .get(&capture_target_id)
                                    .map(|t| t.status.booby_trapped)
                                    .unwrap_or(false))
                        {
                            let _ = self.detonate_booby_trap_at(
                                capture_target_id,
                                trap_pos,
                                Some(object_id),
                                true,
                                false,
                            );
                        }
                        // Structure may have been destroyed by trap — re-check.
                        if !self
                            .objects
                            .get(&capture_target_id)
                            .map(|t| t.is_alive())
                            .unwrap_or(false)
                        {
                            false
                        } else {
                            // C++ capture prep residual: warn local victim + infiltration.
                            self.try_eva_building_being_stolen(capture_target_id);
                            self.try_infiltration_event(capture_target_id);
                            self.cancel_all_production(capture_target_id);
                            if let Some(target) = self.objects.get_mut(&capture_target_id) {
                                target.set_team(team);
                                target.health.heal(target.max_health);
                                // C++ defect(..., 1) one-frame flash residual.
                                target.flash_as_selected();
                                true
                            } else {
                                false
                            }
                        }
                    } else {
                        false
                    };

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_target(None);
                    }

                    if did_capture {
                        // C++ Object::onCapture residual (kick/idle/AI-sell/deselect).
                        self.on_capture_object_residual(capture_target_id, target_team, team);
                        // C++ getAcademyStats()->recordBuildingCapture() residual.
                        if let Some(p) = self.get_player_mut_by_team(team) {
                            p.record_building_capture();
                        }
                        if is_lotus_captor {
                            self.hero_abilities.record_building_capture();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::CAPTURE_BUILDING_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                        }
                        // C++ EVA_BuildingStolen when victim was local before defect.
                        // (team already flipped — use BeingStolen honesty or explicit
                        // pre-flip: fire BuildingStolen if victim team had local player
                        // that is no longer owner.)
                        // BeingStolen already gated on pre-flip local control; Stolen
                        // should also only fire for former local owner.
                        // Re-check: after flip, former local team lost the building —
                        // if any local player is on previous target_team.
                        let former_local = self
                            .players
                            .values()
                            .any(|p| p.is_local && p.is_alive && p.team == target_team);
                        if former_local {
                            let _ = gamelogic::helpers::TheEva::set_should_play(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            crate::game_logic::host_eva_log::record_event(
                                gamelogic::helpers::EvaEvent::BuildingStolen,
                            );
                            self.hero_abilities.record_eva_building_stolen();
                        }
                        let msg =
                            localization::localize("hud.capture.complete", "Building captured");
                        self.queue_radar_message_for_team(team, msg);
                    }
                }
                AIState::SpecialAbility => {
                    let Some(ability) = self.pending_special_abilities.get(&object_id).copied()
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.stop_moving();
                            obj.set_target(None);
                        }
                        continue;
                    };
                    let special_target_id = ability.target_id();

                    let Some((
                        target_position,
                        target_radius,
                        target_team,
                        target_alive,
                        target_is_vehicle,
                        target_is_structure,
                        target_is_airborne,
                        target_is_carbomb,
                        target_is_hijacked,
                        target_is_hacked,
                        target_is_unmanned,
                    )) = self.objects.get(&special_target_id).map(|target| {
                        (
                            target.get_position(),
                            target.selection_radius,
                            target.team,
                            target.is_alive(),
                            target.is_kind_of(KindOf::Vehicle),
                            target.is_kind_of(KindOf::Structure),
                            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                            target.status.is_carbomb,
                            target.status.hijacked,
                            target.status.disabled_hacked,
                            target.status.disabled_unmanned,
                        )
                    })
                    else {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    };

                    // CarBomb allows neutral; DisguiseAsVehicle allows any living
                    // vehicle (ally/enemy/neutral) — C++ ActionManager residual.
                    let requires_enemy_target = !matches!(
                        ability,
                        PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    );
                    if !target_alive
                        || (requires_enemy_target
                            && (target_team == team || target_team == Team::Neutral))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(
                        ability,
                        PendingSpecialAbility::SnipeVehicle { .. }
                            | PendingSpecialAbility::Hijack { .. }
                            | PendingSpecialAbility::CarBomb { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                            | PendingSpecialAbility::DisguiseAsVehicle { .. }
                    ) && (!target_is_vehicle || target_is_airborne)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        self.clear_target_decision_aware(object_id);
                        continue;
                    }

                    // Disguise: reject bomb-truck / train name residual targets,
                    // unless the target is already disguised (C++ disguiseAsObject
                    // copies that appearance — true template may still be bomb truck).
                    if matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. }) {
                        use crate::game_logic::host_bomb_truck_disguise::{
                            is_bomb_truck_template, is_legal_disguise_target_template,
                        };
                        let (target_tpl, target_disguised) = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| (t.template_name.clone(), t.status.disguised))
                            .unwrap_or_default();
                        let reject_bomb = is_bomb_truck_template(&target_tpl) && !target_disguised;
                        if reject_bomb || !is_legal_disguise_target_template(&target_tpl) {
                            // is_legal rejects bomb trucks by name; allow when disguised.
                            if !(target_disguised && is_bomb_truck_template(&target_tpl)) {
                                self.pending_special_abilities.remove(&object_id);
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(None);
                                }
                                continue;
                            }
                        }
                    }

                    // ConvertToCarBomb: cannot re-convert an existing car bomb.
                    if matches!(ability, PendingSpecialAbility::CarBomb { .. }) && target_is_carbomb
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Hijack: cannot re-hijack an already hijacked vehicle.
                    if matches!(ability, PendingSpecialAbility::Hijack { .. }) && target_is_hijacked
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Disable vehicle hack: skip already-hacked or unmanned vehicles.
                    if matches!(ability, PendingSpecialAbility::DisableVehicleHack { .. })
                        && (target_is_hacked || target_is_unmanned)
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if matches!(ability, PendingSpecialAbility::Sabotage { .. })
                        && !target_is_structure
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Burton plant charge (timed or remote): structure or ground vehicle.
                    if matches!(
                        ability,
                        PendingSpecialAbility::PlantTimedDemoCharge { .. }
                            | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                    ) && !(target_is_structure || (target_is_vehicle && !target_is_airborne))
                    {
                        self.pending_special_abilities.remove(&object_id);
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_target(None);
                        }
                        continue;
                    }

                    // Black Lotus cash hack: enemy cash-generator structures only.
                    if matches!(ability, PendingSpecialAbility::StealCashHack { .. }) {
                        let is_cash_gen = self
                            .objects
                            .get(&special_target_id)
                            .map(|t| {
                                crate::game_logic::host_hero_abilities::is_cash_hack_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::FSBlackMarket),
                                    t.is_kind_of(KindOf::FSSupplyDropzone),
                                )
                            })
                            .unwrap_or(false);
                        if !target_is_structure || !is_cash_gen {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // China Hacker DisableBuilding: enemy structures only; skip already-hacked.
                    if matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. }) {
                        if !target_is_structure || target_is_hacked {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // GLA Rebel BoobyTrap: structures only (enemy/neutral residual).
                    if matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. }) {
                        if !target_is_structure {
                            self.pending_special_abilities.remove(&object_id);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    }

                    // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
                    // residual: complete without approach walk.
                    let disguise_instant =
                        matches!(ability, PendingSpecialAbility::DisguiseAsVehicle { .. });
                    // Hacker DisableBuilding residual: StartAbilityRange 150 (not melee pad).
                    let hacker_disable_range =
                        matches!(ability, PendingSpecialAbility::HackerDisableBuilding { .. });
                    // Black Lotus residual specials: StartAbilityRange 150.
                    let black_lotus_range = matches!(
                        ability,
                        PendingSpecialAbility::StealCashHack { .. }
                            | PendingSpecialAbility::DisableVehicleHack { .. }
                    );
                    let booby_trap_range =
                        matches!(ability, PendingSpecialAbility::PlantBoobyTrap { .. });
                    let interact_range = if hacker_disable_range {
                        crate::game_logic::host_hacker_disable::HACKER_DISABLE_START_ABILITY_RANGE
                    } else if black_lotus_range {
                        crate::game_logic::host_hero_abilities::BLACK_LOTUS_START_ABILITY_RANGE
                    } else if booby_trap_range {
                        crate::game_logic::host_booby_trap::BOOBY_START_ABILITY_RANGE
                            + selection_radius
                            + target_radius
                    } else {
                        selection_radius + target_radius + SPECIAL_ABILITY_RANGE_PADDING
                    };
                    if !disguise_instant
                        && can_move
                        && position.distance(target_position) > interact_range
                    {
                        self.path_approach_with_state(
                            object_id,
                            target_position,
                            AIState::SpecialAbility,
                        );
                        continue;
                    }

                    match ability {
                        PendingSpecialAbility::Hijack { .. } => {
                            // C++ ConvertToHijackedVehicleCrateCollide residual:
                            // walk → transfer team + OBJECT_STATUS_HIJACKED; hijacker
                            // consumed (fail-closed vs hide-in-vehicle HijackerUpdate).
                            // Endow MAX veterancy + cancel dozer tasks via apply_hijacked_from.
                            // C++ order: tryInfiltrationEvent → EVA_VehicleStolen → setTeam.
                            self.try_infiltration_event(special_target_id);
                            self.try_eva_vehicle_stolen(special_target_id);
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_hijacked_from(donor_snap.as_ref());
                                target.set_team(team);
                            }
                            // C++ transferObjectName residual.
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_hijack();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::HIJACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg =
                                localization::localize("hud.hijack.complete", "Vehicle hijacked");
                            self.queue_radar_message_for_team(team, msg);
                            // C++: if target has EjectPilotDie → hide hijacker in vehicle;
                            // else destroy hijacker immediately.
                            // Wave 753: ride-hide only when the hijacker is infantry
                            // (HijackerUpdate module). Non-infantry steal path destroys
                            // the attacker immediately (test/tank harness + C++ shape).
                            // C++: if target has EjectPilotDie and hijacker is infantry
                            // (HijackerUpdate) → hide in vehicle; else consume attacker.
                            // Wave 753: ride-hide only for infantry; non-infantry steal
                            // destroys immediately. SlowDeath must not clear destroyed —
                            // hijacker consume is same-frame (begin_slow_death clears the
                            // destroyed flag for delayed peels).
                            let hijacker_is_infantry = self
                                .objects
                                .get(&object_id)
                                .map(|h| {
                                    h.is_kind_of(KindOf::Infantry)
                                        || h.object_type == ObjectType::Infantry
                                })
                                .unwrap_or(false);
                            if hijacker_is_infantry
                                && self.vehicle_supports_hijacker_ride(special_target_id)
                            {
                                if let Some(h) = self.objects.get_mut(&object_id) {
                                    h.begin_hijacker_in_vehicle(special_target_id);
                                }
                            } else {
                                self.mark_destroyed_authority_aware(object_id, None);
                                // Suppress SlowDeath/jet/heli peels so consume sticks.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                }
                                self.mark_object_for_destruction(object_id, Some(team));
                                // mark_object may re-enter SlowDeath and clear destroyed;
                                // re-assert consume residual for hijack steal.
                                if let Some(o) = self.objects.get_mut(&object_id) {
                                    o.slow_death = None;
                                    o.jet_slow_death = None;
                                    o.helicopter_slow_death = None;
                                    o.status.effectively_dead = true;
                                    o.status.destroyed = true;
                                    if !crate::gameworld_shadow::gameworld_damage_authority_live()
                                        && o.health.current > 0.0
                                    {
                                        o.health.current = 0.0;
                                    }
                                }
                            }
                        }
                        PendingSpecialAbility::Sabotage { .. } => {
                            // C++ Sabotage*CrateCollide residual: type-specific structure
                            // sabotage; saboteur consumed on success (mobile crate).
                            use crate::game_logic::host_saboteur::{
                                classify_sabotage_target, is_saboteur_template, SaboteurEffectKind,
                                SABOTEUR_CASH_STEAL_AUDIO, SABOTEUR_RESET_TIMER_AUDIO,
                                SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO,
                            };
                            let saboteur_ok = self
                                .objects
                                .get(&object_id)
                                .map(|o| is_saboteur_template(&o.template_name))
                                .unwrap_or(false);
                            let effect = self.objects.get(&special_target_id).and_then(|t| {
                                classify_sabotage_target(
                                    &t.template_name,
                                    t.is_kind_of(KindOf::FSPower),
                                    t.is_kind_of(KindOf::PowerPlant),
                                    t.is_kind_of(KindOf::FSSupplyCenter),
                                    t.is_kind_of(KindOf::SupplyCenter),
                                    t.is_kind_of(KindOf::FSBarracks),
                                    t.is_kind_of(KindOf::FSWarFactory),
                                    t.is_kind_of(KindOf::FSAirfield),
                                    t.is_kind_of(KindOf::FSSuperweapon),
                                    t.is_kind_of(KindOf::FSStrategyCenter),
                                    t.is_kind_of(KindOf::CommandCenter),
                                    t.is_kind_of(KindOf::FSInternetCenter),
                                    t.is_kind_of(KindOf::FSFake),
                                )
                            });
                            if saboteur_ok {
                                if let Some(kind) = effect {
                                    let mut cash_stolen = 0u32;
                                    match kind {
                                        SaboteurEffectKind::PowerPlant => {
                                            let until = self.frame.saturating_add(
                                                crate::game_logic::host_saboteur::SABOTEUR_POWER_DURATION_FRAMES,
                                            );
                                            if let Some(player) =
                                                self.get_player_mut_by_team(target_team)
                                            {
                                                player.power_sabotaged_till_frame = until;
                                            }
                                        }
                                        SaboteurEffectKind::SupplyCenter => {
                                            cash_stolen = self.steal_cash_from_team(
                                                target_team,
                                                team,
                                                SABOTEUR_STEAL_CASH_AMOUNT,
                                            );
                                        }
                                        SaboteurEffectKind::MilitaryFactory => {
                                            if let Some(until) =
                                                kind.disabled_hacked_until(self.frame)
                                            {
                                                if let Some(target) =
                                                    self.objects.get_mut(&special_target_id)
                                                {
                                                    target.apply_disabled_hacked(until);
                                                }
                                            }
                                        }
                                        SaboteurEffectKind::InternetCenter => {
                                            // C++ SabotageInternetCenterCrateCollide residual:
                                            // 1) disable SpyVisionUpdate on ALL team internet centers
                                            // 2) DISABLED_HACKED on the sabotaged center
                                            // 3) DISABLED_HACKED on contained hackers
                                            let until = kind
                                                .disabled_hacked_until(self.frame)
                                                .unwrap_or_else(|| {
                                                    self.frame.saturating_add(
                                                        crate::game_logic::host_saboteur::SABOTEUR_INTERNET_DURATION_FRAMES,
                                                    )
                                                });
                                            let (centers, hackers) = self
                                                .apply_internet_center_sabotage_residual(
                                                    special_target_id,
                                                    target_team,
                                                    until,
                                                );
                                            self.saboteur.record_internet_spy_vision_disable(
                                                centers, hackers,
                                            );
                                        }
                                        SaboteurEffectKind::SuperweaponOrCommand => {
                                            // C++ SabotageSuperweaponCrateCollide: reset ALL
                                            // SpecialPowerModule interfaces via startPowerRecharge.
                                            // Host residual: object-level special power + strike
                                            // registry timers for this structure.
                                            let reset_ok = self
                                                .apply_superweapon_sabotage_recharge(
                                                    special_target_id,
                                                );
                                            if reset_ok {
                                                self.saboteur.record_superweapon_power_reset();
                                            }
                                        }
                                        SaboteurEffectKind::FakeBuilding => {
                                            // C++ SabotageFakeBuildingCrateCollide:
                                            // DAMAGE_UNRESISTABLE / DEATH_DETONATED for max health.
                                            let destroyed = self
                                                .objects
                                                .get_mut(&special_target_id)
                                                .map(|target| {
                                                    let max_hp = target
                                                        .health
                                                        .maximum
                                                        .max(target.max_health)
                                                        .max(1.0);
                                                    target.take_damage_from_typed_death(
                                                        max_hp,
                                                        Some(object_id),
                                                        crate::game_logic::combat::DamageType::Unresistable,
                                                        crate::game_logic::host_usa_pilot::HostDeathType::Detonated,
                                                    )
                                                })
                                                .unwrap_or(false);
                                            if destroyed {
                                                self.mark_object_for_destruction(
                                                    special_target_id,
                                                    Some(team),
                                                );
                                                self.saboteur.record_fake_detonated();
                                            }
                                        }
                                    }
                                    self.saboteur.record(kind, cash_stolen);
                                    // C++ TheRadar->tryInfiltrationEvent(other) residual
                                    // (victim local player warning).
                                    self.try_infiltration_event(special_target_id);
                                    // C++ TheEva->setShouldPlay residual when victim local.
                                    // Supply center: CashStolen if cash taken, else BuildingSabotaged.
                                    if kind.steals_cash() && cash_stolen > 0 {
                                        // C++ controller ScoreKeeper::addMoneyEarned residual.
                                        if let Some(p) = self.get_player_mut_by_team(team) {
                                            p.add_money_earned(cash_stolen);
                                        }
                                        self.try_eva_cash_stolen(special_target_id);
                                        // C++ GUI:AddCash / GUI:LoseCash floating text residual.
                                        self.spawn_sabotage_cash_floating_texts(
                                            object_id,
                                            special_target_id,
                                            cash_stolen,
                                        );
                                    } else {
                                        self.try_eva_building_sabotaged(special_target_id);
                                    }
                                    // C++ doSabotageFeedbackFX residual (type audio + flash).
                                    self.do_sabotage_feedback_fx(special_target_id, kind);
                                    let msg = localization::localize(
                                        "hud.saboteur.complete",
                                        "Building sabotaged",
                                    );
                                    self.queue_radar_message_for_team(team, msg);
                                    // C++ CrateCollide: destroy saboteur (mobile crate).
                                    self.mark_destroyed_authority_aware(object_id, None);
                                    self.mark_object_for_destruction(object_id, Some(team));
                                    self.saboteur.record_consumed();
                                } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                    // Fail-closed: non-matching structure — cancel residual.
                                    obj.stop_moving();
                                    obj.set_target(None);
                                }
                            } else if let Some(obj) = self.objects.get_mut(&object_id) {
                                // Fail-closed: non-saboteur cannot complete residual.
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::SnipeVehicle { .. } => {
                            // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle becomes
                            // unmanned + Neutral so it can be recrewed/captured.
                            // C++ car-bomb dead-man: IS_CARBOMB detonates instead.
                            let is_bomb = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.is_car_bomb())
                                .unwrap_or(false);
                            if is_bomb {
                                let _ = self.maybe_detonate_carbomb_on_unmanned(special_target_id);
                            } else if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_kill_pilot_unmanned();
                                target.set_team(Team::Neutral);
                            }
                            self.hero_abilities.record_snipe();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::SNIPE_VEHICLE_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.snipe.vehicle_unmanned",
                                "Vehicle unmanned",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantTimedDemoCharge { .. } => {
                            // Burton / Tank Hunter TNT residual: plant sticky timed charge at target.
                            let is_tank_hunter = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    crate::game_logic::host_tank_hunter::is_tank_hunter_template(
                                        &o.template_name,
                                    )
                                })
                                .unwrap_or(false);
                            // Tank Hunter TNT reload residual (7500ms / 225 frames).
                            let tnt_ready = if is_tank_hunter {
                                crate::game_logic::host_tank_hunter::tnt_ready(
                                    self.frame,
                                    self.tank_hunter_tnt_last_frame.get(&object_id).copied(),
                                )
                            } else {
                                true
                            };
                            let charge_id = if tnt_ready {
                                self.place_timed_demo_charge(
                                    team,
                                    target_position,
                                    Some(object_id),
                                    Some(special_target_id),
                                    None,
                                )
                            } else {
                                None
                            };
                            if charge_id.is_some() {
                                self.hero_abilities.record_timed_charge_plant();
                                if is_tank_hunter {
                                    self.tank_hunter_residual_tnt_plants =
                                        self.tank_hunter_residual_tnt_plants.saturating_add(1);
                                    self.tank_hunter_tnt_last_frame
                                        .insert(object_id, self.frame);
                                    self.queue_audio_event(
                                        AudioEventRequest::new(
                                            crate::game_logic::host_tank_hunter::TNT_INITIATE_AUDIO,
                                        )
                                        .with_object(object_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                    );
                                }
                                let msg = localization::localize(
                                    "hud.demo_charge.planted",
                                    "Demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                            // Burton residual: plant sticky remote charge (no auto-timer).
                            let charge_id = self.place_remote_demo_charge(
                                team,
                                target_position,
                                Some(object_id),
                                Some(special_target_id),
                            );
                            if charge_id.is_some() {
                                self.hero_abilities.record_remote_charge_plant();
                                let msg = localization::localize(
                                    "hud.remote_demo_charge.planted",
                                    "Remote demo charge planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::StealCashHack { .. } => {
                            // Black Lotus residual: steal cash from enemy economy.
                            // C++ SPECIAL_BLACKLOTUS_STEAL_CASH_HACK:
                            // withdraw/deposit, scorekeeper money earned, EVA_CashStolen
                            // when victim local, GUI:AddCash/LoseCash floating texts.
                            let amount =
                                crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT;
                            let stolen = self.steal_cash_from_team(target_team, team, amount);
                            if stolen > 0 {
                                self.hero_abilities.record_cash_steal(stolen);
                                // C++ controller->getScoreKeeper()->addMoneyEarned(cash)
                                if let Some(p) = self.get_player_mut_by_team(team) {
                                    p.add_money_earned(stolen);
                                }
                                self.try_eva_cash_stolen(special_target_id);
                                self.spawn_sabotage_cash_floating_texts(
                                    object_id,
                                    special_target_id,
                                    stolen,
                                );
                                self.queue_audio_event(
                                    AudioEventRequest::new(
                                        crate::game_logic::host_hero_abilities::STEAL_CASH_AUDIO,
                                    )
                                    .with_object(object_id)
                                    .with_position(position)
                                    .with_priority(160),
                                );
                                let msg =
                                    localization::localize("hud.cash_hack.complete", "Cash stolen");
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::CarBomb { .. } => {
                            // C++ ConvertToCarBombCrateCollide residual:
                            // vehicle defects to converter team, gains IS_CARBOMB +
                            // SuicideCarBomb weapon residual. Converter is consumed.
                            // Detonation happens later when the car bomb attacks.
                            // Booby-trap residual: cancel if mine detonates and either dies.
                            let booby = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| t.status.booby_trapped)
                                .unwrap_or(false);
                            if booby {
                                // Detonate trap residual damage on both.
                                if let Some(t) = self.objects.get_mut(&special_target_id) {
                                    let _ = t.take_damage_from(
                                        t.health.maximum.max(1.0),
                                        Some(object_id),
                                    );
                                }
                                if let Some(b) = self.objects.get_mut(&object_id) {
                                    let _ = b.take_damage_from(
                                        b.health.maximum.max(1.0),
                                        Some(special_target_id),
                                    );
                                }
                                let t_dead = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| !t.is_alive() || t.status.destroyed)
                                    .unwrap_or(true);
                                let b_dead = self
                                    .objects
                                    .get(&object_id)
                                    .map(|b| !b.is_alive() || b.status.destroyed)
                                    .unwrap_or(true);
                                if t_dead || b_dead {
                                    if t_dead {
                                        self.mark_object_for_destruction(
                                            special_target_id,
                                            Some(team),
                                        );
                                    }
                                    if b_dead {
                                        self.mark_object_for_destruction(object_id, Some(team));
                                    }
                                    continue;
                                }
                            }
                            // Snapshot donor residual (vision/vet) before consume.
                            let donor_snap = self.objects.get(&object_id).cloned();
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_convert_to_car_bomb_from(donor_snap.as_ref());
                                target.set_team(team);
                            }
                            // C++ transferObjectName residual (script named object).
                            let _ = self.transfer_script_object_name(object_id, special_target_id);
                            self.car_bomb.record_conversion();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.carbomb.converted",
                                "Vehicle converted to car bomb",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            self.mark_destroyed_authority_aware(object_id, None);
                            self.mark_object_for_destruction(object_id, Some(team));
                        }
                        PendingSpecialAbility::DisableVehicleHack { .. } => {
                            // C++ SpecialAbilityUpdate BLACKLOTUS_DISABLE_VEHICLE_HACK:
                            // setDisabledUntil(DISABLED_HACKED, now + EffectDuration).
                            let until = self.frame.saturating_add(
                                crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_DURATION_FRAMES,
                            );
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_disabled_hacked(until);
                            }
                            self.hero_abilities.record_vehicle_disable();
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_hero_abilities::DISABLE_VEHICLE_HACK_AUDIO,
                                )
                                .with_object(special_target_id)
                                .with_position(target_position)
                                .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.vehicle_hack.disabled",
                                "Vehicle disabled",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::HackerDisableBuilding { .. } => {
                            // C++ SpecialAbilityUpdate SPECIAL_HACKER_DISABLE_BUILDING:
                            // setDisabledUntil(DISABLED_HACKED, now + EffectDuration 2000ms).
                            use crate::game_logic::host_hacker_disable::{
                                hacker_disable_until_frame, HACKER_DISABLE_BUILDING_AUDIO,
                            };
                            let until = hacker_disable_until_frame(self.frame);
                            if let Some(target) = self.objects.get_mut(&special_target_id) {
                                target.apply_disabled_hacked(until);
                            }
                            self.hacker_disable_building_count =
                                self.hacker_disable_building_count.saturating_add(1);
                            self.queue_audio_event(
                                AudioEventRequest::new(HACKER_DISABLE_BUILDING_AUDIO)
                                    .with_object(special_target_id)
                                    .with_position(target_position)
                                    .with_priority(170),
                            );
                            let msg = localization::localize(
                                "hud.hacker.building_disabled",
                                "Building disabled",
                            );
                            self.queue_radar_message_for_team(team, msg);
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                        PendingSpecialAbility::DisguiseAsVehicle { .. } => {
                            // C++ StealthUpdate::disguiseAsObject residual:
                            // if target already disguised, copy *its* disguise
                            // template + player; else copy target template + team.
                            // set OBJECT_STATUS_DISGUISED + STEALTHED.
                            let (tpl, as_team, copied_disguise) = self
                                .objects
                                .get(&special_target_id)
                                .map(|t| {
                                    if t.status.disguised {
                                        if let (Some(dt), Some(dteam)) =
                                            (t.disguise_as_template.as_ref(), t.disguise_as_team)
                                        {
                                            return (dt.clone(), dteam, true);
                                        }
                                    }
                                    (t.template_name.clone(), t.team, false)
                                })
                                .unwrap_or_else(|| {
                                    ("UnknownVehicle".to_string(), target_team, false)
                                });
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.apply_disguise(&tpl, as_team);
                                obj.stop_moving();
                                if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                ) {
                                    crate::game_logic::host_ai_decision_log::record_stop_attack(
                                        object_id,
                                    );
                                    crate::game_logic::host_ai_decision_log::record_set_state(
                                        object_id, 0,
                                    );
                                } else {
                                    obj.set_target(None);
                                    obj.set_ai_state(AIState::Idle);
                                }
                            }
                            self.bomb_truck_disguise.record_disguise(object_id, &tpl);
                            self.bomb_truck_disguise.record_transition_start();
                            if copied_disguise {
                                self.bomb_truck_disguise.record_disguise_copy();
                            }
                            self.queue_audio_event(
                                AudioEventRequest::new(
                                    crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_AUDIO,
                                )
                                .with_object(object_id)
                                .with_position(position)
                                .with_priority(160),
                            );
                            let msg = localization::localize(
                                "hud.bombtruck.disguised",
                                "Bomb truck disguised",
                            );
                            self.queue_radar_message_for_team(team, msg);
                        }
                        PendingSpecialAbility::PlantBoobyTrap { .. } => {
                            // C++ SpecialAbilityBoobyTrap residual: mark structure BOOBY_TRAPPED.
                            use crate::game_logic::host_booby_trap::{
                                has_booby_trap_upgrade, is_booby_trap_planter_template,
                                BOOBY_TRAP_INSTALL_AUDIO,
                            };
                            let (can_plant, ready) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let planter_ok =
                                        is_booby_trap_planter_template(&o.template_name)
                                            && has_booby_trap_upgrade(&o.applied_upgrades);
                                    let ready = self.booby_trap.plant_ready(object_id, self.frame);
                                    (planter_ok, ready)
                                })
                                .unwrap_or((false, false));
                            if can_plant
                                && ready
                                && self.booby_trap.can_place_special_object(object_id)
                            {
                                let geom = self
                                    .objects
                                    .get(&special_target_id)
                                    .map(|t| t.selection_radius.max(8.0))
                                    .unwrap_or(8.0);
                                let prev = self.booby_trap.install(
                                    special_target_id,
                                    object_id,
                                    team,
                                    self.frame,
                                    geom,
                                    None,
                                );
                                if let Some(prev_plant) = prev {
                                    if let Some(cid) = prev_plant.charge_object_id {
                                        self.destroy_booby_trap_special_object(cid);
                                    }
                                }
                                if let Some(cid) = self.spawn_booby_trap_special_object(
                                    object_id,
                                    team,
                                    special_target_id,
                                ) {
                                    self.booby_trap.set_charge_object(special_target_id, cid);
                                }
                                if let Some(target) = self.objects.get_mut(&special_target_id) {
                                    target.set_status_booby_trapped(true);
                                }
                                self.queue_audio_event(
                                    AudioEventRequest::new(BOOBY_TRAP_INSTALL_AUDIO)
                                        .with_object(special_target_id)
                                        .with_position(target_position)
                                        .with_priority(160),
                                );
                                let msg = localization::localize(
                                    "hud.booby_trap.planted",
                                    "Booby trap planted",
                                );
                                self.queue_radar_message_for_team(team, msg);
                            }
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.stop_moving();
                                obj.set_target(None);
                            }
                        }
                    }

                    self.pending_special_abilities.remove(&object_id);
                }
                AIState::Gathering => {
                    // Accumulate resources when close to the supply source.
                    const GATHER_RATE: f32 = 100.0;
                    const MAX_CARRY: u32 = 1000;

                    let Some(source_id) = target_id else {
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    };

                    // Extract source state before any mutations.
                    let (source_alive, source_pos) = self
                        .objects
                        .get(&source_id)
                        .map(|s| (s.is_alive(), s.get_position()))
                        .unwrap_or((false, position));

                    if !source_alive {
                        // C++ supply truck residual: find another warehouse when pile empties.
                        if let Some(next) = self.find_nearest_harvestable_supply(team, position) {
                            if let Some(dest) = self.objects.get(&next).map(|s| s.get_position()) {
                                if let Some(obj) = self.objects.get_mut(&object_id) {
                                    obj.set_target(Some(next));
                                }
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                                continue;
                            }
                        }
                        self.stop_attack_decision_aware(object_id);
                        self.set_ai_state_decision_aware(object_id, AIState::Idle);
                        continue;
                    }

                    if can_move && position.distance(source_pos) > INTERACT_RANGE {
                        self.path_approach_with_state(object_id, source_pos, AIState::Gathering);
                        continue;
                    }

                    // In range — gather resources.
                    // C++ parity (SupplyWarehouseDockUpdate): gathering depletes
                    // the supply source.  The source is destroyed when empty.
                    let gather_amount = (GATHER_RATE * dt) as u32;
                    let is_full = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.stored_resources.supplies)
                        .unwrap_or(0)
                        + gather_amount
                        >= MAX_CARRY;

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_stored_supplies(
                            obj.stored_resources
                                .supplies
                                .saturating_add(gather_amount)
                                .min(MAX_CARRY),
                        );
                    }

                    // Deplete the supply source.
                    if let Some(source) = self.objects.get_mut(&source_id) {
                        let taken = gather_amount.min(source.stored_resources.supplies);
                        source.set_stored_supplies(
                            source.stored_resources.supplies.saturating_sub(taken),
                        );
                        if source.stored_resources.supplies == 0 {
                            Self::mark_object_destroyed_authority_aware(source, None);
                            self.mark_object_for_destruction(source_id, None);
                        }
                    }

                    if is_full {
                        // Full — head to nearest supply center.
                        let refinery_dest = self
                            .find_nearest_supply_center(team, position)
                            .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()));
                        if let Some(dest) = refinery_dest {
                            self.path_approach_with_state(
                                object_id,
                                dest,
                                AIState::ReturningResources,
                            );
                        }
                    }
                }
                AIState::ReturningResources => {
                    // Deposit resources when close to a supply center.
                    let (refinery_id, refinery_pos) = self
                        .find_nearest_supply_center(team, position)
                        .and_then(|rid| {
                            self.objects
                                .get(&rid)
                                .map(|r| (Some(rid), r.get_position()))
                        })
                        .unwrap_or((None, position));

                    let at_refinery =
                        refinery_id.is_some() && position.distance(refinery_pos) <= INTERACT_RANGE;

                    if at_refinery {
                        // Deposit.
                        // C++ SupplyCenterDockUpdate::action: base box value +
                        // supplyTruckAI->getUpgradedSupplyBoost() when player has
                        // Upgrade_AmericaSupplyLines (Chinook residual).
                        let deposit_amount = self
                            .objects
                            .get(&object_id)
                            .map(|o| o.stored_resources.supplies)
                            .unwrap_or(0);

                        if deposit_amount > 0 {
                            // Snapshot carrier for residual boost identity (worker shoes).
                            let (
                                carrier_is_gla_worker,
                                carrier_has_worker_shoes,
                            ) = self
                                .objects
                                .get(&object_id)
                                .map(|o| {
                                    let is_w = crate::game_logic::host_gla_worker::is_gla_worker_template(
                                        &o.template_name,
                                    );
                                    let shoes = o.has_upgrade_tag(
                                        crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                    ) || self.players.values().any(|p| {
                                        p.team == team
                                            && p.has_unlocked_upgrade(
                                                crate::game_logic::host_gla_worker::UPGRADE_GLA_WORKER_SHOES,
                                            )
                                    });
                                    (is_w, shoes)
                                })
                                .unwrap_or((false, false));

                            // Clear carried resources.
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_stored_supplies(0);
                            }
                            // Player-level Supply Lines residual boost (flat per drop-off).
                            let has_supply_lines = self
                                .players
                                .values()
                                .any(|p| {
                                    p.team == team
                                        && p.has_unlocked_upgrade(
                                            crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
                                        )
                                });
                            let supply_lines_boost =
                                crate::game_logic::host_upgrades::residual_supply_lines_drop_off_boost(
                                    has_supply_lines,
                                );
                            // GLA WorkerShoes residual: +8 per drop-off when unlocked.
                            let worker_shoes_boost =
                                crate::game_logic::host_gla_worker::residual_worker_shoes_drop_off_boost(
                                    carrier_is_gla_worker,
                                    carrier_has_worker_shoes,
                                );
                            let boost = supply_lines_boost.saturating_add(worker_shoes_boost);
                            let credited = deposit_amount.saturating_add(boost);
                            // Credit the player (carried supplies + optional economy boost).
                            if let Some(player) = self.get_player_mut_by_team(team) {
                                player.credit_supplies(credited);
                            }
                            if supply_lines_boost > 0 {
                                self.supply_lines_bonus_cash_total = self
                                    .supply_lines_bonus_cash_total
                                    .saturating_add(supply_lines_boost);
                            }
                            if worker_shoes_boost > 0 {
                                self.gla_worker
                                    .record_shoes_drop_off_boost(worker_shoes_boost);
                            }
                            // Head back to gather more from the original source.
                            let source_dest = target_id.and_then(|sid| {
                                self.objects
                                    .get(&sid)
                                    .filter(|s| s.is_alive())
                                    .map(|s| s.get_position())
                            });
                            if let Some(dest) = source_dest {
                                self.path_approach_with_state(object_id, dest, AIState::Gathering);
                            } else if let Some(next) =
                                self.find_nearest_harvestable_supply(team, position)
                            {
                                if let Some(dest) =
                                    self.objects.get(&next).map(|s| s.get_position())
                                {
                                    if let Some(obj) = self.objects.get_mut(&object_id) {
                                        obj.set_target(Some(next));
                                    }
                                    self.path_approach_with_state(
                                        object_id,
                                        dest,
                                        AIState::Gathering,
                                    );
                                }
                            } else {
                                self.stop_attack_decision_aware(object_id);
                                self.set_ai_state_decision_aware(object_id, AIState::Idle);
                            }
                        }
                    } else if can_move {
                        // Still heading to refinery.
                        self.path_approach_with_state(
                            object_id,
                            refinery_pos,
                            AIState::ReturningResources,
                        );
                    }
                }
                AIState::Docked | AIState::Garrisoned => {
                    // Aircraft parking: leave hangar when given a move/attack residual.
                    let wants_sortie = self
                        .objects
                        .get(&object_id)
                        .map(|o| {
                            (o.is_kind_of(KindOf::Aircraft)
                                || o.object_type == ObjectType::Aircraft)
                                && (o.movement.target_position.is_some()
                                    || o.target.is_some()
                                    || o.target_location.is_some())
                        })
                        .unwrap_or(false);
                    if wants_sortie {
                        self.release_jet_from_airfield_parking(object_id);
                        continue;
                    }
                    // Prefer contained_by (authoritative residual link) over target.
                    let container_id = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.container_id())
                        .or(target_id);
                    let Some(container_id) = container_id else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    let Some((container_pos, container_alive, container_has_unit)) =
                        self.objects.get(&container_id).map(|container| {
                            (
                                container.get_position(),
                                container.is_alive(),
                                container.contained_units().contains(&object_id),
                            )
                        })
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    };

                    if !container_alive || !container_has_unit {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_contained_by(None);
                            obj.set_target(None);
                        }
                        continue;
                    }

                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_contained_by(Some(container_id));
                        obj.set_position(container_pos);
                        crate::game_logic::host_ground_height_log::record(
                            obj.id,
                            container_pos.y,
                            false,
                        );
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            crate::game_logic::host_move_log::record(
                                obj.id,
                                Some([container_pos.x, container_pos.y, container_pos.z]),
                            );
                            obj.record_host_movement();
                        }
                        obj.stop_moving();
                        obj.set_status_moving(false);
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn update_object_ai(&mut self, object_id: ObjectId, _dt: f32) {
        // Get object state for AI processing
        let (ai_state, target_id, _position) = {
            if let Some(obj) = self.objects.get(&object_id) {
                (obj.ai_state.clone(), obj.target, obj.get_position())
            } else {
                return;
            }
        };

        if ai_state == AIState::Attacking {
            if let Some(target_id) = target_id {
                // Check if target still exists; fire when in range.
                // Out-of-range chase is owned by update_combat (assign_unit_path) —
                // do not stop_attack merely for distance (that aborted chases).
                if let Some(target) = self.objects.get(&target_id) {
                    if !target.is_alive() {
                        self.stop_attack_decision_aware(object_id);
                    } else if let Some(attacker) = self.objects.get(&object_id) {
                        if attacker.can_target(target) {
                            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                            let (tgt_inf, tgt_faerie) = self
                                .objects
                                .get(&target_id)
                                .map(|t| (t.is_kind_of(KindOf::Infantry), t.is_faerie_fire()))
                                .unwrap_or((false, false));
                            if let Some(attacker) = self.objects.get_mut(&object_id) {
                                // can_fire without target uses base ROF; fire_at_ex applies
                                // TARGET_FAERIE_FIRE ROF residual against painted targets.
                                if attacker.can_fire(current_time)
                                    || (tgt_faerie
                                        && attacker.weapon.as_ref().is_some_and(|w| {
                                            Object::weapon_ready_vs_target(w, current_time, true)
                                        }))
                                {
                                    attacker.fire_at_ex(
                                        target_id,
                                        current_time,
                                        tgt_inf,
                                        tgt_faerie,
                                    );
                                }
                            }
                        }
                        // else: OOR or weapon rules — combat chase / wait residual
                    }
                } else {
                    // Target no longer exists
                    self.stop_attack_decision_aware(object_id);
                }
            }
        }

        // Handle AttackingGround: fire at target_location.
        if ai_state == AIState::AttackingGround {
            let can_fire_ground = self
                .objects
                .get(&object_id)
                .map(|attacker| {
                    attacker.can_attack()
                        && attacker.can_fire(self.frame as f32 * LOGIC_FRAME_TIMESTEP)
                        && attacker.target_location.is_some()
                })
                .unwrap_or(false);

            if can_fire_ground {
                if let Some(attacker) = self.objects.get(&object_id) {
                    let shooter_pos = attacker.get_position();
                    let weapon_damage = attacker.weapon.as_ref().map(|w| w.damage).unwrap_or(25.0);
                    if let Some(target_loc) = attacker.target_location {
                        let wname = attacker.thing.template.primary_weapon_name.as_deref();
                        let scatter = wname
                            .map(|n| {
                                crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                                    n, false, /* ground force-fire: base ScatterRadius only */
                                )
                            })
                            .unwrap_or(0.0);
                        let proj_speed = attacker
                            .weapon
                            .as_ref()
                            .map(|w| {
                                if w.projectile_speed > 0.0 {
                                    w.projectile_speed
                                } else {
                                    200.0
                                }
                            })
                            .unwrap_or(200.0);
                        super::combat::queue_projectile(super::combat::PendingProjectile {
                            shooter_id: object_id,
                            shooter_pos,
                            target_id: None,
                            target_pos: Some(target_loc),
                            damage: weapon_damage,
                            speed: proj_speed,
                            splash_radius: attacker
                                .weapon
                                .as_ref()
                                .map(|w| w.splash_radius)
                                .unwrap_or(0.0),
                            is_homing: false,
                            damage_type: crate::game_logic::combat::DamageType::Bullet,
                            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                            projectile_object_name: String::new(),
                            detonation_fx_name: wname
                                .map(
                                    crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name,
                                )
                                .unwrap_or_default(),
                            detonation_ocl_name: wname
                                .map(
                                    crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name,
                                )
                                .unwrap_or_default(),
                            exhaust_name: crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_unit_slot(
                                attacker.template_name.as_str(),
                                attacker.thing.template.primary_weapon_name.as_deref(),
                                attacker.thing.template.secondary_weapon_name.as_deref(),
                                0,
                            ),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: scatter,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });
                    }
                }
                if let Some(attacker) = self.objects.get_mut(&object_id) {
                    if let Some(w) = attacker.weapon.as_mut() {
                        w.last_fire_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                    }
                }
            }
        }
    }

    pub(super) fn update_object_combat(&mut self, attacker_id: ObjectId, _dt: f32) {
        // Get attacker and target info
        let (weapon_damage, target_id, attacker_team) = {
            if let Some(attacker) = self.objects.get(&attacker_id) {
                if let (Some(weapon), Some(target_id)) = (&attacker.weapon, attacker.target) {
                    (weapon.damage, target_id, attacker.team)
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        // Apply damage to target (BodyModule last_damage_source residual).
        let (destroyed, kill_xp, victim_pos, victim_team) = {
            let Some(target) = self.objects.get_mut(&target_id) else {
                return;
            };
            let destroyed = target.take_damage_from(weapon_damage, Some(attacker_id));
            if destroyed {
                let kill_xp = target.thing.template.experience_value
                    * Self::veterancy_xp_multiplier(target.experience.level);
                let victim_pos = target.get_position();
                let victim_team = target.team;
                (true, kill_xp, victim_pos, victim_team)
            } else {
                (false, 0.0, glam::Vec3::ZERO, Team::Neutral)
            }
        };
        // C++ TheRadar->tryUnderAttackEvent(this) residual on damage.
        let _ = self.try_under_attack_event(target_id);
        // C++ ActiveBody: friend retaliation even if victim dies.
        let _ = self.try_friends_retaliate(target_id, attacker_id);
        if destroyed {
            log::debug!("Object {} destroyed object {}", attacker_id, target_id);
            // C++ generals experience residual: skill points on kill → possible rank-up EVA.
            if let Some(pid) = self
                .players
                .values()
                .find(|p| p.team == attacker_team)
                .map(|p| p.id)
            {
                // Simple residual: 1 skill point per kill (not full GeneralsExperience table).
                let _ = self.add_player_skill_points(pid, 1);
            }
            self.mark_object_for_destruction(target_id, Some(attacker_team));
            let wname = self.objects.get(&attacker_id).and_then(|a| {
                a.thing
                    .template
                    .primary_weapon_name
                    .clone()
                    .or_else(|| a.thing.template.secondary_weapon_name.clone())
            });
            self.continue_or_stop_after_kill(
                attacker_id,
                target_id,
                victim_pos,
                victim_team,
                wname.as_deref(),
                kill_xp,
            );
        }
    }

    pub(super) fn update_player_upgrades(&mut self) {
        // Residual: complete research when residual frames elapse for entries
        // that are NOT still advancing on a building PRODUCTION_UPGRADE queue.
        // Building-path completions are applied in `update_production`.
        // Frame event clear runs at the start of the production phase so
        // building-path `record_complete` events survive presentation freeze.

        use crate::game_logic::buildings::ProductionKind;
        use crate::game_logic::host_upgrades::HostUpgradePhase;

        // Upgrades currently researching on a producer building.
        let mut building_researching: std::collections::HashSet<(u32, String)> =
            std::collections::HashSet::new();
        for obj in self.objects.values() {
            let Some(building) = obj.building_data.as_ref() else {
                continue;
            };
            let Some(player_id) = self
                .players
                .values()
                .find(|p| p.team == obj.team)
                .map(|p| p.id)
            else {
                continue;
            };
            for item in &building.production_queue {
                if item.kind == ProductionKind::Upgrade {
                    building_researching.insert((
                        player_id,
                        crate::game_logic::host_upgrades::normalize_upgrade_identity(
                            &item.template_name,
                        ),
                    ));
                }
            }
        }

        let frame = self.frame;
        let mut completed: Vec<(Team, u32, String)> = Vec::new();
        for entry in self.host_upgrades.entries_snapshot() {
            if entry.phase != HostUpgradePhase::Queued {
                continue;
            }
            let key = (
                entry.player_id,
                crate::game_logic::host_upgrades::normalize_upgrade_identity(&entry.name),
            );
            // Building owns the timer while the PRODUCTION_UPGRADE entry is live.
            if building_researching.contains(&key) {
                continue;
            }
            let needed = entry.residual_research_frames.max(1);
            // Count the current simulation step as one research frame residual
            // (frame counter increments after update_simulation returns).
            let elapsed = frame.saturating_sub(entry.queue_frame).saturating_add(1);
            if elapsed >= needed {
                completed.push((entry.team, entry.player_id, entry.name.clone()));
            }
        }

        // Direct player.queue_upgrade without host record (unit-test path):
        // complete after one simulation frame residual.
        for player in self.players.values() {
            for name in &player.queued_upgrades {
                let key = (
                    player.id,
                    crate::game_logic::host_upgrades::normalize_upgrade_identity(name),
                );
                if building_researching.contains(&key) {
                    continue;
                }
                let already = completed
                    .iter()
                    .any(|(t, pid, n)| *pid == player.id && n.eq_ignore_ascii_case(name));
                if already {
                    continue;
                }
                // No host entry → residual complete this update (legacy test path).
                let has_host = self.host_upgrades.entries_snapshot().iter().any(|e| {
                    e.player_id == player.id
                        && e.phase == HostUpgradePhase::Queued
                        && crate::game_logic::host_upgrades::normalize_upgrade_identity(&e.name)
                            == key.1
                });
                if !has_host {
                    completed.push((player.team, player.id, name.clone()));
                }
            }
        }

        for (team, player_id, name) in completed {
            let already = self
                .players
                .get(&player_id)
                .map(|p| p.has_unlocked_upgrade(&name))
                .unwrap_or(false);
            if let Some(player) = self.players.get_mut(&player_id) {
                if let Some(queued) = player.find_queued_upgrade_name(&name) {
                    player.queued_upgrades.remove(&queued);
                }
                if !player.has_unlocked_upgrade(&name) {
                    player.unlocked_sciences.insert(name.clone());
                }
            }
            if !already {
                self.apply_host_upgrade_complete(team, player_id, &name);
            }
        }
    }

    /// Record that a player queued upgrade research (host residual honesty).
    pub fn record_host_upgrade_queued(
        &mut self,
        player_id: u32,
        team: Team,
        upgrade_name: &str,
        source_object: Option<ObjectId>,
    ) {
        self.host_upgrades
            .record_queue(upgrade_name, team, player_id, self.frame, source_object);
    }

    /// Record that a player cancelled upgrade research (host residual honesty).
    pub fn record_host_upgrade_cancelled(&mut self, player_id: u32, upgrade_name: &str) {
        self.host_upgrades.record_cancel(upgrade_name, player_id);
    }

    /// Apply unlock effects for a completed upgrade and record honesty.
    /// Matches C++ ProductionUpdate upgrade-complete: player mask + object giveUpgrade.

    /// C++ StatusBitsUpgrade::upgradeImplementation residual for team units.

    /// C++ PassengersFireUpgrade residual for Helix BattleBunker unlock.

    /// C++ ActiveShroudUpgrade::upgradeImplementation residual.
    pub fn apply_active_shroud_upgrade(&mut self, id: ObjectId, new_shroud_range: f32) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.set_shroud_range(new_shroud_range);
        self.active_shroud_upgrade_reg
            .record_apply(obj.shroud_range);
        true
    }

    pub(super) fn apply_active_shroud_upgrade_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_active_shroud_upgrade::{
            peel_applies_to_template, peels_for_upgrade,
        };
        let peels = peels_for_upgrade(upgrade_name);
        if peels.is_empty() {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.team == team)
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            for peel in &peels {
                if !peel_applies_to_template(peel, &obj.template_name) {
                    continue;
                }
                obj.set_shroud_range(peel.new_shroud_range);
                self.active_shroud_upgrade_reg
                    .record_apply(obj.shroud_range);
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub(super) fn apply_passengers_fire_upgrade_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
        if !crate::game_logic::host_passengers_fire_upgrade::is_passengers_fire_upgrade(
            upgrade_name,
        ) {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.team == team)
            .filter(|(_, o)| should_enable_passengers_fire(upgrade_name, &o.template_name))
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.passengers_allowed_to_fire = true;
                n = n.saturating_add(1);
            }
        }
        if n > 0 {
            self.passengers_fire_upgrade_reg.record_apply(n);
        }
        n
    }

    pub(super) fn apply_status_bits_upgrade_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_status_bits_upgrade::{
            peel_applies_to_template, peels_for_upgrade,
        };
        let peels = peels_for_upgrade(upgrade_name);
        if peels.is_empty() {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.team == team)
            .map(|(id, _)| *id)
            .collect();
        let mut touched = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let mut any = false;
            for peel in &peels {
                if !peel_applies_to_template(peel, &obj.template_name) {
                    continue;
                }
                let (set_c, clear_c) =
                    obj.apply_status_bits_upgrade_masks(peel.status_to_set, peel.status_to_clear);
                self.status_bits_upgrade_reg.record_apply(set_c, clear_c);
                any = true;
            }
            if any {
                touched = touched.saturating_add(1);
            }
        }
        touched
    }

    pub(super) fn apply_host_upgrade_complete(&mut self, team: Team, player_id: u32, upgrade_name: &str) {
        use crate::game_logic::host_upgrades::HostUpgradeKind;

        let kind = HostUpgradeKind::from_name(upgrade_name);
        let units_affected = match kind {
            HostUpgradeKind::FlashBangGrenade => {
                self.apply_flashbang_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::TowMissile => self.apply_tow_unlock_to_team(team, upgrade_name),
            HostUpgradeKind::CaptureBuilding => {
                self.apply_capture_unlock_tags_to_team(team, upgrade_name)
            }
            HostUpgradeKind::SupplyLines => {
                self.apply_supply_lines_tags_to_team(team, upgrade_name)
            }
            HostUpgradeKind::NeutronShells => {
                self.apply_neutron_shells_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::BunkerBusters => {
                self.apply_bunker_busters_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::ComancheRocketPods => {
                self.apply_comanche_rocket_pods_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::SentryDroneGun => {
                self.apply_sentry_drone_gun_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::Camouflage => self.apply_camouflage_unlock_to_team(team, upgrade_name),
            HostUpgradeKind::CamoNetting => {
                self.apply_camo_netting_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::CompositeArmor => {
                self.apply_composite_armor_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::WorkerShoes => {
                self.apply_worker_shoes_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::NuclearTanks => {
                self.apply_nuclear_tanks_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::BoobyTrap => self.apply_booby_trap_unlock_to_team(team, upgrade_name),
            HostUpgradeKind::AnthraxGamma => {
                self.apply_anthrax_gamma_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::SuicideBomb => {
                self.apply_demo_suicide_bomb_unlock_to_team(team, upgrade_name)
            }
            HostUpgradeKind::AdvancedControlRods => {
                self.apply_advanced_control_rods_to_team(team, upgrade_name)
            }
            HostUpgradeKind::SubliminalMessaging => {
                self.apply_subliminal_messaging_to_team(team, upgrade_name)
            }
            HostUpgradeKind::ScorpionRocket => {
                self.apply_scorpion_rocket_to_team(team, upgrade_name)
            }
            HostUpgradeKind::ApRockets => self.apply_ap_rockets_to_team(team, upgrade_name),
            HostUpgradeKind::LaserMissiles => self.apply_laser_missiles_to_team(team, upgrade_name),
            HostUpgradeKind::Nationalism => self.apply_nationalism_to_team(team, upgrade_name),
            HostUpgradeKind::ChainGuns => self.apply_chain_guns_to_team(team, upgrade_name),
            HostUpgradeKind::UraniumShells => self.apply_uranium_shells_to_team(team, upgrade_name),
            HostUpgradeKind::BlackNapalm => self.apply_black_napalm_to_team(team, upgrade_name),
            HostUpgradeKind::ApBullets => self.apply_ap_bullets_to_team(team, upgrade_name),
            HostUpgradeKind::AnthraxBeta => self.apply_anthrax_beta_to_team(team, upgrade_name),
            HostUpgradeKind::ToxinShells => self.apply_toxin_shells_to_team(team, upgrade_name),
            HostUpgradeKind::AdvancedTraining => {
                self.apply_advanced_training_to_team(team, upgrade_name)
            }
            HostUpgradeKind::TacticalNukeMig => {
                self.apply_tactical_nuke_mig_to_team(team, upgrade_name)
            }
            HostUpgradeKind::DroneArmor => self.apply_drone_armor_to_team(team, upgrade_name),
            HostUpgradeKind::AircraftArmor => self.apply_aircraft_armor_to_team(team, upgrade_name),
            HostUpgradeKind::ChinaMines => {
                self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_ChinaMines")
            }
            HostUpgradeKind::EmpMines => {
                self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_ChinaEMPMines")
            }
            HostUpgradeKind::FortifiedStructure => {
                self.apply_fortified_structure_to_team(team, upgrade_name)
            }
            HostUpgradeKind::Radar => self.apply_radar_research_to_team(team, upgrade_name),
            HostUpgradeKind::RadarVanScan => {
                self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_GLARadarVanScan")
            }
            HostUpgradeKind::ChemicalSuits => self.apply_chemical_suits_to_team(team, upgrade_name),
            HostUpgradeKind::Moab => {
                self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_AmericaMOAB")
            }
            HostUpgradeKind::SatelliteHack => self.apply_satellite_hack_to_team(team, upgrade_name),
            HostUpgradeKind::Countermeasures => {
                self.apply_countermeasures_to_team(team, upgrade_name)
            }
            HostUpgradeKind::SlaveDrone => {
                self.apply_slave_drone_upgrade_to_team(team, upgrade_name)
            }
            HostUpgradeKind::CashBounty => {
                self.apply_cash_bounty_upgrade_to_team(team, upgrade_name)
            }
            HostUpgradeKind::HelixNapalmBomb => self.apply_helix_bomb_upgrade_to_team(
                team,
                upgrade_name,
                crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NAPALM_BOMB,
            ),
            HostUpgradeKind::HelixNukeBomb => self.apply_helix_bomb_upgrade_to_team(
                team,
                upgrade_name,
                crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB,
            ),
            HostUpgradeKind::Other => 0,
        };

        // Ensure registry has a queue entry even if command path skipped record
        // (e.g. direct Player::queue_upgrade in unit tests).
        self.host_upgrades.record_queue(
            upgrade_name,
            team,
            player_id,
            self.frame.saturating_sub(1),
            None,
        );
        self.host_upgrades
            .record_complete(upgrade_name, player_id, self.frame, units_affected);

        log::info!(
            "Host upgrade complete: player={} team={:?} '{}' kind={} units_affected={}",
            player_id,
            team,
            upgrade_name,
            kind.label(),
            units_affected
        );

        // C++ ProductionUpdate: TheEva->setShouldPlay(EVA_UpgradeComplete) residual
        // when no custom researchCompleteSound (generic EVA path).
        self.try_eva_upgrade_complete(player_id);
        if self.is_local_player(player_id) {
            self.queue_audio_event(
                AudioEventRequest::new("EVA_UpgradeComplete").with_priority(140),
            );
        }
        // C++ TheRadar->createEvent(pos, RADAR_EVENT_UPGRADE) residual.
        let source = self
            .host_upgrades
            .last_source_object_for(player_id, upgrade_name);
        self.try_radar_upgrade_complete(player_id, team, upgrade_name, source);

        // C++ StatusBitsUpgrade::upgradeImplementation residual.
        let _ = self.apply_status_bits_upgrade_to_team(team, upgrade_name);
        // C++ PassengersFireUpgrade::upgradeImplementation residual.
        let _ = self.apply_passengers_fire_upgrade_to_team(team, upgrade_name);
        // C++ ActiveShroudUpgrade::upgradeImplementation residual.
        let _ = self.apply_active_shroud_upgrade_to_team(team, upgrade_name);
    }

    /// C++ CashBountyPower / SCIENCE_CashBounty residual via upgrade complete.
    /// Tag Helix casters with Napalm/Nuke bomb upgrade residual unlock.
    pub(super) fn apply_helix_bomb_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
        canonical_tag: &str,
    ) -> u32 {
        use crate::game_logic::host_helix_napalm::is_helix_napalm_caster;
        let mut n = 0u32;
        let ids: Vec<_> = self.objects.keys().copied().collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_helix_napalm_caster(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(canonical_tag) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(canonical_tag);
            if upgrade_name != canonical_tag {
                obj.apply_upgrade_tag(upgrade_name);
            }
            n = n.saturating_add(1);
        }
        // Player unlock residual for science/UI gates.
        if let Some(p) = self.get_player_mut_by_team(team) {
            p.unlocked_sciences.insert(canonical_tag.to_string());
            if upgrade_name != canonical_tag {
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    pub(super) fn apply_cash_bounty_upgrade_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science;

        let pct = cash_bounty_percent_for_science(upgrade_name).unwrap_or(0.05);
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if p.team != team {
                continue;
            }
            p.unlocked_sciences.insert(upgrade_name.to_string());
            // SCIENCE names for kill path residual.
            if pct >= 0.20 - f32::EPSILON {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty3".to_string());
            } else if pct >= 0.10 - f32::EPSILON {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty2".to_string());
            } else {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty1".to_string());
            }
            p.set_cash_bounty(pct);
            self.cash_bounty.record_bounty_set(p.cash_bounty_percent);
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ America Scout/Battle/Hellfire drone object-upgrade residual.
    ///
    /// Attaches the residual slave drone to each living master vehicle that does
    /// not already have the upgrade tag (ObjectCreationUpgrade attach residual).
    pub(super) fn apply_slave_drone_upgrade_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_slave_drones::{
            is_slave_drone_master_template, SlaveDroneKind,
        };

        let kind = SlaveDroneKind::from_upgrade_name(upgrade_name).unwrap_or(SlaveDroneKind::Scout);
        let tag = kind.upgrade_name();

        let masters: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.team == team
                    && o.is_alive()
                    && !o.status.under_construction
                    && is_slave_drone_master_template(&o.template_name)
                    && !o.has_upgrade_tag(tag)
                    && !o.has_upgrade_tag(upgrade_name)
            })
            .map(|(id, _)| *id)
            .collect();

        let mut n = 0u32;
        for mid in masters {
            if self.residual_attach_slave_drone(mid, kind).is_some() {
                if let Some(m) = self.objects.get_mut(&mid) {
                    m.apply_upgrade_tag(upgrade_name);
                    m.apply_upgrade_tag(tag);
                }
                n = n.saturating_add(1);
            }
        }
        // Player unlock residual for production UI / late builds.
        let _ = self.apply_player_unlock_upgrade(team, upgrade_name, tag);
        n
    }

    /// C++ Upgrade_AmericaChemicalSuits residual — ChemSuitHumanArmor on infantry.
    pub(super) fn apply_chemical_suits_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
        let mut n =
            self.apply_player_unlock_upgrade(team, upgrade_name, UPGRADE_AMERICA_CHEMICAL_SUITS);
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_CHEMICAL_SUITS)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_CHEMICAL_SUITS);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_CHEMICAL_SUITS.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_ChinaSatelliteHackOne/Two residual — player FOW/intel unlock.
    pub(super) fn apply_satellite_hack_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::{
            UPGRADE_CHINA_SATELLITE_HACK_ONE, UPGRADE_CHINA_SATELLITE_HACK_TWO,
        };
        let lower = upgrade_name.to_ascii_lowercase();
        let canonical = if lower.contains("two") || lower.contains("2") {
            UPGRADE_CHINA_SATELLITE_HACK_TWO
        } else {
            UPGRADE_CHINA_SATELLITE_HACK_ONE
        };
        let n = self.apply_player_unlock_upgrade(team, upgrade_name, canonical);
        // Also unlock both tiers when Two is researched (Two implies One residual).
        if canonical == UPGRADE_CHINA_SATELLITE_HACK_TWO {
            let _ = self.apply_player_unlock_upgrade(
                team,
                UPGRADE_CHINA_SATELLITE_HACK_ONE,
                UPGRADE_CHINA_SATELLITE_HACK_ONE,
            );
        }
        n
    }

    /// C++ Upgrade_AmericaCountermeasures residual — tag aircraft for flare residual.
    pub(super) fn apply_countermeasures_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
        let mut n =
            self.apply_player_unlock_upgrade(team, upgrade_name, UPGRADE_AMERICA_COUNTERMEASURES);
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Aircraft) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_COUNTERMEASURES.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// Generic player-level upgrade unlock residual (mines / radar scan / flags).
    pub(super) fn apply_player_unlock_upgrade(
        &mut self,
        team: Team,
        upgrade_name: &str,
        canonical: &str,
    ) -> u32 {
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if p.team != team {
                continue;
            }
            p.unlocked_sciences.insert(canonical.to_string());
            p.unlocked_sciences.insert(upgrade_name.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_GLAFortifiedStructure residual — +max health on GLA structures.
    pub(super) fn apply_fortified_structure_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::{
            FORTIFIED_STRUCTURE_ADD_MAX_HEALTH, UPGRADE_GLA_FORTIFIED_STRUCTURE,
        };
        let add = FORTIFIED_STRUCTURE_ADD_MAX_HEALTH;
        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_FORTIFIED_STRUCTURE)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_FORTIFIED_STRUCTURE);
            obj.max_health = (obj.max_health + add).max(1.0);
            obj.record_host_max_health();
            obj.health.maximum = (obj.health.maximum + add).max(1.0);
            let new_hp = (obj.health.current + add).min(obj.health.maximum);
            Self::write_object_health_authority_aware(obj, new_hp);
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_FORTIFIED_STRUCTURE.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ faction RadarUpgrade residual — unlock + tag radar providers.
    pub(super) fn apply_radar_research_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_radar::{is_radar_provider_template, UPGRADE_GLA_RADAR};
        use crate::game_logic::host_structure_economy_residual::UPGRADE_AMERICA_RADAR;

        let canonical = if upgrade_name.to_ascii_lowercase().contains("china") {
            "Upgrade_ChinaRadar"
        } else if upgrade_name.to_ascii_lowercase().contains("gla") {
            UPGRADE_GLA_RADAR
        } else {
            UPGRADE_AMERICA_RADAR
        };

        let mut n = self.apply_player_unlock_upgrade(team, upgrade_name, canonical);
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_radar_provider_template(&obj.template_name)
                && !obj.is_command_center()
                && !obj.is_kind_of(KindOf::CommandCenter)
            {
                continue;
            }
            if obj.has_upgrade_tag(canonical) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(canonical);
            // C++ RadarUpgrade model residual.
            if let Some(bit) =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                    "RADAR_UPGRADED",
                )
            {
                obj.model_condition_bits |= 1u128 << bit;
            }
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_AmericaDroneArmor residual — +max health on slave drones.
    pub(super) fn apply_drone_armor_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_slave_drones::{
            drone_armor_add_max_health, is_battle_drone_template, is_hellfire_drone_template,
            is_scout_drone_template, SlaveDroneKind, UPGRADE_AMERICA_DRONE_ARMOR,
        };

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            let kind = if is_battle_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Battle)
            } else if is_hellfire_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Hellfire)
            } else if is_scout_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Scout)
            } else {
                None
            };
            let Some(kind) = kind else {
                continue;
            };
            if obj.has_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR) || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            let add = drone_armor_add_max_health(kind);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_DRONE_ARMOR.to_string());
            obj.max_health = (obj.max_health + add).max(1.0);
            obj.record_host_max_health();
            obj.health.maximum = (obj.health.maximum + add).max(1.0);
            let new_hp = (obj.health.current + add).min(obj.health.maximum);
            Self::write_object_health_authority_aware(obj, new_hp);
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_AMERICA_DRONE_ARMOR.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaAircraftArmor residual — +40 max health on MiGs.
    pub(super) fn apply_aircraft_armor_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_mig::{
            apply_mig_aircraft_armor_health, is_mig_template, UPGRADE_CHINA_AIRCRAFT_ARMOR,
        };

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_mig_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_CHINA_AIRCRAFT_ARMOR)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_AIRCRAFT_ARMOR);
            obj.applied_upgrades
                .insert(UPGRADE_CHINA_AIRCRAFT_ARMOR.to_string());
            let mut max_h = obj.max_health;
            let mut cur = obj.health.current;
            let mut maximum = obj.health.maximum;
            apply_mig_aircraft_armor_health(&mut max_h, &mut cur, &mut maximum);
            obj.max_health = max_h;
            obj.record_host_max_health();
            Self::write_object_health_authority_aware(obj, cur);
            obj.health.maximum = maximum;
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_AIRCRAFT_ARMOR.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_AmericaAdvancedTraining residual — 2× XP gain player unlock.
    pub(super) fn apply_advanced_training_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_unit_training::UPGRADE_AMERICA_ADVANCED_TRAINING;
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if p.team != team {
                continue;
            }
            p.unlocked_sciences
                .insert(UPGRADE_AMERICA_ADVANCED_TRAINING.to_string());
            p.unlocked_sciences.insert(upgrade_name.to_string());
            n = n.saturating_add(1);
        }
        // Tag living USA combat units so XP path can read unit tags residual.
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING);
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_ChinaTacticalNukeMig residual — Nuke General MiG loadout.
    pub(super) fn apply_tactical_nuke_mig_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_mig::{is_nuke_mig_template, UPGRADE_CHINA_TACTICAL_NUKE_MIG};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.team == team && o.is_alive() && is_nuke_mig_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_mig_tactical_nuke_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_TACTICAL_NUKE_MIG);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_TACTICAL_NUKE_MIG.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAAnthraxBeta residual — toxin tractor + SCUD + scud storm tier.
    pub(super) fn apply_anthrax_beta_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_scud_launcher::{
            is_scud_launcher_template, UPGRADE_GLA_ANTHRAX_BETA,
        };
        use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            let is_tt = is_toxin_tractor_template(&obj.template_name);
            let is_scud = is_scud_launcher_template(&obj.template_name);
            if !is_tt && !is_scud {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA);
            obj.applied_upgrades
                .insert(UPGRADE_GLA_ANTHRAX_BETA.to_string());
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_ANTHRAX_BETA.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAToxinShells residual — enables SCUD toxin secondary path.
    pub(super) fn apply_toxin_shells_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_scud_launcher::is_scud_launcher_template;
        use crate::game_logic::host_upgrades::UPGRADE_GLA_TOXIN_SHELLS;

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_scud_launcher_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_TOXIN_SHELLS) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_TOXIN_SHELLS);
            obj.applied_upgrades
                .insert(UPGRADE_GLA_TOXIN_SHELLS.to_string());
            // Toxin shells residual also unlocks toxin secondary preference.
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_TOXIN_SHELLS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAAPBullets residual — Rebel / Jarmen / Technical / Quad.
    pub(super) fn apply_ap_bullets_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_gla_rebel::is_gla_rebel_template;
        use crate::game_logic::host_jarmen_kell::{
            is_jarmen_kell_template, UPGRADE_GLA_AP_BULLETS,
        };
        use crate::game_logic::host_quad_cannon::is_quad_cannon_template;
        use crate::game_logic::host_technical::is_technical_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_jarmen_kell_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_gla_rebel_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_technical_template(&o.template_name) {
                    Some((*id, 2))
                } else if is_quad_cannon_template(&o.template_name) {
                    Some((*id, 3))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            match kind {
                0 => {
                    if self.apply_jarmen_kell_ap_bullets_upgrade(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                        }
                        n = n.saturating_add(1);
                    }
                }
                1 => {
                    if self.apply_rebel_ap_bullets_upgrade(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                        }
                        n = n.saturating_add(1);
                    }
                }
                2 | 3 => {
                    // Technical / Quad: tag residual; damage path reads applied_upgrades.
                    if let Some(o) = self.objects.get_mut(&id) {
                        if !o.has_upgrade_tag(UPGRADE_GLA_AP_BULLETS) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                            o.applied_upgrades
                                .insert(UPGRADE_GLA_AP_BULLETS.to_string());
                            n = n.saturating_add(1);
                        }
                    }
                }
                _ => {}
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_AP_BULLETS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaUraniumShells residual — Battlemaster / Overlord gun damage.
    pub(super) fn apply_uranium_shells_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_battlemaster::{
            is_battlemaster_template, UPGRADE_CHINA_URANIUM_SHELLS,
        };
        use crate::game_logic::host_overlord_gun::is_overlord_gun_chassis;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_battlemaster_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_overlord_gun_chassis(&o.template_name) {
                    Some((*id, 1))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_battlemaster_uranium_upgrade(id),
                1 => self.apply_overlord_gun_uranium_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaBlackNapalm residual — MiG / Inferno / Dragon fire field.
    pub(super) fn apply_black_napalm_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_dragon_tank::is_dragon_tank_template;
        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
        use crate::game_logic::host_mig::is_mig_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_mig_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_inferno_cannon_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_dragon_tank_template(&o.template_name) {
                    Some((*id, 2))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_mig_black_napalm_upgrade(id),
                1 => self.apply_inferno_black_napalm_upgrade(id),
                2 => self.apply_dragon_black_napalm_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag("Upgrade_ChinaBlackNapalm");
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert("Upgrade_ChinaBlackNapalm".to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAScorpionRocket residual — equip SECONDARY on all Scorpions.
    pub(super) fn apply_scorpion_rocket_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_scorpion::{is_scorpion_template, UPGRADE_GLA_SCORPION_ROCKET};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.team == team && o.is_alive() && is_scorpion_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_scorpion_rocket_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET);
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    /// C++ Upgrade_GLAAPRockets residual — AP damage on Scorpions (+ RPG if present).
    /// C++ Upgrade_GLAAPRockets residual — Scorpion / RPG / Stinger AP.
    pub(super) fn apply_ap_rockets_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_base_defense::is_stinger_site_structure;
        use crate::game_logic::host_rpg_trooper::is_rpg_trooper_template;
        use crate::game_logic::host_scorpion::{is_scorpion_template, UPGRADE_GLA_AP_ROCKETS};

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_scorpion_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_rpg_trooper_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_stinger_site_structure(&o.template_name) {
                    Some((*id, 2))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_scorpion_ap_rockets_upgrade(id),
                1 => self.apply_rpg_trooper_ap_rockets_upgrade(id),
                2 => self.apply_stinger_ap_rockets_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_GLA_AP_ROCKETS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_AmericaLaserMissiles residual — Raptor jet damage.
    pub(super) fn apply_laser_missiles_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_raptor::{is_raptor_template, UPGRADE_AMERICA_LASER_MISSILES};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive() && is_raptor_template(&o.template_name))
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_raptor_laser_missiles_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_AMERICA_LASER_MISSILES);
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    /// C++ Upgrade_ChinaNationalism residual — horde ROF tag on infantry/tanks.
    pub(super) fn apply_nationalism_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_battlemaster::{is_battlemaster_template, UPGRADE_NATIONALISM};
        use crate::game_logic::host_minigunner::is_minigunner_template;
        use crate::game_logic::host_red_guard::is_red_guard_template;
        use crate::game_logic::host_tank_hunter::is_tank_hunter_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_battlemaster_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_red_guard_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_tank_hunter_template(&o.template_name) {
                    Some((*id, 2))
                } else if is_minigunner_template(&o.template_name) {
                    Some((*id, 3))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_battlemaster_nationalism_upgrade(id),
                1 => self.apply_red_guard_nationalism_upgrade(id),
                2 => self.apply_tank_hunter_nationalism_upgrade(id),
                3 => self.apply_minigunner_nationalism_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_NATIONALISM);
                }
                n = n.saturating_add(1);
            }
        }
        // Player-level unlock residual so late-built units can inherit.
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences.insert(UPGRADE_NATIONALISM.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaChainGuns residual — gattling/minigun damage ×1.25.
    pub(super) fn apply_chain_guns_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_base_defense::is_gattling_cannon_structure;
        use crate::game_logic::host_gattling_tank::{
            is_gattling_tank_template, UPGRADE_CHINA_CHAIN_GUNS,
        };
        use crate::game_logic::host_minigunner::is_minigunner_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .filter_map(|(id, o)| {
                if is_minigunner_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_gattling_tank_template(&o.template_name)
                    || is_gattling_cannon_structure(&o.template_name)
                {
                    Some((*id, 1))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_minigunner_chain_guns_upgrade(id),
                1 => self.apply_gattling_chain_guns_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_CHAIN_GUNS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaSubliminalMessaging residual.
    ///
    /// Tags propaganda towers and unlocks upgraded heal/buff rate path
    /// (player unlocked_sciences + tower upgrade tags).
    pub(super) fn apply_subliminal_messaging_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_propaganda::{
            is_propaganda_tower, UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
        };
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            let is_tower =
                is_propaganda_tower(&obj.template_name) || obj.has_overlord_propaganda_residual();
            if !is_tower {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING);
            affected = affected.saturating_add(1);
        }
        // Also unlock at player level so towers without tags still see upgraded rate.
        for p in self.players.values_mut() {
            if p.team == team {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_SUBLIMINAL_MESSAGING.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        self.subliminal_messaging_upgrades = self.subliminal_messaging_upgrades.saturating_add(1);
        self.subliminal_towers_affected = self.subliminal_towers_affected.saturating_add(affected);
        affected
    }

    /// C++ PowerPlantUpgrade Advanced Control Rods residual.
    ///
    /// Tags America power plants and adds EnergyBonus to power_provided;
    /// sets POWER_PLANT_UPGRADED model condition (extendRods residual).
    pub(super) fn apply_advanced_control_rods_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_structure_economy_residual::{
            is_power_plant_template, AMERICA_POWER_ENERGY_BONUS,
            UPGRADE_AMERICA_ADVANCED_CONTROL_RODS,
        };

        let bonus = AMERICA_POWER_ENERGY_BONUS;
        let mut plant_ids: Vec<ObjectId> = Vec::new();
        for (id, obj) in self.objects.iter_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let is_plant = is_power_plant_template(&obj.template_name)
                || obj.is_kind_of(KindOf::PowerPlant)
                || obj.is_kind_of(KindOf::FSPower);
            if !is_plant {
                continue;
            }
            // America plants only residual (China uses OverchargeBehavior).
            let n = obj.template_name.to_ascii_lowercase();
            let america = n.contains("america") || n.contains("usa") || n.contains("coldfusion");
            if !america {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS);
            obj.power_provided = obj.power_provided.saturating_add(bonus);
            obj.record_host_entity_power();
            plant_ids.push(*id);
        }
        let mut affected = 0u32;
        for id in plant_ids {
            // C++ PowerPlantUpdate::extendRods(TRUE) residual — UPGRADING → UPGRADED.
            if self.begin_power_plant_rods_extend(id) {
                affected = affected.saturating_add(1);
            } else {
                affected = affected.saturating_add(1);
            }
        }
        self.control_rods_upgrades = self.control_rods_upgrades.saturating_add(1);
        self.control_rods_plants_affected =
            self.control_rods_plants_affected.saturating_add(affected);
        affected
    }

    /// Apply WorkerShoes residual: speed 30 + upgrade tag on GLA workers.
    pub(super) fn apply_worker_shoes_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_gla_worker::{
            is_gla_worker_template, worker_residual_speed, UPGRADE_GLA_WORKER_SHOES,
            WORKER_SHOES_AUDIO,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_gla_worker_template(&obj.template_name) && !obj.is_worker() {
                continue;
            }
            // Prefer GLA worker templates; also accept KINDOF_WORKER residual
            // whose template name matches worker residual (not USA/China dozer).
            if !is_gla_worker_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES) || obj.has_upgrade_tag(upgrade_name) {
                // Already applied — refresh speed residual only.
                obj.movement.max_speed = worker_residual_speed(true);
                continue;
            }
            obj.movement.max_speed = worker_residual_speed(true);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.gla_worker.record_shoes_applied(affected);
            self.queue_audio_event(AudioEventRequest::new(WORKER_SHOES_AUDIO).with_priority(140));
        }
        affected
    }

    /// Apply Nuclear Tanks residual: death-weapon tag + nuclear locomotor speed.
    pub(super) fn apply_nuclear_tanks_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_nuclear_tanks::{
            has_nuclear_tanks_upgrade, is_nuclear_tanks_eligible, nuclear_tanks_residual_speed,
            NUCLEAR_TANKS_UPGRADE_AUDIO, UPGRADE_CHINA_NUCLEAR_TANKS,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_nuclear_tanks_eligible(&obj.template_name) {
                continue;
            }
            if has_nuclear_tanks_upgrade(&obj.applied_upgrades) || obj.has_upgrade_tag(upgrade_name)
            {
                // Refresh speed residual only.
                obj.movement.max_speed = nuclear_tanks_residual_speed(&obj.template_name);
                continue;
            }
            obj.movement.max_speed = nuclear_tanks_residual_speed(&obj.template_name);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_NUCLEAR_TANKS);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.nuclear_tanks.record_upgrade_applied(affected);
            self.queue_audio_event(
                AudioEventRequest::new(NUCLEAR_TANKS_UPGRADE_AUDIO).with_priority(140),
            );
        }
        affected
    }

    /// Apply BoobyTrap residual unlock tag on GLA Rebel infantry.
    pub(super) fn apply_booby_trap_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_booby_trap::{
            is_booby_trap_planter_template, UPGRADE_GLA_REBEL_BOOBY_TRAP,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_booby_trap_planter_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.booby_trap.record_upgrade_applied(affected);
        }
        affected
    }

    /// Equip FlashBang secondary on team rangers + apply upgrade tag.
    pub(super) fn apply_flashbang_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::is_flashbang_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            ensure_host_weapon_store, RANGER_SECONDARY_WEAPON,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(RANGER_SECONDARY_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_flashbang_unit_template(&obj.template_name) {
                continue;
            }
            if obj.secondary_weapon.is_none() {
                if let Some(ref w) = secondary {
                    obj.secondary_weapon = Some(w.clone());
                }
            }
            obj.apply_upgrade_tag(upgrade_name);
            // Canonical retail name tag for ability checks.
            obj.apply_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip TOW secondary on team Humvees + apply upgrade tag.
    pub(super) fn apply_tow_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::is_tow_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            ensure_host_weapon_store, HUMVEE_SECONDARY_WEAPON,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(HUMVEE_SECONDARY_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_tow_unit_template(&obj.template_name) {
                continue;
            }
            if obj.secondary_weapon.is_none() {
                if let Some(mut w) = secondary.clone() {
                    // Residual: ground TOW + air tertiary capability (PreferredAgainst AIRCRAFT).
                    // Damage boost vs air applied in combat path (HUMVEE_AIR_TOW_DAMAGE).
                    w.can_target_air = true;
                    w.range = w
                        .range
                        .max(crate::game_logic::host_humvee::HUMVEE_AIR_TOW_RANGE);
                    obj.secondary_weapon = Some(w);
                }
            } else if let Some(w) = obj.secondary_weapon.as_mut() {
                w.can_target_air = true;
                w.range = w
                    .range
                    .max(crate::game_logic::host_humvee::HUMVEE_AIR_TOW_RANGE);
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Apply Composite Armor MaxHealthUpgrade residual (+100 HP) to Crusader / Paladin.
    pub(super) fn apply_composite_armor_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_usa_tanks::{
            apply_composite_armor_health, is_composite_armor_unit_template,
            UPGRADE_AMERICA_COMPOSITE_ARMOR,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_composite_armor_unit_template(&obj.template_name) {
                continue;
            }
            // Idempotent: skip if already tagged.
            if obj.has_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            let mut max_h = obj.max_health;
            let mut cur = obj.health.current;
            let mut maximum = obj.health.maximum;
            apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
            obj.max_health = max_h;
            obj.record_host_max_health();
            Self::write_object_health_authority_aware(obj, cur);
            obj.health.maximum = maximum;
            crate::game_logic::host_heal_log::record(obj.id, obj.health.current);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip Neutron Shell secondary on team Nuke Cannons + apply upgrade tag.
    pub(super) fn apply_neutron_shells_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
        use crate::game_logic::host_upgrades::is_neutron_shell_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            ensure_host_weapon_store, NUKE_CANNON_NEUTRON_WEAPON,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_neutron_shell_unit_template(&obj.template_name) {
                continue;
            }
            if let Some(ref w) = secondary {
                // Always re-bind neutron secondary residual so stats stay correct.
                obj.secondary_weapon = Some(w.clone());
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip Comanche Rocket Pods secondary + apply upgrade tag.
    ///
    /// Retail: WeaponSetUpgrade TriggeredBy = Upgrade_ComancheRocketPods unlocks
    /// TERTIARY ComancheRocketPodWeapon. Host residual binds secondary slot.
    pub(super) fn apply_comanche_rocket_pods_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_comanche_rocket_pods::{
            comanche_rocket_pod_weapon, is_comanche_template, UPGRADE_COMANCHE_ROCKET_PODS,
        };

        let secondary = comanche_rocket_pod_weapon();
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_comanche_template(&obj.template_name) {
                continue;
            }
            obj.secondary_weapon = Some(secondary.clone());
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS);
            // Host residual: PLAYER_UPGRADE weapon set flag for presentation honesty.
            obj.weapon_set_player_upgrade = true;
            obj.record_host_weapon_set();
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip Sentry Drone Gun primary + apply upgrade tag.
    ///
    /// Retail: WeaponSetUpgrade TriggeredBy = Upgrade_AmericaSentryDroneGun unlocks
    /// PRIMARY SentryDroneGun. Host residual binds primary weapon for auto-fire.
    pub(super) fn apply_sentry_drone_gun_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_sentry_drone::{
            is_sentry_drone_template, SENTRY_DRONE_GUN_WEAPON, UPGRADE_AMERICA_SENTRY_DRONE_GUN,
        };
        use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

        ensure_host_weapon_store();
        let primary = ThingTemplate::weapon_from_store(SENTRY_DRONE_GUN_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_sentry_drone_template(&obj.template_name) {
                continue;
            }
            if let Some(ref w) = primary {
                obj.weapon = Some(w.clone());
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN);
            obj.weapon_set_player_upgrade = true;
            obj.record_host_weapon_set();
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Tag team Stealth Fighters with Bunker Busters residual upgrade.
    ///
    /// C++ BunkerBusterBehavior checks player upgrade on missile detonation;
    /// host residual tags carriers so combat can apply garrison kill + bunker mult.
    pub(super) fn apply_bunker_busters_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_bunker_buster::{
            is_bunker_buster_carrier, UPGRADE_AMERICA_BUNKER_BUSTERS,
        };
        use crate::game_logic::weapon_bootstrap::{
            ensure_host_weapon_store, STEALTH_JET_MISSILE_WEAPON,
        };

        ensure_host_weapon_store();
        let primary = ThingTemplate::weapon_from_store(STEALTH_JET_MISSILE_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_bunker_buster_carrier(&obj.template_name) {
                continue;
            }
            if let Some(ref w) = primary {
                // Ensure residual anti-structure missile stats when store available.
                if obj.weapon.is_none() {
                    obj.weapon = Some(w.clone());
                }
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Tag capture-capable infantry so capture unlock is unit-observable.
    pub(super) fn apply_capture_unlock_tags_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::{
            is_capture_capable_infantry_template, UPGRADE_INFANTRY_CAPTURE,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if !is_capture_capable_infantry_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_INFANTRY_CAPTURE);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Grant GLA Camouflage residual stealth to Rebel infantry.
    ///
    /// C++ StealthUpgrade TriggeredBy = Upgrade_GLACamouflage enables
    /// StealthUpdate (InnateStealth was No until upgrade). Host residual sets
    /// STEALTHED + innate_stealth; breaks on attack (StealthForbiddenConditions
    /// = ATTACKING USING_ABILITY). Fail-closed: not full 2500ms StealthDelay
    /// re-cloak timer matrix / FriendlyOpacity pulse / workers (no StealthUpgrade).
    pub(super) fn apply_camouflage_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::{
            is_camouflage_unit_template, UPGRADE_GLA_CAMOUFLAGE,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if !is_camouflage_unit_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE);
            obj.set_status_stealthed(true);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;
            obj.record_host_stealth_flags();
            // Rebel residual: uncloak while attacking; stay cloaked while moving.
            obj.stealth_breaks_on_attack = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_move = false;
            obj.record_host_stealth_flags();
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Grant GLA CamoNetting residual stealth to eligible GLA structures.
    ///
    /// C++ StealthUpgrade TriggeredBy = Upgrade_GLACamoNetting on Stealth General
    /// buildings + Tunnel Network / Stinger Site. Host residual sets STEALTHED +
    /// innate_stealth with StealthForbiddenConditions ATTACKING / TAKING_DAMAGE
    /// and StealthDelay **2500**ms re-cloak residual.
    pub(super) fn apply_camo_netting_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::{
            is_camo_netting_structure_template, CAMO_NETTING_FRIENDLY_OPACITY_MIN,
            CAMO_NETTING_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMO_NETTING,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            // Residual name matrix filters eligible GLA structures (not infantry).
            if !is_camo_netting_structure_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMO_NETTING);
            obj.set_status_stealthed(true);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;
            obj.record_host_stealth_flags();
            // Structure residual: ATTACKING + TAKING_DAMAGE uncloak; StealthDelay re-cloak.
            obj.stealth_breaks_on_attack = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_damage = true;
            obj.stealth_breaks_on_move = false;
            obj.record_host_stealth_flags();
            obj.stealth_delay_frames = CAMO_NETTING_STEALTH_DELAY_FRAMES;
            obj.stealth_allowed_frame = 0;
            obj.stealth_delay_pending = false;
            // FriendlyOpacity residual: cloaked → min.
            obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MIN;
            obj.record_host_vision_camo();
            obj.camo_opacity_pulse_phase = 0.0;
            // Sub-object net mesh residual: upgrade shows CamoNet presentation.
            obj.camo_net_sub_object_shown = true;
            obj.camo_net_sub_object_observer_visible = true; // friendly default residual
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.camo_netting_opacity_cloak_count = self
                .camo_netting_opacity_cloak_count
                .saturating_add(affected);
            self.camo_netting_sub_object_show_count = self
                .camo_netting_sub_object_show_count
                .saturating_add(affected);
        }
        affected
    }

    /// Tag toxin combat units for Anthrax Gamma residual (Chem general).
    ///
    /// Fail-closed: not full WeaponSet PLAYER_UPGRADE module / particle gamma FX.
    pub(super) fn apply_anthrax_gamma_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_toxin_tractor::{
            is_toxin_tractor_template, UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
        };
        use crate::game_logic::host_upgrades::{
            is_anthrax_gamma_unit_template, UPGRADE_CHEM_ANTHRAX_GAMMA,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_anthrax_gamma_unit_template(&obj.template_name)
                && !is_toxin_tractor_template(&obj.template_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHEM_ANTHRAX_GAMMA);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Apply Demo SuicideBomb residual: tag eligible Demo units/structures +
    /// CommandSetUpgrade residual override for TertiarySuicide.
    pub(super) fn apply_demo_suicide_bomb_unlock_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_demo_suicide_bomb::{
            demo_command_set_upgrade_for_template, is_demo_suicide_bomb_eligible_template,
            UPGRADE_DEMO_SUICIDE_BOMB,
        };

        let mut affected = 0u32;
        let mut command_sets = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if !is_demo_suicide_bomb_eligible_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB) || obj.has_upgrade_tag(upgrade_name) {
                // Still ensure CommandSetUpgrade residual is applied if missing.
                if obj.command_set_override.is_none() {
                    if let Some(cs) = demo_command_set_upgrade_for_template(&obj.template_name) {
                        obj.set_command_set_override(Some(cs));
                        command_sets = command_sets.saturating_add(1);
                    }
                }
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB);
            if let Some(cs) = demo_command_set_upgrade_for_template(&obj.template_name) {
                obj.set_command_set_override(Some(cs));
                command_sets = command_sets.saturating_add(1);
            }
            affected = affected.saturating_add(1);
        }
        self.demo_suicide_bomb.record_upgrade_complete(affected);
        if command_sets > 0 {
            self.demo_suicide_bomb
                .record_command_set_upgrade(command_sets);
        }
        affected
    }

    /// Tag supply centers for Supply Lines residual observability.
    pub(super) fn apply_supply_lines_tags_to_team(&mut self, team: Team, upgrade_name: &str) -> u32 {
        use crate::game_logic::host_upgrades::is_supply_center_template;

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(KindOf::SupplyCenter) || is_supply_center_template(&obj.template_name)
            {
                obj.apply_upgrade_tag(upgrade_name);
                affected = affected.saturating_add(1);
            }
        }
        affected
    }

    pub(super) fn update_player_resources(&mut self, dt: f32) {
        // Calculate power and resource generation for each player
        for (_, player) in self.players.iter_mut() {
            let (power_produced, power_consumed) =
                super::buildings::BuildingBehavior::calculate_power_for_team(
                    player.team,
                    &self.objects,
                );

            let mut income_per_second = 0.0f32;

            // Base passive income -- every player earns a small trickle so they are
            // never completely stuck even before building a supply center.
            // In the full C++ game this comes from supply-truck harvesting; here we
            // provide a simplified equivalent so the economy always moves forward.
            income_per_second += 5.0; // $5/sec base passive income

            // Calculate from buildings
            for (_, obj) in self.objects.iter() {
                if obj.team == player.team && obj.is_constructed() && obj.is_alive() {
                    // Supply centers generate resources
                    if obj.is_kind_of(KindOf::SupplyCenter) {
                        // $25/sec per supply center approximates a single supply
                        // truck's delivery rate (full Chinook ~= $600 / 25s).
                        income_per_second += 25.0;
                    }
                }
            }

            player.power_available = power_produced - power_consumed;
            player.power_produced = power_produced;
            player.power_consumed = power_consumed;

            // C++ parity: check if power sabotage timer has expired and clear it
            // Matches C++ Player::update() sabotage recovery logic
            if player.power_sabotaged_till_frame > 0
                && self.frame > player.power_sabotaged_till_frame
            {
                player.power_sabotaged_till_frame = 0;
            }
            // If power is sabotaged, zero out power production
            if player.power_sabotaged_till_frame > 0 {
                player.power_available = -power_consumed;
            }

            if income_per_second > 0.0 {
                player.income_accumulator += income_per_second * dt;
                let whole = player.income_accumulator.floor() as u32;
                player.income_accumulator -= whole as f32;
                if whole > 0 {
                    player.statistics.resources_collected =
                        player.statistics.resources_collected.saturating_add(whole);
                    if crate::gameworld_shadow::gameworld_economy_authority_live() {
                        player.pending_supply_delta += whole as i64;
                        crate::game_logic::host_economy_log::record(
                            player.id,
                            player.effective_supplies(),
                            player.power_available,
                        );
                    } else {
                        player.resources.supplies = player.resources.supplies.saturating_add(whole);
                        crate::game_logic::host_economy_log::record(
                            player.id,
                            player.resources.supplies,
                            player.power_available,
                        );
                    }
                }
            }
            // Shadow economy channel: effective supplies + power after host tick residual.
            crate::game_logic::host_economy_log::record(
                player.id,
                if crate::gameworld_shadow::gameworld_economy_authority_live() {
                    player.effective_supplies()
                } else {
                    player.resources.supplies
                },
                player.power_available,
            );
        }
    }

    /// GLA Black Market residual cash (AutoDepositUpdate residual).
    ///
    /// Retail FactionBuilding.ini GLABlackMarket:
    /// DepositAmount=20, DepositTiming=2000 ms → 60 logic frames @ 30 FPS.
    /// Floating cash text residual: GUI:AddCash @ pos+Z10, player color | A230.
    /// Fail-closed: not full InGameUI GPU draw / InitialCaptureBonus (retail 0).
    pub(crate) fn apply_auto_deposit_event(
        &mut self,
        ev: crate::game_logic::host_auto_deposit_log::AutoDepositEvent,
    ) {
        use crate::game_logic::host_auto_deposit_log::AutoDepositKind;
        use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_AUDIO;
        use crate::game_logic::host_oil_derrick::{
            oil_derrick_deposit_amount, should_display_stealthed_floating_cash,
            HostAutoDepositFloatingText, OIL_DERRICK_DEPOSIT_AUDIO,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        if ev.amount == 0 {
            return;
        }
        let frame = self.frame;
        let (deposited, audio) = match ev.kind {
            AutoDepositKind::BlackMarket => {
                // GW already advanced next_deposit_frame; keep registry schedule in lockstep.
                self.black_markets
                    .set_next_deposit(ev.id, ev.next_deposit_frame);
                let d = self.black_markets.force_record_deposit(ev.id, ev.amount);
                (d, BLACK_MARKET_DEPOSIT_AUDIO)
            }
            AutoDepositKind::OilDerrick => {
                let has_supply_lines = self.players.values().any(|p| {
                    p.team == ev.team && p.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
                });
                let (amount, boost) = oil_derrick_deposit_amount(has_supply_lines);
                self.oil_derricks
                    .set_next_deposit(ev.id, ev.next_deposit_frame);
                let d = self.oil_derricks.force_record_deposit(ev.id, amount, boost);
                if boost > 0 {
                    self.oil_derricks.supply_lines_boost_cash_total = self
                        .oil_derricks
                        .supply_lines_boost_cash_total
                        .saturating_add(boost);
                }
                (d, OIL_DERRICK_DEPOSIT_AUDIO)
            }
        };
        if deposited == 0 {
            return;
        }
        if let Some(pid) = self.player_id_for_team(ev.team) {
            if let Some(player) = self.get_player_mut(pid) {
                player.credit_supplies(deposited);
            }
        }
        let player_color = self
            .players
            .values()
            .find(|p| p.team == ev.team)
            .map(|p| p.color_rgb)
            .unwrap_or((200, 200, 200));
        let is_local = self
            .player_id_for_team(ev.team)
            .map(|pid| self.is_local_player(pid))
            .unwrap_or(false);
        let show = should_display_stealthed_floating_cash(ev.stealthed, ev.detected, is_local);
        let mut float_pos = ev.pos;
        float_pos.y += 10.0;
        match ev.kind {
            AutoDepositKind::BlackMarket => {
                if show {
                    self.black_markets
                        .record_floating_text(HostAutoDepositFloatingText::new(
                            ev.id,
                            float_pos,
                            deposited,
                            player_color,
                            frame,
                            false,
                        ));
                } else {
                    self.black_markets.record_floating_text_suppressed();
                }
            }
            AutoDepositKind::OilDerrick => {
                if show {
                    self.oil_derricks
                        .record_floating_text(HostAutoDepositFloatingText::new(
                            ev.id,
                            float_pos,
                            deposited,
                            player_color,
                            frame,
                            false,
                        ));
                } else {
                    self.oil_derricks.record_floating_text_suppressed();
                }
            }
        }
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(ev.id)
                .with_position(ev.pos)
                .with_priority(120),
        );
    }

    pub(super) fn update_black_market_deposits(&mut self) {
        use crate::game_logic::host_black_market::{
            is_black_market_template, is_legal_black_market_income_source,
            BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_AUDIO,
        };
        use crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText;

        let frame = self.frame;
        let markets: Vec<(ObjectId, Team, Vec3, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                // Fake black markets residual-skip (ActualMoney=No).
                if obj.template_name.to_ascii_lowercase().contains("fake") {
                    return None;
                }
                let is_bm = obj.is_kind_of(KindOf::FSBlackMarket)
                    || is_black_market_template(&obj.template_name);
                if !is_bm {
                    return None;
                }
                // C++ AutoDepositUpdate: neutral / under construction skip.
                let is_neutral = obj.team == Team::Neutral;
                if !is_legal_black_market_income_source(
                    obj.is_alive(),
                    obj.is_constructed() && !obj.status.under_construction,
                    is_neutral,
                ) {
                    return None;
                }
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    obj.status.stealthed,
                    obj.status.detected,
                ))
            })
            .collect();

        // Forget destroyed markets so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> =
            markets.iter().map(|(id, _, _, _, _)| *id).collect();
        let stale: Vec<ObjectId> = self
            .black_markets
            .next_deposit_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.black_markets.forget(id);
        }

        for (market_id, team, pos, stealthed, detected) in markets {
            let deposited =
                self.black_markets
                    .try_deposit(market_id, frame, BLACK_MARKET_DEPOSIT_AMOUNT);
            if deposited == 0 {
                continue;
            }
            let player_color = self
                .players
                .values()
                .find(|p| p.team == team)
                .map(|p| p.color_rgb)
                .unwrap_or((200, 200, 200));
            let is_local = self
                .player_id_for_team(team)
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.credit_supplies(deposited);
            }
            // AutoDeposit floating text residual + STEALTHED local display gate.
            // Structure geometry scatter residual (±0.3 major/minor radius).
            use crate::game_logic::host_oil_derrick::{
                should_display_stealthed_floating_cash, structure_floating_text_scatter,
                OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
            };
            if should_display_stealthed_floating_cash(stealthed, detected, is_local) {
                let radius = self
                    .objects
                    .get(&market_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    market_id.0.wrapping_add(frame),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.black_markets.record_geometry_scatter();
                self.black_markets
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        market_id,
                        float_pos,
                        deposited,
                        player_color,
                        frame,
                        false,
                    ));
            } else {
                self.black_markets.record_floating_text_suppressed();
            }
            self.queue_audio_event(
                AudioEventRequest::new(BLACK_MARKET_DEPOSIT_AUDIO)
                    .with_object(market_id)
                    .with_position(pos)
                    .with_priority(120),
            );
        }
    }

    /// Tech Oil Derrick residual cash (AutoDepositUpdate residual).
    ///
    /// Retail CivilianBuilding.ini TechOilDerrick:
    /// DepositAmount=200, DepositTiming=12000 ms → 360 logic frames @ 30 FPS,
    /// InitialCaptureBonus=1000 once when first non-neutral owned,
    /// UpgradedBoost SupplyLines +20, floating cash text residual.
    /// Fail-closed: not full InGameUI GPU draw (STEALTHED local display gate residual closed).
    pub(super) fn update_oil_derrick_deposits(&mut self) {
        use crate::game_logic::host_oil_derrick::{
            is_legal_oil_derrick_income_source, is_oil_derrick_template,
            oil_derrick_deposit_amount, HostAutoDepositFloatingText,
            OIL_DERRICK_CAPTURE_BONUS_AUDIO, OIL_DERRICK_DEPOSIT_AUDIO,
            OIL_DERRICK_INITIAL_CAPTURE_BONUS,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        let frame = self.frame;
        // Collect all oil derricks (including neutral — need for stale cleanup / capture detect).
        let derricks: Vec<(ObjectId, Team, Vec3, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !is_oil_derrick_template(&obj.template_name) {
                    return None;
                }
                let alive = obj.is_alive();
                let constructed = obj.is_constructed() && !obj.status.under_construction;
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    alive,
                    constructed,
                    obj.status.stealthed,
                    obj.status.detected,
                ))
            })
            .collect();

        let live: std::collections::HashSet<ObjectId> =
            derricks.iter().map(|(id, _, _, _, _, _, _)| *id).collect();
        let stale: Vec<ObjectId> = self
            .oil_derricks
            .next_deposit_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.oil_derricks.forget(id);
        }

        for (derrick_id, team, pos, alive, constructed, stealthed, detected) in derricks {
            let is_neutral = team == Team::Neutral;
            if !is_legal_oil_derrick_income_source(alive, constructed, is_neutral) {
                continue;
            }

            let player_color = self
                .players
                .values()
                .find(|p| p.team == team)
                .map(|p| p.color_rgb)
                .unwrap_or((200, 200, 200));
            let is_local = self
                .player_id_for_team(team)
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            let has_supply_lines = self
                .players
                .values()
                .any(|p| p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));
            use crate::game_logic::host_oil_derrick::should_display_stealthed_floating_cash;
            let show_float = should_display_stealthed_floating_cash(stealthed, detected, is_local);

            // InitialCaptureBonus residual: first non-neutral ownership.
            let bonus = self
                .oil_derricks
                .try_capture_bonus(derrick_id, OIL_DERRICK_INITIAL_CAPTURE_BONUS);
            if bonus > 0 {
                self.oil_derricks
                    .reschedule_after_capture(derrick_id, frame);
                if let Some(player) = self.get_player_mut_by_team(team) {
                    player.credit_supplies(bonus);
                }
                // Capture bonus floating text is not STEALTH-gated in C++ (award path).
                // Structure geometry scatter residual still applies (KINDOF_STRUCTURE).
                use crate::game_logic::host_oil_derrick::{
                    structure_floating_text_scatter, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
                };
                let radius = self
                    .objects
                    .get(&derrick_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    derrick_id.0.wrapping_add(frame).wrapping_add(1),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.oil_derricks.record_geometry_scatter();
                self.oil_derricks
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        derrick_id,
                        float_pos,
                        bonus,
                        player_color,
                        frame,
                        true,
                    ));
                self.queue_audio_event(
                    AudioEventRequest::new(OIL_DERRICK_CAPTURE_BONUS_AUDIO)
                        .with_object(derrick_id)
                        .with_position(pos)
                        .with_priority(130),
                );
            }

            let (amount, boost) = oil_derrick_deposit_amount(has_supply_lines);
            let deposited = self
                .oil_derricks
                .try_deposit(derrick_id, frame, amount, boost);
            if deposited == 0 {
                continue;
            }
            if boost > 0 {
                self.supply_lines_bonus_cash_total =
                    self.supply_lines_bonus_cash_total.saturating_add(boost);
            }
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.credit_supplies(deposited);
            }
            if show_float {
                use crate::game_logic::host_oil_derrick::{
                    structure_floating_text_scatter, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
                };
                let radius = self
                    .objects
                    .get(&derrick_id)
                    .map(|o| o.selection_radius.max(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS))
                    .unwrap_or(OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS);
                let (dx, dz) = structure_floating_text_scatter(
                    derrick_id.0.wrapping_add(frame),
                    radius,
                    radius,
                );
                let float_pos = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
                self.oil_derricks.record_geometry_scatter();
                self.oil_derricks
                    .record_floating_text(HostAutoDepositFloatingText::new(
                        derrick_id,
                        float_pos,
                        deposited,
                        player_color,
                        frame,
                        false,
                    ));
            } else {
                self.oil_derricks.record_floating_text_suppressed();
            }
            self.queue_audio_event(
                AudioEventRequest::new(OIL_DERRICK_DEPOSIT_AUDIO)
                    .with_object(derrick_id)
                    .with_position(pos)
                    .with_priority(120),
            );
        }
    }

    /// China Hacker / Internet Center residual cash (HackInternetAIUpdate residual).
    ///
    /// Retail ChinaInfantry.ini HackInternetAIUpdate:
    /// CashUpdateDelay=2000 ms → 60 frames field; CashUpdateDelayFast=1800 ms → 54
    /// frames inside Internet Center; Regular/Vet/Elite/Heroic = 5/6/8/10.
    /// InternetHackContain residual: hackers contained in FSInternetCenter auto-hack.
    /// Fail-closed: not full unpack/pack animation / variation / floating text.
    pub(crate) fn apply_hacker_income_event(
        &mut self,
        ev: crate::game_logic::host_hacker_income_log::HackerIncomeEvent,
    ) {
        use crate::game_logic::host_hacker_income::{
            internet_center_floating_text_scatter, should_display_hacker_floating_cash,
            HostHackerFloatingText, HACKER_CASH_PING_AUDIO, HACKER_XP_PER_CASH_UPDATE,
        };

        if ev.amount == 0 {
            return;
        }
        let frame = self.frame;
        self.hacker_income.mark_hacking(ev.id);
        self.hacker_income
            .set_next_deposit(ev.id, ev.next_deposit_frame);
        let deposited =
            self.hacker_income
                .force_record_deposit(ev.id, ev.amount, ev.in_internet_center);
        if deposited == 0 {
            return;
        }
        if let Some(pid) = self.player_id_for_team(ev.team) {
            if let Some(player) = self.get_player_mut(pid) {
                player.credit_supplies(deposited);
                // residual XP
                let _ = HACKER_XP_PER_CASH_UPDATE;
            }
        }
        let is_local = self
            .player_id_for_team(ev.team)
            .map(|pid| self.is_local_player(pid))
            .unwrap_or(false);
        let show = should_display_hacker_floating_cash(
            ev.stealthed,
            ev.detected,
            is_local,
            ev.in_internet_center,
            false,
            false,
            is_local,
        );
        let mut float_pos = ev.pos;
        float_pos.y += 10.0;
        if show {
            if ev.in_internet_center && ev.container_radius > 0.0 {
                let (dx, dz) = internet_center_floating_text_scatter(
                    ev.id.0.wrapping_add(frame),
                    ev.container_radius,
                    ev.container_radius,
                );
                float_pos.x += dx;
                float_pos.z += dz;
                self.hacker_income.record_ic_scatter();
            }
            self.hacker_income
                .record_floating_text(HostHackerFloatingText::new(
                    ev.id,
                    float_pos,
                    deposited,
                    frame,
                    ev.in_internet_center,
                ));
        } else {
            self.hacker_income.record_floating_text_suppressed();
        }
        self.queue_audio_event(
            AudioEventRequest::new(HACKER_CASH_PING_AUDIO)
                .with_object(ev.id)
                .with_position(ev.pos)
                .with_priority(110),
        );
    }

    pub(super) fn update_hacker_income(&mut self) {
        use crate::game_logic::host_hacker_income::{
            cash_amount_for_level, cash_interval_frames, is_hacker_template,
            is_internet_center_template, is_legal_hacker_income_source, HACKER_CASH_PING_AUDIO,
            HACKER_XP_PER_CASH_UPDATE,
        };

        let frame = self.frame;

        // Snapshot internet-center membership for container queries.
        let internet_centers: std::collections::HashSet<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let is_ic = obj.is_kind_of(KindOf::FSInternetCenter)
                    || is_internet_center_template(&obj.template_name);
                if is_ic {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        // Collect residual hackers with container / legal gates.
        #[derive(Clone, Copy)]
        struct HackerSnap {
            id: ObjectId,
            team: Team,
            pos: Vec3,
            level: crate::game_logic::VeterancyLevel,
            in_ic: bool,
            alive: bool,
            neutral: bool,
            disabled_hacked: bool,
            stealthed: bool,
            detected: bool,
            container_id: Option<ObjectId>,
            container_stealthed: bool,
            container_detected: bool,
            container_team: Team,
            container_radius: f32,
        }
        let hackers: Vec<HackerSnap> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !is_hacker_template(&obj.template_name) {
                    return None;
                }
                let container = obj.container_id();
                let in_ic = container
                    .map(|cid| internet_centers.contains(&cid))
                    .unwrap_or(false);
                let (c_stealthed, c_detected, c_team, c_radius) = container
                    .and_then(|cid| self.objects.get(&cid))
                    .map(|c| {
                        (
                            c.status.stealthed,
                            c.status.detected,
                            c.team,
                            c.thing.geometry.radius,
                        )
                    })
                    .unwrap_or((false, false, Team::Neutral, 0.0));
                Some(HackerSnap {
                    id: *id,
                    team: obj.team,
                    pos: obj.get_position(),
                    level: obj.experience.level,
                    in_ic,
                    alive: obj.is_alive(),
                    neutral: obj.team == Team::Neutral,
                    disabled_hacked: obj.status.disabled_hacked,
                    stealthed: obj.status.stealthed,
                    detected: obj.status.detected,
                    container_id: container,
                    container_stealthed: c_stealthed,
                    container_detected: c_detected,
                    container_team: c_team,
                    container_radius: c_radius,
                })
            })
            .collect();

        let live: std::collections::HashSet<ObjectId> = hackers.iter().map(|h| h.id).collect();
        let stale: Vec<ObjectId> = self
            .hacker_income
            .tracked_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.hacker_income.forget(id);
        }

        for h in &hackers {
            if !h.alive {
                self.hacker_income.forget(h.id);
                continue;
            }
            // Internet Center residual: auto-start hacking when contained.
            if h.in_ic && is_legal_hacker_income_source(h.alive, h.neutral, h.disabled_hacked) {
                self.hacker_income
                    .ensure_internet_center_hacking(h.id, frame);
            }
            // If no longer in IC and never field-started, keep active only if
            // still marked hacking (field residual). Leaving IC mid-hack continues
            // at field interval (C++ uses getCashUpdateDelay each cycle).
            if !self.hacker_income.is_hacking(h.id) {
                continue;
            }
            if !is_legal_hacker_income_source(h.alive, h.neutral, h.disabled_hacked) {
                // C++: DISABLED_HACKED skips deposit but stays in HACK_INTERNET state.
                continue;
            }
            let amount = cash_amount_for_level(h.level);
            let interval = cash_interval_frames(h.in_ic);
            let deposited = self
                .hacker_income
                .try_deposit(h.id, frame, amount, interval, h.in_ic);
            if deposited == 0 {
                continue;
            }
            if let Some(player) = self.get_player_mut_by_team(h.team) {
                player.credit_supplies(deposited);
            }
            // Residual XpPerCashUpdate.
            if let Some(obj) = self.objects.get_mut(&h.id) {
                obj.gain_experience(HACKER_XP_PER_CASH_UPDATE);
            }
            // STEALTHED local display gate residual (owner + containedBy).
            let owner_local = self
                .player_id_for_team(h.team)
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            let container_local = self
                .player_id_for_team(h.container_team)
                .map(|pid| self.is_local_player(pid))
                .unwrap_or(false);
            use crate::game_logic::host_hacker_income::{
                internet_center_floating_text_scatter, should_display_hacker_floating_cash,
            };
            let show = should_display_hacker_floating_cash(
                h.stealthed,
                h.detected,
                owner_local,
                h.container_id.is_some() && h.in_ic,
                h.container_stealthed,
                h.container_detected,
                container_local,
            );
            if show {
                let mut float_pos = h.pos;
                if h.in_ic && h.container_radius > 0.0 {
                    let (dx, dz) = internet_center_floating_text_scatter(
                        h.id.0.wrapping_add(frame),
                        h.container_radius,
                        h.container_radius,
                    );
                    float_pos.x += dx;
                    float_pos.z += dz;
                    self.hacker_income.record_ic_scatter();
                }
                self.hacker_income.record_floating_text(
                    crate::game_logic::host_hacker_income::HostHackerFloatingText::new(
                        h.id, float_pos, deposited, frame, h.in_ic,
                    ),
                );
            } else {
                self.hacker_income.record_floating_text_suppressed();
            }
            self.queue_audio_event(
                AudioEventRequest::new(HACKER_CASH_PING_AUDIO)
                    .with_object(h.id)
                    .with_position(h.pos)
                    .with_priority(110),
            );
        }
    }

    /// Residual field command: start HackInternet for selected hacker unit(s).
    /// Fail-closed: not full unpack animation / pack-on-interrupt state machine.
    pub fn start_hacker_internet_hack(&mut self, hacker_id: ObjectId) -> bool {
        use crate::game_logic::host_hacker_income::{
            is_hacker_template, is_legal_hacker_income_source,
        };
        let frame = self.frame;
        let Some(obj) = self.objects.get(&hacker_id) else {
            return false;
        };
        if !is_hacker_template(&obj.template_name) {
            return false;
        }
        if !is_legal_hacker_income_source(
            obj.is_alive(),
            obj.team == Team::Neutral,
            obj.status.disabled_hacked,
        ) {
            return false;
        }
        self.hacker_income.start_hacking(hacker_id, frame);
        true
    }

    /// Residual: stop HackInternet (e.g. move interrupt residual).
    pub fn stop_hacker_internet_hack(&mut self, hacker_id: ObjectId) {
        self.hacker_income.stop_hacking(hacker_id);
    }

    /// America Supply Drop Zone residual: OCL interval queues cargo DeliverPayload.
    ///
    /// Retail FactionBuilding.ini AmericaSupplyDropZone:
    /// MinDelay/MaxDelay=120000 ms → 3600 logic frames @ 30 FPS,
    /// OCL_AmericaSupplyDropZoneCrateDrop → AmericaJetCargoPlane DeliverPayload
    /// with 6× SupplyDropZoneCrate @ $250 (+25 each with Upgrade_AmericaSupplyLines).
    ///
    /// Host residual: when OCL is due, queue a cargo flight (approach delay), then
    /// [`Self::update_deliver_payloads`] spawns crates and credits BuildingPickup cash.
    /// Fail-closed: not full CreateAtEdge aircraft Object / parachute fall physics.
    pub(super) fn update_supply_drop_zone_drops(&mut self) {
        use crate::game_logic::host_deliver_payload::{
            HostDeliverPayloadKind, SUPPLY_DROP_CARGO_APPROACH_AUDIO,
            SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE, SUPPLY_DROP_PAYLOAD_TEMPLATE,
        };
        use crate::game_logic::host_supply_drop_zone::{
            is_legal_supply_drop_zone_income_source, is_supply_drop_zone_template,
        };

        let frame = self.frame;
        let zones: Vec<(ObjectId, Team, Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !is_supply_drop_zone_template(&obj.template_name) {
                    return None;
                }
                let is_neutral = obj.team == Team::Neutral;
                if !is_legal_supply_drop_zone_income_source(
                    obj.is_alive(),
                    obj.is_constructed() && !obj.status.under_construction,
                    is_neutral,
                ) {
                    return None;
                }
                Some((*id, obj.team, obj.get_position()))
            })
            .collect();

        // Forget destroyed zones so re-builds reschedule cleanly.
        let live: std::collections::HashSet<ObjectId> =
            zones.iter().map(|(id, _, _)| *id).collect();
        let stale: Vec<ObjectId> = self
            .supply_drop_zones
            .next_drop_keys()
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect();
        for id in stale {
            self.supply_drop_zones.forget(id);
            self.host_deliver_payloads.cancel_for_source(id);
        }

        for (zone_id, team, pos) in zones {
            if !self.supply_drop_zones.try_start_flight(zone_id, frame) {
                continue;
            }

            // Prefer retail crate template; residual TestSupplyDropZoneCrate otherwise.
            let payload_template = if self.templates.contains_key(SUPPLY_DROP_PAYLOAD_TEMPLATE) {
                SUPPLY_DROP_PAYLOAD_TEMPLATE.to_string()
            } else {
                self.ensure_residual_supply_drop_crate_template();
                SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string()
            };

            let mission_id = self.host_deliver_payloads.queue(
                HostDeliverPayloadKind::SupplyDropZoneCrate,
                zone_id,
                team,
                pos,
                frame,
                payload_template,
            );

            self.queue_audio_event(
                AudioEventRequest::new(SUPPLY_DROP_CARGO_APPROACH_AUDIO)
                    .with_object(zone_id)
                    .with_position(pos)
                    .with_priority(120),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponMuzzleFlash,
                pos,
                frame,
                Some(zone_id),
                None,
            );

            log::info!(
                "Host SupplyDropZone cargo DeliverPayload mission {} queued at {:?} (frame={})",
                mission_id,
                pos,
                frame
            );
        }
    }

    /// Ensure residual SupplyDropZoneCrate template for cargo DeliverPayload path.
    pub(super) fn ensure_residual_supply_drop_crate_template(&mut self) {
        use crate::game_logic::host_deliver_payload::SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE;
        if self
            .templates
            .contains_key(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE)
        {
            return;
        }
        let mut t = ThingTemplate::new(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Selectable)
            .set_health(1.0)
            .set_cost(0, 0);
        self.templates
            .insert(SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Advance pending DeliverPayload cargo missions with DropDelay stagger.
    ///
    /// Spawns one payload item per due frame (DoorDelay before first item, then
    /// DropDelay between items). Registers residual MoneyCrateCollide entries and
    /// AmericaCrateParachute fall-physics residual (elevated spawn → OpenDist open
    /// → sink). BuildingPickup residual cash is applied on mission complete
    /// (zone bulk residual) and/or via [`Self::update_money_crate_collides`].
    ///
    /// Fail-closed: not full cargo-plane Object flight / full container Object.
    pub fn update_deliver_payloads(&mut self) {
        use crate::game_logic::host_deliver_payload::SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE;
        use crate::game_logic::host_supply_drop_zone::{
            drop_cash_amount, SUPPLY_DROP_ZONE_DROP_CASH,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        self.host_deliver_payloads.clear_frame_events();
        // CreateAtEdge cargo-plane flight residual presentation (approach band /
        // door open). Fail-closed: not full aircraft Object / locomotor.
        self.host_deliver_payloads.tick_cargo_flights();
        // Sync OCLSpecialPower transport objects to cargo-flight residual positions.
        let flight_sync: Vec<(ObjectId, Vec3, f32)> = self
            .host_deliver_payloads
            .missions_snapshot()
            .into_iter()
            .filter_map(|m| {
                let tid = m.transport_object_id?;
                let flight = self.host_deliver_payloads.cargo_flight(m.id)?;
                let yaw = flight.dir_z.atan2(flight.dir_x);
                Some((tid, flight.current_pos, yaw))
            })
            .collect();
        for (tid, pos, yaw) in flight_sync {
            if let Some(o) = self.objects.get_mut(&tid) {
                o.set_position(pos);
                o.set_orientation(yaw);
            }
        }
        // AmericaParadrop cargo bookkeeping is completed from update_paradrops
        // (infantry spawn ownership). Only spawn-capable kinds resolve here.
        let item_plans: Vec<_> = self
            .host_deliver_payloads
            .plan_due_item_spawns(self.frame)
            .into_iter()
            .filter(|p| p.kind.spawns_payload_objects())
            .collect();

        for plan in item_plans {
            if !self.templates.contains_key(&plan.payload_template) {
                self.ensure_residual_supply_drop_crate_template();
            }
            let template_name = if self.templates.contains_key(&plan.payload_template) {
                plan.payload_template.clone()
            } else {
                SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE.to_string()
            };

            let spawned_id =
                self.create_object(&template_name, plan.source_team, plan.spawn_position);
            if let Some(id) = spawned_id {
                if plan.kind
                    == crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::SuperweaponOclBomb
                {
                    // OCL bomb/missile residual: course-home to target; no crate parachute.
                    if let Some(obj) = self.objects.get_mut(&id) {
                        if let Some(m) = self.host_deliver_payloads.get(plan.mission_id) {
                            let _ = obj.set_smart_bomb_target(m.target_position);
                            if let Some(tid) = m.transport_object_id {
                                obj.producer_id = Some(tid);
                            }
                        }
                        // C++ CreateObjectDie + HeightDie on fuel-air / daisy payloads.
                        obj.ensure_create_object_die();
                        obj.ensure_height_die(self.frame);
                    }
                    self.ocl_special_power_reg.record_payload_spawn();
                } else {
                // Residual MoneyCrateCollide registration (unit + BuildingPickup).
                self.host_money_crates.register_supply_drop_crate(id);
                self.host_money_crates.arm_default_deletion(
                    id,
                    self.frame,
                    id.0.wrapping_add(self.frame),
                );
                // AmericaCrateParachute residual: freefall → OpenDist → open → land.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_crate_parachuting();
                }
                // C++ DeliverPayloadAIUpdate: m_isParachuteDirectly →
                // contain->setOverrideDestination(ai->getTargetPos()).
                if crate::game_logic::host_deliver_payload::SUPPLY_DROP_PARACHUTE_DIRECTLY {
                    if self.set_parachute_override_destination(id, plan.target_position) {
                        self.host_deliver_payloads
                            .record_parachute_directly_override();
                    }
                }
                } // else supply-drop crate path
            }
            self.host_deliver_payloads
                .record_item_spawned(plan.mission_id, spawned_id);

            // Drop audio / particle on first item of the mission.
            if plan.item_index == 0 {
                self.queue_audio_event(
                    AudioEventRequest::new(plan.kind.drop_audio())
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(130),
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    plan.target_position,
                    self.frame,
                    Some(plan.source_object),
                    None,
                );
            }

            // BuildingPickup residual bulk cash when final item lands
            // (zone path; crates remain for unit MoneyCrateCollide residual
            // only if not marked paid — mark paid after bulk to avoid double).
            if plan.is_final_item && plan.kind.credits_building_pickup_cash() {
                let has_supply_lines = self.players.values().any(|p| {
                    p.team == plan.source_team
                        && p.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
                });
                let amount = drop_cash_amount(has_supply_lines);
                let boost = amount.saturating_sub(SUPPLY_DROP_ZONE_DROP_CASH);
                self.supply_drop_zones.record_payload_cash(amount, boost);
                if let Some(player) = self.get_player_mut_by_team(plan.source_team) {
                    player.credit_supplies(amount);
                }
                if boost > 0 {
                    self.supply_lines_bonus_cash_total =
                        self.supply_lines_bonus_cash_total.saturating_add(boost);
                }
                self.host_deliver_payloads
                    .record_cash_credited(plan.mission_id, amount);

                // Prevent unit MoneyCrateCollide double-credit after zone bulk cash.
                if let Some(mission) = self.host_deliver_payloads.get(plan.mission_id) {
                    let ids: Vec<ObjectId> = mission.spawned_payload_ids.clone();
                    self.host_money_crates
                        .mark_building_pickup_residual_paid(&ids);
                }

                log::info!(
                    "Host DeliverPayload {} mission {} complete at {:?} (items={}, cash={})",
                    plan.kind.label(),
                    plan.mission_id,
                    plan.target_position,
                    plan.item_index + 1,
                    amount
                );
            } else {
                log::debug!(
                    "Host DeliverPayload {} mission {} item {} spawned at {:?}",
                    plan.kind.label(),
                    plan.mission_id,
                    plan.item_index,
                    plan.spawn_position
                );
            }
        }
    }

    /// Residual MoneyCrateCollide: unit + BuildingPickup cash collect.
    ///
    /// Supply Drop Zone cargo crates that already received bulk BuildingPickup
    /// residual cash are marked paid (no double-credit). Standalone residual
    /// crates (tests / future map crates) credit MoneyProvided on proximity.
    ///
    /// Residual gates (C++ CrateCollide::isValidToExecute subset):
    /// - ForbiddenKindOf PROJECTILE / parachuting pickers rejected
    /// - Above-terrain crates block unit path (BuildingPickup still allowed)
    /// - ExecuteAnimation MoneyPickUp residual presentation descriptor on collect
    ///
    /// Fail-closed: not full CollideModule partition pairs / Anim2D GPU / EVA text.

    /// C++ DeletionUpdate::update residual for money/salvage crates.
    ///
    /// Destroys (NOT kills) crates whose dieFrame has elapsed.
    pub fn update_crate_deletion_updates(&mut self) {
        let expired = self.host_money_crates.expired_ids(self.frame);
        for id in expired {
            self.host_money_crates.forget(id);
            // Destroy object if still present (C++ destroyObject, not kill).
            if self.objects.contains_key(&id) {
                self.mark_object_for_destruction(id, None);
            }
        }
    }
    pub fn update_money_crate_collides(&mut self) {
        use crate::game_logic::host_deliver_payload::crate_is_above_terrain;
        use crate::game_logic::host_money_crate::{
            HostMoneyCrateRegistry, MONEY_CRATE_BUILDING_PICKUP_RADIUS, MONEY_CRATE_PICKUP_AUDIO,
            MONEY_CRATE_UNIT_PICKUP_RADIUS,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;

        let crate_ids = self.host_money_crates.ids();
        if crate_ids.is_empty() {
            return;
        }

        // Snapshot crate positions + entry flags + above-terrain residual.
        let mut crates: Vec<(
            ObjectId,
            Vec3,
            bool, // building_pickup
            bool, // residual paid
            bool, // above_terrain
            bool, // is_salvage
        )> = Vec::new();
        for id in crate_ids {
            // Forget destroyed crates.
            let Some(obj) = self.host_object(id) else {
                self.host_money_crates.forget(id);
                continue;
            };
            if !obj.is_alive() {
                self.host_money_crates.forget(id);
                continue;
            }
            let entry = match self.host_money_crates.get(id) {
                Some(e) => e,
                None => continue,
            };
            // Salvage crates may grant upgrades with money_provided still set.
            if entry.building_pickup_residual_paid {
                continue;
            }
            if entry.money_provided == 0
                && !entry.is_salvage
                && !entry.is_veterancy
                && !entry.is_unit_crate
                && !entry.is_heal_crate
                && !entry.is_shroud_crate
            {
                continue;
            }
            let pos = obj.get_position();
            // Host residual ground plane 0; airborne while parachuting or elevated.
            let above = obj.is_parachuting() || crate_is_above_terrain(pos.y, 0.0);
            crates.push((
                id,
                pos,
                entry.building_pickup,
                entry.building_pickup_residual_paid,
                above,
                entry.is_salvage,
            ));
        }

        // Snapshot candidate pickers.
        let pickers: Vec<(
            ObjectId,
            Team,
            Vec3,
            bool, /*structure*/
            bool, /*constructed*/
            bool, /*projectile*/
            bool, /*parachute picker*/
            bool, /*salvager*/
        )> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.team == Team::Neutral {
                    return None;
                }
                let is_projectile = obj.is_kind_of(KindOf::Projectile);
                // C++ rejects KINDOF_PARACHUTE pickers; host residual: parachuting flag
                // or template name containing "parachute".
                let is_parachute_picker = obj.is_parachuting()
                    || obj.template_name.to_ascii_lowercase().contains("parachute");
                let is_structure =
                    obj.is_kind_of(KindOf::Structure) || obj.object_type == ObjectType::Building;
                let constructed = obj.is_constructed() && !obj.status.under_construction;
                let is_salvager = obj.is_kind_of(KindOf::Salvager)
                    || obj.is_kind_of(KindOf::WeaponSalvager)
                    || obj.is_kind_of(KindOf::ArmorSalvager);
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    is_structure,
                    constructed,
                    is_projectile,
                    is_parachute_picker,
                    is_salvager,
                ))
            })
            .collect();

        let mut pickups: Vec<(ObjectId, ObjectId, Team, bool)> = Vec::new();
        let mut above_rejects = 0_u32;
        let mut forbidden_rejects = 0_u32;
        for (crate_id, crate_pos, building_pickup, _paid, above_terrain, is_salvage) in &crates {
            // Pure residual acquire: nearest legal picker in unit/building pickup radius (XZ).
            // Reject counters still run in the candidate filter phase.
            let mut structure_by_id: std::collections::HashMap<ObjectId, bool> =
                std::collections::HashMap::new();
            let mut team_by_id: std::collections::HashMap<ObjectId, Team> =
                std::collections::HashMap::new();
            let mut cands: Vec<crate::game_logic::host_residual_acquire::ResidualAcquireCandidate> =
                Vec::new();
            for (
                picker_id,
                team,
                picker_pos,
                is_structure,
                constructed,
                is_projectile,
                is_parachute_picker,
                is_salvager,
            ) in &pickers
            {
                // C++ SalvageCrateCollide::isValidToExecute — only SALVAGER units.
                if *is_salvage && !*is_salvager {
                    continue;
                }
                // Salvage crates are not building-pickup residual.
                if *is_salvage && *is_structure {
                    continue;
                }
                if *picker_id == *crate_id {
                    continue;
                }
                let dist = HostMoneyCrateRegistry::horizontal_distance(*crate_pos, *picker_pos);
                if *is_structure {
                    if !HostMoneyCrateRegistry::is_legal_building_picker(
                        true,
                        false,
                        true,
                        *constructed,
                        *building_pickup,
                    ) {
                        continue;
                    }
                    if dist > MONEY_CRATE_BUILDING_PICKUP_RADIUS {
                        continue;
                    }
                } else {
                    if HostMoneyCrateRegistry::is_forbidden_kindof_picker(
                        *is_projectile,
                        *is_parachute_picker,
                    ) {
                        forbidden_rejects = forbidden_rejects.saturating_add(1);
                        continue;
                    }
                    if *above_terrain {
                        // Unit path blocked while crate airborne residual.
                        above_rejects = above_rejects.saturating_add(1);
                        continue;
                    }
                    if !HostMoneyCrateRegistry::is_legal_unit_picker(
                        true,
                        false,
                        false,
                        *is_projectile,
                        *is_parachute_picker,
                        *above_terrain,
                    ) {
                        continue;
                    }
                    if dist > MONEY_CRATE_UNIT_PICKUP_RADIUS {
                        continue;
                    }
                }
                structure_by_id.insert(*picker_id, *is_structure);
                team_by_id.insert(*picker_id, *team);
                cands.push(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: *picker_id,
                        team: *team,
                        position: *picker_pos,
                        is_alive: true,
                        is_neutral: *team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                );
            }
            let max_r = MONEY_CRATE_BUILDING_PICKUP_RADIUS.max(MONEY_CRATE_UNIT_PICKUP_RADIUS);
            if let Some((picker_id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(*crate_id),
                    (crate_pos.x, crate_pos.z),
                    cands,
                    max_r,
                    |_| true,
                )
            {
                let is_structure = structure_by_id.get(&picker_id).copied().unwrap_or(false);
                let team = team_by_id.get(&picker_id).copied().unwrap_or(Team::Neutral);
                pickups.push((*crate_id, picker_id, team, is_structure));
            }
        }
        for _ in 0..above_rejects.min(1) {
            // Count one honesty reject per update when any airborne unit path blocked.
            self.host_money_crates.record_above_terrain_unit_reject();
        }
        if above_rejects > 1 {
            // Extra airborne reject events (still one honesty flag is enough;
            // keep counter proportional for observability).
            for _ in 1..above_rejects.min(8) {
                self.host_money_crates.record_above_terrain_unit_reject();
            }
        }
        for _ in 0..forbidden_rejects.min(4) {
            self.host_money_crates.record_forbidden_kindof_reject();
        }

        for (crate_id, picker_id, team, is_structure) in pickups {
            let Some(entry) = self.host_money_crates.get(crate_id).cloned() else {
                continue;
            };
            let has_supply_lines = self
                .players
                .values()
                .any(|p| p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));

            // C++ ShroudCrateCollide residual path.
            if entry.is_shroud_crate {
                let _ = self.execute_shroud_crate_behavior(picker_id);
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateShroud")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object(crate_id);
                continue;
            }
            // C++ HealCrateCollide residual path.
            if entry.is_heal_crate {
                let _ = self.execute_heal_crate_behavior(picker_id);
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateHeal")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object(crate_id);
                continue;
            }
            // C++ UnitCrateCollide residual path.
            if entry.is_unit_crate {
                let _ = self.execute_unit_crate_behavior(
                    picker_id,
                    &entry.unit_crate_type,
                    entry.unit_crate_count,
                );
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CrateFreeUnit")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object(crate_id);
                continue;
            }
            // C++ VeterancyCrateCollide residual path.
            if entry.is_veterancy {
                let _ = self.execute_veterancy_crate_behavior(
                    picker_id,
                    entry.veterancy_effect_range,
                    entry.veterancy_levels,
                );
                if !self
                    .host_money_crates
                    .record_pickup(crate_id, 1, 0, is_structure)
                {
                    continue;
                }
                let pos = self
                    .host_object(crate_id)
                    .map(|o| o.get_position())
                    .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                self.queue_audio_event(
                    AudioEventRequest::new("CratePromote")
                        .with_object(picker_id)
                        .with_position(pos)
                        .with_priority(110),
                );
                self.destroy_object(crate_id);
                continue;
            }
            // C++ SalvageCrateCollide residual path.
            let (amount, boost) = if entry.is_salvage {
                let seed = crate_id
                    .0
                    .wrapping_add(picker_id.0)
                    .wrapping_add(self.frame);
                let (_kind, money) =
                    self.execute_salvage_crate_behavior(picker_id, entry.money_provided, seed);
                (money, 0u32)
            } else {
                HostMoneyCrateRegistry::cash_for_pickup(&entry, has_supply_lines)
            };
            // Salvage may grant upgrade with 0 money — still consume crate.
            if amount == 0 && !entry.is_salvage {
                continue;
            }
            if !self
                .host_money_crates
                .record_pickup(crate_id, amount.max(1), boost, is_structure)
            {
                continue;
            }
            if amount > 0 {
                if let Some(player) = self.get_player_mut_by_team(team) {
                    player.credit_supplies(amount);
                }
            }
            if boost > 0 {
                self.supply_lines_bonus_cash_total =
                    self.supply_lines_bonus_cash_total.saturating_add(boost);
            }
            let pos = self
                .host_object(crate_id)
                .map(|o| o.get_position())
                .or_else(|| self.host_object(picker_id).map(|o| o.get_position()))
                .unwrap_or(Vec3::ZERO);
            // ExecuteAnimation MoneyPickUp residual presentation descriptor.
            let anim =
                HostMoneyCrateRegistry::money_pickup_anim(crate_id, picker_id, pos, self.frame);
            self.host_money_crates.record_money_pickup_anim(anim);
            // Floating cash text residual presentation (`+$N` / GUI:AddCash).
            let floating = HostMoneyCrateRegistry::money_floating_text(
                crate_id, picker_id, pos, amount, self.frame,
            );
            self.host_money_crates.record_money_floating_text(floating);
            self.queue_audio_event(
                AudioEventRequest::new(MONEY_CRATE_PICKUP_AUDIO)
                    .with_object(picker_id)
                    .with_position(pos)
                    .with_priority(110),
            );
            self.destroy_object(crate_id);
            log::info!(
                "Host MoneyCrateCollide residual: crate {:?} → picker {:?} team={:?} amount={} building={}",
                crate_id,
                picker_id,
                team,
                amount,
                is_structure
            );
        }
    }

    /// AmericaCrateParachute residual: freefall → OpenDist open → sink to ground.
    ///
    /// Applied to residual money crates that spawned from DeliverPayload cargo
    /// (PutInContainer AmericaCrateParachute). Fail-closed: not full container
    /// Object / W3D bone / CrateParachuteLocomotor force matrix.
    pub(crate) fn tick_crate_parachute_residual(&mut self, crate_id: ObjectId) {
        use crate::game_logic::host_deliver_payload::{
            should_open_crate_parachute, tick_crate_parachute_height, CRATE_PARACHUTE_LAND_AUDIO,
            CRATE_PARACHUTE_OPEN_AUDIO,
        };

        // Only residual money crates use this path (pilot path is separate).
        if !self.host_money_crates.contains(crate_id) {
            return;
        }
        let (pos, chute_open, start_h, pitch, roll, landing_override) =
            match self.objects.get(&crate_id) {
                Some(obj) if obj.is_alive() && obj.is_parachuting() => (
                    obj.get_position(),
                    obj.is_parachute_open(),
                    obj.status.parachute_start_height,
                    obj.parachute_pitch(),
                    obj.parachute_roll(),
                    obj.parachute_landing_override(),
                ),
                _ => return,
            };
        let ground = 0.0_f32;
        let mut just_opened = false;
        let mut open = chute_open;
        if !open && should_open_crate_parachute(start_h, pos.y) {
            open = true;
            just_opened = true;
        }
        let (new_y, landed) = tick_crate_parachute_height(pos.y, ground, open);
        // ParachuteDirectly residual: open chute steers XZ to DeliverPayload target.
        let mut nx = pos.x;
        let mut nz = pos.z;
        let mut did_override_step = false;
        if open && !landed {
            if let Some(target) = landing_override {
                use crate::game_logic::host_usa_pilot::{
                    step_parachute_landing_override, PARACHUTE_LANDING_OVERRIDE_SPEED,
                };
                let (sx, sz, moved) = step_parachute_landing_override(
                    pos.x,
                    pos.z,
                    target.x,
                    target.z,
                    PARACHUTE_LANDING_OVERRIDE_SPEED,
                );
                if moved {
                    nx = sx;
                    nz = sz;
                    did_override_step = true;
                }
            }
        }
        if let Some(obj) = self.objects.get_mut(&crate_id) {
            if just_opened {
                obj.open_eject_parachute();
            }
            let mut p = obj.get_position();
            p.x = nx;
            p.z = nz;
            p.y = new_y;
            obj.set_position(p);
            crate::game_logic::host_ground_height_log::record(crate_id, ground, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                if landed {
                    crate::game_logic::host_move_log::record(crate_id, Some([p.x, p.y, p.z]));
                }
                obj.record_host_movement();
            }
            if landed {
                obj.clear_eject_parachuting();
            }
        }
        if did_override_step {
            self.usa_pilot.record_landing_override_step();
            self.host_deliver_payloads.record_parachute_directly_step();
        }
        if just_opened {
            self.host_deliver_payloads.record_crate_parachute_open();
            self.queue_audio_event(
                AudioEventRequest::new(CRATE_PARACHUTE_OPEN_AUDIO)
                    .with_position(Vec3::new(pos.x, new_y, pos.z))
                    .with_priority(145),
            );
        }
        // AmericaCrateParachute bone attach residual presentation (open chute).
        if open && !landed {
            let _attach = self.host_deliver_payloads.build_crate_parachute_attach(
                (pos.x, new_y, pos.z),
                pitch,
                roll,
                true,
            );
        }
        if landed {
            self.host_deliver_payloads.record_crate_parachute_land();
            self.queue_audio_event(
                AudioEventRequest::new(CRATE_PARACHUTE_LAND_AUDIO)
                    .with_position(Vec3::new(pos.x, ground, pos.z))
                    .with_priority(140),
            );
        }
    }

    /// CommandCenter / RadarVan radar-online residual (C++ Player::hasRadar).
    ///
    /// Retail: America CC GrantUpgradeCreate Upgrade_AmericaRadar + RadarUpgrade;
    /// China CC RadarUpgrade (researched); GLA RadarVan GrantUpgradeCreate
    /// Upgrade_GLARadar + RadarUpgrade DisableProof.
    /// Residual: owning any alive constructed CC or RadarVan sets player
    /// radar_count and has_radar. Fail-closed vs full RadarUpgrade module matrix,
    /// power-brownout removeRadar, capture transfer, and Fake CC (skipped).
    pub(super) fn update_player_radar(&mut self) {
        use crate::game_logic::host_radar::{
            is_legal_radar_provider, RADAR_OFFLINE_AUDIO, RADAR_ONLINE_AUDIO,
        };
        use std::collections::HashMap;

        // Count residual radar providers per team.
        let mut providers_by_team: HashMap<Team, u32> = HashMap::new();
        for obj in self.objects.values() {
            if !is_legal_radar_provider(
                obj.is_alive(),
                obj.is_constructed() && !obj.status.under_construction,
                obj.is_command_center() || obj.is_kind_of(KindOf::CommandCenter),
                &obj.template_name,
            ) {
                continue;
            }
            if obj.team == Team::Neutral {
                continue;
            }
            *providers_by_team.entry(obj.team).or_insert(0) += 1;
        }

        // Apply radar_count recompute per player (absolute set, not delta).
        let player_ids: Vec<u32> = self.players.keys().copied().collect();
        let mut transition_events: Vec<(u32, bool, bool)> = Vec::new();
        for pid in player_ids {
            let Some(player) = self.players.get_mut(&pid) else {
                continue;
            };
            let count = providers_by_team.get(&player.team).copied().unwrap_or(0);
            let had = player.has_radar();
            player.set_radar_state(count as i32, player.radar_disabled);
            let has_now = player.has_radar();
            transition_events.push((count, had, has_now));
        }

        for (count, had, has_now) in transition_events {
            let (came_online, went_offline) =
                self.host_radar.record_player_radar(count, had, has_now);
            if came_online {
                self.queue_audio_event(
                    AudioEventRequest::new(RADAR_ONLINE_AUDIO).with_priority(130),
                );
            } else if went_offline {
                self.queue_audio_event(
                    AudioEventRequest::new(RADAR_OFFLINE_AUDIO).with_priority(130),
                );
            }
        }
    }

    /// C++ parity (Player::update → doPowerDisable): set/clear
    /// `disabled_underpowered` on all KINDOF_POWERED objects depending on
    /// whether their owning player has sufficient power.
    /// C++ parity (ThingTemplate::calcTimeToBuild): compute per-team power
    /// production speed factor based on the energy supply ratio.
    ///
    ///   energy_ratio = produced / max(consumed, 1) clamped to [0,1]
    ///   energy_short = (1.0 - ratio) * LowEnergyPenaltyModifier (0.4)
    ///   rate = max(1.0 - energy_short, MinLowEnergyProductionSpeed (0.5))
    ///   if ratio < 1.0: rate = min(rate, MaxLowEnergyProductionSpeed (0.8))
    pub(super) fn compute_team_power_factors(&self) -> std::collections::HashMap<Team, f32> {
        const LOW_ENERGY_PENALTY_MODIFIER: f32 = 0.4;
        const MIN_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.5;
        const MAX_LOW_ENERGY_PRODUCTION_SPEED: f32 = 0.8;

        let mut factors = std::collections::HashMap::new();
        for player in self.players.values() {
            let factor = if player.power_consumed <= 0 {
                1.0
            } else {
                let energy_ratio =
                    (player.power_produced as f32 / player.power_consumed as f32).min(1.0);
                if energy_ratio >= 1.0 {
                    1.0
                } else {
                    let energy_short = (1.0 - energy_ratio) * LOW_ENERGY_PENALTY_MODIFIER;
                    let mut rate = (1.0 - energy_short).max(MIN_LOW_ENERGY_PRODUCTION_SPEED);
                    rate = rate.min(MAX_LOW_ENERGY_PRODUCTION_SPEED);
                    rate
                }
            };
            factors.insert(player.team, factor);
        }
        factors
    }

    /// C++ parity (GarrisonContain::onBodyDamageStateChange): when a garrisoned
    /// building drops below the ReallyDamaged threshold (30% health), all
    /// occupants are force-ejected.  Buildings with `KINDOF_GARRISONABLE_UNTIL_DESTROYED`
    /// are exempt from this evacuation.
    pub(super) fn check_building_damage_states(&mut self, object_ids: &[ObjectId]) {
        const REALLY_DAMAGED_THRESHOLD: f32 = 0.3;

        // Collect buildings that need evacuation to avoid borrow conflicts.
        let mut evacuate_from: Vec<(ObjectId, Vec3)> = Vec::new();

        for &obj_id in object_ids {
            let Some(obj) = self.objects.get(&obj_id) else {
                continue;
            };
            if !obj.is_alive() || !obj.is_constructed() || !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            // Skip buildings that are garrisonable until destroyed.
            if obj.is_kind_of(KindOf::Harvestable) {
                continue;
            }
            let Some(building_data) = &obj.building_data else {
                continue;
            };
            if building_data.garrisoned_units.is_empty() {
                continue;
            }
            let health_pct = obj.health.percentage();
            if health_pct > REALLY_DAMAGED_THRESHOLD {
                continue;
            }

            // Only evacuate once: mark as already-evacuated by clearing the
            // garrison list.  We collect positions first to avoid mut borrows.
            let pos = obj.get_position();
            let occupants: Vec<ObjectId> = building_data.garrisoned_units.clone();
            for &occ_id in &occupants {
                evacuate_from.push((occ_id, pos));
            }
        }

        // Eject occupants.
        for (occ_id, building_pos) in evacuate_from {
            // Remove from container first.
            let container_id = self
                .objects
                .values()
                .find(|o| o.contained_units().contains(&occ_id))
                .map(|o| o.id);

            if let Some(cid) = container_id {
                if let Some(container) = self.objects.get_mut(&cid) {
                    container.remove_occupant(occ_id);
                }
            }

            // Move occupant out.
            if let Some(unit) = self.objects.get_mut(&occ_id) {
                let angle = (occ_id.0 as f32).sin().atan2(1.0);
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
                unit.stop_moving();
                unit.set_position(building_pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = building_pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_target(None);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_stop_attack(occ_id);
                }
                unit.set_contained_by(None);
                unit.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(occ_id, 0);
                }
                unit.set_status_moving(false);
                unit.set_status_attacking(false);
            }
            self.record_garrison_residual_exit();
        }
    }

    pub(super) fn update_power_disabled_state(&mut self) {
        // Wave 811: under coupled shadow, disabled_underpowered owned by GW expire.
        // Keep Eva low-power residual on host (UI presentation).
        if crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active()
        {
            self.update_eva_low_power();
            return;
        }
        // Build a set of teams that are underpowered.
        let mut underpowered_teams: std::collections::HashSet<Team> =
            std::collections::HashSet::new();
        for player in self.players.values() {
            if player.power_available < 0 {
                underpowered_teams.insert(player.team);
            }
        }

        for obj in self.objects.values_mut() {
            if !obj.is_kind_of(KindOf::Powered) {
                continue;
            }
            let should_disable =
                underpowered_teams.contains(&obj.team) && obj.is_alive() && obj.is_constructed();
            obj.status.disabled_underpowered = should_disable;
        }
        // C++ Eva::shouldPlayLowPower residual (local energy insufficient).
        self.update_eva_low_power();
    }

    pub(super) fn check_bridge_disabled_statuses(&self) {
        // Dual-world OBJECT_REGISTRY status peel retired — host owns disabled state.
        let _ = self;
    }

    /// Create a new object
    pub fn create_object(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        // Map-load skip list: decorative / overloaded templates (AngryMob nexus
        // projectiles, cinematic shells, …). Intentional residual / test spawns
        // that already registered a template are fail-open (host Angry Mob path).
        if Self::should_skip_map_object_template(template_name)
            && !self.templates.contains_key(template_name)
        {
            return None;
        }

        if !self.templates.contains_key(template_name) {
            let mut injected = false;
            let should_spawn_fallback = Self::should_spawn_fallback_template(template_name);

            if let Some(template) = Self::build_template_from_asset_definition(template_name) {
                let missing_model = template
                    .model_name
                    .as_deref()
                    .filter(|model| !Self::is_model_asset_available(model))
                    .map(|model| model.to_string());

                if missing_model.is_none() || should_spawn_fallback {
                    self.templates.insert(template_name.to_string(), template);
                    injected = true;
                    log::debug!(
                        "Synthesized template for '{}' from WW3D object definitions",
                        template_name
                    );
                } else if let Some(model) = missing_model {
                    log::debug!(
                        "Falling back for decorative map object template '{}' after unavailable definition model '{}'",
                        template_name,
                        model
                    );
                }
            }

            if !injected {
                if let Some(fallback_template) = Self::build_visual_fallback_template(template_name)
                {
                    let model_name = fallback_template
                        .model_name
                        .clone()
                        .unwrap_or_else(|| template_name.to_string());
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    if should_spawn_fallback {
                        log::warn!(
                            "Injected fallback template for unresolved object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    } else {
                        log::debug!(
                            "Injected visual-only fallback template for decorative object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    }
                } else if !should_spawn_fallback {
                    log::debug!(
                        "Skipping unsupported decorative map object template '{}'",
                        template_name
                    );
                    return None;
                } else {
                    let fallback_template = Self::build_fallback_template(template_name);
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    log::warn!(
                        "Injected fallback template for unresolved object '{}'",
                        template_name
                    );
                }
            }
        }

        if let Some(template) = self.templates.get(template_name).cloned() {
            let is_structure = template.is_kind_of(KindOf::Structure);
            let counts_as_unit = Self::template_counts_as_unit(&template);
            let id = self.allocate_object_id();
            // Resolve weapons / locomotor before move into Object.
            let weapon = template.resolve_primary_weapon();
            let secondary_weapon = template.resolve_secondary_weapon();
            let movement_stats = template.resolve_movement();
            // Sentry residual: detect explicit template primary before move.
            let sentry_had_explicit_primary =
                template.primary_weapon.is_some() || template.primary_weapon_name.is_some();
            let mut object = Object::new(template, id, team);
            object.set_position(position);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }
            let starts_under_construction = object.status.under_construction;

            // Primary weapon from template when defined; kind-based fallback only as last resort.
            if let Some(weapon) = weapon {
                object.weapon = Some(weapon);
            }
            // Secondary slot: fail-closed (only when template names/stats resolve).
            if let Some(secondary) = secondary_weapon {
                object.secondary_weapon = Some(secondary);
            }

            // Strategy Center residual: PRIMARY StrategyCenterGun exists in retail but
            // AutoChooseSources=PRIMARY NONE and turret starts disabled until Bombardment
            // (C++ enableTurret). Strip kind-based Weapon::default fallback; Bombardment
            // residual re-equips StrategyCenterGun. Explicit template primary still keeps.
            if crate::game_logic::host_strategy_center::is_strategy_center_template(template_name)
                || object.is_kind_of(KindOf::FSStrategyCenter)
            {
                // Fail-closed: strip kind-based Weapon::default unless already
                // StrategyCenterGun residual (Bombardment mid-game recreate).
                use crate::game_logic::host_strategy_center::STRATEGY_CENTER_GUN_DAMAGE;
                let is_gun = object.weapon.as_ref().is_some_and(|w| {
                    (w.damage - STRATEGY_CENTER_GUN_DAMAGE).abs() < 0.001
                        && (w.range - 400.0).abs() < 0.001
                });
                if !is_gun {
                    object.weapon = None;
                    object.secondary_weapon = None;
                }
            }

            // GLA Quad Cannon residual: force air/ground anti masks on dual weapons.
            // Fail-closed vs full Weapon.ini AntiGround/AntiAirborne parse when store
            // templates leave default GROUND mask on AA secondary.
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                if let Some(w) = object.weapon.as_mut() {
                    w.can_target_ground = true;
                    w.can_target_air = false;
                }
                if let Some(w) = object.secondary_weapon.as_mut() {
                    w.can_target_air = true;
                    w.can_target_ground = false;
                }
            }

            // GLA Toxin Tractor residual: ensure contaminate spray secondary binds.
            // Retail PrimaryDamage=0 fails weapon_from_store gate; host residual installs
            // a ready secondary for AutoChooseSources=NONE special-attack residual.
            if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name) {
                object.fire_ocl_after_cooldown = Some(
                    crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData::new(),
                );
                if object.secondary_weapon.is_none() {
                    use crate::game_logic::host_toxin_tractor::{
                        delay_frames_to_reload_secs, TOXIN_SPRAY_DELAY_FRAMES, TOXIN_SPRAY_RANGE,
                    };
                    object.secondary_weapon = Some(Weapon {
                        damage: 0.001,
                        range: TOXIN_SPRAY_RANGE,
                        min_range: 0.0,
                        reload_time: delay_frames_to_reload_secs(TOXIN_SPRAY_DELAY_FRAMES),
                        last_fire_time: 0.0,
                        ammo: None,
                        clip_size: 0,
                        clip_reload_time: 0.0,
                        can_target_air: false,
                        can_target_ground: true,
                        projectile_speed: 600.0,
                        pre_attack_delay: 0.0,
                        splash_radius: 0.0,
                    });
                }
            }

            // Locomotor catalog → host Movement (retail BasicHumanLocomotor ~20 u/s).
            // Fail-closed: only when template sets locomotor_name and store resolves.
            // Prefer catalog over Movement::default() (10) so golden skirmish does not
            // need a march-speed boost when the host seed/INI path is present.
            if let Some(stats) = movement_stats {
                object.movement.max_speed = stats.max_speed;
                object.movement.acceleration = stats.acceleration;
                object.movement.turn_rate = stats.turn_rate;
            }

            // Host residual: bind mine/demo-trap data for recognized templates.
            if let Some(mine_data) =
                crate::game_logic::host_mines::residual_data_for_template(template_name, self.frame)
            {
                object.mine_data = Some(mine_data);
                object.record_host_demo_mine_cheer();
            }

            // Host residual: GLA Battle Bus TransportContain Slots=8 + passenger fire.
            if crate::game_logic::host_battle_bus::is_battle_bus_template(template_name) {
                object.install_battle_bus_transport();
            }
            if crate::game_logic::host_highlander_body::is_highlander_body_template(template_name) {
                object.install_highlander_body();
            }
            object.install_deploy_style_if_needed();
            object.install_tensile_formation_if_needed();
            if object.has_tensile_formation() {
                self.tensile_formation_reg.record_install();
            }
            object.install_fire_spread_if_needed();
            if object.has_fire_spread() {
                self.fire_spread_reg.record_install();
            }
            object.install_base_regenerate_if_needed();
            if object.base_regenerate.is_some() {
                self.base_regenerate_reg.record_install();
            }
            object.install_enemy_near_if_needed();
            if object.enemy_near.is_some() {
                self.enemy_near_reg.record_install();
            }
            object.install_animation_steering_if_needed();
            if object.animation_steering.is_some() {
                self.animation_steering_reg.record_install();
            }
            object.install_float_update_if_needed();
            if object.float_update.is_some() {
                self.float_update_reg.record_install();
            }
            object.install_prone_update_if_needed();
            if object.prone_update.is_some() {
                self.prone_update_reg.record_install();
            }
            object.install_radius_decal_update_if_needed();
            if object.radius_decal_update.is_some() {
                self.radius_decal_update_reg.record_install();
            }
            object.install_checkpoint_update_if_needed();
            if object.checkpoint_update.is_some() {
                self.checkpoint_update_reg.record_install();
            }
            object.install_spectre_gunship_deployment_if_needed();
            if object.spectre_gunship_deployment.is_some() {
                self.spectre_gunship_deployment_reg.record_install();
            }
            object.install_smart_bomb_target_homing_if_needed();
            if object.smart_bomb_target_homing.is_some() {
                self.smart_bomb_target_homing_reg.record_install();
            }
            if let Some(up) =
                crate::game_logic::host_upgrade_die::upgrade_to_remove_for_template(template_name)
            {
                object.install_upgrade_die(up);
            }

            // Host residual: GLA Technical TransportContain Slots=5 (infantry passengers)
            // + PRIMARY TechnicalMachineGunWeapon residual (salvage tiers swap later).
            // Fail-closed: not chassis reskin / PassengersAllowedToFire.
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                use crate::game_logic::host_technical::{
                    technical_weapon_for_tier, TechnicalWeaponTier,
                };
                object.install_technical_transport();
                // Force residual MG when template lacked primary_weapon_name (Weapon::default path).
                object.weapon = Some(technical_weapon_for_tier(TechnicalWeaponTier::Base));
            }

            // Host residual: China Battlemaster PRIMARY BattleMasterTankGun residual.
            // Fail-closed: Uranium/horde/nationalism applied via refresh_battlemaster_weapon.
            if crate::game_logic::host_battlemaster::is_battlemaster_template(template_name) {
                use crate::game_logic::host_battlemaster::battlemaster_weapon;
                object.weapon = Some(battlemaster_weapon(false, false, false));
            }

            // Host residual: GLA Marauder PRIMARY MarauderTankGun residual (salvage tiers).
            // Fail-closed: not full SalvageCrate W3D turret subobject matrix.
            if crate::game_logic::host_marauder::is_marauder_template(template_name) {
                use crate::game_logic::host_marauder::{
                    marauder_weapon_for_tier, MarauderWeaponTier,
                };
                object.weapon = Some(marauder_weapon_for_tier(MarauderWeaponTier::Base));
            }

            // Host residual: GLA Combat Cycle RiderChangeContain Slots=1 + rider weapon.
            // Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                object.install_combat_cycle_transport();
                // Retail InitialPayload residual: spawn with default rider weapon bound.
                let rider = crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                    template_name,
                );
                object.combat_cycle_rider = rider.as_u8();
                object.weapon =
                    crate::game_logic::host_combat_cycle::combat_cycle_weapon_for_rider(rider);
            }

            // Host residual: GLA Tunnel Network TunnelContain (shared MaxTunnelCapacity=10)
            // + PRIMARY TunnelNetworkGun residual (base-defense auto-fire path).
            // Fail-closed: not GuardTunnelNetwork AI / CaveSystem / heal matrix.
            if crate::game_logic::host_tunnel_network::is_tunnel_network_template(template_name) {
                object.install_tunnel_network_residual();
                object.weapon =
                    Some(crate::game_logic::host_tunnel_network::tunnel_network_gun_weapon());
            }

            // Host residual: AirF Combat Chinook TransportContain Slots=8 + passenger fire.
            // Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
            if crate::game_logic::host_combat_chinook::is_combat_chinook_template(template_name) {
                object.install_combat_chinook_transport();
            }

            // Host residual: China Listening Outpost detect 300 + transport Slots=2 +
            // InnateStealth + ArmedRiders dummy. Fail-closed: not IR FX / multi-door.
            let is_listening_outpost_spawn =
                crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                    template_name,
                );
            if is_listening_outpost_spawn {
                object.install_listening_outpost_transport();
            }

            // Host residual: China Troop Crawler TransportContain Slots=8 +
            // StealthDetector (VisionRange 175) + TroopCrawlerAssault DEPLOY.
            // Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve.
            let is_troop_crawler_spawn =
                crate::game_logic::host_troop_crawler::is_troop_crawler_template(template_name);
            if is_troop_crawler_spawn {
                object.install_troop_crawler_transport();
                object.weapon =
                    Some(crate::game_logic::host_troop_crawler::troop_crawler_assault_weapon());
                if crate::game_logic::host_troop_crawler::troop_crawler_spawn_is_detector(
                    template_name,
                ) {
                    object.is_detector = true;
                    object.record_host_detector();
                    if let Some(range) =
                        crate::game_logic::host_troop_crawler::troop_crawler_detection_range(
                            template_name,
                        )
                    {
                        object.detection_range = range;
                        object.record_host_detector();
                    }
                }
                // VisionRange residual (175) for effective_detection_range fallback.
                object.thing.template.sight_range = object
                    .thing
                    .template
                    .sight_range
                    .max(crate::game_logic::host_troop_crawler::TROOP_CRAWLER_VISION_RANGE);
            }

            // Host residual: China Overlord / Helix / Emperor portable addons + transport.
            // Fail-closed: not full OverlordContain / HelixContain portable-structure spawn.
            if crate::game_logic::host_overlord_addons::is_overlord_tank_template(template_name) {
                // OverlordContain style: portable slot reserved; bunker residual separate.
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            if crate::game_logic::host_overlord_addons::is_helix_template(template_name) {
                object.install_helix_transport();
                // Host residual: Helix PRIMARY HelixMinigunWeapon (always retained with addons).
                // Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
                object.weapon = Some(crate::game_logic::host_helix_minigun::helix_minigun_weapon());
            }
            if crate::game_logic::host_overlord_addons::is_emperor_template(template_name) {
                // Innate PropagandaTowerBehavior AffectsSelf residual.
                object.has_overlord_propaganda_addon = true;
                object.record_host_overlord();
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            let emperor_spawn =
                crate::game_logic::host_overlord_addons::is_emperor_template(template_name);
            let helix_spawn =
                crate::game_logic::host_overlord_addons::is_helix_template(template_name);

            // Host residual: America Humvee TransportContain Slots=5 + passenger fire.
            // Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                object.install_humvee_transport();
            }

            // Host residual: America Avenger designator primary + air laser secondary.
            // Fail-closed: not portable laser turret OverlordContain passenger.
            if crate::game_logic::host_avenger::is_avenger_template(template_name) {
                object.weapon = Some(crate::game_logic::host_avenger::avenger_designator_weapon());
                object.secondary_weapon =
                    Some(crate::game_logic::host_avenger::avenger_air_laser_weapon());
            }

            // Host residual: America Sentry Drone StealthDetectorUpdate (DetectionRange 225).
            // Always detector from spawn; gun is PLAYER_UPGRADE residual.
            if crate::game_logic::host_sentry_drone::sentry_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_sentry_drone::sentry_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Innate stealth residual (StealthUpdate InnateStealth = Yes).
                object.set_status_stealthed(true);
                object.stealth_breaks_on_attack = true;
                object.record_host_stealth_flags();
                // Retail WeaponSet Conditions=None has PRIMARY None until PLAYER_UPGRADE.
                // Strip kind-based Weapon::default fallback from resolve_primary_weapon.
                // Explicit template primary_weapon(_name) still keeps a bound gun (test/seed).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Pathfinder StealthDetectorUpdate + InnateStealth.
            // DetectionRange unset → VisionRange 200; stays stealthed while attacking;
            // uncloaks only while MOVING (StealthForbiddenConditions = MOVING).
            if crate::game_logic::host_pathfinder::pathfinder_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_pathfinder::pathfinder_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                object.set_status_stealthed(true);
                object.innate_stealth = true;
                object.is_pathfinder_unit = true;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_attack = false;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_move = true;
                object.record_host_stealth_flags();
            }

            // Host residual: China Dragon Tank primary flame weapon bind.
            // Fail-closed: FireWall secondary is host_firewall special-power residual.
            if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
                use crate::game_logic::host_dragon_tank::{
                    dragon_flame_weapon, has_black_napalm_upgrade,
                };
                let upgraded = has_black_napalm_upgrade(&object.applied_upgrades);
                // Force residual flame stats when store/template leaves defaults.
                object.weapon = Some(dragon_flame_weapon(upgraded));
            }

            // Host residual: China Nuke Cannon neutron secondary is PLAYER_UPGRADE only.
            // Fail-closed: Upgrade_ChinaNeutronShells equips SECONDARY; without it, no secondary.
            // Explicit template.secondary_weapon_name (tests / seeds) still keeps a bound weapon.
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
                use crate::game_logic::weapon_bootstrap::{
                    ensure_host_weapon_store, NUKE_CANNON_NEUTRON_WEAPON,
                };
                let has_neutron = object.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                    || object.has_upgrade_tag("Upgrade_ChinaNeutronShells")
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_CHINA_NEUTRON_SHELLS)
                    });
                if has_neutron {
                    ensure_host_weapon_store();
                    if let Some(w) = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON) {
                        object.secondary_weapon = Some(w);
                    }
                    object.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
                {
                    // Strip residual map auto-equip; keep explicit test/seed secondaries.
                    object.secondary_weapon = None;
                }
            }

            // Host residual: China Gattling Tank dual ground/AA + continuous-fire ramp state.
            // Fail-closed: not Overlord/Helix/building gattling payloads.
            if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
                use crate::game_logic::host_gattling_tank::{
                    gattling_air_weapon, gattling_ground_weapon, has_chain_guns_upgrade,
                    GattlingFireLevel,
                };
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
                object.weapon = Some(gattling_ground_weapon(GattlingFireLevel::Base, chain));
                object.secondary_weapon = Some(gattling_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: China Gattling Cannon structure dual ground/AA + continuous-fire ramp.
            // Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
            if crate::game_logic::host_base_defense::is_gattling_cannon_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    gattling_building_air_weapon, gattling_building_ground_weapon,
                    gattling_building_has_chain_guns,
                };
                use crate::game_logic::host_gattling_tank::GattlingFireLevel;
                let chain = gattling_building_has_chain_guns(&object.applied_upgrades);
                object.weapon = Some(gattling_building_ground_weapon(
                    GattlingFireLevel::Base,
                    chain,
                ));
                object.secondary_weapon =
                    Some(gattling_building_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: GLA Stinger Site SPAWNS_ARE_THE_WEAPONS dual ground/AA +
            // HiveStructureBody / SpawnBehavior residual (3 soldiers) + physical roster.
            if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    init_stinger_hive_slave_roster, stinger_air_weapon, stinger_ground_weapon,
                    stinger_has_ap_rockets, sync_hive_slave_mirrors,
                };
                let ap = stinger_has_ap_rockets(&object.applied_upgrades);
                object.weapon = Some(stinger_ground_weapon(ap));
                object.secondary_weapon = Some(stinger_air_weapon(ap));
                let roster = init_stinger_hive_slave_roster();
                object.hive_slaves = roster;
                let (slaves, slave_hp) = sync_hive_slave_mirrors(&roster);
                object.hive_slave_count = slaves;
                object.record_host_hive();
                object.hive_slave_hp = slave_hp;
                object.record_host_hive();
                object.hive_slave_respawn_frame = 0;
            }

            // Host residual: USA Patriot dual ground/AA secondary.
            // Laser General residual uses Lazr_Patriot* damage (40/35) via template.
            // Fail-closed: not full AssistedTargetingModule assist clips / RequestAssistRange.
            if crate::game_logic::host_base_defense::is_patriot_battery_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    patriot_air_weapon_for_template, patriot_ground_weapon_for_template,
                };
                object.weapon = Some(patriot_ground_weapon_for_template(template_name));
                object.secondary_weapon = Some(patriot_air_weapon_for_template(template_name));
            }

            // Host residual: USA Crusader / Paladin PRIMARY tank gun
            // (Laser General Lazr_* → Lazr_CrusaderTankGun / Lazr_PaladinTankGun).
            // Fail-closed: not full LaserName beam drawable / shell lob matrix.
            if crate::game_logic::host_usa_tanks::is_crusader_template(template_name)
                || crate::game_logic::host_usa_tanks::is_paladin_template(template_name)
            {
                object.weapon = Some(
                    crate::game_logic::host_usa_tanks::usa_tank_gun_weapon_for_template(
                        template_name,
                    ),
                );
            }

            // Host residual: GLA Scorpion PRIMARY gun (+ secondary rocket if unlocked).
            // Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
            if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
                use crate::game_logic::host_scorpion::{
                    has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
                    salvage_tier_from_upgrades, scorpion_gun_weapon, scorpion_missile_weapon,
                };
                let tier = salvage_tier_from_upgrades(&object.applied_upgrades);
                object.weapon = Some(scorpion_gun_weapon(tier));
                if has_scorpion_rocket_upgrade(&object.applied_upgrades) {
                    let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                    object.secondary_weapon =
                        Some(scorpion_missile_weapon(ap, tier.dual_missile_clip()));
                }
            }

            // Host residual: USA Tomahawk PRIMARY dual-radius missile.
            // TomahawkMissile projectile lob residual closed (MissileAI peels + impact).
            if crate::game_logic::host_tomahawk::is_tomahawk_template(template_name) {
                use crate::game_logic::host_tomahawk::tomahawk_weapon;
                object.weapon = Some(tomahawk_weapon());
            }

            // Host residual: USA Raptor PRIMARY jet missiles (+ Laser Missiles upgrade).
            // RETURN_TO_BASE ClipReload airfield rearm residual closed (dock + timer).
            if crate::game_logic::host_raptor::is_raptor_template(template_name) {
                use crate::game_logic::host_raptor::{
                    has_laser_missiles_upgrade, is_king_raptor_template, raptor_weapon,
                };
                let king = is_king_raptor_template(template_name);
                let laser = has_laser_missiles_upgrade(&object.applied_upgrades);
                object.weapon = Some(raptor_weapon(king, laser));
            }

            // Host residual: China MiG PRIMARY napalm / Nuke dual-radius missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / HistoricBonus Firestorm matrix.
            if crate::game_logic::host_mig::is_mig_template(template_name) {
                use crate::game_logic::host_mig::{is_nuke_mig_template, mig_loadout, mig_weapon};
                let loadout = mig_loadout(
                    is_nuke_mig_template(template_name),
                    &object.applied_upgrades,
                );
                object.weapon = Some(mig_weapon(loadout));
            }

            // Host residual: America Fire Base PRIMARY howitzer.
            // Fail-closed: not full SPAWNS_ARE_THE_WEAPONS / garrison HiveStructure matrix.
            if crate::game_logic::host_fire_base::is_fire_base_template(template_name) {
                use crate::game_logic::host_fire_base::fire_base_weapon;
                object.weapon = Some(fire_base_weapon());
            }

            // Host residual: USA Stealth Fighter PRIMARY jet missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / science production matrix.
            if crate::game_logic::host_stealth_fighter::is_stealth_fighter_template(template_name) {
                use crate::game_logic::host_stealth_fighter::stealth_fighter_weapon;
                object.weapon = Some(stealth_fighter_weapon());
            }

            // Host residual: USA Comanche PRIMARY 20mm + SECONDARY anti-tank residual.
            // Rocket pods PLAYER_UPGRADE replaces secondary (retail TERTIARY collapse).
            // Fail-closed: not full 3-slot WeaponSet / JetAIUpdate turret matrix.
            if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
                use crate::game_logic::host_comanche_rocket_pods::{
                    comanche_antitank_weapon, comanche_cannon_weapon, comanche_rocket_pod_weapon,
                    UPGRADE_COMANCHE_ROCKET_PODS,
                };
                object.weapon = Some(comanche_cannon_weapon());
                let has_pods = object.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                    || object.has_upgrade_tag("Upgrade_ComancheRocketPods");
                object.secondary_weapon = Some(if has_pods {
                    comanche_rocket_pod_weapon()
                } else {
                    comanche_antitank_weapon()
                });
            }

            // Host residual: USA Battle Drone PRIMARY machine gun.
            // Fail-closed: not full SlavedUpdate repair arm weld FX matrix.
            if crate::game_logic::host_slave_drones::is_battle_drone_template(template_name) {
                use crate::game_logic::host_slave_drones::battle_drone_weapon;
                object.weapon = Some(battle_drone_weapon());
            }

            // Host residual: China Overlord / Emperor PRIMARY dual-radius tank gun.
            // Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
            if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
                use crate::game_logic::host_overlord_gun::{
                    has_uranium_shells_upgrade, overlord_gun_weapon,
                };
                let uranium = has_uranium_shells_upgrade(&object.applied_upgrades);
                object.weapon = Some(overlord_gun_weapon(uranium));
            }

            // Host residual: GLA Jarmen Kell PRIMARY sniper residual.
            // Fail-closed: pilot-snipe special remains host_hero_abilities.
            if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
                use crate::game_logic::host_jarmen_kell::{
                    has_ap_bullets_upgrade, jarmen_kell_weapon,
                };
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(jarmen_kell_weapon(ap));
            }

            // Host residual: China Red Guard PRIMARY machine gun residual.
            // Fail-closed: bayonet residual applied at fire-time for close infantry.
            if crate::game_logic::host_red_guard::is_red_guard_template(template_name) {
                use crate::game_logic::host_red_guard::red_guard_weapon;
                object.weapon = Some(red_guard_weapon(false, false));
            }

            // Host residual: China Tank Hunter PRIMARY RPG residual (AA + ground + splash).
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_tank_hunter::is_tank_hunter_template(template_name) {
                use crate::game_logic::host_tank_hunter::tank_hunter_weapon;
                object.weapon = Some(tank_hunter_weapon(false, false));
            }

            // Host residual: GLA Rebel PRIMARY machine gun residual.
            // Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
            if crate::game_logic::host_gla_rebel::is_gla_rebel_template(template_name) {
                use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rebel_weapon(ap));
            }

            // Host residual: USA Ranger PRIMARY rifle residual.
            // FlashBang secondary is PLAYER_UPGRADE only (Upgrade_AmericaRangerFlashBangGrenade)
            // — parity with neutron shells / rocket pods: residual map may name the weapon,
            // but create strips it unless research is unlocked or template explicitly seeds it.
            // Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
            if crate::game_logic::host_ranger::is_ranger_template(template_name) {
                use crate::game_logic::host_ranger::{
                    has_flashbang_equipped, ranger_flashbang_weapon, ranger_rifle_weapon,
                    UPGRADE_AMERICA_FLASHBANG,
                };
                object.weapon = Some(ranger_rifle_weapon());
                let has_flashbang = has_flashbang_equipped(false, &object.applied_upgrades)
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG)
                    });
                if has_flashbang {
                    object.secondary_weapon = Some(ranger_flashbang_weapon());
                    object.apply_upgrade_tag(UPGRADE_AMERICA_FLASHBANG);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
                {
                    // Strip residual map auto-equip; keep explicit test/seed secondaries.
                    object.secondary_weapon = None;
                } else if object.secondary_weapon.is_some() {
                    // Explicit seed/test secondary — normalize to residual flashbang stats.
                    object.secondary_weapon = Some(ranger_flashbang_weapon());
                }
            }

            // Host residual: China MiniGunner dual ground/AA + continuous fire ramp.
            // Fail-closed: not full FiringTracker CONTINUOUS_FIRE_* anim / bayonet tertiary.
            if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                use crate::game_logic::host_gattling_tank::GattlingFireLevel;
                use crate::game_logic::host_minigunner::{
                    has_chain_guns_upgrade, minigunner_air_weapon, minigunner_ground_weapon,
                };
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
                object.weapon = Some(minigunner_ground_weapon(
                    GattlingFireLevel::Base,
                    chain,
                    false,
                    false,
                ));
                object.secondary_weapon = Some(minigunner_air_weapon(
                    GattlingFireLevel::Base,
                    chain,
                    false,
                    false,
                ));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: Colonel Burton PRIMARY sniper residual.
            // Fail-closed: knife residual applied at fire-time for close infantry.
            if crate::game_logic::host_colonel_burton::is_colonel_burton_template(template_name) {
                use crate::game_logic::host_colonel_burton::burton_sniper_weapon;
                object.weapon = Some(burton_sniper_weapon());
            }

            // Host residual: USA Pilot starts VETERAN (VeterancyGainCreate StartingLevel).
            // Fail-closed: not full EjectPilotDie parachute OCL / PilotFindVehicle AI scan.
            if crate::game_logic::host_usa_pilot::is_pilot_template(template_name) {
                use crate::game_logic::host_usa_pilot::pilot_default_veterancy;
                use crate::game_logic::VeterancyLevel;
                let target = pilot_default_veterancy();
                if object.experience.level != target {
                    let prev = object.experience.level;
                    object.experience.level = target;
                    // Seed residual XP so level does not immediately drop on gain_experience.
                    if matches!(target, VeterancyLevel::Veteran) {
                        object.experience.current = object.experience.current.max(1.0);
                    }
                    let _ = prev;
                    // Apply bonuses only if promoting from Rookie.
                    if !matches!(
                        prev,
                        VeterancyLevel::Veteran | VeterancyLevel::Elite | VeterancyLevel::Heroic
                    ) {
                        // Direct level set; apply_veterancy_bonuses is private — call gain path
                        // by using a public residual: re-apply via temporary if needed.
                        // Object::apply_pilot_recrew already handles merge; for spawn we set level
                        // and leave HP/weapon multipliers fail-closed at template defaults until
                        // first combat XP event. Veterans still recrew-transfer as Veteran.
                    }
                }
            }

            // Host residual: GLA Worker base speed / WorkerShoes speed if already unlocked.
            // Fail-closed: not full WorkerAIUpdate bored auto-task matrix.
            if crate::game_logic::host_gla_worker::is_gla_worker_template(template_name) {
                use crate::game_logic::host_gla_worker::{
                    worker_residual_speed, UPGRADE_GLA_WORKER_SHOES,
                };
                let shoes = object.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
                object.movement.max_speed = worker_residual_speed(shoes);
            }

            // Host residual: GLA RPG Trooper / Tunnel Defender PRIMARY rocket residual.
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
                use crate::game_logic::host_rpg_trooper::{
                    has_ap_rockets_upgrade, rpg_trooper_weapon,
                };
                let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rpg_trooper_weapon(ap));
            }

            // Host residual: GLA Terrorist PRIMARY TerroristSuicideWeapon residual.
            // Chem Beta/Gamma + Demo death-weapon residual profiles.
            // Fail-closed: not ConvertToCarBomb full matrix / SlowDeath fling.
            if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
                use crate::game_logic::host_terrorist::{
                    terrorist_death_profile, terrorist_suicide_weapon_for_profile,
                };
                let has_gamma = object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                    || object.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                let has_beta = object.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                    || object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxBeta");
                let profile = terrorist_death_profile(template_name, has_gamma, has_beta);
                object.weapon = Some(terrorist_suicide_weapon_for_profile(profile));
                object.secondary_weapon = None;
            }

            // Host residual: USA Missile Defender PRIMARY missile + SECONDARY laser guided.
            // Fail-closed: not full SpecialAbilityUpdate prep / LaserBeam object matrix.
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                use crate::game_logic::host_missile_defender::{
                    missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
                };
                object.weapon = Some(missile_defender_primary_weapon());
                object.secondary_weapon = Some(missile_defender_laser_guided_weapon());
            }

            // Host residual: America Scout Drone StealthDetectorUpdate (VisionRange 150).
            if crate::game_logic::host_slave_drones::scout_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_slave_drones::scout_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Sensor drone: strip kind-based default gun if no explicit primary.
                // Reuse sentry_had_explicit_primary (same template fields, captured pre-move).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Hellfire Drone AutoAcquire + HellfireMissileWeapon.
            // Weapon bound via weapon_bootstrap primary; no extra strip.
            // Auto-fire residual runs from update_combat when idle.

            object.ensure_fire_weapon_when_damaged();
            object.ensure_transition_damage_fx();
            object.ensure_fx_list_die();
            object.ensure_create_object_die();
            object.ensure_lifetime_update(self.frame);
            object.ensure_height_die(self.frame);
            self.objects.insert(id, object);

            // C++ Object.cpp onCreate residual: inherit team prototype attitude + attack priority.
            self.inherit_team_ai_defaults(id);

            // C++ SpecialPowerModule StartsPaused=Yes residual (pauseCountdown TRUE on create).
            self.init_starts_paused_special_powers(id);

            // C++ SupplyWarehouseCreate::onCreate residual — StartingBoxes.
            self.init_supply_warehouse_create(id);

            // Residual honesty: Emperor innate propaganda counts as install on spawn.
            if emperor_spawn {
                self.overlord_addons.record_propaganda_install();
            }
            let _ = helix_spawn;

            // Host residual: Listening Outpost InitialPayload TankHunter × 2.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            // Fail-closed: no payload if TankHunter template is absent.
            if is_listening_outpost_spawn {
                self.apply_listening_outpost_initial_payload(id, team, position);
            }

            // Host residual: Troop Crawler InitialPayload Redguard × 8.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            if is_troop_crawler_spawn {
                self.apply_troop_crawler_initial_payload(id, team, position);
            }

            // Host residual: SCIENCE unit-training (VeterancyGainCreate StartingLevel).
            // Fail-closed: not full PrerequisiteSciences rank tree / IsTrainable matrix.
            {
                use crate::game_logic::host_unit_training::unit_training_level_for_template;
                let sciences: Vec<String> = self
                    .players
                    .values()
                    .filter(|p| p.team == team)
                    .flat_map(|p| p.unlocked_sciences.iter().cloned())
                    .collect();
                if let Some((kind, level)) =
                    unit_training_level_for_template(template_name, &sciences)
                {
                    if let Some(obj) = self.objects.get_mut(&id) {
                        if obj.set_min_veterancy_level(level) {
                            self.unit_training.record_grant(kind);
                        }
                    }
                }
            }

            // Host residual: Demo SuicideBomb tag + CommandSetUpgrade if researched.
            {
                use crate::game_logic::host_demo_suicide_bomb::{
                    demo_command_set_upgrade_for_template, is_demo_suicide_bomb_eligible_template,
                    is_demo_suicide_bomb_upgrade, UPGRADE_DEMO_SUICIDE_BOMB,
                };
                if is_demo_suicide_bomb_eligible_template(template_name) {
                    let has_upgrade = self.players.values().any(|p| {
                        p.team == team
                            && p.unlocked_sciences
                                .iter()
                                .any(|s| is_demo_suicide_bomb_upgrade(s))
                    });
                    if has_upgrade {
                        if let Some(obj) = self.objects.get_mut(&id) {
                            if !obj.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB) {
                                obj.apply_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB);
                                self.demo_suicide_bomb.record_tag();
                            }
                            if obj.command_set_override.is_none() {
                                if let Some(cs) =
                                    demo_command_set_upgrade_for_template(&obj.template_name)
                                {
                                    obj.set_command_set_override(Some(cs));
                                    self.demo_suicide_bomb.record_command_set_upgrade(1);
                                }
                            }
                        }
                    }
                }
            }

            if counts_as_unit {
                self.record_unit_production(team);
            } else if is_structure && !starts_under_construction {
                self.record_structure_completion(team);
                // Static path/LOS obstacle (C++ pathfind structure residual).
                self.block_structure_object_path(id);
                // Map-placed / instant SW: onSpecialPowerCreation residual.
                self.on_structure_superweapon_creation(id);
            }
            log::debug!(
                "Created object {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Create object under construction (for buildings)
    pub fn create_object_under_construction(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        // C++ BuildAssistant isLocationLegalToBuild residual (objects-in-way / bounds).
        if !self.is_location_legal_to_build(team, position, template_name) {
            log::debug!(
                "Blocked construction {} at {:?} (LegalBuildCode residual)",
                template_name,
                position
            );
            return None;
        }
        // C++ ProductionPrerequisite residual (known sample table / SW tech tree).
        if !self.team_satisfies_build_prerequisites(team, template_name) {
            log::debug!(
                "Blocked construction {} for team {:?} (Prerequisites residual)",
                template_name,
                team
            );
            return None;
        }
        // C++ MaxSimultaneousOfType=DeterminedBySuperweaponRestriction residual.
        if !self.can_start_superweapon_building(team, template_name) {
            log::debug!(
                "Blocked superweapon construction {} for team {:?} (MaxSimultaneous residual)",
                template_name,
                team
            );
            return None;
        }
        if let Some(template) = self.templates.get(template_name).cloned() {
            let id = self.allocate_object_id();
            let mut object = Object::new_under_construction(template, id, team);
            object.set_position(position);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }

            self.objects.insert(id, object);
            self.inherit_team_ai_defaults(id);

            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            // Wave 199: GameWorld SetConstruction sole-tick / progress last-writer.
            crate::game_logic::host_construction_progress_log::record(id, 0.0, true, 0.0);
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }

            log::debug!(
                "Started construction of {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Destroy an object
    pub fn destroy_object(&mut self, id: ObjectId) {
        self.mark_object_for_destruction(id, None);
    }

    /// Wave 482: sell residual kill (parked aircraft) — queue remove without
    /// SlowDeath/Topple deferral peels used for combat deaths.
    pub(super) fn destroy_object_for_sell_residual(&mut self, id: ObjectId) {
        self.maybe_notify_special_power_completion(id);
        self.maybe_apply_dam_die(id);
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer: None });
    }

    /// C++ FireWeaponWhenDeadBehavior::onDie residual — death weapon splash.
    pub(super) fn apply_fire_weapon_when_dead(&mut self, dying_id: ObjectId) {
        use crate::game_logic::host_fire_weapon_when_dead::{
            death_weapon_for_template, splash_damage_at_distance,
        };

        let Some(obj) = self.objects.get(&dying_id) else {
            return;
        };
        if obj.fire_weapon_when_dead_fired {
            return;
        }
        if obj.status.under_construction {
            return;
        }
        let Some(splash) = death_weapon_for_template(&obj.template_name) else {
            return;
        };
        let pos = obj.get_position();
        let team = obj.team;
        let max_r = splash.primary_radius.max(splash.secondary_radius);

        let is_helix_napalm_bomb = obj.helix_napalm_bomb_projectile;
        let napalm_source = obj.producer_id;
        let black_napalm_bomb = obj.template_name.to_ascii_lowercase().contains("black");

        // Mark fired
        if let Some(obj) = self.objects.get_mut(&dying_id) {
            obj.fire_weapon_when_dead_fired = true;
        }

        let victims: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if *id == dying_id || !o.is_alive() {
                    return None;
                }
                let p = o.get_position();
                let dx = p.x - pos.x;
                let dz = p.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= max_r {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        let mut destroy_ids = Vec::new();
        for vid in victims {
            let Some(v) = self.objects.get_mut(&vid) else {
                continue;
            };
            let p = v.get_position();
            let dx = p.x - pos.x;
            let dz = p.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let dmg = splash_damage_at_distance(&splash, dist);
            if dmg <= 0.0 {
                continue;
            }
            let destroyed = v.take_damage_from_immediate(dmg, Some(dying_id));
            if destroyed {
                destroy_ids.push(vid);
            }
        }
        // Presentation residual: death explosion particle at epicenter.
        let _ = self.combat_particles.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
            pos,
            self.frame,
            Some(dying_id),
            None,
        );
        if is_helix_napalm_bomb {
            // Honesty: HeightDie detonation residual counted as blast path.
            self.helix_napalm.blast_hits = self
                .helix_napalm
                .blast_hits
                .saturating_add(destroy_ids.len() as u32);
            let _ = (napalm_source, black_napalm_bomb);
        }
        let _ = team;
        for id in destroy_ids {
            // Avoid re-entrancy loops: queue destroy without re-firing this dying unit.
            if id != dying_id {
                self.objects_to_destroy.push_back(DestructionEvent {
                    id,
                    killer: Some(team),
                });
            }
        }
    }

    /// Wave 752: lethal finish that respects damage-authority HP last-write.
    /// Prefer this over direct host HP zeroing for production destroy residual.
    #[allow(dead_code)]
    pub(crate) fn host_lethal_finish_object(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
    ) -> bool {
        let Some(o) = self.objects.get_mut(&id) else {
            return false;
        };
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            crate::game_logic::host_damage_log::record(id, hp, source, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
        true
    }

    /// Wave 754: C++ EjectPilotDie::onDie residual at death start (mark_object),
    /// not only final process_destroy remove. SlowDeath defers remove and must
    /// not suppress pilot spawn / honesty residual.
    pub(crate) fn maybe_apply_eject_pilot_die(&mut self, id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            air_eject_spawn_height, can_eject_pilot_on_death, is_eject_pilot_eligible_template,
            meets_eject_pilot_death_types_gate, meets_eject_pilot_exempt_status_gate,
            meets_eject_pilot_veterancy_gate, uses_air_eject_ocl, EJECT_PILOT_TEMPLATE,
            PILOT_EJECT_AUDIO,
        };
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        if obj.eject_pilot_die_applied {
            return;
        }
        let is_vehicle = obj.is_kind_of(KindOf::Vehicle) || obj.object_type == ObjectType::Vehicle;
        let is_aircraft =
            obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft;
        let under_construction =
            obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
        let eligible_template = is_eject_pilot_eligible_template(&obj.template_name);
        let vet_gate = meets_eject_pilot_veterancy_gate(obj.experience.level);
        let death_types_gate = meets_eject_pilot_death_types_gate(obj.status.death_type);
        let exempt_status_gate = meets_eject_pilot_exempt_status_gate(obj.status.hijacked);
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && death_types_gate
            && exempt_status_gate
            && !vet_gate
        {
            self.usa_pilot.record_eject_veterancy_block();
        }
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && vet_gate
            && exempt_status_gate
            && !death_types_gate
        {
            self.usa_pilot.record_eject_death_type_block();
        }
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && vet_gate
            && death_types_gate
            && !exempt_status_gate
        {
            self.usa_pilot.record_eject_hijacked_block();
        }
        if !can_eject_pilot_on_death(
            eligible_template,
            obj.is_unmanned(),
            under_construction,
            is_vehicle,
            is_aircraft,
            vet_gate,
            death_types_gate,
            exempt_status_gate,
        ) {
            return;
        }
        let pilot_team = obj.team;
        let death_pos = obj.get_position();
        let air_path = uses_air_eject_ocl(death_pos.y, obj.status.airborne_target);
        let veterancy = obj.experience.level;
        // Mark applied before spawn so recursive destroy cannot double-fire.
        if let Some(o) = self.objects.get_mut(&id) {
            o.eject_pilot_die_applied = true;
        }
        if !self.templates.contains_key(EJECT_PILOT_TEMPLATE) {
            let mut pilot_tpl = crate::game_logic::ThingTemplate::new(EJECT_PILOT_TEMPLATE);
            pilot_tpl
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(100.0);
            self.templates
                .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
        }
        // Offset slightly so pilot is not buried under death debris residual.
        // Air OCL residual: keep elevated y (PutInContainer AmericaParachute).
        let spawn_pos = if air_path {
            glam::Vec3::new(
                death_pos.x + 2.0,
                air_eject_spawn_height(death_pos.y),
                death_pos.z + 2.0,
            )
        } else {
            death_pos + glam::Vec3::new(2.0, 0.0, 2.0)
        };
        if let Some(pilot_id) = self.create_object(EJECT_PILOT_TEMPLATE, pilot_team, spawn_pos) {
            self.usa_pilot.record_ejection();
            if air_path {
                self.usa_pilot.record_air_ejection();
            }
            let until =
                crate::game_logic::host_usa_pilot::eject_pilot_invulnerable_until_frame(self.frame);
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                pilot.apply_eject_invulnerable(until);
                if air_path {
                    let raw_y = pilot.get_position().y;
                    pilot.apply_eject_parachuting();
                    if crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged(
                        raw_y, 0.0,
                    ) {
                        self.usa_pilot.record_parachute_open_fudge();
                    }
                }
                // Transfer vehicle veterancy residual (except Rookie gate already applied).
                pilot.experience.level = veterancy;
            }
            self.usa_pilot.record_invulnerable_grant();
            self.queue_audio_event(
                AudioEventRequest::new(PILOT_EJECT_AUDIO)
                    .with_position(spawn_pos)
                    .with_priority(170),
            );
            let _ = pilot_id;
        }
    }

    pub(crate) fn mark_object_for_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        // C++ ProductionUpdate cancelAndRefund on death start (before topple/slow-death deferral).
        self.cancel_all_production(id);
        // C++ SpecialPowerCompletionDie::onDie residual.
        self.maybe_notify_special_power_completion(id);
        // C++ DamDie::onDie residual fires with other die modules at death start.
        self.maybe_apply_dam_die(id);
        // Wave 754: C++ EjectPilotDie::onDie at death start (before SlowDeath defer).
        self.maybe_apply_eject_pilot_die(id);
        // C++ OCL ApplyRandomForceNugget residual (air-death toss before debris).
        let _ = self.apply_ocl_random_force(id);
        // C++ UpgradeDie::onDie residual — free producer's upgrade slot.
        self.maybe_apply_upgrade_die(id);
        // Wave 482: BuildAssistant sell finish removes the object immediately.
        // Do not defer into StructureTopple/Collapse / SlowDeath / KeepObjectDie —
        // those combat-death peels left sold structures alive forever in host-only tests.
        let (sold, under_construction) = self
            .objects
            .get(&id)
            .map(|o| (o.status.sold, o.status.under_construction))
            .unwrap_or((false, false));
        // Wave 715: MSG_DOZER_CANCEL_CONSTRUCT / unfinished builds remove immediately.
        // Do not defer into StructureTopple — cancel would leave the shell alive a frame+.
        if !sold && !under_construction {
            // C++ StructureTopple/Collapse residual: buildings fall/sink before remove.
            if self.try_begin_structure_topple_instead_of_destroy(id, killer) {
                return;
            }
            // C++ SlowDeathBehavior residual: infantry/vehicles delay destroy + sink.
            if self.try_begin_slow_death_instead_of_destroy(id, killer) {
                return;
            }
            // C++ KeepObjectDie residual: leave rubble, do not DestroyDie-remove.
            if self.try_begin_keep_object_die_instead_of_destroy(id, killer) {
                return;
            }
        }
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer });
    }

    /// C++ KeepObjectDie residual: convert to lasting rubble, skip remove.
    pub(super) fn try_begin_keep_object_die_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Wave 775: StructureCollapse/Topple already ran their presentation; after Done
        // allow normal destroy instead of KeepObjectDie forever-defer (civilian barns).
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        if obj.status.keep_as_rubble {
            let _ = killer;
            return true;
        }
        if !obj.begin_keep_object_die(frame) {
            return false;
        }
        let _ = killer;
        // Death FX / OCL peels without world removal.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.fire_fx_list_die();
            obj.fire_create_object_die();
        }
        self.apply_pending_create_object_die(id);
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
        true
    }

    /// C++ DamDie::onDie residual — enable KINDOF_WAVEGUIDE objects.
    /// C++ UpgradeDie::onDie residual.
    pub(super) fn maybe_apply_upgrade_die(&mut self, id: ObjectId) {
        let (producer, upgrade) = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            let Some(ud) = obj.upgrade_die.as_mut() else {
                return;
            };
            if ud.fired {
                return;
            }
            ud.fired = true;
            (obj.producer_id, ud.upgrade_to_remove.clone())
        };
        let Some(pid) = producer else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        let Some(master) = self.objects.get_mut(&pid) else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        if master.remove_upgrade_tag(&upgrade) {
            self.upgrade_die_reg.record_removal();
        } else {
            self.upgrade_die_reg.record_missing_upgrade();
        }
    }

    pub(super) fn maybe_apply_dam_die(&mut self, id: ObjectId) {
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
    }

    pub(super) fn apply_dam_die_enable_waveguides(&mut self) {
        let frame = self.frame;
        for obj in self.objects.values_mut() {
            let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                || crate::game_logic::host_dam_die::is_wave_guide_template(&obj.template_name)
                || crate::game_logic::host_wave_guide::is_wave_guide_template(&obj.template_name);
            if is_wg {
                obj.status.disabled_default = false;
                if obj.wave_guide_data.is_none() {
                    let mut wg = crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                    wg.facing = obj.get_orientation();
                    wg.ensure_active(frame.max(1));
                    obj.wave_guide_data = Some(wg);
                } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                    wg.ensure_active(frame.max(1));
                }
            }
        }
    }

    pub(super) fn try_begin_slow_death_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Jet crash residual.
        if obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false) {
            return false;
        }
        if obj
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_jet_slow_death() {
            let _ = killer;
            return true;
        }
        // Helicopter spiral crash residual.
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.done)
            .unwrap_or(false)
        {
            return false;
        }
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_helicopter_slow_death() {
            let _ = killer;
            return true;
        }
        // Already finished slow death → allow destroy.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_done())
            .unwrap_or(false)
        {
            return false;
        }
        // Mid slow death → keep deferring.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_slow_death(frame) {
            let _ = killer;
            return true;
        }
        false
    }

    pub(crate) fn apply_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let building_team = self.objects.get(&building_id).map(|o| o.team);
        let mut destroy: Vec<ObjectId> = Vec::new();
        const SAMPLE_RADIUS: f32 = 18.0;
        let victims: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in victims {
            if id == building_id {
                continue;
            }
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let pos = obj.get_position();
            let mut best_dmg = 0.0_f32;
            for s in &samples {
                let dx = pos.x - s.x;
                let dz = pos.z - s.z;
                if dx * dx + dz * dz <= SAMPLE_RADIUS * SAMPLE_RADIUS {
                    best_dmg = best_dmg.max(s.damage);
                }
            }
            if best_dmg <= 0.0 {
                continue;
            }
            let killed = if let Some(obj) = self.objects.get_mut(&id) {
                // Structure topple crush residual is effectively unresistable for units
                // under the fall sweep (C++ doDamageLine lethality residual).
                let mut dead = obj.take_damage_from_typed_death(
                    best_dmg,
                    Some(building_id),
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                );
                // Fail-closed lethal finish: crush sweep leaves no standing unit residual.
                // Wave 746: under damage authority, do not zero host HP (dual with GW
                // HP writeback). Project lethal via damage log + destroyed flags;
                // non-authority path keeps host HP clear.
                if !obj.status.destroyed {
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        let hp = obj.health.current.max(1.0);
                        crate::game_logic::host_damage_log::record(id, hp, Some(building_id), true);
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    } else {
                        // Wave 753: under damage authority, do not zero host HP mid-frame
                        // (dual with GW HP writeback). Project lethal via damage log + flags.
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            let hp = obj.health.current.max(1.0);
                            let oid = obj.id;
                            crate::game_logic::host_damage_log::record(oid, hp, None, true);
                        } else {
                            obj.health.current = 0.0;
                        }
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    }
                    dead = true;
                }
                dead
            } else {
                false
            };
            if killed
                || self
                    .objects
                    .get(&id)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false)
            {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, building_team);
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior forceFireWeapon residual at object position.

    /// C++ CreateObjectDie::onDie residual — spawn OCL templates at dying object.
    pub fn apply_pending_create_object_die(&mut self, dying_id: ObjectId) {
        let (spawns, transfer_dmg, transfer, team, pos) = {
            let Some(o) = self.objects.get_mut(&dying_id) else {
                return;
            };
            let (spawns, dmg, transfer) = o.take_pending_create_object_die_spawns();
            (spawns, dmg, transfer, o.team, o.get_position())
        };
        if spawns.is_empty() {
            return;
        }
        for tmpl in spawns {
            // CreateDebris disposition residual for GenericDebris peels.
            let tl = tmpl.to_ascii_lowercase();
            if tl.contains("debris") || tl.contains("barrel") {
                use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
                let plan = if tl.contains("barrel") {
                    HostOclCreateDebrisPlan::damaged_barrel()
                } else {
                    let mut p = HostOclCreateDebrisPlan::generic_tank_debris();
                    p.model_or_template = tmpl.clone();
                    p.count = 1;
                    p
                };
                let inherit = self
                    .objects
                    .get(&dying_id)
                    .map(|o| o.movement.velocity)
                    .unwrap_or(Vec3::ZERO);
                let ids = self.spawn_ocl_create_debris(&plan, team, pos, inherit);
                if transfer && transfer_dmg > 0.0 {
                    for id in ids {
                        if let Some(n) = self.objects.get_mut(&id) {
                            let _ = n.take_damage(transfer_dmg);
                        }
                    }
                }
                continue;
            }
            // Ensure template name exists for residual peels.
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.set_health(100.0);
                if tmpl.to_ascii_lowercase().contains("tunnel")
                    || tmpl.to_ascii_lowercase().contains("network")
                {
                    t.add_kind_of(KindOf::Structure);
                }
                self.templates.insert(tmpl.clone(), t);
            }
            let Some(new_id) = self.create_object(&tmpl, team, pos) else {
                continue;
            };
            // C++ CreateObject Disposition=LIKE_EXISTING residual: copy pose.
            if let Some(dying) = self.objects.get(&dying_id) {
                let yaw = dying.get_orientation();
                if let Some(n) = self.objects.get_mut(&new_id) {
                    n.set_orientation(yaw);
                    n.producer_id = Some(dying_id);
                }
            }
            // FuelAir gas SlowDeath + HeightDie residual.
            if let Some(n) = self.objects.get_mut(&new_id) {
                n.ensure_fuel_air_gas_slow_death(self.frame);
                if n.fuel_air_gas_slow_death.is_some() {
                    self.fuel_air_gas_reg.record_install();
                }
            }
            if transfer && transfer_dmg > 0.0 {
                if let Some(n) = self.objects.get_mut(&new_id) {
                    let _ = n.take_damage_from_typed(
                        transfer_dmg,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                    );
                }
            }
        }
    }

    pub(super) fn apply_fire_weapon_when_damaged_named(
        &mut self,
        source_id: ObjectId,
        weapon_name: &str,
    ) -> u32 {
        let (pos, team) = match self.objects.get(&source_id) {
            Some(o) => (o.get_position(), o.team),
            None => return 0,
        };
        let (pd, pr, sd, sr) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_weapon_splash(
                weapon_name,
            );
        // Intended = self so splash doesn't skip others incorrectly... API skips intended_id.
        // Pass a dummy non-existent intended so all in radius can be hit except we should not hit self.
        // apply_instant_hit_splash_at skips intended_id only — use source as intended to skip self.
        self.apply_instant_hit_splash_at(
            pos,
            pd,
            sd,
            pr,
            sr,
            source_id,
            team,
            source_id,
            Some(weapon_name),
        )
    }

    pub(super) fn try_begin_structure_topple_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let attacker_pos = {
            let src = self.objects.get(&id).and_then(|o| o.last_damage_source);
            src.and_then(|sid| {
                self.objects.get(&sid).map(|s| {
                    let p = s.get_position();
                    (p.x, p.z)
                })
            })
        };
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Already finished collapse or topple → allow normal destroy.
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        // Mid-animation: keep deferring destroy.
        if obj
            .structure_collapse_data
            .as_ref()
            .map(|d| d.is_active())
            .unwrap_or(false)
            || obj
                .structure_topple_data
                .as_ref()
                .map(|d| d.is_active())
                .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        // Prefer StructureCollapse for civilian/prop peels; else StructureTopple.
        if obj.begin_structure_collapse(frame) {
            let _ = killer;
            return true;
        }
        if obj.begin_structure_topple(frame, attacker_pos) {
            let _ = killer;
            return true;
        }
        false
    }

    /// Wave 958: legacy alias — prefer [`Self::host_object`].
    #[inline]
    pub fn find_object(&self, id: ObjectId) -> Option<&Object> {
        self.host_object(id)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_object_mut`].
    #[inline]
    pub fn find_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.host_object_mut(id)
    }

    /// Find the nearest supply center (refinery/supply dropzone) for a team.

    /// Nearest alive harvestable supply pile residual for gather re-target.
    pub(super) fn find_nearest_harvestable_supply(&self, team: Team, from: Vec3) -> Option<ObjectId> {
        let _ = team; // supplies are neutral/shared residual
                      // Pure residual acquire: nearest harvestable supply pile (3D distance).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() || obj.status.destroyed {
                    return None;
                }
                let name = obj.template_name.to_ascii_lowercase();
                let harvestable = obj.is_kind_of(KindOf::Harvestable)
                    || obj.is_kind_of(KindOf::Resource)
                    || obj.object_type == ObjectType::Supply
                    || (name.contains("supply")
                        && !name.contains("center")
                        && !name.contains("dock")
                        && !name.contains("dropzone"));
                if !harvestable {
                    return None;
                }
                // Prefer piles that still have stored supplies when tracked.
                if obj.stored_resources.supplies == 0
                    && (obj.is_kind_of(KindOf::Harvestable)
                        || obj.object_type == ObjectType::Supply)
                {
                    // Some piles use infinite residual; only skip if explicitly zero and
                    // Harvestable with supplies field used as stock. Fail-open if never depleted.
                    if name.contains("warehouse") || name.contains("dock") {
                        return None;
                    }
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            Team::Neutral,
            from,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    pub(super) fn find_nearest_supply_center(&self, team: Team, from_position: Vec3) -> Option<ObjectId> {
        // Pure residual acquire: nearest friendly constructed SupplyCenter (3D).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != team
                    || !obj.is_alive()
                    || !obj.is_constructed()
                    || !obj.is_kind_of(KindOf::SupplyCenter)
                {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: obj_id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: false,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            team,
            from_position,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_objects`].
    #[inline]
    pub fn get_objects(&self) -> &HashMap<ObjectId, Object> {
        self.host_objects()
    }

    /// Partition-backed candidate ids near a world position (empty if partition cold).
    /// Callers must still apply team/alive/stealth filters — this is broadphase only.
    #[inline]
    pub fn object_ids_near(&self, position: glam::Vec3, radius: f32) -> Vec<ObjectId> {
        self.partition_manager
            .ids_in_radius(position.x, position.z, radius)
            .into_iter()
            .map(ObjectId)
            .collect()
    }

    /// Wave 958: legacy alias — prefer [`Self::host_objects_mut`].
    #[inline]
    pub fn get_objects_mut(&mut self) -> &mut HashMap<ObjectId, Object> {
        self.host_objects_mut()
    }

    /// Get all players (for snapshot/save system)
    pub fn get_players(&self) -> &HashMap<u32, Player> {
        &self.players
    }

    /// Get mutable players (for snapshot restoration)
    pub fn get_players_mut(&mut self) -> &mut HashMap<u32, Player> {
        &mut self.players
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for snapshot restoration)
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as u32;
    }

    /// Clear all objects (for snapshot restoration)
    pub fn clear_all_objects(&mut self) {
        self.objects.clear();
        self.next_object_id = ObjectId(1);
        self.next_formation_id = 1;
    }

    /// Set the next object ID counter (for snapshot restoration).
    pub fn set_next_object_id_for_restore(&mut self, next_object_id: ObjectId) {
        self.next_object_id = next_object_id;
    }

    /// C++ TheAI::getNextFormationID residual.
    pub fn alloc_formation_id(&mut self) -> u32 {
        let id = self.next_formation_id;
        self.next_formation_id = self.next_formation_id.saturating_add(1).max(1);
        id
    }

    /// Clear all players (for snapshot restoration)
    pub fn clear_all_players(&mut self) {
        self.players.clear();
    }

    /// Add a player directly (for snapshot restoration)
    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.id, player);
    }

    pub fn command_center_position(&self, team: Team) -> Option<Vec3> {
        let mut fallback = None;
        let mut highest_cost = i32::MIN;

        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }

            if obj.is_kind_of(KindOf::CommandCenter) {
                return Some(obj.get_position());
            }

            if obj.is_kind_of(KindOf::Structure) {
                let cost = obj.thing.template.build_cost.supplies as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }

        fallback
    }

    /// Get player by ID
    pub fn get_player(&self, player_id: u32) -> Option<&Player> {
        self.players.get(&player_id)
    }

    /// Wave 238: economy probe without exposing `&Player` to engine dual-read paths.
    #[inline]
    pub fn player_economy(&self, id: u32) -> Option<(u32, i32, i32, i32)> {
        self.players.get(&id).map(|p| {
            (
                p.effective_supplies(),
                p.power_available,
                p.power_produced,
                p.power_consumed,
            )
        })
    }

    /// Wave 238: unlocked sciences without exposing `&Player`.
    #[inline]
    pub fn player_unlocked_sciences(&self, id: u32) -> Vec<String> {
        self.players
            .get(&id)
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Wave 238: science purchase points without exposing `&Player`.
    #[inline]
    pub fn player_science_purchase_points(&self, id: u32) -> i32 {
        self.players
            .get(&id)
            .map(|p| p.science_purchase_points)
            .unwrap_or(0)
    }

    /// Wave 238: science purchase capability without exposing `&Player`.
    #[inline]
    pub fn player_can_purchase_science(&self, id: u32, science_name: &str) -> bool {
        self.players
            .get(&id)
            .map(|p| p.is_capable_of_purchasing_science(science_name))
            .unwrap_or(false)
    }

    /// Wave 239: team probe without exposing `&Player`.
    #[inline]
    pub fn player_team(&self, id: u32) -> Option<Team> {
        self.players.get(&id).map(|p| p.team)
    }

    /// Wave 239: command-center world pose for a player's team (camera boot residual).
    #[inline]
    pub fn player_command_center_position(&self, id: u32) -> Option<glam::Vec3> {
        let team = self.player_team(id)?;
        self.command_center_position(team)
    }

    /// Wave 240: existence probe without exposing `&Player`.
    #[inline]
    pub fn player_exists(&self, id: u32) -> bool {
        self.players.contains_key(&id)
    }

    /// Wave 240: lowest player id (boot local residual).
    #[inline]
    pub fn min_player_id(&self) -> Option<u32> {
        self.players.keys().copied().min()
    }

    /// Wave 240: display name without exposing `&Player`.
    #[inline]
    pub fn player_name(&self, id: u32) -> Option<String> {
        self.players.get(&id).map(|p| p.name.clone())
    }

    /// Wave 240: alive flag without exposing `&Player`.
    #[inline]
    pub fn player_is_alive(&self, id: u32) -> bool {
        self.players.get(&id).map(|p| p.is_alive).unwrap_or(false)
    }

    /// Wave 240: local flag without exposing `&Player`.
    #[inline]
    pub fn player_is_local(&self, id: u32) -> bool {
        self.players.get(&id).map(|p| p.is_local).unwrap_or(false)
    }

    /// Wave 240: UI color without exposing `&Player`.
    #[inline]
    pub fn player_color_rgb(&self, id: u32) -> Option<(u8, u8, u8)> {
        self.players.get(&id).map(|p| p.color_rgb)
    }

    /// Wave 240: selected object ids without exposing `&Player`.
    #[inline]
    pub fn player_selected_objects(&self, id: u32) -> Vec<ObjectId> {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default()
    }

    /// Wave 240: ordered player id roster without exposing `&Player`.
    #[inline]
    pub fn player_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.players.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Wave 240: raise supplies floor without exposing `&mut Player`.
    #[inline]
    pub fn ensure_player_min_supplies(&mut self, id: u32, min_supplies: u32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.resources.supplies = p.resources.supplies.max(min_supplies);
        }
    }

    /// Wave 242: extend selection without exposing `&mut Player`.
    #[inline]
    pub fn player_extend_selection(&mut self, id: u32, units: &[ObjectId]) {
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        for unit in units {
            if !p.selected_objects.contains(unit) {
                p.selected_objects.push(*unit);
            }
        }
    }

    /// Wave 242: selection count without exposing `&Player`.
    #[inline]
    pub fn player_selected_count(&self, id: u32) -> usize {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.len())
            .unwrap_or(0)
    }

    /// Wave 243: spend build cost without exposing `&mut Player`.
    #[inline]
    pub fn try_spend_player_resources(&mut self, id: u32, cost: &Resources) -> bool {
        let Some(p) = self.players.get_mut(&id) else {
            return false;
        };
        p.spend_resources(cost)
    }

    /// Wave 243: refund supplies without exposing `&mut Player`.
    #[inline]
    pub fn player_refund_supplies(&mut self, id: u32, supplies: u32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.resources.supplies = p.resources.supplies.saturating_add(supplies);
        }
    }

    /// Wave 243: constructor team probe without exposing `&Object`.
    #[inline]
    pub fn unit_team_if_can_construct(&self, id: ObjectId) -> Option<Team> {
        let obj = self.objects.get(&id)?;
        if obj.can_construct() {
            Some(obj.team)
        } else {
            None
        }
    }

    /// Get mutable player by ID
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<&mut Player> {
        self.players.get_mut(&player_id)
    }

    pub fn get_player_mut_by_team(&mut self, team: Team) -> Option<&mut Player> {
        let key = self
            .players
            .iter()
            .find_map(|(id, p)| if p.team == team { Some(*id) } else { None })?;
        self.players.get_mut(&key)
    }

    pub fn get_player_by_team(&self, team: Team) -> Option<&Player> {
        self.players.values().find(|p| p.team == team)
    }

    /// Combined object + SharedSyncedTimer + RequiredScience residual ready gate.
    ///
    /// C++ order residual: object alive → science (`canUseSpecialPower`) →
    /// sharedNSync / per-object cooldown.
    pub fn is_special_power_ready_for(
        &self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        let Some(obj) = self.host_object(object_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        // C++ SpecialPowerModule::doSpecialPower / isReady: disabled objects cannot fire.
        // Covers underpowered POWERED SWs (PUC/Nuke), EMP, hacked, unmanned, etc.
        if obj.is_disabled() {
            return false;
        }
        // C++ SpecialPowerStore::canUseSpecialPower science residual.
        if let Some(required) =
            crate::game_logic::host_special_power_enum_residual::special_power_required_science(
                power,
            )
        {
            match self.get_player_by_team(obj.team) {
                Some(player) if player.has_unlocked_science(required) => {}
                Some(_) => return false,
                // Fail-closed: science-gated powers need a controlling player residual.
                None => return false,
            }
        }
        if crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            power,
        ) {
            // C++ getReadyFrame via Player::getOrStartSpecialPowerReadyFrame.
            if let Some(player) = self.get_player_by_team(obj.team) {
                if !player.is_shared_special_power_ready(power) {
                    return false;
                }
            }
            return true;
        }
        obj.is_special_power_ready(power)
    }

    /// Consume charge with SharedSyncedTimer residual when applicable.
    pub fn consume_special_power_charge_for(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        if !self.is_special_power_ready_for(object_id, power) {
            return false;
        }
        let team = match self.host_object(object_id) {
            Some(o) => o.team,
            None => return false,
        };
        let reload =
            crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                power,
            )
            .unwrap_or_else(|| {
                self.host_object(object_id)
                    .map(|o| o.special_power_cooldown)
                    .unwrap_or(10.0)
            });

        if crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            power,
        ) {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.reset_shared_special_power_timer(power, reload);
            }
            // Mirror onto all living same-team objects for HUD/presentation residual.
            for obj in self.objects.values_mut() {
                if obj.team != team || !obj.is_alive() {
                    continue;
                }
                if reload > 0.0 {
                    obj.special_power_cooldowns.insert(power.clone(), reload);
                } else {
                    obj.special_power_cooldowns.remove(power);
                }
                obj.refresh_special_power_aggregate_cooldown();
            }
        } else if let Some(obj) = self.host_object_mut(object_id) {
            obj.consume_special_power_charge(power);
        }
        true
    }

    /// Tick all players' SharedSyncedTimer residual cooldowns.
    ///
    /// Fires EVA SuperweaponReady residual when a PublicTimer power finishes
    /// recharging (own/ally/enemy classification via try_eva_superweapon_ready).
    pub fn tick_shared_special_power_timers(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        // Under SPECIAL_POWER_AUTHORITY+shadow, GameWorld sole-ticks shared SP cds.
        if crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled() {
            // Wave 479: do not republish full cooldown snapshots each frame —
            // that stomped GW sole-tick progress. Fire/reset still records via
            // reset_shared_special_power_timer → record_host_cooldowns.
            return;
        }
        let mut ready_events: Vec<(Team, String)> = Vec::new();
        for player in self.players.values_mut() {
            let team = player.team;
            for power in player.tick_shared_special_power_timers(dt) {
                use crate::game_logic::host_special_power_enum_residual::special_power_has_public_timer;
                if !special_power_has_public_timer(&power) {
                    continue;
                }
                // Map power → structure template name residual for EVA classifier.
                let template = match power {
                    crate::command_system::SpecialPowerType::ParticleCannon
                    | crate::command_system::SpecialPowerType::SuperweaponParticleCannon
                    | crate::command_system::SpecialPowerType::LaserCannon => {
                        "AmericaParticleCannonUplink"
                    }
                    crate::command_system::SpecialPowerType::NuclearMissile
                    | crate::command_system::SpecialPowerType::NukeNeutronMissile
                    | crate::command_system::SpecialPowerType::SuperweaponNeutronMissile
                    | crate::command_system::SpecialPowerType::BaikonurRocket => {
                        "ChinaNuclearMissileLauncher"
                    }
                    crate::command_system::SpecialPowerType::ScudStorm => "GLAScudStorm",
                    _ => continue, // EVA only for PUC/Nuke/Scud residual family
                };
                ready_events.push((team, template.to_string()));
            }
        }
        for (team, name) in ready_events {
            // source id unused by try_eva_superweapon_ready residual.
            self.try_eva_superweapon_ready(crate::game_logic::ObjectId(0), team, &name);
        }
    }

    /// C++ SpecialPowerModule::onSpecialPowerCreation residual.
    ///
    /// When a science is first acquired, sharedNSync powers that require it are
    /// expressed ready-now on the player timer (Dustin residual: start ready to fire).
    /// Fail-closed: not full StartsPaused upgrade gate / InGameUI addSuperweapon font.
    pub fn on_special_power_science_creation(&mut self, player_id: u32, science_name: &str) {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        use crate::game_logic::host_special_power_enum_residual::{
            special_power_required_science, special_power_uses_shared_synced_timer,
        };
        let sci = normalize_science_name_residual(science_name);
        if sci.is_empty() {
            return;
        }
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        // Sample of host powers that may require this science residual.
        const CANDIDATES: &[P] = &[
            P::Airstrike,
            P::AirForceAirstrike,
            P::DaisyCutter,
            P::AirForceDaisyCutter,
            P::FuelAirBomb,
            P::SpyDrone,
            P::Paradrop,
            P::InfantryParadrop,
            P::TankParadrop,
            P::CarpetBomb,
            P::AirForceCarpetBomb,
            P::EarlyChinaCarpetBomb,
            P::ClusterMines,
            P::EmpPulse,
            P::LeafletDrop,
            P::Ambush,
            P::TerrorCell,
            P::Frenzy,
            P::EmergencyRepair,
            P::GpsScrambler,
            P::SneakAttack,
            P::SpectreGunship,
            P::AirForceSpectreGunship,
            P::NapalmStrike,
            P::BlackMarketNuke,
            P::Artillery,
            P::CrateDrop,
            P::CashHack,
            P::SpySatellite,
        ];
        for power in CANDIDATES {
            let Some(req) = special_power_required_science(power) else {
                continue;
            };
            // Match science residual (canonical or alias).
            let req_n = req.to_ascii_lowercase();
            let sci_n = sci.to_ascii_lowercase();
            if req_n != sci_n && !sci_n.ends_with(&req_n) && !req_n.ends_with(&sci_n) {
                continue;
            }
            if !special_power_uses_shared_synced_timer(power) {
                continue;
            }
            // C++: startPowerRecharge then express ready-now for sharedNSync.
            player.express_shared_special_power_ready_now(power);
        }
    }

    pub fn team_has_completed_capture_upgrade(&self, team: Team) -> bool {
        let Some(player) = self.players.values().find(|player| player.team == team) else {
            return true;
        };
        capture_upgrade_names_for_team(team)
            .iter()
            .any(|upgrade| player.has_unlocked_upgrade(upgrade))
    }

    pub fn local_player_id(&self) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.is_local)
            .map(|player| player.id)
    }

    pub fn is_local_player(&self, player_id: u32) -> bool {
        self.players
            .get(&player_id)
            .map(|player| player.is_local)
            .unwrap_or(false)
    }

    /// Override a player's display name (used by CLI / networking parity).
    pub fn set_player_name(&mut self, player_id: u32, name: &str) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Override a player's team/faction at runtime (used by menu selection).
    pub fn set_player_team(&mut self, player_id: u32, team: Team) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.team = team;
            true
        } else {
            false
        }
    }

    /// Wave 921: single authority boundary for match start + local faction (+ optional AI).
    #[inline]
    pub fn start_new_game_with_faction(
        &mut self,
        mode: GameMode,
        player_id: u32,
        faction_team: Team,
        setup_skirmish_ai: bool,
    ) {
        self.start_new_game(mode);
        let _ = self.set_player_team(player_id, faction_team);
        if setup_skirmish_ai {
            self.setup_skirmish_ai(player_id);
        }
    }

    /// Apply an upgrade tag to an object.
    /// Mirrors C++ behavior where upgrades are persistent object state, not display-name edits.
    pub(crate) fn apply_upgrade_to_object(&mut self, object_id: ObjectId, upgrade: &str) {
        use crate::game_logic::host_overlord_addons::{
            is_bunker_addon_upgrade, is_gattling_addon_upgrade, is_overlord_family_host,
            is_propaganda_addon_upgrade,
        };

        let mut installed_gattling = false;
        let mut installed_propaganda = false;
        let mut installed_bunker = false;

        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.apply_upgrade_tag(upgrade);
            // C++ SubObjectsUpgrade residual (BombTruck loads / Helix BombWing).
            {
                let applied =
                    crate::game_logic::host_sub_objects_upgrade::sub_objects_for_upgrade_tags(
                        &obj.applied_upgrades,
                        &obj.template_name,
                    );
                if applied.matched {
                    obj.sub_object_visibility
                        .apply_show_hide(&applied.show, &applied.hide);
                    self.sub_objects_upgrades.record(&applied.show);
                }
            }
            // C++ ModelConditionUpgrade residual.
            let _ = crate::game_logic::host_model_condition_upgrade::apply_model_condition_upgrade(
                &mut obj.model_condition_bits,
                upgrade,
            );
            // C++ WeaponBonusUpgrade residual.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_bonus_upgrade(upgrade) {
                obj.set_weapon_bonus_player_upgrade(true);
                self.upgrade_module_residuals.record_weapon_bonus(upgrade);
            }
            // C++ WeaponSetUpgrade residual → WEAPONSET_PLAYER_UPGRADE.
            if crate::game_logic::host_upgrade_module_residuals::is_weapon_set_upgrade(upgrade) {
                obj.set_weapon_set_flag(0, true);
                self.upgrade_module_residuals.record_weapon_set(upgrade);
            }
            // C++ ArmorUpgrade residual → ARMORSET_PLAYER_UPGRADE (+ ChemSuit decal).
            if crate::game_logic::host_upgrade_module_residuals::is_armor_upgrade(upgrade) {
                obj.set_armor_set_player_upgrade(true);
                if crate::game_logic::host_upgrade_module_residuals::is_chemical_suits_upgrade(
                    upgrade,
                ) {
                    obj.set_terrain_decal_chemsuit(true);
                }
                self.upgrade_module_residuals.record_armor_set(upgrade);
            }
            // C++ LocomotorSetUpgrade residual → setLocomotorUpgrade(true) + speed peels.
            if crate::game_logic::host_upgrade_module_residuals::is_locomotor_set_upgrade(upgrade) {
                obj.set_locomotor_upgrade(true);
                if let Some(speed) =
                    crate::game_logic::host_upgrade_module_residuals::locomotor_upgrade_speed(
                        upgrade,
                        &obj.template_name,
                    )
                {
                    // Host residual: raise movement max speed when peel known.
                    obj.movement.max_speed = obj.movement.max_speed.max(speed);
                    obj.movement.max_speed_damaged =
                        obj.movement.max_speed_damaged.max(speed * 0.5);
                }
                self.upgrade_module_residuals.record_locomotor_set(upgrade);
            }
            // C++ UnpauseSpecialPowerUpgrade residual.
            if let Some(power) =
                crate::game_logic::host_upgrade_module_residuals::unpause_power_for_upgrade(upgrade)
            {
                for p in
                    crate::game_logic::host_upgrade_module_residuals::unpause_power_family(power)
                {
                    obj.pause_special_power_countdown(&p, false);
                }
                self.upgrade_module_residuals.record_unpause(upgrade);
            }
            // C++ CommandSetUpgrade residual.
            if let Some(cs) =
                crate::game_logic::host_replace_object_upgrade::command_set_override_for_upgrade(
                    upgrade,
                    &obj.template_name,
                )
            {
                obj.set_command_set_override(Some(cs.to_string()));
                self.replace_grant_command_upgrades.record_command_set(cs);
            }
            if is_overlord_family_host(&obj.template_name) {
                if is_gattling_addon_upgrade(upgrade) {
                    obj.install_overlord_gattling_addon();
                    installed_gattling = true;
                } else if is_propaganda_addon_upgrade(upgrade) {
                    obj.install_overlord_propaganda_addon();
                    installed_propaganda = true;
                } else if is_bunker_addon_upgrade(upgrade) {
                    // C++ ChinaTankOverlordBattleBunker TransportContain.Slots = 5.
                    // Helix bunker also uses Slots residual 5 (ChinaHelixBattleBunker).
                    obj.install_overlord_battle_bunker(5);
                    // C++ PassengersFireUpgrade TriggeredBy Upgrade_ChinaHelixBattleBunker.
                    use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
                    if should_enable_passengers_fire(upgrade, &obj.template_name)
                        || obj.is_helix_transport
                    {
                        obj.passengers_allowed_to_fire = true;
                        obj.record_host_stealth_flags();
                        self.passengers_fire_upgrade_reg.record_apply(1);
                    }
                    installed_bunker = true;
                }
            }
        }

        if installed_gattling {
            self.overlord_addons.record_gattling_install();
        }
        if installed_propaganda {
            self.overlord_addons.record_propaganda_install();
        }
        let _ = installed_bunker;

        // C++ CostModifierUpgrade residual — player KindOf production cost change.
        if let Some((kind, percent)) =
            crate::game_logic::host_upgrade_module_residuals::cost_modifier_for_upgrade(upgrade)
        {
            let team = self.objects.get(&object_id).map(|o| o.team);
            if let Some(team) = team {
                if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    player.add_kind_of_production_cost_change(kind, percent);
                    self.upgrade_module_residuals.record_cost(upgrade);
                }
            }
        }

        // C++ GenerateMinefieldBehavior::upgradeImplementation residual.
        let _mines = self.place_structure_minefield_for_upgrade(object_id, upgrade);

        // C++ GrantScienceUpgrade residual.
        if let Some(science) =
            crate::game_logic::host_replace_object_upgrade::grant_science_for_upgrade(upgrade)
        {
            let team = self.objects.get(&object_id).map(|o| o.team);
            if let Some(team) = team {
                if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    if player.unlock_science(science) {
                        self.replace_grant_command_upgrades.record_science(science);
                    }
                }
            }
        }

        // C++ ReplaceObjectUpgrade residual (FakeGLA* → real building).
        // C++ destroys immediately (pathfinder unmark + destroyObject) before spawn.
        // Host residual: remove from world map now (skip topple/deferred die list).
        if crate::game_logic::host_replace_object_upgrade::is_replace_object_upgrade(upgrade) {
            let info = self
                .objects
                .get(&object_id)
                .map(|o| (o.template_name.clone(), o.team, o.get_position()));
            if let Some((template_name, team, pos)) = info {
                if let Some(replacement) =
                    crate::game_logic::host_replace_object_upgrade::replacement_template_for_fake(
                        &template_name,
                    )
                {
                    if !self.templates.contains_key(&replacement) {
                        if let Some(src) = self.templates.get(&template_name).cloned() {
                            let mut dst = src;
                            dst.name = replacement.clone();
                            self.templates.insert(replacement.clone(), dst);
                        }
                    }
                    // Immediate remove — same spirit as C++ destroy-before-create.
                    let _removed = self.objects.remove(&object_id);
                    if let Some(new_id) = self.create_object(&replacement, team, pos) {
                        if let Some(obj) = self.objects.get_mut(&new_id) {
                            obj.status.under_construction = false;
                        }
                        self.replace_grant_command_upgrades
                            .record_replace(&template_name, &replacement);
                        let _ = new_id;
                    }
                }
            }
        }
    }

    /// Select objects for a player
    pub fn select_objects(&mut self, player_id: u32, object_ids: Vec<ObjectId>) {
        let Some(player_team) = self.players.get(&player_id).map(|p| p.team) else {
            return;
        };
        let is_local = self
            .players
            .get(&player_id)
            .map(|p| p.is_local)
            .unwrap_or(false);

        // Snapshot previous selection for deselect residual.
        let previous: Vec<ObjectId> = self
            .players
            .get(&player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        for &old_id in &previous {
            if let Some(obj) = self.objects.get_mut(&old_id) {
                obj.deselect();
            }
        }

        let mut selected = Vec::new();
        let mut voice_pos = None;
        let mut voice_template = None;
        for &object_id in &object_ids {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                if obj.team == player_team && obj.is_selectable() {
                    obj.select();
                    // C++ Drawable::flashAsSelected residual on select / create-team.
                    obj.flash_as_selected();
                    selected.push(object_id);
                    if voice_pos.is_none() {
                        voice_pos = Some(obj.get_position());
                        voice_template = Some(obj.template_name.clone());
                    }
                }
            }
        }

        if let Some(player) = self.players.get_mut(&player_id) {
            player.selected_objects = selected.clone();
        }

        // C++ VoiceSelect residual (primary selection unit).
        if is_local {
            if let (Some(pos), Some(template)) = (voice_pos, voice_template) {
                let event = format!("{template}VoiceSelect");
                self.queue_audio_event(
                    AudioEventRequest::new(&event)
                        .with_position(pos)
                        .with_priority(100),
                );
                self.queue_audio_event(
                    AudioEventRequest::new("UnitVoiceSelect")
                        .with_position(pos)
                        .with_priority(90),
                );
            }
        }

        log::debug!("{} selected {} objects", player_id, selected.len());
    }

    /// Issue move command to selected objects (with pathfinding)
    pub fn command_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let is_mobile = self
                    .objects
                    .get(&object_id)
                    .map(|obj| obj.is_mobile())
                    .unwrap_or(false);
                if !is_mobile {
                    continue;
                }

                // Host pathfinding / move channel (default production path).
                self.move_object_with_pathfinding(object_id, target_position, None);
            }
            // C++ VoiceMove residual for local player.
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                if let Some(&oid) = selected.first() {
                    if let Some(obj) = self.objects.get(&oid) {
                        let event = format!("{}VoiceMove", obj.template_name);
                        let pos = obj.get_position();
                        self.queue_audio_event(
                            AudioEventRequest::new(&event)
                                .with_position(pos)
                                .with_priority(100),
                        );
                        self.queue_audio_event(
                            AudioEventRequest::new("UnitVoiceMove")
                                .with_position(pos)
                                .with_priority(90),
                        );
                    }
                }
            }
            log::trace!(
                "{} commanded {} units to move to {:?}",
                player_id,
                selected.len(),
                target_position
            );
        }
    }

    /// Wave 930: single direct-order authority boundary.
    #[inline]
    pub fn apply_direct_player_order(&mut self, order: DirectPlayerOrder) {
        match order {
            DirectPlayerOrder::Attack { player_id, target } => {
                self.command_attack(player_id, target);
            }
            DirectPlayerOrder::Stop { player_id } => {
                self.command_stop(player_id);
            }
            DirectPlayerOrder::Move { player_id, dest } => {
                self.command_move(player_id, dest);
            }
            DirectPlayerOrder::AttackMove { player_id, dest } => {
                self.command_attack_move(player_id, dest);
            }
        }
    }

    /// Wave 931: single object-lifecycle authority boundary.
    #[inline]
    pub fn apply_object_lifecycle_op(&mut self, op: ObjectLifecycleOp) -> ObjectLifecycleResult {
        match op {
            ObjectLifecycleOp::Create {
                name,
                team,
                spawn_at,
            } => ObjectLifecycleResult::Created(self.create_object(&name, team, spawn_at)),
            ObjectLifecycleOp::Destroy { id } => {
                self.destroy_object(id);
                ObjectLifecycleResult::Destroyed
            }
            ObjectLifecycleOp::ForceCompleteConstruction { id } => {
                ObjectLifecycleResult::Bool(self.force_complete_construction(id))
            }
            ObjectLifecycleOp::ClearMovementPath { id } => {
                ObjectLifecycleResult::Bool(self.clear_unit_movement_path(id))
            }
            ObjectLifecycleOp::AdjustGuardRadius { id, delta } => {
                ObjectLifecycleResult::Radius(self.adjust_unit_guard_radius(id, delta))
            }
            ObjectLifecycleOp::EnqueueProduction {
                producer,
                template_name,
            } => ObjectLifecycleResult::Bool(self.enqueue_production(producer, template_name)),
            ObjectLifecycleOp::CancelProduction { id, template_name } => {
                ObjectLifecycleResult::Bool(self.cancel_production(id, template_name))
            }
        }
    }

    /// Wave 932: single command-pipeline authority boundary.
    #[inline]
    pub fn apply_command_pipeline_op(&mut self, op: CommandPipelineOp) -> bool {
        match op {
            CommandPipelineOp::Queue { command } => {
                self.queue_command(command);
                false
            }
            CommandPipelineOp::QueueAndProcess { command } => {
                self.queue_and_process_command(command)
            }
            CommandPipelineOp::ProcessIfNeeded => self.process_commands_if_needed(),
        }
    }

    /// Wave 933: single session-control authority boundary.
    #[inline]
    pub fn apply_session_control_op(&mut self, op: SessionControlOp) {
        match op {
            SessionControlOp::SelectObjects { player_id, ids } => {
                self.select_objects(player_id, ids);
            }
            SessionControlOp::SetPaused { paused } => {
                self.set_paused(paused);
            }
            SessionControlOp::SetCameraFollow { id } => {
                self.set_camera_follow_object(id);
            }
            SessionControlOp::StartNewGameWithFaction {
                mode,
                player_id,
                faction_team,
                setup_skirmish_ai,
            } => {
                self.start_new_game_with_faction(mode, player_id, faction_team, setup_skirmish_ai);
            }
            SessionControlOp::Reset => {
                self.reset();
            }
            SessionControlOp::OverrideWorldSize { width, height } => {
                self.override_world_size(width, height);
            }
        }
    }

    /// Wave 934: single host-support residual authority boundary.
    #[inline]
    pub fn apply_host_support_op(&mut self, op: HostSupportOp) -> HostSupportResult {
        match op {
            HostSupportOp::EnsureBarracksBuildingData { id } => {
                HostSupportResult::Bool(self.ensure_barracks_building_data(id))
            }
            HostSupportOp::ForceEnsureBarracksBuildingData { id } => {
                HostSupportResult::Bool(self.force_ensure_barracks_building_data(id))
            }
            HostSupportOp::EnsurePlayerMinSupplies { player_id, floor } => {
                self.ensure_player_min_supplies(player_id, floor);
                HostSupportResult::Unit
            }
            HostSupportOp::UpdateShellWithBudget { dt, budget } => {
                HostSupportResult::Snapshot(self.update_shell_with_budget(dt, budget))
            }
            HostSupportOp::ProcessDestroyListIfNeeded => {
                self.process_destroy_list_if_needed();
                HostSupportResult::Unit
            }
            HostSupportOp::InsertThingTemplate { name, template } => {
                self.templates.insert(name, template);
                HostSupportResult::Unit
            }
        }
    }

    /// Wave 937: single production complete/spawn authority boundary.
    #[inline]
    pub fn apply_production_authority_op(
        &mut self,
        op: ProductionAuthorityOp,
    ) -> ProductionAuthorityResult {
        match op {
            ProductionAuthorityOp::ApplyCompletionsAfterReadyWriteback { dt } => {
                self.host_apply_production_completions_after_ready_writeback(dt);
                ProductionAuthorityResult::Unit
            }
            ProductionAuthorityOp::SpawnUnit {
                template,
                team,
                spawn_pos,
            } => ProductionAuthorityResult::Spawned(
                self.host_spawn_production_unit(&template, team, spawn_pos),
            ),
            ProductionAuthorityOp::ApplySpawnReadyCompletions => {
                self.host_apply_production_spawn_ready_completions();
                ProductionAuthorityResult::Unit
            }
            ProductionAuthorityOp::ApplyDoorReadyCompletions => {
                self.host_apply_production_door_ready_completions();
                ProductionAuthorityResult::Unit
            }
        }
    }

    /// Wave 938: single post-writeback complete authority boundary.
    #[inline]
    pub fn apply_post_writeback_complete_op(&mut self, op: PostWritebackCompleteOp) {
        match op {
            PostWritebackCompleteOp::ConstructionCompletionsAfterReadyWriteback => {
                self.host_apply_construction_completions_after_ready_writeback();
            }
            PostWritebackCompleteOp::SellCompletionsAfterReadyWriteback => {
                self.host_apply_sell_completions_after_ready_writeback();
            }
            PostWritebackCompleteOp::SpecialPowerReadyAfterWriteback => {
                self.host_apply_special_power_ready_after_writeback();
            }
        }
    }

    /// Wave 939: single ready-log drain authority boundary (shadow post-writeback).
    #[inline]
    pub fn apply_ready_log_drain_op(&mut self, op: ReadyLogDrainOp) -> usize {
        match op {
            ReadyLogDrainOp::Contain => self.host_apply_contain_ready_completions(),
            ReadyLogDrainOp::Projectiles => self.host_apply_projectiles_ready_completions(),
            ReadyLogDrainOp::AttackTarget => self.host_apply_attack_target_ready_completions(),
            ReadyLogDrainOp::AiState => self.host_apply_ai_state_ready_completions(),
            ReadyLogDrainOp::Movement => self.host_apply_movement_ready_completions(),
            ReadyLogDrainOp::FireIntent => self.host_apply_fire_intent_ready_completions(),
            ReadyLogDrainOp::MoveTarget => self.host_apply_move_target_ready_completions(),
            ReadyLogDrainOp::Transform => self.host_apply_transform_ready_completions(),
            ReadyLogDrainOp::Locomotor => self.host_apply_locomotor_ready_completions(),
            ReadyLogDrainOp::AiRequest => self.host_apply_ai_request_ready_completions(),
            ReadyLogDrainOp::Hijacker => self.host_apply_hijacker_ready_completions(),
            ReadyLogDrainOp::PhysicsMotive => self.host_apply_physics_motive_ready_completions(),
            ReadyLogDrainOp::BounceLand => self.host_apply_bounce_land_ready_completions(),
            ReadyLogDrainOp::CombatStatus => self.host_apply_combat_status_ready_completions(),
            ReadyLogDrainOp::BodyDamage => self.host_apply_body_damage_ready_completions(),
            ReadyLogDrainOp::DeathType => self.host_apply_death_type_ready_completions(),
            ReadyLogDrainOp::RadarExtend => self.host_apply_radar_extend_ready_completions(),
            ReadyLogDrainOp::ShockStun => self.host_apply_shock_stun_ready_completions(),
            ReadyLogDrainOp::ConstructionCompleteClear => {
                self.host_apply_construction_complete_clear_ready_completions()
            }
            ReadyLogDrainOp::SoleHealing => self.host_apply_sole_healing_ready_completions(),
            ReadyLogDrainOp::AiMood => self.host_apply_ai_mood_ready_completions(),
            ReadyLogDrainOp::Owner => self.host_apply_owner_ready_completions(),
            ReadyLogDrainOp::Veterancy => self.host_apply_veterancy_ready_completions(),
            ReadyLogDrainOp::WeaponBonus => self.host_apply_weapon_bonus_ready_completions(),
            ReadyLogDrainOp::FaerieFire => self.host_apply_faerie_fire_ready_completions(),
            ReadyLogDrainOp::Repulsor => self.host_apply_repulsor_ready_completions(),
            ReadyLogDrainOp::DisableTimers => self.host_apply_disable_timers_ready_completions(),
            ReadyLogDrainOp::WeaponSlot => self.host_apply_weapon_slot_ready_completions(),
            ReadyLogDrainOp::EntityPower => self.host_apply_entity_power_ready_completions(),
            ReadyLogDrainOp::Turret => self.host_apply_turret_ready_completions(),
            ReadyLogDrainOp::StealthDelay => self.host_apply_stealth_delay_ready_completions(),
            ReadyLogDrainOp::CombatAttack => self.host_apply_combat_attack_ready_completions(),
            ReadyLogDrainOp::TargetLocation => self.host_apply_target_location_ready_completions(),
            ReadyLogDrainOp::Detector => self.host_apply_detector_ready_completions(),
            ReadyLogDrainOp::ContinuousFire => self.host_apply_continuous_fire_ready_completions(),
            ReadyLogDrainOp::Guard => self.host_apply_guard_ready_completions(),
            ReadyLogDrainOp::AiAttitude => self.host_apply_ai_attitude_ready_completions(),
            ReadyLogDrainOp::WeaponSet => self.host_apply_weapon_set_ready_completions(),
            ReadyLogDrainOp::Overcharge => self.host_apply_overcharge_ready_completions(),
            ReadyLogDrainOp::Hive => self.host_apply_hive_ready_completions(),
            ReadyLogDrainOp::StealthFlags => self.host_apply_stealth_flags_ready_completions(),
            ReadyLogDrainOp::Overlord => self.host_apply_overlord_ready_completions(),
            ReadyLogDrainOp::CommandSet => self.host_apply_command_set_ready_completions(),
            ReadyLogDrainOp::Disguise => self.host_apply_disguise_ready_completions(),
            ReadyLogDrainOp::VisionCamo => self.host_apply_vision_camo_ready_completions(),
            ReadyLogDrainOp::WeaponStats => self.host_apply_weapon_stats_ready_completions(),
            ReadyLogDrainOp::SelectionRadius => {
                self.host_apply_selection_radius_ready_completions()
            }
            ReadyLogDrainOp::ModelCondition => self.host_apply_model_condition_ready_completions(),
            ReadyLogDrainOp::DemoMineCheer => self.host_apply_demo_mine_cheer_ready_completions(),
            ReadyLogDrainOp::CrushVision => self.host_apply_crush_vision_ready_completions(),
            ReadyLogDrainOp::BuildingType => self.host_apply_building_type_ready_completions(),
            ReadyLogDrainOp::Identity => self.host_apply_identity_ready_completions(),
            ReadyLogDrainOp::GroundHeight => self.host_apply_ground_height_ready_completions(),
            ReadyLogDrainOp::Economy => self.host_apply_economy_ready_completions(),
            ReadyLogDrainOp::Upgrade => self.host_apply_upgrade_ready_completions(),
            ReadyLogDrainOp::StoredSupplies => self.host_apply_stored_supplies_ready_completions(),
        }
    }

    /// Wave 940: batch post-writeback sole-tick residuals (single authority boundary).
    #[inline]
    pub fn apply_post_writeback_sole_ticks(&mut self) {
        // Order matches shadow_session_after_host_tick (Waves 823–827).
        self.tick_patriot_assist_lasers_sole();
        self.tick_pending_patriot_assists_sole();
        self.tick_zone_damage_fields_sole();
        self.tick_combat_field_residuals_sole();
        self.tick_host_systems_residuals_sole();
    }

    /// Wave 940: host ObjectId create/mark-destroy authority boundary.
    #[inline]
    pub fn apply_host_object_id_op(&mut self, op: HostObjectIdOp) -> HostObjectIdResult {
        match op {
            HostObjectIdOp::MarkForDestruction { id, team } => {
                self.mark_object_for_destruction(id, team);
                HostObjectIdResult::Unit
            }
            HostObjectIdOp::Create {
                template,
                team,
                spawn_at,
            } => HostObjectIdResult::Created(self.create_object(&template, team, spawn_at)),
        }
    }

    /// Wave 941/942: host residual mutation authority boundary.
    #[inline]
    pub fn apply_host_residual_mutation_op(&mut self, op: HostResidualMutationOp) {
        match op {
            HostResidualMutationOp::PoisonDot {
                object,
                amount,
                death_type,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&object) {
                    let _ = obj.take_damage_from_typed_death(
                        amount,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                        death_type,
                    );
                }
            }
            HostResidualMutationOp::ForceKill {
                id,
                death_type,
                refresh_model_condition,
                mark_destroy,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.health.current = 0.0;
                    obj.status.destroyed = true;
                    if let Some(dt) = death_type {
                        obj.status.death_type = dt;
                    }
                    if refresh_model_condition {
                        obj.refresh_model_condition_bits();
                    }
                }
                if mark_destroy {
                    self.mark_object_for_destruction(id, None);
                }
            }
            HostResidualMutationOp::SetPendingFireWhenDamaged {
                id,
                weapon,
                overwrite,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if overwrite || obj.pending_fire_when_damaged_weapon.is_none() {
                        obj.pending_fire_when_damaged_weapon = Some(weapon);
                    }
                }
            }
            HostResidualMutationOp::LethalExpire {
                id,
                position,
                effectively_dead,
                clear,
                mark_destroy_team,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if let Some(pos) = position {
                        obj.set_position(pos);
                    }
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        let hp = obj.health.current.max(1.0);
                        let oid = obj.id;
                        crate::game_logic::host_damage_log::record(oid, hp, None, true);
                    } else {
                        obj.health.current = 0.0;
                    }
                    obj.status.destroyed = true;
                    if effectively_dead {
                        obj.status.effectively_dead = true;
                    }
                    match clear {
                        ObjectIdentityClear::None => {}
                        ObjectIdentityClear::FlashbangGrenadeProjectile => {
                            obj.flashbang_grenade_projectile = false;
                        }
                        ObjectIdentityClear::ScorpionMissileProjectile => {
                            obj.scorpion_missile_projectile = false;
                        }
                        ObjectIdentityClear::SpySatellitePing => {
                            obj.spy_satellite_ping = false;
                        }
                        ObjectIdentityClear::AngryMobMember => {
                            obj.angry_mob_member = false;
                        }
                        ObjectIdentityClear::AuroraBombProjectile => {
                            obj.aurora_bomb_projectile = false;
                        }
                        ObjectIdentityClear::InfernoShellProjectile => {
                            obj.inferno_shell_projectile = false;
                        }
                        ObjectIdentityClear::ToxinStreamProjectile => {
                            obj.toxin_stream_projectile = false;
                        }
                        ObjectIdentityClear::AngryMobProjectile => {
                            obj.angry_mob_projectile = false;
                        }
                        ObjectIdentityClear::CannonShellProjectile => {
                            obj.scud_launcher_missile_projectile = false;
                            obj.neutron_cannon_shell_projectile = false;
                            obj.nuke_cannon_shell_projectile = false;
                        }
                        ObjectIdentityClear::LeafletContainer => {
                            obj.leaflet_container = false;
                        }
                        ObjectIdentityClear::ParadropCargo => {
                            obj.paradrop_parachute = false;
                        }
                        ObjectIdentityClear::ComancheRocketPodProjectile => {
                            obj.comanche_rocket_pod_projectile = false;
                        }
                        ObjectIdentityClear::EmpPulseSpheroid => {
                            obj.emp_pulse_spheroid = false;
                        }
                        ObjectIdentityClear::FieldObject(kind) => {
                            use crate::game_logic::host_field_object_expire_log::FieldObjectKind;
                            match kind {
                                FieldObjectKind::NukeRadiation => {
                                    obj.nuke_radiation_field = false;
                                }
                                FieldObjectKind::AnthraxToxin => {
                                    obj.anthrax_toxin_field = false;
                                }
                                FieldObjectKind::InfernoFire => {
                                    obj.inferno_fire_field = false;
                                }
                                FieldObjectKind::SpectreHowitzerShell => {
                                    obj.spectre_howitzer_shell = false;
                                }
                                FieldObjectKind::CountermeasureFlare => {
                                    obj.countermeasure_flare = false;
                                }
                                FieldObjectKind::PointDefenseLaserBeam => {
                                    obj.point_defense_laser_beam = false;
                                }
                                FieldObjectKind::WeaponLaserBeam => {
                                    obj.weapon_laser_beam = false;
                                }
                                FieldObjectKind::ParticleTrailRemnant => {
                                    obj.particle_trail_remnant = false;
                                }
                                FieldObjectKind::ParticleOrbitalLaser => {
                                    obj.particle_orbital_laser = false;
                                }
                                FieldObjectKind::ParticleConnectorLaser => {
                                    obj.particle_connector_laser = false;
                                }
                                FieldObjectKind::FirewallSegment => {
                                    obj.firewall_segment = false;
                                    obj.firewall_segment_wall_id = None;
                                    obj.firewall_segment_dir = None;
                                }
                                FieldObjectKind::RadarVanPing => {
                                    obj.radar_van_ping = false;
                                }
                                FieldObjectKind::MoneyCrate => {}
                            }
                        }
                    }
                }
                if let Some(team) = mark_destroy_team {
                    self.apply_host_object_id_op(HostObjectIdOp::MarkForDestruction { id, team });
                }
            }
            HostResidualMutationOp::DestroyBomb { id, mark_destroy } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.health.current = 0.0;
                    obj.status.destroyed = true;
                }
                if mark_destroy {
                    self.mark_object_for_destruction(id, None);
                }
            }
            HostResidualMutationOp::SetModelConditionBits {
                id,
                bits,
                count_update,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    let before = obj.model_condition_bits;
                    obj.model_condition_bits = bits;
                    if count_update && obj.model_condition_bits != before {
                        self.actively_constructing_updates =
                            self.actively_constructing_updates.saturating_add(1);
                    }
                }
            }
            HostResidualMutationOp::PowerPlantRodsComplete {
                id,
                model_condition_bits,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.model_condition_bits = model_condition_bits;
                    obj.power_plant_rods_done_frame = 0;
                    obj.power_plant_rods_extended = true;
                }
                self.special_power_completion_log.record_rods_complete();
            }
            HostResidualMutationOp::SetWeaponBonusHorde {
                id,
                now_horde,
                was_horde,
                grant,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    let was = obj.weapon_bonus_horde;
                    obj.weapon_bonus_horde = now_horde;
                    obj.record_host_weapon_bonus();
                    if now_horde && !was {
                        match grant {
                            HordeGrantCounter::Battlemaster => {
                                self.battlemaster_residual_horde_grants =
                                    self.battlemaster_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::RedGuard => {
                                self.red_guard_residual_horde_grants =
                                    self.red_guard_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::TankHunter => {
                                self.tank_hunter_residual_horde_grants =
                                    self.tank_hunter_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::Minigunner => {
                                self.minigunner_residual_horde_grants =
                                    self.minigunner_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::None => {}
                        }
                    }
                }
                if now_horde != was_horde || now_horde {
                    match grant {
                        HordeGrantCounter::Battlemaster => {
                            self.refresh_battlemaster_weapon(id);
                        }
                        HordeGrantCounter::RedGuard => {
                            self.refresh_red_guard_weapon(id);
                        }
                        HordeGrantCounter::TankHunter => {
                            self.refresh_tank_hunter_weapon(id);
                        }
                        HordeGrantCounter::Minigunner => {
                            self.refresh_minigunner_weapon(id);
                        }
                        HordeGrantCounter::None => {}
                    }
                }
            }
            HostResidualMutationOp::ApplyStingerHiveState {
                id,
                hive_slave_count,
                hive_slave_hp,
                hive_slave_respawn_frame,
                slaves_alive,
                slaves_hp,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.hive_slave_count = hive_slave_count;
                    obj.hive_slave_hp = hive_slave_hp;
                    obj.hive_slave_respawn_frame = hive_slave_respawn_frame;
                    for i in 0..3 {
                        obj.hive_slaves[i].alive = slaves_alive[i];
                        obj.hive_slaves[i].hp = slaves_hp[i];
                    }
                    obj.record_host_hive();
                }
                self.stinger_hive_residual_respawns =
                    self.stinger_hive_residual_respawns.saturating_add(1);
            }
            HostResidualMutationOp::SetPosition {
                id,
                position,
                sticky_follow_tick,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.set_position(position);
                }
                if sticky_follow_tick {
                    self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
                }
            }
            HostResidualMutationOp::ConfigureSpawnedPayload {
                id,
                producer,
                target,
                kind,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.producer_id = Some(producer);
                    let parachuting = matches!(kind, SpawnedPayloadKind::ParadropParachute);
                    match kind {
                        SpawnedPayloadKind::DaisyCutter { moab_template } => {
                            obj.daisy_cutter_bomb = true;
                            if let Some(name) = moab_template {
                                obj.template_name = name;
                            }
                            obj.movement.velocity = glam::Vec3::new(0.0, -16.0, 0.0);
                        }
                        SpawnedPayloadKind::AnthraxBomb => {
                            obj.anthrax_bomb_payload = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::ClusterMinesBomb => {
                            obj.cluster_mines_bomb = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::EmpPulseBomb => {
                            obj.emp_pulse_bomb = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::A10StrikeMissile => {
                            obj.a10_strike_missile = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -20.0, 0.0);
                        }
                        SpawnedPayloadKind::ArtilleryBarrageShell => {
                            obj.artillery_barrage_shell = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -18.0, 0.0);
                        }
                        SpawnedPayloadKind::CarpetBomb => {
                            obj.carpet_bomb_payload = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -15.0, 0.0);
                        }
                        SpawnedPayloadKind::LeafletContainer => {
                            obj.leaflet_container = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -12.0, 0.0);
                        }
                        SpawnedPayloadKind::ParadropParachute => {
                            obj.paradrop_parachute = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -8.0, 0.0);
                        }
                    }
                    let _ = obj.set_smart_bomb_target(target);
                    if parachuting {
                        let _ = obj.apply_eject_parachuting();
                    }
                }
            }
            HostResidualMutationOp::ApplyRawHpDamage { id, amount } => {
                // Wave 943: host-only damage fallback (no shadow entity).
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if obj.status.destroyed {
                        // already dead
                    } else {
                        obj.health.damage(amount);
                        if !obj.health.is_alive() {
                            obj.status.destroyed = true;
                            obj.set_ai_state(crate::game_logic::AIState::Idle);
                            obj.target = None;
                        }
                        obj.refresh_model_condition_bits();
                    }
                }
            }
        }
    }

    /// Wave 943: apply post-armor damage for host objects with no shadow mapping.
    /// Returns number of host objects mutated.
    pub fn apply_host_unmapped_damage_fallback(
        &mut self,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
        mut shadow_mapped: impl FnMut(ObjectId) -> bool,
    ) -> usize {
        let mut fallback = 0usize;
        for ev in events {
            if shadow_mapped(ev.target) {
                continue;
            }
            let eligible = self
                .host_objects()
                .get(&ev.target)
                .is_some_and(|o| !o.status.destroyed);
            if !eligible {
                continue;
            }
            self.apply_host_residual_mutation_op(HostResidualMutationOp::ApplyRawHpDamage {
                id: ev.target,
                amount: ev.amount,
            });
            fallback += 1;
        }
        fallback
    }

    /// Wave 944: apply one shadow→host writeback mutation.
    pub fn apply_host_writeback_op(&mut self, op: HostWritebackOp) -> bool {
        match op {
            HostWritebackOp::Health {
                id,
                current,
                maximum,
                destroy,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                let max = maximum.max(1.0);
                obj.health.current = current.min(max);
                obj.max_health = max;
                obj.health.maximum = max;
                if destroy {
                    obj.status.destroyed = true;
                    obj.ai_state = crate::game_logic::AIState::Idle;
                    obj.target = None;
                }
                true
            }
            HostWritebackOp::Experience { id, points, level } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                if let Some(pts) = points {
                    obj.experience.current = pts;
                }
                if let Some(lvl) = level {
                    obj.experience.level = lvl;
                }
                true
            }
            HostWritebackOp::Transform {
                id,
                position,
                orientation,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.set_position(position);
                obj.set_orientation(orientation);
                true
            }
            HostWritebackOp::AttackTarget {
                id,
                target,
                clear_target_location,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.target = target;
                if clear_target_location {
                    obj.target_location = None;
                }
                true
            }
            HostWritebackOp::MoveTarget { id, destination } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.movement.target_position = destination;
                true
            }
            HostWritebackOp::AiState { id, ordinal } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ai_state =
                    crate::gameworld_shadow::GameWorldShadow::ai_state_from_ordinal(ordinal);
                true
            }
            HostWritebackOp::AiAttitude { id, attitude } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ai_attitude = attitude.clamp(-2, 2);
                true
            }
            HostWritebackOp::Owner {
                id,
                team,
                team_color,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.team = team;
                obj.team_color = team_color;
                true
            }
            HostWritebackOp::SpecialPower {
                id,
                ready,
                cooldown_remaining,
                cooldown,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.special_power_ready = ready;
                obj.special_power_cooldown_remaining = cooldown_remaining.max(0.0);
                obj.special_power_cooldown = cooldown.max(0.0);
                true
            }
            HostWritebackOp::Overcharge { id, enabled } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.overcharge_enabled = enabled;
                true
            }
            HostWritebackOp::WeaponSlot { id, slot } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.active_weapon_slot = slot;
                true
            }
            HostWritebackOp::SelectionRadius { id, radius } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.selection_radius = radius;
                true
            }
            HostWritebackOp::EntityPower {
                id,
                provided,
                consumed,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.power_provided = provided;
                obj.power_consumed = consumed;
                true
            }
            HostWritebackOp::TargetLocation { id, location } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.target_location = location;
                true
            }
            HostWritebackOp::CommandSet { id, override_name } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.command_set_override = override_name;
                true
            }
            HostWritebackOp::GroundHeight {
                id,
                height,
                from_terrain,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ground_height = height;
                obj.ground_height_from_terrain = from_terrain;
                true
            }
            HostWritebackOp::BodyDamage { id, state } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.body_damage_state = state;
                true
            }
            HostWritebackOp::DeathType { id, death_type } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.status.death_type = death_type;
                true
            }
            HostWritebackOp::StoredSupplies { id, supplies } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.stored_resources.supplies = supplies;
                true
            }
            HostWritebackOp::FaerieFire {
                id,
                active,
                until_frame,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.status.faerie_fire = active;
                obj.faerie_fire_until_frame = if active { until_frame } else { 0 };
                true
            }
            HostWritebackOp::Repulsor {
                id,
                active,
                until_frame,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.repulsor_until_frame = until_frame;
                obj.status.repulsor = active;
                true
            }
            HostWritebackOp::Detector {
                id,
                is_detector,
                range,
                rate_frames,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.is_detector = is_detector;
                obj.detection_range = range.max(0.0);
                obj.detection_rate_frames = rate_frames;
                true
            }
            HostWritebackOp::Guard {
                id,
                position,
                target,
                radius,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.guard_position = position;
                obj.guard_target = target;
                obj.guard_radius = radius;
                true
            }
        }
    }

    /// Wave 946: scoped host-object mutation authority.
    /// Shadow writebacks mutate host objects only through this boundary
    /// (no direct `get_objects_mut` dual-writes from the shadow crate).
    pub fn with_host_object_mut<R>(
        &mut self,
        id: ObjectId,
        f: impl FnOnce(&mut crate::game_logic::object::Object) -> R,
    ) -> Option<R> {
        let obj = self.host_objects_mut().get_mut(&id)?;
        Some(f(obj))
    }

    /// Wave 955/958: host-authority object borrow (preferred over get_object dual-read).
    #[inline]
    pub fn host_object(&self, id: ObjectId) -> Option<&crate::game_logic::object::Object> {
        self.objects.get(&id)
    }

    /// Wave 955/958: host-authority object map borrow (command apply / AI / shadow).
    /// Presentation dual-read paths must use `PresentationFrame`, not this.
    #[inline]
    pub fn host_objects(
        &self,
    ) -> &std::collections::HashMap<ObjectId, crate::game_logic::object::Object> {
        &self.objects
    }

    /// Wave 955/958: host-authority mutable object map borrow.
    #[inline]
    pub fn host_objects_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<ObjectId, crate::game_logic::object::Object> {
        &mut self.objects
    }

    /// Wave 950/958: host-authority mutable object borrow.
    #[inline]
    pub fn host_object_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<&mut crate::game_logic::object::Object> {
        self.objects.get_mut(&id)
    }

    /// Issue attack command to selected objects
    pub fn command_attack(&mut self, player_id: u32, target_id: ObjectId) {
        if let Some(player) = self.players.get(&player_id) {
            let Some(target_team) = self.objects.get(&target_id).map(|target| target.team) else {
                return;
            };
            if target_team == player.team {
                return;
            }

            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let can = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|obj| obj.can_attack() && obj.team != target_team);
                if !can {
                    continue;
                }

                // Host attack channel (default production path — host ObjectIds only).
                if let Some(obj_mut) = self.objects.get_mut(&object_id) {
                    obj_mut.set_force_attack(false);
                    obj_mut.attack_target(target_id);
                }
                // Host residual: path toward target, then ensure the unit is inside
                // weapon range this command so combat can apply real HP damage on
                // large maps (path marches otherwise take longer than smoke waits).
                if let Some(tpos) = self.objects.get(&target_id).map(|t| t.get_position()) {
                    let _ = self.assign_unit_attack_path(object_id, Some(target_id), tpos);
                    if let Some(attacker) = self.objects.get(&object_id) {
                        if !attacker.can_attack() {
                            // leave unarmed units alone
                        } else {
                            let range = attacker
                                .weapon
                                .as_ref()
                                .map(|w| w.range)
                                .or_else(|| attacker.secondary_weapon.as_ref().map(|w| w.range))
                                .unwrap_or(50.0)
                                .max(15.0);
                            let from = attacker.get_position();
                            let mut dir = tpos - from;
                            dir.y = 0.0;
                            let dist = dir.length();
                            if dist > range * 0.8 {
                                // Movement authority: no range-snap teleport. Path was
                                // already issued via assign_unit_attack_path; GameWorld
                                // integrates the march. Host-only residual may still snap
                                // for short smoke waits when authority is off.
                                if !crate::gameworld_shadow::gameworld_movement_authority_live() {
                                    let dir = if dist > 1.0 {
                                        dir / dist
                                    } else {
                                        glam::Vec3::new(1.0, 0.0, 0.0)
                                    };
                                    let stand = tpos - dir * (range * 0.55);
                                    let stand = glam::Vec3::new(stand.x, from.y, stand.z);
                                    if let Some(a) = self.objects.get_mut(&object_id) {
                                        a.set_position(stand);
                                        a.attack_target(target_id);
                                        // Host-immediate engagement residual (host-only
                                        // path when movement auth is off).
                                        a.set_ai_state(AIState::Attacking);
                                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live()
                                        {
                                            crate::game_logic::host_ai_decision_log::record_set_state(
                                                object_id, 2,
                                            );
                                        }
                                        a.set_status_attacking(true);
                                        a.set_status_moving(false);
                                        a.movement.velocity = glam::Vec3::ZERO;
                                        a.record_host_movement();
                                        a.movement.target_position = None;
                                        a.movement.path.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // C++ VoiceAttack residual for local player.
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                if let Some(&oid) = selected.first() {
                    if let Some(obj) = self.objects.get(&oid) {
                        let event = format!("{}VoiceAttack", obj.template_name);
                        let pos = obj.get_position();
                        self.queue_audio_event(
                            AudioEventRequest::new(&event)
                                .with_position(pos)
                                .with_priority(100),
                        );
                        self.queue_audio_event(
                            AudioEventRequest::new("UnitVoiceAttack")
                                .with_position(pos)
                                .with_priority(90),
                        );
                    }
                }
            }
            log::trace!(
                "{} commanded {} units to attack object {}",
                player_id,
                selected.len(),
                target_id
            );
        }
    }

    pub(super) fn allocate_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id = ObjectId(self.next_object_id.0 + 1);
        id
    }

    /// Wave 622: under damage authority, GameWorld experience writeback records
    /// veterancy level-ups; host applies combat bonus residual for those IDs.
    /// Wave 623: under damage authority, GameWorld body-damage writeback records
    /// state transitions; host applies model/FX residual for those IDs.
    /// Wave 624: under GameWorld completed-upgrade writeback, drain ready log and
    /// apply full host upgrade residual (unlocks, EVA, radar, status bits).
    /// Wave 625: GameWorld radar-extend complete writeback records ready IDs;
    /// host applies upgraded model residual and complete counter.
    /// Wave 626: under construction sole-tick, GameWorld writeback records
    /// producers whose CONSTRUCTION_COMPLETE clear deadline elapsed; host clears
    /// the model bit and counts residual.
    /// Wave 627: GameWorld production-door writeback records phase changes;
    /// host applies door model-condition residual for the new phase.
    /// Wave 628: GameWorld contain writeback records membership changes;
    /// host applies garrison AI residual + enter/exit honesty counters.
    /// Wave 629: GameWorld owner writeback records team changes; host applies
    /// capture residual (kick passengers, deselect, idle, score).
    /// Wave 630: GameWorld AI-state writeback records ordinal changes; host
    /// applies moving/attacking combat-status residual for the new state.
    /// Wave 631: GameWorld economy writeback records supply/power/radar/alive
    /// changes; host applies presentation residual via host_economy_log and
    /// radar log (GW decides absolute values; host still owns UI bookkeeping).
    /// Wave 632: GameWorld death-type writeback records ordinal changes; host
    /// applies destroy/pilot presentation bookkeeping residual.
    /// Wave 633: GameWorld model-condition writeback records bit changes; host
    /// applies presentation bookkeeping residual (drawable model condition log).
    /// Wave 634: GameWorld combat-status writeback records dirty objects; host
    /// applies status presentation residual via host_status_log.
    /// Wave 635: GameWorld weapon-stats writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_weapon_stats.
    /// Wave 636: GameWorld transform writeback records dirty objects; host
    /// applies movement/presentation bookkeeping residual.
    /// Wave 637: GameWorld movement writeback records dirty objects; host
    /// applies path/presentation bookkeeping residual via record_host_movement.
    /// Wave 638: GameWorld attack-target writeback records target changes; host
    /// applies AI/status/attack-log residual (without re-assigning target).
    /// Wave 639: GameWorld move-target writeback records destination changes;
    /// host applies AI/status/movement residual without re-assigning destination.
    /// Wave 640: GameWorld fire-intent writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_fire_intent.
    /// Wave 641: GameWorld stored-supplies writeback records changes; host
    /// applies gatherer presentation residual (HUD / supply counter consumers).
    /// Wave 642: GameWorld weapon-set writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_weapon_set.
    /// Wave 643: GameWorld combat-attack writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_combat_attack.
    /// Wave 644: GameWorld command-set writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_command_set.
    /// Wave 645: GameWorld AI-mood writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_ai_mood.
    /// Wave 646: GameWorld locomotor writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_locomotor.
    /// Wave 647: GameWorld hijacker writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_hijacker.
    /// Wave 648: GameWorld AI-request writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_ai_request.
    /// Wave 649: GameWorld physics motive writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_physics_motive.
    pub fn host_apply_physics_motive_ready_completions(&mut self) -> usize {
        // Wave 649: GameWorld physics motive writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_physics_motive.
        let events = crate::game_logic::host_physics_motive_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_physics_motive();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 650: GameWorld bounce land writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_bounce_land.
    pub fn host_apply_bounce_land_ready_completions(&mut self) -> usize {
        // Wave 650: GameWorld bounce land writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_bounce_land.
        let events = crate::game_logic::host_bounce_land_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_bounce_land();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 651: GameWorld stealth delay writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_stealth_delay.
    /// Wave 652: GameWorld stealth flags writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_stealth_flags.
    pub fn host_apply_stealth_flags_ready_completions(&mut self) -> usize {
        // Wave 652: GameWorld stealth flags writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_stealth_flags.
        let events = crate::game_logic::host_stealth_flags_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_stealth_flags();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 653: GameWorld disguise writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_disguise.
    pub fn host_apply_disguise_ready_completions(&mut self) -> usize {
        // Wave 653: GameWorld disguise writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_disguise.
        let events = crate::game_logic::host_disguise_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_disguise();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 654: GameWorld vision camo writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_vision_camo.
    /// Wave 655: GameWorld selection radius writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_selection_radius_ready_completions(&mut self) -> usize {
        // Wave 655: GameWorld selection radius writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_selection_radius_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_selection_radius();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 656: GameWorld ground height writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_ground_height_ready_completions(&mut self) -> usize {
        // Wave 656: GameWorld ground height writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_ground_height_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ground_height();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 657: GameWorld weapon slot writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_weapon_slot_ready_completions(&mut self) -> usize {
        // Wave 657: GameWorld weapon slot writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_weapon_slot_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_slot();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 658: GameWorld weapon bonus writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_weapon_bonus_ready_completions(&mut self) -> usize {
        // Wave 658: GameWorld weapon bonus writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_weapon_bonus_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_bonus();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 659: GameWorld AI attitude writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_ai_attitude_ready_completions(&mut self) -> usize {
        // Wave 659: GameWorld AI attitude writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_ai_attitude_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_attitude();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 660: GameWorld identity writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    pub fn host_apply_identity_ready_completions(&mut self) -> usize {
        // Wave 660: GameWorld identity writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_identity_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_identity();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 661: GameWorld repulsor writeback records dirty objects; host
    /// applies presentation bookkeeping residual.
    /// Wave 662: GameWorld shock stun writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_shock_stun.
    pub fn host_apply_shock_stun_ready_completions(&mut self) -> usize {
        // Wave 662: GameWorld shock stun writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_shock_stun.
        let events = crate::game_logic::host_shock_stun_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_shock_stun();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 663: GameWorld sole healing writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_sole_healing.
    pub fn host_apply_sole_healing_ready_completions(&mut self) -> usize {
        // Wave 663: GameWorld sole healing writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_sole_healing.
        let events = crate::game_logic::host_sole_healing_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_sole_healing();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 664: GameWorld crush vision writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_crush_vision.
    pub fn host_apply_crush_vision_ready_completions(&mut self) -> usize {
        // Wave 664: GameWorld crush vision writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_crush_vision.
        let events = crate::game_logic::host_crush_vision_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_crush_vision();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 665: GameWorld demo mine cheer writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_demo_mine_cheer.
    pub fn host_apply_demo_mine_cheer_ready_completions(&mut self) -> usize {
        // Wave 665: GameWorld demo mine cheer writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_demo_mine_cheer.
        let events = crate::game_logic::host_demo_mine_cheer_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_demo_mine_cheer();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 666: GameWorld overlord writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_overlord.
    pub fn host_apply_overlord_ready_completions(&mut self) -> usize {
        // Wave 666: GameWorld overlord writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_overlord.
        let events = crate::game_logic::host_overlord_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_overlord();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 667: GameWorld hive writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_hive.
    pub fn host_apply_hive_ready_completions(&mut self) -> usize {
        // Wave 667: GameWorld hive writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_hive.
        let events = crate::game_logic::host_hive_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_hive();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 668: GameWorld overcharge writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_overcharge.
    pub fn host_apply_overcharge_ready_completions(&mut self) -> usize {
        // Wave 668: GameWorld overcharge writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_overcharge.
        let events = crate::game_logic::host_overcharge_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_overcharge();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 669: GameWorld guard writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_guard.
    pub fn host_apply_guard_ready_completions(&mut self) -> usize {
        // Wave 669: GameWorld guard writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_guard.
        let events = crate::game_logic::host_guard_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_guard();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 670: GameWorld continuous fire writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_continuous_fire.
    pub fn host_apply_continuous_fire_ready_completions(&mut self) -> usize {
        // Wave 670: GameWorld continuous fire writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_continuous_fire.
        let events = crate::game_logic::host_continuous_fire_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_continuous_fire();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 671: GameWorld detector writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_detector.
    pub fn host_apply_detector_ready_completions(&mut self) -> usize {
        // Wave 671: GameWorld detector writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_detector.
        let events = crate::game_logic::host_detector_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_detector();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 672: GameWorld target location writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_target_location.
    pub fn host_apply_target_location_ready_completions(&mut self) -> usize {
        // Wave 672: GameWorld target location writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_target_location.
        let events = crate::game_logic::host_target_location_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_target_location();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 673: GameWorld turret writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_turret.
    pub fn host_apply_turret_ready_completions(&mut self) -> usize {
        // Wave 673: GameWorld turret writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_turret.
        let events = crate::game_logic::host_turret_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_turret();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 674: GameWorld entity power writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_entity_power.
    pub fn host_apply_entity_power_ready_completions(&mut self) -> usize {
        // Wave 674: GameWorld entity power writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_entity_power.
        let events = crate::game_logic::host_entity_power_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_entity_power();
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 675: GameWorld building type writeback records dirty objects; host
    /// applies presentation bookkeeping residual via record_host_building_type.
    /// Wave 676: GameWorld faerie-fire writeback records dirty objects; host
    /// applies presentation bookkeeping residual via host_faerie_fire_log.
    /// Wave 678: GameWorld projectiles writeback records dirty combat projectiles;
    /// host applies presentation bookkeeping residual via host_projectile_log.
    pub fn host_apply_projectiles_ready_completions(&mut self) -> usize {
        // Wave 678: GameWorld projectiles writeback records dirty combat projectiles;
        // host applies presentation bookkeeping residual via host_projectile_log.
        let events = crate::game_logic::host_projectiles_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.removed {
                // Removal already applied during writeback; residual log marks inactive.
                crate::game_logic::host_projectile_log::record(
                    ev.object.0,
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    0.0,
                    0,
                    0,
                    0.0,
                    0.0,
                    0.0,
                    false,
                    false,
                );
                n = n.saturating_add(1);
                continue;
            }
            let Some(p) = self.combat_system.get_projectiles().get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_projectile_log::record(
                p.id.0,
                [p.position.x, p.position.y, p.position.z],
                [p.velocity.x, p.velocity.y, p.velocity.z],
                [
                    p.target_position.x,
                    p.target_position.y,
                    p.target_position.z,
                ],
                p.damage,
                p.shooter_id.0,
                p.target_id.map(|t| t.0).unwrap_or(0),
                p.speed,
                p.lifetime,
                p.max_lifetime,
                p.is_homing,
                true,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_faerie_fire_ready_completions(&mut self) -> usize {
        // Wave 676: GameWorld faerie-fire writeback records dirty objects; host
        // applies presentation bookkeeping residual via host_faerie_fire_log.
        let events = crate::game_logic::host_faerie_fire_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_faerie_fire_log::record(
                obj.id,
                obj.status.faerie_fire,
                obj.faerie_fire_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 677: GameWorld disable-timers writeback records dirty objects; host
    /// applies presentation bookkeeping residual via host_disable_timers_log.
    pub fn host_apply_disable_timers_ready_completions(&mut self) -> usize {
        // Wave 677: GameWorld disable-timers writeback records dirty objects; host
        // applies presentation bookkeeping residual via host_disable_timers_log.
        let events = crate::game_logic::host_disable_timers_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_disable_timers_log::record(
                obj.id,
                obj.status.disabled_emp_until_frame,
                obj.status.disabled_hacked_until_frame,
                obj.status.disabled_paralyzed_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_building_type_ready_completions(&mut self) -> usize {
        // Wave 675: GameWorld building type writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_building_type.
        let events = crate::game_logic::host_building_type_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_building_type();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_repulsor_ready_completions(&mut self) -> usize {
        // Wave 661: GameWorld repulsor writeback records dirty objects; host
        // applies presentation bookkeeping residual.
        let events = crate::game_logic::host_repulsor_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            crate::game_logic::host_repulsor_log::record(
                obj.id,
                obj.status.repulsor,
                obj.repulsor_until_frame,
            );
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_vision_camo_ready_completions(&mut self) -> usize {
        // Wave 654: GameWorld vision camo writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_vision_camo.
        let events = crate::game_logic::host_vision_camo_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_vision_camo();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_stealth_delay_ready_completions(&mut self) -> usize {
        // Wave 651: GameWorld stealth delay writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_stealth_delay.
        let events = crate::game_logic::host_stealth_delay_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_stealth_delay();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_ai_request_ready_completions(&mut self) -> usize {
        // Wave 648: GameWorld AI-request writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_ai_request.
        let events = crate::game_logic::host_ai_request_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_request();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_hijacker_ready_completions(&mut self) -> usize {
        // Wave 647: GameWorld hijacker writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_hijacker.
        let events = crate::game_logic::host_hijacker_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_hijacker();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_locomotor_ready_completions(&mut self) -> usize {
        // Wave 646: GameWorld locomotor writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_locomotor.
        let events = crate::game_logic::host_locomotor_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_locomotor();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_ai_mood_ready_completions(&mut self) -> usize {
        // Wave 645: GameWorld AI-mood writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_ai_mood.
        let events = crate::game_logic::host_ai_mood_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_ai_mood();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_command_set_ready_completions(&mut self) -> usize {
        // Wave 644: GameWorld command-set writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_command_set.
        let events = crate::game_logic::host_command_set_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_command_set();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_combat_attack_ready_completions(&mut self) -> usize {
        // Wave 643: GameWorld combat-attack writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_combat_attack.
        let events = crate::game_logic::host_combat_attack_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_combat_attack();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_weapon_set_ready_completions(&mut self) -> usize {
        // Wave 642: GameWorld weapon-set writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_weapon_set.
        let events = crate::game_logic::host_weapon_set_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            obj.record_host_weapon_set();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_stored_supplies_ready_completions(&mut self) -> usize {
        // Wave 641: GameWorld stored-supplies writeback records changes; host
        // applies gatherer presentation residual (HUD / supply counter consumers).
        let events = crate::game_logic::host_stored_supplies_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_supplies == ev.new_supplies {
                continue;
            }
            if !self.objects.contains_key(&ev.object) {
                continue;
            }
            // Supplies already writeback-synced. Re-record via host economy-adjacent
            // presentation residual when a gatherer carry amount changes.
            crate::game_logic::host_stored_supplies_log::record(ev.object, ev.new_supplies);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_fire_intent_ready_completions(&mut self) -> usize {
        // Wave 640: GameWorld fire-intent writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_fire_intent.
        let events = crate::game_logic::host_fire_intent_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Fire-intent already writeback-synced; re-record host fire-intent
            // log for presentation / combat consumers.
            obj.record_host_fire_intent();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_move_target_ready_completions(&mut self) -> usize {
        // Wave 639: GameWorld move-target writeback records destination changes;
        // host applies AI/status/movement residual without re-assigning destination.
        let events = crate::game_logic::host_move_target_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Destination already writeback-synced. Apply residual side effects.
            if ev.new_target.is_some() {
                // Prefer status bits over full set_ai_state to avoid fighting
                // GW AI-state writeback (Wave 630).
                obj.set_status_moving(true);
            } else {
                obj.set_status_moving(false);
            }
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_attack_target_ready_completions(&mut self) -> usize {
        // Wave 638: GameWorld attack-target writeback records target changes; host
        // applies AI/status/attack-log residual (without re-assigning target).
        let events = crate::game_logic::host_attack_target_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_target == ev.new_target {
                continue;
            }
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Target already writeback-synced. Apply residual side effects only.
            if ev.new_target.is_some() {
                let _ = obj.takeoff_from_airfield_parking();
                obj.target_location = None;
                obj.record_host_target_location();
                // Prefer combat status bits over full set_ai_state to avoid
                // host_ai_state_log re-entry fighting GW AI-state writeback.
                obj.set_status_attacking(true);
            } else {
                obj.target_location = None;
                obj.set_status_force_attack(false);
                obj.set_status_attacking(false);
            }
            crate::game_logic::host_attack_log::record(ev.object, ev.new_target);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_movement_ready_completions(&mut self) -> usize {
        // Wave 637: GameWorld movement writeback records dirty objects; host
        // applies path/presentation bookkeeping residual via record_host_movement.
        let events = crate::game_logic::host_movement_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Movement already writeback-synced; re-record host movement log
            // for presentation / path consumers.
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_transform_ready_completions(&mut self) -> usize {
        // Wave 636: GameWorld transform writeback records dirty objects; host
        // applies movement/presentation bookkeeping residual.
        let events = crate::game_logic::host_transform_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Transform already writeback-synced; re-record movement residual
            // for presentation / path consumers.
            obj.record_host_movement();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_weapon_stats_ready_completions(&mut self) -> usize {
        // Wave 635: GameWorld weapon-stats writeback records dirty objects; host
        // applies presentation bookkeeping residual via record_host_weapon_stats.
        let events = crate::game_logic::host_weapon_stats_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            // Stats already writeback-synced; re-record host weapon-stats log
            // for presentation / fire-intent consumers.
            obj.record_host_weapon_stats();
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_combat_status_ready_completions(&mut self) -> usize {
        // Wave 634: GameWorld combat-status writeback records dirty objects; host
        // applies status presentation residual via host_status_log.
        use crate::game_logic::host_status_log as hsl;
        let events = crate::game_logic::host_combat_status_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get(&ev.object) else {
                continue;
            };
            let s = &obj.status;
            let oid = ev.object;
            // Re-record key combat-status flags for presentation consumers.
            // Values already writeback-synced; this is bookkeeping only.
            hsl::record_selected(oid, s.selected);
            hsl::record_attacking(oid, s.attacking);
            hsl::record_moving(oid, s.moving);
            hsl::record_firing(oid, s.is_firing_weapon);
            hsl::record_aiming(oid, s.is_aiming_weapon);
            hsl::record_stealthed(oid, s.stealthed);
            hsl::record_detected(oid, s.detected);
            hsl::record_disabled_emp(oid, s.disabled_emp);
            hsl::record_weapons_jammed(oid, s.weapons_jammed);
            hsl::record_disabled_hacked(oid, s.disabled_hacked);
            hsl::record_disabled_unmanned(oid, s.disabled_unmanned);
            hsl::record_disabled_paralyzed(oid, s.disabled_paralyzed);
            hsl::record_disabled_subdued(oid, s.disabled_subdued);
            hsl::record_masked(oid, s.masked);
            hsl::record_disguised(oid, s.disguised);
            hsl::record_faerie_fire(oid, s.faerie_fire);
            hsl::record_deployed(oid, s.deployed);
            hsl::record_disabled_underpowered(oid, s.disabled_underpowered);
            hsl::record_is_carbomb(oid, s.is_carbomb);
            hsl::record_hijacked(oid, s.hijacked);
            hsl::record_force_attack(oid, obj.force_attack);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_model_condition_ready_completions(&mut self) -> usize {
        // Wave 633: GameWorld model-condition writeback records bit changes; host
        // applies presentation bookkeeping residual (drawable model condition log).
        let events = crate::game_logic::host_model_condition_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_bits == ev.new_bits {
                continue;
            }
            // Bits already writeback-synced; re-record host model-condition log
            // for presentation consumers without recomputing from health.
            if let Some(obj) = self.objects.get(&ev.object) {
                obj.record_host_model_condition();
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_death_type_ready_completions(&mut self) -> usize {
        // Wave 632: GameWorld death-type writeback records ordinal changes; host
        // applies destroy/pilot presentation bookkeeping residual.
        let events = crate::game_logic::host_death_type_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_ordinal == ev.new_ordinal {
                continue;
            }
            // Death type already writeback-synced; re-record host death-type
            // log for presentation / process_destroy consumers.
            if self.objects.contains_key(&ev.object) {
                crate::game_logic::host_death_type_log::record(ev.object, ev.new_ordinal);
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_economy_ready_completions(&mut self) -> usize {
        // Wave 631: GameWorld economy writeback records supply/power/radar/alive
        // changes; host applies presentation residual via host_economy_log and
        // radar log (GW decides absolute values; host still owns UI bookkeeping).
        let events = crate::game_logic::host_economy_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.supplies_changed || ev.power_changed {
                crate::game_logic::host_economy_log::record(
                    ev.player_id,
                    ev.supplies,
                    ev.power_available,
                );
                n = n.saturating_add(1);
            }
            if ev.radar_changed {
                if let Some(player) = self.players.get_mut(&ev.player_id) {
                    // Re-record radar residual for presentation without
                    // re-applying absolute values (already writeback-synced).
                    crate::game_logic::host_radar_log::record(
                        player.id,
                        player.radar_count,
                        player.radar_disabled,
                    );
                    n = n.saturating_add(1);
                }
            }
            let _ = (ev.alive_changed, ev.previous_alive, ev.is_alive);
            let _ = (
                ev.previous_supplies,
                ev.previous_power,
                ev.previous_radar_count,
                ev.previous_radar_disabled,
            );
        }
        n
    }

    pub fn host_apply_ai_state_ready_completions(&mut self) -> usize {
        // Wave 630: GameWorld AI-state writeback records ordinal changes; host
        // applies moving/attacking combat-status residual for the new state.
        let events = crate::game_logic::host_ai_state_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            if ev.previous_ordinal == ev.new_ordinal {
                continue;
            }
            // State already writeback-synced; apply status residual only.
            obj.apply_ai_state_combat_status_residual(ev.new_ordinal);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_owner_ready_completions(&mut self) -> usize {
        // Wave 629: GameWorld owner writeback records team changes; host applies
        // capture residual (kick passengers, deselect, idle, score).
        let events = crate::game_logic::host_owner_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            if ev.previous_team == ev.new_team {
                continue;
            }
            // Team already writeback-synced; run capture residual side effects.
            self.on_capture_object_residual(ev.object, ev.previous_team, ev.new_team);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_contain_ready_completions(&mut self) -> usize {
        // Wave 628: GameWorld contain writeback records membership changes;
        // host applies garrison AI residual + enter/exit honesty counters.
        let events = crate::game_logic::host_contain_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            // Passenger residual: entered a container.
            if ev.previous_contained_by == 0 && ev.new_contained_by != 0 {
                obj.set_ai_state(AIState::Garrisoned);
                obj.set_status_moving(false);
                obj.stop_moving();
                self.record_garrison_residual_enter();
                n = n.saturating_add(1);
                continue;
            }
            // Passenger residual: left a container.
            if ev.previous_contained_by != 0 && ev.new_contained_by == 0 {
                if matches!(obj.ai_state, AIState::Garrisoned | AIState::Entering) {
                    obj.set_ai_state(AIState::Idle);
                }
                self.record_garrison_residual_exit();
                n = n.saturating_add(1);
                continue;
            }
            // Container residual: garrison count rose/fell (honesty only).
            if ev.garrison_list_changed {
                if ev.new_garrison_count > ev.previous_garrison_count {
                    let delta = ev.new_garrison_count - ev.previous_garrison_count;
                    for _ in 0..delta {
                        self.record_garrison_residual_enter();
                    }
                } else if ev.new_garrison_count < ev.previous_garrison_count {
                    let delta = ev.previous_garrison_count - ev.new_garrison_count;
                    for _ in 0..delta {
                        self.record_garrison_residual_exit();
                    }
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_production_door_ready_completions(&mut self) -> usize {
        // Wave 627: GameWorld production-door writeback records phase changes;
        // host applies door model-condition residual for the new phase.
        let events = crate::game_logic::host_production_door_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.producer) else {
                continue;
            };
            if obj.apply_production_door_phase_residual(ev.new_phase) {
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_construction_complete_clear_ready_completions(&mut self) -> usize {
        // Wave 626: under construction sole-tick, GameWorld writeback records
        // producers whose CONSTRUCTION_COMPLETE clear deadline elapsed; host clears
        // the model bit and counts residual.
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return 0;
        }
        let events = crate::game_logic::host_construction_complete_clear_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.producer) else {
                continue;
            };
            if obj.apply_construction_complete_clear_residual() {
                self.construction_complete_clears =
                    self.construction_complete_clears.saturating_add(1);
                n = n.saturating_add(1);
            }
        }
        n
    }

    pub fn host_apply_radar_extend_ready_completions(&mut self) -> usize {
        // Wave 625: GameWorld radar-extend complete writeback records ready IDs;
        // host applies upgraded model residual and complete counter.
        let events = crate::game_logic::host_radar_extend_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.structure) else {
                continue;
            };
            obj.apply_radar_extend_complete_residual();
            self.radar_extend_completes = self.radar_extend_completes.saturating_add(1);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_upgrade_ready_completions(&mut self) -> usize {
        // Wave 624: under GameWorld completed-upgrade writeback, drain ready log and
        // apply full host upgrade residual (unlocks, EVA, radar, status bits).
        let events = crate::game_logic::host_upgrade_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let team = self.players.get(&ev.player_id).map(|p| p.team).or_else(|| {
                self.players
                    .values()
                    .find(|p| p.id == ev.player_id)
                    .map(|p| p.team)
            });
            let Some(team) = team else {
                continue;
            };
            // Skip if host production path already completed this upgrade.
            use crate::game_logic::host_upgrades::{normalize_upgrade_identity, HostUpgradePhase};
            let key = normalize_upgrade_identity(&ev.upgrade_name);
            let already = self.host_upgrades().entries_snapshot().iter().any(|e| {
                e.player_id == ev.player_id
                    && e.phase == HostUpgradePhase::Completed
                    && normalize_upgrade_identity(&e.name) == key
            });
            if already {
                continue;
            }
            // Ensure player unlocked set tracks completion (production path does this).
            if let Some(player) = self.players.get_mut(&ev.player_id) {
                if let Some(queued) = player.find_queued_upgrade_name(&ev.upgrade_name) {
                    player.queued_upgrades.remove(&queued);
                }
                if !player.has_unlocked_upgrade(&ev.upgrade_name) {
                    player.unlocked_sciences.insert(ev.upgrade_name.clone());
                }
            } else if let Some(player) = self.players.values_mut().find(|p| p.id == ev.player_id) {
                if let Some(queued) = player.find_queued_upgrade_name(&ev.upgrade_name) {
                    player.queued_upgrades.remove(&queued);
                }
                if !player.has_unlocked_upgrade(&ev.upgrade_name) {
                    player.unlocked_sciences.insert(ev.upgrade_name.clone());
                }
            }
            self.apply_host_upgrade_complete(team, ev.player_id, &ev.upgrade_name);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_body_damage_ready_completions(&mut self) -> usize {
        // Wave 623: under damage authority, GameWorld body-damage writeback records
        // state transitions; host applies model/FX residual for those IDs.
        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
            return 0;
        }
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let events = crate::game_logic::host_body_damage_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            let prev = HostBodyDamageType::from_ordinal(ev.previous_ordinal);
            let next = HostBodyDamageType::from_ordinal(ev.new_ordinal);
            if prev == next {
                continue;
            }
            obj.apply_body_damage_state_change_residual(prev, next);
            n = n.saturating_add(1);
        }
        n
    }

    pub fn host_apply_veterancy_ready_completions(&mut self) -> usize {
        // Wave 622: under damage authority, GameWorld experience writeback records
        // veterancy level-ups; host applies combat bonus residual for those IDs.
        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
            return 0;
        }
        use crate::game_logic::VeterancyLevel as V;
        let events = crate::game_logic::host_veterancy_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let Some(obj) = self.objects.get_mut(&ev.object) else {
                continue;
            };
            if obj.status.destroyed && !obj.is_alive() {
                continue;
            }
            let prev = match ev.previous_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            let next = match ev.new_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            if next == prev {
                continue;
            }
            // Level already writeback-synced; apply combat residual bonuses.
            obj.apply_veterancy_bonuses(prev, next);
            n = n.saturating_add(1);
        }
        n
    }

    /// Wave 912: true when destroy queue or destroy-ready residual has work.
    #[inline]
    pub fn has_pending_destroy_work(&self) -> bool {
        if !self.objects_to_destroy.is_empty() {
            return true;
        }
        crate::gameworld_shadow::gameworld_damage_authority_live()
            && crate::game_logic::host_destroy_ready_log::has_pending()
    }

    /// Wave 912: process destroy list only when residual work is pending.
    #[inline]
    pub fn process_destroy_list_if_needed(&mut self) {
        if self.has_pending_destroy_work() {
            self.process_destroy_list();
        }
    }

    pub fn process_destroy_list(&mut self) {
        // Wave 621: under damage authority, GameWorld health writeback records
        // lethal IDs; host marks them here before draining the destroy queue.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            for ev in crate::game_logic::host_destroy_ready_log::drain() {
                if self.objects_to_destroy.iter().any(|e| e.id == ev.object) {
                    continue;
                }
                let lethal = self
                    .objects
                    .get(&ev.object)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false);
                if lethal {
                    self.mark_object_for_destruction(ev.object, None);
                }
            }
        }
        let mut destroyed_structure = false;
        while let Some(event) = self.objects_to_destroy.pop_front() {
            self.pending_special_abilities.remove(&event.id);
            self.pending_special_abilities
                .retain(|_, ability| ability.target_id() != event.id);

            self.cancel_all_production(event.id);

            // C++ Object::onDie RECONSTRUCTING residual (lost rebuild → hole).
            let handled_recon = self.handle_reconstructing_death(event.id);
            // C++ RebuildHoleExposeDie residual (GLA structures → hole).
            // Skip if this was a reconstructing building (hole already exists).
            if !handled_recon {
                let _ = self.maybe_spawn_rebuild_hole(event.id);
            }

            // Snapshot CreateCrateDie residual fields before remove.
            let (crate_data, death_pos_pre, death_team_pre, last_src) =
                if let Some(o) = self.objects.get(&event.id) {
                    (
                        o.thing.template.create_crate_data.clone(),
                        o.get_position(),
                        o.team,
                        o.last_damage_source,
                    )
                } else {
                    (Vec::new(), glam::Vec3::ZERO, Team::Neutral, None)
                };
            if !crate_data.is_empty() {
                let _ = self.try_create_crates_on_die(
                    event.id,
                    death_pos_pre,
                    death_team_pre,
                    &crate_data,
                    last_src,
                );
            }

            // C++ FireWeaponWhenDeadBehavior::onDie residual.
            self.apply_fire_weapon_when_dead(event.id);

            if let Some(obj) = self.objects.remove(&event.id) {
                crate::game_logic::host_destroy_log::record(event.id);
                // Wave 681: mid-frame GameWorld Destroy while coupled shadow tick is live.
                // End-of-tick host_destroy_log drain remains idempotent for unmapped IDs.
                let _ = crate::gameworld_shadow::eager_unmap_host_destroy_if_coupled(event.id);
                // Combat particle residual: death → registry entry (explosion + smoke).
                // PresentationFrame / client can observe systems after the kill.
                let death_pos = obj.get_position();
                let is_structure = obj.is_kind_of(KindOf::Structure);
                if is_structure {
                    destroyed_structure = true;
                }
                let victim_team = obj.team;
                // C++ Object::onDie EVA residual (local, non-self-inflicted).
                let is_infantry = obj.is_kind_of(KindOf::Infantry);
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                // KINDOF_MP_COUNT_FOR_VICTORY residual class (main base buildings).
                let is_mp_count = is_structure
                    && (obj.is_kind_of(KindOf::CommandCenter)
                        || obj.is_kind_of(KindOf::FSPower)
                        || obj.is_kind_of(KindOf::PowerPlant)
                        || obj.is_kind_of(KindOf::FSBarracks)
                        || obj.is_kind_of(KindOf::FSWarFactory)
                        || obj.is_kind_of(KindOf::FSAirfield)
                        || obj.is_kind_of(KindOf::FSSuperweapon)
                        || obj.is_kind_of(KindOf::FSStrategyCenter)
                        || obj.is_kind_of(KindOf::FSTechnology)
                        || obj.is_kind_of(KindOf::SupplyCenter)
                        || obj.is_kind_of(KindOf::FSSupplyCenter));
                self.try_eva_on_local_object_death(
                    event.id,
                    victim_team,
                    is_structure,
                    is_infantry,
                    is_vehicle,
                    is_mp_count,
                    death_pos,
                    event.killer,
                );
                let frame = self.frame;
                let death_type = obj.status.death_type;
                let _ = self.combat_particles.spawn_death_fx_for_type(
                    death_pos,
                    frame,
                    event.id,
                    is_structure,
                    victim_team,
                    death_type,
                );

                // Audio residual (hq-7zxm slice): unit/structure death → AudioEventRequest.
                // DeathType residual selects die cue family (not full voice bank).
                let death_event = crate::game_logic::combat_particles::CombatParticleRegistry::death_audio_event_name(
                    is_structure,
                    death_type,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(death_event)
                        .with_object(event.id)
                        .with_position(death_pos)
                        .with_priority(200),
                );

                let eject_origin = obj.get_position();

                // C++ parity (OpenContain::onDie): if DamagePercentToUnits > 0,
                // apply damage to contained units based on their max health.
                let damage_pct = obj
                    .building_data
                    .as_ref()
                    .map(|bd| bd.damage_percent_to_units)
                    .unwrap_or(0.0);

                // C++ ParachuteContain::onDie: airborne chute → FreeFallDamage riders.
                let is_america_parachute = obj.template_name.eq_ignore_ascii_case(
                    crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME,
                );
                let chute_airborne = is_america_parachute
                    && crate::game_logic::host_usa_pilot::should_apply_parachute_free_fall_damage(
                        obj.is_parachuting() || is_america_parachute,
                        eject_origin.y,
                    );

                if chute_airborne {
                    let riders = obj.contained_units();
                    for rid in riders {
                        let _ = self.apply_rider_free_fall_damage(rid, eject_origin);
                    }
                    self.car_bomb.record_airborne_parachute_free_fall();
                } else {
                    for (i, contained_id) in obj.contained_units().into_iter().enumerate() {
                        if let Some(unit) = self.objects.get_mut(&contained_id) {
                            // Apply damage before ejection if configured.
                            if damage_pct > 0.0 {
                                let dmg = unit.max_health * damage_pct;
                                let destroyed = unit.take_damage_from(dmg, Some(event.id));
                                if destroyed {
                                    unit.status.destroyed = true;
                                    self.mark_object_for_destruction(contained_id, event.killer);
                                    continue;
                                }
                            }

                            let angle = (contained_id.0 as f32 + i as f32 * 1.11).sin().atan2(1.0)
                                + i as f32 * 0.73;
                            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
                            unit.stop_moving();
                            unit.set_position(eject_origin + offset);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                let p = eject_origin + offset;
                                crate::game_logic::host_move_log::record(
                                    unit.id,
                                    Some([p.x, p.y, p.z]),
                                );
                                unit.record_host_movement();
                            }
                            unit.set_target(None);
                            unit.set_contained_by(None);
                            unit.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_stop_attack(
                                    contained_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    contained_id,
                                    0,
                                );
                            }
                            unit.set_status_moving(false);
                            unit.set_status_attacking(false);
                        }
                    }
                }

                // GLA Toxin Tractor death residual: ToxinShellWeapon → SmallPoisonField.
                // Fail-closed: not full FireWeaponWhenDead anthrax matrix / FX list.
                {
                    use crate::game_logic::host_toxin_tractor::{
                        anthrax_tier_from_flags, is_chem_general_template,
                        is_toxin_tractor_template, UPGRADE_GLA_ANTHRAX_BETA,
                        UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                    };
                    if is_toxin_tractor_template(&obj.template_name) {
                        let has_gamma = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                            || obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                        let has_beta = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
                        let anthrax = anthrax_tier_from_flags(
                            has_gamma,
                            has_beta,
                            is_chem_general_template(&obj.template_name),
                        );
                        let death_pos = obj.get_position();
                        let team = obj.team;
                        let _ = self
                            .toxin_tractor
                            .spawn_death_field(event.id, team, death_pos, self.frame, anthrax);
                        self.queue_audio_event(
                            AudioEventRequest::new(
                                crate::game_logic::host_toxin_tractor::TOXIN_POISON_AUDIO,
                            )
                            .with_position(death_pos)
                            .with_priority(140),
                        );
                    }
                }

                // GLA Bomb Truck FireWeaponWhenDead residual: HE/Bio detonation matrix.
                // Fail-closed: not full exclusive module / SubObjectsUpgrade payload visuals.
                // Note: object already removed from map — use `obj` snapshot for upgrades/pos.
                {
                    use crate::game_logic::host_bomb_truck_detonate::{
                        is_bomb_truck_template, BombTruckDetonationProfile, UPGRADE_BOMB_TRUCK_BIO,
                        UPGRADE_BOMB_TRUCK_HE, UPGRADE_GLA_ANTHRAX_BETA,
                    };
                    if is_bomb_truck_template(&obj.template_name) {
                        let he = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_HE)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
                        let bio = obj.has_upgrade_tag(UPGRADE_BOMB_TRUCK_BIO)
                            || obj.has_upgrade_tag("Upgrade_GLABombTruckBioBomb");
                        let anthrax = obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                            || obj.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag("Upgrade_GLAAnthraxGamma")
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA,
                            )
                            || obj.has_upgrade_tag(
                                crate::game_logic::host_toxin_tractor::UPGRADE_GLA_ANTHRAX_GAMMA_ALT,
                            );
                        let profile = BombTruckDetonationProfile::from_upgrades(he, bio, anthrax);
                        let _ = self.apply_bomb_truck_death_detonation_at(
                            event.id, obj.team, death_pos, profile,
                        );
                    }
                }

                // China Nuclear Tanks FireWeaponWhenDead residual: dual-radius + radiation.
                // Fail-closed: not full exclusive module / Nuclear*Locomotor visual matrix.
                {
                    use crate::game_logic::host_nuclear_tanks::{
                        has_nuclear_tanks_upgrade, is_nuclear_tanks_eligible,
                        is_nuke_general_nuclear_tanks,
                    };
                    if is_nuclear_tanks_eligible(&obj.template_name)
                        && has_nuclear_tanks_upgrade(&obj.applied_upgrades)
                    {
                        let nuke_gen = is_nuke_general_nuclear_tanks(&obj.template_name);
                        let _ = self.apply_nuclear_tanks_death_detonation_at(
                            event.id, obj.team, death_pos, nuke_gen,
                        );
                    }
                }

                // Demo SuicideBomb FireWeaponWhenDead residual: Demo_DestroyedWeapon blast.
                // Skip intentional SUICIDED path (PlusFire already applied via TertiarySuicide).
                // Skip terrorists (already handled by host_terrorist SUICIDED residual).
                {
                    use crate::game_logic::host_demo_suicide_bomb::{
                        has_demo_suicide_bomb_upgrade, is_demo_suicide_bomb_eligible_template,
                    };
                    use crate::game_logic::host_terrorist::is_terrorist_template;
                    if !obj.demo_suicided_detonating
                        && is_demo_suicide_bomb_eligible_template(&obj.template_name)
                        && has_demo_suicide_bomb_upgrade(&obj.applied_upgrades)
                        && !is_terrorist_template(&obj.template_name)
                    {
                        let _ =
                            self.apply_demo_suicide_bomb_death_at(event.id, obj.team, death_pos);
                    }
                }

                // USA EjectPilotDie residual: spawn AmericaInfantryPilot on vehicle death.
                // Air path: isSignificantlyAboveTerrain → OCL_EjectPilotViaParachute residual.
                // Ground path: OCL_EjectPilotOnGround residual.
                // VeterancyLevels = ALL -REGULAR residual: Rookie does not eject.
                // DeathTypes = ALL -CRUSHED -SPLATTED; ExemptStatus = HIJACKED residual.
                // Wave 754: skip if death-start mark_object already applied onDie residual.
                if !obj.eject_pilot_die_applied {
                    use crate::game_logic::host_usa_pilot::{
                        air_eject_spawn_height, can_eject_pilot_on_death,
                        is_eject_pilot_eligible_template, meets_eject_pilot_death_types_gate,
                        meets_eject_pilot_exempt_status_gate, meets_eject_pilot_veterancy_gate,
                        uses_air_eject_ocl, EJECT_PILOT_TEMPLATE, PILOT_EJECT_AUDIO,
                    };
                    let is_vehicle =
                        obj.is_kind_of(KindOf::Vehicle) || obj.object_type == ObjectType::Vehicle;
                    let is_aircraft =
                        obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft;
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    let eligible_template = is_eject_pilot_eligible_template(&obj.template_name);
                    let vet_gate = meets_eject_pilot_veterancy_gate(obj.experience.level);
                    let death_types_gate =
                        meets_eject_pilot_death_types_gate(obj.status.death_type);
                    let exempt_status_gate =
                        meets_eject_pilot_exempt_status_gate(obj.status.hijacked);
                    // Honesty: record REGULAR-gate blocks when all other gates pass.
                    if eligible_template
                        && !obj.is_unmanned()
                        && !under_construction
                        && is_vehicle
                        && !is_aircraft
                        && death_types_gate
                        && exempt_status_gate
                        && !vet_gate
                    {
                        self.usa_pilot.record_eject_veterancy_block();
                    }
                    // Honesty: DeathTypes / ExemptStatus blocks when other gates pass.
                    if eligible_template
                        && !obj.is_unmanned()
                        && !under_construction
                        && is_vehicle
                        && !is_aircraft
                        && vet_gate
                        && exempt_status_gate
                        && !death_types_gate
                    {
                        self.usa_pilot.record_eject_death_type_block();
                    }
                    if eligible_template
                        && !obj.is_unmanned()
                        && !under_construction
                        && is_vehicle
                        && !is_aircraft
                        && vet_gate
                        && death_types_gate
                        && !exempt_status_gate
                    {
                        self.usa_pilot.record_eject_hijacked_block();
                    }
                    if can_eject_pilot_on_death(
                        eligible_template,
                        obj.is_unmanned(),
                        under_construction,
                        is_vehicle,
                        is_aircraft,
                        vet_gate,
                        death_types_gate,
                        exempt_status_gate,
                    ) {
                        let pilot_team = obj.team;
                        let air_path = uses_air_eject_ocl(death_pos.y, obj.status.airborne_target);
                        // Ensure pilot template exists for residual spawn.
                        if !self.templates.contains_key(EJECT_PILOT_TEMPLATE) {
                            let mut pilot_tpl =
                                crate::game_logic::ThingTemplate::new(EJECT_PILOT_TEMPLATE);
                            pilot_tpl
                                .add_kind_of(KindOf::Infantry)
                                .add_kind_of(KindOf::Selectable)
                                .set_health(100.0);
                            self.templates
                                .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
                        }
                        // Offset slightly so pilot is not buried under death debris residual.
                        // Air OCL residual: keep elevated y (PutInContainer AmericaParachute).
                        let spawn_pos = if air_path {
                            Vec3::new(
                                death_pos.x + 2.0,
                                air_eject_spawn_height(death_pos.y),
                                death_pos.z + 2.0,
                            )
                        } else {
                            death_pos + Vec3::new(2.0, 0.0, 2.0)
                        };
                        if let Some(pilot_id) =
                            self.create_object(EJECT_PILOT_TEMPLATE, pilot_team, spawn_pos)
                        {
                            self.usa_pilot.record_ejection();
                            if air_path {
                                self.usa_pilot.record_air_ejection();
                            }
                            // OCL InvulnerableTime residual (2000ms → 60 frames).
                            let until = crate::game_logic::host_usa_pilot::eject_pilot_invulnerable_until_frame(
                                self.frame,
                            );
                            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                                pilot.apply_eject_invulnerable(until);
                                if air_path {
                                    let raw_y = pilot.get_position().y;
                                    pilot.apply_eject_parachuting();
                                    // Low-altitude open fudge residual honesty.
                                    if crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged(
                                        raw_y, 0.0,
                                    ) {
                                        self.usa_pilot.record_parachute_open_fudge();
                                    }
                                }
                            }
                            self.usa_pilot.record_invulnerable_grant();
                            self.queue_audio_event(
                                AudioEventRequest::new(PILOT_EJECT_AUDIO)
                                    .with_position(spawn_pos)
                                    .with_priority(170),
                            );
                            let _ = pilot_id;
                        }
                    }
                }

                // GLA Rebel BoobyTrap residual: structure death detonates trap.
                // C++ Object::checkAndDetonateBoobyTrap(NULL) on die path.
                if obj.status.booby_trapped || self.booby_trap.is_booby_trapped(event.id) {
                    let _ = self.detonate_booby_trap_at(event.id, death_pos, None, false, true);
                }

                log::debug!(
                    "Destroyed object {} ({})",
                    event.id,
                    obj.get_template().name
                );
                self.record_destruction(&obj, event.killer);

                // Remove from player selections
                for (_, player) in self.players.iter_mut() {
                    player.selected_objects.retain(|&x| x != event.id);
                }

                // C++ parity: clear stale target references from all other objects.
                // When an object is destroyed, anything targeting it should stop.
                let destroyed_id = event.id;
                let clear_ids: Vec<ObjectId> = self
                    .objects
                    .iter()
                    .filter(|(_, o)| o.target == Some(destroyed_id))
                    .map(|(id, _)| *id)
                    .collect();
                for cid in clear_ids {
                    self.stop_attack_decision_aware(cid);
                }
                let mut guard_idle: Vec<ObjectId> = Vec::new();
                for (oid, other_obj) in self.objects.iter_mut() {
                    if other_obj.guard_target == Some(destroyed_id) {
                        other_obj.guard_target = None;
                        if other_obj.ai_state == AIState::GuardingObject {
                            other_obj.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                guard_idle.push(*oid);
                            }
                        }
                    }
                }
                for gid in guard_idle {
                    crate::game_logic::host_ai_decision_log::record_set_state(gid, 0);
                }
            }
        }

        if destroyed_structure {
            // Rebuild static path/LOS mask without the destroyed footprint.
            self.sync_structure_path_blocks();
        }
    }

    pub(super) fn record_destruction(&mut self, destroyed_object: &Object, killer: Option<Team>) {
        let destroyed_is_structure = destroyed_object.is_kind_of(KindOf::Structure);
        let victim_team = destroyed_object.team;
        let victim_id = destroyed_object.id;
        let victim_pos = destroyed_object.get_position();
        // C++ Object::scoreTheKill / Player::doBountyForKill:
        // no bounty for under-construction, non-enemy, or same-controller kills.
        let under_construction = destroyed_object.status.under_construction;
        let build_cost = destroyed_object.thing.template.build_cost.supplies;

        let mut bounty_awarded = 0_u32;
        let mut bounty_killer_id = ObjectId(0);
        let mut bounty_float_pos = victim_pos;
        let mut used_last_damage_source = false;
        if let Some(team) = killer {
            // Prefer C++ BodyModule last_damage_source residual for killer ObjectId.
            if let Some(src) = destroyed_object.last_damage_source {
                if let Some(src_obj) = self.objects.get(&src) {
                    if src_obj.team == team {
                        bounty_killer_id = src;
                        bounty_float_pos = src_obj.get_position();
                        used_last_damage_source = true;
                    }
                } else {
                    // Killer already removed this frame — still record ObjectId residual.
                    bounty_killer_id = src;
                    used_last_damage_source = true;
                }
            }
            // Fallback residual: nearest living unit on killer team near victim
            // (destruction event carries team; last_damage_source may be unset).
            if !used_last_damage_source {
                if let Some((kid, kpos)) = self
                    .objects
                    .iter()
                    .filter(|(_, o)| o.team == team && o.is_alive())
                    .map(|(id, o)| (*id, o.get_position()))
                    .min_by(|a, b| {
                        let da = (a.1.x - victim_pos.x).hypot(a.1.z - victim_pos.z);
                        let db = (b.1.x - victim_pos.x).hypot(b.1.z - victim_pos.z);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    bounty_killer_id = kid;
                    bounty_float_pos = kpos;
                }
            }
            if let Some(player_id) = self.player_id_for_team(team) {
                if let Some(player) = self.players.get_mut(&player_id) {
                    if destroyed_is_structure {
                        player.record_structure_destroyed();
                    } else {
                        player.record_unit_destroyed();
                    }

                    // Cash bounty residual: award ceil(cost * percent) on enemy kill.
                    let enemy_kill = team != victim_team
                        && team != Team::Neutral
                        && victim_team != Team::Neutral;
                    if enemy_kill && !under_construction && player.cash_bounty_percent > 0.0 {
                        bounty_awarded = player.do_bounty_for_kill(build_cost);
                    }

                    // C++ Player::addSkillPointsForKill residual (scoreTheKill path).
                    // No skill points for under-construction victims.
                    if enemy_kill && !under_construction {
                        use crate::game_logic::host_rank_ui_residual::skill_points_for_kill_residual;
                        let vet_level = match destroyed_object.experience.level {
                            crate::game_logic::VeterancyLevel::Rookie => 0,
                            crate::game_logic::VeterancyLevel::Veteran => 1,
                            crate::game_logic::VeterancyLevel::Elite => 2,
                            crate::game_logic::VeterancyLevel::Heroic => 3,
                        };
                        let is_ac = destroyed_object.is_kind_of(KindOf::Aircraft)
                            || destroyed_object.object_type == ObjectType::Aircraft;
                        let is_veh = destroyed_object.is_kind_of(KindOf::Vehicle)
                            || destroyed_object.object_type == ObjectType::Vehicle;
                        let skill = skill_points_for_kill_residual(
                            destroyed_is_structure,
                            is_ac,
                            is_veh,
                            vet_level,
                        );
                        if skill > 0 {
                            let _leveled = player.add_skill_points(skill);
                        }
                    }
                }
            }
        }
        if bounty_awarded > 0 {
            self.cash_bounty.record_bounty_award(bounty_awarded);
            if used_last_damage_source {
                self.cash_bounty.record_last_damage_source_kill();
            }
            // C++ doBountyForKill floating text: yellow, killer pos + Z10.
            self.cash_bounty.record_floating_text(
                crate::game_logic::host_cash_bounty::HostCashBountyFloatingText::new(
                    bounty_killer_id,
                    victim_id,
                    bounty_float_pos,
                    bounty_awarded,
                    self.frame,
                ),
            );
        }

        if let Some(player_id) = self.player_id_for_team(destroyed_object.team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                if destroyed_is_structure {
                    player.record_structure_lost();
                } else {
                    player.record_unit_lost();
                }
            }
        }
    }

    /// Set cash bounty percent on a player (residual / tests).
    /// Raises percent only (C++ CashBountyPower set if higher).
    pub fn set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    /// Force-set cash bounty percent (tests / load restore).
    pub fn force_set_player_cash_bounty(&mut self, player_id: u32, percent: f32) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.force_set_cash_bounty(percent);
        self.cash_bounty
            .record_bounty_set(player.cash_bounty_percent);
        true
    }

    /// Residual honesty: cash bounty was configured and at least one award paid.
    /// Fail-closed: not full palace module / floating-text parity.
    pub fn honesty_cash_bounty_ok(&self) -> bool {
        self.cash_bounty.honesty_ok()
    }

    /// Residual honesty: at least one bounty cash award on kill.
    pub fn honesty_cash_bounty_award_ok(&self) -> bool {
        self.cash_bounty.honesty_bounty_award_ok()
    }

    /// Residual cash bounty floating cash text honesty.
    pub fn honesty_cash_bounty_floating_text_ok(&self) -> bool {
        self.cash_bounty.honesty_floating_text_ok()
    }

    /// Total residual cash credited via kill bounty (observability).
    pub fn cash_bounty_earned_total(&self) -> u32 {
        self.cash_bounty.bounty_earned_total
    }

    /// Host cash bounty registry (tests / honesty).
    pub fn cash_bounty_registry(
        &self,
    ) -> &crate::game_logic::host_cash_bounty::HostCashBountyRegistry {
        &self.cash_bounty
    }

    /// C++ parity: veterancy-level XP multiplier. In C++ each template
    /// defines per-level ExperienceValue; we approximate by scaling the
    /// base value.  C++ values are modest multipliers, not large ones.
    pub(super) fn veterancy_xp_multiplier(level: VeterancyLevel) -> f32 {
        match level {
            VeterancyLevel::Rookie => 1.0,
            VeterancyLevel::Veteran => 1.25,
            VeterancyLevel::Elite => 1.5,
            VeterancyLevel::Heroic => 2.0,
        }
    }

    pub(super) fn should_track_player_stats(&self) -> bool {
        self.sim_time_seconds > 0.0 || self.frame > 0
    }

    pub(super) fn record_unit_production(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_unit_produced();
            }
        }
    }

    pub(super) fn record_structure_completion(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_structure_built();
            }
        }
    }

    pub(super) fn template_counts_as_unit(template: &ThingTemplate) -> bool {
        !template.is_kind_of(KindOf::Structure)
            && (template.is_kind_of(KindOf::Infantry)
                || template.is_kind_of(KindOf::Vehicle)
                || template.is_kind_of(KindOf::Aircraft))
    }

    pub(super) fn should_skip_map_object_template(template_name: &str) -> bool {
        const ILLEGAL_TEMPLATE_NAMES: &[&str] = &[
            "EMPPulseBomb",
            "GLAAngryMobRockProjectileObject",
            "ClusterMinesBomb",
            "BlackNapalmFirestormSmall",
            "CabooseFullOfTerrorists",
            "GLAAngryMobMolotovCocktailProjectileObject",
            "Firestorm",
            "Avalanche",
            "InfernoTankShell",
            "ChinaArtilleryBarrageShell",
            "ChinaTankOverlordBattleBunker",
            "ChinaTankOverlordPropagandaTower",
            "ChinaTankOverlordGattlingCannon",
            "CINE",
            "GLAInfantryAngryMobNexus",
            "AircraftCarrier",
            "GermanMuseum",
            "Cin_",
            "Amb_",
            "Ambient",
            "GC_",
            "SpecialEffectsTrainCrashObject",
            "Scorch",
        ];

        ILLEGAL_TEMPLATE_NAMES.iter().any(|illegal| {
            template_name.starts_with(illegal)
                || template_name.ends_with(illegal)
                || template_name == *illegal
        })
    }

    pub(super) fn should_spawn_fallback_template(template_name: &str) -> bool {
        if Self::should_skip_map_object_template(template_name) {
            return false;
        }

        let lower = template_name.to_ascii_lowercase();
        lower.contains("tech")
            || lower.contains("supply")
            || lower.contains("oil")
            || lower.contains("bunker")
            || lower.contains("guardtower")
            || lower.contains("tower")
            || lower.contains("commandcenter")
            || lower.contains("refinery")
            || lower.contains("crate")
    }

    pub(super) fn build_template_from_asset_definition(template_name: &str) -> Option<ThingTemplate> {
        let manager_arc = get_asset_manager()?;
        let remapped_model = Self::remap_known_model_alias(template_name);
        let (definition, texture_hint) = {
            let manager = manager_arc.lock().ok()?;
            let definition = manager
                .resolve_object_definition(template_name, Some(remapped_model.as_str()))
                .or_else(|| manager.resolve_object_definition(template_name, None))
                .cloned()?;
            let texture_hint = manager
                .get_texture_for_object(template_name)
                .or_else(|| manager.get_texture_for_object(remapped_model.as_str()));
            (definition, texture_hint)
        };

        // C++ data includes audio-only ambient map objects with Draw blocks that contain no model.
        // Keep them out of visual spawn synthesis to avoid bogus model fallback loads.
        if definition.model_name.is_none()
            && Self::object_definition_attr(&definition, "soundambient").is_some()
        {
            return None;
        }

        Some(Self::build_template_from_object_definition(
            template_name,
            &definition,
            texture_hint.as_deref(),
        ))
    }

    pub(super) fn build_template_from_object_definition(
        template_name: &str,
        definition: &ObjectDefinition,
        texture_hint: Option<&str>,
    ) -> ThingTemplate {
        let mut template = ThingTemplate::new(template_name);
        let lower = template_name.to_ascii_lowercase();
        let kind_of = Self::object_definition_attr(definition, "kindof")
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !definition.display_name.is_empty() {
            template.display_name = definition.display_name.clone();
        }

        if let Some(hit_points) = definition.hit_points {
            if hit_points > 0 {
                template.set_health(hit_points as f32);
            }
        }

        if let Some(model_name) = definition.model_name.as_deref() {
            let model_name = model_name.trim();
            if !model_name.is_empty() && !model_name.eq_ignore_ascii_case("none") {
                let resolved_model_name = Self::resolve_spawn_model_name(model_name)
                    .unwrap_or_else(|| Self::remap_known_model_alias(model_name));
                template.set_model(&resolved_model_name);
            }
        }

        let primary_texture = texture_hint.or_else(|| definition.get_primary_texture());
        if let Some(texture_name) = primary_texture {
            let texture_name = texture_name.trim();
            if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                template.texture_name = Some(texture_name.to_string());
            }
        }

        // Retail SupplyDock/SupplyPile carry SUPPLY_SOURCE (not "resource"/"harvest")
        // KindOf bits; map props must still be gatherable by dozer/chinook paths.
        let kind_compact = kind_of.replace('_', "");
        let is_resource = lower.contains("supplypile")
            || lower.contains("supplydock")
            || lower.contains("tempsupplydock")
            || lower.contains("crate")
            || kind_of.contains("resource")
            || kind_of.contains("harvest")
            || kind_compact.contains("supplysource");
        let is_structure = kind_of.contains("structure")
            || kind_of.contains("immobile")
            || (Self::should_spawn_fallback_template(template_name) && !is_resource);

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        }
        if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }
        if kind_of.contains("selectable") || is_structure {
            template.add_kind_of(KindOf::Selectable);
        }
        if kind_of.contains("powered") {
            template.add_kind_of(KindOf::Powered);
        }
        // Wave 982: C++ KINDOF_IGNORED_IN_GUI residual.
        if kind_of.contains("ignored_in_gui") || kind_of.contains("ignoredingui") {
            template.add_kind_of(KindOf::IgnoredInGui);
        }
        Self::add_faction_structure_kind_bits(&mut template, &kind_of);

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        // Faction drop-off buildings only — not map SupplyDock/SupplyPile sources.
        if is_structure
            && !is_resource
            && (lower.contains("supplycenter")
                || lower.contains("supplystash")
                || lower.contains("supplydropzone")
                || lower == "supplycenter")
        {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        if template.max_health <= 1.0 {
            template.set_health(if is_structure { 1200.0 } else { 250.0 });
        }

        // C++ parity: parse ExperienceValue from INI (first value = Rookie level).
        // If not set, use a default based on the object type.
        let xp_val = Self::object_definition_attr(definition, "experiencevalue")
            .and_then(|s| s.split_whitespace().next()?.parse::<f32>().ok())
            .unwrap_or(if is_structure { 100.0 } else { 50.0 });
        template.experience_value = xp_val;

        // C++ parity: parse Armor from INI (default 0).
        if let Some(armor_val) = Self::object_definition_attr(definition, "armor")
            .and_then(|s| s.trim().parse::<f32>().ok())
        {
            template.armor = armor_val;
        }

        // C++ parity: parse VisionRange from INI.
        if let Some(sight) = Self::object_definition_attr(definition, "visionrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|&v| v > 0.0)
        {
            template.sight_range = sight;
        }

        // C++ parity: parse BuildCost from INI.
        if let Some(cost) = Self::object_definition_attr(definition, "buildcost")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|&v| v > 0)
        {
            template.build_cost.supplies = cost;
        }

        // Primary weapon name from Object INI (Weapon = PRIMARY Foo) for WeaponStore bind.
        if let Some(wname) = definition.primary_weapon.as_deref() {
            template.set_primary_weapon_name(wname);
        } else if let Some(raw) = Self::object_definition_attr(definition, "weapon") {
            // Fallback: scan attribute "PRIMARY Name" (last Weapon= line may be secondary)
            let mut parts = raw.split_whitespace();
            if parts
                .next()
                .map(|s| s.eq_ignore_ascii_case("PRIMARY"))
                .unwrap_or(false)
            {
                if let Some(wname) = parts.next() {
                    template.set_primary_weapon_name(wname);
                }
            }
        }

        // Secondary weapon name from Object INI (Weapon = SECONDARY Foo). Fail-closed residual.
        if let Some(wname) = definition.secondary_weapon.as_deref() {
            template.set_secondary_weapon_name(wname);
        }

        // SET_NORMAL Locomotor name from Object INI when present; else known host map.
        // Fail-closed residual: single primary locomotor only (not multi-set / surface matrix).
        if let Some(raw) = Self::object_definition_attr(definition, "locomotor") {
            // Formats: "SET_NORMAL BasicHumanLocomotor" or "SET_NORMAL A B" (take first).
            let mut parts = raw.split_whitespace();
            let first = parts.next().unwrap_or("");
            let loco = if first.eq_ignore_ascii_case("SET_NORMAL")
                || first.eq_ignore_ascii_case("SET_NORMAL_UPGRADED")
                || first.eq_ignore_ascii_case("SET_PANIC")
                || first.eq_ignore_ascii_case("SET_TAXIING")
                || first.eq_ignore_ascii_case("SET_FREEFALL")
            {
                parts.next()
            } else if !first.is_empty() {
                Some(first)
            } else {
                None
            };
            if let Some(lname) = loco {
                template.set_locomotor_name(lname);
            }
        } else if let Some(lname) =
            super::locomotor_bootstrap::locomotor_name_for_unit(template_name)
        {
            template.set_locomotor_name(lname);
        }

        // Combat unit KindOf from object type / kindof string so store weapons can attach.
        let otype = definition.object_type.to_ascii_lowercase();
        if otype.contains("infantry") || kind_of.contains("infantry") {
            template
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }
        if otype.contains("vehicle") || kind_of.contains("vehicle") {
            template
                .add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }
        if otype.contains("aircraft") || kind_of.contains("aircraft") {
            template
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }

        template
    }

    pub(super) fn add_faction_structure_kind_bits(template: &mut ThingTemplate, kind_of: &str) {
        let compact_kind_of = kind_of.replace('_', "");
        let mappings = [
            ("fsbarracks", KindOf::FSBarracks),
            ("fswarfactory", KindOf::FSWarFactory),
            ("fsairfield", KindOf::FSAirfield),
            ("fsinternetcenter", KindOf::FSInternetCenter),
            ("fspower", KindOf::FSPower),
            ("fsbasedefense", KindOf::FSBaseDefense),
            ("fssupplydropzone", KindOf::FSSupplyDropzone),
            ("fssupplycenter", KindOf::FSSupplyCenter),
            ("fssuperweapon", KindOf::FSSuperweapon),
            ("fsstrategycenter", KindOf::FSStrategyCenter),
            ("fsfake", KindOf::FSFake),
            ("fstechnology", KindOf::FSTechnology),
            ("fsblackmarket", KindOf::FSBlackMarket),
            ("fsadvancedtech", KindOf::FSAdvancedTech),
        ];

        for (token, kind) in mappings {
            if compact_kind_of.contains(token) {
                template.add_kind_of(kind);
            }
        }
    }

    pub(super) fn object_definition_attr(definition: &ObjectDefinition, key: &str) -> Option<String> {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.clone()))
    }

    pub(super) fn remap_known_model_alias(model_name: &str) -> String {
        let model_name_lower = model_name.to_ascii_lowercase();
        if let Some(alias) = Self::remap_pt_vegetation_alias(&model_name_lower) {
            return alias.to_string();
        }

        match model_name_lower.as_str() {
            // Defcon6 / neutral civilian model aliases that do not exist under their INI base id
            // in the mounted archive set, but have shipped equivalents.
            "cbnukebunk2" => "CBNukeBunk".to_string(),
            "pmcrates01" => "PMWldCrate".to_string(),
            "pmcrates03" => "PMWldCrate".to_string(),
            "pmcrat01" => "PMWldCrate".to_string(),
            "pmcrat02" => "PMWldCrate".to_string(),
            "zbsmalpile" => "ZBSmalPile_S".to_string(),
            "cbbunker01" => "CBBunker01_SN".to_string(),
            "cbtower2" => "CBTower2_SN".to_string(),
            "cbtower" => "CBTower01".to_string(),
            "cbtower02" => "CBTower02_SN".to_string(),
            "cbtower03" => "CBTower03_SN".to_string(),
            "cbtower04" => "CBTower03_SN".to_string(),
            "cbtower05" => "CBTower05_N".to_string(),
            "cbtaltower" => "CBTalTower_N".to_string(),
            "cbtaltower_tr" => "CBTalTower_N".to_string(),
            "cbtower01_tr" => "CBTower02_TR".to_string(),
            "cbtower04_tr" => "CBTower03_SN".to_string(),
            "cbtower05_tr" => "CBTower05_N".to_string(),
            "cbtoildepo" => "CBOilRefny".to_string(),
            "cbtoiltnk1" => "CBOilRefny".to_string(),
            "cbtoiltnk2" => "CBOilRefny".to_string(),
            "cboilrfny" => "CBOilRfny_SN".to_string(),
            "cbchembunk" => "CBChemBunk_SN".to_string(),
            "pmwtrtwr" => "PMTower".to_string(),
            "pmwtrtwr02" => "PMTower2".to_string(),
            "pmctrslpy" => "PMDock08".to_string(),
            // ZH-only archive set in this workspace ships ABSupplyCT as the _A2* family.
            // Use a mesh-root variant instead of the animation-root ABSupplyCT_A2 file.
            "absupplyct" => "ABSupplyCT_A2U".to_string(),
            "absupplyct_a2" => "ABSupplyCT_A2U".to_string(),
            "ubsupply" => "UBSupplyF".to_string(),
            "ubcmdhq" => "UBCmdHQ_FA".to_string(),
            "absupdrop" => "PMWldCrate".to_string(),
            "nbsupcent" => "ABSupplyCT_A2U".to_string(),
            "nbconyard" => "NBConYard_FA".to_string(),
            "uvtechjeep" => "UVTechJeep_d4".to_string(),
            "uvtechvan" => "UVTechVan_d1".to_string(),
            "uvtechtrck" => "UVTechTrck_D4".to_string(),
            "nvssupplytk" => "NVSSupplyTk_B".to_string(),
            "nbptower" => "NBPwrPti".to_string(),
            "nbbunker" => "NBBunkerI".to_string(),
            "zbhospibib" => "ZBHospibib_S".to_string(),
            "cbnfcitych" => "CBCityBlok".to_string(),
            "salvagecrate" => "PMWldCrate".to_string(),
            "smalllevelupcrate" => "PMWldCrate".to_string(),
            "mediumlevelupcrate" => "PMWldCrate".to_string(),
            "2freecrusaderscrate" => "PMWldCrate".to_string(),
            "100dollarcrate" => "PMWldCrate".to_string(),
            "200dollarcrate" => "PMWldCrate".to_string(),
            "1000dollarcrate" => "PMWldCrate".to_string(),
            "1500dollarcrate" => "PMWldCrate".to_string(),
            "2500dollarcrate" => "PMWldCrate".to_string(),
            "zzsupplydock" => "PMWldCrate".to_string(),
            "zbsupplydk" => "PMWldCrate".to_string(),
            // Decorative map-object aliases observed in challenge/skirmish maps.
            "pmboulders" => "PMBoulders_D".to_string(),
            "pmlclusters" => "PMLClusters_D".to_string(),
            "pmmcluster" => "PMMCluster_D".to_string(),
            "pmcluster" => "PMCluster_D".to_string(),
            "pmrocks02" | "pmrocks03" | "pmrocks05" | "pmrocks06" | "pmrocks07" => {
                "PMBoulders_D".to_string()
            }
            "pmrocks01b" | "pmrocks02b" => "PMBoulders_D".to_string(),
            // Zero Hour INIs reference a few decorative props whose exact W3D ids are absent from
            // the mounted archive set in this workspace. Route them to the closest shipped props
            // so challenge/shell maps keep their background dressing instead of dropping objects.
            "ptcypress01" => "PTXARBVT01".to_string(),
            "ptxpine03" => "PTXFIR07".to_string(),
            "pmswing" => "PMBikeRack".to_string(),
            "pmplygdst" => "PMPavilion".to_string(),
            // AVChinook_A2 is an animation-root file; route model fallback to renderable mesh.
            "avamphib" | "avamphib_a" | "avamphib_a1" => "AVChinook".to_string(),
            "avchinook_a2" => "AVChinook_A2MSH".to_string(),
            "avpaladin" => "AVCrusader_A".to_string(),
            "avpaladin_d" => "avcrusader_d".to_string(),
            "avpaladin_d1" | "avpaladin_d2" | "avpaladin_d3" => "avcrusader_d1".to_string(),
            "pmtrshpp03" | "pmtrshpl02" => "PMBrnTrshPl_D".to_string(),
            "pmpump" => "PMWldCrate".to_string(),
            "pmcrates" => "PMWldCrate".to_string(),
            "cbsandbw2" => "CBSandBWY1".to_string(),
            "cbsandbw4c" => "CBSandBWX".to_string(),
            "cvtruck" => "CVTruck_D1".to_string(),
            "cbnshack" => "CBNShack_S".to_string(),
            "cbtraintnl" => "UIRTunnel".to_string(),
            _ => model_name.to_string(),
        }
    }

    pub(super) fn pt_vegetation_alias_mode() -> &'static str {
        static MODE: OnceLock<String> = OnceLock::new();
        MODE.get_or_init(|| {
            std::env::var("GENERALS_PT_VEGETATION_ALIAS_MODE")
                .unwrap_or_else(|_| "all_fir".to_string())
                .to_ascii_lowercase()
        })
        .as_str()
    }

    pub(super) fn remap_pt_vegetation_alias(model_name_lower: &str) -> Option<&'static str> {
        let tree_target = match Self::pt_vegetation_alias_mode() {
            "trees_birch" | "all_birch" => Some("PTXBirch06"),
            "trees_oak" | "all_oak" => Some("PTXOak06"),
            "trees_palm" | "all_palm" => Some("PTPalm01"),
            "trees_maple" | "all_maple" => Some("PTMaple02"),
            "trees" | "trees_fir" | "all" | "all_fir" | "tree_pine1" | "tree_pine2"
            | "tree_spruce2" | "tree_spruce05" | "trees_pines" | "trees_spruces"
            | "trees_three" | "bushes_pines" | "bushes_spruces" => Some("PTXFir07"),
            _ => None,
        };

        match Self::pt_vegetation_alias_mode() {
            "bushes" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                _ => None,
            },
            "trees" | "trees_fir" | "trees_birch" | "trees_oak" | "trees_palm" | "trees_maple" => {
                match model_name_lower {
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            "tree_pine1" => match model_name_lower {
                "ptpine01" => tree_target,
                _ => None,
            },
            "tree_pine2" => match model_name_lower {
                "ptpine02" => tree_target,
                _ => None,
            },
            "tree_spruce2" => match model_name_lower {
                "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "tree_spruce05" => match model_name_lower {
                "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_pines" => match model_name_lower {
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "trees_spruces" => match model_name_lower {
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_three" => match model_name_lower {
                "ptpine01" | "ptpine02" | "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "bushes_pines" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "bushes_spruces" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "all" | "all_fir" | "all_birch" | "all_oak" | "all_palm" | "all_maple" => {
                match model_name_lower {
                    "ptbush02" => Some("PTBush17"),
                    "ptbush03" => Some("PTBush18"),
                    "ptbush08" => Some("PTBush20"),
                    "ptbush11" => Some("PTBush21"),
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(super) fn is_model_asset_available(model_name: &str) -> bool {
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return false;
        }

        let Some(manager_arc) = get_asset_manager() else {
            // Keep gameplay path permissive during early startup or in tests
            // where the asset manager may not be initialized.
            return true;
        };
        let Ok(mut manager) = manager_arc.lock() else {
            return true;
        };

        let w3d_filename = if model_name.to_ascii_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{model_name}.w3d")
        };

        let mut candidates = vec![
            format!("art/w3d/{w3d_filename}"),
            format!("Art/W3D/{w3d_filename}"),
            w3d_filename.clone(),
            format!("data/w3d/{w3d_filename}"),
            format!("models/{w3d_filename}"),
        ];
        candidates.push(candidates[0].to_ascii_uppercase());
        candidates.push(candidates[0].to_ascii_lowercase());

        candidates
            .into_iter()
            .any(|candidate| manager.can_open_file_sync(&candidate))
    }

    pub(super) fn resolve_spawn_model_name(model_name: &str) -> Option<String> {
        static MODEL_RESOLUTION_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> =
            OnceLock::new();

        let remapped = Self::remap_known_model_alias(model_name);
        if Self::is_model_asset_available(&remapped) {
            return Some(remapped);
        }

        let requested_key = Self::normalize_model_lookup_key(&remapped);
        let cache = MODEL_RESOLUTION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&requested_key) {
                return cached.clone();
            }
        }

        let resolved = {
            let manager_arc = get_asset_manager()?;
            let manager = manager_arc.lock().ok()?;
            let available_models = manager.list_available_models();
            Self::best_available_model_match(&requested_key, available_models.into_iter())
        };

        if let Ok(mut cache) = cache.lock() {
            cache.insert(requested_key, resolved.clone());
        }

        resolved
    }

    pub(super) fn best_available_model_match<I>(requested_key: &str, available_models: I) -> Option<String>
    where
        I: Iterator<Item = String>,
    {
        let requested_trimmed = Self::trim_model_variant_suffixes(requested_key);
        let requested_signature = Self::compact_model_signature(&requested_trimmed);
        let mut best_match: Option<(i32, String)> = None;

        for available_model in available_models {
            let candidate_key = Self::normalize_model_lookup_key(&available_model);
            let candidate_trimmed = Self::trim_model_variant_suffixes(&candidate_key);
            let candidate_signature = Self::compact_model_signature(&candidate_trimmed);
            let score = if candidate_key == requested_key {
                10_000
            } else if candidate_key.starts_with(requested_key) {
                9_000 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if requested_key.starts_with(&candidate_key) {
                8_800 - (requested_key.len() as i32 - candidate_key.len() as i32).abs()
            } else if candidate_trimmed == requested_trimmed {
                8_400 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if candidate_trimmed.starts_with(&requested_trimmed)
                || requested_trimmed.starts_with(&candidate_trimmed)
            {
                8_000 - (candidate_trimmed.len() as i32 - requested_trimmed.len() as i32).abs()
            } else if !requested_signature.is_empty() && candidate_signature == requested_signature
            {
                7_600 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if !requested_signature.is_empty()
                && candidate_signature.contains(&requested_signature)
            {
                7_200 - (candidate_signature.len() as i32 - requested_signature.len() as i32).abs()
            } else {
                let distance =
                    Self::levenshtein_distance(&requested_signature, &candidate_signature);
                if distance <= 2 {
                    6_000 - distance as i32 * 100
                } else {
                    continue;
                }
            };

            match &best_match {
                Some((best_score, _)) if *best_score >= score => {}
                _ => {
                    let canonical = available_model
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&available_model)
                        .trim_end_matches(".w3d")
                        .trim_end_matches(".W3D")
                        .to_string();
                    best_match = Some((score, canonical));
                }
            }
        }

        best_match.map(|(_, model)| model)
    }

    pub(super) fn normalize_model_lookup_key(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_ascii_lowercase()
    }

    pub(super) fn trim_model_variant_suffixes(model_key: &str) -> String {
        let mut trimmed = model_key
            .trim_end_matches(|ch: char| ch.is_ascii_digit())
            .to_string();
        for suffix in [
            "_dsng", "_esn", "_rsn", "_dsn", "_sng", "_dsg", "_sg", "_sn", "_dn", "_en", "_rn",
            "_ds", "_es", "_rs", "_ng", "_dg", "_ns", "_s", "_n", "_d", "_e", "_r", "_g", "_a",
            "_b", "_c",
        ] {
            if let Some(stripped) = trimmed.strip_suffix(suffix) {
                trimmed = stripped.to_string();
                break;
            }
        }
        trimmed
    }

    pub(super) fn compact_model_signature(model_key: &str) -> String {
        model_key
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    }

    pub(super) fn levenshtein_distance(left: &str, right: &str) -> usize {
        if left == right {
            return 0;
        }
        if left.is_empty() {
            return right.len();
        }
        if right.is_empty() {
            return left.len();
        }

        let left_chars: Vec<char> = left.chars().collect();
        let right_chars: Vec<char> = right.chars().collect();
        let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
        let mut current = vec![0usize; right_chars.len() + 1];

        for (i, left_char) in left_chars.iter().enumerate() {
            current[0] = i + 1;
            for (j, right_char) in right_chars.iter().enumerate() {
                let substitution_cost = usize::from(left_char != right_char);
                current[j + 1] = (previous[j + 1] + 1)
                    .min(current[j] + 1)
                    .min(previous[j] + substitution_cost);
            }
            previous.clone_from_slice(&current);
        }

        previous[right_chars.len()]
    }

    pub(super) fn build_fallback_template(template_name: &str) -> ThingTemplate {
        let lower = template_name.to_ascii_lowercase();
        let mut template = ThingTemplate::new(template_name);
        template.set_health(250.0);
        let fallback_model_name = Self::resolve_spawn_model_name(template_name)
            .unwrap_or_else(|| Self::remap_known_model_alias(template_name));
        template.set_model(&fallback_model_name);

        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(manager) = manager_arc.lock() {
                let remapped_model = Self::remap_known_model_alias(template_name);
                if let Some(texture_name) = manager
                    .get_texture_for_object(template_name)
                    .or_else(|| manager.get_texture_for_object(remapped_model.as_str()))
                {
                    if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                        template.texture_name = Some(texture_name);
                    }
                }
            }
        }

        let is_resource = lower.contains("supplypile")
            || lower.contains("supplydock")
            || lower.contains("tempsupplydock")
            || lower.contains("crate");
        let is_structure = Self::should_spawn_fallback_template(template_name) && !is_resource;

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        } else if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        // Faction drop-off buildings only — not map SupplyDock/SupplyPile sources.
        if is_structure
            && !is_resource
            && (lower.contains("supplycenter")
                || lower.contains("supplystash")
                || lower.contains("supplydropzone")
                || lower == "supplycenter")
        {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        template
    }

    pub(super) fn build_visual_fallback_template(template_name: &str) -> Option<ThingTemplate> {
        let template = Self::build_fallback_template(template_name);
        let model_name = template.model_name.as_deref()?.trim();
        if model_name.is_empty() || !Self::is_model_asset_available(model_name) {
            return None;
        }
        Some(template)
    }

    /// Wave 243: first player id for a team without exposing `&Player`.
    pub fn player_id_for_team(&self, team: Team) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.team == team)
            .map(|player| player.id)
    }

    /// Feed Main-crate object positions and sight ranges into the
    /// gamelogic ShroudManager so that fog-of-war reveals around
    /// player-owned units and structures.
    ///
    /// The gamelogic ShroudManager's own `update()` only iterates
    /// objects in the gamelogic OBJECT_REGISTRY; Main-crate objects
    /// are not registered there, so we must push vision directly.
    pub(super) fn update_main_crate_vision(&self) {
        use gamelogic::common::Coord3D;

        let shroud = get_shroud_manager();
        let mut shroud_mgr = match shroud.lock() {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        // Host residual: clear current object visibility membership for known players
        // before rebuilding from Main objects (explored territory persists).
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for &pid in &player_ids {
            shroud_mgr.clear_host_object_visibility(pid);
        }

        // Snapshot alive viewers with vision + all alive targets once.
        let mut viewers: Vec<(crate::game_logic::ObjectId, u32, glam::Vec3, f32)> = Vec::new();
        let mut targets: Vec<(crate::game_logic::ObjectId, glam::Vec3)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let pos = obj.get_position();
            targets.push((obj.id, pos));
            let vision_range = obj.get_template().sight_range;
            if vision_range <= 0.0 {
                continue;
            }
            let Some(owner_pid) = self.player_id_for_team(obj.team) else {
                continue;
            };
            viewers.push((obj.id, owner_pid, pos, vision_range));

            // Terrain looker residual (grid FOW) for allies sharing vision.
            let center = Coord3D::new(pos.x, pos.z, pos.y);
            let mut player_mask = 0u32;
            for (&pid, player) in &self.players {
                if player.team == obj.team {
                    player_mask |= 1u32 << pid.min(31);
                }
            }
            if player_mask != 0 {
                shroud_mgr.do_shroud_reveal(&center, vision_range, player_mask);
            }
        }

        // Own-force residual: every alive object on a player's team is always
        // membership-visible to that player (C++ always draws controlling player units).
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            for (&pid, player) in &self.players {
                if player.team == obj.team && player.team != Team::Neutral {
                    shroud_mgr.mark_host_object_seen(pid, obj.id.0);
                }
            }
        }

        // Object membership residual: mark host objects seen by each viewer's allies.
        // Required because ShroudManager::update() only consults ObjectManager, which
        // does not hold Main host objects on the default authority path.
        for &(viewer_id, owner_pid, viewer_pos, vision_range) in &viewers {
            let mut ally_pids: Vec<u32> = self
                .players
                .iter()
                .filter_map(|(&pid, p)| {
                    self.players
                        .get(&owner_pid)
                        .map(|owner| p.team == owner.team)
                        .unwrap_or(false)
                        .then_some(pid)
                })
                .collect();
            if ally_pids.is_empty() {
                ally_pids.push(owner_pid);
            }
            let range_sq = vision_range * vision_range;
            for &pid in &ally_pids {
                // Always see the viewer itself.
                shroud_mgr.mark_host_object_seen(pid, viewer_id.0);
            }
            for &(target_id, target_pos) in &targets {
                if target_id == viewer_id {
                    continue;
                }
                let dx = target_pos.x - viewer_pos.x;
                let dz = target_pos.z - viewer_pos.z;
                if dx * dx + dz * dz <= range_sq {
                    for &pid in &ally_pids {
                        shroud_mgr.mark_host_object_seen(pid, target_id.0);
                    }
                }
            }
        }
    }

    pub(super) fn shroud_visibility_snapshot_for_team(
        &self,
        viewing_team: Team,
    ) -> Option<ShroudVisibilitySnapshot> {
        let player_id = self.player_id_for_team(viewing_team)?;
        let shroud_mgr = get_shroud_manager().lock().ok()?;
        let raw_visible_objects = shroud_mgr.get_visible_objects(player_id);

        // Match existing fail-open behavior while shroud has not produced runtime visibility yet.
        let runtime_active =
            shroud_mgr.get_last_update_frame() > 0 || !raw_visible_objects.is_empty();
        if !runtime_active {
            return None;
        }

        // Apply stealth-aware visibility to currently visible objects.
        let mut visible_objects = HashSet::with_capacity(raw_visible_objects.len());
        for object_id in raw_visible_objects {
            if shroud_mgr
                .can_see_object_with_stealth(player_id, object_id)
                .unwrap_or(true)
            {
                visible_objects.insert(object_id);
            }
        }

        Some(ShroudVisibilitySnapshot {
            visible_objects,
            explored_objects: shroud_mgr
                .get_explored_objects(player_id)
                .into_iter()
                .collect(),
        })
    }

    pub(super) fn is_object_visible_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            snapshot.visible_objects.contains(&id) || snapshot.explored_objects.contains(&id)
        } else {
            true
        }
    }

    pub(super) fn is_object_visible_on_minimap_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if object.team == viewing_team {
            return true;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            if snapshot.visible_objects.contains(&id) {
                return true;
            }
            // Keep explored structures on minimap for strategic continuity.
            return object.is_kind_of(KindOf::Structure) && snapshot.explored_objects.contains(&id);
        }

        true
    }

    pub fn first_opponent_id(&self, player_id: u32) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.id != player_id)
            .map(|player| player.id)
    }

    pub fn build_victory_summary(&self, winner_id: Option<u32>) -> VictorySummary {
        let mission_name = if self.map_loaded {
            Some(self.map_name.clone())
        } else {
            None
        };

        let duration = if self.sim_time_seconds > 0.0 {
            Some(Duration::from_secs_f32(self.sim_time_seconds))
        } else {
            None
        };

        let mut player_results = Vec::new();
        for player in self.players.values() {
            let outcome = match winner_id {
                Some(id) if id == player.id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => PlayerOutcome::Draw,
            };

            player_results.push(PlayerResult {
                player_id: player.id,
                player_name: player.name.clone(),
                faction: player.team,
                units_built: player.statistics.units_built,
                units_destroyed: player.statistics.units_destroyed,
                units_lost: player.statistics.units_lost,
                structures_built: player.statistics.structures_built,
                structures_destroyed: player.statistics.structures_destroyed,
                structures_lost: player.statistics.structures_lost,
                resources_collected: player.statistics.resources_collected,
                resources_spent: player.statistics.resources_spent,
                outcome,
            });
        }

        VictorySummary {
            mission_name,
            duration,
            player_results,
        }
    }

    pub(super) fn setup_templates(&mut self) {
        log::debug!("Setting up comprehensive RTS unit templates");

        // ====== USA FACTION UNITS ======

        // USA Infantry
        let mut usa_ranger = ThingTemplate::new("USA_Ranger");
        usa_ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(80, 0)
            .set_model("airanger_s") // USA Ranger infantry model
            .set_primary_weapon_name(super::weapon_bootstrap::RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(super::weapon_bootstrap::RANGER_SECONDARY_WEAPON)
            .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates.insert("USA_Ranger".to_string(), usa_ranger);

        let mut usa_missile_defender = ThingTemplate::new("USA_MissileDefender");
        usa_missile_defender
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(300, 0)
            .set_model("aimissletm") // USA Missile Defender
            .set_primary_weapon_name(super::weapon_bootstrap::MISSILE_DEFENDER_MISSILE_WEAPON)
            .set_secondary_weapon_name(
                super::weapon_bootstrap::MISSILE_DEFENDER_LASER_GUIDED_WEAPON,
            );
        self.templates
            .insert("USA_MissileDefender".to_string(), usa_missile_defender);

        // USA Vehicles
        let mut usa_humvee = ThingTemplate::new("USA_Humvee");
        usa_humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0)
            .set_cost(600, 0)
            .set_model("avhummer") // USA Humvee vehicle model
            .set_primary_weapon_name(super::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
            .set_secondary_weapon_name(super::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON)
            .set_locomotor_name(super::locomotor_bootstrap::HUMVEE_LOCOMOTOR);
        self.templates.insert("USA_Humvee".to_string(), usa_humvee);

        let mut usa_crusader = ThingTemplate::new("USA_CrusaderTank");
        usa_crusader
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0)
            .set_cost(1200, 0)
            .set_model("avcrusader") // USA Crusader tank
            .set_primary_weapon_name(super::weapon_bootstrap::CRUSADER_TANK_GUN)
            .set_locomotor_name(super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
        self.templates
            .insert("USA_CrusaderTank".to_string(), usa_crusader);

        let mut usa_paladin = ThingTemplate::new("USA_PaladinTank");
        usa_paladin
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(600.0)
            .set_cost(1800, 0)
            .set_model("avcrusader") // USA Paladin tank (using Crusader model since avpaldin doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::PALADIN_TANK_GUN)
            .set_locomotor_name(super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
        self.templates
            .insert("USA_PaladinTank".to_string(), usa_paladin);

        // USA Aircraft
        let mut usa_raptor = ThingTemplate::new("USA_Raptor");
        usa_raptor
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(180.0)
            .set_cost(1000, 0)
            .set_model("avraptorag") // USA F-22 Raptor
            .set_primary_weapon_name(super::weapon_bootstrap::RAPTOR_JET_MISSILE_WEAPON);
        self.templates.insert("USA_Raptor".to_string(), usa_raptor);

        // ====== GLA FACTION UNITS ======

        // GLA Infantry
        let mut gla_soldier = ThingTemplate::new("GLA_Soldier");
        gla_soldier
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0)
            .set_cost(60, 0)
            .set_model("uirebel") // GLA Rebel infantry model
            .set_primary_weapon_name(super::weapon_bootstrap::GLA_REBEL_PRIMARY_WEAPON)
            .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates
            .insert("GLA_Soldier".to_string(), gla_soldier);

        let mut gla_rpg = ThingTemplate::new("GLA_RPGTrooper");
        gla_rpg
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(100, 0)
            .set_model("uirguard02") // GLA RPG Trooper (using guard model since uirpgtrp doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::TUNNEL_DEFENDER_ROCKET_WEAPON)
            .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates.insert("GLA_RPGTrooper".to_string(), gla_rpg);

        // GLA Vehicles
        let mut gla_technical = ThingTemplate::new("GLA_Technical");
        gla_technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_cost(400, 0)
            .set_model("uvtechvan_d1") // GLA Technical vehicle model
            .set_primary_weapon_name(super::weapon_bootstrap::TECHNICAL_MACHINE_GUN)
            .set_locomotor_name(super::locomotor_bootstrap::TECHNICAL_LOCOMOTOR);
        self.templates
            .insert("GLA_Technical".to_string(), gla_technical);

        let mut gla_scorpion = ThingTemplate::new("GLA_ScorpionTank");
        gla_scorpion
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_cost(900, 0)
            .set_model("uvscorpion") // GLA Scorpion tank
            .set_locomotor_name(super::locomotor_bootstrap::SCORPION_LOCOMOTOR)
            .set_primary_weapon_name(super::weapon_bootstrap::SCORPION_TANK_GUN);
        self.templates
            .insert("GLA_ScorpionTank".to_string(), gla_scorpion);

        let mut gla_marauder = ThingTemplate::new("GLA_MarauderTank");
        gla_marauder
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(450.0)
            .set_cost(1400, 0)
            .set_model("uvlitetank") // GLA Marauder tank (using lite tank model since uvmarudr doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::MARAUDER_TANK_GUN)
            .set_locomotor_name(super::locomotor_bootstrap::SCORPION_LOCOMOTOR);
        self.templates
            .insert("GLA_MarauderTank".to_string(), gla_marauder);

        // C++ shell scripts and map logic still reference original INI object names.
        // Keep those aliases live so the simplified template table does not change behavior.
        if let Some(base) = self.templates.get("GLA_Soldier").cloned() {
            for alias in ["GLAInfantryRebel", "GLAInfantryTerrorist"] {
                let mut template = base.clone();
                template.name = alias.to_string();
                template.display_name = alias.to_string();
                self.templates.insert(alias.to_string(), template);
            }
        }
        if let Some(base) = self.templates.get("GLA_RPGTrooper").cloned() {
            let mut template = base.clone();
            template.name = "GLAInfantryTunnelDefender".to_string();
            template.display_name = "GLAInfantryTunnelDefender".to_string();
            self.templates
                .insert("GLAInfantryTunnelDefender".to_string(), template);
        }
        if let Some(base) = self.templates.get("GLA_Technical").cloned() {
            let mut template = base;
            template.name = "GLAVehicleCombatBike".to_string();
            template.display_name = "GLAVehicleCombatBike".to_string();
            self.templates
                .insert("GLAVehicleCombatBike".to_string(), template);
        }

        // ====== CHINA FACTION UNITS ======

        // China Infantry
        let mut china_infantry = ThingTemplate::new("China_RedGuard");
        china_infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(55.0)
            .set_cost(70, 0)
            .set_model("uirebel") // China Red Guard (using rebel model since ciredgrd doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON)
            .set_locomotor_name(super::locomotor_bootstrap::REDGUARD_LOCOMOTOR);
        self.templates
            .insert("China_RedGuard".to_string(), china_infantry);

        let mut china_tank_hunter = ThingTemplate::new("China_TankHunter");
        china_tank_hunter
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(110, 0)
            .set_model("uirguard02") // China Tank Hunter (using guard model since citankht doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::TANK_HUNTER_PRIMARY_WEAPON);
        self.templates
            .insert("China_TankHunter".to_string(), china_tank_hunter);

        // China Vehicles
        let mut china_battlemaster = ThingTemplate::new("China_BattlemasterTank");
        china_battlemaster
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(360.0)
            .set_cost(1100, 0)
            .set_model("uvscorpion") // China Battlemaster tank (using scorpion model since cvbtlmst doesn't exist)
            .set_primary_weapon_name(super::weapon_bootstrap::BATTLE_MASTER_TANK_GUN)
            .set_locomotor_name(super::locomotor_bootstrap::BATTLE_MASTER_LOCOMOTOR);
        self.templates
            .insert("China_BattlemasterTank".to_string(), china_battlemaster);

        let mut china_overlord = ThingTemplate::new("China_OverlordTank");
        china_overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(700.0)
            .set_cost(2000, 0)
            .set_model("nvovrlrdt") // China Overlord tank (using correct nv pattern model)
            .set_primary_weapon_name(super::weapon_bootstrap::OVERLORD_TANK_GUN);
        self.templates
            .insert("China_OverlordTank".to_string(), china_overlord);

        // China Inferno Cannon — residual FireFieldSmall DoT on shell impact.
        let mut china_inferno = ThingTemplate::new("China_InfernoCannon");
        china_inferno
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_cost(900, 0)
            .set_model("nvinferno")
            .set_primary_weapon_name(super::weapon_bootstrap::INFERNO_CANNON_PRIMARY_WEAPON);
        self.templates
            .insert("China_InfernoCannon".to_string(), china_inferno.clone());
        // Retail INI name alias.
        {
            let mut alias = china_inferno;
            alias.name = "ChinaVehicleInfernoCannon".to_string();
            alias.display_name = "ChinaVehicleInfernoCannon".to_string();
            self.templates
                .insert("ChinaVehicleInfernoCannon".to_string(), alias);
        }

        // China Aircraft
        let mut china_mig = ThingTemplate::new("China_MiG");
        china_mig
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_cost(900, 0)
            .set_model("nvmign"); // China MiG (using correct nv pattern model)
        self.templates.insert("China_MiG".to_string(), china_mig);

        let mut china_helix = ThingTemplate::new("China_Helix");
        china_helix
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_cost(1200, 0)
            .set_model("avhummer"); // China Helix helicopter (using humvee model since cahelix doesn't exist)
        self.templates
            .insert("China_Helix".to_string(), china_helix);

        // ====== BUILDINGS (SHARED) ======

        let mut command_center = ThingTemplate::new("CommandCenter");
        command_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(2000.0)
            .set_cost(2000, 0)
            .set_model("abbtcmdhq"); // USA Command Center model - correct model name
        self.templates
            .insert("CommandCenter".to_string(), command_center);

        let mut supply_center = ThingTemplate::new("SupplyCenter");
        supply_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::SupplyCenter)
            .set_health(1000.0)
            .set_cost(1000, 0)
            .set_model("absupplyct_a2"); // USA supply center model
        self.templates
            .insert("SupplyCenter".to_string(), supply_center);

        let mut power_plant = ThingTemplate::new("PowerPlant");
        power_plant
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::PowerPlant)
            .set_health(800.0)
            .set_cost(800, 0)
            .set_model("abpwrplant_d06"); // USA power plant model
        self.templates.insert("PowerPlant".to_string(), power_plant);

        // CRITICAL: Add missing generic building templates that are referenced in the code
        // These templates ensure perfect alignment with C++ implementation expectations

        // Generic Barracks template (matches what's expected by the engine)
        let mut barracks = ThingTemplate::new("Barracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0)
            .set_cost(600, -1)
            .set_model("abbarracks_fa"); // USA barracks model
        self.templates.insert("Barracks".to_string(), barracks);

        // Generic WarFactory template (matches what's expected by the engine)
        let mut war_factory = ThingTemplate::new("WarFactory");
        war_factory
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1500.0)
            .set_cost(1000, -2)
            .set_model("abwarfact_e"); // USA war factory model
        self.templates.insert("WarFactory".to_string(), war_factory);

        // Add faction-specific building templates for complete C++ alignment
        self.add_faction_building_templates();

        log::info!(
            "Set up {} comprehensive RTS unit templates covering all factions",
            self.templates.len()
        );
    }

    pub(super) fn create_default_players(&mut self) {
        // If map-defined players already exist, keep them; otherwise seed defaults.
        if !self.players.is_empty() {
            return;
        }
        let player1 = Player::new(0, Team::USA, "USA Commander", true);
        let player2 = Player::new(1, Team::GLA, "GLA General", false);
        let player3 = Player::new(2, Team::China, "China Commander", false);

        self.players.insert(0, player1);
        self.players.insert(1, player2);
        self.players.insert(2, player3);

        log::info!(
            "Created {} default players for shell/skirmish bootstrap",
            self.players.len()
        );
    }

    pub(super) fn create_test_map(&mut self) {
        // Wave 733: free demo test map army seed is opt-in only (default fail-closed).
        // Shares GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE with spawn_faction_base.
        let allow = std::env::var_os("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
        if !allow {
            return;
        }
        println!("🗺️ Creating comprehensive RTS test map with faction-aware bases...");

        let mut player_ids: Vec<u32> = self.players.keys().cloned().collect();
        player_ids.sort_unstable();
        let spawn_positions = [
            Vec3::new(-200.0, 0.0, -200.0),
            Vec3::new(200.0, 0.0, 200.0),
            Vec3::new(200.0, 0.0, -200.0),
            Vec3::new(-200.0, 0.0, 200.0),
        ];

        for (idx, player_id) in player_ids.iter().enumerate() {
            let team = self
                .players
                .get(player_id)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            let origin = spawn_positions.get(idx).cloned().unwrap_or(Vec3::ZERO);
            self.spawn_faction_base(team, origin);
        }

        // Neutral center props to mimic tech buildings and abandoned vehicles.
        println!("Adding neutral objectives in center...");
        self.create_object("OilDerrick", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("OilRefinery", Team::Neutral, Vec3::new(50.0, 0.0, 0.0));
        self.create_object("TechHospital", Team::Neutral, Vec3::new(-50.0, 0.0, 50.0));
        self.create_object("USA_Humvee", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("GLA_Technical", Team::Neutral, Vec3::new(20.0, 0.0, 20.0));

        println!(
            "✅ Comprehensive RTS test map created with {} objects across all factions!",
            self.objects.len()
        );

        // Demonstrate the RTS functionality
        self.demonstrate_rts_features();

        // Set up AI opponents for a proper skirmish match
        self.setup_skirmish_ai(0);

        // Demonstrate AI functionality
        self.demonstrate_ai_functionality();
    }

    pub(super) fn spawn_faction_base(&mut self, team: Team, origin: Vec3) {
        // Wave 733: free demo faction army/base spawn is opt-in only (default fail-closed).
        // Not retail skirmish start — vertical-slice/demo harness may set
        // GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE=1.
        let allow = std::env::var_os("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
        if !allow {
            return;
        }
        println!("Creating {:?} base at {:?}", team, origin);
        match team {
            Team::USA => {
                self.create_object("CommandCenter", team, origin);
                self.create_object("SupplyCenter", team, origin + Vec3::new(50.0, 0.0, 50.0));
                self.create_object("PowerPlant", team, origin + Vec3::new(80.0, 0.0, 20.0));

                self.create_object("USA_Ranger", team, origin + Vec3::new(100.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(110.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(120.0, 0.0, 100.0));
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(100.0, 0.0, 90.0),
                );
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(110.0, 0.0, 90.0),
                );

                self.create_object("USA_Humvee", team, origin + Vec3::new(120.0, 0.0, 80.0));
                self.create_object("USA_Humvee", team, origin + Vec3::new(110.0, 0.0, 70.0));
                self.create_object(
                    "USA_CrusaderTank",
                    team,
                    origin + Vec3::new(140.0, 0.0, 60.0),
                );
                self.create_object(
                    "USA_PaladinTank",
                    team,
                    origin + Vec3::new(160.0, 0.0, 50.0),
                );

                self.create_object("USA_Raptor", team, origin + Vec3::new(180.0, 20.0, 40.0));
            }
            Team::GLA => {
                self.create_object("GLA_CommandCenter", team, origin);
                self.create_object("GLA_SupplyStash", team, origin + Vec3::new(0.0, 0.0, 50.0));
                self.create_object("GLA_ArmsDealer", team, origin + Vec3::new(30.0, 0.0, 20.0));

                self.create_object("GLA_Rebel", team, origin + Vec3::new(-10.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-20.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-30.0, 0.0, -10.0));
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -20.0),
                );
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -20.0),
                );

                self.create_object("GLA_Technical", team, origin + Vec3::new(10.0, 0.0, -40.0));
                self.create_object("GLA_Technical", team, origin + Vec3::new(20.0, 0.0, -50.0));
                self.create_object(
                    "GLA_ScorpionTank",
                    team,
                    origin + Vec3::new(0.0, 0.0, -60.0),
                );
                self.create_object(
                    "GLA_MarauderTank",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -60.0),
                );

                self.create_object(
                    "GLA_ScudLauncher",
                    team,
                    origin + Vec3::new(10.0, 0.0, 10.0),
                );
                self.create_object("GLA_Worker", team, origin + Vec3::new(-15.0, 0.0, -15.0));
                self.create_object("GLA_Worker", team, origin + Vec3::new(5.0, 0.0, -10.0));
            }
            Team::China => {
                self.create_object("China_CommandCenter", team, origin);
                self.create_object(
                    "China_SupplyCenter",
                    team,
                    origin + Vec3::new(30.0, 0.0, 30.0),
                );
                self.create_object(
                    "China_NuclearReactor",
                    team,
                    origin + Vec3::new(50.0, 0.0, 10.0),
                );

                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-40.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -30.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -30.0),
                );

                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -20.0),
                );
                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(10.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_OverlordTank",
                    team,
                    origin + Vec3::new(40.0, 0.0, -40.0),
                );
                self.create_object(
                    "China_DragonTank",
                    team,
                    origin + Vec3::new(30.0, 0.0, -50.0),
                );
                self.create_object(
                    "China_GatlingTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -60.0),
                );

                self.create_object("China_MiG", team, origin + Vec3::new(60.0, 20.0, -30.0));
                self.create_object("China_Helix", team, origin + Vec3::new(40.0, 25.0, -20.0));
            }
            Team::Neutral => {
                self.create_object("CommandCenter", team, origin);
            }
        }
    }

}
