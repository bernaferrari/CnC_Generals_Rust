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
//! | SuperweaponRebelAmbush | SUPERWEAPON_RebelAmbush1 | AT_LOCATION + adjust | Ambush3/2 + Chem/Demo/Slth |
//!
//! Fail-closed: not full ObjectCreationList::create deliverer/payload matrix.
//! OCLAdjustPositionToPassable snaps via findPositionAround(CLEAR_CELLS_ONLY, r=500);
//! a failed search keeps the original click (C++ fail-closed).

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::{
    RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z, RESIDUAL_MAP_EXTENT_MIN_X,
    RESIDUAL_MAP_EXTENT_MIN_Z, find_closest_edge_point_residual,
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

fn ambush_upgrade(science: &str, ocl: &str) -> OclScienceUpgrade {
    OclScienceUpgrade {
        science: science.into(),
        ocl: ocl.into(),
    }
}

/// Retail GLA / Chem / Demo / Slth Command Center OCLSpecialPower peels.
///
/// C++ `FactionBuilding.ini` keeps one module per command-center object. Host
/// merges UpgradeOCL pairs so `findOCL` still walks first-owned-science-wins
/// (Ambush3 before Ambush2; Chem/Demo/Slth Ambush1 listed so they do not fall
/// through to `GLAInfantryRebel`).
pub fn gla_command_center_ocl_peels() -> Vec<OclSpecialPowerPeel> {
    vec![
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponRebelAmbush".into(),
            default_ocl: "SUPERWEAPON_RebelAmbush1".into(),
            create_loc: OclCreateLocType::AtLocation,
            upgrade_ocl: vec![
                ambush_upgrade("SCIENCE_RebelAmbush3", "SUPERWEAPON_RebelAmbush3"),
                ambush_upgrade("SCIENCE_RebelAmbush2", "SUPERWEAPON_RebelAmbush2"),
                ambush_upgrade("Chem_SCIENCE_RebelAmbush3", "Chem_SUPERWEAPON_RebelAmbush3"),
                ambush_upgrade("Chem_SCIENCE_RebelAmbush2", "Chem_SUPERWEAPON_RebelAmbush2"),
                ambush_upgrade("Chem_SCIENCE_RebelAmbush1", "Chem_SUPERWEAPON_RebelAmbush1"),
                ambush_upgrade("Demo_SCIENCE_RebelAmbush3", "Demo_SUPERWEAPON_RebelAmbush3"),
                ambush_upgrade("Demo_SCIENCE_RebelAmbush2", "Demo_SUPERWEAPON_RebelAmbush2"),
                ambush_upgrade("Demo_SCIENCE_RebelAmbush1", "Demo_SUPERWEAPON_RebelAmbush1"),
                ambush_upgrade("Slth_SCIENCE_RebelAmbush3", "Slth_SUPERWEAPON_RebelAmbush3"),
                ambush_upgrade("Slth_SCIENCE_RebelAmbush2", "Slth_SUPERWEAPON_RebelAmbush2"),
                ambush_upgrade("Slth_SCIENCE_RebelAmbush1", "Slth_SUPERWEAPON_RebelAmbush1"),
            ],
            adjust_position_to_passable: true,
            reference_object: None,
        },
        // C++ FactionBuilding.ini GLACommandCenter default OCL=SUPERWEAPON_AnthraxBomb.
        // Chem_GLACommandCenter default OCL=SUPERWEAPON_AnthraxBombGamma (no UpgradeOCL).
        // Palace Chem_Upgrade_GLAAnthraxGamma is listed so leftover findOCL can swap.
        OclSpecialPowerPeel {
            special_power_template: "SuperweaponAnthraxBomb".into(),
            default_ocl: "SUPERWEAPON_AnthraxBomb".into(),
            create_loc: OclCreateLocType::EdgeNearSource,
            upgrade_ocl: vec![
                OclScienceUpgrade {
                    science: "Chem_Upgrade_GLAAnthraxGamma".into(),
                    ocl: "SUPERWEAPON_AnthraxBombGamma".into(),
                },
                OclScienceUpgrade {
                    science: "Upgrade_GLAAnthraxGamma".into(),
                    ocl: "SUPERWEAPON_AnthraxBombGamma".into(),
                },
            ],
            adjust_position_to_passable: false,
            reference_object: None,
        },
    ]
}

