//! Host tick `impl GameLogic` — `mood`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// C++ `TheScriptEngine->m_objectsShouldReceiveDifficultyBonus` last applied to live objects.
static LAST_OBJECTS_SHOULD_RECEIVE_DIFFICULTY_BONUS: AtomicBool = AtomicBool::new(true);

impl GameLogic {
    pub fn sync_attack_priority_from_script_engine(&mut self) {
        use gamelogic::scripting::engine::get_script_engine;
        let engine_arc = get_script_engine();
        let Ok(guard) = engine_arc.read() else {
            return;
        };
        let Some(engine) = guard.as_ref() else {
            return;
        };

        // Import all named attack priority sets from script engine.
        // Engine stores Vec with index 0 default; named entries from 1..
        // Public API: get_attack_info by name; also iterate via reflection if available.
        // Fail-closed: pull known sets from object_attack_priority_sets values + common names.
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        // Collect set names referenced by objects.
        // We need access to object_attack_priority_sets - use public get if exists.
        // Engine may only expose get_object_attack_priority_set per id.

        // Sync host objects: host ObjectId is the script-engine key.
        let object_ids: Vec<(ObjectId, Option<u32>)> = self
            .objects
            .iter()
            .map(|(id, _o)| (*id, Some(id.0)))
            .collect();

        for (host_id, eng_opt) in object_ids {
            let Some(eng_id) = eng_opt else {
                continue;
            };
            let set_name = engine
                .get_object_attack_priority_set(eng_id)
                .map(|s| s.to_string());
            if let Some(ref name) = set_name {
                names.insert(name.clone());
            }
            if let Some(u) = self.objects.get_mut(&host_id) {
                u.attack_priority_set = set_name.filter(|s| !s.is_empty());
            }
        }

        // Also import any set already registered on host (no-op) and pull definitions.
        for name in names {
            if let Some(info) = engine.get_attack_info(&name) {
                let mut host = AttackPriorityInfo::new(info.get_name());
                host.default_priority = info.default_priority;
                for (tmpl, pri) in &info.priority_map {
                    host.set_priority_template(tmpl, *pri);
                }
                self.register_attack_priority_set(host);
            }
        }
        let bonus_flag = engine.get_objects_should_receive_difficulty_bonus();
        // Drop engine borrow before inheriting team defaults / walking objects.
        drop(guard);
        drop(engine_arc);
        if LAST_OBJECTS_SHOULD_RECEIVE_DIFFICULTY_BONUS.load(Ordering::Relaxed) != bonus_flag {
            LAST_OBJECTS_SHOULD_RECEIVE_DIFFICULTY_BONUS.store(bonus_flag, Ordering::Relaxed);
            self.apply_or_strip_difficulty_bonuses_for_all_objects(bonus_flag);
        }
        let need_inherit: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.attack_priority_set.is_none() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in need_inherit {
            self.inherit_team_ai_defaults(id);
        }
    }

    pub fn register_attack_priority_set(&mut self, info: AttackPriorityInfo) {
        let key = info.name.to_ascii_lowercase();
        self.attack_priority_sets.insert(key, info);
    }

