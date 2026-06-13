//! Build cost and time calculation system
//!
//! Faithfully ports the C++ ThingTemplate::calcCostToBuild and calcTimeToBuild
//! functions from ThingTemplate.cpp, including all modifiers and bonuses.

use crate::common::*;
use game_engine::common::global_data;

/// Global constants for build time modifiers
/// Matches C++ GlobalData in TheGlobalData
#[derive(Debug, Clone)]
pub struct GlobalBuildModifiers {
    /// Low energy penalty modifier (default 0.5 for 50% penalty)
    pub low_energy_penalty_modifier: f32,
    /// Minimum production speed when low on energy (default 0.5)
    pub min_low_energy_production_speed: f32,
    /// Maximum production speed penalty (default 0.9)
    pub max_low_energy_production_speed: f32,
    /// Multiple factory bonus (default 0.8 for 20% faster per factory)
    pub multiple_factory_bonus: f32,
    /// Logic frames per second (default 30)
    pub logic_frames_per_second: u32,
}

impl Default for GlobalBuildModifiers {
    fn default() -> Self {
        // Matches C++ TheGlobalData default values
        Self {
            low_energy_penalty_modifier: 0.5,
            min_low_energy_production_speed: 0.5,
            max_low_energy_production_speed: 0.9,
            multiple_factory_bonus: 0.8,
            logic_frames_per_second: 30,
        }
    }
}

impl GlobalBuildModifiers {
    /// Build modifiers sourced from runtime global data (TheGlobalData).
    pub fn from_global_data() -> Self {
        let guard = global_data::read();
        Self {
            low_energy_penalty_modifier: guard.low_energy_penalty_modifier,
            min_low_energy_production_speed: guard.min_low_energy_production_speed,
            max_low_energy_production_speed: guard.max_low_energy_production_speed,
            multiple_factory_bonus: guard.multiple_factory,
            logic_frames_per_second: crate::common::LOGICFRAMES_PER_SECOND,
        }
    }
}

/// Player-specific build modifiers
#[derive(Debug, Clone)]
pub struct PlayerBuildModifiers {
    /// Production cost change percent for specific unit (-.2 = 20% cheaper)
    pub production_cost_change_percent: f32,
    /// Production cost change based on KindOf flags
    pub production_cost_change_by_kind: f32,
    /// Handicap cost multiplier
    pub handicap_cost_multiplier: f32,
    /// Production time change percent
    pub production_time_change_percent: f32,
    /// Handicap time multiplier
    pub handicap_time_multiplier: f32,
    /// Energy supply ratio (0.0 to 1.0+)
    pub energy_supply_ratio: f32,
    /// Instant build cheat enabled
    pub builds_instantly: bool,
}

impl Default for PlayerBuildModifiers {
    fn default() -> Self {
        Self {
            production_cost_change_percent: 0.0,
            production_cost_change_by_kind: 1.0,
            handicap_cost_multiplier: 1.0,
            production_time_change_percent: 0.0,
            handicap_time_multiplier: 1.0,
            energy_supply_ratio: 1.0,
            builds_instantly: false,
        }
    }
}

/// Build facility context for multi-factory bonus
#[derive(Debug, Clone)]
pub struct BuildFacilityContext {
    /// Number of build facilities of the same type
    pub facility_count: i32,
    /// Whether this unit appears at rally point
    pub appears_at_rally_point: bool,
}

impl Default for BuildFacilityContext {
    fn default() -> Self {
        Self {
            facility_count: 1,
            appears_at_rally_point: false,
        }
    }
}

/// Build cost and time calculator
/// Matches C++ ThingTemplate::calcCostToBuild and calcTimeToBuild logic
#[derive(Debug)]
pub struct BuildCostCalculator {
    global_modifiers: GlobalBuildModifiers,
}

impl BuildCostCalculator {
    /// Create a new calculator with default global modifiers
    pub fn new() -> Self {
        Self {
            global_modifiers: GlobalBuildModifiers::default(),
        }
    }

    /// Create with custom global modifiers
    pub fn with_modifiers(modifiers: GlobalBuildModifiers) -> Self {
        Self {
            global_modifiers: modifiers,
        }
    }