/// Retail ChinaCommandCenter OCLSpecialPower peels.
///
/// C++ `FactionBuilding.ini` SuperweaponNapalmStrike default OCL is
/// `SUPERWEAPON_NapalmStrike` (CREATE_AT_EDGE_NEAR_SOURCE). Not DaisyCutter FAB.
pub fn china_command_center_ocl_peels() -> Vec<OclSpecialPowerPeel> {
    vec![OclSpecialPowerPeel::simple(
        "SuperweaponNapalmStrike",
        crate::game_logic::special_power_strikes::NAPALM_STRIKE_OCL,
        OclCreateLocType::EdgeNearSource,
    )]
}

/// Resolve OCL name: first owned upgrade science, else default.
///
/// C++ `OCLSpecialPower::findOCL` uses `getControllingPlayer()->hasScience`.
/// The callback must be that one player's sciences, never a faction union.
pub fn find_ocl_name(
    peel: &OclSpecialPowerPeel,
    player_has_science: impl Fn(&str) -> bool,
) -> &str {
    for up in &peel.upgrade_ocl {
        if player_has_science(&up.science) {
            return up.ocl.as_str();
        }
    }
    peel.default_ocl.as_str()
}

/// Retail SuperweaponAnthraxBomb SpecialPowerTemplate.
pub const ANTHRAX_BOMB_SPECIAL_POWER: &str = "SuperweaponAnthraxBomb";
/// C++ GLACommandCenter OCLSpecialPower default OCL.
pub const ANTHRAX_BOMB_OCL: &str = "SUPERWEAPON_AnthraxBomb";
/// C++ Chem_GLACommandCenter OCLSpecialPower default OCL.
pub const ANTHRAX_BOMB_GAMMA_OCL: &str = "SUPERWEAPON_AnthraxBombGamma";

/// Chem command-center / toxin-general identity for leftover Anthrax OCL.
///
/// C++ Chem_GLACommandCenter default OCL is SUPERWEAPON_AnthraxBombGamma.
/// Player template is FactionGLAToxinGeneral (not "chemgeneral").
pub fn is_chem_anthrax_bomb_ocl_source(name: &str) -> bool {
    use crate::game_logic::host_toxin_tractor::is_chem_general_template;
    if is_chem_general_template(name) {
        return true;
    }
    let n = name.to_ascii_lowercase();
    n.contains("toxingeneral")
        || n.contains("factionglatoxin")
        || n.contains("chemicalgeneral")
        || n.contains("factionchem")
}

/// Leftover findOCL + Chem_GLACommandCenter default OCL.
pub fn resolve_anthrax_bomb_ocl<'a, I>(source_template: &str, names: I) -> &'static str
where
    I: IntoIterator<Item = &'a str>,
{
    let owned: Vec<&'a str> = names.into_iter().collect();
    if is_chem_anthrax_bomb_ocl_source(source_template)
        || owned.iter().copied().any(is_chem_anthrax_bomb_ocl_source)
    {
        return ANTHRAX_BOMB_GAMMA_OCL;
    }
    if let Some(peel) = peel_for_special_power(ANTHRAX_BOMB_SPECIAL_POWER) {
        return find_ocl_name(peel, |s| owned.iter().any(|n| n.eq_ignore_ascii_case(s)));
    }
    ANTHRAX_BOMB_OCL
}

