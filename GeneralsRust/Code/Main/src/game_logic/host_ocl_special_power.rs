//! Host OCLSpecialPower residual (object-creation-list driven special powers).
//!
//! C++: `OCLSpecialPower::doSpecialPowerAtLocation` / `findOCL`
//! - Select OCL via science UpgradeOCL pairs (first owned science wins) else default
//! - Create deliverer at CreateLocation relative to source/target
//! - Optional passable-cell adjust (fail-closed residual flag)
//!
//! Retail AmericaCommandCenter peels (`FactionBuilding.ini`):
//! | Power | Default OCL | CreateLocation | UpgradeOCL |
//! |---|---|---|---|
//! | SuperweaponDaisyCutter | SUPERWEAPON_DaisyCutter | EDGE_NEAR_SOURCE | SCIENCE_MOAB → SUPERWEAPON_MOAB |
//! | SpecialPowerSpyDrone | SUPERWEAPON_SpyDrone | ABOVE_LOCATION | — |
//! | SuperweaponParadropAmerica | SUPERWEAPON_Paradrop1 | EDGE_NEAR_SOURCE + adjust | Paradrop2/3 sciences |
//! | SpecialPowerSpySatellite | SUPERWEAPON_SpySatellite | AT_LOCATION | — |
//! | SuperweaponCrateDrop | SUPERWEAPON_CrateDrop | EDGE_NEAR_SOURCE | — |
//! | SuperweaponA10… | SUPERWEAPON_A10…1 | EDGE_NEAR_SOURCE | Strike2/3 sciences |
//! | SuperweaponEmergencyRepair | SUPERWEAPON_RepairVehicles1 | AT_LOCATION | Repair2/3 |
//! | SuperweaponLeafletDrop | SUPERWEAPON_LeafletDrop | EDGE_NEAR_SOURCE | — |
//!
//! Fail-closed: not full ObjectCreationList::create deliverer/payload matrix /
//! partition findPositionAround passable search (flag recorded only).

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::{
    find_closest_edge_point_residual, RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z,
    RESIDUAL_MAP_EXTENT_MIN_X, RESIDUAL_MAP_EXTENT_MIN_Z,
};
use crate::game_logic::host_spectre_gunship_deployment::find_farthest_edge_point_residual;

/// C++ CREATE_ABOVE_LOCATION_HEIGHT residual.
pub const OCL_CREATE_ABOVE_LOCATION_HEIGHT: f32 = 300.0;
/// C++ MAX_ADJUST_RADIUS residual.
pub const OCL_MAX_ADJUST_RADIUS: f32 = 500.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum OclCreateLocType {
    #[default]
    EdgeNearSource = 0,
    EdgeNearTarget = 1,
    AtLocation = 2,
    UseOwnerObject = 3,
    AboveLocation = 4,
    EdgeFarthestFromTarget = 5,
}

impl OclCreateLocType {
    pub fn from_ini(name: &str) -> Self {
        let n = name.to_ascii_uppercase().replace(' ', "_");
        if n.contains("FARTHEST_FROM_TARGET") {
            Self::EdgeFarthestFromTarget
        } else if n.contains("NEAR_TARGET") {
            Self::EdgeNearTarget
        } else if n.contains("NEAR_SOURCE") {
            Self::EdgeNearSource
        } else if n.contains("ABOVE_LOCATION") {
            Self::AboveLocation
        } else if n.contains("USE_OWNER") {
            Self::UseOwnerObject
        } else {
            Self::AtLocation
        }
    }
}

/// Science → OCL upgrade pair residual.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OclScienceUpgrade {
    pub science: String,
    pub ocl: String,
}

/// One OCLSpecialPower module residual peel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OclSpecialPowerPeel {
    pub special_power_template: String,
    pub default_ocl: String,
    pub create_loc: OclCreateLocType,
    pub upgrade_ocl: Vec<OclScienceUpgrade>,
    pub adjust_position_to_passable: bool,
    pub reference_object: Option<String>,
}

