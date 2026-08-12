//! Host tick `impl GameLogic` — `teams`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Drive mood auto-acquire for idle units (AI + player AutoAcquireEnemiesWhenIdle).
    pub(crate) fn tick_mood_auto_acquire(&mut self, object_ids: &[ObjectId]) {
        for &id in object_ids {
            let (is_player_local, do_check) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let is_local = self
                    .player_id_for_team(o.team)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| p.is_local)
                    .unwrap_or(false);
                // C++ attack-move auto-acquire: Idle OR AttackMoving/is_attack_path.
                let idle = matches!(o.ai_state, AIState::Idle) && o.target.is_none();
                let attack_moving = matches!(o.ai_state, AIState::AttackMoving) || o.is_attack_path;
                let want = (idle || attack_moving)
                    && o.auto_acquire_when_idle
                    && o.is_alive()
                    && o.can_attack()
                    && o.target.is_none();
                (is_local, want)
            };
            if do_check {
                // C++ AutoAcquireEnemiesWhenIdle applies to player and AI units.
                let _ = self.try_mood_auto_acquire(id, is_player_local);
            }
        }
    }

    #[cfg(test)]
    pub fn tick_mood_auto_acquire_for_test(&mut self, object_ids: &[ObjectId]) {
        self.tick_mood_auto_acquire(object_ids);
    }

    /// C++ AIUpdateInterface::getMoodMatrixActionAdjustment residual.
    ///
    /// Uses `ai_attitude` on the object (-2 Sleep .. +2 Aggressive).
    /// Player-controlled residual always returns ACTION_OK.

    /// C++ AI vision mood factor residual (AI_VISIONFACTOR_MOOD).
    pub fn adjusted_vision_range_for_mood(&self, unit_id: ObjectId) -> f32 {
        let Some(obj) = self.objects.get(&unit_id) else {
            return 0.0;
        };
        let base = obj.vision_range.max(0.0);
        // Attitude multipliers residual (fail-closed approximate).
        let mult = match obj.ai_attitude.clamp(-2, 2) {
            -2 => 0.0, // Sleep: ignore all
            -1 => 1.0, // Passive: wait-for-attack (range still used for last-attacker)
            0 => 1.0,  // Normal
            1 => 1.25, // Alert
            _ => 1.5,  // Aggressive
        };
        base * mult
    }

    /// C++ AIUpdateInterface::getNextMoodTarget residual.
    ///
    /// Returns a candidate enemy to auto-acquire, or None.

    /// C++ AI::findClosestEnemy residual (host simplified partition filters).
    ///
    /// Qualifiers: see `find_enemy_flags`. When the hunter has an AttackPriorityInfo
    /// set, uses priority-distance scoring (C++ modPriority = pri - dist/modifier).

    /// Register/replace a named AttackPriorityInfo set.

    /// Bridge C++ ScriptEngine AttackPriorityInfo / object sets into host GameLogic.
    ///
    /// Copies named priority sets and applies per-object set names so
    /// `find_closest_enemy` priority scoring matches script actions.

    /// Parse C++ AttitudeType name/ordinal residual to host i8.
    pub fn parse_attitude_token(token: &str) -> i8 {
        use crate::game_logic::host_strategy_center::HostAiAttitude;
        match token.trim().to_ascii_uppercase().as_str() {
            "SLEEP" | "-2" => HostAiAttitude::Sleep.as_i8(),
            "PASSIVE" | "-1" => HostAiAttitude::Passive.as_i8(),
            "NORMAL" | "0" => HostAiAttitude::Normal.as_i8(),
            "ALERT" | "DEFENSIVE" | "1" => HostAiAttitude::Alert.as_i8(),
            "AGGRESSIVE" | "2" => HostAiAttitude::Aggressive.as_i8(),
            _ => HostAiAttitude::Normal.as_i8(),
        }
    }

    /// C++ AIUpdateInterface::setAttitude residual (host mood matrix field).
    pub fn set_unit_attitude(&mut self, unit_id: ObjectId, attitude_i8: i8) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.set_ai_attitude_i8(attitude_i8.clamp(-2, 2));
        true
    }

    /// C++ ScriptActions::updateNamedSetAttitude residual.
    pub fn set_named_unit_attitude(&mut self, unit_name: &str, attitude_token: &str) -> bool {
        let id = self.find_object_id_by_name(unit_name);
        let Some(id) = id else {
            return false;
        };
        self.set_unit_attitude(id, Self::parse_attitude_token(attitude_token))
    }

    /// Apply attack priority set name to all objects on a host team.
    pub fn apply_attack_priority_set_to_team(
        &mut self,
        team: crate::game_logic::Team,
        set_name: Option<&str>,
    ) -> usize {
        let mut n = 0usize;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.set_unit_attack_priority_set(id, set_name);
            n += 1;
        }
        n
    }

    /// Inherit team prototype attack priority + initial attitude when missing.
    ///
    /// C++ Object creation: ai->setAttitude(proto initial); setAttackInfo(proto priority name).
    pub fn inherit_team_ai_defaults(&mut self, unit_id: ObjectId) {
        let team = match self.objects.get(&unit_id).map(|o| o.team) {
            Some(t) => t,
            None => return,
        };
        let team_name = match team {
            crate::game_logic::Team::USA => "America",
            crate::game_logic::Team::China => "China",
            crate::game_logic::Team::GLA => "GLA",
            crate::game_logic::Team::Neutral => return,
        };
        let (prio_name, initial_att_i8) = {
            let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
                return;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return;
            };
            let name = proto.get_attack_priority_name().as_str().to_string();
            // C++ Object.cpp: ai->setAttitude(proto->getTemplateInfo()->m_initialTeamAttitude)
            let att = match proto.get_initial_team_attitude() {
                gamelogic::team::AttitudeType::Sleep => -2i8,
                gamelogic::team::AttitudeType::Passive => -1,
                gamelogic::team::AttitudeType::Normal => 0,
                gamelogic::team::AttitudeType::Alert => 1,
                gamelogic::team::AttitudeType::Aggressive => 2,
                gamelogic::team::AttitudeType::Invalid => 0,
            };
            (name, att)
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            if u.attack_priority_set.is_none() && !prio_name.is_empty() {
                u.attack_priority_set = Some(prio_name);
            }
            // Only apply prototype attitude when still at default Normal (0).
            // Script/setAttitude overrides must win over re-inherit.
            if u.ai_attitude == 0 && initial_att_i8 != 0 {
                u.set_ai_attitude_i8(initial_att_i8.clamp(-2, 2));
            }
        }
    }

    /// Resolve host Team from C++/script team name residual.
    pub fn resolve_host_team_name(team_name: &str) -> Option<crate::game_logic::Team> {
        let n = team_name.trim().to_ascii_lowercase();
        let n = n.strip_prefix("team").unwrap_or(&n);
        let n = n.strip_prefix("player").unwrap_or(n);
        match n {
            "usa" | "america" | "us" | "player_1" | "plyrus" | "teamamerica" => {
                Some(crate::game_logic::Team::USA)
            }
            "china" | "prc" | "player_2" | "plyrchina" | "teamchina" => {
                Some(crate::game_logic::Team::China)
            }
            "gla" | "player_3" | "plyrgla" | "teamgla" => Some(crate::game_logic::Team::GLA),
            "neutral" | "civilian" | "pne" => Some(crate::game_logic::Team::Neutral),
            _ => None,
        }
    }

    /// C++ ScriptActions::updateTeamSetAttitude / AIGroup::setAttitude residual.
    /// Applies attitude to every living member of the host team.
    pub fn set_team_attitude(&mut self, team: crate::game_logic::Team, attitude_i8: i8) -> usize {
        let att = attitude_i8.clamp(-2, 2);
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if self.set_unit_attitude(id, att) {
                n += 1;
            }
        }
        n
    }

    /// Named team attitude token residual (TEAM_SET_ATTITUDE script action).
    pub fn set_team_attitude_by_name(&mut self, team_name: &str, attitude_token: &str) -> usize {
        let Some(team) = Self::resolve_host_team_name(team_name) else {
            return 0;
        };
        self.set_team_attitude(team, Self::parse_attitude_token(attitude_token))
    }

    /// Look up host ObjectId by unit name (named object tracker residual).
    pub fn find_object_id_by_name(&self, name: &str) -> Option<ObjectId> {
        // Prefer host object name residual (production path — no engine_object_id).
        let lower = name.to_ascii_lowercase();
        if let Some((id, _)) = self.objects.iter().find(|(_, o)| {
            (!o.name.is_empty() && o.name.eq_ignore_ascii_case(name))
                || o.thing.template.name.eq_ignore_ascii_case(name)
                || o.template_name.eq_ignore_ascii_case(name)
                || o.thing.template.name.to_ascii_lowercase().contains(&lower)
        }) {
            return Some(*id);
        }
        // Engine named tracker residual only when dual-world object bridge is enabled.
        None
    }

    /// C++ TAiData::m_enableRepulsors residual.
    pub fn set_enable_repulsors(&mut self, enabled: bool) {
        self.enable_repulsors = enabled;
        crate::game_logic::host_repulsor_gate::set_enabled(enabled);
    }

    /// C++ Object::setStatus(OBJECT_STATUS_REPULSOR) residual.

    /// C++ Player::setLogicalRetaliationModeEnabled residual.
    pub fn set_logical_retaliation_mode(&mut self, player_id: u32, enabled: bool) {
        if let Some(p) = self.players.get_mut(&player_id) {
            p.logical_retaliation_mode_enabled = enabled;
        }
    }

    /// Enable logical retaliation for all local (human) players.
    pub fn set_all_local_logical_retaliation(&mut self, enabled: bool) {
        for p in self.players.values_mut() {
            if p.is_local {
                p.logical_retaliation_mode_enabled = enabled;
            }
        }
    }

    /// C++ ActiveBody::shouldRetaliateAgainstAggressor residual.
    pub fn should_retaliate_against_aggressor(
        &self,
        victim_id: ObjectId,
        damager_id: ObjectId,
    ) -> bool {
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        let Some(damager) = self.objects.get(&damager_id) else {
            return false;
        };
        if !damager.is_alive() && !damager.status.destroyed {
            // still allow if alive
        }
        if !damager.is_alive() {
            return false;
        }
        // Airborne targets never trigger friend retaliation.
        if damager.status.airborne_target || damager.is_kind_of(KindOf::Aircraft) {
            return false;
        }
        // Enemies only.
        if damager.team == victim.team
            || damager.team == Team::Neutral
            || victim.team == Team::Neutral
        {
            return false;
        }
        let vp = victim.get_position();
        let dp = damager.get_position();
        let dx = vp.x - dp.x;
        let dz = vp.z - dp.z;
        let d2 = dx * dx + dz * dz;
        let max_d = self.max_retaliate_distance;
        if d2 > max_d * max_d {
            return false;
        }
        // Controlling player must be human (local residual).
        let human = self
            .players
            .values()
            .find(|p| p.team == victim.team)
            .map(|p| p.is_local && p.logical_retaliation_mode_enabled)
            .unwrap_or(false);
        if !human {
            return false;
        }
        if victim.is_kind_of(KindOf::Drone) {
            return false;
        }
        true
    }

    /// C++ ActiveBody::shouldRetaliate residual (friend unit eligibility).
    pub fn should_retaliate_friend(&self, unit_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        if !obj.is_alive() || obj.status.destroyed {
            return false;
        }
        if obj.is_kind_of(KindOf::CannotRetaliate)
            || obj.is_kind_of(KindOf::Immobile)
            || obj.is_kind_of(KindOf::Structure)
            || obj.is_kind_of(KindOf::Drone)
        {
            return false;
        }
        // Idle only.
        if !matches!(obj.ai_state, AIState::Idle) || obj.target.is_some() {
            return false;
        }
        // Stealthed + not detected → no.
        if obj.status.stealthed && !obj.status.detected {
            return false;
        }
        obj.can_attack()
    }

    /// C++ ActiveBody damage path friend retaliation residual.
    ///
    /// Nearby allied idle mobile units that can attack the damager enter
    /// GuardRetaliate (host: attack damager). Invoked even if victim died.
    pub fn try_friends_retaliate(&mut self, victim_id: ObjectId, damager_id: ObjectId) -> usize {
        if !self.should_retaliate_against_aggressor(victim_id, damager_id) {
            return 0;
        }
        let (vpos, vteam, vradius) = {
            let Some(v) = self.objects.get(&victim_id) else {
                return 0;
            };
            // Bounding circle residual: use collision radius or default 10.
            let r = if v.is_kind_of(KindOf::Structure) {
                40.0
            } else {
                10.0
            };
            (v.get_position(), v.team, r)
        };
        let range = self.retaliate_friends_radius + vradius;
        let range_sq = range * range;
        let candidates: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(id, o)| {
                **id != victim_id && **id != damager_id && o.team == vteam && o.is_alive() && {
                    let p = o.get_position();
                    let dx = p.x - vpos.x;
                    let dz = p.z - vpos.z;
                    dx * dx + dz * dz <= range_sq
                }
            })
            .map(|(id, _)| *id)
            .collect();

        let mut n = 0usize;
        for fid in candidates {
            if !self.should_retaliate_friend(fid) {
                continue;
            }
            // AbleToAttack residual.
            match self.get_able_to_attack_specific_object(
                fid,
                damager_id,
                AbleToAttackType::NewTarget,
                false,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {}
                _ => continue,
            }
            // C++ aiGuardRetaliate residual.
            if let Some(friend) = self.objects.get_mut(&fid) {
                let anchor = friend.get_position();
                friend.begin_guard_retaliate(damager_id, Some(anchor), None);
                n += 1;
            }
        }
        n
    }
    /// Tick all GuardRetaliating units (victim death / return residual).
    pub fn tick_guard_retaliate_states(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| matches!(o.ai_state, AIState::GuardRetaliating))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let victim_id = self.objects.get(&id).and_then(|o| o.guard_retaliate_victim);
            let (alive, vpos) = match victim_id {
                Some(vid) => match self.objects.get(&vid) {
                    Some(v) if v.is_alive() && !v.status.destroyed => {
                        (true, Some(v.get_position()))
                    }
                    _ => (false, None),
                },
                None => (false, None),
            };
            if let Some(o) = self.objects.get_mut(&id) {
                o.tick_guard_retaliate(alive, vpos);
            }
        }
    }

    pub fn set_unit_repulsor(&mut self, unit_id: ObjectId, repulsor: bool) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.status.repulsor = repulsor;
        true
    }

    /// C++ ScriptActions::doNamedSetRepulsor residual.
    pub fn set_named_unit_repulsor(&mut self, unit_name: &str, repulsor: bool) -> bool {
        let Some(id) = self.find_object_id_by_name(unit_name) else {
            return false;
        };
        self.set_unit_repulsor(id, repulsor)
    }

    /// C++ ScriptActions::doTeamSetRepulsor residual.
    pub fn set_team_repulsor(&mut self, team: crate::game_logic::Team, repulsor: bool) -> usize {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if self.set_unit_repulsor(id, repulsor) {
                n += 1;
            }
        }
        n
    }

    pub fn set_team_repulsor_by_name(&mut self, team_name: &str, repulsor: bool) -> usize {
        let Some(team) = Self::resolve_host_team_name(team_name) else {
            return 0;
        };
        self.set_team_repulsor(team, repulsor)
    }

    /// C++ PartitionFilterRepulsor + AI::findClosestRepulsor residual.
    ///
    /// Returns closest living repulsor in range:
    /// - OBJECT_STATUS_REPULSOR flag, OR
    /// - enemy able-to-attack structure, OR  
    /// - enemy able-to-attack non-structure (C++ filter residual simplified)
    /// Fail-closed vs full PartitionManager filters / stealth reject.
    pub(in super::super) fn find_closest_repulsor(
        &self,
        unit_id: ObjectId,
        range: f32,
    ) -> Option<(ObjectId, f32)> {
        if !self.enable_repulsors {
            return None;
        }
        let me = self.objects.get(&unit_id)?;
        if !me.is_alive() {
            return None;
        }
        let my_pos = me.get_position();
        let my_team = me.team;
        // Pure residual acquire: nearest repulsor in range (XZ).
        // Dead units only count when explicitly flagged repulsor (ActiveBody residual).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&oid, other)| {
                if oid == unit_id {
                    return None;
                }
                let alive = other.is_alive();
                if !alive && !other.status.repulsor {
                    return None;
                }
                // Stealth residual: stealthed + not detected + not disguised → reject
                if other.status.stealthed && !other.status.detected && !other.status.disguised {
                    return None;
                }
                let is_flag_repulsor = other.status.repulsor;
                let is_enemy = other.team != my_team
                    && other.team != Team::Neutral
                    && my_team != Team::Neutral;
                let enemy_attacker = is_enemy && other.can_attack();
                // C++ PartitionFilterRepulsor: flag OR (enemy attackers; structures only if can attack)
                let allow = is_flag_repulsor || enemy_attacker;
                if !allow {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: oid,
                        team: other.team,
                        position: other.get_position(),
                        // Keep dead flagged-repulsors eligible for distance pick.
                        is_alive: true,
                        is_neutral: other.team == Team::Neutral,
                        under_construction: other.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: other.is_kind_of(KindOf::Aircraft) || other.status.airborne_target,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(unit_id),
            (my_pos.x, my_pos.z),
            candidates,
            range.max(0.0),
            |_| true,
        )
        .map(|(id, dist, _)| (id, dist))
    }

    /// C++ AIIdleState repulsor branch + AIMoveAwayFromRepulsors residual.
    ///
    /// For KINDOF_CAN_BE_REPULSED idle units: flee closest repulsor via
    /// ai_move_away_from_unit / request_safe_path residual.

    /// C++ AIUpdateInterface::notifyCrate host bridge.
    pub fn notify_unit_crate(&mut self, unit_id: ObjectId, crate_id: ObjectId) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.notify_crate(crate_id);
        true
    }

    /// C++ AIIdleState crate branch residual.
    ///
    /// If unit has a pending crate notification and the crate still exists in
    /// the money-crate registry (or as a live object), move to pick it up.
    pub fn try_idle_crate_pickup(&mut self, unit_id: ObjectId) -> bool {
        let crate_id = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            // C++ only while idle
            if !matches!(u.ai_state, AIState::Idle) || u.target.is_some() {
                return false;
            }
            // Computer AI residual: humans don't auto-notify; path still works if notified.
            match u.check_for_crate_to_pickup() {
                Some(id) => id,
                None => return false,
            }
        };
        // Crate must still exist and be a registered money crate (or live object).
        let crate_alive = self
            .objects
            .get(&crate_id)
            .map(|c| c.is_alive() && !c.status.destroyed)
            .unwrap_or(false);
        if !crate_alive {
            return false;
        }
        // Prefer registered money crates (wouldLikeToCollide residual).
        let is_money = self.host_money_crates.get(crate_id).is_some();
        if !is_money {
            // Fail-closed: only money crate residual for now.
            return false;
        }
        let crate_pos = self.objects.get(&crate_id).map(|c| c.get_position());
        let Some(pos) = crate_pos else {
            return false;
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            if !u.can_move() {
                return false;
            }
            // C++ aiMoveToObject residual.
            u.move_to(pos);
            u.requested_victim_id = Some(crate_id);
        } else {
            return false;
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_ai_state(AIState::Moving);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        true
    }
}
