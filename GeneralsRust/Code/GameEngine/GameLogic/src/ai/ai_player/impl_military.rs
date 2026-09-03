//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// Update AI strategy based on current conditions
    pub(super) fn update_strategy(&mut self) -> Result<(), AiError> {
        let current_frame = TheGameLogic::get_frame();

        // Update strategic decision maker
        self.strategic_decision_maker.update(current_frame);

        // Analyze current situation
        self.analyze_economic_situation()?;
        self.analyze_military_situation()?;
        self.analyze_threats()?;

        // Calculate base health from owned structures
        // In full implementation, would scan all player buildings and calculate average health
        let base_health = self.calculate_base_health();

        // Make strategic decision using new system
        let decision = self.strategic_decision_maker.make_decision(
            self.military_state.total_military_strength,
            self.military_state.enemy_strength_estimate,
            base_health,
            self.threat_assessment.overall_threat_level as f32 / 5.0, // Convert enum to 0.0-1.0
            self.economic_state
                .current_resources
                .get("money")
                .copied()
                .unwrap_or(0),
        );

        // Execute decision
        self.execute_strategic_decision(decision)?;

        // Legacy strategy change logic
        if self.should_change_strategy()? {
            let new_strategy = self.determine_optimal_strategy()?;
            self.change_strategy(new_strategy, current_frame)?;
        }

        Ok(())
    }

    /// Execute a strategic decision made by the decision maker
    pub(super) fn execute_strategic_decision(
        &mut self,
        decision: StrategicDecision,
    ) -> Result<(), AiError> {
        match decision {
            StrategicDecision::BuildUpForces => {
                // Focus on building military units
                self.prioritize_military_production()?;
            }
            StrategicDecision::LaunchAttack => {
                // Initiate attack on enemy
                self.launch_attack()?;
                self.strategic_decision_maker.on_attack_launched();
            }
            StrategicDecision::DefendBase => {
                // Build defenses and position units defensively
                self.prioritize_defensive_buildings()?;
            }
            StrategicDecision::Expand => {
                // Expand to new locations
                if self.strategic_decision_maker.expansion.can_expand {
                    self.initiate_expansion()?;
                    self.strategic_decision_maker.on_expansion_complete();
                }
            }
            StrategicDecision::EconomicGrowth => {
                // Focus on economy
                self.prioritize_economic_buildings()?;
            }
            StrategicDecision::TechProgression => {
                // Research upgrades
                self.prioritize_tech_upgrades()?;
            }
            StrategicDecision::Harassment => {
                // Send harassing units
                self.initiate_harassment()?;
            }
            StrategicDecision::Turtle => {
                // Build heavy defenses
                self.build_ai_base_defense(false)?;
                self.build_ai_base_defense(true)?;
            }
            StrategicDecision::AllOut => {
                // All-out attack with everything
                self.launch_all_out_attack()?;
            }
        }
        Ok(())
    }

    /// Prioritize military production
    pub(super) fn prioritize_military_production(&mut self) -> Result<(), AiError> {
        // Adjust resource allocation to favor military
        self.strategic_decision_maker
            .resources
            .allocations
            .insert("military".to_string(), 0.7);
        self.strategic_decision_maker
            .resources
            .allocations
            .insert("economy".to_string(), 0.2);
        Ok(())
    }

    /// Launch attack on enemy
    /// Coordinates attack teams and selects strategic targets
    pub(super) fn launch_attack(&mut self) -> Result<(), AiError> {
        // Build attack teams if we don't have enough
        let military_strength = self.military_state.total_military_strength;
        let enemy_strength = self.military_state.enemy_strength_estimate;

        // Only attack if we have reasonable strength (difficulty affects this)
        let strength_threshold = match self.difficulty {
            GameDifficulty::Easy => 0.6,   // Easy AI needs 60% of enemy strength
            GameDifficulty::Normal => 0.8, // Normal needs 80%
            GameDifficulty::Hard => 1.0,   // Hard attacks at parity
            GameDifficulty::Brutal => 1.2, // Brutal attacks when weaker
        };

        if military_strength < enemy_strength * strength_threshold {
            // Not strong enough yet, keep building
            self.prioritize_military_production()?;
            return Ok(());
        }

        // Select attack target based on strategic value
        let target = self.select_attack_target()?;

        if let Some(_target_location) = target {
            // Queue attack teams
            self.build_specific_ai_team("attack_force", true)?;

            // Update military stance
            self.military_state.current_military_stance = MilitaryStance::Aggressive;
        }

        Ok(())
    }

    /// Select best attack target based on strategic priorities
    /// Considers: economy disruption, defensive weakness, strategic value
    pub(super) fn select_attack_target(&self) -> Result<Option<Coord3D>, AiError> {
        // Wave 255: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        // Priority order (matches C++ AIPlayer behavior):
        // 1. Enemy supply centers (economy disruption)
        // 2. Enemy production facilities (tactical advantage)
        // 3. Enemy defenses (if we can win)
        // 4. Enemy command center (decisive strike)

        let list = player_list().read().map_err(|_| AiError::LockFailed)?;
        let mut best: Option<(f32, Coord3D)> = None;

        for (idx, player_arc) in list.iter().enumerate() {
            if idx as u32 == self.player_id {
                continue;
            }
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Neutral {
                continue;
            }
            for obj_id in player_guard.get_all_objects() {
                if let Some((score, pos)) = OBJECT_REGISTRY
                    .with_object(obj_id, |obj_guard| {
                        if obj_guard.is_destroyed() {
                            return None;
                        }
                        let score = self.score_attack_target(obj_guard);
                        Some((score, *obj_guard.get_position()))
                    })
                    .flatten()
                {
                    if best
                        .map(|(best_score, _)| score > best_score)
                        .unwrap_or(true)
                    {
                        best = Some((score, pos));
                    }
                }
            }
        }

        Ok(best.map(|(_, pos)| pos))
    }

    pub(super) fn score_attack_target(&self, obj: &Object) -> f32 {
        let mut score = if obj.is_kind_of(KindOf::SupplySource)
            || obj.is_kind_of(KindOf::ResourceNode)
            || obj.is_kind_of(KindOf::FSSupplyCenter)
            || obj.is_kind_of(KindOf::FSSupplyDropzone)
            || obj.is_kind_of(KindOf::Refinery)
        {
            0.9
        } else if obj.is_kind_of(KindOf::CommandCenter) || obj.is_kind_of(KindOf::KeyStructure) {
            1.0
        } else if obj.is_kind_of(KindOf::Factory)
            || obj.is_kind_of(KindOf::FSWarfactory)
            || obj.is_kind_of(KindOf::FSAirfield)
            || obj.is_kind_of(KindOf::FSBarracks)
        {
            0.8
        } else if obj.is_kind_of(KindOf::PowerPlant) || obj.is_kind_of(KindOf::FSPower) {
            0.7
        } else if obj.is_kind_of(KindOf::Defense) {
            0.6
        } else if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Building) {
            0.5
        } else if obj.is_kind_of(KindOf::Vehicle) || obj.is_kind_of(KindOf::Infantry) {
            0.3
        } else {
            0.2
        };

        let health = obj.get_health_percentage().clamp(0.0, 1.0);
        score *= 0.7 + (1.0 - health) * 0.3;

        if let Some(base_center) = self.get_base_center() {
            let dx = obj.get_position().x - base_center.x;
            let dy = obj.get_position().y - base_center.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let dist_factor = (1.0 / (1.0 + dist / 500.0)).clamp(0.2, 1.0);
            score *= dist_factor;
        }

        score.clamp(0.0, 1.0)
    }

    /// Initiate expansion to new location
    pub(super) fn initiate_expansion(&mut self) -> Result<(), AiError> {
        // Queue dozer and expansion buildings
        Ok(())
    }

    /// Prioritize economic buildings
    pub(super) fn prioritize_economic_buildings(&mut self) -> Result<(), AiError> {
        self.build_specific_ai_building("SupplyCenter")?;
        Ok(())
    }

    /// Prioritize tech upgrades
    pub(super) fn prioritize_tech_upgrades(&mut self) -> Result<(), AiError> {
        // Queue upgrades from skillset
        Ok(())
    }

    /// Initiate harassment attacks
    pub(super) fn initiate_harassment(&mut self) -> Result<(), AiError> {
        // Build fast units for hit-and-run
        Ok(())
    }

    /// Launch all-out attack
    pub(super) fn launch_all_out_attack(&mut self) -> Result<(), AiError> {
        // Send all military units to attack
        Ok(())
    }

    /// Analyze current economic situation
    /// Matches C++ AIPlayer economic analysis
    /// Updates resource tracking, income rates, and economic pressure
    pub(super) fn analyze_economic_situation(&mut self) -> Result<(), AiError> {
        let current_resources = if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    let money = player_guard.get_money().get_money();
                    let power = player_guard.get_energy().get_power() as i32;
                    self.economic_state
                        .current_resources
                        .insert("money".to_string(), money);
                    self.economic_state
                        .current_resources
                        .insert("power".to_string(), power);
                    self.economic_state.resource_income_rate.insert(
                        "money".to_string(),
                        player_guard.get_money().get_income_rate(),
                    );
                    self.economic_state.power_shortage = power < 0;
                    self.economic_state.supply_shortage = money < RESOURCES_POOR;
                    self.economic_state.economic_pressure = if money < RESOURCES_POOR {
                        0.8
                    } else if money > RESOURCES_WEALTHY {
                        0.2
                    } else {
                        0.5
                    };
                    money
                } else {
                    self.economic_state
                        .current_resources
                        .get("money")
                        .copied()
                        .unwrap_or(0)
                }
            } else {
                self.economic_state
                    .current_resources
                    .get("money")
                    .copied()
                    .unwrap_or(0)
            }
        } else {
            self.economic_state
                .current_resources
                .get("money")
                .copied()
                .unwrap_or(0)
        };

        // C++ AIPlayer uses TheAI->getAiData() thresholds (retail Wealthy=7000, Poor=2000).
        let ai_store = the_ai();let (poor_threshold, wealthy_threshold) = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    let poor = if data.resources_poor > 0 {
                        data.resources_poor
                    } else {
                        RESOURCES_POOR
                    };
                    let wealthy = if data.resources_wealthy > 0 {
                        data.resources_wealthy
                    } else {
                        RESOURCES_WEALTHY
                    };
                    (poor, wealthy)
                })
            })
            .unwrap_or((RESOURCES_POOR, RESOURCES_WEALTHY));

        // Update strategic decision maker's resource management
        self.strategic_decision_maker.resources.update(
            current_resources,
            wealthy_threshold,
            poor_threshold,
        );

        // Calculate economic pressure based on resources and income
        // High pressure = need more income, low resources
        self.economic_state.economic_pressure = if current_resources < poor_threshold {
            0.9 // Very high pressure - need supply centers urgently
        } else if current_resources < wealthy_threshold {
            0.5 // Moderate pressure - could use more income
        } else {
            0.2 // Low pressure - economy is good
        };

        // Difficulty affects economic pressure tolerance
        // Easy AI more conservative, Hard AI more aggressive with spending
        self.economic_state.economic_pressure *= match self.difficulty {
            GameDifficulty::Easy => 1.3,   // More cautious
            GameDifficulty::Normal => 1.0, // Standard
            GameDifficulty::Hard => 0.8,   // More aggressive
            GameDifficulty::Brutal => 0.6, // Very aggressive
        };

        // Check for supply shortage (count active supply trucks)
        // This would scan player units for KINDOF_HARVESTER
        let active_harvesters = self.count_active_harvesters();
        let desired_harvesters = 3 * self.count_supply_centers(); // 3 per center
        self.economic_state.supply_shortage = active_harvesters < desired_harvesters;

        // Check for power shortage (scan for power plants vs power usage)
        self.economic_state.power_shortage = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_energy().is_low_power())
            })
            .unwrap_or(false);

        Ok(())
    }

    /// Count number of active supply centers
    pub(super) fn count_supply_centers(&self) -> usize {
        // Wave 255: empty dual-world → zero.
        if dual_world_registry_unavailable() {
            return 0;
        }

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let is_supply = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    obj_guard.is_kind_of(KindOf::SupplySource)
                        || obj_guard.is_kind_of(KindOf::ResourceNode)
                        || obj_guard.is_kind_of(KindOf::FSSupplyCenter)
                        || obj_guard.is_kind_of(KindOf::FSSupplyDropzone)
                        || obj_guard.is_kind_of(KindOf::Refinery)
                })
                .unwrap_or(false);
            if is_supply {
                count += 1;
            }
        }
        count
    }

    /// Calculate average base health from all structures
    /// Used for strategic decision making
    pub(super) fn calculate_base_health(&self) -> f32 {
        // Wave 255: empty dual-world → zero.
        if dual_world_registry_unavailable() {
            return 0.0;
        }

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 1.0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 1.0;
        };
        let mut total = 0.0;
        let mut count = 0.0;
        for obj_id in player_guard.get_all_objects() {
            let Some(pct) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if !obj_guard.is_kind_of(KindOf::Structure)
                        && !obj_guard.is_kind_of(KindOf::Building)
                    {
                        return None;
                    }
                    Some(obj_guard.get_health_percentage())
                })
                .flatten()
            else {
                continue;
            };
            total += pct;
            count += 1.0;
        }
        if count > 0.0 {
            (total / count).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Check if strategy should be changed
    pub(super) fn should_change_strategy(&self) -> Result<bool, AiError> {
        let current_frame = TheGameLogic::get_frame();
        let time_in_strategy =
            current_frame.saturating_sub(self.strategy_state.last_strategy_change);
        let time_threshold = LOGICFRAMES_PER_SECOND * 60;

        if time_in_strategy > time_threshold {
            return Ok(true);
        }

        if self.threat_assessment.overall_threat_level > 0.7
            && self.strategy_state.current_strategy != AiStrategy::Turtle
        {
            return Ok(true);
        }

        if self.economic_state.economic_pressure > 0.8
            && self.strategy_state.current_strategy != AiStrategy::Economic
        {
            return Ok(true);
        }

        if self.military_state.enemy_strength_estimate
            > self.military_state.total_military_strength * 1.2
            && self.strategy_state.current_strategy != AiStrategy::Turtle
        {
            return Ok(true);
        }

        Ok(false)
    }

    /// Prioritize defensive buildings
    /// Adds defensive structures to construction queue based on threat level
    pub(super) fn prioritize_defensive_buildings(&mut self) -> Result<(), AiError> {
        // Add defensive structures to construction priorities
        let defensive_priority = ConstructionPriority {
            building_type: "GuardTower".to_string(),
            priority: 5, // High priority for defense
            prerequisites_met: true,
            max_count: Some(4), // Build up to 4 guard towers
            current_count: 0,
            desired_location: None,
            desired_angle: None,
        };

        self.construction_priorities.push(defensive_priority);

        // Could integrate with build order optimizer for more sophisticated prioritization
        // For now, direct insertion into construction queue is sufficient

        Ok(())
    }

    /// Determine optimal strategy for current situation
    /// Matches C++ AIPlayer strategic decision making
    /// Considers resources, threats, game phase, and difficulty
    pub(super) fn determine_optimal_strategy(&self) -> Result<AiStrategy, AiError> {
        // Strategy selection based on multiple factors
        let current_money = self
            .economic_state
            .current_resources
            .get("money")
            .copied()
            .unwrap_or(0);
        let military_strength = self.military_state.total_military_strength;
        let enemy_strength = self.military_state.enemy_strength_estimate;
        let threat_level = self.threat_assessment.overall_threat_level;

        // Early game (low resources, low military)
        if current_money < 2000 && military_strength < 50.0 {
            return Ok(match self.difficulty {
                GameDifficulty::Easy => AiStrategy::Turtle, // Play safe
                GameDifficulty::Normal => AiStrategy::Economic, // Build economy
                GameDifficulty::Hard => AiStrategy::Rush,   // Early pressure
                GameDifficulty::Brutal => AiStrategy::Rush, // Aggressive start
            });
        }

        // Under heavy threat - defend
        if threat_level > 0.7 || enemy_strength > military_strength * 1.5 {
            return Ok(AiStrategy::Turtle);
        }

        // Strong military advantage - attack
        if military_strength > enemy_strength * 1.3 {
            return Ok(match self.difficulty {
                GameDifficulty::Easy => AiStrategy::Balanced, // Cautious attack
                GameDifficulty::Normal => AiStrategy::Balanced, // Standard attack
                GameDifficulty::Hard => AiStrategy::AllOut,   // Aggressive
                GameDifficulty::Brutal => AiStrategy::AllOut, // Very aggressive
            });
        }

        // Good economy but weak military - tech rush
        if current_money > 8000 && military_strength < enemy_strength {
            return Ok(AiStrategy::TechRush);
        }

        // Resource shortage - focus economy
        if self.economic_state.economic_pressure > 0.6 {
            return Ok(AiStrategy::Economic);
        }

        // Default to balanced approach
        Ok(AiStrategy::Balanced)
    }

    /// Change to new strategy
    pub(super) fn change_strategy(
        &mut self,
        new_strategy: AiStrategy,
        current_frame: u32,
    ) -> Result<(), AiError> {
        self.strategy_state.current_strategy = new_strategy;
        self.strategy_state.last_strategy_change = current_frame;
        self.strategy_state.time_in_strategy = 0;
        self.strategy_state.strategy_confidence = 1.0;

        Ok(())
    }

    /// Add work orders for a specific team
    /// C++ work-order composition for a team prototype (optional then required).
    pub(super) fn add_work_orders_for_team(
        &mut self,
        team: &mut TeamInQueue,
        team_name: &str,
    ) -> Result<(), AiError> {
        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok(());
        };
        if let Some(proto) = factory_guard.find_team_prototype(team_name) {
            let mut orders = Vec::new();
            // Optional: max-min
            for unit in proto.units_info() {
                if unit.unit_thing_name.is_empty() {
                    continue;
                }
                let count = (unit.max_units - unit.min_units).max(0);
                if count <= 0 {
                    continue;
                }
                let mut order = WorkOrder::new(unit.unit_thing_name.to_string());
                order.num_required = count;
                order.required = false;
                orders.insert(0, order);
            }
            // Required: min
            for unit in proto.units_info() {
                if unit.unit_thing_name.is_empty() {
                    continue;
                }
                let count = unit.min_units.max(0);
                if count <= 0 {
                    continue;
                }
                let mut order = WorkOrder::new(unit.unit_thing_name.to_string());
                order.num_required = count;
                order.required = true;
                orders.insert(0, order);
            }
            team.work_orders = orders;
            return Ok(());
        }

        Ok(())
    }

    /// Determine appropriate base defense structure
    pub(super) fn determine_base_defense_structure(&self, flank: bool) -> Result<String, AiError> {
        // Choose defense based on:
        // - Faction
        // - Current threats
        // - Resource availability
        // - Strategic position (front vs flank)

        if flank {
            Ok("PatriotMissileBattery".to_string())
        } else {
            Ok("FirebasePatriotMissileBattery".to_string())
        }
    }

    /// Find suitable location for defense structure
    pub(super) fn find_defense_location(&self, flank: bool) -> Result<Coord3D, AiError> {
        let base = self
            .get_base_center()
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
        let offset = if flank { 160.0 } else { 80.0 };
        let candidate = Coord3D::new(base.x + offset, base.y, base.z);

        let mut position = candidate;
        if let Some(terrain) = TheTerrainLogic::get() {
            position.z = terrain.get_ground_height(position.x, position.y, None);
        }

        // Host residual helper (C++ solo AI has no buildAIBaseDefenseStructure).
        // Use same legalize flags as skirmish base defense when validating.
        let validator = FoundationValidator::from_build_options(
            LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
        );
        if validator
            .validate_placement(
                &position,
                "PatriotMissileBattery",
                0.0,
                self.player_id as ObjectID,
            )
            .is_ok()
        {
            return Ok(position);
        }

        Ok(base)
    }

    /// Queue structure for construction
    pub(super) fn queue_structure_construction(
        &mut self,
        structure_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<(), AiError> {
        // Add to construction queue
        let priority = ConstructionPriority {
            building_type: structure_name.to_string(),
            priority: 10,
            prerequisites_met: true,
            max_count: None,
            current_count: 0,
            desired_location: Some(location),
            desired_angle: Some(angle),
        };

        self.construction_priorities.push(priority);
        Ok(())
    }

    /// Update construction priorities based on current needs
    pub(super) fn update_construction_priorities(&mut self) -> Result<(), AiError> {
        // Remove completed priorities
        self.construction_priorities.retain(|p| {
            if let Some(max) = p.max_count {
                p.current_count < max
            } else {
                true
            }
        });

        // Sort by priority
        self.construction_priorities.sort_by_key(|p| p.priority);

        Ok(())
    }
}
