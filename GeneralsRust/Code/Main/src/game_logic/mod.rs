pub mod audio_dispatch_impl;
pub mod buildings;
pub mod combat;
pub mod combat_particles;
pub mod game_logic;
pub mod mission_scripts;
pub mod object;
pub mod partition_manager;
pub mod pathfinding;
pub mod radar_notifications;
pub mod resources;
pub mod script_events;
pub mod script_loader;
pub mod special_power_strikes;
pub mod host_upgrades;
pub mod host_cash_bounty;
pub mod host_black_market;
pub mod host_oil_derrick;
pub mod host_hacker_income;
pub mod host_mines;
pub mod host_radar;
pub mod host_radar_scan;
pub mod host_spy_satellite;
pub mod host_cia_intelligence;
pub mod host_hero_abilities;
pub mod host_car_bomb;
pub mod host_repair;
pub mod host_heal;
pub mod host_propaganda;
pub mod host_base_defense;
pub mod host_ecm_jam;
pub mod host_emp_pulse;
pub mod host_frenzy;
pub mod host_emergency_repair;
pub mod host_gps_scrambler;
pub mod host_leaflet_drop;
pub mod host_sneak_attack;
pub mod host_point_defense;
pub mod host_neutron_shell;
pub mod host_paradrop;
pub mod host_ambush;
pub mod host_firewall;
pub mod host_inferno_cannon;
pub mod host_battle_bus;
pub mod terrain;
pub mod thing;
pub mod units;
pub mod victory;
pub mod victory_conditions;
pub mod locomotor_bootstrap;
pub mod weapon_bootstrap;

