//! Host tick `impl GameLogic` — `teams`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `PATHFIND_CELL_SIZE_F`.
const HOST_WANDER_CELL: f32 = 10.0;

/// C++ `AIWanderInPlaceState` origin + current hop + leftover repulsor timer.
struct HostWanderInPlace {
    origin: glam::Vec3,
    hop: glam::Vec3,
    timer: i32,
    wait_frames: i32,
}

/// C++ `AIWanderState` / `AIPanicState` leftover-bail timer while following a path.
struct HostWanderPath {
    timer: i32,
    wait_frames: i32,
}

fn wander_in_place_sessions()
-> &'static std::sync::Mutex<std::collections::HashMap<u32, HostWanderInPlace>> {
    static SESSIONS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<u32, HostWanderInPlace>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    &SESSIONS
}

fn wander_in_place_lock()
-> std::sync::MutexGuard<'static, std::collections::HashMap<u32, HostWanderInPlace>> {
    wander_in_place_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn wander_path_sessions()
-> &'static std::sync::Mutex<std::collections::HashMap<u32, HostWanderPath>> {
    static SESSIONS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<u32, HostWanderPath>>,
    > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    &SESSIONS
}

fn wander_path_lock()
-> std::sync::MutexGuard<'static, std::collections::HashMap<u32, HostWanderPath>> {
    wander_path_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Leftover `AIWanderState::update_group_offset` / `AIPanicState::update_group_offset`.
/// C++ `m_groupOffset = GameLogicRandomValue(-delta,delta)*PATHFIND_CELL_SIZE_F`
/// with `delta = floor(WanderWidthFactor+0.5).max(1)` when factor > 0.
fn leftover_wander_group_offset(wander_width_factor: f32) -> glam::Vec2 {
    if wander_width_factor <= 0.0 {
        return glam::Vec2::ZERO;
    }
    let mut delta = (wander_width_factor + 0.5).floor() as i32;
    if delta < 1 {
        delta = 1;
    }
    let x = game_engine::common::random_value::get_game_logic_random_value(-delta, delta) as f32
        * HOST_WANDER_CELL;
    let y = game_engine::common::random_value::get_game_logic_random_value(-delta, delta) as f32
        * HOST_WANDER_CELL;
    glam::Vec2::new(x, y)
}

fn leftover_apply_wander_group_offset(pos: glam::Vec3, offset: glam::Vec2) -> glam::Vec3 {
    glam::Vec3::new(pos.x + offset.x, pos.y, pos.z + offset.y)
}

/// C++ `m_waitFrames = 10 + (getID() & 0x7)`.
fn leftover_wander_wait_frames(id: ObjectId) -> i32 {
    10 + ((id.0 & 0x7) as i32)
}

/// C++ timer starts 0; first update decrements then scans.
fn leftover_wander_tick_timer(can_be_repulsed: bool, timer: i32, wait_frames: i32) -> (i32, bool) {
    if !can_be_repulsed {
        return (timer, false);
    }
    let t = timer - 1;
    if t < 0 {
        (wait_frames, true)
    } else {
        (t, false)
    }
}

/// Leftover `the_ai.find_closest_repulsor` then live leftover-faithful host port.
fn leftover_wander_has_repulsor(logic: &GameLogic, id: ObjectId, vision: f32) -> bool {
    let ai_store = gamelogic::ai::the_ai();let leftover_hit = ai_store
        .read()
        .ok()
        .and_then(|ai| ai.find_closest_repulsor(id.0, vision).ok())
        .flatten()
        .is_some();
    leftover_hit || logic.find_closest_repulsor(id, vision).is_some()
}

/// C++ `AIWanderInPlaceState::chooseNewGoal` delta (cells).
/// Loco template present: `floor(WanderAboutPointRadius / cell + 0.5)`.
/// No loco / unknown template: C++ fallback of 3.
fn host_wander_radius_for_name(name: &str) -> Option<f32> {
    let store = game_engine::common::ini::ini_locomotor::get_locomotor_store();
    if let Some(template) = store.find_template(name) {
        return Some(template.wander_about_point_radius);
    }
    gamelogic::locomotor::ini_bridge::convert_named(name).map(|t| t.wander_about_point_radius)
}

fn host_wander_about_point_delta(obj: &crate::game_logic::object::Object) -> i32 {
    use crate::game_logic::host_upgrade_module_residuals::{
        HostLocomotorSetKind, locomotor_name_for_set_kind,
    };
    let wander_set = obj
        .get_cur_locomotor_set_token()
        .is_some_and(|s| s.eq_ignore_ascii_case("SET_WANDER"));
    let name = if wander_set {
        locomotor_name_for_set_kind(&obj.template_name, HostLocomotorSetKind::Wander)
            .map(str::to_string)
            .or_else(|| obj.cur_locomotor_name.clone())
    } else {
        obj.cur_locomotor_name.clone()
    };
    let Some(name) = name else {
        return 3;
    };
    match host_wander_radius_for_name(&name) {
        Some(radius) => ((radius / HOST_WANDER_CELL) + 0.5).floor() as i32,
        None => 3,
    }
}

fn host_wander_choose_hop(
    obj: &crate::game_logic::object::Object,
    origin: glam::Vec3,
) -> glam::Vec3 {
    let delta = host_wander_about_point_delta(obj);
    let ox = game_engine::common::random_value::get_game_logic_random_value(-delta, delta) as f32
        * HOST_WANDER_CELL;
    let oz = game_engine::common::random_value::get_game_logic_random_value(-delta, delta) as f32
        * HOST_WANDER_CELL;
    glam::Vec3::new(origin.x + ox, origin.y, origin.z + oz)
}

