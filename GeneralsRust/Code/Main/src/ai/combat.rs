use super::*;

impl AIPlayer {
    /// Minimum seconds between host AI **attack re-evaluations**.
    ///
    /// Wave 616: residual-locked at 60s (not gate-driven early-attack).
    /// Shares the **numeric** 60s value from C++ `AIPlayer::checkReadyTeams`
    /// (`GeneralsMD/.../AI/AIPlayer.cpp`: force-start ready team after
    /// `60 * LOGICFRAMES_PER_SECOND`), but this is **not** a port of that function.
    /// C++ uses 60s for team activation at rally; this host AI uses 60s only as
    /// spacing between strength-threshold attack decisions. Full checkReadyTeams
    /// (idle/anyIdle, production-condition scripts, setActive) remains unported.
    pub const ATTACK_RECHECK_SECONDS: f32 = 60.0;

    /// Retail `AIData.ini` defaults (Default/AIData.ini).
    /// StructureSeconds = 0 → try structure decisions every AI economic tick when ready.
    pub const STRUCTURE_SECONDS: f32 = 0.0;
    /// TeamSeconds = 10 → wait between successful new-team selections.
    pub const TEAM_SECONDS: f32 = 10.0;
    /// C++ `AISkirmishPlayer::doTeamBuilding`: `m_teamDelay = 2 * LOGICFRAMES_PER_SECOND`.
    pub const TEAM_QUEUE_RETRY_SECONDS: f32 = 2.0;
    /// RebuildDelayTimeSeconds = 30 (base rebuild delay residual; full C++ path unported).
    pub const REBUILD_DELAY_SECONDS: f32 = 30.0;
    /// Wealthy resource threshold (AIData `Wealthy`).
    pub const WEALTHY_RESOURCES: u32 = 7000;
    /// Poor resource threshold (AIData `Poor`).
    pub const POOR_RESOURCES: u32 = 2000;
    /// StructuresWealthyRate — interval divisor when wealthy (2=twice as fast).
    pub const STRUCTURES_WEALTHY_RATE: f32 = 2.0;
    /// StructuresPoorRate.
    pub const STRUCTURES_POOR_RATE: f32 = 0.6;
    /// TeamsWealthyRate.
    pub const TEAMS_WEALTHY_RATE: f32 = 2.0;
    /// TeamsPoorRate.
    pub const TEAMS_POOR_RATE: f32 = 0.6;
    /// Retail AIData `TeamResourcesToStart` fallback when leftover AIData is unset.
    pub const TEAM_RESOURCES_TO_START: f32 = 0.1;

    /// Evaluate opportunities to attack enemies (strength-threshold + C++-aligned spacing).

    /// AIData wealth/poor rate residual: returns speed multiplier (>= rate means faster).
    pub(super) fn resource_speed_rate(&self, game_logic: &GameLogic, for_structures: bool) -> f32 {
        let supplies = game_logic
            .get_player(self.player_id)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        if supplies >= Self::WEALTHY_RESOURCES {
            if for_structures {
                Self::STRUCTURES_WEALTHY_RATE
            } else {
                Self::TEAMS_WEALTHY_RATE
            }
        } else if supplies <= Self::POOR_RESOURCES {
            if for_structures {
                Self::STRUCTURES_POOR_RATE
            } else {
                Self::TEAMS_POOR_RATE
            }
        } else {
            1.0
        }
    }

    /// Base interval seconds scaled by difficulty and wealth/poor rates.
    pub(super) fn scaled_interval_seconds(
        &self,
        game_logic: &GameLogic,
        base_seconds: f32,
        for_structures: bool,
    ) -> f32 {
        if base_seconds <= 0.0 {
            return 0.0;
        }
        let delay = self.difficulty.get_build_delay_modifier().max(0.01);
        let rate = self
            .resource_speed_rate(game_logic, for_structures)
            .max(0.01);
        // C++ rate multiplies speed → shorter wait when rate > 1.
        (base_seconds * delay) / rate
    }