pub use buildings::*;
pub use combat::*;
pub use combat_particles::{
    CombatParticleKind, CombatParticleRegistry, CombatParticleSystemEntry,
};
pub use special_power_strikes::{
    HostRadiationField, HostSpecialPowerStrike, HostSpecialPowerStrikeRegistry, HostStrikePhase,
    HostSuperweaponKind, NUKE_RADIATION_DAMAGE_PER_TICK, NUKE_RADIATION_DURATION_FRAMES,
    NUKE_RADIATION_RADIUS, NUKE_RADIATION_TICK_INTERVAL_FRAMES,
};
pub use host_upgrades::{
    HostUpgradeKind, HostUpgradePhase, HostUpgradeRegistry, HostUpgradeResearch,
};
pub use host_cash_bounty::{
    cash_bounty_percent_for_science, compute_bounty_award, HostCashBountyRegistry,
    CASH_BOUNTY1_PERCENT, CASH_BOUNTY2_PERCENT, CASH_BOUNTY3_PERCENT, SCIENCE_CASH_BOUNTY1,
    SCIENCE_CASH_BOUNTY2, SCIENCE_CASH_BOUNTY3,
};
pub use host_black_market::{
    deposit_interval_frames_from_ms, is_black_market_structure, is_black_market_template,
    is_legal_black_market_income_source, HostBlackMarketRegistry, BLACK_MARKET_DEPOSIT_AMOUNT,
    BLACK_MARKET_DEPOSIT_AUDIO, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    BLACK_MARKET_DEPOSIT_TIMING_MS,
};
pub use host_oil_derrick::{
    is_legal_oil_derrick_income_source, is_oil_derrick_structure, is_oil_derrick_template,
    HostOilDerrickRegistry, OIL_DERRICK_DEPOSIT_AMOUNT, OIL_DERRICK_DEPOSIT_AUDIO,
    OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES, OIL_DERRICK_DEPOSIT_TIMING_MS,
    OIL_DERRICK_INITIAL_CAPTURE_BONUS, OIL_DERRICK_CAPTURE_BONUS_AUDIO,
};
pub use host_hacker_income::{
    cash_amount_for_level, cash_interval_frames, is_hacker_template, is_internet_center_template,
    is_legal_hacker_income_source, HostHackerIncomeRegistry, HACKER_CASH_INTERVAL_FAST_FRAMES,
    HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_PING_AUDIO, HACKER_CASH_REGULAR,
    HACKER_XP_PER_CASH_UPDATE,
};
pub use host_mines::{
    can_clear_mine_kind, is_mine_clearer, HostMineData, HostMineDetonateReason, HostMineKind,
    HostMineDetonationPlan, DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE,
    MINE_CLEARED_AUDIO,
};
pub use host_radar::{
    is_legal_radar_provider, is_radar_command_center_template, is_radar_provider_template,
    is_radar_van_template, HostRadarRegistry, RADAR_OFFLINE_AUDIO, RADAR_ONLINE_AUDIO,
};
pub use host_radar_scan::{
    HostRadarScan, HostRadarScanRegistry, RADAR_SCAN_ACTIVATE_AUDIO, RADAR_SCAN_DURATION_FRAMES,
    RADAR_SCAN_RADIUS,
};
pub use host_spy_satellite::{
    HostSpySatellite, HostSpySatelliteRegistry, SPY_SATELLITE_ACTIVATE_AUDIO,
    SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_RADIUS,
};
pub use host_cia_intelligence::{
    HostCiaIntelligence, HostCiaIntelligenceRegistry, HostCiaIntelligenceSpiedUnit,
    CIA_INTELLIGENCE_ACTIVATE_AUDIO, CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
    CIA_INTELLIGENCE_DURATION_FRAMES,
};
pub use host_hero_abilities::{
    HostHeroAbilityRegistry, DISABLE_VEHICLE_HACK_AUDIO, DISABLE_VEHICLE_HACK_DURATION_FRAMES,
    STEAL_CASH_DEFAULT_AMOUNT, SNIPE_VEHICLE_AUDIO, STEAL_CASH_AUDIO,
};
pub use host_car_bomb::{
    car_bomb_damage_at_distance, suicide_car_bomb_weapon, HostCarBombRegistry,
    CAR_BOMB_CONVERT_AUDIO, CAR_BOMB_DETONATE_AUDIO, HIJACK_AUDIO, SUICIDE_CAR_BOMB_ATTACK_RANGE,
    SUICIDE_CAR_BOMB_DAMAGE, SUICIDE_CAR_BOMB_RADIUS,
};
pub use host_heal::{
    is_ambulance_healer, is_legal_ambulance_infantry_heal_target, HOST_AMBULANCE_HEAL_RADIUS,
    HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC,
};
pub use host_propaganda::{
    is_legal_propaganda_target, is_propaganda_tower, propaganda_heal_amount,
    HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC, HOST_PROPAGANDA_TOWER_RADIUS,
    HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC, UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
};
pub use host_base_defense::{
    is_base_defense_structure, is_legal_base_defense_target, primary_weapon_name_for_defense,
    GATTLING_BUILDING_PRIMARY_WEAPON, PATRIOT_PRIMARY_WEAPON,
};
pub use host_ecm_jam::{
    is_ecm_jammer, is_legal_ecm_jam_target, HOST_ECM_JAM_RADIUS,
};
pub use host_emp_pulse::{
    is_legal_emp_disable_target, HostEmpPulse, HostEmpPulseRegistry, EMP_PULSE_ACTIVATE_AUDIO,
    EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
};
pub use host_frenzy::{
    is_legal_frenzy_target, HostFrenzy, HostFrenzyLevel, HostFrenzyRegistry,
    FRENZY_ACTIVATE_AUDIO, HOST_FRENZY_RADIUS,
};
pub use host_emergency_repair::{
    is_legal_emergency_repair_target, HostEmergencyRepair, HostEmergencyRepairLevel,
    HostEmergencyRepairRegistry, EMERGENCY_REPAIR_ACTIVATE_AUDIO, HOST_EMERGENCY_REPAIR_RADIUS,
};
pub use host_gps_scrambler::{
    is_legal_gps_scrambler_target, HostGpsScrambler, HostGpsScramblerRegistry,
    GPS_SCRAMBLER_ACTIVATE_AUDIO, HOST_GPS_SCRAMBLER_RADIUS,
};
pub use host_leaflet_drop::{
    is_legal_leaflet_disable_target, HostLeafletDropKind, HostLeafletDropMission,
    HostLeafletDropPhase, HostLeafletDropRegistry, HOST_LEAFLET_RADIUS,
    LEAFLET_DELAY_FRAMES, LEAFLET_DISABLED_DURATION_FRAMES,
};
pub use host_sneak_attack::{
    is_legal_sneak_shockwave_target, HostSneakAttackKind, HostSneakAttackMission,
    HostSneakAttackPhase, HostSneakAttackRegistry, GLA_SNEAK_TUNNEL_TEMPLATE,
    HOST_SNEAK_ATTACK_RADIUS, SNEAK_ATTACK_RESIDUAL_TEMPLATE, SNEAK_ATTACK_SHOCKWAVE_DAMAGE,
    SNEAK_ATTACK_SHOCKWAVE_RADIUS, SNEAK_ATTACK_SPAWN_DELAY_FRAMES,
};
pub use host_point_defense::{
    is_missile_name_residual, is_point_defense_carrier, is_primary_intercept_target,
    pdl_delay_frames, pdl_damage, pdl_fire_range, pdl_scan_range, AVENGER_PDL_FIRE_RANGE,
    PALADIN_PDL_FIRE_RANGE, PDL_INTERCEPT_AUDIO,
};
pub use host_neutron_shell::{
    is_nuke_cannon_template, neutron_effect_for_target, should_apply_neutron_blast, NeutronEffect,
    HOST_NEUTRON_BLAST_RADIUS, NUKE_CANNON_NEUTRON_WEAPON, NUKE_CANNON_PRIMARY_WEAPON,
    UPGRADE_CHINA_NEUTRON_SHELLS,
};
pub use host_paradrop::{
    HostParadropKind, HostParadropMission, HostParadropPhase, HostParadropRegistry,
    AMERICA_PARADROP_UNIT_COUNT, PARADROP_DROP_SPACING, PARADROP_RESIDUAL_TEMPLATE,
};
pub use host_ambush::{
    HostAmbushKind, HostAmbushMission, HostAmbushPhase, HostAmbushRegistry,
    AMBUSH_RESIDUAL_TEMPLATE, AMBUSH_SPAWN_RADIUS, GLA_AMBUSH1_UNIT_COUNT,
};
pub use host_firewall::{
    HostFireWall, HostFireWallRegistry, HostFireWallSegment, FIREWALL_ACTIVATE_AUDIO,
    FIREWALL_DAMAGE_PER_TICK, FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_RADIUS,
    FIREWALL_TICK_INTERVAL_FRAMES,
};
pub use host_inferno_cannon::{
    is_inferno_cannon_template, HostInfernoFireZone, HostInfernoFireZoneRegistry,
    INFERNO_CANNON_FIRE_AUDIO, INFERNO_CANNON_PRIMARY_WEAPON, INFERNO_CANNON_SHELL_DAMAGE,
    INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DURATION_FRAMES, INFERNO_FIRE_RADIUS,
    INFERNO_FIRE_TICK_INTERVAL_FRAMES,
};
pub use host_battle_bus::{
    battle_bus_passenger_dummy_weapon, is_battle_bus_template, rider_has_viable_weapon,
    HostBattleBusRegistry, BATTLE_BUS_TRANSPORT_SLOTS,
};
pub use game_logic::*;
pub use mission_scripts::*;
pub use object::*;
pub use partition_manager::*;
pub use pathfinding::*;
pub use radar_notifications::*;
pub use resources::*;
pub use script_events::*;
pub use script_loader::*;
pub use terrain::*;
pub use thing::*;
pub use units::*;
pub use victory::*;
pub use victory_conditions::*;
pub use locomotor_bootstrap::{
    ensure_host_locomotor_store, locomotor_name_for_unit, resolve_host_movement,
    BASIC_HUMAN_LOCOMOTOR, BATTLE_MASTER_LOCOMOTOR, CRUSADER_LOCOMOTOR, HUMVEE_LOCOMOTOR,
    REDGUARD_LOCOMOTOR, SCORPION_LOCOMOTOR, TECHNICAL_LOCOMOTOR,
};
pub use weapon_bootstrap::{
    ensure_host_weapon_store, primary_weapon_name_for_unit, secondary_weapon_name_for_unit,
    GATTLING_BUILDING_PRIMARY_WEAPON as HOST_GATTLING_BUILDING_PRIMARY_WEAPON,
    GLA_REBEL_PRIMARY_WEAPON, HUMVEE_PRIMARY_WEAPON, HUMVEE_SECONDARY_WEAPON,
    PATRIOT_PRIMARY_WEAPON as HOST_PATRIOT_PRIMARY_WEAPON, RANGER_PRIMARY_WEAPON,
    RANGER_SECONDARY_WEAPON, REDGUARD_PRIMARY_WEAPON,
};

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for game objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = ObjectId(0);

