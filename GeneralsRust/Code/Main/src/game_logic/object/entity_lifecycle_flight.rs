//! Later gun-projectile flight residuals (MissileAIUpdate / DumbProjectile).
//!
//! C++ Object::xfer v9 persists these inside per-module snapshots
//! (Object.cpp:4264-4356). Data table of flattened Main fields.

use super::Object;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct LaterProjectileFlight {
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

impl LaterProjectileFlight {
    pub(crate) fn present(object: &Object) -> bool {
        object.comanche_rocket_pod_projectile
            || object.stealth_jet_missile_projectile
            || object.helix_napalm_bomb_projectile
            || object.scud_launcher_missile_projectile
            || object.tomahawk_missile_projectile
            || object.aurora_bomb_projectile
            || object.rocket_buggy_missile_projectile
            || object.neutron_cannon_shell_projectile
            || object.nuke_cannon_shell_projectile
            || object.usa_tank_shell_projectile
            || object.ecm_missile_jammed
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            comanche_rocket_pod_projectile: object.comanche_rocket_pod_projectile,
            comanche_rocket_pod_projectile_expires_frame: object
                .comanche_rocket_pod_projectile_expires_frame,
            stealth_jet_missile_projectile: object.stealth_jet_missile_projectile,
            stealth_jet_missile_aim: object.stealth_jet_missile_aim,
            stealth_jet_missile_intended: object.stealth_jet_missile_intended,
            stealth_jet_missile_travelled: object.stealth_jet_missile_travelled,
            stealth_jet_missile_fuel_expires_frame: object.stealth_jet_missile_fuel_expires_frame,
            stealth_jet_missile_ignition_frame: object.stealth_jet_missile_ignition_frame,
            helix_napalm_bomb_projectile: object.helix_napalm_bomb_projectile,
            scud_launcher_missile_projectile: object.scud_launcher_missile_projectile,
            scud_launcher_missile_toxin: object.scud_launcher_missile_toxin,
            scud_launcher_missile_aim: object.scud_launcher_missile_aim,
            scud_launcher_missile_travelled: object.scud_launcher_missile_travelled,
            scud_launcher_missile_fuel_expires_frame: object
                .scud_launcher_missile_fuel_expires_frame,
            tomahawk_missile_projectile: object.tomahawk_missile_projectile,
            tomahawk_missile_aim: object.tomahawk_missile_aim,
            tomahawk_missile_travelled: object.tomahawk_missile_travelled,
            tomahawk_missile_fuel_expires_frame: object.tomahawk_missile_fuel_expires_frame,
            aurora_bomb_projectile: object.aurora_bomb_projectile,
            aurora_bomb_aim: object.aurora_bomb_aim,
            aurora_bomb_mission_id: object.aurora_bomb_mission_id,
            rocket_buggy_missile_projectile: object.rocket_buggy_missile_projectile,
            rocket_buggy_missile_aim: object.rocket_buggy_missile_aim,
            rocket_buggy_missile_intended: object.rocket_buggy_missile_intended,
            rocket_buggy_missile_travelled: object.rocket_buggy_missile_travelled,
            rocket_buggy_missile_fuel_expires_frame: object.rocket_buggy_missile_fuel_expires_frame,
            neutron_cannon_shell_projectile: object.neutron_cannon_shell_projectile,
            neutron_shell_from: object.neutron_shell_from,
            neutron_shell_aim: object.neutron_shell_aim,
            neutron_shell_launch_frame: object.neutron_shell_launch_frame,
            neutron_shell_flight_frames: object.neutron_shell_flight_frames,
            nuke_cannon_shell_projectile: object.nuke_cannon_shell_projectile,
            nuke_shell_from: object.nuke_shell_from,
            nuke_shell_aim: object.nuke_shell_aim,
            nuke_shell_launch_frame: object.nuke_shell_launch_frame,
            nuke_shell_flight_frames: object.nuke_shell_flight_frames,
            usa_tank_shell_projectile: object.usa_tank_shell_projectile,
            usa_tank_shell_from: object.usa_tank_shell_from,
            usa_tank_shell_aim: object.usa_tank_shell_aim,
            usa_tank_shell_launch_frame: object.usa_tank_shell_launch_frame,
            usa_tank_shell_flight_frames: object.usa_tank_shell_flight_frames,
            usa_tank_shell_weapon_speed: object.usa_tank_shell_weapon_speed,
            usa_tank_shell_intended: object.usa_tank_shell_intended,
            ecm_missile_jammed: object.ecm_missile_jammed,
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.comanche_rocket_pod_projectile = self.comanche_rocket_pod_projectile;
        object.comanche_rocket_pod_projectile_expires_frame =
            self.comanche_rocket_pod_projectile_expires_frame;
        object.stealth_jet_missile_projectile = self.stealth_jet_missile_projectile;
        object.stealth_jet_missile_aim = self.stealth_jet_missile_aim;
        object.stealth_jet_missile_intended = self.stealth_jet_missile_intended;
        object.stealth_jet_missile_travelled = self.stealth_jet_missile_travelled;
        object.stealth_jet_missile_fuel_expires_frame = self.stealth_jet_missile_fuel_expires_frame;
        object.stealth_jet_missile_ignition_frame = self.stealth_jet_missile_ignition_frame;
        object.helix_napalm_bomb_projectile = self.helix_napalm_bomb_projectile;
        object.scud_launcher_missile_projectile = self.scud_launcher_missile_projectile;
        object.scud_launcher_missile_toxin = self.scud_launcher_missile_toxin;
        object.scud_launcher_missile_aim = self.scud_launcher_missile_aim;
        object.scud_launcher_missile_travelled = self.scud_launcher_missile_travelled;
        object.scud_launcher_missile_fuel_expires_frame =
            self.scud_launcher_missile_fuel_expires_frame;
        object.tomahawk_missile_projectile = self.tomahawk_missile_projectile;
        object.tomahawk_missile_aim = self.tomahawk_missile_aim;
        object.tomahawk_missile_travelled = self.tomahawk_missile_travelled;
        object.tomahawk_missile_fuel_expires_frame = self.tomahawk_missile_fuel_expires_frame;
        object.aurora_bomb_projectile = self.aurora_bomb_projectile;
        object.aurora_bomb_aim = self.aurora_bomb_aim;
        object.aurora_bomb_mission_id = self.aurora_bomb_mission_id;
        object.rocket_buggy_missile_projectile = self.rocket_buggy_missile_projectile;
        object.rocket_buggy_missile_aim = self.rocket_buggy_missile_aim;
        object.rocket_buggy_missile_intended = self.rocket_buggy_missile_intended;
        object.rocket_buggy_missile_travelled = self.rocket_buggy_missile_travelled;
        object.rocket_buggy_missile_fuel_expires_frame =
            self.rocket_buggy_missile_fuel_expires_frame;
        object.neutron_cannon_shell_projectile = self.neutron_cannon_shell_projectile;
        object.neutron_shell_from = self.neutron_shell_from;
        object.neutron_shell_aim = self.neutron_shell_aim;
        object.neutron_shell_launch_frame = self.neutron_shell_launch_frame;
        object.neutron_shell_flight_frames = self.neutron_shell_flight_frames;
        object.nuke_cannon_shell_projectile = self.nuke_cannon_shell_projectile;
        object.nuke_shell_from = self.nuke_shell_from;
        object.nuke_shell_aim = self.nuke_shell_aim;
        object.nuke_shell_launch_frame = self.nuke_shell_launch_frame;
        object.nuke_shell_flight_frames = self.nuke_shell_flight_frames;
        object.usa_tank_shell_projectile = self.usa_tank_shell_projectile;
        object.usa_tank_shell_from = self.usa_tank_shell_from;
        object.usa_tank_shell_aim = self.usa_tank_shell_aim;
        object.usa_tank_shell_launch_frame = self.usa_tank_shell_launch_frame;
        object.usa_tank_shell_flight_frames = self.usa_tank_shell_flight_frames;
        object.usa_tank_shell_weapon_speed = self.usa_tank_shell_weapon_speed;
        object.usa_tank_shell_intended = self.usa_tank_shell_intended;
        object.ecm_missile_jammed = self.ecm_missile_jammed;
    }
}