    /// C++ AIUpdateInterface::setAttackInfo residual (by set name).
    pub fn set_unit_attack_priority_set(&mut self, unit_id: ObjectId, set_name: Option<&str>) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.attack_priority_set = set_name.map(|s| s.to_string());
        }
    }

    /// Resolve AttackPriorityInfo for a unit (None = default closest-only).
    pub fn attack_priority_info_for(&self, unit_id: ObjectId) -> Option<&AttackPriorityInfo> {
        let name = self.objects.get(&unit_id)?.attack_priority_set.as_ref()?;
        self.attack_priority_sets.get(&name.to_ascii_lowercase())
    }

    /// Priority for a candidate target under optional info (includes kind residual).
    pub fn attack_priority_for_target(
        &self,
        info: &AttackPriorityInfo,
        target: &crate::game_logic::object::Object,
    ) -> i32 {
        let mut pri = info.get_priority_for_template(&target.thing.template.name);
        for (kind, &kp) in &info.kind_priorities {
            let hit = match kind.as_str() {
                "infantry" => target.is_kind_of(crate::game_logic::KindOf::Infantry),
                "vehicle" => target.is_kind_of(crate::game_logic::KindOf::Vehicle),
                "structure" | "building" => {
                    target.is_kind_of(crate::game_logic::KindOf::Structure)
                        || target.object_type == crate::game_logic::ObjectType::Building
                }
                "aircraft" => target.is_kind_of(crate::game_logic::KindOf::Aircraft),
                _ => false,
            };
            if hit && kp > pri {
                pri = kp;
            }
        }
        if !target.contained_units().is_empty() {
            for cid in target.contained_units() {
                if let Some(c) = self.objects.get(&cid) {
                    let cp = info.get_priority_for_template(&c.thing.template.name);
                    if cp > pri {
                        pri = cp;
                    }
                }
            }
        }
        pri
    }

    pub fn find_closest_enemy(
        &self,
        unit_id: ObjectId,
        range: f32,
        qualifiers: u32,
    ) -> Option<ObjectId> {
        use find_enemy_flags::*;
        let Some(me) = self.objects.get(&unit_id) else {
            return None;
        };
        if !me.is_alive() {
            return None;
        }
        if (qualifiers & CAN_ATTACK) != 0 && !me.can_attack() {
            return None;
        }
        let me_pos = me.get_position();
        let me_team = me.team;
        let attack_buildings = (qualifiers & ATTACK_BUILDINGS) != 0;
        let within_ar = (qualifiers & WITHIN_ATTACK_RANGE) != 0;
        let need_los = (qualifiers & CAN_SEE) != 0;
        let unfogged = (qualifiers & UNFOGGED) != 0;
        let ignore_insig = (qualifiers & IGNORE_INSIGNIFICANT_BUILDINGS) != 0;
        // C++ PartitionFilterSameMapStatus / leftover find_closest_enemy:
        // owner.is_off_map() != target.is_off_map() is illegal.
        let me_off = crate::game_logic::host_deliver_payload::is_off_map_residual(
            me_pos,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        );
        // C++ AIAttackSquadState::chooseVictim: script flag forces DIFFICULTY_NORMAL
        // (closest), so Easy/Hard AI pick the same target.
        let force_normal = gamelogic::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .map(|engine| engine.get_choose_victim_always_uses_normal())
            })
            .unwrap_or(false);
        let prio = if force_normal {
            None
        } else {
            self.attack_priority_info_for(unit_id)
        };

        let mut best_dist: Option<(ObjectId, f32)> = None;
        let mut best_prio: Option<(ObjectId, i32, i32)> = None; // id, eff, actual

        for (&oid, obj) in self.objects.iter() {
            if oid == unit_id {
                continue;
            }
            let is_enemy = if me.is_undetected_defector() || obj.is_undetected_defector() {
                // C++ Object::getRelationship: self undetected → Neutral,
                // that undetected → Allies. Neither is ENEMIES.
                false
            } else if self.has_object_ownership_provenance(me, obj) {
                self.object_relationship(me, obj) == gamelogic::common::Relationship::Enemies
            } else {
                obj.is_targetable_by_enemy_of(me_team)
            };
            if !is_enemy {
                continue;
            }
            if me_off
                != crate::game_logic::host_deliver_payload::is_off_map_residual(
                    obj.get_position(),
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                )
            {
                continue;
            }
            let is_bldg = obj.is_kind_of(crate::game_logic::KindOf::Structure)
                || obj.object_type == crate::game_logic::ObjectType::Building;
            if !attack_buildings && !self.host_reject_buildings_allows(unit_id, obj) {
                continue;
            }
            // C++ PartitionFilterInsignificantBuildings: drop non-FS structures
            // that are not MP_COUNT_FOR_VICTORY (civilian huts).
            if ignore_insig
                && is_bldg
                && obj.is_non_faction_structure()
                && !obj.is_kind_of(crate::game_logic::KindOf::MpCountForVictory)
            {
                continue;
            }
            let opos = obj.get_position();
            // C++ AI::findClosestEnemy / leftover FROM_BOUNDINGSPHERE_2D:
            // center XY minus both bounding-circle radii (0 if hulls overlap).
            let dist = me.distance_to_object(obj);
            if dist > range {
                continue;
            }
            if within_ar && !me.is_within_attack_range(obj) {
                continue;
            }
            if need_los {
                if self.attack_view_blocked(unit_id, Some(oid), opos)
                    || self.pathfinding_system.is_attack_view_blocked(me_pos, opos)
                {
                    continue;
                }
            }
            if unfogged {
                // C++ PartitionFilterFreeOfFog: human idle AI only acquires
                // OBJECTSHROUD_CLEAR targets (AIUpdate.cpp:4608-4619).
                let viewer = me.owner_player_id;
                let clear = viewer
                    .and_then(|pid| {
                        gamelogic::system::shroud_manager::get_shroud_manager()
                            .lock()
                            .ok()
                            .and_then(|mgr| mgr.get_host_object_shroud_status(pid, oid.0))
                    })
                    .is_some_and(|status| {
                        matches!(status, gamelogic::common::ObjectShroudStatus::Clear)
                    });
                if !clear {
                    continue;
                }
            }
            if (qualifiers & CAN_ATTACK) != 0 {
                let r = self.get_able_to_attack_specific_object(
                    unit_id,
                    oid,
                    AbleToAttackType::NewTarget,
                    false,
                );
                if !matches!(
                    r,
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                ) {
                    continue;
                }
            }

            if let Some(info) = prio {
                let cur = self.attack_priority_for_target(info, obj);
                if cur == 0 {
                    continue; // C++ skip zero priority
                }
                let modifier = (dist / ATTACK_PRIORITY_DISTANCE_MODIFIER) as i32;
                let mut mod_pri = cur - modifier;
                if mod_pri < 1 {
                    mod_pri = 1;
                }
                match best_prio {
                    Some((_, eff, act)) if mod_pri > eff || (mod_pri == eff && cur > act) => {
                        best_prio = Some((oid, mod_pri, cur));
                    }
                    None => best_prio = Some((oid, mod_pri, cur)),
                    _ => {}
                }
            } else {
                match best_dist {
                    Some((_, bd)) if dist < bd => best_dist = Some((oid, dist)),
                    None => best_dist = Some((oid, dist)),
                    _ => {}
                }
            }
        }
        if prio.is_some() {
            best_prio.map(|(id, _, _)| id)
        } else {
            best_dist.map(|(id, _)| id)
        }
    }

    pub(crate) fn host_team_instance_key(object: &crate::game_logic::Object) -> String {
        if !object.team_instance_name.is_empty() {
            object.team_instance_name.clone()
        } else {
            format!("team{}", object.team.get_name())
        }
    }

    /// C++ `TeamPrototype::m_attackCommonTarget` (default true when proto missing).
    pub(crate) fn team_wants_common_attack(&self, object_id: ObjectId) -> bool {
        let Some(object) = self.objects.get(&object_id) else {
            return false;
        };
        let name = Self::host_team_instance_key(object);
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(&name) {
                return proto.attack_common_target();
            }
            if let Some(team) = factory.find_team_instances(&name).into_iter().next() {
                drop(factory);
                if let Ok(tg) = team.read() {
                    return tg.attack_common_target();
                }
            }
        }
        !object.team_instance_name.is_empty()
    }

    pub(crate) fn host_team_common_target(&mut self, object_id: ObjectId) -> Option<ObjectId> {
        if !self.team_wants_common_attack(object_id) {
            return None;
        }
        let name = {
            let object = self.objects.get(&object_id)?;
            Self::host_team_instance_key(object)
        };
        let target_id = *self.team_common_attack_targets.get(&name)?;
        let reject = match self.objects.get(&target_id) {
            None => true,
            Some(target) => {
                !target.is_alive()
                    || (target.status.stealthed
                        && !target.status.detected
                        && !target.status.disguised)
                    || target.contained_by.is_some()
                    || target.is_kind_of(KindOf::Aircraft)
            }
        };
        if reject {
            // C++ Team::getTeamTargetObject clears m_commonAttackTarget.
            self.team_common_attack_targets.remove(&name);
            return None;
        }
        Some(target_id)
    }

    pub(crate) fn set_host_team_common_target(
        &mut self,
        object_id: ObjectId,
        target: Option<ObjectId>,
    ) {
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        let name = Self::host_team_instance_key(object);
        // C++ Team::setTeamTargetObject: NULL always clears, even for humans.
        if target.is_none() {
            self.team_common_attack_targets.remove(&name);
            return;
        }
        if !self.team_wants_common_attack(object_id) {
            return;
        }
        let Some(object) = self.objects.get(&object_id) else {
            return;
        };
        // C++ PLAYER_COMPUTER only; Easy never shares a victim.
        // C++ getPlayerDifficulty() is the controller, not session dominant.
        let owner_pid = object.owner_player_id;
        let is_computer = owner_pid
            .and_then(|pid| self.players.get(&pid))
            .map(|p| !p.is_local)
            .unwrap_or(false);
        if !is_computer {
            return;
        }
        let controller_diff = owner_pid
            .and_then(|pid| self.host_ai_difficulty(pid))
            .unwrap_or_else(|| self.get_difficulty());
        if controller_diff == crate::ai::AIDifficulty::Easy {
            return;
        }
        if let Some(id) = target {
            self.team_common_attack_targets.insert(name, id);
        }
    }

    pub fn get_next_mood_target(
        &mut self,
        unit_id: ObjectId,
        called_by_ai: bool,
        called_during_idle: bool,
        is_player_controlled: bool,
    ) -> Option<ObjectId> {
        let now = self.frame;
        // Snapshot gates.
        let (
            alive,
            using_ability,
            attacking,
            stealthed,
            auto_idle,
            attitude,
            last_dmg,
            pos,
            team,
            rate,
            next_check,
        ) = {
            let o = self.objects.get(&unit_id)?;
            if o.is_kind_of(crate::game_logic::KindOf::Projectile) {
                return None;
            }
            (
                o.is_alive() && !o.status.destroyed,
                o.status.using_ability,
                o.status.attacking || o.ai_state == AIState::Attacking,
                o.status.stealthed,
                o.auto_acquire_idle_bits,
                o.ai_attitude,
                o.last_damage_source,
                o.get_position(),
                o.team,
                o.mood_attack_check_rate.max(1),
                o.next_mood_check_time,
            )
        };
        if !alive || using_ability {
            return None;
        }
        use gamelogic::object::update::ai_update_interface::{
            AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS,
            AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING, AUTO_ACQUIRE_IDLE_STEALTHED,
        };
        if called_during_idle && (auto_idle & AUTO_ACQUIRE_IDLE) == 0 {
            return None;
        }
        if called_during_idle && stealthed && (auto_idle & AUTO_ACQUIRE_IDLE_STEALTHED) == 0 {
            return None;
        }
        if attacking && (auto_idle & AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING) != 0 {
            return None;
        }
        // Sleep mood: no acquire.
        if attitude <= -2 && !is_player_controlled {
            return None;
        }

        // Passive mood: return last damage source if legal enemy.
        if attitude == -1 && !is_player_controlled {
            if let Some(src) = last_dmg {
                if src != unit_id {
                    let ok = matches!(
                        self.get_able_to_attack_specific_object(
                            unit_id,
                            src,
                            AbleToAttackType::NewTarget,
                            false
                        ),
                        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                    );
                    if ok {
                        return Some(src);
                    }
                }
            }
            // Passive without recent attacker: no proactive acquire.
            return None;
        }

        // C++ AIUpdate.cpp:4520-4535 — team common victim before mood scan rate.
        if called_by_ai && attitude >= 0 {
            if let Some(team_victim) = self.host_team_common_target(unit_id) {
                // C++ getNextMoodTarget: caller can-attack vetoes this member
                // only. Team::getTeamTargetObject never clears on that check.
                let can = matches!(
                    self.get_able_to_attack_specific_object(
                        unit_id,
                        team_victim,
                        AbleToAttackType::NewTarget,
                        false,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                );
                if can {
                    return Some(team_victim);
                }
            }
        }
        if called_by_ai {
            if now < next_check && next_check != 0 {
                return None;
            }
            // Schedule next check.
            if let Some(o) = self.objects.get_mut(&unit_id) {
                o.next_mood_check_time = now.saturating_add(rate);
            }
        }

        let mut range = self.adjusted_vision_range_for_mood(unit_id);
        if range <= 0.0 {
            return None;
        }
        // Container radius residual omitted (fail-closed).

        // Human AI residual: only within attack range.
        if called_by_ai && is_player_controlled {
            if let Some(o) = self.objects.get(&unit_id) {
                let wr = o
                    .weapon
                    .as_ref()
                    .map(|w| w.range)
                    .or_else(|| o.secondary_weapon.as_ref().map(|w| w.range))
                    .unwrap_or(0.0);
                range = wr.min(range);
            }
        }

        // C++ findClosestEnemy flags residual.
        use find_enemy_flags::*;
        let mut flags = CAN_ATTACK;
        if let Some(o) = self.objects.get(&unit_id) {
            if Self::aidata_attack_uses_line_of_sight()
                && o.is_kind_of(crate::game_logic::KindOf::AttackNeedsLineOfSight)
            {
                flags |= CAN_SEE;
            }
        }
        if called_by_ai && is_player_controlled {
            flags |= WITHIN_ATTACK_RANGE | UNFOGGED;
        }
        if (auto_idle & AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS) != 0 {
            flags |= ATTACK_BUILDINGS;
        }
        if Self::aidata_attack_ignore_insignificant_buildings() {
            flags |= IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        let _ = (pos, team);
        self.find_closest_enemy(unit_id, range, flags)
    }

    /// C++ `TheAI->getAiData()->m_attackIgnoreInsignificantBuildings`.
    fn aidata_attack_ignore_insignificant_buildings() -> bool {
        if let Some(data) = game_engine::common::ini::get_ai_data_store().get_active() {
            return data.attack_ignore_insignificant_buildings;
        }
        gamelogic::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.attack_ignore_insignificant_buildings)
            })
            .unwrap_or(false)
    }

    /// C++ `TheAI->getAiData()->m_attackUsesLineOfSight` (default true).
    fn aidata_attack_uses_line_of_sight() -> bool {
        if let Some(data) = game_engine::common::ini::get_ai_data_store().get_active() {
            return data.attack_uses_line_of_sight;
        }
        gamelogic::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.attack_uses_line_of_sight)
            })
            .unwrap_or(true)
    }

    /// Idle mood auto-acquire: if idle and mood allows, set attack target.
    pub fn try_mood_auto_acquire(
        &mut self,
        unit_id: ObjectId,
        is_player_controlled: bool,
    ) -> Option<ObjectId> {
        let eligible = self.objects.get(&unit_id).map(|o| {
            let idle = matches!(o.ai_state, AIState::Idle) && !o.status.attacking;
            let attack_moving = matches!(o.ai_state, AIState::AttackMoving) || o.is_attack_path;
            let disabled_skip = o.status.disabled_paralyzed
                || o.status.disabled_unmanned
                || o.status.disabled_emp
                || o.status.disabled_subdued
                || o.status.disabled_hacked;
            (idle || attack_moving) && o.target.is_none() && o.is_alive() && !disabled_skip
        })?;
        if !eligible {
            return None;
        }
        // C++ AIIdleState::update: MAA_Affect_Range_IgnoreAll (Sleep) does not acquire.
        {
            use mood_action_adjust::*;
            let adj = self.get_mood_matrix_action_adjustment(
                unit_id,
                MoodMatrixAction::Idle,
                is_player_controlled,
            );
            if (adj & AFFECT_RANGE_IGNORE_ALL) != 0 {
                return None;
            }
        }
        if !self.mood_allows_attack(unit_id, is_player_controlled) {
            return None;
        }
        let victim = self.get_next_mood_target(unit_id, true, true, is_player_controlled)?;
        // Host-immediate attack enter + decision log inside attack_state_enter.
        if self.attack_state_enter(unit_id, victim) == AttackMachineResult::Continue {
            Some(victim)
        } else {
            None
        }
    }

    pub fn get_mood_matrix_action_adjustment(
        &self,
        unit_id: ObjectId,
        action: MoodMatrixAction,
        is_player_controlled: bool,
    ) -> u32 {
        use mood_action_adjust::*;
        let Some(obj) = self.objects.get(&unit_id) else {
            return ACTION_OK;
        };
        // Mob member residual: IGNORED_IN_GUI KindOf not fully ported.
        if is_player_controlled {
            return ACTION_OK;
        }
        // Attitude ordinals: -2 Sleep, -1 Passive, 0 Normal, 1 Alert, 2 Aggressive.
        let mood = obj.ai_attitude.clamp(-2, 2);
        match action {
            MoodMatrixAction::Idle => match mood {
                -2 => ACTION_OK | AFFECT_RANGE_IGNORE_ALL,
                -1 => ACTION_OK | AFFECT_RANGE_WAIT_FOR_ATTACK,
                0 => ACTION_OK,
                1 => ACTION_OK | AFFECT_RANGE_ALERT,
                _ => ACTION_OK | AFFECT_RANGE_AGGRESSIVE,
            },
            MoodMatrixAction::Move => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                -1 => ACTION_OK | AFFECT_RANGE_WAIT_FOR_ATTACK,
                0 => ACTION_OK,
                1 => ACTION_TO_ATTACK_MOVE | AFFECT_RANGE_ALERT,
                _ => ACTION_TO_ATTACK_MOVE | AFFECT_RANGE_AGGRESSIVE,
            },
            MoodMatrixAction::Attack => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                _ => ACTION_OK,
            },
            MoodMatrixAction::AttackMove => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                -1 | 0 => ACTION_OK,
                1 => ACTION_OK | AFFECT_RANGE_ALERT,
                _ => ACTION_OK | AFFECT_RANGE_AGGRESSIVE,
            },
        }
    }

    pub fn mood_adjusted_move_state(&self, unit_id: ObjectId, state: AIState) -> AIState {
        if state != AIState::Moving {
            return state;
        }
        let is_player = self
            .objects
            .get(&unit_id)
            .and_then(|o| o.owner_player_id)
            .and_then(|pid| self.players.get(&pid))
            .is_some_and(|p| p.is_local);
        let adj =
            self.get_mood_matrix_action_adjustment(unit_id, MoodMatrixAction::Move, is_player);
        if (adj & mood_action_adjust::ACTION_TO_ATTACK_MOVE) != 0 {
            AIState::AttackMoving
        } else {
            state
        }
    }

    /// True when mood allows attack action (MAA_Action_Ok bit set, not forced to idle).
    pub fn mood_allows_attack(&self, unit_id: ObjectId, is_player_controlled: bool) -> bool {
        use mood_action_adjust::*;
        let adj = self.get_mood_matrix_action_adjustment(
            unit_id,
            MoodMatrixAction::Attack,
            is_player_controlled,
        );
        (adj & ACTION_OK) != 0 && (adj & ACTION_TO_IDLE) == 0
    }

    pub fn get_able_to_attack_specific_object(
        &self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        attack_type: AbleToAttackType,
        from_player: bool,
    ) -> CanAttackResult {
        let Some(source) = self.objects.get(&unit_id) else {
            return CanAttackResult::NotPossible;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return CanAttackResult::NotPossible;
        };
        // Basic sanity.
        if !source.is_alive()
            || !victim.is_alive()
            || source.status.destroyed
            || victim.status.destroyed
            || unit_id == victim_id
        {
            return CanAttackResult::NotPossible;
        }
        // C++ WeaponSet rejects UNATTACKABLE before stealth, relationships, or
        // forced-attack handling. These objects still exist for lifecycle and
        // vision, but must never become a player/AI weapon target.
        if victim.is_kind_of(KindOf::Unattackable) {
            return CanAttackResult::NotPossible;
        }
        // C++ OBJECT_STATUS_MASKED is an explicit targetability override.  A
        // masked object may remain alive in the world (for example during a
        // rebuild transition), but WeaponSet must never acquire or fire on it.
        if victim.status.masked {
            return CanAttackResult::NotPossible;
        }
        // MinefieldBehavior and related C++ modules set this generic status
        // bit to keep automatic acquisition off an object while preserving
        // explicit player/script interactions.  All current `from_player =
        // false` callers are the host's AI/mood/retaliation paths.
        if !from_player && victim.has_object_status_bit("NO_ATTACK_FROM_AI") {
            return CanAttackResult::NotPossible;
        }

        let owner_relationship_known = self.has_object_ownership_provenance(source, victim);
        let owner_relationship = self.object_relationship(source, victim);
        let (allies, enemies) = if owner_relationship_known {
            (
                owner_relationship == gamelogic::common::Relationship::Allies,
                owner_relationship == gamelogic::common::Relationship::Enemies,
            )
        } else {
            let allies = source.team == victim.team;
            let enemies = !allies && source.team != Team::Neutral && victim.team != Team::Neutral;
            (allies, enemies)
        };
        let force = attack_type.is_forced();
        let same_owner_force = force && allies;

        // Stealth residual.
        let mut allow_stealth_block = true;
        if source.status.ignoring_stealth || same_owner_force {
            allow_stealth_block = false;
        }
        if force && victim.is_kind_of(KindOf::Disguiser) && victim.status.disguised {
            allow_stealth_block = false;
        }
        if allow_stealth_block && victim.status.stealthed && !victim.status.detected {
            if !victim.status.disguised {
                return CanAttackResult::NotPossible;
            }

            // C++ StealthUpdate keeps a disguised bomb truck targetable only
            // when the *apparent* controller is hostile to the attacker.
            // Relationship checks below intentionally use the real owner, so
            // they cannot substitute for this earlier visibility gate.
            if !crate::game_logic::host_bomb_truck_disguise::is_auto_targetable_as_enemy(
                victim.team,
                victim.disguise_as_team,
                true,
                source.team,
            ) {
                return CanAttackResult::NotPossible;
            }
        }

        // C++ WeaponSet.cpp:529-543 / leftover weapon_set_able.rs:301-311:
        // CMD_FROM_PLAYER non-force attack on a non-enemy is NOT_POSSIBLE
        // unless the victim is a non-allied mine or SCRIPT_TARGETABLE
        // (map `objectTargetable` / leftover_sa `Player Targetable`).
        let is_mine = victim.is_kind_of(KindOf::Mine)
            || victim.is_kind_of(KindOf::DemoTrap)
            || victim.is_disarmable_mine();
        if allies && !force {
            return CanAttackResult::NotPossible;
        }
        if !enemies && !force && !(is_mine && !allies) && from_player {
            if !victim.is_script_targetable() {
                return CanAttackResult::NotPossible;
            }
        }

        // C++ Object::isAbleToAttack (Object.cpp:3167) consults
        // ContainModule::isPassengerAllowedToFire(id). TransportContain is
        // infantry-only — Combat Chinook vehicle riders never shoot out.
        if let Some(cid) = source.contained_by {
            if let Some(container) = self.objects.get(&cid) {
                let bunker_may = crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
                    container.overlord_bunker_slot_capacity(),
                    container.contained_by.is_some(),
                );
                if (container.passengers_allowed_to_fire
                    || bunker_may
                    || container.is_combat_chinook_style_container())
                    && !gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(
                        source.is_kind_of(KindOf::Infantry),
                    )
                {
                    return CanAttackResult::NotPossible;
                }
            }
        }

        // C++ WeaponSet.cpp:545-550 — reject only enclosing containers.
        // Fire Base crew, Overlord/Helix portable, parachute riders stay
        // targetable. MASKED already covers typical enclosing occupants.
        if let Some(cid) = victim.contained_by {
            if self
                .objects
                .get(&cid)
                .is_some_and(|container| container.is_enclosing_container_for(victim))
            {
                return CanAttackResult::NotPossible;
            }
        }

        // C++ WeaponSet.cpp:552-571 / leftover apparent_controller_blocks_player.
        // FROM_PLAYER non-force: hide-garrison apparent controller (original
        // civilian/tech owner) that is not ENEMIES blocks unless SCRIPT_TARGETABLE.
        if !force {
            let r = if owner_relationship_known {
                owner_relationship
            } else if allies {
                gamelogic::common::Relationship::Allies
            } else if enemies {
                gamelogic::common::Relationship::Enemies
            } else {
                gamelogic::common::Relationship::Neutral
            };
            if self.host_apparent_controller_blocks_player(source, victim, from_player, r) {
                return CanAttackResult::NotPossible;
            }
        }

        // Weapon legality / range residual.
        let result =
            self.get_able_to_use_weapon_against_target(unit_id, Some(victim_id), None, attack_type);
        // C++ ActionManager.cpp:740-748 — dozer DISARM InvalidShot → NOT_POSSIBLE.
        if result == CanAttackResult::InvalidShot && source.is_kind_of(KindOf::Dozer) {
            let disarm = source
                .weapon_name_for_slot(source.active_weapon_slot)
                .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                .unwrap_or(true);
            if disarm {
                return CanAttackResult::NotPossible;
            }
        }
        result
    }

    /// C++ WeaponSet.cpp:552-571 — leftover `apparent_controller_blocks_player`.
    fn host_apparent_controller_blocks_player(
        &self,
        source: &Object,
        victim: &Object,
        from_player: bool,
        source_to_victim: gamelogic::common::Relationship,
    ) -> bool {
        let Some(_source_player_id) = self.host_controlling_player_id(source) else {
            return false;
        };
        let Some(apparent_team) = self.host_contain_apparent_controller_team(source, victim) else {
            return false;
        };
        let source_to_apparent = self.host_source_team_to_apparent_default(source, apparent_team);
        gamelogic::weapon::apparent_controller_blocks_player(
            true,
            source_to_apparent,
            from_player,
            victim.is_script_targetable(),
            source_to_victim,
        )
    }

    fn host_controlling_player_id(&self, obj: &Object) -> Option<u32> {
        obj.owner_player_id.or_else(|| {
            self.players
                .values()
                .find(|p| p.team == obj.team)
                .map(|p| p.id)
        })
    }

    /// Leftover GarrisonContain / CaveContain `getApparentControllingPlayer`.
    /// OpenContain default is NULL.
    fn host_contain_apparent_controller_team(
        &self,
        source: &Object,
        victim: &Object,
    ) -> Option<Team> {
        let garrison = victim.is_garrison_contain();
        let cave = victim.is_cave_style_container();
        if !garrison && !cave {
            return None;
        }
        if garrison {
            let hide = victim
                .building_data
                .as_ref()
                .is_some_and(|bd| bd.hide_garrisoned_state);
            let original = victim
                .building_data
                .as_ref()
                .and_then(|bd| bd.original_team);
            let current_to_observer = match (
                self.host_controlling_player_id(victim),
                self.host_controlling_player_id(source),
            ) {
                (Some(cur), Some(obs)) => self.player_relationship(cur, obs),
                _ => gamelogic::common::Relationship::Neutral,
            };
            if gamelogic::object::contain::garrison_hide_returns_original_controller(
                hide,
                original.is_some(),
                self.host_controlling_player_id(victim).is_some(),
                self.host_controlling_player_id(source).is_some(),
                current_to_observer,
            ) {
                return original;
            }
        }
        Some(victim.team)
    }

    /// C++ `source->getTeam()->getRelationship(apparent->getDefaultTeam())`.
    fn host_source_team_to_apparent_default(
        &self,
        source: &Object,
        apparent_team: Team,
    ) -> gamelogic::common::Relationship {
        let apparent_owner = self
            .players
            .values()
            .find(|p| p.team == apparent_team)
            .map(|p| p.id);
        match (self.host_controlling_player_id(source), apparent_owner) {
            (Some(src), Some(app)) => self.player_relationship(src, app),
            _ if source.team == apparent_team => gamelogic::common::Relationship::Allies,
            _ if source.team == Team::Neutral || apparent_team == Team::Neutral => {
                gamelogic::common::Relationship::Neutral
            }
            _ => gamelogic::common::Relationship::Enemies,
        }
    }

    /// C++ WeaponSet::getAbleToUseWeaponAgainstTarget residual.
    pub fn get_able_to_use_weapon_against_target(
        &self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        pos: Option<glam::Vec3>,
        attack_type: AbleToAttackType,
    ) -> CanAttackResult {
        let Some(source) = self.objects.get(&unit_id) else {
            return CanAttackResult::NotPossible;
        };
        // A C++ WeaponSet lock constrains *all* target validation to that
        // concrete slot.  In particular, a permanently selected PRIMARY must
        // not borrow SECONDARY's AntiMask merely because the latter could hit
        // the candidate.  TERTIARY remains manual-only when it is explicitly
        // selected, while normal automatic acquisition considers A/B only.
        let candidate_slots: &[u8] = if source.is_weapon_locked() {
            match source.weapon_lock_slot {
                0 => &[0],
                1 => &[1],
                2 => &[2],
                _ => &[],
            }
        } else if source.selected_weapon_slot() == Some(2) {
            &[2]
        } else {
            &[0, 1]
        };
        let has_a_weapon = candidate_slots
            .iter()
            .copied()
            .any(|slot| source.weapon_slot(slot).is_some());

        let contained_by = source.contained_by;
        let immobile_or_spawn =
            source.is_kind_of(KindOf::Immobile) || source.is_spawns_are_the_weapons();

        let (target_pos, has_legal_anti, has_legal_estimate, pitch_ok) = if let Some(vid) =
            victim_id
        {
            let Some(v) = self.objects.get(&vid) else {
                return CanAttackResult::NotPossible;
            };
            let target_anti_mask = v.weapon_target_anti_mask();
            let kind_ok = |slot: u8, weapon: &crate::game_logic::Weapon| {
                source.weapon_allows_target_anti_mask(weapon, Some(slot), target_anti_mask)
            };
            let owner_relationship_known = self.has_object_ownership_provenance(source, v);
            let allied = if owner_relationship_known {
                self.object_relationship(source, v) == gamelogic::common::Relationship::Allies
            } else {
                v.team == source.team
            };
            if v.is_effectively_stealthed() && !allied && !source.status.ignoring_stealth {
                return CanAttackResult::NotPossible;
            }
            let contain_count = self
                .objects
                .values()
                .filter(|o| o.contained_by == Some(v.id))
                .count() as u32;
            let current_is_primary = if source.is_weapon_locked() {
                source.weapon_lock_slot == 0
            } else {
                source.active_weapon_slot == 0
            };
            // C++ WeaponSet.cpp:706 — skip DAMAGE_KILLPILOT on heroes while
            // current is PRIMARY and no specific slot (rifle-mode Jarmen).
            let skip_hero_kill_pilot = (source.is_kind_of(KindOf::Hero)
                || crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(
                    &source.template_name,
                ))
                && current_is_primary
                && !source.is_weapon_locked();
            let has_legal_anti = candidate_slots.iter().copied().any(|slot| {
                source
                    .weapon_slot(slot)
                    .is_some_and(|weapon| kind_ok(slot, weapon))
            });
            let has_legal_estimate = candidate_slots.iter().copied().any(|slot| {
                source.weapon_slot(slot).is_some_and(|weapon| {
                    if !kind_ok(slot, weapon) {
                        return false;
                    }
                    let name = source.weapon_name_for_slot(slot).unwrap_or("");
                    if skip_hero_kill_pilot
                        && crate::game_logic::weapon_bootstrap::host_weapon_is_kill_pilot_cursor_slot(
                            name,
                        )
                    {
                        return false;
                    }
                    let est = crate::game_logic::weapon_bootstrap::host_estimate_weapon_from_name(
                        name,
                        weapon.damage,
                    );
                    let dt = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
                        name,
                    );
                    let victim_est =
                        crate::game_logic::weapon_bootstrap::host_estimate_victim_from_object(
                            v,
                            contain_count,
                            crate::game_logic::host_armor_residual::apply_residual_armor(
                                v, dt, 1.0,
                            ),
                        );
                    crate::game_logic::weapon_bootstrap::estimate_weapon_template_damage(
                        &est,
                        Some(&victim_est),
                    ) > 0.0
                })
            });
            let pitch_ok = source.is_any_within_target_pitch_for_slots(v, candidate_slots);
            (
                v.get_position(),
                has_legal_anti,
                has_legal_estimate,
                pitch_ok,
            )
        } else if let Some(p) = pos {
            let has_legal_anti = candidate_slots.iter().copied().any(|slot| {
                source.weapon_slot(slot).is_some_and(|weapon| {
                    source.weapon_allows_target_anti_mask(
                        weapon,
                        Some(slot),
                        gamelogic::weapon::WeaponAntiMask::GROUND,
                    )
                })
            });
            (p, has_legal_anti, has_legal_anti, true)
        } else {
            return CanAttackResult::NotPossible;
        };

        // C++ WeaponSet.cpp:630-647 — enclosing garrison uses FIREPOINT goal pose.
        let fire_goal = contained_by.and_then(|cid| {
            self.objects
                .get(&cid)
                .and_then(|c| Object::enclosing_garrison_fire_goal(c, unit_id, target_pos))
        });
        // C++ WeaponSet.cpp:621-647 — range is always m_curWeapon, not any slot.
        let range_slot = source.selected_weapon_slot();
        let within = if let Some(goal) = fire_goal {
            if let Some(vid) = victim_id {
                let v = self.objects.get(&vid).unwrap();
                range_slot.is_some_and(|slot| {
                    source.is_within_attack_range_for_slot_from_goal(slot, goal, v)
                })
            } else {
                range_slot.is_some_and(|slot| {
                    source.is_within_attack_range_pos_for_slot_from_goal(slot, goal, target_pos)
                })
            }
        } else if let Some(vid) = victim_id {
            let v = self.objects.get(&vid).unwrap();
            range_slot.is_some_and(|slot| source.is_within_attack_range_for_slot(slot, v))
        } else {
            range_slot
                .is_some_and(|slot| source.is_within_attack_range_pos_for_slot(slot, target_pos))
        };

        // C++ WeaponSet.cpp:656-660 — immobile / spawn-weapons / contained
        // with a bound weapon out of range is InvalidShot (not walk-there).
        if (immobile_or_spawn || contained_by.is_some())
            && has_a_weapon
            && !within
            && attack_type != AbleToAttackType::TunnelNetworkGuard
        {
            return CanAttackResult::InvalidShot;
        }

        let mut ok_result = if within {
            CanAttackResult::Possible
        } else {
            CanAttackResult::PossibleAfterMoving
        };

        if has_a_weapon {
            if !has_legal_anti {
                return CanAttackResult::InvalidShot;
            }
            if victim_id.is_none() {
                return ok_result;
            }
            if !pitch_ok {
                return CanAttackResult::InvalidShot;
            }
            if has_legal_estimate {
                return ok_result;
            }
        }

        if let Some(r) = self.passenger_weapon_able_result(unit_id, victim_id, pos, attack_type) {
            return r;
        }

        if self.spawn_slaves_possible_against(unit_id, victim_id, pos, attack_type) {
            if self.objects.get(&unit_id).is_some_and(|s| {
                s.is_kind_of(KindOf::Immobile)
                    && s.is_spawns_are_the_weapons()
                    && ok_result == CanAttackResult::PossibleAfterMoving
            }) {
                ok_result = CanAttackResult::Possible;
            }
            return ok_result;
        }

        CanAttackResult::InvalidShot
    }

    /// C++ WeaponSet.cpp:716-737 — occupied container passenger recurse.
    fn passenger_weapon_able_result(
        &self,
        source_id: ObjectId,
        victim_id: Option<ObjectId>,
        pos: Option<glam::Vec3>,
        attack_type: AbleToAttackType,
    ) -> Option<CanAttackResult> {
        let members = {
            let source = self.objects.get(&source_id)?;
            let bunker_may =
                crate::game_logic::host_passengers_fire_upgrade::overlord_bunker_passengers_may_fire(
                    source.overlord_bunker_slot_capacity(),
                    source.contained_by.is_some(),
                );
            // C++ GarrisonContain::isPassengerAllowedToFire — always, unless SUBDUED.
            let garrison_may = source.is_garrison_contain() && !source.is_subdued();
            if !source.passengers_allowed_to_fire && !bunker_may && !garrison_may {
                return None;
            }
            source.contained_units()
        };
        for mid in members {
            if !self.objects.get(&mid).is_some_and(|m| m.can_attack()) {
                continue;
            }
            let r = self.get_able_to_use_weapon_against_target(mid, victim_id, pos, attack_type);
            if matches!(
                r,
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            ) {
                return Some(r);
            }
        }
        None
    }

    /// C++ WeaponSet.cpp:741-756 — hive/spawn slaves POSSIBLE.
    fn spawn_slaves_possible_against(
        &self,
        source_id: ObjectId,
        victim_id: Option<ObjectId>,
        pos: Option<glam::Vec3>,
        attack_type: AbleToAttackType,
    ) -> bool {
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        if !source.is_spawns_are_the_weapons() {
            return false;
        }
        let hive_alive = source.hive_slaves.iter().any(|s| s.alive);
        let slave_ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.producer_id == Some(source_id) && o.id != source_id && o.can_attack())
            .map(|o| o.id)
            .collect();
        for sid in slave_ids {
            if self.get_able_to_use_weapon_against_target(sid, victim_id, pos, attack_type)
                == CanAttackResult::Possible
            {
                return true;
            }
        }
        // Residual Stinger hive soldiers are not full Objects; an alive slot
        // is C++ getCanAnySlavesUseWeaponAgainstTarget POSSIBLE.
        hive_alive
    }
}

