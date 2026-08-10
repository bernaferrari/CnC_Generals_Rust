//! Host tick `impl GameLogic` — `mood`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
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
        // Drop engine borrow before inheriting team defaults.
        drop(guard);
        drop(engine_arc);
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
        let _unfogged = (qualifiers & UNFOGGED) != 0;
        let _ignore_insig = (qualifiers & IGNORE_INSIGNIFICANT_BUILDINGS) != 0;
        let prio = self.attack_priority_info_for(unit_id);

        let mut best_dist: Option<(ObjectId, f32)> = None;
        let mut best_prio: Option<(ObjectId, i32, i32)> = None; // id, eff, actual

        for (&oid, obj) in self.objects.iter() {
            if oid == unit_id {
                continue;
            }
            if !obj.is_targetable_by_enemy_of(me_team) {
                continue;
            }
            let is_bldg = obj.is_kind_of(crate::game_logic::KindOf::Structure)
                || obj.object_type == crate::game_logic::ObjectType::Building;
            if is_bldg && !attack_buildings {
                let bldg_can_attack =
                    obj.can_attack() || obj.weapon.is_some() || obj.secondary_weapon.is_some();
                if !bldg_can_attack {
                    continue;
                }
            }
            let opos = obj.get_position();
            let dx = opos.x - me_pos.x;
            let dz = opos.z - me_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
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
                o.status.stealthed && !o.status.detected,
                o.auto_acquire_when_idle,
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
        if called_during_idle && !auto_idle {
            return None;
        }
        // Stealthed idle acquire residual: block unless contained-fire (not ported).
        if called_during_idle && stealthed {
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
            if o.is_kind_of(crate::game_logic::KindOf::Structure) || !o.can_move() {
                flags |= CAN_SEE;
            }
        }
        if called_by_ai && is_player_controlled {
            flags |= WITHIN_ATTACK_RANGE | UNFOGGED;
        }
        let _ = (pos, team);
        self.find_closest_enemy(unit_id, range, flags)
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
            (idle || attack_moving) && o.target.is_none() && o.is_alive()
        })?;
        if !eligible {
            return None;
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
        // MASKED residual: under_construction structures not attackable until built? skip.
        // UNATTACKABLE kind residual: name-token fail-closed.
        // UNATTACKABLE residual: KindOf token not yet ported; skip name gate.
        // NO_ATTACK_FROM_AI residual not fully tracked; skip unless AI path later.

        let force = attack_type.is_forced();
        let same_owner_force = force && source.team == victim.team;

        // Stealth residual.
        let mut allow_stealth_block = true;
        if source.status.ignoring_stealth || same_owner_force {
            allow_stealth_block = false;
        }
        if force && victim.status.disguised {
            allow_stealth_block = false;
        }
        if allow_stealth_block
            && victim.status.stealthed
            && !victim.status.detected
            && !victim.status.disguised
        {
            return CanAttackResult::NotPossible;
        }

        // Relationship residual: ENEMIES required unless force / mine.
        let enemies = source.team != victim.team;
        let is_mine = false; // KindOf::Projectile residual pending full matrix
        if !enemies && !force && !(is_mine && source.team != victim.team) {
            // Player command rejects non-enemies; AI/script may continue.
            if from_player {
                return CanAttackResult::NotPossible;
            }
            // AI residual: still allow if not same team allies — same team blocked.
            if source.team == victim.team {
                return CanAttackResult::NotPossible;
            }
        }

        // Contained in enclosing container residual.
        if victim.contained_by.is_some() {
            // Fail-closed: treat any contained victim as not directly attackable
            // unless force (garrison fire residual elsewhere).
            if !force {
                return CanAttackResult::NotPossible;
            }
        }

        // Weapon legality / range residual.
        self.get_able_to_use_weapon_against_target(unit_id, Some(victim_id), None, attack_type)
    }

    /// C++ WeaponSet::getAbleToUseWeaponAgainstTarget residual.
    pub fn get_able_to_use_weapon_against_target(
        &self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        pos: Option<glam::Vec3>,
        _attack_type: AbleToAttackType,
    ) -> CanAttackResult {
        let Some(source) = self.objects.get(&unit_id) else {
            return CanAttackResult::NotPossible;
        };
        if source.weapon.is_none() && source.secondary_weapon.is_none() {
            return CanAttackResult::InvalidShot;
        }

        let target_pos = if let Some(vid) = victim_id {
            let Some(v) = self.objects.get(&vid) else {
                return CanAttackResult::NotPossible;
            };
            // Kind legality residual (air/ground), NOT range — range decides
            // Possible vs PossibleAfterMoving below (C++ WeaponSet split).
            let target_is_air =
                v.is_kind_of(crate::game_logic::KindOf::Aircraft) || v.status.airborne_target;
            let kind_ok = |w: &crate::game_logic::Weapon| {
                if target_is_air {
                    w.can_target_air
                } else {
                    w.can_target_ground
                }
            };
            // Stealthed gate already applied in get_able_to_attack_specific_object;
            // still block here if stealthed and not ignoring (defense in depth).
            if v.is_effectively_stealthed()
                && v.team != source.team
                && !source.status.ignoring_stealth
            {
                return CanAttackResult::NotPossible;
            }
            let primary_ok = source.weapon.as_ref().is_some_and(kind_ok);
            let secondary_ok = source.secondary_weapon.as_ref().is_some_and(kind_ok);
            if !primary_ok && !secondary_ok {
                return CanAttackResult::InvalidShot;
            }
            v.get_position()
        } else if let Some(p) = pos {
            p
        } else {
            return CanAttackResult::NotPossible;
        };

        let within = if let Some(vid) = victim_id {
            let v = self.objects.get(&vid).unwrap();
            source.is_within_attack_range(v)
        } else {
            source.is_within_attack_range_pos(target_pos)
        };

        // Contact / invalid pitch residual not expanded — range gate only.
        if within {
            CanAttackResult::Possible
        } else {
            // Mobile residual: max_speed > 0 or can_move.
            let mobile = source.can_move() || source.movement.max_speed > 1e-3;
            if mobile {
                CanAttackResult::PossibleAfterMoving
            } else {
                // Immobile and out of range → invalid shot residual.
                CanAttackResult::InvalidShot
            }
        }
    }
}
