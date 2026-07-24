//! Host SpectreGunshipDeploymentUpdate residual (CC spawns orbit gunship).
//!
//! C++: `SpectreGunshipDeploymentUpdate::initiateIntentToDoSpecialPower`
//! - Validate SpecialPowerTemplate link
//! - Destroy prior gunship id if still alive
//! - Spawn `GunshipTemplateName` on caster team with producer = CC
//! - Creation edge from `CreateLocation` (retail: **FARTHEST_FROM_TARGET**)
//! - Push spawn further off-map by `distance + GunshipOrbitRadius`
//! - Orient toward target, mark SP triggered, fire gunship SP at target
//! - Select gunship for controlling player
//!
//! Retail peels (`FactionBuilding.ini` AmericaCommandCenter):
//! - SpecialPowerTemplate = SuperweaponSpectreGunship
//! - GunshipTemplateName = AmericaJetSpectreGunship
//! - AttackAreaRadius = **200**
//! - CreateLocation = CREATE_AT_EDGE_FARTHEST_FROM_TARGET
//! - GunshipOrbitRadius (on jet) = **250**
//!
//! Implements SpecialPowerUpdateInterface residual (see
//! `host_special_power_update_module`).
//!
//! Fail-closed: not full SpectreGunshipUpdate continuous fire / decal pair /
//! gattling strafe FX / academy stats.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::{
    find_closest_edge_point_residual, RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z,
    RESIDUAL_MAP_EXTENT_MIN_X, RESIDUAL_MAP_EXTENT_MIN_Z,
};
use crate::game_logic::special_power_strikes::SPECTRE_GUNSHIP_ORBIT_RADIUS;
use crate::game_logic::ObjectId;

/// Retail AttackAreaRadius residual.
pub const SPECTRE_DEPLOY_ATTACK_AREA_RADIUS: f32 = 200.0;
/// Retail gunship template peel.
pub const SPECTRE_GUNSHIP_TEMPLATE: &str = "AmericaJetSpectreGunship";
/// AirF tiered gunship templates.
pub const SPECTRE_GUNSHIP_TEMPLATE_AIRF1: &str = "AirF_AmericaJetSpectreGunship1";
pub const SPECTRE_GUNSHIP_TEMPLATE_AIRF2: &str = "AirF_AmericaJetSpectreGunship2";
pub const SPECTRE_GUNSHIP_TEMPLATE_AIRF3: &str = "AirF_AmericaJetSpectreGunship3";
/// Special power template peel.
pub const SPECTRE_SPECIAL_POWER_TEMPLATE: &str = "SuperweaponSpectreGunship";
/// Preferred flight altitude residual when locomotor height unavailable.
pub const SPECTRE_PREFERRED_ELEVATION: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GunshipCreateLocType {
    EdgeNearSource = 0,
    EdgeFarthestFromSource = 1,
    EdgeNearTarget = 2,
    EdgeFarthestFromTarget = 3,
}

impl Default for GunshipCreateLocType {
    fn default() -> Self {
        Self::EdgeFarthestFromTarget
    }
}

impl GunshipCreateLocType {
    pub fn from_ini(name: &str) -> Self {
        let n = name.to_ascii_uppercase();
        if n.contains("NEAR_SOURCE") && n.contains("FARTHEST") {
            Self::EdgeFarthestFromSource
        } else if n.contains("NEAR_SOURCE") {
            Self::EdgeNearSource
        } else if n.contains("NEAR_TARGET") {
            Self::EdgeNearTarget
        } else {
            Self::EdgeFarthestFromTarget
        }
    }
}

/// Module data residual on command center.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSpectreGunshipDeploymentData {
    pub special_power_template: String,
    pub gunship_template_name: String,
    pub attack_area_radius: f32,
    pub create_loc: GunshipCreateLocType,
    pub gunship_orbit_radius: f32,
    pub preferred_elevation: f32,
    /// Live gunship object id residual (INVALID = none).
    pub gunship_id: Option<ObjectId>,
    pub initial_target: Option<Vec3>,
    pub last_spawn_pos: Option<Vec3>,
}

impl Default for HostSpectreGunshipDeploymentData {
    fn default() -> Self {
        Self {
            special_power_template: SPECTRE_SPECIAL_POWER_TEMPLATE.into(),
            gunship_template_name: SPECTRE_GUNSHIP_TEMPLATE.into(),
            attack_area_radius: SPECTRE_DEPLOY_ATTACK_AREA_RADIUS,
            create_loc: GunshipCreateLocType::EdgeFarthestFromTarget,
            gunship_orbit_radius: SPECTRE_GUNSHIP_ORBIT_RADIUS,
            preferred_elevation: SPECTRE_PREFERRED_ELEVATION,
            gunship_id: None,
            initial_target: None,
            last_spawn_pos: None,
        }
    }
}