#[cfg(test)]
mod common_target_parity {
    use super::*;
    use crate::game_logic::{
        GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate, Weapon,
    };

    fn reset_session_difficulty() {
        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(
            i32::MIN,
        );
    }

    #[test]
    fn set_common_target_skips_easy_and_human() {
        reset_session_difficulty();
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        logic.add_player(Player::new(2, Team::USA, "Human", true));
        let mut ranger = ThingTemplate::new("W21Ranger");
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Attackable);
        logic.templates.insert("W21Ranger".into(), ranger);
        let attacker = logic
            .create_object("W21Ranger", Team::USA, glam::Vec3::ZERO)
            .expect("atk");
        if let Some(o) = logic.host_object_mut(attacker) {
            o.owner_player_id = Some(1);
            o.team_instance_name = "USA_AttackSquad".into();
        }
        let victim = logic
            .create_object("W21Ranger", Team::GLA, glam::Vec3::new(12.0, 0.0, 0.0))
            .expect("vic");

        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(0);
        logic.set_host_team_common_target(attacker, Some(victim));
        assert!(
            logic.team_common_attack_targets.is_empty(),
            "Easy must not seed a shared victim"
        );
        reset_session_difficulty();

        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(1);
        if let Some(o) = logic.host_object_mut(attacker) {
            o.owner_player_id = Some(2);
        }
        logic.set_host_team_common_target(attacker, Some(victim));
        assert!(
            logic.team_common_attack_targets.is_empty(),
            "human teams must not seed a shared victim"
        );

