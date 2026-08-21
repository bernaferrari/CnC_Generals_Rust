//! Host scripts `impl GameLogic` — `unit_commands`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! pick / unit commands
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Wave 246: world-position object pick without exposing object dual-walk to callers.
    ///
    /// Priority bands mirror command_integration residual acquire:
    /// - with selection: enemy attackable, then friendly selectable, then other
    /// - without selection: own selectable first, then any other selectable
    pub fn pick_object_id_at_world(
        &self,
        origin: glam::Vec3,
        player_team: Option<Team>,
        has_selected_units: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_residual_acquire::{
            pick_best_priority_residual_target, PriorityAcquireCandidate,
        };

        let cands: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let distance = (pos - origin).length();
                let radius = base_selection_radius.max(obj.selection_radius);
                if distance > radius {
                    return None;
                }
                let priority = if has_selected_units {
                    match player_team {
                        Some(team)
                            if obj.team != team
                                && obj.is_attackable()
                                && !obj.is_kind_of(KindOf::Unattackable)
                                && !obj.status.masked =>
                        {
                            Some(0)
                        }
                        Some(team) if obj.team == team && obj.is_selectable() => Some(1),
                        _ if obj.is_attackable()
                            && !obj.is_kind_of(KindOf::Unattackable)
                            && !obj.status.masked =>
                        {
                            Some(2)
                        }
                        _ if obj.is_selectable() => Some(3),
                        _ => None,
                    }
                } else {
                    // C++ SelectionXlat.cpp:181-189 / 679-693: a point click
                    // may select a lone enemy, civilian, or allied drawable.
                    match player_team {
                        Some(team) if obj.team == team && obj.is_selectable() => Some(0),
                        Some(_) if obj.is_selectable() => Some(1),
                        None if obj.is_selectable() => Some(0),
                        _ => None,
                    }
                };
                Some(PriorityAcquireCandidate {
                    id,
                    position: pos,
                    is_alive: true,
                    priority,
                })
            })
            .collect();

        pick_best_priority_residual_target(
            ObjectId(0),
            origin,
            (origin.x, origin.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    #[inline]
    pub fn unit_is_dead_or_missing(&self, id: ObjectId) -> bool {
        match self.objects.get(&id) {
            Some(o) => !o.is_alive(),
            None => true,
        }
    }

    /// C++ `HackInternetAIUpdate::aiDoCommand`: PACKING on any new command.
    #[inline]
    fn note_hacker_ai_command(&mut self, id: ObjectId) {
        self.stop_hacker_internet_hack(id);
    }

    /// Prepare move: stop attack then assign path (fallback set_destination).
    /// Wave 230/232: stop attack residual then path or set destination + Moving.
    pub fn unit_command_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        self.note_hacker_ai_command(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        let ok = if self.assign_unit_path(id, destination, &[]) {
            true
        } else if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_destination(destination);
            true
        } else {
            false
        };
        if ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_ai_state(AIState::Moving);
            }
        }
        ok
    }

    /// Wave 232: path with waypoints after stop_attack (executor move_to residual).
    pub fn unit_command_move_to_waypoints(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        waypoints: &[glam::Vec3],
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.note_hacker_ai_command(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        self.assign_unit_path(id, destination, waypoints)
    }

    /// Wave 232: force-move — stop attack, path, force Moving state.
    pub fn unit_command_force_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.note_hacker_ai_command(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 230/232: attack target (records host attack log).
    pub fn unit_command_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        self.note_hacker_ai_command(id);
        // This is the authority boundary for a player AttackObject order.
        // Do not merely stamp `target`: C++ WeaponSet validates the concrete
        // target relationship/status and every available Weapon.ini Anti*
        // mask before an attack state is allowed to begin.
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTarget,
                true,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.set_force_attack(false);
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_ai_state(AIState::Attacking);
        drop(unit);
        if let Some(tgt) = self.objects.get_mut(&target_id) {
            tgt.add_jet_targeter(id, true, self.frame);
        }
        true

    }

    /// Wave 230/232: force-attack target (records host attack log).
    pub fn unit_command_force_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        self.note_hacker_ai_command(id);
        // C++ force attack still uses WeaponSet target legality: it changes
        // relationship/force handling, not UNATTACKABLE, MASKED, contained,
        // stealth, or exact Weapon.ini anti-mask rules.
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTargetForced,
                true,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_force_attack(true);
        unit.set_ai_state(AIState::Attacking);
        drop(unit);
        if let Some(tgt) = self.objects.get_mut(&target_id) {
            tgt.add_jet_targeter(id, true, self.frame);
        }
        true

    }

    /// Wave 230/232: full player stop (idle + clear guard/target/force + logs).
    pub fn unit_command_stop(&mut self, id: ObjectId) -> bool {
        if self.flight_deck_ai_do_command(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::Idle,
            None,
            None,
        ) {
            return true;
        }

        self.note_hacker_ai_command(id);
        let should_land = self.objects.get(&id).is_some_and(|unit| {
            unit.is_alive()
                && (unit.is_kind_of(KindOf::Aircraft) || unit.object_type == ObjectType::Aircraft)
                && unit.status.airborne_target
                && !Self::object_is_produced_at_helipad(unit)
        });
        if should_land {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.stop();
                unit.set_target(None);
                unit.set_force_attack(false);
                unit.set_guard_position(None);
                unit.set_guard_target(None);
                crate::game_logic::host_attack_log::record(id, None);
                crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
                unit.end_guard_retaliate();
                unit.hunting = false;
                unit.return_to_base_requested = true;
                unit.jet_ai.allow_interrupt_for_reload = false;
            }
            let _ = self.try_return_to_base_rearm(id);
            return true;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop();
        unit.set_target(None);
        unit.set_force_attack(false);
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        crate::game_logic::host_attack_log::record(id, None);
        crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
        unit.end_guard_retaliate();
        unit.hunting = false;
        unit.set_ai_state(AIState::Idle);
        true

    }

    pub fn unit_command_guard_position(&mut self, id: ObjectId, pos: glam::Vec3) -> bool {
        if self.flight_deck_ai_do_command(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::GuardPosition,
            None,
            Some(pos),
        ) {
            return true;
        }

        self.note_hacker_ai_command(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_position(Some(pos));
        unit.hunting = false;
        unit.set_ai_state(AIState::GuardingArea);
        unit.mark_jet_command_for_reload_interrupt(true);

        true
    }

    pub fn unit_command_guard_object(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        self.note_hacker_ai_command(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_target(Some(target_id));
        unit.hunting = false;
        unit.set_ai_state(AIState::GuardingObject);
        unit.mark_jet_command_for_reload_interrupt(true);

        true
    }

    /// Wave 231/232: attack-move via path + AttackMoving state.
    pub fn unit_command_attack_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.unit_command_attack_move_to_ex(id, destination, -1)
    }

    /// Wave 232: attack-move with max-shots + attack-path flags (executor residual).
    pub fn unit_command_attack_move_to_ex(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        self.note_hacker_ai_command(id);
        if self.flight_deck_ai_do_command(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackMoveToPosition,
            None,
            Some(destination),
        ) {
            return true;
        }

        let (can_move, can_attack) = match self.objects.get(&id) {
            Some(unit) => (
                unit.is_alive() && unit.can_move(),
                unit.can_attack() || unit.weapon.is_some(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_force_attack(false);
            unit.set_max_shots_to_fire(max_shots);
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if !path_ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(destination);
            } else {
                return false;
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if can_attack {
                unit.is_attack_path = true;
                unit.auto_acquire_when_idle = true;
                unit.set_ai_state(AIState::AttackMoving);
            } else {
                unit.is_attack_path = false;
                unit.set_ai_state(AIState::Moving);
            }
            return true;
        }
        false
    }

    /// Wave 232: promote unit onto attack-move path after waypoint follow.
    pub fn unit_command_promote_attack_path(&mut self, id: ObjectId) -> bool {
        let can_attack = self
            .objects
            .get(&id)
            .map(|u| u.is_alive() && (u.can_attack() || u.weapon.is_some()))
            .unwrap_or(false);
        if !can_attack {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if let Some(slot) = unit.find_waypoint_following_capable_weapon_slot() {
                unit.set_active_weapon_slot(slot);
            }
            unit.is_attack_path = true;
            unit.set_ai_state(AIState::AttackMoving);
            return true;
        }
        false
    }

    /// Wave 231: force-attack ground location.
    pub fn unit_command_attack_ground(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        self.note_hacker_ai_command(id);
        if !matches!(
            self.get_able_to_use_weapon_against_target(
                id,
                None,
                Some(location),
                AbleToAttackType::NewTargetForced,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 231: move helper that always leaves unit in Moving state (scatter/formation).
    pub fn unit_command_move_to_moving(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        self.note_hacker_ai_command(id);
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: dozer construct — path/destination + Constructing AI state.
    /// C++ `findGoodBuildOrRepairPosition` docks at half major radius and
    /// `ignoreObstacle(goalObject)` so the scaffold is not an A* wall.
    pub fn unit_command_begin_construct(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.note_hacker_ai_command(id);
        let (dock, ignore) = {
            let Some(dozer) = self.objects.get(&id) else {
                return false;
            };
            let dozer_pos = dozer.get_position();
            if let Some(tid) = dozer.target {
                if let Some(st) = self.objects.get(&tid) {
                    let dock = crate::game_logic::host_repair::dozer_repair_approach_position(
                        dozer_pos,
                        st.get_position(),
                        st.selection_radius,
                    );
                    (dock, Some(tid))
                } else {
                    (location, None)
                }
            } else {
                (location, None)
            }
        };
        if !self.assign_unit_path_ignoring(id, dock, &[], ignore) {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(dock);
            } else {
                return false;
            }
        }
        // C++ WorkerAIUpdate::newTask — drop preferred dock and leave supply mode.
        self.worker_exit_supply_for_dozer_task(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Constructing);
            unit.set_ultra_accurate(true);
            return true;
        }
        false
    }

    /// Additive selection mark on a selectable unit controlled by this player.
    pub fn unit_select_if_player(&mut self, id: ObjectId, player_id: u32) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.owner_player_id == Some(player_id) && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
            true
        } else {
            false
        }
    }

    /// Compatibility path for faction-only legacy callers. New player-command
    /// paths must use `unit_select_if_player` so duplicate factions stay apart.
    pub fn unit_select_if_team(&mut self, id: ObjectId, player_team: Team) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.team == player_team && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
            true
        } else {
            false
        }
    }

    /// Wave 232: path after stop_attack; optionally clear formation id (free move).
    pub fn unit_command_move_clear_formation(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        clear_formation: bool,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if clear_formation && unit.formation_id != 0 {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 232: tighten-group prep — stop attack, clear formation/guard, then Moving path.
    pub fn unit_command_tighten_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_formation(0, glam::Vec2::ZERO);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: stamp formation id + offset (create/dissolve).
    pub fn unit_command_set_formation(
        &mut self,
        id: ObjectId,
        formation_id: u32,
        offset: glam::Vec2,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_formation(formation_id, offset);
        true
    }

    /// Wave 232: full guard order (position or object) with radius/mode.
    /// Returns whether unit accepted the order; caller may still path.
    pub fn unit_command_guard_full(
        &mut self,
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        guard_radius: f32,
        mode: GuardMode,
    ) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        // For object guard, require living target position when provided.
        let (gpos, gtarget, ai_state) = if let Some(tid) = target {
            let tpos = self
                .objects
                .get(&tid)
                .filter(|o| o.is_alive())
                .map(|o| o.get_position());
            if tpos.is_none() {
                return false;
            }
            (
                tpos.map(|p| [p.x, p.y, p.z]),
                tid.0,
                AIState::GuardingObject,
            )
        } else if let Some(pos) = position {
            (Some([pos.x, pos.y, pos.z]), 0u32, AIState::GuardingArea)
        } else {
            return false;
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.guard_radius = guard_radius;
            unit.set_guard_mode(mode);
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.end_guard_retaliate();
            unit.hunting = false;
            if let Some(tid) = target {
                unit.guard_position = None;
                unit.set_guard_target(Some(tid));
            } else if let Some(pos) = position {
                unit.set_guard_target(None);
                unit.set_guard_position(Some(pos));
            }
            unit.set_ai_state(ai_state);
            crate::game_logic::host_guard_log::record(id, gpos, gtarget, guard_radius);
            crate::game_logic::host_attack_log::record(id, None);
            return true;
        }
        false
    }

    /// Wave 232: set guard radius only (guard area residual).
    pub fn unit_command_set_guard_radius(&mut self, id: ObjectId, radius: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.guard_radius = radius;
        true
    }

    /// Wave 232: attack-ground with max shots + force flag.
    pub fn unit_command_attack_ground_ex(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        if !matches!(
            self.get_able_to_use_weapon_against_target(
                id,
                None,
                Some(location),
                AbleToAttackType::NewTargetForced,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(None);
        unit.set_force_attack(true);
        unit.set_max_shots_to_fire(max_shots);
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 232: hunt/patrol residual.
    pub fn unit_command_patrol(&mut self, id: ObjectId) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
            crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
            crate::game_logic::host_attack_log::record(id, None);
            unit.auto_acquire_when_idle = true;
            unit.hunting = true;
            unit.set_ai_state(AIState::Patrolling);
            unit.mark_jet_command_for_reload_interrupt(true);

            unit.set_status_moving(false);
            return true;
        }
        false
    }

    /// Wave 232: cheer model-condition residual.
    pub fn unit_command_cheer(
        &mut self,
        id: ObjectId,
        cheer_secs: f32,
        cheer_bit: Option<usize>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.begin_cheer(cheer_secs, cheer_bit);
        true
    }

    /// Wave 232: toggle deployed status for deploy-style units.
    pub fn unit_command_set_deployed(&mut self, id: ObjectId, deployed: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_deployed(deployed);
        if deployed {
            unit.set_ai_state(AIState::Idle);
        }
        true
    }

    /// Execute an explicit Deploy toggle for an object that owns an exact
    /// `DeployStyleAIUpdate` module.  The command is authoritative here as
    /// well as in the executor: stale UI input cannot turn an arbitrary
    /// vehicle into a DeployStyle unit merely by sharing a familiar name.
    ///
    /// C++ reverses an in-progress deploy/undeploy when the opposing intent
    /// arrives.  The compact host state's two directions preserve that timing
    /// behavior; OBJECT_STATUS_DEPLOYED only changes when unpack finishes and
    /// clears immediately when packing begins.
    pub fn unit_command_toggle_deploy_style(&mut self, id: ObjectId) -> bool {
        let frame = self.frame;
        let mut deployed_direction = false;
        let changed = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            if !unit.is_alive() || unit.get_template().deploy_style_metadata.is_none() {
                return false;
            }
            let Some(style) = unit.deploy_style.as_mut() else {
                // A template with metadata must have installed state during
                // normal construction.  A malformed/legacy snapshot without
                // it is not safe to silently reconstruct mid-command.
                return false;
            };

            deployed_direction = matches!(
                style.state,
                crate::game_logic::host_deploy_style::HostDeployStyleState::ReadyToMove
                    | crate::game_logic::host_deploy_style::HostDeployStyleState::Undeploying
            );
            let transitioned = if deployed_direction {
                style.begin_deploy(frame)
            } else {
                style.begin_undeploy(frame)
            };
            if transitioned {
                unit.stop_moving();
                unit.set_status_moving(false);
                unit.set_ai_state(AIState::Idle);
                if !deployed_direction {
                    // C++ DeployStyleAIUpdate::setMyState(UNDEPLOY) clears
                    // OBJECT_STATUS_DEPLOYED before the pack timer elapses.
                    unit.set_deployed(false);
                }
            }
            transitioned
        };

        if !changed {
            return false;
        }
        if deployed_direction {
            self.deploy_style_reg.record_deploy();
        } else {
            self.deploy_style_reg.record_undeploy();
        }
        true
    }

    /// Wave 232: attack nearest of team without force flag (attack-team residual).
    pub fn unit_command_attack_soft(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        // AI/script acquisition is subject to the same real WeaponSet masks,
        // with CMD_FROM_PLAYER false so C++ AI relationship behavior remains
        // distinct from an explicit right-click.
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(Some(target_id));
        unit.set_force_attack(false);
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 232: free group move — stop attack, path, clear formation if goal not offset.
    pub fn unit_command_move_free(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        click_destination: glam::Vec3,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            // C++ setFormationID(NO_FORMATION) on free individual move when goal is not
            // the stamped formation offset destination.
            if unit.formation_id != 0 {
                let off = unit.formation_offset;
                let expected = glam::Vec3::new(
                    click_destination.x + off.x,
                    click_destination.y,
                    click_destination.z + off.y,
                );
                if (goal - expected).length() > 0.5 {
                    unit.set_formation(0, glam::Vec2::ZERO);
                }
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 233: set order target (records host attack/order log residual).
    pub fn unit_command_set_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(target);
        let can_construct = unit.can_construct();
        if can_construct {
            if let Some(sid) = target {
                if let Some(st) = self.objects.get_mut(&sid) {
                    if st.status.under_construction {
                        // C++ Object::setBuilder (DozerAIUpdate.cpp:1677 / 1986).
                        st.builder_id = Some(id);
                    }
                }
            }
        }
        true
    }

    /// Wave 233: stop moving then set order target (enter/gather/dock residual).
    pub fn unit_command_stop_moving_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_order_target(target);
        true
    }

    /// Begin the C++ `SpecialAbilityUpdate` capture intent.  This records an
    /// order and starts approach only; `markSpecialPowerTriggered` belongs to
    /// the later preparation phase once the source reaches StartAbilityRange.
    pub fn unit_command_begin_capture(&mut self, id: ObjectId, target: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        unit.stop_moving();
        unit.set_order_target(Some(target));
        unit.capture_channel = None;
        unit.set_status_using_ability(false);
        unit.set_ai_state(AIState::Capturing);
        true
    }

    /// Begin a parsed Hacker Disable Building `SpecialAbilityUpdate` intent.
    ///
    /// Like capture, this records only the player-authorized target and
    /// resets an interrupted channel.  It deliberately does not spend the
    /// paired SpecialPower charge: C++ starts that reload in
    /// `startPreparation`, after the hacker reaches its authored range and
    /// finishes unpacking.
    pub fn unit_command_begin_hacker_disable_building(
        &mut self,
        id: ObjectId,
        target: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.stop_moving();
        unit.set_order_target(Some(target));
        unit.hacker_disable_channel = Some(crate::game_logic::HackerDisableChannelState::new(
            target,
            crate::game_logic::HackerDisableChannelPhase::Approaching,
            0,
        ));
        unit.set_status_using_ability(false);
        unit.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Wave 233: set AI attitude residual.
    pub fn unit_command_set_ai_attitude(
        &mut self,
        id: ObjectId,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_ai_attitude(attitude);
        true
    }

    /// Wave 233: building orientation stamp after under-construction create.
    pub fn unit_command_set_orientation(&mut self, id: ObjectId, orientation: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_orientation(orientation);
        true
    }

    /// Wave 233: set building rally point + host_rally_log.
    pub fn unit_command_set_rally_point(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        if obj.building_data.is_none() {
            return false;
        }
        let from = obj.get_position();
        if !self
            .pathfinding_system
            .grid
            .quick_path_exists(from, location)
        {
            #[cfg(feature = "game_client")]
            game_client::helpers::TheInGameUI::message("GUI:RallyPointNoPath");
            self.play_ui_sound("UnableToSetRallyPoint");
            return false;
        }
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.rally_point = Some(location);
        crate::game_logic::host_rally_log::record(id, Some([location.x, location.y, location.z]));
        #[cfg(feature = "game_client")]
        game_client::helpers::TheInGameUI::message("GUI:RallyPointSet");
        self.play_ui_sound("RallyPointSet");
        true
    }

    /// Wave 233: return-supplies order target + ReturningResources state.
    pub fn unit_command_return_supplies(&mut self, id: ObjectId, supply_center: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        // `privateDock(..., CMD_FROM_PLAYER)` stores this target in C++
        // SupplyTruckAIUpdate/WorkerAIUpdate.  The next gather/return cycle
        // must retain the selected center rather than choose the nearest one.
        unit.preferred_dock_id = Some(supply_center);
        unit.set_order_target(Some(supply_center));
        unit.set_ai_state(AIState::ReturningResources);
        true
    }

    /// C++ `SupplyTruckAIUpdate::privateDock` / `WorkerAIUpdate::privateDock`
    /// against `SupplyWarehouseDockUpdate`: remember the chosen warehouse and
    /// enter the collection loop.  This is intentionally not `Entering` — a
    /// warehouse grants supply boxes and never becomes the unit's container.
    pub fn unit_command_dock_at_supply_warehouse(
        &mut self,
        id: ObjectId,
        warehouse: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack();
        unit.preferred_dock_id = Some(warehouse);
        unit.set_order_target(Some(warehouse));
        unit.set_ai_state(AIState::Gathering);
        true
    }

    /// C++ `RailedTransportDockUpdate` dispatches through `AI_DOCK`, not the
    /// generic Enter command.  The support-state loader may ultimately use
    /// the shared containment storage, but the command and legality stay
    /// distinct so normal transports do not accidentally qualify as docks.
    pub fn unit_command_dock_at_railed_transport(
        &mut self,
        id: ObjectId,
        transport: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack();
        unit.set_order_target(Some(transport));
        unit.set_ai_state(AIState::Docking);
        true
    }

    /// Wave 233: waypoint-path prep — stop attack and clear guard anchors.
    pub fn unit_command_waypoint_path_prep(&mut self, id: ObjectId, as_team: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack();
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        unit.end_guard_retaliate();
        // AsTeam keeps formation identity; free follow clears it.
        if !as_team {
            unit.set_formation(0, glam::Vec2::ZERO);
        }
        true
    }

    /// Wave 233: remove occupant from container (enter/exit residual).
    pub fn unit_command_remove_occupant(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) -> bool {
        let is_rider_change = self.objects.get(&container_id).is_some_and(|container| {
            container.thing.template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RiderChange
        });
        if is_rider_change {
            // RiderChangeContain removal owns selection/veterancy/scuttle
            // ordering.  It must not be reduced to a bare Vec removal or the
            // next normal Enter would leave the old rider/container corrupted.
            return self.rider_change_remove_occupant(container_id, occupant_id);
        }
        let Some(container) = self.objects.get_mut(&container_id) else {
            return false;
        };
        container.remove_occupant(occupant_id);
        true
    }

    /// Wave 233: exit-unit drop residual (position/contain/target/ai).
    pub fn unit_command_exit_drop(&mut self, id: ObjectId, drop_position: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_position(drop_position);
        unit.set_contained_by(None);
        unit.set_target(None);
        unit.set_ai_state(AIState::Idle);
        unit.set_status_moving(false);
        unit.set_status_attacking(false);
        true
    }

    /// Wave 233: mine-clearing weapon-set detail residual.
    pub fn unit_command_set_mine_clearing_detail(&mut self, id: ObjectId, enabled: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_weapon_set_mine_clearing_detail(enabled);
        true
    }

    /// Wave 233: evacuate-on-stop pending flags residual.
    pub fn unit_command_set_pending_evacuate(
        &mut self,
        id: ObjectId,
        pending_evacuate_on_stop: bool,
        pending_exit_after_evacuate: bool,
        prep_move: bool,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.pending_evacuate_on_stop = pending_evacuate_on_stop;
        obj.pending_exit_after_evacuate = pending_exit_after_evacuate;
        if prep_move {
            obj.set_target(None);
            obj.set_force_attack(false);
            obj.set_guard_position(None);
            obj.set_guard_target(None);
            obj.end_guard_retaliate();
        }
        true
    }

    /// Wave 233: order target + Entering state (deploy-to-garrison residual).
    pub fn unit_command_order_enter(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(Some(target_id));
        unit.set_ai_state(AIState::Entering);
        true
    }

    /// Wave 233: set weapon-set flag residual.
    pub fn unit_command_set_weapon_set_flag(
        &mut self,
        id: ObjectId,
        flag: u8,
        enabled: bool,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_weapon_set_flag(flag, enabled)
    }

    /// Wave 233: surrender residual.
    pub fn unit_command_set_surrendered(&mut self, id: ObjectId, surrendered: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_surrendered(surrendered);
        true
    }

    /// Wave 233: set AI state if alive.
    pub fn unit_command_set_ai_state(&mut self, id: ObjectId, state: AIState) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_ai_state(state);
        true
    }

    /// Select a concrete weapon slot for a player-issued `FIRE_WEAPON` command.
    ///
    /// Every accepted ordinal must map to a real concrete WeaponSet slot.
    /// Unknown slots fail closed; in particular, TERTIARY never falls through
    /// to PRIMARY.
    pub fn unit_command_select_weapon_slot(&mut self, id: ObjectId, slot: u8) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        let has_requested_slot = matches!(slot, 0..=2) && unit.weapon_slot(slot).is_some();
        if !has_requested_slot {
            return false;
        }

        // C++ Object::doCommandButton sets a temporary weapon lock before it
        // starts the attack, so the command cannot be re-routed by auto-choose.
        unit.set_weapon_lock(slot, WeaponLockType::LockedTemporarily)
    }

    /// Wave 233: weapon fire at object/location residual.
    pub fn unit_command_fire_weapon(
        &mut self,
        id: ObjectId,
        target_object: Option<ObjectId>,
        target_location: Option<glam::Vec3>,
        max_shots_to_fire: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }

        // A new player order replaces a nested AI attack machine in retail.
        // Without clearing these ownership bits, update_combat correctly
        // defers to the nested machine, which can leave a manual TERTIARY
        // command behind an old auto-acquired target state.
        unit.set_status_aiming_weapon(false);
        unit.set_status_firing_weapon(false);
        unit.attack_substate = AttackSubState::AimAtTarget;

        if let Some(tid) = target_object {
            unit.set_target(Some(tid));
        } else if let Some(pos) = target_location {
            unit.set_target_location(Some(pos));
            unit.set_ai_state(AIState::AttackingGround);
        } else {
            return false;
        }
        // C++ MSG_DO_WEAPON[_AT_*] forwards CommandButton::MaxShotsToFire
        // into AIUpdateInterface before the attack state starts.  Do not
        // normalize a finite budget or NO_MAX_SHOTS_LIMIT at this boundary.
        unit.set_max_shots_to_fire(max_shots_to_fire);
        true
    }

    /// Wave 233: infantry go-prone residual.
    pub fn unit_command_go_prone(&mut self, id: ObjectId, prone_secs: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        let is_infantry =
            unit.is_kind_of(KindOf::Infantry) || unit.object_type == ObjectType::Infantry;
        if !is_infantry {
            return false;
        }
        unit.go_prone(prone_secs);
        true
    }

    /// Wave 233: emoticon residual.
    pub fn unit_command_set_emoticon(
        &mut self,
        id: ObjectId,
        name: &str,
        duration_frames: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_emoticon(name, duration_frames);
        true
    }

    /// Wave 233: weapon lock residual.
    /// DemoTrapUpdate: PRIMARY=detonate, SECONDARY=proximity, TERTIARY=manual.
    pub fn unit_command_set_weapon_lock(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
    ) -> bool {
        let is_demo_trap = self
            .objects
            .get(&id)
            .and_then(|unit| unit.mine_data.as_ref())
            .is_some_and(|md| {
                matches!(md.kind, crate::game_logic::host_mines::HostMineKind::DemoTrap)
                    && !md.detonated
            });
        let locked = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            if !unit.is_alive() {
                return false;
            }
            unit.set_weapon_lock(slot, lock_type)
        };
        if locked && is_demo_trap {
            use crate::game_logic::host_mines::DemoTrapMode;
            let mode = match slot {
                0 => DemoTrapMode::Detonate,
                1 => DemoTrapMode::Proximity,
                2 => DemoTrapMode::Manual,
                _ => return locked,
            };
            let _ = self.set_demo_trap_mode(id, mode);
        }
        locked
    }

    /// Wave 233: release weapon lock residual.
    pub fn unit_command_release_weapon_lock(
        &mut self,
        id: ObjectId,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.release_weapon_lock(lock_type);
        true
    }

    /// Wave 233: switch weapons residual.
    pub fn unit_command_switch_weapons(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let candidates = match unit.active_weapon_slot {
            0 => [1u8, 2u8, 0u8],
            1 => [2u8, 0u8, 1u8],
            2 => [0u8, 1u8, 2u8],
            _ => [0u8, 1u8, 2u8],
        };
        let next = candidates
            .into_iter()
            .find(|slot| unit.weapon_slot(*slot).is_some());
        let Some(next) = next else {
            return false;
        };
        let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
        unit.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Wave 233: special-power overridable destination residual.
    pub fn unit_command_set_special_power_overridable_destination(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_special_power_overridable_destination(location, None);
        true
    }

    /// Wave 233: queue upgrade on producer building residual.
    pub fn unit_command_building_add_upgrade_to_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
        research_secs: f32,
        cost: Resources,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // C++ ProductionUpdate::queueUpgrade OBJECT hasUpgrade gate.
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name)
            && obj.has_object_upgrade_complete(upgrade_name)
        {
            return false;
        }
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.add_upgrade_to_queue(upgrade_name.to_string(), research_secs, cost)
    }

    /// Wave 233: remove upgrade entry from producer production queue.
    pub fn unit_command_building_remove_upgrade_from_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        let before = building.production_queue.len();
        building.production_queue.retain(|item| {
            !(item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
        });
        building.production_queue.len() < before
    }

    /// Wave 233: path then set AI state (executor path_to_goal_with_state residual).
    pub fn unit_command_path_with_state(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
    ) -> bool {
        self.unit_command_path_with_state_ignoring(id, goal, state, None)
    }

    /// C++ `ignoreObstacle(goalObject)` then move (DozerAIUpdate.cpp:210-211).
    pub fn unit_command_path_with_state_ignoring(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) -> bool {
        if !self.assign_unit_path_ignoring(id, goal, &[], ignore_obstacle) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(state);
            return true;
        }
        false
    }

    /// Wave 233: C++ groupIdle stealth mood delay residual for one unit.
    /// C++ AIGroup.cpp:2042-2061: CAN_STEALTH + canAutoAcquire + not STEALTHED/DETECTED
    /// **and** !canAutoAcquireWhileStealthed. Host residual for the while-stealthed
    /// gate is `stealth_breaks_on_attack` (units that fight while stealthed skip).
    pub fn unit_command_apply_stealth_mood_delay(
        &mut self,
        id: ObjectId,
        now_frame: u32,
        skew: u32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let can_stealth = unit.innate_stealth || unit.stealth_delay_frames > 0;
        if can_stealth
            && unit.auto_acquire_when_idle
            && unit.can_attack()
            && !unit.status.stealthed
            && !unit.status.detected
            && unit.stealth_breaks_on_attack
        {
            let delay = unit.stealth_delay_frames.max(1);
            unit.next_mood_check_time = now_frame.saturating_add(delay).saturating_add(skew);
            return true;
        }
        false
    }

    #[inline]
    pub fn unit_position_if_movable(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects
            .get(&id)
            .filter(|o| o.can_move())
            .map(|o| o.get_position())
    }

    /// Wave 227: world position probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 224: host residual — force-complete an under-construction structure
    /// (train/construct producer path). Authority mutation owned by GameLogic.
    pub fn force_complete_construction(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
        obj.health.current = obj.health.maximum;
        true
    }

    /// Wave 224: host residual — ensure barracks `building_data` for force-picked
    /// producers so production queue identity is honest without engine dual-scan.
    pub fn ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let need_bd = obj.building_data.is_none()
            || obj
                .building_data
                .as_ref()
                .map(|b| !matches!(b.building_type, BuildingType::Barracks))
                .unwrap_or(true);
        let name_ok = obj.template_name.to_ascii_lowercase().contains("barracks")
            || obj.is_kind_of(KindOf::FSBarracks);
        // Wave 834: also accept force-complete residual producers that already
        // look like infantry factories (Barracks building_type stamp only).
        if need_bd && name_ok {
            // Mirror engine residual: stamp Barracks building_data when missing/mismatched.
            obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
            return true;
        }
        false
    }

    /// Wave 834: force-stamp Barracks building_data for auto_target train residual
    /// when the producer is already known (host spawn / force-complete path).
    pub fn force_ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
        obj.status.under_construction = false;
        obj.construction_percent = 1.0;
        true
    }

    /// Wave 225: clear movement path / target on a unit (host residual).
    pub fn clear_unit_movement_path(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.movement.path.is_empty() && obj.movement.target_position.is_none() {
            return false;
        }
        obj.movement.path.clear();
        obj.movement.current_path_index = 0;
        obj.movement.target_position = None;
        obj.status.moving = false;
        true
    }

    /// Wave 225: adjust guard radius residual on a unit. Returns new radius when applied.
    pub fn adjust_unit_guard_radius(&mut self, id: ObjectId, delta: f32) -> Option<f32> {
        let obj = self.objects.get_mut(&id)?;
        let guarding = matches!(
            obj.ai_state,
            AIState::GuardingArea | AIState::GuardingObject
        ) || obj.guard_position.is_some()
            || obj.guard_target.is_some();
        if !guarding && obj.guard_radius <= 0.0 {
            let base = obj.selection_radius.max(20.0) * 2.0;
            obj.guard_radius = (base + delta).clamp(30.0, 400.0);
        } else {
            let cur = if obj.guard_radius > 1.0 {
                obj.guard_radius
            } else {
                obj.selection_radius.max(20.0) * 2.0
            };
            obj.guard_radius = (cur + delta).clamp(30.0, 400.0);
        }
        Some(obj.guard_radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_slot_logic() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "ThreeSlotTest".to_string(),
            ThingTemplate::new("ThreeSlotTest"),
        );
        let id = logic
            .create_object("ThreeSlotTest", Team::USA, glam::Vec3::ZERO)
            .expect("test object");
        let object = logic.host_object_mut(id).expect("test object mut");
        object.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.secondary_weapon = Some(Weapon {
            damage: 2.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.tertiary_weapon = Some(Weapon {
            damage: 3.0,
            range: 100.0,
            ..Weapon::default()
        });
        (logic, id)
    }

    #[test]
    fn select_tertiary_slot_is_concrete_and_temporary_locked() {
        let (mut logic, id) = three_slot_logic();
        assert!(logic.unit_command_select_weapon_slot(id, 2));
        let object = logic.host_object(id).expect("selected object");
        assert_eq!(object.active_weapon_slot, 2);
        assert_eq!(object.weapon_lock_slot, 2);
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(object.weapon_slot(2).map(|weapon| weapon.damage), Some(3.0));
        assert!(object.weapon_slot(3).is_none(), "unknown slots fail closed");
    }

    #[test]
    fn absent_or_unknown_tertiary_selection_cannot_fall_back_to_primary() {
        let (mut logic, id) = three_slot_logic();
        logic.host_object_mut(id).expect("object").tertiary_weapon = None;
        assert!(!logic.unit_command_select_weapon_slot(id, 2));
        assert!(!logic.unit_command_select_weapon_slot(id, 7));
        let object = logic.host_object(id).expect("object");
        assert_eq!(object.weapon_slot(0).map(|weapon| weapon.damage), Some(1.0));
        assert!(object.weapon_slot(2).is_none());
    }

    #[test]
    fn player_tertiary_fire_order_replaces_an_existing_nested_attack() {
        let (mut logic, id) = three_slot_logic();
        let target_id = logic
            .create_object("ThreeSlotTest", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("target");
        {
            let object = logic.host_object_mut(id).expect("object");
            object.status.is_aiming_weapon = true;
            object.status.is_firing_weapon = true;
            object.attack_substate = AttackSubState::FireWeapon;
        }

        assert!(logic.unit_command_select_weapon_slot(id, 2));
        assert!(logic.unit_command_fire_weapon(id, Some(target_id), None, -1));

        let object = logic.host_object(id).expect("object");
        assert_eq!(object.target, Some(target_id));
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(object.weapon_lock_slot, 2);
        assert!(!object.status.is_aiming_weapon);
        assert!(!object.status.is_firing_weapon);
        assert_eq!(object.attack_substate, AttackSubState::AimAtTarget);
    }

    #[test]
    fn switch_weapons_cycles_to_tertiary_when_it_is_next_available_slot() {
        let (mut logic, id) = three_slot_logic();
        logic
            .host_object_mut(id)
            .expect("object")
            .active_weapon_slot = 1;
        assert!(logic.unit_command_switch_weapons(id));
        let object = logic.host_object(id).expect("object");
        assert_eq!(object.active_weapon_slot, 2);
        assert_eq!(object.weapon_lock_slot, 2);
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedPermanently);
    }
}