impl HostSpectreGunshipDeploymentData {
    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_spectre_deployment_host(template_name) {
            let mut d = Self::default();
            let n = template_name.to_ascii_lowercase();
            if n.contains("airf") || n.contains("airforce") {
                d.gunship_template_name = SPECTRE_GUNSHIP_TEMPLATE_AIRF2.into();
            }
            Some(d)
        } else {
            None
        }
    }

    /// Compute map-edge creation coordinate (before off-map push).
    pub fn edge_creation_coord(
        &self,
        source_pos: Vec3,
        target_pos: Vec3,
        map_min_x: f32,
        map_min_z: f32,
        map_max_x: f32,
        map_max_z: f32,
    ) -> Vec3 {
        match self.create_loc {
            GunshipCreateLocType::EdgeNearSource => find_closest_edge_point_residual(
                source_pos,
                map_min_x,
                map_min_z,
                map_max_x,
                map_max_z,
                self.preferred_elevation,
            ),
            GunshipCreateLocType::EdgeFarthestFromSource => find_farthest_edge_point_residual(
                source_pos,
                map_min_x,
                map_min_z,
                map_max_x,
                map_max_z,
                self.preferred_elevation,
            ),
            GunshipCreateLocType::EdgeNearTarget => find_closest_edge_point_residual(
                target_pos,
                map_min_x,
                map_min_z,
                map_max_x,
                map_max_z,
                self.preferred_elevation,
            ),
            GunshipCreateLocType::EdgeFarthestFromTarget => find_farthest_edge_point_residual(
                target_pos,
                map_min_x,
                map_min_z,
                map_max_x,
                map_max_z,
                self.preferred_elevation,
            ),
        }
    }

    /// C++ off-map push: creation = target - normalize(target-edge) * (dist + orbitR).
    pub fn final_spawn_position(&self, edge: Vec3, target: Vec3) -> Vec3 {
        let mut delta = Vec3::new(target.x - edge.x, 0.0, target.z - edge.z);
        let distance = delta.length();
        if distance < 1.0e-3 {
            return Vec3::new(edge.x, self.preferred_elevation, edge.z);
        }
        delta = delta / distance;
        let push = distance + self.gunship_orbit_radius;
        Vec3::new(
            target.x - delta.x * push,
            self.preferred_elevation,
            target.z - delta.z * push,
        )
    }

    pub fn orientation_toward_target(spawn: Vec3, target: Vec3) -> f32 {
        (target.z - spawn.z).atan2(target.x - spawn.x)
    }

    /// Record an initiate residual plan (spawn coords + template).
    pub fn plan_initiate(
        &mut self,
        source_pos: Vec3,
        target_pos: Vec3,
        map_min_x: f32,
        map_min_z: f32,
        map_max_x: f32,
        map_max_z: f32,
    ) -> SpectreDeploySpawnPlan {
        self.initial_target = Some(target_pos);
        let edge = self.edge_creation_coord(
            source_pos,
            target_pos,
            map_min_x,
            map_min_z,
            map_max_x,
            map_max_z,
        );
        let spawn = self.final_spawn_position(edge, target_pos);
        self.last_spawn_pos = Some(spawn);
        let orient = Self::orientation_toward_target(spawn, target_pos);
        SpectreDeploySpawnPlan {
            gunship_template: self.gunship_template_name.clone(),
            spawn_pos: spawn,
            orientation: orient,
            target_pos,
            attack_area_radius: self.attack_area_radius,
            replace_prior: self.gunship_id,
        }
    }

    pub fn bind_gunship(&mut self, id: ObjectId) {
        self.gunship_id = Some(id);
    }

    pub fn clear_gunship(&mut self) {
        self.gunship_id = None;
    }
}

/// Result of initiateIntent residual (host applies spawn).
#[derive(Debug, Clone)]
pub struct SpectreDeploySpawnPlan {
    pub gunship_template: String,
    pub spawn_pos: Vec3,
    pub orientation: f32,
    pub target_pos: Vec3,
    pub attack_area_radius: f32,
    pub replace_prior: Option<ObjectId>,
}