        if let Some(o) = logic.host_object_mut(attacker) {
            o.owner_player_id = Some(1);
        }
        logic.set_host_team_common_target(attacker, Some(victim));
        assert_eq!(
            logic.team_common_attack_targets.get("USA_AttackSquad"),
            Some(&victim),
            "computer Medium may share a victim"
        );
        reset_session_difficulty();
    }

    #[test]
    fn set_common_target_uses_controller_difficulty_not_session() {
        reset_session_difficulty();
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        logic.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Easy);
        let mut ranger = ThingTemplate::new("W26RangerCtrl");
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Attackable);
        logic.templates.insert("W26RangerCtrl".into(), ranger);
        let attacker = logic
            .create_object("W26RangerCtrl", Team::USA, glam::Vec3::ZERO)
            .expect("atk");
        if let Some(o) = logic.host_object_mut(attacker) {
            o.owner_player_id = Some(1);
            o.team_instance_name = "USA_AttackSquad".into();
        }
        let victim = logic
            .create_object("W26RangerCtrl", Team::GLA, glam::Vec3::new(12.0, 0.0, 0.0))
            .expect("vic");

        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(2);
        logic.set_host_team_common_target(attacker, Some(victim));
        assert!(
            logic.team_common_attack_targets.is_empty(),
            "Easy controller in a Hard session must not seed a shared victim"
        );

        logic.set_ai_difficulty(1, crate::ai::AIDifficulty::Hard);
        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(0);
        logic.set_host_team_common_target(attacker, Some(victim));
        assert_eq!(
            logic.team_common_attack_targets.get("USA_AttackSquad"),
            Some(&victim),
            "Hard controller in an Easy session may share a victim"
        );
        reset_session_difficulty();
    }

    #[test]
    fn get_common_target_clears_garrisoned_and_aircraft() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        let mut ranger = ThingTemplate::new("W21Ranger2");
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Attackable);
        logic.templates.insert("W21Ranger2".into(), ranger);
        let mut jet = ThingTemplate::new("W21Jet");
        jet.add_kind_of(KindOf::Aircraft);
        jet.add_kind_of(KindOf::Attackable);
        logic.templates.insert("W21Jet".into(), jet);

        let attacker = logic
            .create_object("W21Ranger2", Team::USA, glam::Vec3::ZERO)
            .expect("atk");
        if let Some(o) = logic.host_object_mut(attacker) {
            o.owner_player_id = Some(1);
            o.team_instance_name = "USA_AttackSquad2".into();
        }
        let plane = logic
            .create_object("W21Jet", Team::GLA, glam::Vec3::new(20.0, 40.0, 0.0))
            .expect("plane");
        logic
            .team_common_attack_targets
            .insert("USA_AttackSquad2".into(), plane);
        assert!(logic.host_team_common_target(attacker).is_none());
        assert!(
            !logic
                .team_common_attack_targets
                .contains_key("USA_AttackSquad2"),
            "aircraft common target must be cleared"
        );

        let garrisoned = logic
            .create_object("W21Ranger2", Team::GLA, glam::Vec3::new(8.0, 0.0, 0.0))
            .expect("garr");
        if let Some(o) = logic.host_object_mut(garrisoned) {
            o.contained_by = Some(crate::game_logic::ObjectId(99));
        }
        logic
            .team_common_attack_targets
            .insert("USA_AttackSquad2".into(), garrisoned);
        assert!(logic.host_team_common_target(attacker).is_none());
        assert!(
            !logic
                .team_common_attack_targets
                .contains_key("USA_AttackSquad2"),
            "garrisoned common target must be cleared"
        );
    }

    #[test]
    fn detected_stealthed_idle_requires_stealthed_bit() {
        use gamelogic::object::update::ai_update_interface::{
            AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_STEALTHED,
        };
        let mut logic = GameLogic::new();
        logic.frame = 100;
        let mut at = ThingTemplate::new("StealthScout");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        let aid = ObjectId(2601);
        logic.objects.insert(aid, {
            let mut o = Object::new(at, aid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            o.ai_attitude = 0;
            o.vision_range = 200.0;
            o.next_mood_check_time = 0;
            o.auto_acquire_idle_bits = AUTO_ACQUIRE_IDLE;
            o.status.stealthed = true;
            o.status.detected = true;
            o.weapon = Some(Weapon {
                range: 80.0,
                damage: 10.0,
                can_target_ground: true,
                ..Default::default()
            });
            o
        });
        let mut vt = ThingTemplate::new("StealthVic");
        vt.add_kind_of(KindOf::Infantry);
        vt.add_kind_of(KindOf::Attackable);
        let vid = ObjectId(2602);
        logic.objects.insert(vid, {
            let mut o = Object::new(vt, vid, Team::GLA);
            o.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
            o
        });
        assert!(
            logic.get_next_mood_target(aid, true, true, false).is_none(),
            "DETECTED must not lift the STEALTHED idle veto"
        );
        if let Some(o) = logic.objects.get_mut(&aid) {
            o.auto_acquire_idle_bits = AUTO_ACQUIRE_IDLE | AUTO_ACQUIRE_IDLE_STEALTHED;
            o.next_mood_check_time = 0;
        }
        assert_eq!(
            logic.get_next_mood_target(aid, true, true, false),
            Some(vid),
            "Stealthed INI bit must allow idle acquire"
        );
    }

    #[test]
    fn mood_scan_ignores_insignificant_buildings() {
        use gamelogic::object::update::ai_update_interface::{
            AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS,
        };
        let prev = {
            let mut store = game_engine::common::ini::get_ai_data_store_mut();
            store.ensure_base();
            let prev = store
                .get_active()
                .map(|d| d.attack_ignore_insignificant_buildings)
                .unwrap_or(false);
            if let Some(data) = store.get_active_mut() {
                data.attack_ignore_insignificant_buildings = true;
            }
            prev
        };
        let mut logic = GameLogic::new();
        logic.frame = 100;
        let mut at = ThingTemplate::new("MoodHutAtk");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        let aid = ObjectId(2701);
        logic.objects.insert(aid, {
            let mut o = Object::new(at, aid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            o.ai_attitude = 0;
            o.vision_range = 200.0;
            o.next_mood_check_time = 0;
            o.auto_acquire_idle_bits = AUTO_ACQUIRE_IDLE | AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS;
            o.weapon = Some(Weapon {
                range: 80.0,
                damage: 10.0,
                can_target_ground: true,
                ..Default::default()
            });
            o
        });
        let mut hut_t = ThingTemplate::new("CivilianHut");
        hut_t.add_kind_of(KindOf::Structure);
        hut_t.add_kind_of(KindOf::Attackable);
        let hut = ObjectId(2702);
        logic.objects.insert(hut, {
            let mut o = Object::new(hut_t, hut, Team::GLA);
            o.set_position(glam::Vec3::new(30.0, 0.0, 0.0));
            o
        });
        assert!(
            logic.get_next_mood_target(aid, true, true, false).is_none(),
            "ATTACK_BUILDINGS + ignore-insig must skip civilian huts"
        );
        if let Some(o) = logic.objects.get_mut(&aid) {
            o.next_mood_check_time = 0;
        }
        let mut inf_t = ThingTemplate::new("MoodHutInf");
        inf_t.add_kind_of(KindOf::Infantry);
        inf_t.add_kind_of(KindOf::Attackable);
        let inf = ObjectId(2703);
        logic.objects.insert(inf, {
            let mut o = Object::new(inf_t, inf, Team::GLA);
            o.set_position(glam::Vec3::new(45.0, 0.0, 0.0));
            o
        });
        assert_eq!(
            logic.get_next_mood_target(aid, true, true, false),
            Some(inf),
            "ignore-insig must still acquire units"
        );
        {
            let mut store = game_engine::common::ini::get_ai_data_store_mut();
            if let Some(data) = store.get_active_mut() {
                data.attack_ignore_insignificant_buildings = prev;
            }
        }
    }

    #[test]
    fn find_closest_enemy_rejects_opposite_off_map() {
        use crate::game_logic::find_enemy_flags;
        let mut logic = GameLogic::new();
        let mut at = ThingTemplate::new("HuntOnMap");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        let aid = ObjectId(2710);
        logic.objects.insert(aid, {
            let mut o = Object::new(at, aid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 9999.0,
                can_target_ground: true,
                damage: 5.0,
                ..Default::default()
            });
            o
        });
        let mut et = ThingTemplate::new("HuntOffMapCargo");
        et.add_kind_of(KindOf::Aircraft);
        et.add_kind_of(KindOf::Attackable);
        let eid = ObjectId(2711);
        let (_, mx) = logic.world_bounds();
        let off = glam::Vec3::new(mx.x + 200.0, 0.0, mx.z + 200.0);
        logic.objects.insert(eid, {
            let mut o = Object::new(et, eid, Team::GLA);
            o.set_position(off);
            o
        });
        assert!(
            logic
                .find_closest_enemy(aid, 9999.9, find_enemy_flags::CAN_ATTACK)
                .is_none(),
            "on-map hunt must not acquire off-map cargo/A10"
        );
    }

    #[test]
    fn find_closest_enemy_ranks_by_bounding_sphere_2d() {
        use crate::game_logic::{HostGeometryInfo, HostGeometryType, ObjectType, find_enemy_flags};
        let mut logic = GameLogic::new();
        let mut at = ThingTemplate::new("HuntHullRanker");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        at.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Sphere,
            is_small: true,
            height: 5.0,
            major_radius: 5.0,
            minor_radius: 5.0,
            authored: true,
        };
        let aid = ObjectId(2720);
        logic.objects.insert(aid, {
            let mut o = Object::new(at, aid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 9999.0,
                can_target_ground: true,
                damage: 5.0,
                ..Default::default()
            });
            o
        });

        let mut bt = ThingTemplate::new("HuntHullBuilding");
        bt.add_kind_of(KindOf::Structure);
        bt.add_kind_of(KindOf::Attackable);
        bt.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Sphere,
            is_small: false,
            height: 50.0,
            major_radius: 50.0,
            minor_radius: 50.0,
            authored: true,
        };
        let bid = ObjectId(2721);
        logic.objects.insert(bid, {
            let mut o = Object::new(bt, bid, Team::GLA);
            o.object_type = ObjectType::Building;
            o.set_position(glam::Vec3::new(200.0, 0.0, 0.0));
            o
        });

        let mut it = ThingTemplate::new("HuntHullInfantry");
        it.add_kind_of(KindOf::Infantry);
        it.add_kind_of(KindOf::Attackable);
        it.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Sphere,
            is_small: true,
            height: 5.0,
            major_radius: 5.0,
            minor_radius: 5.0,
            authored: true,
        };
        let iid = ObjectId(2722);
        logic.objects.insert(iid, {
            let mut o = Object::new(it, iid, Team::GLA);
            o.set_position(glam::Vec3::new(180.0, 0.0, 0.0));
            o
        });

        // Center: infantry 180 < building 200. Hull: building 145 < infantry 170.
        assert_eq!(
            logic.find_closest_enemy(aid, 9999.9, find_enemy_flags::CAN_ATTACK),
            Some(bid),
            "FROM_BOUNDINGSPHERE_2D ranks nearer hull (building) over nearer center (infantry)"
        );
    }

    #[test]
    fn find_closest_enemy_skips_undetected_defector() {
        // C++ PartitionFilterLiveMapEnemies uses getRelationship == ENEMIES.
        use crate::game_logic::{Player, find_enemy_flags};
        use gamelogic::common::Relationship;
        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "PlyrAmerica", true);
        let mut gla = Player::new(1, Team::GLA, "PlyrGLA", false);
        usa.set_map_relationship(1, Relationship::Enemies);
        gla.set_map_relationship(0, Relationship::Enemies);
        logic.add_player(usa);
        logic.add_player(gla);

        let mut at = ThingTemplate::new("HuntDefectorOwner");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        let aid = ObjectId(2730);
        logic.objects.insert(aid, {
            let mut o = Object::new(at, aid, Team::USA);
            o.owner_player_id = Some(0);
            o.set_position(glam::Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 9999.0,
                can_target_ground: true,
                damage: 5.0,
                ..Default::default()
            });
            o
        });

        let mut dt = ThingTemplate::new("FlashingDefector");
        dt.add_kind_of(KindOf::Infantry);
        dt.add_kind_of(KindOf::Attackable);
        let did = ObjectId(2731);
        logic.objects.insert(did, {
            let mut o = Object::new(dt, did, Team::GLA);
            o.owner_player_id = Some(1);
            o.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
            o.begin_undetected_defection(0, 30, false);
            o
        });

        let mut et = ThingTemplate::new("LiveEnemy");
        et.add_kind_of(KindOf::Infantry);
        et.add_kind_of(KindOf::Attackable);
        let eid = ObjectId(2732);
        logic.objects.insert(eid, {
            let mut o = Object::new(et, eid, Team::GLA);
            o.owner_player_id = Some(1);
            o.set_position(glam::Vec3::new(80.0, 0.0, 0.0));
            o
        });

        assert_eq!(
            logic.find_closest_enemy(aid, 9999.9, find_enemy_flags::CAN_ATTACK),
            Some(eid),
            "closer undetected defector must lose to farther live enemy"
        );

        if let Some(o) = logic.objects.get_mut(&aid) {
            o.begin_undetected_defection(0, 30, false);
        }
        assert!(
            logic
                .find_closest_enemy(aid, 9999.9, find_enemy_flags::CAN_ATTACK)
                .is_none(),
            "undetected defector hunter must not auto-acquire"
        );
    }
}