/// Team/faction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    GLA,
    USA,
    China,
    Neutral,
}

impl Team {
    /// Convert player ID to team
    pub fn from_player_id(player_id: u32) -> Self {
        match player_id {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
    }

    /// Get the team's primary color for UI display
    pub fn get_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.2, 0.4, 0.8, 1.0],     // Blue
            Team::China => [0.8, 0.2, 0.2, 1.0],   // Red
            Team::GLA => [0.8, 0.6, 0.2, 1.0],     // Desert/Tan
            Team::Neutral => [0.5, 0.5, 0.5, 1.0], // Gray
        }
    }

    /// Get the team's name as a string
    pub fn get_name(&self) -> &'static str {
        match self {
            Team::USA => "USA",
            Team::China => "China",
            Team::GLA => "GLA",
            Team::Neutral => "Neutral",
        }
    }

    /// Get the team's secondary color for highlights
    pub fn get_highlight_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.4, 0.6, 1.0, 1.0],     // Light blue
            Team::China => [1.0, 0.4, 0.4, 1.0],   // Light red
            Team::GLA => [1.0, 0.8, 0.4, 1.0],     // Light tan
            Team::Neutral => [0.7, 0.7, 0.7, 1.0], // Light gray
        }
    }
}

/// Object kinds for type checking and behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KindOf {
    Structure,
    Infantry,
    Vehicle,
    Aircraft,
    Projectile,
    Resource,
    Selectable,
    Attackable,
    CommandCenter,
    Worker,
    Hero,
    SupplyCenter,
    PowerPlant,
    FSBarracks,
    FSWarFactory,
    FSAirfield,
    FSInternetCenter,
    FSPower,
    FSBaseDefense,
    FSSupplyDropzone,
    FSSupplyCenter,
    FSSuperweapon,
    FSStrategyCenter,
    FSFake,
    FSTechnology,
    FSBlackMarket,
    FSAdvancedTech,
    Harvestable,
    /// C++ KINDOF_POWERED: object gets DISABLED_UNDERPOWERED when player
    /// power consumption exceeds supply (defenses, factories, etc).
    Powered,
}