    /// Calculate cost to build a unit/structure
    ///
    /// Matches C++ ThingTemplate.cpp line 1508:
    /// ```cpp
    /// Int ThingTemplate::calcCostToBuild( const Player* player) const
    /// {
    ///     Real factionModifier = 1 + player->getProductionCostChangePercent( getName() );
    ///     factionModifier *= player->getProductionCostChangeBasedOnKindOf( m_kindof );
    ///     return getBuildCost() * factionModifier * player->getHandicap()->getHandicap(Handicap::BUILDCOST, this);
    /// }
    /// ```
    pub fn calc_cost_to_build(
        &self,
        base_cost: i32,
        player_modifiers: &PlayerBuildModifiers,
    ) -> i32 {
        if base_cost == 0 {
            return 0;
        }

        // Apply faction modifier (-.2 equals 20% cheaper)
        let faction_modifier = 1.0 + player_modifiers.production_cost_change_percent;

        // Apply KindOf-based modifier
        let total_modifier = faction_modifier * player_modifiers.production_cost_change_by_kind;

        // Apply handicap
        let final_modifier = total_modifier * player_modifiers.handicap_cost_multiplier;

        // C++ returns a Real expression as Int, truncating at conversion.
        ((base_cost as f32) * final_modifier) as i32
    }

    /// Calculate time to build a unit/structure in logic frames
    ///
    /// Matches C++ ThingTemplate.cpp lines 1524-1576:
    /// Applies:
    /// 1. Base build time conversion to frames
    /// 2. Handicap time multiplier
    /// 3. Faction time modifier
    /// 4. Debug instant build check
    /// 5. Energy supply penalty
    /// 6. Multiple factory bonus
    pub fn calc_time_to_build(
        &self,
        base_build_time_seconds: f32,
        player_modifiers: &PlayerBuildModifiers,
        facility_context: Option<&BuildFacilityContext>,
    ) -> u32 {
        // Convert seconds to logic frames
        let mut build_time = (base_build_time_seconds
            * (self.global_modifiers.logic_frames_per_second as f32))
            as i32;

        // Apply handicap multiplier
        build_time = ((build_time as f32) * player_modifiers.handicap_time_multiplier) as i32;

        // Apply faction time modifier (1 + percent change)
        let faction_modifier = 1.0 + player_modifiers.production_time_change_percent;
        build_time = ((build_time as f32) * faction_modifier) as i32;

        if player_modifiers.builds_instantly {
            build_time = 1;
        }

        // Apply energy supply penalty
        // Matches C++ lines 1540-1555
        build_time = self.apply_energy_penalty(build_time, player_modifiers.energy_supply_ratio);

        // Apply multiple factory bonus if applicable
        if let Some(context) = facility_context {
            if context.appears_at_rally_point && context.facility_count > 1 {
                build_time = self.apply_factory_bonus(build_time, context.facility_count);
            }
        }

        build_time.max(0) as u32
    }

    /// Apply energy supply penalty to build time
    ///
    /// Matches C++ ThingTemplate.cpp lines 1540-1555:
    /// ```cpp
    /// Real EnergyPercent = player->getEnergy()->getEnergySupplyRatio();
    /// if (EnergyPercent > 1.0f) EnergyPercent = 1.0f;
    /// Real EnergyShort = 1.0f - EnergyPercent;
    /// EnergyShort *= TheGlobalData->m_LowEnergyPenaltyModifier;
    /// Real penaltyRate = 1.0f - EnergyShort;
    /// penaltyRate = max(penaltyRate, TheGlobalData->m_MinLowEnergyProductionSpeed);
    /// if( EnergyPercent < 1.0f )
    ///     penaltyRate = min(penaltyRate, TheGlobalData->m_MaxLowEnergyProductionSpeed);
    /// if (penaltyRate <= 0.0f) penaltyRate = 0.01f;
    /// buildTime /= penaltyRate;
    /// ```
    fn apply_energy_penalty(&self, build_time: i32, energy_ratio: f32) -> i32 {
        let energy_percent = energy_ratio.min(1.0);

        // Calculate how short we are on energy
        let energy_short = 1.0 - energy_percent;
        let energy_short = energy_short * self.global_modifiers.low_energy_penalty_modifier;

        // Calculate penalty rate
        let mut penalty_rate = 1.0 - energy_short;

        // Bind to minimum speed
        penalty_rate = penalty_rate.max(self.global_modifiers.min_low_energy_production_speed);

        // If we're short on energy, cap at max penalty
        if energy_percent < 1.0 {
            penalty_rate = penalty_rate.min(self.global_modifiers.max_low_energy_production_speed);
        }

        // Ensure not zero
        let penalty_rate = if penalty_rate <= 0.0 {
            0.01
        } else {
            penalty_rate
        };

        // Apply penalty (divide time by rate means slower)
        ((build_time as f32) / penalty_rate) as i32
    }