impl OclSpecialPowerPeel {
    pub fn simple(power: &str, ocl: &str, loc: OclCreateLocType) -> Self {
        Self {
            special_power_template: power.into(),
            default_ocl: ocl.into(),
            create_loc: loc,
            upgrade_ocl: Vec::new(),
            adjust_position_to_passable: false,
            reference_object: None,
        }
    }
}

/// Retail AmericaCommandCenter OCLSpecialPower peels.
pub fn america_command_center_ocl_peels() -> Vec<OclSpecialPowerPeel> {
    vec![
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponDaisyCutter".into(),
            default_ocl: "SUPERWEAPON_DaisyCutter".into(),
            create_loc: OclCreateLocType::EdgeNearSource,
            upgrade_ocl: vec![OclScienceUpgrade {
                science: "SCIENCE_MOAB".into(),
                ocl: "SUPERWEAPON_MOAB".into(),
            }],
            adjust_position_to_passable: false,
            reference_object: None,
        },
        OclSpecialPowerPeel::simple(
            "SpecialPowerSpyDrone",
            "SUPERWEAPON_SpyDrone",
            OclCreateLocType::AboveLocation,
        ),
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponParadropAmerica".into(),
            default_ocl: "SUPERWEAPON_Paradrop1".into(),
            create_loc: OclCreateLocType::EdgeNearSource,
            upgrade_ocl: vec![
                // C++ iterates in order; first owned science wins — list high tiers first
                // matching INI declaration order (Paradrop3 then Paradrop2).
                OclScienceUpgrade {
                    science: "SCIENCE_Paradrop3".into(),
                    ocl: "SUPERWEAPON_Paradrop3".into(),
                },
                OclScienceUpgrade {
                    science: "SCIENCE_Paradrop2".into(),
                    ocl: "SUPERWEAPON_Paradrop2".into(),
                },
            ],
            adjust_position_to_passable: true,
            reference_object: None,
        },
        OclSpecialPowerPeel::simple(
            "SpecialPowerSpySatellite",
            "SUPERWEAPON_SpySatellite",
            OclCreateLocType::AtLocation,
        ),
        OclSpecialPowerPeel::simple(
            "SuperweaponCrateDrop",
            "SUPERWEAPON_CrateDrop",
            OclCreateLocType::EdgeNearSource,
        ),
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponA10ThunderboltMissileStrike".into(),
            default_ocl: "SUPERWEAPON_A10ThunderboltMissileStrike1".into(),
            create_loc: OclCreateLocType::EdgeNearSource,
            upgrade_ocl: vec![
                OclScienceUpgrade {
                    science: "SCIENCE_A10ThunderboltMissileStrike3".into(),
                    ocl: "SUPERWEAPON_A10ThunderboltMissileStrike3".into(),
                },
                OclScienceUpgrade {
                    science: "SCIENCE_A10ThunderboltMissileStrike2".into(),
                    ocl: "SUPERWEAPON_A10ThunderboltMissileStrike2".into(),
                },
            ],
            adjust_position_to_passable: false,
            reference_object: None,
        },
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponEmergencyRepair".into(),
            default_ocl: "SUPERWEAPON_RepairVehicles1".into(),
            create_loc: OclCreateLocType::AtLocation,
            upgrade_ocl: vec![
                OclScienceUpgrade {
                    science: "SCIENCE_EmergencyRepair3".into(),
                    ocl: "SUPERWEAPON_RepairVehicles3".into(),
                },
                OclScienceUpgrade {
                    science: "SCIENCE_EmergencyRepair2".into(),
                    ocl: "SUPERWEAPON_RepairVehicles2".into(),
                },
            ],
            adjust_position_to_passable: false,
            reference_object: None,
        },
        OclSpecialPowerPeel::simple(
            "SuperweaponLeafletDrop",
            "SUPERWEAPON_LeafletDrop",
            OclCreateLocType::EdgeNearSource,
        ),
    ]
}