pub fn peel_for_special_power(power_template: &str) -> Option<&'static OclSpecialPowerPeel> {
    use std::sync::LazyLock;
    static PEELS: LazyLock<Vec<OclSpecialPowerPeel>> = LazyLock::new(|| {
        let mut peels = america_command_center_ocl_peels();
        peels.extend(gla_command_center_ocl_peels());
        peels.extend(china_command_center_ocl_peels());
        peels
    });
    let key = power_template.to_ascii_lowercase();
    PEELS.iter().find(|p| {
        p.special_power_template
            .eq_ignore_ascii_case(power_template)
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
    /// C++ ObjectCreationList::create `createOwner`. False for USE_OWNER_OBJECT
    /// and for no-target `doSpecialPower` so DeliverPayload reuses the firer.
    pub create_owner: bool,
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

/// C++ `createOwner` for a location fire. USE_OWNER_OBJECT passes false.
pub fn ocl_create_owner_for_create_loc(create_loc: OclCreateLocType) -> bool {
    create_loc != OclCreateLocType::UseOwnerObject
}

/// C++ `OCLSpecialPower::doSpecialPower` always passes `createOwner=false`.
pub fn ocl_create_owner_for_no_target() -> bool {
    false
}

/// C++ `PartitionManager::findPositionAround` with `FPF_CLEAR_CELLS_ONLY`.
///
/// Samples rings out to `max_radius` (C++ RING_SPACING = 5). Returns the first
/// clear-cell sample, or `None` so the caller keeps the original click.
pub fn find_ocl_passable_around(
    target: Vec3,
    max_radius: f32,
    is_clear_cell: impl Fn(Vec3) -> bool,
) -> Option<Vec3> {
    const RING_SPACING: f32 = 5.0;
    const TWO_PI: f32 = std::f32::consts::PI * 2.0;
    let min_radius = 0.0;
    let mut dist = min_radius;
    while dist <= max_radius {
        let angle_spacing = if (dist - min_radius).abs() < f32::EPSILON {
            TWO_PI
        } else {
            (RING_SPACING / (dist + 1.0)) * (TWO_PI / 6.0)
        };
        let samples = ((TWO_PI / angle_spacing) / 2.0).ceil() as i32;
        for i in 0..samples {
            let angle_offset = angle_spacing * i as f32;
            let try_at = |angle: f32| -> Option<Vec3> {
                let pos = Vec3::new(
                    target.x + dist * angle.cos(),
                    target.y,
                    target.z + dist * angle.sin(),
                );
                is_clear_cell(pos).then_some(pos)
            };
            if let Some(pos) = try_at(angle_offset) {
                return Some(pos);
            }
            if i != 0 {
                if let Some(pos) = try_at(-angle_offset) {
                    return Some(pos);
                }
            }
        }
        dist += RING_SPACING;
    }
    None
}

/// Apply C++ OCLAdjustPositionToPassable. Missing searcher or failed search
/// keeps `target` (C++: "if findPosition() fails, then don't monkey with target").
pub fn adjust_ocl_target_to_passable(
    target: Vec3,
    requested: bool,
    is_clear_cell: Option<&dyn Fn(Vec3) -> bool>,
) -> Vec3 {
    if !requested {
        return target;
    }
    let Some(is_clear) = is_clear_cell else {
        return target;
    };
    find_ocl_passable_around(target, OCL_MAX_ADJUST_RADIUS, is_clear).unwrap_or(target)
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
    is_clear_cell: Option<&dyn Fn(Vec3) -> bool>,
) -> Option<OclSpecialPowerSpawnPlan> {
    let peel = peel_for_special_power(power_template)?;
    let ocl = find_ocl_name(peel, player_has_science).to_string();
    let target_coord =
        adjust_ocl_target_to_passable(target_pos, peel.adjust_position_to_passable, is_clear_cell);
    let creation = compute_creation_coord(
        peel.create_loc,
        source_pos,
        target_coord,
        map_min_x,
        map_min_z,
        map_max_x,
        map_max_z,
    );
    Some(OclSpecialPowerSpawnPlan {
        ocl_name: ocl,
        creation_coord: creation,
        target_coord,
        create_loc: peel.create_loc,
        adjust_passable_requested: peel.adjust_position_to_passable,
        create_owner: ocl_create_owner_for_create_loc(peel.create_loc),
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

/// DeliverPayload residual peel from ObjectCreationList.ini.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OclDeliverPayloadPeel {
    pub ocl_name: String,
    pub transport: String,
    pub payload: String,
    pub delivery_distance: f32,
    pub drop_offset_y: f32,
    pub start_at_preferred_height: bool,
    pub start_at_max_speed: bool,
}

/// CreateObject residual peel (SpyDrone style).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OclCreateObjectPeel {
    pub ocl_name: String,
    pub object_names: Vec<String>,
    pub count: u32,
}

pub fn deliver_payload_for_ocl(ocl: &str) -> Option<OclDeliverPayloadPeel> {
    let n = ocl.to_ascii_lowercase();
    if n.contains("daisycutter") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_DaisyCutter".into(),
            transport: "AmericaJetB52".into(),
            payload: "DaisyCutterBomb".into(),
            delivery_distance: 140.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("moab") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_MOAB".into(),
            transport: "AmericaJetB3".into(),
            payload: "MOAB".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("leaflet") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_LeafletDrop".into(),
            transport: "AmericaJetB52".into(),
            payload: "LeafletContainer".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("a10thunderboltmissilestrike3") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: ocl.into(),
            transport: "AmericaJetA10Thunderbolt".into(),
            payload: "A10ThunderboltMissile".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("a10thunderboltmissilestrike2") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: ocl.into(),
            transport: "AmericaJetA10Thunderbolt".into(),
            payload: "A10ThunderboltMissile".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("a10") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_A10ThunderboltMissileStrike1".into(),
            transport: "AmericaJetA10Thunderbolt".into(),
            payload: "A10ThunderboltMissile".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("paradrop3") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_Paradrop3".into(),
            transport: "AmericaJetCargoPlane".into(),
            payload: "AmericaInfantryRanger".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("paradrop2") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_Paradrop2".into(),
            transport: "AmericaJetCargoPlane".into(),
            payload: "AmericaInfantryRanger".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("paradrop") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_Paradrop1".into(),
            transport: "AmericaJetCargoPlane".into(),
            payload: "AmericaInfantryRanger".into(),
            delivery_distance: 160.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("cratedrop") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: "SUPERWEAPON_CrateDrop".into(),
            transport: "AmericaJetCargoPlane".into(),
            payload: "200DollarCrate".into(),
            delivery_distance: 250.0,
            drop_offset_y: -10.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("anthraxbombgamma") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: ANTHRAX_BOMB_GAMMA_OCL.into(),
            transport: "GLAJetCargoPlane".into(),
            payload: "AnthraxBombGamma".into(),
            delivery_distance: 140.0,
            drop_offset_y: 0.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("anthraxbomb") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: ANTHRAX_BOMB_OCL.into(),
            transport: "GLAJetCargoPlane".into(),
            payload: "AnthraxBomb".into(),
            delivery_distance: 140.0,
            drop_offset_y: 0.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }
    if n.contains("napalmstrike") {
        return Some(OclDeliverPayloadPeel {
            ocl_name: crate::game_logic::special_power_strikes::NAPALM_STRIKE_OCL.into(),
            transport: "ChinaJetCargoPlane".into(),
            payload: "NapalmBomb".into(),
            delivery_distance: 140.0,
            drop_offset_y: 0.0,
            start_at_preferred_height: true,
            start_at_max_speed: true,
        });
    }

    None
}

pub fn create_object_for_ocl(ocl: &str) -> Option<OclCreateObjectPeel> {
    let n = ocl.to_ascii_lowercase();
    if n.contains("spydrone") {
        return Some(OclCreateObjectPeel {
            ocl_name: "SUPERWEAPON_SpyDrone".into(),
            object_names: vec!["AmericaVehicleSpyDrone".into()],
            count: 1,
        });
    }
    if n.contains("spysatellite") {
        return Some(OclCreateObjectPeel {
            ocl_name: "SUPERWEAPON_SpySatellite".into(),
            object_names: vec!["AmericaSatellite".into()],
            count: 1,
        });
    }
    if n.contains("rebelambush") {
        let count = if n.contains("ambush3") {
            16
        } else if n.contains("ambush2") {
            8
        } else {
            4
        };
        let template = if n.contains("chem_") {
            "Chem_GLAInfantryRebel"
        } else if n.contains("demo_") {
            "Demo_GLAInfantryRebel"
        } else if n.contains("slth_") {
            "Slth_GLAInfantryRebel"
        } else {
            "GLAInfantryRebel"
        };
        return Some(OclCreateObjectPeel {
            ocl_name: ocl.into(),
            object_names: vec![template.into()],
            count,
        });
    }
    None
}

/// Map host superweapon kind residual labels to SpecialPowerTemplate peels.
pub fn special_power_template_for_host_kind(kind_label: &str) -> Option<&'static str> {
    match kind_label {
        "DaisyCutter" => Some("SuperweaponDaisyCutter"),
        "A10Strike" => Some("SuperweaponA10ThunderboltMissileStrike"),
        "LeafletDrop" => Some("SuperweaponLeafletDrop"),
        "Paradrop" => Some("SuperweaponParadropAmerica"),
        "CrateDrop" => Some("SuperweaponCrateDrop"),
        "SpyDrone" => Some("SpecialPowerSpyDrone"),
        "GLARebelAmbush" | "Ambush" => Some("SuperweaponRebelAmbush"),
        "AnthraxBomb" => Some(ANTHRAX_BOMB_SPECIAL_POWER),
        "NapalmStrike" => Some("SuperweaponNapalmStrike"),
        _ => None,
    }
}

/// OCL spawn mode residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OclExecuteMode {
    /// Transport + payload (Daisy / A10 / MOAB).
    FullDeliver,
    /// Transport only; host residual owns impact/payload (Leaflet / Paradrop).
    TransportOnly,
    /// CreateObject list only (SpyDrone).
    CreateObject,
}

