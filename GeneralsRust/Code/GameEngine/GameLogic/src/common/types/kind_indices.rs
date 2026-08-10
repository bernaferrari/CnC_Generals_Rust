// Kind-of engine indices and geometry conversion
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Utility functions

const SELECTABLE_KIND_INDICES: &[u32] = &[1];
const UNIT_KIND_INDICES: &[u32] = &[8, 9, 10, 11, 12, 13, 19, 20, 72, 81, 89];
const STRUCTURE_KIND_INDICES: &[u32] = &[
    7, 22, 23, 24, 37, 61, 62, 63, 64, 93, 94, 95, 96, 97, 102, 103, 108, 109, 110, 111,
];
const VEHICLE_KIND_INDICES: &[u32] = &[9, 11, 12, 13, 21];
const HARVESTER_KIND_INDICES: &[u32] = &[13];
const AIRCRAFT_KIND_INDICES: &[u32] = &[10, 110, 111];
const DRONE_KIND_INDICES: &[u32] = &[72];
const CRATE_KIND_INDICES: &[u32] = &[48];
const RESOURCE_NODE_KIND_INDICES: &[u32] = &[85];
const SUPPLY_SOURCE_ON_PREVIEW_KIND_INDICES: &[u32] = &[76];
const SUPPLY_SOURCE_KIND_INDICES: &[u32] = &[85];
const DISGUISER_KIND_INDICES: &[u32] = &[87];
const TECH_BUILDING_KIND_INDICES: &[u32] = &[69];
const BRIDGE_KIND_INDICES: &[u32] = &[22, 23, 24];
const WALL_KIND_INDICES: &[u32] = &[60];
const SALVAGER_KIND_INDICES: &[u32] = &[19];
const WEAPON_SALVAGER_KIND_INDICES: &[u32] = &[20];
const ARMOR_SALVAGER_KIND_INDICES: &[u32] = &[99];
const ALWAYS_SELECTABLE_INDICES: &[u32] = &[57];
const CAN_ATTACK_KIND_INDICES: &[u32] = &[3];
const PROJECTILE_KIND_INDICES: &[u32] = &[25];
const CLIFF_JUMPER_INDICES: &[u32] = &[92];
const PRISON_KIND_INDICES: &[u32] = &[15];
const COLLECTS_PRISON_BOUNTY_INDICES: &[u32] = &[16];
const POW_TRUCK_KIND_INDICES: &[u32] = &[17];
const CAN_SURRENDER_KIND_INDICES: &[u32] = &[44];