fn host_wander_horiz_dist_sq(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

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
        use crate::game_logic::host_radar_stealth_vision_residual::{
            VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL, VISION_ALERT_RANGE_MODIFIER_RESIDUAL,
        };
        let leftover = game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| (d.alert_range_modifier, d.aggressive_range_modifier));
        let leftover = leftover.or_else(|| {
            gamelogic::ai::the_ai().read().ok().and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| (d.alert_range_modifier, d.aggressive_range_modifier))
            })
        });
        let (alert, aggressive) = leftover
            .map(|(a, g)| {
                (
                    if a > 0.0 {
                        a
                    } else {
                        VISION_ALERT_RANGE_MODIFIER_RESIDUAL
                    },
                    if g > 0.0 {
                        g
                    } else {
                        VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL
                    },
                )
            })
            .unwrap_or((
                VISION_ALERT_RANGE_MODIFIER_RESIDUAL,
                VISION_AGGRESSIVE_RANGE_MODIFIER_RESIDUAL,
            ));
        let mult = match obj.ai_attitude.clamp(-2, 2) {
            -2 => 0.0, // Sleep: ignore all
            -1 => 1.0, // Passive: wait-for-attack (range still used for last-attacker)
            0 => 1.0,  // Normal
            1 => alert,
            _ => aggressive,
        };
        base * mult
    }

    /// C++ AIGuardMachine::getStdGuardRange / outer vision (no GUARDINNER).
    /// Returns (inner, outer). Sleep mood yields (0, 0).
    pub fn host_std_guard_ranges(&self, unit_id: ObjectId) -> (f32, f32) {
        let Some(obj) = self.objects.get(&unit_id) else {
            return (0.0, 0.0);
        };
        let player_is_human = self
            .player_id_for_team(obj.team)
            .and_then(|pid| self.players.get(&pid))
            .map(|p| p.is_local)
            .unwrap_or(false);
        let mood = obj.ai_attitude.clamp(-2, 2);
        let weapon_r = obj
            .weapon
            .as_ref()
            .map(|w| w.range)
            .or_else(|| obj.secondary_weapon.as_ref().map(|w| w.range))
            .unwrap_or(0.0);
        let contained = obj.contained_by.is_some();
        let base = obj.vision_range.max(0.0);
        let inner =
            crate::game_logic::host_radar_stealth_vision_residual::vision_adjusted_range_residual(
                base,
                player_is_human,
                true,
                contained,
                weapon_r,
                mood == 1,
                mood >= 2,
                mood <= -2,
                true,
            );
        let outer =
            crate::game_logic::host_radar_stealth_vision_residual::vision_adjusted_range_residual(
                base,
                player_is_human,
                false,
                contained,
                weapon_r,
                mood == 1,
                mood >= 2,
                mood <= -2,
                true,
            );
        (inner, outer)
    }

    /// C++ TAiData::m_guardChaseUnitFrames — leftover AIData, else retail 4s.
    pub fn host_guard_chase_unit_frames(&self) -> u32 {
        let leftover = game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| d.guard_chase_unit_frames)
            .filter(|&frames| frames > 0)
            .or_else(|| {
                gamelogic::ai::the_ai().read().ok().and_then(|ai| {
                    ai.get_ai_data()
                        .read()
                        .ok()
                        .map(|d| d.guard_chase_unit_frames)
                        .filter(|&frames| frames > 0)
                })
            });
        leftover.unwrap_or(
            crate::game_logic::host_radar_stealth_vision_residual::GUARD_CHASE_UNIT_FRAMES_RESIDUAL,
        )
    }

    /// C++ TAiData::m_guardEnemyScanRate — leftover AIData, else 0.5s.
    pub fn host_guard_enemy_scan_rate(&self) -> u32 {
        game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| d.guard_enemy_scan_rate)
            .filter(|&rate| rate > 0)
            .or_else(|| {
                gamelogic::ai::the_ai().read().ok().and_then(|ai| {
                    ai.get_ai_data()
                        .read()
                        .ok()
                        .map(|d| d.guard_enemy_scan_rate)
                        .filter(|&rate| rate > 0)
                })
            })
            .unwrap_or(30)
    }

    /// C++ TAiData::m_guardEnemyReturnScanRate — leftover AIData, else 1s.
    pub fn host_guard_enemy_return_scan_rate(&self) -> u32 {
        game_engine::common::ini::get_ai_data_store()
            .get_active()
            .map(|d| d.guard_enemy_return_scan_rate)
            .filter(|&rate| rate > 0)
            .or_else(|| {
                gamelogic::ai::the_ai().read().ok().and_then(|ai| {
                    ai.get_ai_data()
                        .read()
                        .ok()
                        .map(|d| d.guard_enemy_return_scan_rate)
                        .filter(|&rate| rate > 0)
                })
            })
            .unwrap_or(60)
    }

    /// C++ PartitionFilterRejectBuildings — keep non-buildings; computer
    /// players acquire all enemy structures; humans keep FS_BASE_DEFENSE and
    /// garrisoned attacking buildings.
    pub(crate) fn host_reject_buildings_allows(&self, owner_id: ObjectId, cand: &Object) -> bool {
        if !cand.is_kind_of(KindOf::Structure) {
            return true;
        }
        let owner_is_computer = self
            .objects
            .get(&owner_id)
            .and_then(|me| me.owner_player_id)
            .and_then(|pid| self.players.get(&pid))
            .map(|p| !p.is_local)
            // C++ PartitionFilterRejectBuildings ctor (PartitionManager.cpp:4890-4899):
            // only a PLAYER_COMPUTER controller sets m_acquireEnemies. The C++
            // neutral/default controller is PLAYER_HUMAN, so an ownerless object
            // (tests, pre-assignment) must reject plain enemy structures.
            .unwrap_or(false);
        if owner_is_computer {
            return true;
        }
        if cand.is_kind_of(KindOf::FSBaseDefense) {
            return true;
        }
        cand.can_attack() && (cand.is_garrison_contain() || !cand.contained_units().is_empty())
    }

    /// C++ GuardRetaliateExitConditions — timer, per-state victim radius, owner leash.
    fn guard_retaliate_chase_should_exit(
        &self,
        unit_id: ObjectId,
        victim_pos: Option<glam::Vec3>,
    ) -> bool {
        let Some(me) = self.objects.get(&unit_id) else {
            return false;
        };
        // C++ Inner scan victims stamp no give-up timer.
        if me.guard_chase_give_up_frame != 0 && self.frame >= me.guard_chase_give_up_frame {
            return true;
        }
        let center = me
            .guard_retaliate_anchor
            .or(me.guard_position)
            .unwrap_or_else(|| me.get_position());
        let (inner, outer) = self.host_std_guard_ranges(unit_id);
        // Inner: 1.5×stdGuard. Outer: 0.67×(vision+stdGuard). Aggressor: vision+stdGuard.
        let victim_r = match me.guard_chase_phase {
            1 => 1.5 * inner,
            2 => 0.67 * (outer + inner),
            _ => outer + inner,
        };
        if let Some(vp) = victim_pos {
            let dx = vp.x - center.x;
            let dz = vp.z - center.z;
            if victim_r > 0.0 && dx * dx + dz * dz > victim_r * victim_r {
                return true;
            }
        }
        let us = me.get_position();
        let dx = us.x - center.x;
        let dz = us.z - center.z;
        inner > 0.0 && dx * dx + dz * dz > inner * inner
    }

    /// C++ AIGuardRetaliate lookForInnerTarget — leftover `ALLOW_ENEMIES`,
    /// reject buildings except base defenses / garrisoned attackers /
    /// computer-owned scans.
    fn scan_guard_retaliate_inner(&self, unit_id: ObjectId) -> Option<ObjectId> {
        let me = self.objects.get(&unit_id)?;
        let team = me.team;
        let owner_player = me.owner_player_id;
        let owner_inst = me.team_instance_name.clone();
        let owner_undetected = me.is_undetected_defector();
        let anchor = me
            .guard_retaliate_anchor
            .or(me.guard_position)
            .unwrap_or_else(|| me.get_position());
        let (inner, _) = self.host_std_guard_ranges(unit_id);
        if inner <= 0.0 {
            return None;
        }
        let (world_min, world_max) = self.world_bounds();
        let owner_off = crate::game_logic::host_deliver_payload::is_off_map_residual(
            me.get_position(),
            world_min.x,
            world_min.z,
            world_max.x,
            world_max.z,
        );
        let enter_guard = me.thing.template.enter_guard;
        let hijack_guard = me.thing.template.hijack_guard;
        let radius_sq = inner * inner;
        let mut best: Option<(ObjectId, f32)> = None;
        for (cid, cand) in self.objects.iter() {
            if *cid == unit_id || !cand.is_alive() || cand.status.destroyed {
                continue;
            }
            let cand_pos = cand.get_position();
            if owner_off
                != crate::game_logic::host_deliver_payload::is_off_map_residual(
                    cand_pos,
                    world_min.x,
                    world_min.z,
                    world_max.x,
                    world_max.z,
                )
            {
                continue;
            }
            let dx = anchor.x - cand_pos.x;
            let dz = anchor.z - cand_pos.z;
            let d_sq = dx * dx + dz * dz;
            if d_sq > radius_sq {
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
                    if rel != Relationship::Enemies || !self.can_hijack_vehicle(unit_id, cand) {
                        continue;
                    }
                } else if rel != Relationship::Neutral
                    || !self.can_unit_enter_normal_target(unit_id, *cid)
                {
                    continue;
                }
            } else {
                if !self.host_reject_buildings_allows(unit_id, cand) {
                    continue;
                }
                if rel != Relationship::Enemies {
                    continue;
                }
                if !matches!(
                    self.get_able_to_attack_specific_object(
                        unit_id,
                        *cid,
                        AbleToAttackType::NewTarget,
                        false,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                ) {
                    continue;
                }
            }
            if best.map(|(_, bd)| d_sq < bd).unwrap_or(true) {
                best = Some((*cid, d_sq));
            }
        }
        best.map(|(id, _)| id)
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

    /// Inherit named team prototype attack priority + initial attitude.
    ///
    /// C++ Object.cpp:439-448 / leftover `apply_team_ai_profile`:
    /// `ai->setAttitude(getTeam()->getPrototype()->...m_initialTeamAttitude)`.
    /// The prototype is the object's Team instance (`AmericaTeamRangers`,
    /// `teamAmerica`), never the faction enum (`USA`/`America`).
    pub fn inherit_team_ai_defaults(&mut self, unit_id: ObjectId) {
        let (owner, team, instance) = match self.objects.get(&unit_id) {
            Some(o) => (o.owner_player_id, o.team, o.team_instance_name.clone()),
            None => return,
        };
        let trimmed = instance.trim();
        if trimmed.is_empty() {
            let default = self.default_host_team_instance_name(owner, team);
            if default.is_empty() {
                return;
            }
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.team_instance_name = default;
            }
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            // Create / re-inherit: scripted setAttitude must win.
            u.apply_named_team_ai_profile(false);
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

    /// C++ default team name is `"team" + playerName` (Player.cpp).
    pub fn default_host_team_instance_name(
        &self,
        owner_player_id: Option<u32>,
        team: crate::game_logic::Team,
    ) -> String {
        if let Some(pid) = owner_player_id {
            if let Some(player) = self.players.get(&pid) {
                let name = player.name.trim();
                if !name.is_empty() {
                    return format!("team{name}");
                }
            }
        }
        format!("team{}", team.get_name())
    }

    /// C++ GameLogic.cpp:1888 `team->setActive()` when the team first has members.
    pub fn activate_leftover_team_for_host_object(&self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let team_name = obj.team_instance_name.trim();
        if team_name.is_empty() {
            return;
        }
        let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
            return;
        };
        let team = factory
            .find_team(team_name)
            .or_else(|| factory.create_inactive_team(team_name));
        let Some(team) = team else {
            return;
        };
        if let Ok(mut guard) = team.write() {
            guard.add_member(id.0);
            guard.set_active();
        }
    }

    /// C++ Object.cpp:4592 `m_team->notifyTeamOfObjectDeath()`.
    pub fn notify_leftover_team_of_host_object_death(&self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let team_name = obj.team_instance_name.trim();
        if team_name.is_empty() {
            return;
        }
        let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
            return;
        };
        let Some(team) = factory.find_team(team_name) else {
            return;
        };
        if let Ok(mut guard) = team.write() {
            if !guard.has_member(id.0) {
                return;
            }
            guard.notify_team_of_object_death();
            guard.remove_member(id.0);
        }
        drop(factory);
        gamelogic::team::flush_pending_team_script_events();
    }

    /// C++ Object destructor unlinks from TeamMemberList.
    pub fn unlink_leftover_team_host_member(&self, id: ObjectId, team_name: &str) {
        let team_name = team_name.trim();
        if team_name.is_empty() {
            return;
        }
        let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
            return;
        };
        let Some(team) = factory.find_team(team_name) else {
            return;
        };
        if let Ok(mut guard) = team.write() {
            guard.remove_member(id.0);
        }
    }

    /// Live Team member ids for leftover TEAM_* census (includes dead, no faction bleed).
    pub fn host_script_team_census_member_ids(&self, team_name: &str) -> Vec<u32> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        self.host_objects()
            .values()
            .filter(|obj| {
                if !obj.team_instance_name.is_empty() {
                    return obj.team_instance_name.eq_ignore_ascii_case(needle);
                }
                self.default_host_team_instance_name(obj.owner_player_id, obj.team)
                    .eq_ignore_ascii_case(needle)
            })
            .map(|obj| obj.id.0)
            .collect()
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
    /// C++ `updateTeamSetAttitude` walks the named team's live members.
    pub fn set_team_attitude_by_name(&mut self, team_name: &str, attitude_token: &str) -> usize {
        let att = Self::parse_attitude_token(attitude_token);
        let needle = team_name.trim();
        if needle.is_empty() {
            return 0;
        }
        let faction = Self::resolve_host_team_name(team_name);
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && ((!o.team_instance_name.is_empty()
                        && o.team_instance_name.eq_ignore_ascii_case(needle))
                        || faction.map(|t| o.team == t).unwrap_or(false)
                        || o.team.get_name().eq_ignore_ascii_case(needle))
            })
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

    /// C++ GameEngine.cpp:480 TheAI init after AIData.ini — live player path.
    /// `GameLogic::new` stays false (TAiData ctor); start_new_game applies this.
    pub fn apply_aidata_enable_repulsors(&mut self) {
        self.enable_repulsors =
            crate::game_logic::host_repulsor_gate::apply_resolved_to_leftover_and_gate();
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
        // C++ ActiveBody.cpp:717 — damager->getRelationship(obj) != ENEMIES.
        let enemies = match (damager.owner_player_id, victim.owner_player_id) {
            (Some(_), Some(_)) => {
                self.object_relationship(damager, victim)
                    == gamelogic::common::Relationship::Enemies
            }
            _ => {
                damager.team != victim.team
                    && damager.team != Team::Neutral
                    && victim.team != Team::Neutral
            }
        };
        if !enemies {
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
                if **id == victim_id || **id == damager_id || !o.is_alive() {
                    return false;
                }
                // C++ PartitionFilterPlayerAffiliation ALLOW_ALLIES.
                let allied = self.object_relationship(o, &self.objects[&victim_id])
                    == gamelogic::common::Relationship::Allies
                    || (o.owner_player_id.is_none() && o.team == vteam && o.team != Team::Neutral);
                if !allied {
                    return false;
                }
                let p = o.get_position();
                let dx = p.x - vpos.x;
                let dz = p.z - vpos.z;
                dx * dx + dz * dz <= range_sq
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
    /// Tick all GuardRetaliating units (victim death / return / inner scan).
    pub fn tick_guard_retaliate_states(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| matches!(o.ai_state, AIState::GuardRetaliating))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let frames = self.host_guard_chase_unit_frames();
            let now = self.frame;
            if let Some(o) = self.objects.get_mut(&id) {
                if (o.guard_chase_phase == GUARD_CHASE_PHASE_RETALIATE || o.guard_chase_phase == 2)
                    && o.guard_chase_give_up_frame == 0
                {
                    o.guard_chase_give_up_frame = now.saturating_add(frames);
                }
            }
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
            // C++ AIGuardRetaliateOuterState::update: refresh give-up while the
            // goal stays within stdGuardRange of the retaliate center.
            if alive {
                let phase = self
                    .objects
                    .get(&id)
                    .map(|o| o.guard_chase_phase)
                    .unwrap_or(0);
                if phase == 2 {
                    if let Some(vp) = vpos {
                        let center = self
                            .objects
                            .get(&id)
                            .map(|o| {
                                o.guard_retaliate_anchor
                                    .or(o.guard_position)
                                    .unwrap_or_else(|| o.get_position())
                            })
                            .unwrap_or(vp);
                        let (inner, _) = self.host_std_guard_ranges(id);
                        let dx = vp.x - center.x;
                        let dz = vp.z - center.z;
                        if inner > 0.0 && dx * dx + dz * dz <= inner * inner {
                            let frames = self.host_guard_chase_unit_frames();
                            let now = self.frame;
                            if let Some(o) = self.objects.get_mut(&id) {
                                o.guard_chase_give_up_frame = now.saturating_add(frames);
                            }
                        }
                    }
                }
            }
            if alive && self.guard_retaliate_chase_should_exit(id, vpos) {
                let phase = self
                    .objects
                    .get(&id)
                    .map(|o| o.guard_chase_phase)
                    .unwrap_or(0);
                if phase == 1 {
                    // C++ INNER success AND failure → OUTER. Re-attack the same
                    // nemesis with 0.67*(vision+stdGuard) + give-up timer.
                    // AIGuardRetaliateInnerState::onExit: setTeamTargetObject(NULL).
                    let frames = self.host_guard_chase_unit_frames();
                    let now = self.frame;
                    if let Some(o) = self.objects.get_mut(&id) {
                        o.guard_chase_phase = 2;
                        o.guard_chase_give_up_frame = now.saturating_add(frames);
                    }
                    self.set_host_team_common_target(id, None);
                    continue;
                }
                if let Some(o) = self.objects.get_mut(&id) {
                    o.guard_retaliate_victim = None;
                    o.target = None;
                    o.status.attacking = false;
                    o.clear_guard_chase();
                    o.tick_guard_retaliate(false, None);
                }
                // C++ AIGuardRetaliate Inner/Aggressor onExit:
                // getTeam()->setTeamTargetObject(NULL).
                self.set_host_team_common_target(id, None);
                continue;
            }
            if !alive {
                let (inner, _) = self.host_std_guard_ranges(id);
                let returning = self.objects.get(&id).is_some_and(|o| {
                    let center = o
                        .guard_retaliate_anchor
                        .or(o.guard_position)
                        .unwrap_or_else(|| o.get_position());
                    let us = o.get_position();
                    let dx = us.x - center.x;
                    let dz = us.z - center.z;
                    inner > 0.0 && dx * dx + dz * dz > inner * inner
                });

                if self.guard_acquire_scan_due(id, returning) {
                    if let Some(team_id) = self.host_team_common_target(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.guard_retaliate_victim = Some(team_id);
                            o.target = Some(team_id);
                            o.status.attacking = true;
                            o.guard_chase_phase = 1;
                            o.guard_chase_give_up_frame = 0;
                        }
                        continue;
                    }
                    if let Some(next) = self.scan_guard_retaliate_inner(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.guard_retaliate_victim = Some(next);
                            o.target = Some(next);
                            o.status.attacking = true;
                            o.guard_chase_phase = 1;
                            o.guard_chase_give_up_frame = 0;
                        }
                        continue;
                    }
                }
            }

            // C++ hasAttackedMeAndICanReturnFire on RETURN/IDLE.
            let last = self
                .objects
                .get_mut(&id)
                .and_then(|o| o.last_damage_source.take());
            if let Some(aid) = last {
                if aid != id {
                    let legal = self
                        .objects
                        .get(&aid)
                        .is_some_and(|a| a.is_alive() && !a.status.destroyed)
                        && matches!(
                            self.get_able_to_attack_specific_object(
                                id,
                                aid,
                                AbleToAttackType::NewTarget,
                                false,
                            ),
                            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                        )
                        && self.objects.get(&aid).is_some_and(|a| {
                            self.objects
                                .get(&id)
                                .is_some_and(|me| a.is_targetable_by_enemy_of(me.team))
                        });
                    if legal {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.guard_retaliate_victim = Some(aid);
                            o.target = Some(aid);
                            o.status.attacking = true;
                            o.guard_chase_phase = GUARD_CHASE_PHASE_RETALIATE;
                            o.guard_chase_give_up_frame = 0;
                        }
                        continue;
                    }
                }
            }
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

    /// C++ `chooseLocomotorSet(PANIC/WANDER)` on one host unit.
    pub fn apply_unit_locomotor_set(&mut self, unit_id: ObjectId, set: &str) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        use crate::game_logic::host_upgrade_module_residuals::{
            HostLocomotorSetKind, apply_choose_locomotor_set,
        };
        let (kind, panicking) = match set.trim().to_ascii_lowercase().as_str() {
            "panic" => (HostLocomotorSetKind::Panic, true),
            "wander" => (HostLocomotorSetKind::Wander, false),
            "normal" => (HostLocomotorSetKind::Normal, false),
            "taxiing" | "set_taxiing" => (HostLocomotorSetKind::Taxiing, false),
            _ => return false,
        };
        apply_choose_locomotor_set(u, kind, panicking)
    }

    /// C++ TEAM_PANIC / TEAM_WANDER member loop residual.
    pub fn apply_team_locomotor_set(&mut self, team_name: &str, set: &str) -> usize {
        let needle = team_name.trim();
        let faction = Self::resolve_host_team_name(team_name);
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|obj| {
                obj.is_alive()
                    && (faction.map(|t| obj.team == t).unwrap_or(false)
                        || (!obj.team_instance_name.is_empty()
                            && obj.team_instance_name.eq_ignore_ascii_case(needle))
                        || obj.team.get_name().eq_ignore_ascii_case(needle))
            })
            .map(|obj| obj.id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if self.apply_unit_locomotor_set(id, set) {
                n += 1;
            }
        }
        n
    }

    /// C++ ScriptActions::doTeamPanic residual.
    pub fn set_team_panic_by_name(&mut self, team_name: &str) -> usize {
        self.apply_team_locomotor_set(team_name, "panic")
    }

    /// Named-unit panic residual (TEAM_PANIC member / UNIT_PANIC).
    pub fn set_named_unit_panic(&mut self, unit_name: &str) -> bool {
        let Some(id) = self.find_object_id_by_name(unit_name) else {
            return false;
        };
        self.apply_unit_locomotor_set(id, "panic")
    }

    /// Drain leftover TEAM_PANIC / TEAM_WANDER / named-unit set swaps.
    /// C++ `doTeamWander` / `doTeamPanic` / `doTeamWanderInPlace`: chooseLocomotorSet
    /// then `aiWander` / `aiPanic` / `aiWanderInPlace` (`CMD_FROM_SCRIPT`).
    pub fn apply_host_loco_set_script_requests(&mut self) {
        self.apply_host_team_factory_script_requests();
        self.apply_host_merge_team_script_requests();
        self.apply_host_capture_nearest_unowned_script_requests();
        self.apply_host_load_transports_script_requests();
        self.apply_host_named_fire_weapon_path_script_requests();
        for (team_name, set, waypoint) in gamelogic::scripting::take_host_team_loco_set_requests() {
            self.host_script_apply_team_wander_panic(&team_name, &set, waypoint.as_deref());
        }
        for (unit_name, set) in gamelogic::scripting::take_host_unit_loco_set_requests() {
            if let Some(id) = self.find_object_id_by_name(&unit_name) {
                let _ = self.apply_unit_locomotor_set(id, &set);
            }
        }
        self.tick_host_wander_in_place();
    }

    /// C++ `doTeamWander` / `doTeamPanic` member loop: closest waypoint on path,
    /// then wander/panic locomotor + follow. Missing path returns like C++.
    fn host_script_apply_team_wander_panic(
        &mut self,
        team_name: &str,
        set: &str,
        waypoint: Option<&str>,
    ) {
        let needle = team_name.trim();
        let faction = Self::resolve_host_team_name(team_name);
        let members: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|obj| {
                obj.is_alive()
                    && (faction.map(|t| obj.team == t).unwrap_or(false)
                        || (!obj.team_instance_name.is_empty()
                            && obj.team_instance_name.eq_ignore_ascii_case(needle))
                        || obj.team.get_name().eq_ignore_ascii_case(needle))
            })
            .map(|obj| obj.id)
            .collect();
        for id in members {
            let Some(pos) = self
                .host_object(id)
                .filter(|u| u.is_alive() && u.can_move())
                .map(|u| u.get_position())
            else {
                continue;
            };
            if let Some(label) = waypoint {
                let Some(path) = self.host_wander_waypoint_path_from(label, pos) else {
                    // C++ doTeamWander/doTeamPanic: first missing waypoint returns.
                    return;
                };
                let _ = self.apply_unit_locomotor_set(id, set);
                self.host_wander_issue_path(id, &path);
            } else {
                let _ = self.apply_unit_locomotor_set(id, set);
                self.host_wander_in_place(id, pos);
            }
        }
    }

    /// C++ `TheTerrainLogic->getClosestWaypointOnPath` then `link[0]` chain.
    fn host_wander_waypoint_path_from(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<Vec<glam::Vec3>> {
        let leftover_pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
        let start = terrain.get_closest_waypoint_on_path(&leftover_pos, path_label)?;
        let chain = terrain.walk_link0_chain(start, gamelogic::terrain::WAYPOINT_PATH_LIMIT);
        if chain.is_empty() {
            return None;
        }
        Some(
            chain
                .into_iter()
                .map(|wp| {
                    let loc = *wp.get_location();
                    let mut pos = glam::Vec3::new(loc.x, loc.z, loc.y);
                    if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                        pos.y = h;
                    }
                    pos
                })
                .collect(),
        )
    }

    /// C++ `aiWander` / `aiPanic` follow the waypoint path as individuals.
    /// Leftover `update_group_offset` spreads the issued dest by WanderWidthFactor.
    fn host_wander_issue_path(&mut self, id: ObjectId, waypoints: &[glam::Vec3]) {
        if waypoints.is_empty() {
            return;
        }
        wander_in_place_lock().remove(&id.0);
        let offset = self
            .host_object(id)
            .map(|obj| leftover_wander_group_offset(obj.wander_width_factor))
            .unwrap_or(glam::Vec2::ZERO);
        let offset_pts: Vec<glam::Vec3> = waypoints
            .iter()
            .copied()
            .map(|p| leftover_apply_wander_group_offset(p, offset))
            .collect();
        let goal = *offset_pts.last().unwrap();
        let via = &offset_pts[..offset_pts.len().saturating_sub(1)];
        let _ = self.unit_command_waypoint_path_prep(id, false);
        let _ = self.assign_unit_path(id, goal, via);
        wander_path_lock().insert(
            id.0,
            HostWanderPath {
                timer: 0,
                wait_frames: leftover_wander_wait_frames(id),
            },
        );
    }

    /// C++ `AIWanderInPlaceState::chooseNewGoal` — loco radius, re-pick each hop.
    fn host_wander_in_place(&mut self, id: ObjectId, origin: glam::Vec3) {
        wander_path_lock().remove(&id.0);
        let dest = match self.host_object(id) {
            Some(obj) => host_wander_choose_hop(obj, origin),
            None => origin,
        };
        wander_in_place_lock().insert(
            id.0,
            HostWanderInPlace {
                origin,
                hop: dest,
                timer: 0,
                wait_frames: leftover_wander_wait_frames(id),
            },
        );
        if host_wander_horiz_dist_sq(dest, origin) > 0.25 {
            let _ = self.unit_command_move_to(id, dest);
        }
    }

    /// Leftover wander/panic `find_closest_repulsor` → `STATE_FAILURE`
    /// (`AI_MOVE_AWAY_FROM_REPULSORS`). Does not require Idle.
    fn host_wander_fail_to_repulse(&mut self, unit_id: ObjectId) -> bool {
        let vision = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            if !u.is_kind_of(KindOf::CanBeRepulsed) {
                return false;
            }
            u.vision_range
        };
        let Some((rep_id, _)) = self.find_closest_repulsor(unit_id, vision) else {
            return false;
        };
        let Some(rep_pos) = self.objects.get(&rep_id).map(|r| r.get_position()) else {
            return false;
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            crate::game_logic::host_upgrade_module_residuals::apply_choose_locomotor_set(
                u,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Panic,
                true,
            );
            u.ai_move_away_from_unit(rep_id, rep_pos);
            let dest = u.move_away_destination.unwrap_or(rep_pos);
            let _ = u.begin_request_safe_path(rep_id, dest, self.frame);
            true
        } else {
            false
        }
    }

    /// C++ `AIWanderInPlaceState::update` + wander/panic leftover-bail on repulsors.
    fn tick_host_wander_in_place(&mut self) {
        let mut flee = Vec::new();

        let path_sessions: Vec<(u32, i32, i32)> = wander_path_lock()
            .iter()
            .map(|(&id, session)| (id, session.timer, session.wait_frames))
            .collect();
        if !path_sessions.is_empty() {
            let mut path_drop = Vec::new();
            let mut path_timers = Vec::new();
            for (raw, timer, wait) in path_sessions {
                let id = ObjectId(raw);
                let Some(obj) = self.host_object(id) else {
                    path_drop.push(raw);
                    continue;
                };
                if !obj.is_alive() {
                    path_drop.push(raw);
                    continue;
                }
                if !matches!(obj.ai_state, AIState::Idle | AIState::Moving) {
                    path_drop.push(raw);
                    continue;
                }
                let can_be = obj.is_kind_of(KindOf::CanBeRepulsed);
                let vision = obj.vision_range;
                let (new_timer, scan) = leftover_wander_tick_timer(can_be, timer, wait);
                if scan && leftover_wander_has_repulsor(self, id, vision) {
                    path_drop.push(raw);
                    flee.push(raw);
                    continue;
                }
                let stopped = !obj.status.moving && obj.movement.target_position.is_none();
                if stopped && matches!(obj.ai_state, AIState::Idle) {
                    path_drop.push(raw);
                    continue;
                }
                path_timers.push((raw, new_timer));
            }
            let mut map = wander_path_lock();
            for raw in path_drop {
                map.remove(&raw);
            }
            for (raw, timer) in path_timers {
                if let Some(session) = map.get_mut(&raw) {
                    session.timer = timer;
                }
            }
        }

        let sessions: Vec<(u32, glam::Vec3, glam::Vec3, i32, i32)> = wander_in_place_lock()
            .iter()
            .map(|(&id, session)| {
                (
                    id,
                    session.origin,
                    session.hop,
                    session.timer,
                    session.wait_frames,
                )
            })
            .collect();
        if sessions.is_empty() {
            for raw in flee {
                let _ = self.host_wander_fail_to_repulse(ObjectId(raw));
            }
            return;
        }
        let mut drop = Vec::new();
        let mut reissue = Vec::new();
        let mut keep_timers = Vec::new();
        for (raw, origin, hop, timer, wait) in sessions {
            let id = ObjectId(raw);
            let Some(obj) = self.host_object(id) else {
                drop.push(raw);
                continue;
            };
            if !obj.is_alive() {
                drop.push(raw);
                continue;
            }
            if !matches!(obj.ai_state, AIState::Idle | AIState::Moving) {
                drop.push(raw);
                continue;
            }
            let can_be = obj.is_kind_of(KindOf::CanBeRepulsed);
            let vision = obj.vision_range;
            let (new_timer, scan) = leftover_wander_tick_timer(can_be, timer, wait);
            if scan && leftover_wander_has_repulsor(self, id, vision) {
                drop.push(raw);
                flee.push(raw);
                continue;
            }
            if let Some(dest) = obj.requested_destination.or(obj.movement.target_position) {
                if host_wander_horiz_dist_sq(dest, hop) > (3.0 * HOST_WANDER_CELL).powi(2) {
                    drop.push(raw);
                    continue;
                }
            }
            let pos = obj.get_position();
            let near = host_wander_horiz_dist_sq(pos, hop) <= HOST_WANDER_CELL * HOST_WANDER_CELL;
            let stopped = !obj.status.moving && obj.movement.target_position.is_none();
            if near || (stopped && matches!(obj.ai_state, AIState::Idle)) {
                reissue.push((
                    raw,
                    origin,
                    host_wander_choose_hop(obj, origin),
                    new_timer,
                    wait,
                ));
            } else {
                keep_timers.push((raw, new_timer));
            }
        }
        {
            let mut map = wander_in_place_lock();
            for raw in drop {
                map.remove(&raw);
            }
            for (raw, origin, hop, timer, wait) in &reissue {
                map.insert(
                    *raw,
                    HostWanderInPlace {
                        origin: *origin,
                        hop: *hop,
                        timer: *timer,
                        wait_frames: *wait,
                    },
                );
            }
            for (raw, timer) in keep_timers {
                if let Some(session) = map.get_mut(&raw) {
                    session.timer = timer;
                }
            }
        }
        for (raw, origin, hop, _, _) in reissue {
            if host_wander_horiz_dist_sq(hop, origin) > 0.25 {
                let _ = self.unit_command_move_to(ObjectId(raw), hop);
            }
        }
        for raw in flee {
            let _ = self.host_wander_fail_to_repulse(ObjectId(raw));
        }
    }

    /// Drain leftover `TEAM_SET_ATTITUDE` onto live host members.
    pub fn apply_host_team_attitude_script_requests(&mut self) {
        for (team_name, mood) in gamelogic::scripting::take_host_team_attitude_requests() {
            let _ = self.set_team_attitude_by_name(&team_name, &mood.to_string());
        }
    }

    /// Drain leftover `BUILD_TEAM` / `RECRUIT_TEAM` onto host AIPlayer.
    /// C++ `ScriptActions::doBuildTeam` / `doRecruitTeam`.
    fn apply_host_team_factory_script_requests(&mut self) {
        let builds = gamelogic::scripting::take_host_build_team_requests();
        let recruits = gamelogic::scripting::take_host_recruit_team_requests();
        if builds.is_empty() && recruits.is_empty() {
            return;
        }
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        for (owner, team) in builds {
            let _ = ai_mgr.build_specific_ai_team_for_token(self, &owner, &team, true);
        }
        for (owner, team, radius) in recruits {
            let _ = ai_mgr.recruit_specific_ai_team_for_token(self, &owner, &team, radius);
        }
        self.ai_manager = ai_mgr;
    }

    /// C++ `ScriptActions::doMergeTeamIntoTeam` live drain.
    /// Leftover `set_team` misses empty `OBJECT_REGISTRY`; rewrite live
    /// `team_instance_name` so TEAM/NAMED census follows dest.
    pub fn apply_host_merge_team_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_script_merge_team_requests() {
            self.host_script_merge_team_into_team(&req.source, &req.dest);
        }
    }

    /// C++ `obj->setTeam(teamDest)` + `updateTeamAndPlayerStuff` + dest `setActive`.
    fn host_script_merge_team_into_team(&mut self, source: &str, dest: &str) {
        let source = source.trim();
        let dest = dest.trim();
        if source.is_empty() || dest.is_empty() || source.eq_ignore_ascii_case(dest) {
            return;
        }
        let dest_owner = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| {
                factory
                    .find_team(dest)
                    .or_else(|| factory.create_team(dest))
            })
            .and_then(|team| {
                team.read()
                    .ok()
                    .and_then(|guard| guard.get_controlling_player_id())
            });
        let ids: Vec<ObjectId> = self
            .host_script_team_census_member_ids(source)
            .into_iter()
            .map(ObjectId)
            .collect();
        for id in ids {
            if let Some(obj) = self.host_object_mut(id) {
                obj.team_instance_name = dest.to_string();
                if let Some(pid) = dest_owner {
                    obj.owner_player_id = Some(pid);
                }
                // C++ obj->setTeam(teamDest) applies dest proto attitude.
                obj.apply_named_team_ai_profile(true);
            }
            self.activate_leftover_team_for_host_object(id);
        }
    }

    /// C++ `ScriptActions::doTeamCaptureNearestUnownedFactionUnit` live drain.
    /// Leftover partition is empty on the player path.
    pub fn apply_host_capture_nearest_unowned_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_script_capture_nearest_unowned_requests() {
            self.host_script_team_capture_nearest_unowned(&req.team);
        }
    }

    /// C++ AIGroup center + `getClosestObject` (unmanned + enemies/neutral +
    /// on-map) + `groupEnter(..., CMD_FROM_SCRIPT)`.
    fn host_script_team_capture_nearest_unowned(&mut self, team_name: &str) {
        let members: Vec<ObjectId> = self
            .host_script_team_census_member_ids(team_name)
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.host_object(*id)
                    .is_some_and(|obj| obj.is_alive() && !obj.status.destroyed)
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let mut sum = glam::Vec3::ZERO;
        let mut n = 0u32;
        let mut source_owner = None;
        let mut source_inst = String::new();
        for id in &members {
            let Some(obj) = self.host_object(*id) else {
                continue;
            };
            sum += obj.get_position();
            n += 1;
            if source_owner.is_none() {
                source_owner = obj.owner_player_id;
                source_inst = obj.team_instance_name.clone();
            }
        }
        if n == 0 {
            return;
        }
        let center = sum / n as f32;
        let Some(target_id) = self.host_script_closest_unowned_faction_unit(
            center,
            &members,
            source_owner,
            &source_inst,
        ) else {
            return;
        };
        let Some(target_pos) = self.host_object(target_id).map(|obj| obj.get_position()) else {
            return;
        };
        for id in members {
            let _ = self.unit_command_stop_moving_order_target(id, Some(target_id));
            if !self.unit_command_path_with_state_ignoring(
                id,
                target_pos,
                AIState::Entering,
                Some(target_id),
            ) {
                if let Some(obj) = self.host_object_mut(id) {
                    obj.ignored_obstacle_id = Some(target_id);
                    obj.set_ai_state(AIState::Entering);
                }
            }
        }
    }

    /// C++ `ScriptActions::doLoadAllTransports` live drain.
    /// Leftover BinPartitionSolver + `chooseLocomotorSet(NORMAL)` + `aiEnter`.
    pub fn apply_host_load_transports_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_script_load_transports_requests() {
            self.host_script_team_load_transports(&req.team);
        }
    }

    /// C++ team member walk → leftover `PartitionSolver(PREFER_FAST)` → enter.
    fn host_script_team_load_transports(&mut self, team_name: &str) {
        use game_engine::common::partition_solver::{
            BinPartitionSolver, EntriesVec, SolutionType, SpacesVec,
        };

        let members = self.host_script_team_census_member_ids(team_name);
        let mut units = EntriesVec::new();
        let mut transports = SpacesVec::new();
        for id in members {
            let Some(obj) = self.host_object(ObjectId(id)) else {
                continue;
            };
            if obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Transport) {
                transports.push((id, obj.max_transport as u32));
            } else {
                units.push((id, obj.transport_slot_count() as u32));
            }
        }
        let mut solver =
            BinPartitionSolver::new(units, transports, SolutionType::PreferFastSolution);
        solver.solve();
        for (unit_id, transport_id) in solver.get_solution() {
            let unit = ObjectId(*unit_id);
            let trans = ObjectId(*transport_id);
            let Some(pos) = self.host_object(trans).map(|obj| obj.get_position()) else {
                continue;
            };
            let _ = self.apply_unit_locomotor_set(unit, "normal");
            let _ = self.unit_command_stop_moving_order_target(unit, Some(trans));
            if !self.unit_command_path_with_state_ignoring(
                unit,
                pos,
                AIState::Entering,
                Some(trans),
            ) {
                if let Some(obj) = self.host_object_mut(unit) {
                    obj.ignored_obstacle_id = Some(trans);
                    obj.set_ai_state(AIState::Entering);
                }
            }
        }
    }

    /// C++ `doNamedFireWeaponFollowingWaypointPath` live drain.
    /// Leftover forceFire + leftover follow-waypoint-path on the live projectile.
    pub fn apply_host_named_fire_weapon_path_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_script_named_fire_weapon_path_requests() {
            self.host_script_named_fire_weapon_following_path(&req.unit, &req.waypoint);
        }
    }

    /// C++ `findWaypointFollowingCapableWeapon` + `forceFireWeapon` at unit pos,
    /// then projectile `leaveGroup` + `chooseLocomotorSet(NORMAL)` +
    /// `aiFollowWaypointPath(..., CMD_FROM_SCRIPT)`.
    fn host_script_named_fire_weapon_following_path(&mut self, unit: &str, waypoint: &str) {
        use crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name;

        let Some(id) = self.host_object_id_by_script_name(unit) else {
            return;
        };
        let Some((pos, team, weapon_name)) = self.host_object(id).and_then(|obj| {
            if !obj.is_alive() || obj.status.destroyed {
                return None;
            }
            let weapon_name = self.host_script_waypoint_following_weapon_name(obj)?;
            Some((obj.get_position(), obj.team, weapon_name))
        }) else {
            return;
        };
        let Some(path) = self.host_wander_waypoint_path_from(waypoint, pos) else {
            return;
        };
        let proj_name = host_projectile_name_for_weapon_name(&weapon_name);
        if proj_name.is_empty() {
            return;
        }
        if !self.templates.contains_key(&proj_name) {
            let mut tmpl = ThingTemplate::new(&proj_name);
            tmpl.set_health(100.0)
                .add_kind_of(KindOf::Projectile)
                .add_kind_of(KindOf::Aircraft);
            self.templates.insert(proj_name.clone(), tmpl);
        }
        let Some(proj_id) = self.create_object(&proj_name, team, pos) else {
            return;
        };
        if let Some(obj) = self.host_object_mut(proj_id) {
            obj.note_producer(id);
            obj.set_formation(0, glam::Vec2::ZERO);
        }
        let _ = self.apply_unit_locomotor_set(proj_id, "normal");
        if path.is_empty() {
            return;
        }
        let goal = *path.last().unwrap();
        let via = &path[..path.len().saturating_sub(1)];
        let _ = self.unit_command_waypoint_path_prep(proj_id, false);
        if self.assign_unit_path(proj_id, goal, via) {
            if let Some(obj) = self.host_object_mut(proj_id) {
                obj.stamp_pending_waypoint_labels(std::iter::once(waypoint.to_string()));
            }
        }
    }

    /// Leftover `WeaponSet::findWaypointFollowingCapableWeapon` (TERTIARY..PRIMARY)
    /// on leftover store `CapableOfFollowingWaypoints`. Name seeds are never consulted.
    fn host_script_waypoint_following_weapon_name(
        &self,
        obj: &crate::game_logic::object::Object,
    ) -> Option<String> {
        obj.find_waypoint_following_capable_weapon_slot()
            .and_then(|slot| obj.weapon_name_for_slot(slot).map(str::to_string))
    }

    /// C++ `PartitionFilterUnmannedObject` + `ALLOW_ENEMIES|ALLOW_NEUTRAL` +
    /// `PartitionFilterOnMap`, `FROM_CENTER_2D`.
    fn host_script_closest_unowned_faction_unit(
        &self,
        center: glam::Vec3,
        exclude: &[ObjectId],
        source_owner: Option<u32>,
        source_inst: &str,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_deliver_payload::is_off_map_default_residual;
        use gamelogic::common::Relationship;

        let mut best: Option<(ObjectId, f32)> = None;
        for (id, obj) in self.host_objects() {
            if exclude.contains(id) {
                continue;
            }
            if !obj.is_alive() || obj.status.destroyed || !obj.is_unmanned() {
                continue;
            }
            if is_off_map_default_residual(obj.get_position()) {
                continue;
            }
            // C++ kill-pilot moves the husk to Neutral; affiliation is Neutral
            // even when `unmanned_owner_player_id` is retained for recrew.
            let rel = if obj.team == Team::Neutral || obj.is_unmanned() {
                Relationship::Neutral
            } else {
                Self::object_relationship_from_owners(
                    &self.players,
                    source_owner,
                    source_inst,
                    obj.owner_player_id,
                    &obj.team_instance_name,
                )
            };
            if !matches!(rel, Relationship::Enemies | Relationship::Neutral) {
                continue;
            }
            let pos = obj.get_position();
            let dx = pos.x - center.x;
            let dz = pos.z - center.z;
            let dist2 = dx * dx + dz * dz;
            if best.is_none_or(|(_, best_d)| dist2 < best_d) {
                best = Some((*id, dist2));
            }
        }
        best.map(|(id, _)| id)
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

    /// C++ crate pickup is wired into Idle, Hunt, Guard, Attack-Move, and
    /// GuardRetaliate — not Idle alone.
    pub fn try_idle_crate_pickup(&mut self, unit_id: ObjectId) -> bool {
        let (crate_id, keep_parent_state) = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            let parent_ok = matches!(
                u.ai_state,
                AIState::Idle
                    | AIState::Patrolling
                    | AIState::GuardingArea
                    | AIState::GuardingObject
                    | AIState::AttackMoving
                    | AIState::GuardRetaliating
            );
            if !parent_ok || u.target.is_some() {
                return false;
            }
            let keep = !matches!(u.ai_state, AIState::Idle);
            match u.check_for_crate_to_pickup() {
                Some(id) => (id, keep),
                None => return false,
            }
        };
        let crate_alive = self
            .objects
            .get(&crate_id)
            .map(|c| c.is_alive() && !c.status.destroyed)
            .unwrap_or(false);
        if !crate_alive {
            return false;
        }
        let is_money = self.host_money_crates.get(crate_id).is_some();
        if !is_money {
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
            u.movement.target_position = Some(pos);
            u.set_status_moving(true);
            u.requested_victim_id = Some(crate_id);
            crate::game_logic::host_move_log::record(unit_id, Some([pos.x, pos.y, pos.z]));
            if !keep_parent_state {
                u.set_ai_state(AIState::Moving);
            }
        } else {
            return false;
        }
        if !keep_parent_state && crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
        }
        true
    }
}