    /// Apply multiple factory bonus
    ///
    /// Matches C++ ThingTemplate.cpp lines 1557-1572:
    /// ```cpp
    /// Int count = 0;
    /// player->countObjectsByThingTemplate(1, &tmpl, false, &count);
    /// Real factoryMult = TheGlobalData->m_MultipleFactory;
    /// if (factoryMult > 0.0f) {
    ///     for(int i=0; i < count - 1; i++)
    ///         buildTime *= factoryMult;
    /// }
    /// ```
    fn apply_factory_bonus(&self, build_time: i32, facility_count: i32) -> i32 {
        if self.global_modifiers.multiple_factory_bonus <= 0.0 {
            return build_time;
        }

        let mut result = build_time;

        // Apply bonus for each additional factory (count - 1)
        for _ in 0..(facility_count - 1).max(0) {
            result = ((result as f32) * self.global_modifiers.multiple_factory_bonus) as i32;
        }

        result
    }

    /// Get global modifiers (read-only)
    pub fn global_modifiers(&self) -> &GlobalBuildModifiers {
        &self.global_modifiers
    }

    /// Update global modifiers
    pub fn set_global_modifiers(&mut self, modifiers: GlobalBuildModifiers) {
        self.global_modifiers = modifiers;
    }
}

impl Default for BuildCostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cost_calculation() {
        let calc = BuildCostCalculator::new();
        let mods = PlayerBuildModifiers::default();