fn engine_kind_indices(kind: KindOf) -> &'static [u32] {
    match kind {
        KindOf::Selectable | KindOf::AlwaysSelectable => SELECTABLE_KIND_INDICES,
        KindOf::Unit => UNIT_KIND_INDICES,
        KindOf::Building | KindOf::Structure | KindOf::KeyStructure | KindOf::CommandCenter => {
            STRUCTURE_KIND_INDICES
        }
        KindOf::Vehicle | KindOf::Dozer | KindOf::Hulk => VEHICLE_KIND_INDICES,
        KindOf::Harvester => HARVESTER_KIND_INDICES,
        KindOf::Infantry | KindOf::Saboteur | KindOf::Hacker => &[8],
        KindOf::Aircraft | KindOf::AircraftCarrier => AIRCRAFT_KIND_INDICES,
        KindOf::Drone => DRONE_KIND_INDICES,
        KindOf::CliffJumper => CLIFF_JUMPER_INDICES,
        KindOf::Weapon => CAN_ATTACK_KIND_INDICES,
        KindOf::Projectile => PROJECTILE_KIND_INDICES,
        KindOf::Crate => CRATE_KIND_INDICES,
        KindOf::ResourceNode => RESOURCE_NODE_KIND_INDICES,
        KindOf::SupplySourceOnPreview => SUPPLY_SOURCE_ON_PREVIEW_KIND_INDICES,
        KindOf::SupplySource => SUPPLY_SOURCE_KIND_INDICES,
        KindOf::Disguiser => DISGUISER_KIND_INDICES,
        KindOf::TechBuilding => TECH_BUILDING_KIND_INDICES,
        KindOf::Bridge => BRIDGE_KIND_INDICES,
        KindOf::Barrier => WALL_KIND_INDICES,
        KindOf::Shrubbery => &[6],
        KindOf::CanSeeThrough => &[73],
        KindOf::CanCrossBridges => BRIDGE_KIND_INDICES,
        KindOf::BridgeTower => BRIDGE_KIND_INDICES,
        KindOf::WaveGuide => &[],
        KindOf::Boat => &[79],
        KindOf::Salvager => SALVAGER_KIND_INDICES,
        KindOf::WeaponSalvager => WEAPON_SALVAGER_KIND_INDICES,
        KindOf::ArmorSalvager => ARMOR_SALVAGER_KIND_INDICES,
        KindOf::FSBarracks => &[108],
        KindOf::FSWarfactory => &[109],
        KindOf::FSAirfield => &[110],
        KindOf::FSInternetCenter => &[103],
        KindOf::FSPower | KindOf::PowerPlant => &[61],
        KindOf::FSBaseDefense => &[63],
        KindOf::FSSupplyDropzone => &[93],
        KindOf::FSSupplyCenter | KindOf::Refinery => &[96],
        KindOf::FSSuperweapon => &[94],
        KindOf::FSStrategyCenter => &[97],
        KindOf::FSFake => &[102],
        KindOf::Defense => &[63, 115],
        KindOf::Factory => &[62],
        KindOf::Mine => &[],
        KindOf::Prison => PRISON_KIND_INDICES,
        KindOf::CollectsPrisonBounty => COLLECTS_PRISON_BOUNTY_INDICES,
        KindOf::PowTruck => POW_TRUCK_KIND_INDICES,
        KindOf::CanSurrender => CAN_SURRENDER_KIND_INDICES,
        KindOf::Civilian
        | KindOf::Destructible
        | KindOf::Amphibious
        | KindOf::AmphibiousTransport
        | KindOf::CanCapture
        | KindOf::Hero
        | KindOf::CountsForVictory
        | KindOf::CleanupHazard
        | KindOf::Immobile
        | KindOf::BoobyTrap
        | KindOf::CanBeRepulsed
        | KindOf::EmpHardened
        | KindOf::SpawnsAreTheWeapons => &[],
        KindOf::Obstacle => &[0],
        KindOf::CanAttack => &[3],
        KindOf::StickToTerrainSlope
        | KindOf::CanCastReflections
        | KindOf::Preload
        | KindOf::NoCollide
        | KindOf::StealthGarrison
        | KindOf::DrawableOnly
        | KindOf::Score
        | KindOf::ScoreCreate
        | KindOf::ScoreDestroy
        | KindOf::NoHealIcon
        | KindOf::Parachutable
        | KindOf::AlwaysVisible
        | KindOf::Unattackable
        | KindOf::AircraftPathAround
        | KindOf::LowOverlappable
        | KindOf::ForceAttackable
        | KindOf::AutoRallypoint
        | KindOf::ClickThrough
        | KindOf::ShowPortraitWhenControlled
        | KindOf::CannotBuildNearSupplies
        | KindOf::RevealToAll
        | KindOf::IgnoresSelectAll
        | KindOf::DontAutoCrushInfantry
        | KindOf::FsBlackMarket
        | KindOf::FsAdvancedTech
        | KindOf::RevealsEnemyPaths
        | KindOf::NoSelect
        | KindOf::CannotRetaliate
        | KindOf::Demotrap
        | KindOf::ConservativeBuilding
        | KindOf::BlastCrater
        | KindOf::Prop
        | KindOf::OptimizedTree
        | KindOf::WaveEffect
        | KindOf::ClearedByBuild => &[],
        KindOf::HugeVehicle => &[11],
        KindOf::LineBuild => &[60],
        KindOf::SmallMissile | KindOf::BallisticMissile => &[25],
        KindOf::AttackNeedsLineOfSight => &[3],
        KindOf::WalkOnTopOfWall | KindOf::DefensiveWall => &[60],
        KindOf::MoneyHacker => &[87],
        KindOf::TechBaseDefense => &[69],
        KindOf::LandmarkBridge => &[22, 23, 24],
        _ => &[],
    }
}

pub fn kind_of_indices(kind: KindOf) -> &'static [u32] {
    engine_kind_indices(kind)
}

fn engine_geometry_to_logic(info: &EngineGeometryInfo) -> GeometryInfo {
    let half_width = info.width * 0.5;
    let half_depth = info.depth * 0.5;
    let height = info.height.max(0.0);

    // Approximate bounds centered at origin
    let min = Coord3D::new(-half_width, -half_depth, 0.0);
    let max = Coord3D::new(half_width, half_depth, height);

    GeometryInfo {
        position: Coord3D::new(0.0, 0.0, if info.is_small { 0.0 } else { height * 0.5 }),
        angle: 0.0,
        bounds: AABox { min, max },
        height_above_terrain: if matches!(info.geometry_type, EngineGeometryType::Sphere) {
            0.0
        } else {
            height
        },
        geometry_type: info.geometry_type,
        is_small: info.is_small,
    }
}

/// Test disabled mask function (matching C++ TEST_DISABLEDMASK)
pub fn test_disabled_mask(mask: DisabledMaskType, disabled_type: DisabledType) -> bool {
    mask.test(disabled_type)
}