#[cfg(test)]
impl GameLogic {
    pub fn test_host_wander_in_place(&mut self, id: ObjectId, origin: glam::Vec3) {
        self.host_wander_in_place(id, origin);
    }

    pub fn test_tick_host_wander_in_place(&mut self) {
        self.tick_host_wander_in_place();
    }

    pub fn test_wander_in_place_hop(&self, id: ObjectId) -> Option<glam::Vec3> {
        wander_in_place_lock().get(&id.0).map(|session| session.hop)
    }

    pub fn test_host_wander_issue_path(&mut self, id: ObjectId, waypoints: &[glam::Vec3]) {
        self.host_wander_issue_path(id, waypoints);
    }

    pub fn test_wander_path_active(&self, id: ObjectId) -> bool {
        wander_path_lock().contains_key(&id.0)
    }

    pub fn test_wander_in_place_active(&self, id: ObjectId) -> bool {
        wander_in_place_lock().contains_key(&id.0)
    }

    pub fn scan_guard_retaliate_inner_for_test(&self, unit_id: ObjectId) -> Option<ObjectId> {
        self.scan_guard_retaliate_inner(unit_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, ObjectId, Player, Team, ThingTemplate};
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::ini::ini_locomotor::{LocomotorTemplate, get_locomotor_store_mut};

    fn register_wander_loco(name: &str, radius: f32) {
        let mut template = LocomotorTemplate::new(AsciiString::from(name));
        template.wander_about_point_radius = radius;
        get_locomotor_store_mut()
            .add_template(template)
            .expect("register wander loco");
    }

    fn spawn_wander_civilian(logic: &mut GameLogic, name: &str) -> ObjectId {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::CanBeRepulsed);
        logic.templates.insert(name.to_string(), tmpl);
        let id = logic
            .create_object(name, Team::USA, glam::Vec3::new(100.0, 0.0, 200.0))
            .expect("spawn wander civilian");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.owner_player_id = Some(1);
        }
        id
    }

    #[test]
    fn wander_in_place_uses_loco_radius_and_repicks() {
        register_wander_loco("WanderHumanLocomotor", 50.0);
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let id = spawn_wander_civilian(&mut logic, "CivilianInfantryWanderRadius");
        assert!(logic.apply_unit_locomotor_set(id, "wander"));

        let origin = logic.host_object(id).unwrap().get_position();
        logic.test_host_wander_in_place(id, origin);
        let hop = logic
            .test_wander_in_place_hop(id)
            .expect("wander session started");
        let dist = host_wander_horiz_dist_sq(hop, origin).sqrt();
        assert!(
            dist <= 50.0 + 0.01,
            "hop {hop:?} must stay inside WanderAboutPointRadius 50, dist={dist}"
        );

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(hop);
            obj.movement.target_position = None;
            obj.requested_destination = None;
            obj.status.moving = false;
            obj.set_ai_state(AIState::Idle);
        }
        logic.test_tick_host_wander_in_place();
        let next = logic
            .test_wander_in_place_hop(id)
            .expect("C++ never leaves wander-in-place until told");
        let next_dist = host_wander_horiz_dist_sq(next, origin).sqrt();
        assert!(
            next_dist <= 50.0 + 0.01,
            "re-pick {next:?} must stay inside radius 50, dist={next_dist}"
        );
    }

    #[test]
    fn wander_in_place_delta_falls_back_to_three_cells_without_loco() {
        let mut tmpl = ThingTemplate::new("NoLocoWanderUnit");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut obj = crate::game_logic::object::Object::new(tmpl, ObjectId(42), Team::USA);
        obj.cur_locomotor_name = None;
        obj.jet_ai.cur_locomotor_set = None;
        assert_eq!(host_wander_about_point_delta(&obj), 3);
    }

    #[test]
    fn leftover_wander_group_offset_zero_when_factor_non_positive() {
        assert_eq!(leftover_wander_group_offset(0.0), glam::Vec2::ZERO);
        assert_eq!(leftover_wander_group_offset(-1.0), glam::Vec2::ZERO);
    }

    #[test]
    fn leftover_wander_group_offset_is_cell_multiples_within_delta() {
        let factor = 2.0_f32;
        let delta = (factor + 0.5).floor() as i32;
        let offset = leftover_wander_group_offset(factor);
        let cell = HOST_WANDER_CELL;
        for component in [offset.x, offset.y] {
            let cells = component / cell;
            assert!(
                (cells - cells.round()).abs() < 1.0e-4,
                "offset {component} must be a PATHFIND_CELL_SIZE_F multiple"
            );
            assert!(
                cells.abs() <= delta as f32 + 1.0e-4,
                "offset {component} outside leftover delta {delta}"
            );
        }
    }

    #[test]
    fn host_wander_issue_path_offsets_goal_by_wander_width_factor() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let id = spawn_wander_civilian(&mut logic, "CivilianInfantryWanderWidth");
        let goal = glam::Vec3::new(400.0, 0.0, 500.0);
        let via = glam::Vec3::new(250.0, 0.0, 350.0);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.wander_width_factor = 0.0;
            obj.movement.max_speed = 5.0;
        }
        logic.test_host_wander_issue_path(id, &[via, goal]);
        let exact = logic
            .host_object(id)
            .and_then(|o| o.movement.target_position)
            .expect("zero-factor dest");
        assert!(
            host_wander_horiz_dist_sq(exact, goal) < 0.01,
            "WanderWidthFactor 0 must keep the exact leftover goal, got {exact:?}"
        );

        if let Some(obj) = logic.host_object_mut(id) {
            obj.wander_width_factor = 2.0;
        }
        let seed = game_engine::common::random_value::get_game_logic_random_seed_state();
        let expected = leftover_wander_group_offset(2.0);
        game_engine::common::random_value::set_game_logic_random_seed_state(seed);
        logic.test_host_wander_issue_path(id, &[via, goal]);
        let dest = logic
            .host_object(id)
            .and_then(|o| o.movement.target_position)
            .expect("offset dest");
        let want = leftover_apply_wander_group_offset(goal, expected);
        assert!(
            host_wander_horiz_dist_sq(dest, want) < 0.01,
            "leftover group offset dest {dest:?} != {want:?} (offset {expected:?})"
        );
        assert!(logic.test_wander_path_active(id));
    }

    fn spawn_flagged_repulsor(logic: &mut GameLogic, id: ObjectId, pos: glam::Vec3) {
        let mut tmpl = ThingTemplate::new("WanderRepulsorThreat");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut enemy = crate::game_logic::object::Object::new(tmpl, id, Team::GLA);
        enemy.set_position(pos);
        enemy.set_status_repulsor(true);
        logic.objects.insert(id, enemy);
    }

    #[test]
    fn wander_in_place_leftover_bails_on_repulsor_mid_hop() {
        register_wander_loco("WanderHumanLocomotorBail", 50.0);
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.set_enable_repulsors(true);
        let id = spawn_wander_civilian(&mut logic, "CivilianInfantryWanderBail");
        assert!(logic.apply_unit_locomotor_set(id, "wander"));
        let origin = logic.host_object(id).unwrap().get_position();
        if let Some(obj) = logic.host_object_mut(id) {
            obj.vision_range = 200.0;
            obj.movement.max_speed = 5.0;
        }
        logic.test_host_wander_in_place(id, origin);
        let hop = logic
            .test_wander_in_place_hop(id)
            .expect("wander session started");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_ai_state(AIState::Moving);
            obj.status.moving = true;
            obj.movement.target_position = Some(hop);
            obj.requested_destination = Some(hop);
        }
        spawn_flagged_repulsor(
            &mut logic,
            ObjectId(4229),
            glam::Vec3::new(origin.x + 20.0, 0.0, origin.z),
        );

        logic.test_tick_host_wander_in_place();
        assert!(
            !logic.test_wander_in_place_active(id),
            "leftover find_closest_repulsor must drop mid-hop wander"
        );
        let civ = logic.host_object(id).expect("civ");
        assert_eq!(civ.move_away_from, Some(ObjectId(4229)));
        assert!(civ.move_away_frames > 0);
        assert!(
            civ.is_panicking,
            "leftover-bail must enter MOVE_AWAY_FROM_REPULSORS"
        );
    }

    #[test]
    fn wander_path_leftover_bails_on_repulsor_mid_hop() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.set_enable_repulsors(true);
        let id = spawn_wander_civilian(&mut logic, "CivilianInfantryWanderPathBail");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.vision_range = 200.0;
            obj.movement.max_speed = 5.0;
            obj.wander_width_factor = 0.0;
        }
        let origin = logic.host_object(id).unwrap().get_position();
        let goal = glam::Vec3::new(origin.x + 200.0, origin.y, origin.z);
        logic.test_host_wander_issue_path(id, &[goal]);
        assert!(logic.test_wander_path_active(id));
        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_ai_state(AIState::Moving);
            obj.status.moving = true;
        }
        spawn_flagged_repulsor(
            &mut logic,
            ObjectId(4230),
            glam::Vec3::new(origin.x + 15.0, 0.0, origin.z),
        );

        logic.test_tick_host_wander_in_place();
        assert!(
            !logic.test_wander_path_active(id),
            "mid-hop path wander must leftover-bail on repulsor"
        );
        let civ = logic.host_object(id).expect("civ");
        assert_eq!(civ.move_away_from, Some(ObjectId(4230)));
        assert!(civ.is_panicking);
    }

    fn w25_guard_weapon() -> crate::game_logic::Weapon {
        crate::game_logic::Weapon {
            range: 400.0,
            can_target_ground: true,
            can_target_air: true,
            ..Default::default()
        }
    }

    fn w25_spawn_fighter(
        logic: &mut GameLogic,
        id: ObjectId,
        name: &str,
        team: Team,
        pos: glam::Vec3,
    ) {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut obj = crate::game_logic::object::Object::new(tmpl, id, team);
        obj.set_position(pos);
        obj.vision_range = 200.0;
        obj.weapon = Some(w25_guard_weapon());
        logic.objects.insert(id, obj);
    }

    #[test]
    fn guard_inner_skips_off_map_same_map_status() {
        // C++ PartitionFilterSameMapStatus: on-map guard ignores off-map cargo/A10.
        let mut logic = GameLogic::new();
        let gid = ObjectId(25091);
        let off_id = ObjectId(25092);
        let on_id = ObjectId(25093);
        w25_spawn_fighter(&mut logic, gid, "W25GuardOn", Team::China, glam::Vec3::ZERO);
        w25_spawn_fighter(
            &mut logic,
            off_id,
            "W25CargoOff",
            Team::USA,
            glam::Vec3::new(-300.0, 80.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            on_id,
            "W25RangerOn",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        );
        let found = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            200.0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(found, Some(on_id), "off-map candidate must lose to on-map");
    }

    #[test]
    fn guard_inner_acquire_uses_from_center_2d() {
        // C++ getClosestObject(..., FROM_CENTER_2D): height is ignored.
        let mut logic = GameLogic::new();
        let gid = ObjectId(25094);
        let flyer = ObjectId(25095);
        let ground = ObjectId(25096);
        w25_spawn_fighter(&mut logic, gid, "W25Guard2d", Team::China, glam::Vec3::ZERO);
        w25_spawn_fighter(
            &mut logic,
            flyer,
            "W25Comanche",
            Team::USA,
            glam::Vec3::new(20.0, 180.0, 0.0),
        );
        if let Some(o) = logic.objects.get_mut(&flyer) {
            o.status.airborne_target = true;
        }
        w25_spawn_fighter(
            &mut logic,
            ground,
            "W25GroundFar",
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        );
        let found = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            200.0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(
            found,
            Some(flyer),
            "high flyer closer in XZ must beat farther ground unit"
        );
        assert!(
            20.0f32 * 20.0 < 80.0f32 * 80.0,
            "sanity: flyer XZ closer than ground"
        );
    }

    #[test]
    fn guard_area_scan_skips_when_trigger_stamp_expired() {
        use gamelogic::common::{AsciiString, ICoord3D};
        use gamelogic::polygon_trigger::PolygonTrigger;
        let trigger = PolygonTrigger::new(
            25097,
            AsciiString::from("W25GuardAreaStamp"),
            vec![
                ICoord3D::new(-50, -50, 0),
                ICoord3D::new(50, -50, 0),
                ICoord3D::new(50, 50, 0),
                ICoord3D::new(-50, 50, 0),
            ],
        );
        let mut logic = GameLogic::new();
        let gid = ObjectId(25098);
        let eid = ObjectId(25099);
        w25_spawn_fighter(
            &mut logic,
            gid,
            "W25AreaGuard",
            Team::China,
            glam::Vec3::ZERO,
        );
        w25_spawn_fighter(
            &mut logic,
            eid,
            "W25AreaEnemy",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 10.0),
        );
        logic.frame = 1;
        let first = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            80.0,
            false,
            false,
            false,
            Some(&trigger),
        );
        assert_eq!(first, Some(eid), "fresh occupancy must scan");
        let rate = logic.host_guard_enemy_scan_rate();
        logic.frame = 1 + rate + 5;
        let second = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            80.0,
            false,
            false,
            false,
            Some(&trigger),
        );
        assert!(
            second.is_none(),
            "C++ lookForInnerTarget returns false after stamp+scan_rate"
        );
    }

    #[test]
    fn guard_inner_skips_neutral_civilians_and_tech() {
        // C++ PartitionFilterRelationship ALLOW_ENEMIES: Neutral is never
        // auto-acquired. is_targetable_by_enemy_of(team) used to treat
        // Neutral != China as hostile.
        let mut logic = GameLogic::new();
        let gid = ObjectId(25120);
        let civ = ObjectId(25121);
        let derrick = ObjectId(25122);
        let enemy = ObjectId(25123);
        w25_spawn_fighter(&mut logic, gid, "W25GuardN", Team::China, glam::Vec3::ZERO);
        w25_spawn_fighter(
            &mut logic,
            civ,
            "W25Civilian",
            Team::Neutral,
            glam::Vec3::new(12.0, 0.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            derrick,
            "W25OilDerrick",
            Team::Neutral,
            glam::Vec3::new(18.0, 0.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            enemy,
            "W25Ranger",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        );
        let found = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            200.0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(
            found,
            Some(enemy),
            "closer Neutral civilian/tech must not beat farther Enemy"
        );
        logic.objects.remove(&enemy);
        let only_neutral = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            200.0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(
            only_neutral, None,
            "Neutral civilians/tech must not be auto-acquired"
        );
    }

    #[test]
    fn guard_inner_skips_undetected_defector() {
        // C++ lookForInnerTarget ALLOW_ENEMIES uses getRelationship.
        let mut logic = GameLogic::new();
        let gid = ObjectId(25130);
        let def_id = ObjectId(25131);
        let live = ObjectId(25132);
        w25_spawn_fighter(
            &mut logic,
            gid,
            "W25GuardDef",
            Team::China,
            glam::Vec3::ZERO,
        );
        w25_spawn_fighter(
            &mut logic,
            def_id,
            "W25FlashDefector",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        );
        if let Some(o) = logic.objects.get_mut(&def_id) {
            o.begin_undetected_defection(0, 30, false);
        }
        w25_spawn_fighter(
            &mut logic,
            live,
            "W25LiveEnemy",
            Team::USA,
            glam::Vec3::new(60.0, 0.0, 0.0),
        );
        let found = logic.scan_guard_inner_target_for_test(
            gid,
            Team::China,
            glam::Vec3::ZERO,
            200.0,
            false,
            false,
            false,
            None,
        );
        assert_eq!(
            found,
            Some(live),
            "closer undetected defector must lose to farther Enemy"
        );
    }

    #[test]
    fn guard_retaliate_inner_skips_neutral_civilians() {
        let mut logic = GameLogic::new();
        let id = ObjectId(25124);
        let civ = ObjectId(25125);
        let enemy = ObjectId(25126);
        w25_spawn_fighter(&mut logic, id, "W25RetN", Team::USA, glam::Vec3::ZERO);
        if let Some(o) = logic.objects.get_mut(&id) {
            o.guard_retaliate_anchor = Some(glam::Vec3::ZERO);
        }
        w25_spawn_fighter(
            &mut logic,
            civ,
            "W25RetCiv",
            Team::Neutral,
            glam::Vec3::new(10.0, 0.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            enemy,
            "W25RetEnemy",
            Team::GLA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        );
        assert_eq!(
            logic.scan_guard_retaliate_inner_for_test(id),
            Some(enemy),
            "retaliate inner must leftover-filter Enemies, not Neutral"
        );
        logic.objects.remove(&enemy);
        assert_eq!(
            logic.scan_guard_retaliate_inner_for_test(id),
            None,
            "Neutral civilian must not be retaliate-acquired"
        );
    }

    #[test]
    fn guard_retaliate_inner_skips_off_map_and_uses_2d() {
        let mut logic = GameLogic::new();
        let id = ObjectId(25100);
        let off_id = ObjectId(25101);
        let on_id = ObjectId(25102);
        w25_spawn_fighter(&mut logic, id, "W25RetOn", Team::USA, glam::Vec3::ZERO);
        if let Some(o) = logic.objects.get_mut(&id) {
            o.guard_retaliate_anchor = Some(glam::Vec3::ZERO);
            o.owner_player_id = Some(0);
        }
        logic.add_player(Player::new(0, Team::USA, "Human", true));
        w25_spawn_fighter(
            &mut logic,
            off_id,
            "W25A10Off",
            Team::GLA,
            glam::Vec3::new(-280.0, 120.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            on_id,
            "W25OnMap",
            Team::GLA,
            glam::Vec3::new(30.0, 0.0, 0.0),
        );
        assert_eq!(
            logic.scan_guard_retaliate_inner_for_test(id),
            Some(on_id),
            "retaliate must ignore off-map strike aircraft"
        );
    }

    #[test]
    fn guard_retaliate_rescan_uses_team_attack_common_target() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA AI", false));
        let id = ObjectId(25103);
        let dead = ObjectId(25104);
        let near = ObjectId(25105);
        let shared = ObjectId(25106);
        w25_spawn_fighter(&mut logic, id, "W25RetCommon", Team::USA, glam::Vec3::ZERO);
        if let Some(o) = logic.objects.get_mut(&id) {
            o.owner_player_id = Some(1);
            o.team_instance_name = "USA_RetaliateSquad".into();
            o.begin_guard_retaliate(dead, Some(glam::Vec3::ZERO), None);
        }
        w25_spawn_fighter(
            &mut logic,
            dead,
            "W25DeadVic",
            Team::GLA,
            glam::Vec3::new(8.0, 0.0, 0.0),
        );
        logic.objects.get_mut(&dead).unwrap().status.destroyed = true;
        logic.objects.get_mut(&dead).unwrap().health.current = 0.0;
        w25_spawn_fighter(
            &mut logic,
            near,
            "W25NearLocal",
            Team::GLA,
            glam::Vec3::new(15.0, 0.0, 0.0),
        );
        w25_spawn_fighter(
            &mut logic,
            shared,
            "W25TeamVictim",
            Team::GLA,
            glam::Vec3::new(60.0, 0.0, 0.0),
        );
        logic
            .team_common_attack_targets
            .insert("USA_RetaliateSquad".into(), shared);
        logic.guard_next_enemy_scan.insert(id, logic.frame);
        logic.tick_guard_retaliate_states();
        assert_eq!(
            logic.objects[&id].guard_retaliate_victim,
            Some(shared),
            "C++ lookForInnerTarget must seed nemesis from team common target first"
        );
    }

    #[test]
    fn team_merge_rewrites_live_team_instance_name() {
        use gamelogic::object::registry::OBJECT_REGISTRY;
        use gamelogic::scripting::{
            request_host_script_merge_team, take_host_script_merge_team_requests,
        };

        OBJECT_REGISTRY.clear();
        let _ = take_host_script_merge_team_requests();

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
        let mut tmpl = ThingTemplate::new("W29MergeRanger");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.set_health(100.0);
        logic.templates.insert("W29MergeRanger".into(), tmpl);

        let a = logic
            .create_object(
                "W29MergeRanger",
                Team::USA,
                glam::Vec3::new(10.0, 0.0, 10.0),
            )
            .expect("src a");
        let b = logic
            .create_object(
                "W29MergeRanger",
                Team::USA,
                glam::Vec3::new(20.0, 0.0, 10.0),
            )
            .expect("src b");
        let keep = logic
            .create_object(
                "W29MergeRanger",
                Team::USA,
                glam::Vec3::new(40.0, 0.0, 10.0),
            )
            .expect("dest keep");
        if let Some(obj) = logic.host_object_mut(a) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_SrcSquad".into();
        }
        if let Some(obj) = logic.host_object_mut(b) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_SrcSquad".into();
        }
        if let Some(obj) = logic.host_object_mut(keep) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_DestSquad".into();
        }

        request_host_script_merge_team("USA_SrcSquad", "USA_DestSquad");
        logic.apply_host_merge_team_script_requests();

        assert_eq!(logic.objects[&a].team_instance_name, "USA_DestSquad");
        assert_eq!(logic.objects[&b].team_instance_name, "USA_DestSquad");
        assert_eq!(logic.objects[&keep].team_instance_name, "USA_DestSquad");
        let dest = logic.host_script_team_census_member_ids("USA_DestSquad");
        assert!(dest.contains(&a.0) && dest.contains(&b.0) && dest.contains(&keep.0));
        assert!(
            logic
                .host_script_team_census_member_ids("USA_SrcSquad")
                .is_empty(),
            "census must follow rewritten team_instance_name"
        );
    }

    #[test]
    fn team_capture_nearest_unowned_enters_live_husk() {
        use crate::game_logic::AIState;
        use gamelogic::object::registry::OBJECT_REGISTRY;
        use gamelogic::scripting::{
            request_host_script_capture_nearest_unowned,
            take_host_script_capture_nearest_unowned_requests,
        };

        OBJECT_REGISTRY.clear();
        let _ = take_host_script_capture_nearest_unowned_requests();

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
        let mut ranger = ThingTemplate::new("W29HijackRanger");
        ranger.add_kind_of(KindOf::Infantry);
        ranger.set_health(100.0);
        logic.templates.insert("W29HijackRanger".into(), ranger);
        let mut tank = ThingTemplate::new("W29HijackTank");
        tank.add_kind_of(KindOf::Vehicle);
        tank.set_health(400.0);
        logic.templates.insert("W29HijackTank".into(), tank);

        let infantry = logic
            .create_object(
                "W29HijackRanger",
                Team::USA,
                glam::Vec3::new(50.0, 0.0, 50.0),
            )
            .expect("infantry");
        if let Some(obj) = logic.host_object_mut(infantry) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_HijackSquad".into();
        }

        let near = logic
            .create_object(
                "W29HijackTank",
                Team::Neutral,
                glam::Vec3::new(80.0, 0.0, 50.0),
            )
            .expect("near husk");
        if let Some(obj) = logic.host_object_mut(near) {
            obj.status.disabled_unmanned = true;
            obj.owner_player_id = None;
            obj.team_instance_name.clear();
        }
        let far = logic
            .create_object(
                "W29HijackTank",
                Team::Neutral,
                glam::Vec3::new(200.0, 0.0, 50.0),
            )
            .expect("far husk");
        if let Some(obj) = logic.host_object_mut(far) {
            obj.status.disabled_unmanned = true;
            obj.owner_player_id = None;
            obj.team_instance_name.clear();
        }
        let off = logic
            .create_object(
                "W29HijackTank",
                Team::Neutral,
                glam::Vec3::new(-40.0, 0.0, 50.0),
            )
            .expect("off-map husk");
        if let Some(obj) = logic.host_object_mut(off) {
            obj.status.disabled_unmanned = true;
            obj.owner_player_id = None;
            obj.team_instance_name.clear();
        }

        request_host_script_capture_nearest_unowned("USA_HijackSquad");
        logic.apply_host_capture_nearest_unowned_script_requests();

        let infantry = logic.host_object(infantry).expect("infantry after");
        assert_eq!(infantry.target, Some(near), "groupEnter nearest unmanned");
        assert_eq!(infantry.ai_state, AIState::Entering);
        assert_ne!(infantry.target, Some(far));
        assert_ne!(infantry.target, Some(off));
    }

    #[test]
    fn team_load_transports_enters_live_transport() {
        use crate::game_logic::AIState;
        use gamelogic::object::registry::OBJECT_REGISTRY;
        use gamelogic::scripting::{
            request_host_script_load_transports, take_host_script_load_transports_requests,
        };

        OBJECT_REGISTRY.clear();
        let _ = take_host_script_load_transports_requests();

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
        let mut ranger = ThingTemplate::new("Scr2LoadRanger");
        ranger.add_kind_of(KindOf::Infantry);
        ranger.set_health(100.0);
        ranger.transport_slot_count = Some(1);
        logic.templates.insert("Scr2LoadRanger".into(), ranger);
        let mut humvee = ThingTemplate::new("Scr2LoadHumvee");
        humvee.add_kind_of(KindOf::Vehicle);
        humvee.add_kind_of(KindOf::Transport);
        humvee.set_health(400.0);
        logic.templates.insert("Scr2LoadHumvee".into(), humvee);

        let infantry = logic
            .create_object(
                "Scr2LoadRanger",
                Team::USA,
                glam::Vec3::new(50.0, 0.0, 50.0),
            )
            .expect("infantry");
        if let Some(obj) = logic.host_object_mut(infantry) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_Convoy".into();
        }
        let transport = logic
            .create_object(
                "Scr2LoadHumvee",
                Team::USA,
                glam::Vec3::new(80.0, 0.0, 50.0),
            )
            .expect("transport");
        if let Some(obj) = logic.host_object_mut(transport) {
            obj.owner_player_id = Some(1);
            obj.team_instance_name = "USA_Convoy".into();
            obj.max_transport = 8;
        }

        request_host_script_load_transports("USA_Convoy");
        logic.apply_host_load_transports_script_requests();

        let infantry = logic.host_object(infantry).expect("infantry after");
        assert_eq!(
            infantry.target,
            Some(transport),
            "aiEnter leftover solver pair"
        );
        assert_eq!(infantry.ai_state, AIState::Entering);
    }

    #[test]
    fn named_fire_weapon_follows_live_waypoint_path() {
        use crate::game_logic::Weapon;
        use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;
        use gamelogic::object::registry::OBJECT_REGISTRY;
        use gamelogic::scripting::{
            request_host_script_named_fire_weapon_path,
            take_host_script_named_fire_weapon_path_requests,
        };
        use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};
        use gamelogic::weapon::WeaponTemplate;

        OBJECT_REGISTRY.clear();
        let _ = take_host_script_named_fire_weapon_path_requests();
        ensure_host_weapon_store();
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = store
                .find_weapon_template("ScudStormWeapon")
                .map(|wt| (**wt).clone())
                .unwrap_or_else(|| WeaponTemplate::new("ScudStormWeapon".to_string()));
            template.capable_of_following_waypoint = true;
            store.add_weapon_template(template);
        });

        let mut data = MapData::new();
        data.width = 16;
        data.height = 16;
        data.heightmap = vec![0; 256];
        data.waypoints.push(MapWaypoint {
            id: 1,
            name: "Scr2Path1".into(),
            location: Coord3D {
                x: 50.0,
                y: 50.0,
                z: 0.0,
            },
            path_label1: "Scr2Cruise".into(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
        data.waypoints.push(MapWaypoint {
            id: 2,
            name: "Scr2Path2".into(),
            location: Coord3D {
                x: 200.0,
                y: 50.0,
                z: 0.0,
            },
            path_label1: "Scr2Cruise".into(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
        data.waypoint_links.push((1, 2));
        gamelogic::terrain::get_terrain_logic()
            .write()
            .expect("terrain")
            .load_map_data(data);

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
        let mut launcher = ThingTemplate::new("Scr2ScudLauncher");
        launcher.add_kind_of(KindOf::Vehicle);
        launcher.set_health(400.0);
        launcher.primary_weapon_name = Some("ScudStormWeapon".into());
        logic.templates.insert("Scr2ScudLauncher".into(), launcher);

        let id = logic
            .create_object(
                "Scr2ScudLauncher",
                Team::USA,
                glam::Vec3::new(50.0, 0.0, 50.0),
            )
            .expect("launcher");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.owner_player_id = Some(1);
            obj.name = "NamedScud".into();
            if obj.weapon.is_none() {
                obj.weapon = Some(Weapon::default());
            }
        }

        request_host_script_named_fire_weapon_path("NamedScud", "Scr2Cruise");
        logic.apply_host_named_fire_weapon_path_script_requests();

        let proj = logic
            .host_objects()
            .values()
            .find(|obj| obj.is_kind_of(KindOf::Projectile) && obj.producer_id == Some(id))
            .expect("leftover forceFire live projectile");
        assert_eq!(proj.formation_id, 0, "leaveGroup");
        let dest = proj
            .requested_destination
            .or(proj.movement.target_position)
            .or_else(|| proj.movement.path.last().copied());
        assert!(
            dest.is_some_and(|p| (p.x - 200.0).abs() < 1.0 && (p.z - 50.0).abs() < 1.0),
            "projectile follows leftover waypoint path, dest={dest:?}"
        );
    }
}