    pub(super) fn evaluate_attack_opportunities(
        &mut self,
        game_logic: &mut GameLogic,
        _current_time: f32,
    ) {
        // C++ AIPlayer has no all-army raid latch. Teams only AttackMove when
        // OnCreate scripts say so (checkReadyTeams → setActive). Keep the host
        // attack_in_progress flag only so a finished raid can clear.
        self.clear_finished_attack(game_logic);
    }

    /// Calculate our military strength
    pub(super) fn calculate_military_strength(&self, game_logic: &GameLogic) -> f32 {
        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == self.team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1; // Basic strength calculation
            }
        }

        strength
    }

    /// Estimate enemy military strength
    pub(super) fn estimate_enemy_strength(&self, game_logic: &GameLogic, enemy_id: u32) -> f32 {
        let enemy_team = if let Some(player) = game_logic.get_player(enemy_id) {
            player.team
        } else {
            return 0.0;
        };

        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == enemy_team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1;
            }
        }

        strength
    }

    /// Record C++ `AIAttackMoveState` / `AIInternalMoveToState::onEnter` on the
    /// crate `AiStateMachine` (move/attack only; does not run the 48-state graph).
    pub(super) fn dispatch_crate_attack_move(
        unit_id: ObjectId,
        dest: Vec3,
        focus: Option<ObjectId>,
    ) {
        let dest = gamelogic::common::types::Coord3D::new(dest.x, dest.y, dest.z);
        let _ = gamelogic::ai::state_machine::dispatch_host_move_attack(
            unit_id.0,
            gamelogic::ai::state_machine::HostMoveAttackKind::AttackMoveTo,
            Some(dest),
            focus.map(|id| id.0),
        );
    }

    /// AttackMove the given units toward the enemy base (OnCreate residual).
    pub(super) fn attack_move_units(
        &mut self,
        game_logic: &mut GameLogic,
        attack_units: &[ObjectId],
        current_time: f32,
    ) {
        if attack_units.is_empty() {
            return;
        }
        let enemy_base = if let Some(enemy_id) = self.enemy_player_id {
            if let Some(player) = game_logic.get_player(enemy_id) {
                self.find_enemy_base_center(game_logic, player.team)
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };
        let enemy_team = self
            .enemy_player_id
            .and_then(|eid| game_logic.get_player(eid).map(|p| p.team));
        let focus_enemy = enemy_team.and_then(|eteam| {
            game_logic
                .host_objects()
                .iter()
                .filter(|(_, o)| {
                    o.team == eteam
                        && o.is_alive()
                        && o.is_kind_of(crate::game_logic::KindOf::Attackable)
                })
                .min_by(|(_, a), (_, b)| {
                    let da = a.get_position().distance(enemy_base);
                    let db = b.get_position().distance(enemy_base);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, _)| *id)
        });

        for &unit_id in attack_units {
            if let Some(focus) = focus_enemy {
                game_logic.apply_engagement_decision_aware_for_ai(unit_id, focus);
            }
            let mobile = game_logic
                .host_object(unit_id)
                .map(|u| u.is_mobile() && u.is_alive())
                .unwrap_or(false);
            if !mobile {
                continue;
            }
            if game_logic.assign_unit_path(unit_id, enemy_base, &[]) {
                game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                if let Some(unit) = game_logic.host_object_mut(unit_id) {
                    unit.is_attack_path = true;
                    unit.requested_destination = Some(enemy_base);
                }
                Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
            } else {
                if let Some(unit) = game_logic.host_object_mut(unit_id) {
                    unit.move_to(enemy_base);
                    unit.is_attack_path = true;
                    unit.requested_destination = Some(enemy_base);
                }
                game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_move_to(unit_id, enemy_base);
                }
            }
        }

        self.attack_in_progress = true;
        self.last_attack_time = current_time;
        self.activity_count = self.activity_count.saturating_add(1);
    }

    /// Launch coordinated attack
    pub(crate) fn launch_attack(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        log::debug!(
            "AI Player {} ({}) launching attack!",
            self.player_id,
            self.team.get_name()
        );

        let mut attack_units = Vec::new();
        for (object_id, object) in game_logic.host_objects() {
            if object.team == self.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
            {
                attack_units.push(*object_id);
            }
        }
        self.attack_move_units(game_logic, &attack_units, current_time);
    }

    /// Clear the host raid latch once launched attackers are idle or gone.
    ///
    /// C++ `AIPlayer` / `AISkirmishPlayer` have no permanent `m_attackInProgress`.
    /// Teams activate via `checkReadyTeams` (`AIPlayer.cpp:2729`) when idle (or
    /// after 60s) and later scripts can start another attack. The host latch
    /// must not survive the raid or AI attacks exactly once per game.
    pub(super) fn clear_finished_attack(&mut self, game_logic: &GameLogic) {
        if !self.attack_in_progress {
            return;
        }
        if !self.raid_units_still_committed(game_logic) {
            self.attack_in_progress = false;
        }
    }

    pub(super) fn raid_units_still_committed(&self, game_logic: &GameLogic) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == self.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
                && matches!(
                    object.ai_state,
                    AIState::AttackMoving | AIState::Attacking | AIState::AttackingGround
                )
        })
    }

    /// C++ `ScriptActions::doSkirmishFireSpecialPowerAtMostCost`.
    /// Fires only the named SpecialPower template, never the first ready one.
    pub fn fire_named_special_power(&mut self, game_logic: &mut GameLogic, power_name: &str) {
        if power_name.is_empty() {
            return;
        }
        let Some(enemy_id) = self.enemy_player_id else {
            return;
        };
        let Some(enemy_team) = game_logic.get_player(enemy_id).map(|p| p.team) else {
            return;
        };

        let mut ready: Vec<(
            ObjectId,
            crate::command_system::SpecialPowerType,
            String,
            bool,
        )> = Vec::new();
        for (id, object) in game_logic.host_objects() {
            if object.team != self.team || !object.is_alive() {
                continue;
            }
            for module in &object.thing.template.special_power_modules {
                if !module
                    .special_power_template
                    .eq_ignore_ascii_case(power_name)
                {
                    continue;
                }
                let Some(power) = module.command_power.clone() else {
                    continue;
                };
                if !game_logic.is_special_power_ready_for(*id, &power) {
                    continue;
                }
                let sneak = matches!(power, crate::command_system::SpecialPowerType::SneakAttack);
                ready.push((*id, power, module.special_power_template.clone(), sneak));
            }
        }
        if ready.is_empty() {
            return;
        }

        for (caster, power, template_name, sneak) in ready {
            let cluster = matches!(
                power,
                crate::command_system::SpecialPowerType::ClusterMines
                    | crate::command_system::SpecialPowerType::NukeDrop
            );
            let Some(mut location) = (if cluster {
                self.compute_cluster_mines_target(game_logic, enemy_team)
            } else {
                let mut radius = 50.0;
                let cursor = Self::radius_cursor_for_power(&power, &template_name);
                if cursor > radius {
                    radius = cursor;
                }
                self.compute_superweapon_target(game_logic, enemy_team, radius, !sneak)
            }) else {
                continue;
            };
            if sneak {
                if let Some(legal) = self.calc_closest_construction_zone_location(
                    game_logic,
                    crate::game_logic::GLA_SNEAK_TUNNEL_TEMPLATE,
                    location,
                ) {
                    location = legal;
                } else {
                    continue;
                }
            }
            if location.length_squared() <= 0.0 {
                continue;
            }
            game_logic.queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::DoSpecialPower {
                    power_type: power,
                    target: crate::command_system::PowerTarget::Location(location),
                },
                player_id: self.player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![caster],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            self.activity_count = self.activity_count.saturating_add(1);
            break;
        }
    }

    /// C++ `Player::calcClosestConstructionZoneLocation` residual: seed
    /// `NO_OBJECT_OVERLAP`, then wiggle if the sneak pad is illegal.
    pub(super) fn calc_closest_construction_zone_location(
        &self,
        game_logic: &GameLogic,
        template_name: &str,
        seed: Vec3,
    ) -> Option<Vec3> {
        if game_logic.is_location_legal_to_build(self.team, seed, template_name) {
            return Some(seed);
        }
        const STEP: f32 = 20.0;
        for ring in 1..12 {
            let reach = STEP * ring as f32;
            for dx in [-1_i32, 0, 1] {
                for dz in [-1_i32, 0, 1] {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let candidate = Vec3::new(
                        seed.x + dx as f32 * reach,
                        seed.y,
                        seed.z + dz as f32 * reach,
                    );
                    if game_logic.is_location_legal_to_build(self.team, candidate, template_name) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Host residual of C++ `AIPlayer::computeSuperweaponTarget`.
    /// Ground plane is host XZ (C++ samples XY).
    pub(super) fn compute_superweapon_target(
        &mut self,
        game_logic: &GameLogic,
        enemy_team: Team,
        weapon_radius: f32,
        target_military_units: bool,
    ) -> Option<Vec3> {
        let radius = weapon_radius.max(1.0);
        let (mut min_x, mut min_z, mut max_x, mut max_z) =
            self.player_structure_bounds(game_logic, enemy_team);
        if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
            let (lo, hi) = game_logic.world_bounds();
            min_x = lo.x;
            min_z = lo.z;
            max_x = hi.x;
            max_z = hi.z;
        }

        min_x += radius;
        max_x -= radius;
        if max_x < min_x {
            let mid = (max_x + min_x) * 0.5;
            min_x = mid;
            max_x = mid;
        }
        if max_z < min_z {
            let mid = (max_z + min_z) * 0.5;
            min_z = mid;
            max_z = mid;
        }

        let width = (max_x - min_x).max(0.0);
        let height = (max_z - min_z).max(0.0);
        let mut x_count = (width / radius).ceil() as i32 + 1;
        let mut z_count = (height / radius).ceil() as i32 + 1;
        if x_count > 10 {
            x_count = 10;
        }
        if z_count > 10 {
            z_count = 10;
        }
        if x_count < 1 {
            x_count = 1;
        }
        if z_count < 1 {
            z_count = 1;
        }

        // C++ GameLogicRandomValue(1, 4) scan-direction residual.
        let (x_delta, z_delta, x_start, z_start) = match self.placement_rng.next_int(1, 4) {
            1 => (1_i32, 1_i32, 0_i32, 0_i32),
            2 => (-1, 1, x_count, 0),
            3 => (1, -1, 0, z_count),
            _ => (-1, -1, x_count, z_count),
        };

        let mut best_cash: i32 = -1;
        let mut best_pos = Vec3::new(min_x, 0.0, min_z);
        let mut x_index = x_start;
        for _ in 0..x_count {
            let mut z_index = z_start;
            for _ in 0..z_count {
                let pos = Vec3::new(
                    min_x + (width * x_index as f32) / x_count as f32,
                    0.0,
                    min_z + (height * z_index as f32) / z_count as f32,
                );
                let value = self.player_superweapon_value(
                    game_logic,
                    enemy_team,
                    pos,
                    2.0 * radius,
                    target_military_units,
                );
                if value > best_cash {
                    best_cash = value;
                    best_pos = pos;
                }
                z_index += z_delta;
            }
            x_index += x_delta;
        }

        // Fine tune: C++ uses (x-5) for BOTH axes (legacy bug — keep for parity).
        let mut fine_best = best_pos;
        let mut fine_cash: i32 = -1;
        let mut fine_count = 0_i32;
        for x in 0..11 {
            for _y in 0..11 {
                let offset = (x - 5) as f32 * (radius / 10.0);
                let pos = Vec3::new(best_pos.x + offset, 0.0, best_pos.z + offset);
                let value = self.player_superweapon_value(
                    game_logic,
                    enemy_team,
                    pos,
                    radius,
                    target_military_units,
                );
                if value > fine_cash {
                    fine_cash = value;
                    fine_best = pos;
                    fine_count = 1;
                } else if value == fine_cash {
                    fine_best.x += pos.x;
                    fine_best.z += pos.z;
                    fine_count += 1;
                }
            }
        }
        if fine_count > 1 {
            fine_best.x /= fine_count as f32;
            fine_best.z /= fine_count as f32;
        }
        if fine_cash > -1 {
            Some(fine_best)
        } else {
            None
        }
    }

    /// C++ `ScriptActions` radius: `max(50, power->getRadiusCursorRadius())`.
    pub(super) fn radius_cursor_for_power(
        power: &crate::command_system::SpecialPowerType,
        template_name: &str,
    ) -> f32 {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::special_power_template_row_wave109;
        if let Some(row) = special_power_template_row_wave109(template_name) {
            return row.radius_cursor_radius;
        }
        match power {
            P::ClusterMines | P::NukeDrop => 100.0,
            P::ScudStorm => 200.0,
            P::DaisyCutter | P::AirForceDaisyCutter => 170.0,
            P::NuclearMissile | P::NukeNeutronMissile | P::SuperweaponNeutronMissile => 210.0,
            P::EmpPulse => 200.0,
            P::SpySatellite => 300.0,
            P::SpyDrone => 250.0,
            P::Artillery => 125.0,
            P::CarpetBomb => 100.0,
            P::EarlyChinaCarpetBomb | P::NukeChinaCarpetBomb | P::AirForceCarpetBomb => 180.0,
            P::AnthraxBomb => 250.0,
            P::EmergencyRepair | P::EarlyEmergencyRepair => 100.0,
            P::GpsScrambler | P::StealthGpsScrambler => 100.0,
            P::Frenzy | P::EarlyFrenzy => 200.0,
            P::LeafletDrop | P::EarlyLeafletDrop => 110.0,
            _ => 0.0,
        }
    }
    /// C++ `AISkirmishPlayer::computeSuperweaponTarget` cluster-mines branch.
    pub(super) fn compute_cluster_mines_target(
        &mut self,
        game_logic: &GameLogic,
        enemy_team: Team,
    ) -> Option<Vec3> {
        let start_index = game_logic
            .get_player(self.player_id)
            .map(|p| p.start_position.max(0))
            .unwrap_or(0);
        let mode = self.placement_rng.next_int(0, 2);
        let _path_label = match mode {
            1 => format!("SkirmFlank{}", start_index + 1),
            2 => format!("SkirmBackdoor{}", start_index + 1),
            _ => format!("SkirmCenter{}", start_index + 1),
        };
        // Host leftover has no TerrainLogic waypoint walk; C++ falls back to
        // enemy structure-bounds center when the labeled path is missing.
        let (min_x, min_z, max_x, max_z) = self.player_structure_bounds(game_logic, enemy_team);
        let goal = if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
            self.find_enemy_base_center(game_logic, enemy_team)
        } else {
            Vec3::new(
                min_x + (max_x - min_x) * 0.5,
                0.0,
                min_z + (max_z - min_z) * 0.5,
            )
        };
        let mut offset_x = goal.x - self.base_center.x;
        let mut offset_z = goal.z - self.base_center.z;
        let length = (offset_x * offset_x + offset_z * offset_z).sqrt();
        if length > 0.001 {
            offset_x /= length;
            offset_z /= length;
        }
        offset_x *= self.base_radius;
        offset_z *= self.base_radius;
        Some(Vec3::new(
            self.base_center.x + offset_x,
            0.0,
            self.base_center.z + offset_z,
        ))
    }

    /// C++ `AIPlayer::repairStructure` (`AIPlayer.cpp:2254-2280`).
    pub fn repair_structure(&mut self, game_logic: &GameLogic, structure_id: ObjectId) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let Some(structure) = game_logic.host_object(structure_id) else {
            return;
        };
        if matches!(structure.body_damage_state, HostBodyDamageType::Pristine)
            && structure.health.current + 0.01 >= structure.health.maximum
        {
            return;
        }
        if self.structures_to_repair.contains(&structure_id) {
            return;
        }
        if self.structures_to_repair.len() >= MAX_STRUCTURES_TO_REPAIR {
            return;
        }
        self.structures_to_repair.push(structure_id);
    }

    /// C++ `AISkirmishPlayer::checkBridges` (AISkirmishPlayer.cpp:694-711).
    ///
    /// Walk leftover `way->getNext()` hops. Live
    /// `clientSafeQuickDoesPathExist` continues; else leftover
    /// `findBrokenBridge` (then live destroyed-layer scan) → `repairStructure`
    /// → true. Does **not** enqueue every damaged span.
    pub fn check_bridges(
        &mut self,
        game_logic: &GameLogic,
        unit_id: ObjectId,
        start_waypoint_id: u32,
    ) -> bool {
        let Some(unit) = game_logic.host_object(unit_id) else {
            return false;
        };
        // C++: if (!ai) return false;
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        let unit_pos = unit.get_position();
        let surfaces = if unit.locomotor_surfaces != 0 {
            unit.locomotor_surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let is_crusher = unit.crusher_level > 0;
        let from = gamelogic::common::Coord3D::new(unit_pos.x, unit_pos.z, unit_pos.y);
        let hop_targets: Vec<gamelogic::common::Coord3D> = {
            let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
                return false;
            };
            let mut out = Vec::new();
            let mut cur = terrain.get_waypoint_by_id(start_waypoint_id);
            while let Some(way) = cur {
                out.push(*way.get_location());
                cur = way.get_next();
            }
            out
        };
        let loco = gamelogic::locomotor::LocomotorSet::from_surfaces(surfaces);
        let ai_store = gamelogic::ai::the_ai();let leftover_pf = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder());

        for target in hop_targets {
            let hop = Vec3::new(target.x, target.z, target.y);
            // Player path: live zone connectivity. Leftover empty zones
            // would report true and skip a destroyed span.
            if game_logic
                .pathfinding_system
                .client_safe_quick_does_path_exist_for_crusher(unit_pos, hop, surfaces, is_crusher)
            {
                continue;
            }
            if let Some(pf_arc) = leftover_pf.as_ref() {
                if let Ok(pf) = pf_arc.read() {
                    if pf.client_safe_quick_does_path_exist(&loco, &from, &target) {
                        // Leftover terrain still joins; live already said no.
                    }
                    if let Some(bridge_id) = pf.find_broken_bridge(&loco, &from, &target) {
                        if bridge_id != 0 {
                            self.repair_structure(game_logic, ObjectId(bridge_id));
                            return true;
                        }
                    }
                }
            }
            if let Some(bridge_id) = game_logic
                .pathfinding_system
                .find_broken_bridge(unit_pos, hop)
            {
                self.repair_structure(game_logic, bridge_id);
                return true;
            }
        }
        false
    }

    /// C++ `AIPlayer::updateBridgeRepair` (`AIPlayer.cpp:2296-2384`).
    pub(super) fn update_bridge_repair(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        if self.structures_to_repair.is_empty() {
            return;
        }
        if self.last_bridge_repair_time >= 0.0 && current_time - self.last_bridge_repair_time < 1.0
        {
            return;
        }
        self.last_bridge_repair_time = current_time;

        while !self.structures_to_repair.is_empty() {
            let head = self.structures_to_repair[0];
            if game_logic.host_object(head).is_some_and(|o| o.is_alive()) {
                break;
            }
            self.structures_to_repair.remove(0);
        }
        if self.structures_to_repair.is_empty() {
            return;
        }
        let bridge_id = self.structures_to_repair[0];
        let Some(bridge) = game_logic.host_object(bridge_id) else {
            return;
        };
        let bridge_pos = bridge.get_position();
        let bridge_pristine = matches!(bridge.body_damage_state, HostBodyDamageType::Pristine)
            && bridge.health.current + 0.01 >= bridge.health.maximum;

        if self.repair_dozer.is_none() {
            self.dozer_is_repairing = false;
            if self.dozer_queued_for_repair {
                return;
            }
            if let Some(dozer_id) =
                Self::find_available_dozer(game_logic, self.team, bridge_pos, None)
            {
                self.repair_dozer = Some(dozer_id);
                if let Some(dozer) = game_logic.host_object(dozer_id) {
                    self.repair_dozer_origin = dozer.get_position();
                }
                game_logic.queue_command(crate::command_system::GameCommand {
                    command_type: crate::command_system::CommandType::Repair {
                        target_id: bridge_id,
                    },
                    player_id: self.player_id,
                    command_id: 0,
                    timestamp: std::time::SystemTime::now(),
                    selected_units: vec![dozer_id],
                    modifier_keys: crate::command_system::ModifierKeys::default(),
                });
                self.dozer_is_repairing = true;
                return;
            }
            self.queue_dozer(game_logic, current_time);
            self.dozer_queued_for_repair = true;
            return;
        }

        let Some(dozer_id) = self.repair_dozer else {
            return;
        };
        let Some(dozer) = game_logic.host_object(dozer_id) else {
            self.repair_dozer = None;
            self.last_bridge_repair_time = -1.0;
            return;
        };
        if !dozer.is_alive() {
            self.repair_dozer = None;
            self.last_bridge_repair_time = -1.0;
            return;
        }
        let dozer_idle = dozer.ai_state == AIState::Idle;

        if self.dozer_is_repairing {
            if !dozer_idle {
                return;
            }
            if bridge_pristine {
                self.structures_to_repair.remove(0);
                self.dozer_is_repairing = false;
                if self.structures_to_repair.is_empty() {
                    let mut home = if self.base_center.length_squared() > 0.0 {
                        self.base_center
                    } else {
                        self.repair_dozer_origin
                    };
                    // C++ AIPlayer.cpp:2370-2372 adjustToPossibleDestination then aiMoveToPosition.
                    let _ = game_logic.adjust_to_possible_destination(dozer_id, &mut home);
                    let _ = game_logic.unit_command_move_to(dozer_id, home);
                }
                return;
            }
        }

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::Repair {
                target_id: bridge_id,
            },
            player_id: self.player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![dozer_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        self.dozer_is_repairing = true;
    }

    pub(super) fn player_structure_bounds(
        &self,
        game_logic: &GameLogic,
        enemy_team: Team,
    ) -> (f32, f32, f32, f32) {
        let mut any = false;
        let mut min_x = 0.0;
        let mut min_z = 0.0;
        let mut max_x = 0.0;
        let mut max_z = 0.0;
        for object in game_logic.host_objects().values() {
            if object.team != enemy_team
                || !object.is_alive()
                || !object.is_kind_of(KindOf::Structure)
            {
                continue;
            }
            let p = object.get_position();
            if !any {
                min_x = p.x;
                max_x = p.x;
                min_z = p.z;
                max_z = p.z;
                any = true;
            } else {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_z = min_z.min(p.z);
                max_z = max_z.max(p.z);
            }
        }
        if any {
            (min_x, min_z, max_x, max_z)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Host residual of C++ `AIPlayer::getPlayerSuperweaponValue`.
    pub(super) fn player_superweapon_value(
        &self,
        game_logic: &GameLogic,
        enemy_team: Team,
        center: Vec3,
        radius: f32,
        include_military_units: bool,
    ) -> i32 {
        let radius = radius.max(4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL);
        let rad_sqr = radius * radius;
        let mut cash = 0.0_f32;
        for object in game_logic.host_objects().values() {
            if object.team != enemy_team || !object.is_alive() {
                continue;
            }
            let mut apply_neg = false;
            if !include_military_units {
                if object.is_kind_of(KindOf::FSBaseDefense) {
                    apply_neg = true;
                } else if (object.is_kind_of(KindOf::Vehicle)
                    || object.is_kind_of(KindOf::Infantry))
                    && !object.is_kind_of(KindOf::Dozer)
                    && !object.is_kind_of(KindOf::Harvester)
                {
                    apply_neg = true;
                }
            } else if object.is_kind_of(KindOf::Aircraft)
                && (object.status.airborne_target
                    || crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                        object.get_position().y,
                    ))
            {
                continue;
            }
            let pos = object.get_position();
            let dx = center.x - pos.x;
            let dz = center.z - pos.z;
            let dist_sqr = dx * dx + dz * dz;
            if dist_sqr >= rad_sqr {
                continue;
            }
            let dist = dist_sqr.sqrt();
            let factor = 1.0 - (dist / (2.0 * radius));
            let mut value = object.thing.template.build_cost.supplies as f32;
            if object.is_kind_of(KindOf::CommandCenter) || object.is_kind_of(KindOf::FSSuperweapon)
            {
                if include_military_units {
                    value /= 10.0;
                } else {
                    value *= 5.0;
                }
            }
            if apply_neg {
                cash -= factor * value * 5.0;
            } else {
                cash += factor * value;
            }
        }
        cash as i32
    }

    /// Find center of enemy base
    pub(super) fn find_enemy_base_center(&self, game_logic: &GameLogic, enemy_team: Team) -> Vec3 {
        let mut center = Vec3::ZERO;
        let mut count = 0;

        // Find enemy command center or other key buildings
        for object in game_logic.host_objects().values() {
            if object.team == enemy_team
                && object.is_alive()
                && (object.is_kind_of(KindOf::CommandCenter)
                    || object.is_kind_of(KindOf::Structure))
            {
                center += object.get_position();
                count += 1;
            }
        }

        if count > 0 {
            center / count as f32
        } else {
            // Default to opposite corner if no buildings found
            -self.base_center
        }
    }

    /// Update strategic phase based on game state
    pub(super) fn update_strategy_phase(&mut self, game_logic: &GameLogic, current_time: f32) {
        let game_time = current_time; // Game time in seconds

        match game_time {
            t if t < 300.0 => self.current_strategy = AIStrategy::EarlyGame, // First 5 minutes
            t if t < 900.0 => self.current_strategy = AIStrategy::MidGame,   // 5-15 minutes
            _ => self.current_strategy = AIStrategy::LateGame,               // After 15 minutes
        }

        // Check for desperate situation
        if let Some(player) = game_logic.get_player(self.player_id) {
            if player.resources.supplies < 200 {
                self.current_strategy = AIStrategy::Desperate;
            }
        }
    }

    /// Update build phase based on progress
    pub(super) fn update_build_phase(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Count constructed buildings
        let built_buildings = self.building_queue.iter().filter(|b| b.is_built).count();

        // Count military units
        let military_units = game_logic
            .host_objects()
            .iter()
            .filter(|(_, obj)| obj.team == self.team && obj.can_attack())
            .count();

        self.build_phase = match (built_buildings, military_units) {
            (0..=2, _) => AIBuildPhase::BaseConstruction,
            (_, 0..=5) => AIBuildPhase::UnitProduction,
            (3..=5, _) => AIBuildPhase::Expansion,
            _ => AIBuildPhase::MassProduction,
        };
    }
}