        // No modifiers = same cost
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 1000);

        // Zero cost stays zero
        assert_eq!(calc.calc_cost_to_build(0, &mods), 0);
    }

    #[test]
    fn test_cost_discount() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // 20% discount (-.2)
        mods.production_cost_change_percent = -0.2;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 800);

        // 50% discount
        mods.production_cost_change_percent = -0.5;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 500);
    }

    #[test]
    fn test_cost_increase() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // 25% increase
        mods.production_cost_change_percent = 0.25;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 1250);

        // C++ truncates when converting the Real expression back to Int.
        mods.production_cost_change_percent = 0.333;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 1333);
    }

    #[test]
    fn test_handicap_cost() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // 80% cost (easier difficulty)
        mods.handicap_cost_multiplier = 0.8;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 800);

        // 120% cost (harder difficulty)
        mods.handicap_cost_multiplier = 1.2;
        assert_eq!(calc.calc_cost_to_build(1000, &mods), 1200);
    }

    #[test]
    fn test_basic_time_calculation() {
        let calc = BuildCostCalculator::new();
        let mods = PlayerBuildModifiers::default();

        // 10 seconds at 30 FPS = 300 frames
        assert_eq!(calc.calc_time_to_build(10.0, &mods, None), 300);

        // 5 seconds = 150 frames
        assert_eq!(calc.calc_time_to_build(5.0, &mods, None), 150);
    }

    #[test]
    fn test_instant_build() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        mods.builds_instantly = true;

        // C++ sets buildTime to 1, then still applies later modifiers.
        assert_eq!(calc.calc_time_to_build(10.0, &mods, None), 1);
        assert_eq!(calc.calc_time_to_build(100.0, &mods, None), 1);

        mods.energy_supply_ratio = 0.5;
        assert_eq!(calc.calc_time_to_build(10.0, &mods, None), 1);
    }

    #[test]
    fn test_energy_penalty() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // Full energy = no penalty
        mods.energy_supply_ratio = 1.0;
        let base_time = calc.calc_time_to_build(10.0, &mods, None);
        assert_eq!(base_time, 300);

        // 50% energy = slower (default penalty modifier is 0.5)
        // 50% short * 0.5 modifier = 25% penalty
        // penalty_rate = 1.0 - 0.25 = 0.75
        // time = 300 / 0.75 = 400
        mods.energy_supply_ratio = 0.5;
        let penalized_time = calc.calc_time_to_build(10.0, &mods, None);
        assert!(penalized_time > base_time);
        assert_eq!(penalized_time, 400);

        // Zero energy = minimum speed
        // With default min of 0.5, time = 300 / 0.5 = 600
        mods.energy_supply_ratio = 0.0;
        let min_speed_time = calc.calc_time_to_build(10.0, &mods, None);
        assert_eq!(min_speed_time, 600);
    }

    #[test]
    fn test_multiple_factory_bonus() {
        let calc = BuildCostCalculator::new();
        let mods = PlayerBuildModifiers::default();

        // 1 factory = base time
        let context1 = BuildFacilityContext {
            facility_count: 1,
            appears_at_rally_point: true,
        };
        let time1 = calc.calc_time_to_build(10.0, &mods, Some(&context1));
        assert_eq!(time1, 300);

        // 2 factories = 0.8x time (20% faster)
        let context2 = BuildFacilityContext {
            facility_count: 2,
            appears_at_rally_point: true,
        };
        let time2 = calc.calc_time_to_build(10.0, &mods, Some(&context2));
        assert_eq!(time2, 240); // 300 * 0.8 = 240

        // 3 factories = 0.8 * 0.8 = 0.64x time
        let context3 = BuildFacilityContext {
            facility_count: 3,
            appears_at_rally_point: true,
        };
        let time3 = calc.calc_time_to_build(10.0, &mods, Some(&context3));
        assert_eq!(time3, 192); // 300 * 0.64 = 192

        // Non-rally-point units don't get bonus
        let context_no_rally = BuildFacilityContext {
            facility_count: 3,
            appears_at_rally_point: false,
        };
        let time_no_rally = calc.calc_time_to_build(10.0, &mods, Some(&context_no_rally));
        assert_eq!(time_no_rally, 300); // No bonus
    }

    #[test]
    fn test_combined_modifiers() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // Combine time change and energy penalty
        mods.production_time_change_percent = -0.2; // 20% faster
        mods.energy_supply_ratio = 0.8; // 80% energy

        let context = BuildFacilityContext {
            facility_count: 2,
            appears_at_rally_point: true,
        };

        // Base: 10 seconds = 300 frames
        // After faction: 300 * 0.8 = 240 frames
        // After energy: 240 / 0.9 = 266 frames after C++ truncation
        // After factory: 266 * 0.8 = 212 frames after C++ truncation
        let time = calc.calc_time_to_build(10.0, &mods, Some(&context));
        assert_eq!(time, 212);
    }

    #[test]
    fn test_handicap_time() {
        let calc = BuildCostCalculator::new();
        let mut mods = PlayerBuildModifiers::default();

        // Easy difficulty (faster build)
        mods.handicap_time_multiplier = 0.7;
        let easy_time = calc.calc_time_to_build(10.0, &mods, None);
        assert_eq!(easy_time, 210); // 300 * 0.7

        // Hard difficulty (slower build)
        mods.handicap_time_multiplier = 1.5;
        let hard_time = calc.calc_time_to_build(10.0, &mods, None);
        assert_eq!(hard_time, 450); // 300 * 1.5
    }

    #[test]
    fn test_sub_frame_time_truncates_like_cpp() {
        let calc = BuildCostCalculator::new();
        let mods = PlayerBuildModifiers::default();

        // C++ stores the frame count in Int immediately; sub-frame values become 0.
        let time = calc.calc_time_to_build(0.001, &mods, None);
        assert_eq!(time, 0);
    }
}