/// Resolve OCL name: first owned upgrade science, else default.
pub fn find_ocl_name(peel: &OclSpecialPowerPeel, player_has_science: impl Fn(&str) -> bool) -> &str {
    for up in &peel.upgrade_ocl {
        if player_has_science(&up.science) {
            return up.ocl.as_str();
        }
    }
    peel.default_ocl.as_str()
}

pub fn peel_for_special_power(power_template: &str) -> Option<&'static OclSpecialPowerPeel> {
    use std::sync::LazyLock;
    static PEELS: LazyLock<Vec<OclSpecialPowerPeel>> =
        LazyLock::new(america_command_center_ocl_peels);
    let key = power_template.to_ascii_lowercase();
    PEELS.iter().find(|p| {
        p.special_power_template.eq_ignore_ascii_case(power_template)
            || p.special_power_template.to_ascii_lowercase().contains(&key)
            || key.contains(&p.special_power_template.to_ascii_lowercase())
    })
}

/// Creation + target coordinates for ObjectCreationList::create residual.
#[derive(Debug, Clone)]
pub struct OclSpecialPowerSpawnPlan {
    pub ocl_name: String,
    pub creation_coord: Vec3,
    pub target_coord: Vec3,
    pub create_loc: OclCreateLocType,
    pub adjust_passable_requested: bool,
    pub special_power_template: String,
}

/// Compute creation coordinate (host Y-up; C++ Z-up height → Y).
pub fn compute_creation_coord(
    create_loc: OclCreateLocType,
    source_pos: Vec3,
    target_pos: Vec3,
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,
) -> Vec3 {
    match create_loc {
        OclCreateLocType::EdgeNearSource => find_closest_edge_point_residual(
            source_pos,
            map_min_x,
            map_min_z,
            map_max_x,
            map_max_z,
            source_pos.y,
        ),
        OclCreateLocType::EdgeNearTarget => find_closest_edge_point_residual(
            target_pos,
            map_min_x,
            map_min_z,
            map_max_x,
            map_max_z,
            target_pos.y,
        ),
        OclCreateLocType::EdgeFarthestFromTarget => {
            let mut c = find_farthest_edge_point_residual(
                target_pos,
                map_min_x,
                map_min_z,
                map_max_x,
                map_max_z,
                target_pos.y,
            );
            c.y += OCL_CREATE_ABOVE_LOCATION_HEIGHT;
            c
        }
        OclCreateLocType::AtLocation | OclCreateLocType::UseOwnerObject => target_pos,
        OclCreateLocType::AboveLocation => {
            let mut c = target_pos;
            c.y += OCL_CREATE_ABOVE_LOCATION_HEIGHT;
            c
        }
    }
}

pub fn plan_ocl_special_power_at_location(
    power_template: &str,
    source_pos: Vec3,
    target_pos: Vec3,
    player_has_science: impl Fn(&str) -> bool,
    map_min_x: f32,
    map_min_z: f32,
    map_max_x: f32,
    map_max_z: f32,
) -> Option<OclSpecialPowerSpawnPlan> {
    let peel = peel_for_special_power(power_template)?;
    let ocl = find_ocl_name(peel, player_has_science).to_string();
    let creation = compute_creation_coord(
        peel.create_loc,
        source_pos,
        target_pos,
        map_min_x,
        map_min_z,
        map_max_x,
        map_max_z,
    );
    Some(OclSpecialPowerSpawnPlan {
        ocl_name: ocl,
        creation_coord: creation,
        target_coord: target_pos,
        create_loc: peel.create_loc,
        adjust_passable_requested: peel.adjust_position_to_passable,
        special_power_template: peel.special_power_template.clone(),
    })
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
pub struct HostOclSpecialPowerRegistry {
    pub plans: u32,
    pub by_edge_near_source: u32,
    pub by_above_location: u32,
    pub by_at_location: u32,
    pub science_upgrades: u32,
    pub last_ocl: String,
}

impl HostOclSpecialPowerRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_plan(&mut self, plan: &OclSpecialPowerSpawnPlan, used_upgrade: bool) {
        self.plans = self.plans.saturating_add(1);
        self.last_ocl = plan.ocl_name.clone();
        if used_upgrade {
            self.science_upgrades = self.science_upgrades.saturating_add(1);
        }
        match plan.create_loc {
            OclCreateLocType::EdgeNearSource => {
                self.by_edge_near_source = self.by_edge_near_source.saturating_add(1);
            }
            OclCreateLocType::AboveLocation => {
                self.by_above_location = self.by_above_location.saturating_add(1);
            }
            OclCreateLocType::AtLocation | OclCreateLocType::UseOwnerObject => {
                self.by_at_location = self.by_at_location.saturating_add(1);
            }
            _ => {}
        }
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.plans > 0 || honesty_ocl_special_power_residual_ok()
    }
}