#[cfg(test)]
mod get_able_weapon_parity {
    use super::*;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;

    #[test]
    fn garrisoned_building_recurses_to_occupant_fire() {
        let mut logic = GameLogic::new();
        let mut bt = ThingTemplate::new("CivBunker");
        bt.add_kind_of(KindOf::Structure);
        bt.add_kind_of(KindOf::Immobile);
        let bid = ObjectId(4001);
        logic.objects.insert(bid, {
            let mut o = Object::new(bt, bid, Team::USA);
            o.set_position(Vec3::ZERO);
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.garrisoned_units = vec![ObjectId(4002)];
            bd.garrison_fire_points = vec![Vec3::new(70.0, 0.0, 0.0)];
            bd.garrison_point_occupant = vec![Some(ObjectId(4002))];
            o.building_data = Some(bd);
            o
        });
        let mut rt = ThingTemplate::new("AmericaRanger");
        rt.add_kind_of(KindOf::Infantry);
        rt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4002), {
            let mut o = Object::new(rt, ObjectId(4002), Team::USA);
            o.set_position(Vec3::ZERO);
            o.contained_by = Some(bid);
            o.movement.max_speed = 10.0;
            o.weapon = Some(Weapon {
                range: 40.0,
                damage: 10.0,
                can_target_ground: true,
                ..Default::default()
            });
            o
        });
        let mut vt = ThingTemplate::new("GlaRebel");
        vt.add_kind_of(KindOf::Infantry);
        vt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4003), {
            let mut o = Object::new(vt, ObjectId(4003), Team::GLA);
            o.set_position(Vec3::new(100.0, 0.0, 0.0));
            o
        });
        assert_eq!(
            logic.get_able_to_use_weapon_against_target(
                bid,
                Some(ObjectId(4003)),
                None,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::Possible,
            "garrisoned bunker with no own weapon must inherit occupant FIREPOINT shot"
        );
    }

    #[test]
    fn contained_infantry_oor_is_invalid_shot_not_after_moving() {
        let mut logic = GameLogic::new();
        let mut rt = ThingTemplate::new("GarrRangerOor");
        rt.add_kind_of(KindOf::Infantry);
        rt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4010), {
            let mut o = Object::new(rt, ObjectId(4010), Team::USA);
            o.set_position(Vec3::ZERO);
            o.contained_by = Some(ObjectId(4099));
            o.movement.max_speed = 10.0;
            o.weapon = Some(Weapon {
                range: 30.0,
                damage: 10.0,
                can_target_ground: true,
                ..Default::default()
            });
            o
        });
        let mut vt = ThingTemplate::new("GarrVicOor");
        vt.add_kind_of(KindOf::Infantry);
        vt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4011), {
            let mut o = Object::new(vt, ObjectId(4011), Team::GLA);
            o.set_position(Vec3::new(200.0, 0.0, 0.0));
            o
        });
        assert_eq!(
            logic.get_able_to_use_weapon_against_target(
                ObjectId(4010),
                Some(ObjectId(4011)),
                None,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::InvalidShot,
            "contained infantry cannot show walk-there cursor"
        );
    }

    #[test]
    fn pitch_limited_weapon_is_invalid_shot() {
        let mut logic = GameLogic::new();
        let mut at = ThingTemplate::new("CrusaderPitch");
        at.add_kind_of(KindOf::Vehicle);
        at.primary_weapon_name = Some("CrusaderTankGun".to_string());
        logic.objects.insert(ObjectId(4020), {
            let mut o = Object::new(at, ObjectId(4020), Team::USA);
            o.set_position(Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 200.0,
                damage: 10.0,
                can_target_ground: true,
                ..Default::default()
            });
            o
        });
        let mut vt = ThingTemplate::new("HighInf");
        vt.add_kind_of(KindOf::Infantry);
        vt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4021), {
            let mut o = Object::new(vt, ObjectId(4021), Team::GLA);
            o.set_position(Vec3::new(10.0, 50.0, 0.0));
            o
        });
        assert_eq!(
            logic.get_able_to_use_weapon_against_target(
                ObjectId(4020),
                Some(ObjectId(4021)),
                None,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::InvalidShot,
            "pitch-limited tank gun must reject a steep loft"
        );
    }

    #[test]
    fn stinger_site_falls_through_to_hive_slaves() {
        let mut logic = GameLogic::new();
        let mut st = ThingTemplate::new("GLAStingerSite");
        st.add_kind_of(KindOf::Structure);
        st.add_kind_of(KindOf::Immobile);
        logic.objects.insert(ObjectId(4030), {
            let mut o = Object::new(st, ObjectId(4030), Team::GLA);
            o.set_position(Vec3::ZERO);
            o.hive_slaves = crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
            o.hive_slave_count = 3;
            o
        });
        let mut vt = ThingTemplate::new("UsaRanger");
        vt.add_kind_of(KindOf::Infantry);
        vt.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4031), {
            let mut o = Object::new(vt, ObjectId(4031), Team::USA);
            o.set_position(Vec3::new(80.0, 0.0, 0.0));
            o
        });
        assert_eq!(
            logic.get_able_to_use_weapon_against_target(
                ObjectId(4030),
                Some(ObjectId(4031)),
                None,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::Possible,
            "immobile SPAWNS_ARE_THE_WEAPONS site must inherit slave POSSIBLE"
        );
    }

    #[test]
    fn get_able_accepts_non_enclosing_firebase_crew() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        let mut logic = GameLogic::new();
        let mut at = ThingTemplate::new("AbleAtk");
        at.add_kind_of(KindOf::Infantry);
        at.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4101), {
            let mut o = Object::new(at, ObjectId(4101), Team::USA);
            o.set_position(Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 100.0,
                damage: 10.0,
                ..Default::default()
            });
            o
        });
        let mut fb = ThingTemplate::new("AmericaFireBase");
        fb.add_kind_of(KindOf::Structure);
        fb.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(4),
            is_enclosing_container: false,
            ..Default::default()
        };
        let bid = ObjectId(4102);
        logic.objects.insert(bid, {
            let mut o = Object::new(fb, bid, Team::GLA);
            o.set_position(Vec3::new(10.0, 0.0, 0.0));
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = 4;
            bd.garrisoned_units = vec![ObjectId(4103)];
            o.building_data = Some(bd);
            o
        });
        let mut crew = ThingTemplate::new("AbleCrew");
        crew.add_kind_of(KindOf::Infantry);
        crew.add_kind_of(KindOf::Attackable);
        logic.objects.insert(ObjectId(4103), {
            let mut o = Object::new(crew, ObjectId(4103), Team::GLA);
            o.set_position(Vec3::new(10.0, 0.0, 0.0));
            o.set_contained_by_enclosing(Some(bid), false);
            o
        });
        assert_eq!(
            logic.get_able_to_attack_specific_object(
                ObjectId(4101),
                ObjectId(4103),
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible,
            "Fire Base crew is non-enclosing and must stay targetable"
        );
    }

    #[test]
    fn get_able_rejects_enclosing_garrison_occupant() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        let mut logic = GameLogic::new();
        let mut at = ThingTemplate::new("AbleAtk2");
        at.add_kind_of(KindOf::Infantry);
        logic.objects.insert(ObjectId(4111), {
            let mut o = Object::new(at, ObjectId(4111), Team::USA);
            o.set_position(Vec3::ZERO);
            o.weapon = Some(Weapon {
                range: 100.0,
                damage: 10.0,
                ..Default::default()
            });
            o
        });
        let mut bunker = ThingTemplate::new("CivBunkerAble");
        bunker.add_kind_of(KindOf::Structure);
        bunker.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Garrison,
            slots: Some(5),
            is_enclosing_container: true,
            ..Default::default()
        };
        let bid = ObjectId(4112);
        logic.objects.insert(bid, {
            let mut o = Object::new(bunker, bid, Team::GLA);
            o.set_position(Vec3::new(10.0, 0.0, 0.0));
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = 5;
            bd.garrisoned_units = vec![ObjectId(4113)];
            o.building_data = Some(bd);
            o
        });
        let mut occ = ThingTemplate::new("AbleOcc");
        occ.add_kind_of(KindOf::Infantry);
        logic.objects.insert(ObjectId(4113), {
            let mut o = Object::new(occ, ObjectId(4113), Team::GLA);
            o.set_position(Vec3::new(10.0, 0.0, 0.0));
            o.set_contained_by_enclosing(Some(bid), true);
            o
        });
        assert_eq!(
            logic.get_able_to_attack_specific_object(
                ObjectId(4111),
                ObjectId(4113),
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::NotPossible,
            "enclosing garrison occupants stay untargetable"
        );
    }
}