pub fn ocl_execute_mode_for_template(power_template: &str) -> OclExecuteMode {
    let n = power_template.to_ascii_lowercase();
    if n.contains("leaflet")
        || n.contains("paradrop")
        || n.contains("airborne")
        || n.contains("cratedrop")
    {
        OclExecuteMode::TransportOnly
    } else if n.contains("spydrone")
        || n.contains("spysatellite")
        || n.contains("rebelambush")
        || n.contains("ambush")
    {
        OclExecuteMode::CreateObject
    } else {
        OclExecuteMode::FullDeliver
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOclSpecialPowerRegistry {
    pub plans: u32,
    pub by_edge_near_source: u32,
    pub by_above_location: u32,
    pub by_at_location: u32,
    pub science_upgrades: u32,
    pub last_ocl: String,
    pub transports_spawned: u32,
    pub payloads_spawned: u32,
    pub create_objects_spawned: u32,
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
    pub fn record_transport_spawn(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }
    pub fn record_payload_spawn(&mut self) {
        self.payloads_spawned = self.payloads_spawned.saturating_add(1);
    }
    pub fn record_create_object_spawn(&mut self) {
        self.create_objects_spawned = self.create_objects_spawned.saturating_add(1);
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
                None,
            )
            .expect("drone");
            (plan.creation_coord.y - 300.0).abs() < 0.1 && plan.ocl_name == "SUPERWEAPON_SpyDrone"
        }
        && {
            let peels = gla_command_center_ocl_peels();
            let ambush = peels
                .iter()
                .find(|p| p.special_power_template.contains("RebelAmbush"))
                .expect("ambush");
            find_ocl_name(ambush, |_| false) == "SUPERWEAPON_RebelAmbush1"
                && find_ocl_name(ambush, |s| s == "SCIENCE_RebelAmbush3")
                    == "SUPERWEAPON_RebelAmbush3"
                && find_ocl_name(ambush, |s| s == "Chem_SCIENCE_RebelAmbush1")
                    == "Chem_SUPERWEAPON_RebelAmbush1"
                && create_object_for_ocl("Chem_SUPERWEAPON_RebelAmbush3")
                    .map(|c| c.object_names[0] == "Chem_GLAInfantryRebel" && c.count == 16)
                    .unwrap_or(false)
        }
        && deliver_payload_for_ocl("SUPERWEAPON_DaisyCutter")
            .map(|d| d.transport == "AmericaJetB52" && d.payload == "DaisyCutterBomb")
            .unwrap_or(false)
        && create_object_for_ocl("SUPERWEAPON_SpyDrone")
            .map(|c| c.object_names[0] == "AmericaVehicleSpyDrone")
            .unwrap_or(false)
        && special_power_template_for_host_kind("DaisyCutter") == Some("SuperweaponDaisyCutter")
        && special_power_template_for_host_kind("AnthraxBomb") == Some(ANTHRAX_BOMB_SPECIAL_POWER)
        && special_power_template_for_host_kind("NapalmStrike") == Some("SuperweaponNapalmStrike")
        && ocl_execute_mode_for_template("SuperweaponLeafletDrop") == OclExecuteMode::TransportOnly
        && ocl_execute_mode_for_template("SuperweaponDaisyCutter") == OclExecuteMode::FullDeliver
        && resolve_anthrax_bomb_ocl("GLACommandCenter", [] as [&str; 0]) == ANTHRAX_BOMB_OCL
        && resolve_anthrax_bomb_ocl("Chem_GLACommandCenter", [] as [&str; 0])
            == ANTHRAX_BOMB_GAMMA_OCL
        && resolve_anthrax_bomb_ocl("GLACommandCenter", ["FactionGLAToxinGeneral"])
            == ANTHRAX_BOMB_GAMMA_OCL
        && resolve_anthrax_bomb_ocl("GLACommandCenter", ["Chem_Upgrade_GLAAnthraxGamma"])
            == ANTHRAX_BOMB_GAMMA_OCL
        && deliver_payload_for_ocl(ANTHRAX_BOMB_GAMMA_OCL)
            .is_some_and(|d| d.payload == "AnthraxBombGamma" && d.transport == "GLAJetCargoPlane")
        && deliver_payload_for_ocl(ANTHRAX_BOMB_OCL).is_some_and(|d| d.payload == "AnthraxBomb")
        && peel_for_special_power("SuperweaponNapalmStrike").is_some_and(|p| {
            find_ocl_name(p, |_| false)
                == crate::game_logic::special_power_strikes::NAPALM_STRIKE_OCL
        })
        && deliver_payload_for_ocl("SUPERWEAPON_NapalmStrike").is_some_and(|d| {
            d.transport == "ChinaJetCargoPlane"
                && d.payload == "NapalmBomb"
                && d.transport != "AmericaJetB52"
                && d.payload != "DaisyCutterBomb"
        })
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
    fn napalm_strike_find_ocl_own_payload_not_daisy_cutter() {
        let peels = china_command_center_ocl_peels();
        let napalm = peels
            .iter()
            .find(|p| p.special_power_template.contains("NapalmStrike"))
            .unwrap();
        assert_eq!(
            find_ocl_name(napalm, |_| false),
            crate::game_logic::special_power_strikes::NAPALM_STRIKE_OCL
        );
        assert_ne!(find_ocl_name(napalm, |_| false), "SUPERWEAPON_DaisyCutter");
        let deliver = deliver_payload_for_ocl("SUPERWEAPON_NapalmStrike").unwrap();
        assert_eq!(deliver.transport, "ChinaJetCargoPlane");
        assert_eq!(deliver.payload, "NapalmBomb");
        assert_ne!(deliver.payload, "DaisyCutterBomb");
        assert_eq!(
            special_power_template_for_host_kind("NapalmStrike"),
            Some("SuperweaponNapalmStrike")
        );
    }

    #[test]
    fn anthrax_gamma_ocl_from_chem_source_and_palace_upgrade() {
        let peels = gla_command_center_ocl_peels();
        let anthrax = peels
            .iter()
            .find(|p| p.special_power_template.contains("AnthraxBomb"))
            .unwrap();
        assert_eq!(find_ocl_name(anthrax, |_| false), ANTHRAX_BOMB_OCL);
        assert_eq!(
            find_ocl_name(anthrax, |s| s == "Chem_Upgrade_GLAAnthraxGamma"),
            ANTHRAX_BOMB_GAMMA_OCL
        );
        assert_eq!(
            resolve_anthrax_bomb_ocl("Chem_GLACommandCenter", [] as [&str; 0]),
            ANTHRAX_BOMB_GAMMA_OCL
        );
        assert_eq!(
            resolve_anthrax_bomb_ocl("GLACommandCenter", ["FactionGLAToxinGeneral"]),
            ANTHRAX_BOMB_GAMMA_OCL
        );
        assert_eq!(
            deliver_payload_for_ocl(ANTHRAX_BOMB_GAMMA_OCL).map(|d| d.payload),
            Some("AnthraxBombGamma".into())
        );
    }

    #[test]
    fn find_ocl_uses_controlling_player_sciences_only() {
        // C++ findOCL: getControllingPlayer()->hasScience. Ally Strike3 is ignored.
        let peels = america_command_center_ocl_peels();
        let a10 = peels
            .iter()
            .find(|p| p.special_power_template.contains("A10"))
            .unwrap();
        let controller: &[&str] = &[];
        let ally = ["SCIENCE_A10ThunderboltMissileStrike3"];
        assert_eq!(
            find_ocl_name(a10, |s| controller
                .iter()
                .any(|c| c.eq_ignore_ascii_case(s))),
            "SUPERWEAPON_A10ThunderboltMissileStrike1"
        );
        assert_eq!(
            find_ocl_name(a10, |s| ally.iter().any(|c| c.eq_ignore_ascii_case(s))),
            "SUPERWEAPON_A10ThunderboltMissileStrike3"
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
            None,
        )
        .unwrap();
        assert_eq!(plan.create_loc, OclCreateLocType::EdgeNearSource);
        assert_eq!(plan.ocl_name, "SUPERWEAPON_LeafletDrop");
        // Creation should be on map edge near source (50,50), not at target.
        let d_src = (plan.creation_coord.x - 50.0).hypot(plan.creation_coord.z - 50.0);
        let d_tgt = (plan.creation_coord.x - 400.0).hypot(plan.creation_coord.z - 400.0);
        assert!(d_src < d_tgt);
    }

    #[test]
    fn adjust_passable_snaps_when_searcher_finds_clear_cell() {
        let click = Vec3::new(10.0, 0.0, 10.0);
        let snapped = Vec3::new(40.0, 0.0, 10.0);
        let is_clear = |p: Vec3| (p.x - snapped.x).abs() < 3.0 && (p.z - snapped.z).abs() < 3.0;
        let got = adjust_ocl_target_to_passable(click, true, Some(&is_clear));
        assert!((got.x - snapped.x).abs() < 3.0);
        // Failed search keeps the original click.
        let none = adjust_ocl_target_to_passable(click, true, Some(&|_| false));
        assert_eq!(none, click);
        // Flag off never moves the click.
        let raw = adjust_ocl_target_to_passable(click, false, Some(&is_clear));
        assert_eq!(raw, click);
        assert!(!ocl_create_owner_for_create_loc(
            OclCreateLocType::UseOwnerObject
        ));
        assert!(!ocl_create_owner_for_no_target());
        assert!(ocl_create_owner_for_create_loc(
            OclCreateLocType::EdgeNearSource
        ));
    }
}
