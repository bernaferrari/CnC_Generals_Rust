//! Later gun-projectile flight residuals (MissileAIUpdate / DumbProjectile).
//!
//! C++ Object::xfer v9 persists these inside per-module snapshots
//! (Object.cpp:4264-4356). Data table of flattened Main fields.

use super::Object;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct LaterProjectileFlight {
    pub helix_napalm_bomb_projectile: bool,
    pub tomahawk_missile_projectile: bool,
    pub tomahawk_missile_aim: Option<[f32; 3]>,
    pub tomahawk_missile_travelled: f32,
    pub tomahawk_missile_fuel_expires_frame: Option<u32>,
    pub aurora_bomb_projectile: bool,
    pub aurora_bomb_aim: Option<[f32; 3]>,
    pub aurora_bomb_mission_id: Option<u32>,
    pub ecm_missile_jammed: bool,
}

impl LaterProjectileFlight {
    pub(crate) fn present(object: &Object) -> bool {
        object.helix_napalm_bomb_projectile
            || object.tomahawk_missile_projectile
            || object.aurora_bomb_projectile
            || object.ecm_missile_jammed
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            helix_napalm_bomb_projectile: object.helix_napalm_bomb_projectile,
            tomahawk_missile_projectile: object.tomahawk_missile_projectile,
            tomahawk_missile_aim: object.tomahawk_missile_aim,
            tomahawk_missile_travelled: object.tomahawk_missile_travelled,
            tomahawk_missile_fuel_expires_frame: object.tomahawk_missile_fuel_expires_frame,
            aurora_bomb_projectile: object.aurora_bomb_projectile,
            aurora_bomb_aim: object.aurora_bomb_aim,
            aurora_bomb_mission_id: object.aurora_bomb_mission_id,
            ecm_missile_jammed: object.ecm_missile_jammed,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.helix_napalm_bomb_projectile = self.helix_napalm_bomb_projectile;
        object.tomahawk_missile_projectile = self.tomahawk_missile_projectile;
        object.tomahawk_missile_aim = self.tomahawk_missile_aim;
        object.tomahawk_missile_travelled = self.tomahawk_missile_travelled;
        object.tomahawk_missile_fuel_expires_frame = self.tomahawk_missile_fuel_expires_frame;
        object.aurora_bomb_projectile = self.aurora_bomb_projectile;
        object.aurora_bomb_aim = self.aurora_bomb_aim;
        object.aurora_bomb_mission_id = self.aurora_bomb_mission_id;
        object.ecm_missile_jammed = self.ecm_missile_jammed;
    }
}

/// Pre-v2 `ProjectileFlightResiduals` payload layout (decode only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct LegacyLaterProjectileFlight {
    pub comanche_rocket_pod_projectile: bool,
    pub comanche_rocket_pod_projectile_expires_frame: Option<u32>,
    pub stealth_jet_missile_projectile: bool,
    pub stealth_jet_missile_aim: Option<[f32; 3]>,
    pub stealth_jet_missile_intended: Option<u32>,
    pub stealth_jet_missile_travelled: f32,
    pub stealth_jet_missile_fuel_expires_frame: Option<u32>,
    pub stealth_jet_missile_ignition_frame: Option<u32>,
    pub helix_napalm_bomb_projectile: bool,
    pub scud_launcher_missile_projectile: bool,
    pub scud_launcher_missile_toxin: bool,
    pub scud_launcher_missile_aim: Option<[f32; 3]>,
    pub scud_launcher_missile_travelled: f32,
    pub scud_launcher_missile_fuel_expires_frame: Option<u32>,
    pub tomahawk_missile_projectile: bool,
    pub tomahawk_missile_aim: Option<[f32; 3]>,
    pub tomahawk_missile_travelled: f32,
    pub tomahawk_missile_fuel_expires_frame: Option<u32>,
    pub aurora_bomb_projectile: bool,
    pub aurora_bomb_aim: Option<[f32; 3]>,
    pub aurora_bomb_mission_id: Option<u32>,
    pub rocket_buggy_missile_projectile: bool,
    pub rocket_buggy_missile_aim: Option<[f32; 3]>,
    pub rocket_buggy_missile_intended: Option<u32>,
    pub rocket_buggy_missile_travelled: f32,
    pub rocket_buggy_missile_fuel_expires_frame: Option<u32>,
    pub neutron_cannon_shell_projectile: bool,
    pub neutron_shell_from: Option<[f32; 3]>,
    pub neutron_shell_aim: Option<[f32; 3]>,
    pub neutron_shell_launch_frame: Option<u32>,
    pub neutron_shell_flight_frames: u32,
    pub nuke_cannon_shell_projectile: bool,
    pub nuke_shell_from: Option<[f32; 3]>,
    pub nuke_shell_aim: Option<[f32; 3]>,
    pub nuke_shell_launch_frame: Option<u32>,
    pub nuke_shell_flight_frames: u32,
    pub usa_tank_shell_projectile: bool,
    pub usa_tank_shell_from: Option<[f32; 3]>,
    pub usa_tank_shell_aim: Option<[f32; 3]>,
    pub usa_tank_shell_launch_frame: Option<u32>,
    pub usa_tank_shell_flight_frames: u32,
    pub usa_tank_shell_weapon_speed: f32,
    pub usa_tank_shell_intended: Option<u32>,
    pub ecm_missile_jammed: bool,
}

impl LegacyLaterProjectileFlight {
    /// Read-and-discard: the removed host projectile fields no longer exist; only
    /// state still owned by live updaters is restored.
    pub(crate) fn apply(self, object: &mut Object) {
        object.helix_napalm_bomb_projectile = self.helix_napalm_bomb_projectile;
        object.tomahawk_missile_projectile = self.tomahawk_missile_projectile;
        object.tomahawk_missile_aim = self.tomahawk_missile_aim;
        object.tomahawk_missile_travelled = self.tomahawk_missile_travelled;
        object.tomahawk_missile_fuel_expires_frame = self.tomahawk_missile_fuel_expires_frame;
        object.aurora_bomb_projectile = self.aurora_bomb_projectile;
        object.aurora_bomb_aim = self.aurora_bomb_aim;
        object.aurora_bomb_mission_id = self.aurora_bomb_mission_id;
        object.ecm_missile_jammed = self.ecm_missile_jammed;
    }
}