#[test]
fn player_attack_rejects_neutral_without_script_targetable() {
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("TgtAtk");
    at.add_kind_of(KindOf::Infantry);
    logic.objects.insert(ObjectId(4201), {
        let mut o = Object::new(at, ObjectId(4201), Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            ..Default::default()
        });
        o
    });
    let mut nt = ThingTemplate::new("CivInfantry");
    nt.add_kind_of(KindOf::Infantry);
    nt.add_kind_of(KindOf::Attackable);
    logic.objects.insert(ObjectId(4202), {
        let mut o = Object::new(nt, ObjectId(4202), Team::Neutral);
        o.set_position(Vec3::new(10.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4201),
            ObjectId(4202),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::NotPossible,
        "CMD_FROM_PLAYER must reject neutrals without SCRIPT_TARGETABLE"
    );
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4201),
            ObjectId(4202),
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::Possible,
        "AI/script attacks ignore SCRIPT_TARGETABLE"
    );
    logic
        .objects
        .get_mut(&ObjectId(4202))
        .unwrap()
        .apply_object_panel_flag("Player Targetable", true);
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4201),
            ObjectId(4202),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::Possible,
        "leftover_sa objectTargetable must admit player attack on neutrals"
    );
    logic
        .objects
        .get_mut(&ObjectId(4202))
        .unwrap()
        .set_script_targetable(false);
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4201),
            ObjectId(4202),
            AbleToAttackType::NewTargetForced,
            true,
        ),
        CanAttackResult::Possible,
        "force attack still ignores SCRIPT_TARGETABLE"
    );
}