pub fn honesty_ocl_special_power_residual_ok() -> bool {
    OCL_CREATE_ABOVE_LOCATION_HEIGHT == 300.0
        && OCL_MAX_ADJUST_RADIUS == 500.0
        && america_command_center_ocl_peels().len() >= 8
        && {
            let peels = america_command_center_ocl_peels();
            let daisy = peels
                .iter()
                .find(|p| p.special_power_template.contains("DaisyCutter"))
                .expect("daisy");
            find_ocl_name(daisy, |_| false) == "SUPERWEAPON_DaisyCutter"
                && find_ocl_name(daisy, |s| s == "SCIENCE_MOAB") == "SUPERWEAPON_MOAB"
        }
        && OclCreateLocType::from_ini("CREATE_AT_EDGE_NEAR_SOURCE")
            == OclCreateLocType::EdgeNearSource
        && OclCreateLocType::from_ini("CREATE_ABOVE_LOCATION") == OclCreateLocType::AboveLocation
        && {
            let (minx, minz, maxx, maxz) = default_map_extents();
            let plan = plan_ocl_special_power_at_location(
                "SpecialPowerSpyDrone",
                Vec3::new(100.0, 0.0, 100.0),
                Vec3::new(200.0, 0.0, 200.0),
                |_| false,
                minx,
                minz,
                maxx,
                maxz,
            )
            .expect("drone");
            (plan.creation_coord.y - 300.0).abs() < 0.1
                && plan.ocl_name == "SUPERWEAPON_SpyDrone"
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_science_upgrade() {
        assert!(honesty_ocl_special_power_residual_ok());
        let peels = america_command_center_ocl_peels();
        let a10 = peels
            .iter()
            .find(|p| p.special_power_template.contains("A10"))
            .unwrap();
        assert_eq!(
            find_ocl_name(a10, |s| s.contains("Strike3")),
            "SUPERWEAPON_A10ThunderboltMissileStrike3"
        );
        assert_eq!(
            find_ocl_name(a10, |_| false),
            "SUPERWEAPON_A10ThunderboltMissileStrike1"
        );
    }

    #[test]
    fn edge_near_source_plan() {
        let (minx, minz, maxx, maxz) = default_map_extents();
        let plan = plan_ocl_special_power_at_location(
            "SuperweaponLeafletDrop",
            Vec3::new(50.0, 0.0, 50.0),
            Vec3::new(400.0, 0.0, 400.0),
            |_| false,
            minx,
            minz,
            maxx,
            maxz,
        )
        .unwrap();
        assert_eq!(plan.create_loc, OclCreateLocType::EdgeNearSource);
        assert_eq!(plan.ocl_name, "SUPERWEAPON_LeafletDrop");
        // Creation should be on map edge near source (50,50), not at target.
        let d_src = (plan.creation_coord.x - 50.0).hypot(plan.creation_coord.z - 50.0);
        let d_tgt = (plan.creation_coord.x - 400.0).hypot(plan.creation_coord.z - 400.0);
        assert!(d_src < d_tgt);
    }
}
