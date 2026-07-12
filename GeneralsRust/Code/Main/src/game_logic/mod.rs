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
    HostSpecialPowerStrike, HostSpecialPowerStrikeRegistry, HostStrikePhase, HostSuperweaponKind,
};
pub use host_upgrades::{
    HostUpgradeKind, HostUpgradePhase, HostUpgradeRegistry, HostUpgradeResearch,
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
    GLA_REBEL_PRIMARY_WEAPON, HUMVEE_PRIMARY_WEAPON, HUMVEE_SECONDARY_WEAPON, RANGER_PRIMARY_WEAPON,
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
    pub stealthed: bool,
    /// C++ DISABLED_UNDERPOWERED: set when player's power supply < demand.
    pub disabled_underpowered: bool,
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