#[test]
fn player_attack_allows_non_allied_mine_without_script_targetable() {
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("MineAtk");
    at.add_kind_of(KindOf::Infantry);
    logic.objects.insert(ObjectId(4211), {
        let mut o = Object::new(at, ObjectId(4211), Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            ..Default::default()
        });
        o
    });
    let mut mt = ThingTemplate::new("DemoTrap");
    mt.add_kind_of(KindOf::Mine);
    logic.objects.insert(ObjectId(4212), {
        let mut o = Object::new(mt, ObjectId(4212), Team::Neutral);
        o.set_position(Vec3::new(10.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4211),
            ObjectId(4212),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::Possible,
        "non-allied mines stay player-targetable without SCRIPT_TARGETABLE"
    );
}

#[test]
fn player_attack_rejects_hidden_garrison_without_script_targetable() {
    use crate::game_logic::{
        BuildingData, BuildingType, ContainModuleKind, ContainModuleMetadata, Player,
    };
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.alliance_team = 1;
    let mut gla = Player::new(1, Team::GLA, "GLA", false);
    gla.alliance_team = 2;
    let civ = Player::new(9, Team::Neutral, "Civilian", false);
    logic.add_player(usa);
    logic.add_player(gla);
    logic.add_player(civ);

    let mut at = ThingTemplate::new("HideAtk");
    at.add_kind_of(KindOf::Infantry);
    logic.objects.insert(ObjectId(4301), {
        let mut o = Object::new(at, ObjectId(4301), Team::USA);
        o.set_position(Vec3::ZERO);
        o.owner_player_id = Some(0);
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            ..Default::default()
        });
        o
    });

    let mut bunker = ThingTemplate::new("CivBunkerHide");
    bunker.add_kind_of(KindOf::Structure);
    bunker.add_kind_of(KindOf::Attackable);
    bunker.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        is_enclosing_container: true,
        ..Default::default()
    };
    logic.objects.insert(ObjectId(4302), {
        let mut o = Object::new(bunker, ObjectId(4302), Team::Neutral);
        o.set_position(Vec3::new(10.0, 0.0, 0.0));
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 5;
        bd.original_team = Some(Team::Neutral);
        bd.hide_garrisoned_state = true;
        o.building_data = Some(bd);
        o.set_team_and_owner(Team::GLA, Some(1));
        o
    });

    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4301),
            ObjectId(4302),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::NotPossible,
        "FROM_PLAYER must reject hide-garrison whose apparent controller is the original non-enemy owner"
    );
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4301),
            ObjectId(4302),
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::Possible,
        "AI/script attacks ignore apparent-controller hide"
    );
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4301),
            ObjectId(4302),
            AbleToAttackType::NewTargetForced,
            true,
        ),
        CanAttackResult::Possible,
        "force attack skips leftover apparent_controller_blocks_player"
    );
    logic
        .objects
        .get_mut(&ObjectId(4302))
        .unwrap()
        .apply_object_panel_flag("Player Targetable", true);
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4301),
            ObjectId(4302),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::Possible,
        "SCRIPT_TARGETABLE admits player attack on hidden garrison"
    );
    logic
        .objects
        .get_mut(&ObjectId(4302))
        .unwrap()
        .set_script_targetable(false);
    if let Some(bd) = logic
        .objects
        .get_mut(&ObjectId(4302))
        .unwrap()
        .building_data
        .as_mut()
    {
        bd.hide_garrisoned_state = false;
    }
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            ObjectId(4301),
            ObjectId(4302),
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::Possible,
        "visible occupant-owned garrison stays player-attackable"
    );
}