/// Command centers / SW buildings that host SpectreGunshipDeploymentUpdate.
pub fn is_spectre_deployment_host(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("americacommandcenter") {
        return true;
    }
    if n.contains("commandcenter") || n.contains("command_center") {
        return n.contains("america")
            || n.contains("airf")
            || n.contains("superweapon")
            || n.contains("boss")
            || n.contains("laser")
            || n.contains("swgen")
            || n.starts_with("sw_");
    }
    false
}

/// C++ TerrainLogic::findFarthestEdgePoint residual (XZ rectangle).
pub fn find_farthest_edge_point_residual(
    target: Vec3,
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,
    preferred_height: f32,
) -> Vec3 {
    let candidates = [
        Vec3::new(target.x.clamp(map_min_x, map_max_x), preferred_height, map_min_z),
        Vec3::new(map_max_x, preferred_height, target.z.clamp(map_min_z, map_max_z)),
        Vec3::new(target.x.clamp(map_min_x, map_max_x), preferred_height, map_max_z),
        Vec3::new(map_min_x, preferred_height, target.z.clamp(map_min_z, map_max_z)),
    ];
    let mut best = candidates[0];
    let mut best_d = -1.0f32;
    for c in candidates {
        let d = (c.x - target.x).hypot(c.z - target.z);
        if d > best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

pub fn default_map_extents() -> (f32, f32, f32, f32) {
    (
        RESIDUAL_MAP_EXTENT_MIN_X,
        RESIDUAL_MAP_EXTENT_MIN_Z,
        RESIDUAL_MAP_EXTENT_MAX_X,
        RESIDUAL_MAP_EXTENT_MAX_Z,
    )
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSpectreGunshipDeploymentRegistry {
    pub installed: u32,
    pub initiates: u32,
    pub spawns: u32,
    pub prior_clears: u32,
}

impl HostSpectreGunshipDeploymentRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_initiate(&mut self) {
        self.initiates = self.initiates.saturating_add(1);
    }
    pub fn record_spawn(&mut self) {
        self.spawns = self.spawns.saturating_add(1);
    }
    pub fn record_prior_clear(&mut self) {
        self.prior_clears = self.prior_clears.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.initiates > 0 || self.spawns > 0
    }
}

pub fn honesty_spectre_gunship_deployment_residual_ok() -> bool {
    SPECTRE_DEPLOY_ATTACK_AREA_RADIUS == 200.0
        && (SPECTRE_GUNSHIP_ORBIT_RADIUS - 250.0).abs() < 0.01
        && SPECTRE_GUNSHIP_TEMPLATE == "AmericaJetSpectreGunship"
        && SPECTRE_SPECIAL_POWER_TEMPLATE == "SuperweaponSpectreGunship"
        && is_spectre_deployment_host("AmericaCommandCenter")
        && is_spectre_deployment_host("AirF_AmericaCommandCenter")
        && !is_spectre_deployment_host("AmericaTankCrusader")
        && {
            let mut d = HostSpectreGunshipDeploymentData::default();
            let (minx, minz, maxx, maxz) = default_map_extents();
            let plan = d.plan_initiate(
                Vec3::new(100.0, 0.0, 100.0),
                Vec3::new(250.0, 0.0, 250.0),
                minx,
                minz,
                maxx,
                maxz,
            );
            plan.spawn_pos.y == SPECTRE_PREFERRED_ELEVATION
                && plan.gunship_template == SPECTRE_GUNSHIP_TEMPLATE
                && (plan.attack_area_radius - 200.0).abs() < 0.01
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_spectre_gunship_deployment_residual_ok());
    }

    #[test]
    fn farthest_from_target_pushes_beyond_edge() {
        let d = HostSpectreGunshipDeploymentData::default();
        let target = Vec3::new(250.0, 0.0, 250.0);
        let edge = find_farthest_edge_point_residual(target, 0.0, 0.0, 500.0, 500.0, 120.0);
        let spawn = d.final_spawn_position(edge, target);
        // Spawn should be further from target than the edge point.
        let de = (edge.x - target.x).hypot(edge.z - target.z);
        let ds = (spawn.x - target.x).hypot(spawn.z - target.z);
        assert!(ds > de + 200.0);
    }

    #[test]
    fn create_loc_ini_parse() {
        assert_eq!(
            GunshipCreateLocType::from_ini("CREATE_AT_EDGE_FARTHEST_FROM_TARGET"),
            GunshipCreateLocType::EdgeFarthestFromTarget
        );
        assert_eq!(
            GunshipCreateLocType::from_ini("CREATE_AT_EDGE_NEAR_SOURCE"),
            GunshipCreateLocType::EdgeNearSource
        );
    }
}
