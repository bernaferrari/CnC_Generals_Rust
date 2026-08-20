// Kind-of engine indices and geometry conversion
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Utility functions

const SELECTABLE_KIND_INDICES: &[u32] = &[1];
const UNIT_KIND_INDICES: &[u32] = &[8, 9, 10, 11, 12, 13, 16, 17, 68, 77, 85];
const STRUCTURE_KIND_INDICES: &[u32] = &[
    7, 19, 20, 21, 34, 57, 58, 59, 60, 89, 90, 91, 92, 93, 98, 99, 104, 105, 106, 107,
];
const VEHICLE_KIND_INDICES: &[u32] = &[9, 11, 12, 13, 18];
const HARVESTER_KIND_INDICES: &[u32] = &[13];
const AIRCRAFT_KIND_INDICES: &[u32] = &[10, 106, 107];
const DRONE_KIND_INDICES: &[u32] = &[68];
const CRATE_KIND_INDICES: &[u32] = &[44];
const RESOURCE_NODE_KIND_INDICES: &[u32] = &[81];
const SUPPLY_SOURCE_ON_PREVIEW_KIND_INDICES: &[u32] = &[72];
const SUPPLY_SOURCE_KIND_INDICES: &[u32] = &[81];
const DISGUISER_KIND_INDICES: &[u32] = &[83];
const TECH_BUILDING_KIND_INDICES: &[u32] = &[65];
const BRIDGE_KIND_INDICES: &[u32] = &[19, 20, 21];
const WALL_KIND_INDICES: &[u32] = &[56];
const SALVAGER_KIND_INDICES: &[u32] = &[16];
const WEAPON_SALVAGER_KIND_INDICES: &[u32] = &[17];
const ARMOR_SALVAGER_KIND_INDICES: &[u32] = &[95];
const ALWAYS_SELECTABLE_INDICES: &[u32] = &[53];
const CAN_ATTACK_KIND_INDICES: &[u32] = &[3];
const PROJECTILE_KIND_INDICES: &[u32] = &[22];
const CLIFF_JUMPER_INDICES: &[u32] = &[88];

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
        KindOf::Transport => &[18],
        KindOf::SupplySource => SUPPLY_SOURCE_KIND_INDICES,
        KindOf::Disguiser => DISGUISER_KIND_INDICES,
        KindOf::TechBuilding => TECH_BUILDING_KIND_INDICES,
        KindOf::Bridge => BRIDGE_KIND_INDICES,
        KindOf::Barrier => WALL_KIND_INDICES,
        KindOf::Shrubbery => &[6],
        KindOf::CanSeeThrough => &[69],
        KindOf::CanCrossBridges => BRIDGE_KIND_INDICES,
        KindOf::BridgeTower => BRIDGE_KIND_INDICES,
        KindOf::WaveGuide => &[25],
        KindOf::Boat => &[75],
        KindOf::Salvager => SALVAGER_KIND_INDICES,
        KindOf::WeaponSalvager => WEAPON_SALVAGER_KIND_INDICES,
        KindOf::ArmorSalvager => ARMOR_SALVAGER_KIND_INDICES,
        KindOf::FSBarracks => &[104],
        KindOf::FSWarfactory => &[105],
        KindOf::FSAirfield => &[106],
        KindOf::FSInternetCenter => &[99],
        KindOf::FSPower | KindOf::PowerPlant => &[57],
        KindOf::FSBaseDefense => &[59],
        KindOf::FSSupplyDropzone => &[89],
        KindOf::FSSupplyCenter | KindOf::Refinery => &[92],
        KindOf::FSSuperweapon => &[90],
        KindOf::FSStrategyCenter => &[93],
        KindOf::FSFake => &[98],
        KindOf::Defense => &[59, 111],
        KindOf::Factory => &[58],
        KindOf::Mine => &[50],
        KindOf::Prison | KindOf::CollectsPrisonBounty | KindOf::PowTruck | KindOf::CanSurrender => {
            &[]
        }
        KindOf::Civilian
        | KindOf::Destructible
        | KindOf::Amphibious
        | KindOf::AmphibiousTransport
        | KindOf::CanCapture => &[],
        KindOf::Hero => &[85],
        KindOf::CountsForVictory => &[33],
        KindOf::CleanupHazard => &[51],
        KindOf::Immobile => &[2],
        KindOf::Inert => &[84],
        KindOf::BoobyTrap => &[97],
        KindOf::CanBeRepulsed => &[41],
        KindOf::EmpHardened => &[112],
        KindOf::SpawnsAreTheWeapons => &[79],
        KindOf::Obstacle => &[0],
        KindOf::CanAttack => &[3],
        KindOf::StickToTerrainSlope => &[4],
        KindOf::CanCastReflections => &[5],
        KindOf::Preload => &[23],
        KindOf::NoCollide => &[27],
        KindOf::StealthGarrison => &[30],
        KindOf::DrawableOnly => &[32],
        KindOf::Score => &[35],
        KindOf::ScoreCreate => &[36],
        KindOf::ScoreDestroy => &[37],
        KindOf::NoHealIcon => &[38],
        KindOf::Parachutable => &[40],
        KindOf::AlwaysVisible => &[48],
        KindOf::Unattackable => &[49],
        KindOf::AircraftPathAround => &[61],
        KindOf::LowOverlappable => &[62],
        KindOf::ForceAttackable => &[63],
        KindOf::AutoRallypoint => &[64],
        KindOf::ClickThrough => &[71],
        KindOf::ShowPortraitWhenControlled => &[78],
        KindOf::CannotBuildNearSupplies => &[80],
        KindOf::RevealToAll => &[82],
        KindOf::IgnoresSelectAll => &[86],
        KindOf::DontAutoCrushInfantry => &[87],
        KindOf::FsBlackMarket => &[91],
        KindOf::FsAdvancedTech => &[103],
        KindOf::RevealsEnemyPaths => &[96],
        KindOf::NoSelect => &[108],
        KindOf::CannotRetaliate => &[110],
        KindOf::Demotrap => &[113],
        KindOf::ConservativeBuilding => &[114],
        KindOf::BlastCrater => &[100],
        KindOf::Prop => &[101],
        KindOf::OptimizedTree => &[102],
        KindOf::WaveEffect => &[26],
        KindOf::ClearedByBuild => &[46],
        KindOf::HugeVehicle => &[11],
        KindOf::LineBuild => &[15],
        KindOf::SmallMissile => &[47],
        KindOf::BallisticMissile => &[70],
        KindOf::AttackNeedsLineOfSight => &[54],
        KindOf::WalkOnTopOfWall | KindOf::DefensiveWall => &[56],
        KindOf::MoneyHacker => &[94],
        KindOf::TechBaseDefense => &[111],
        KindOf::LandmarkBridge => &[20],
        KindOf::HealPad => &[29],
        KindOf::PortableStructure => &[52],
        KindOf::CanRappel => &[39],
        KindOf::IgnoreDockingBones => &[115],
        KindOf::RepairPad => &[28],
        KindOf::RejectUnmanned => &[109],
        KindOf::IgnoredInGui => &[43],
        KindOf::MobNexus => &[42],
        KindOf::Capturable => &[45],
        KindOf::ImmuneToCapture => &[76],
        KindOf::CashGenerator => &[31],
        KindOf::RebuildHole => &[34],
        KindOf::FSTechnology => &[60],
        KindOf::NoGarrison => &[24],
        KindOf::GarrisonableUntilDestroyed => &[74],
        KindOf::Powered => &[66],
        KindOf::ProducedAtHelipad => &[67],
        KindOf::Parachute => &[73],
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