/// Object status flags
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ObjectStatus {
    pub destroyed: bool,
    pub under_construction: bool,
    pub selected: bool,
    pub moving: bool,
    pub attacking: bool,
    pub airborne_target: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual (revealed by detector / temporary reveal).
    /// Stealthed + not detected => not targetable / not visible to enemies.
    pub detected: bool,
    /// C++ DISABLED_UNDERPOWERED: set when player's power supply < demand.
    pub disabled_underpowered: bool,
    /// C++ DISABLED_UNMANNED residual (DAMAGE_KILLPILOT / Jarmen Kell snipe).
    /// Vehicle stays alive but cannot act; team is typically Neutral.
    #[serde(default)]
    pub disabled_unmanned: bool,
    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    /// Vehicle stays alive on its team but cannot move/attack until frame expires.
    #[serde(default)]
    pub disabled_hacked: bool,
    /// Absolute host logic frame when DISABLED_HACKED expires (0 = inactive).
    #[serde(default)]
    pub disabled_hacked_until_frame: u32,
    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    /// Vehicle/structure stays alive but cannot move/attack/produce until frame expires.
    #[serde(default)]
    pub disabled_emp: bool,
    /// Absolute host logic frame when DISABLED_EMP expires (0 = inactive).
    #[serde(default)]
    pub disabled_emp_until_frame: u32,
    /// Host ECM tank / jammer residual: weapons cannot fire while inside jam radius.
    /// C++ DISABLED_SUBDUED cannot-fire residual (Microwave/ECM vehicle disabler).
    /// Fail-closed: continuous aura (not full subdual damage accumulate/heal).
    #[serde(default)]
    pub weapons_jammed: bool,
    /// C++ OBJECT_STATUS_IS_CARBOMB residual (ConvertToCarBombCrateCollide).
    /// Vehicle uses SuicideCarBomb weapon set residual and detonates on attack fire.
    #[serde(default)]
    pub is_carbomb: bool,
    /// C++ OBJECT_STATUS_HIJACKED residual (ConvertToHijackedVehicleCrateCollide).
    #[serde(default)]
    pub hijacked: bool,
}

/// Basic geometry information for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryInfo {
    pub position: Vec3,
    pub rotation: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub radius: f32,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min: Vec3::splat(-1.0),
            bounds_max: Vec3::splat(1.0),
            radius: 1.0,
        }
    }
}

/// Health and damage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(max_health: f32) -> Self {
        Self {
            current: max_health,
            maximum: max_health,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.maximum
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 {
            self.current / self.maximum
        } else {
            0.0
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }
}

/// Movement and pathfinding state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movement {
    pub target_position: Option<Vec3>,
    pub velocity: Vec3,
    pub max_speed: f32,
    pub acceleration: f32,
    pub turn_rate: f32,
    pub path: Vec<Vec3>,
    pub current_path_index: usize,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            target_position: None,
            velocity: Vec3::ZERO,
            max_speed: 10.0,
            acceleration: 5.0,
            turn_rate: std::f32::consts::PI,
            path: Vec::new(),
            current_path_index: 0,
        }
    }
}

/// Economic resources
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Resources {
    pub supplies: u32,
    pub power: i32, // Can be negative
}

/// Experience and veterancy system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VeterancyLevel {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub current: f32,
    pub level: VeterancyLevel,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            current: 0.0,
            level: VeterancyLevel::Rookie,
        }
    }
}

/// Weapon and combat stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: f32,
    pub range: f32,
    /// C++ parity (WeaponTemplate::m_minimumAttackRange): weapons cannot fire
    /// at targets closer than this distance.  0.0 = no minimum range.
    pub min_range: f32,
    pub reload_time: f32,
    pub last_fire_time: f32,
    pub ammo: Option<u32>,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    /// C++ parity (WeaponTemplate::m_weaponSpeed): projectile travel speed.
    /// 0.0 = instant-hit (laser/flame weapons).
    pub projectile_speed: f32,
    /// C++ parity (WeaponTemplate::m_preAttackDelay): delay before firing
    /// after a target is acquired, in seconds.  0.0 = no delay.
    pub pre_attack_delay: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 25.0,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
        }
    }
}
