//! Object classifications and their packed simulation membership.

mod set;

use serde::{Deserialize, Serialize};
pub use set::KindOfSet;

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
    /// C++ KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT — fire gated by attack view LOS.
    AttackNeedsLineOfSight,
    /// C++ KINDOF_IMMOBILE — structures/defenses; skip terrain LOS detour residual.
    Immobile,
    /// C++ KINDOF_CAN_BE_REPULSED — civilians flee OBJECT_STATUS_REPULSOR.
    CanBeRepulsed,
    /// C++ KINDOF_CANNOT_RETALIATE — excluded from friend retaliation.
    CannotRetaliate,
    /// C++ KINDOF_DRONE — drones never retaliate.
    Drone,
    /// C++ KINDOF_IGNORED_IN_GUI — mouseover remaps to slaver/producer.
    IgnoredInGui,
    /// C++ KINDOF_SALVAGER — GLA salvage crate pickers.
    Salvager,
    /// C++ KINDOF_WEAPON_SALVAGER — can gain weapon crate upgrades.
    WeaponSalvager,
    /// C++ KINDOF_ARMOR_SALVAGER — can gain armor crate upgrades.
    ArmorSalvager,
    /// C++ KINDOF_AIRCRAFT_PATH_AROUND — tall buildings aircraft path around.
    AircraftPathAround,
    /// C++ KINDOF_WAVEGUIDE — flood wave objects enabled by DamDie.
    WaveGuide,
    /// C++ `KINDOF_DOZER` (KindOfType ordinal 12).  Kept distinct from
    /// `Worker`: a builder is not necessarily a resource collector.
    Dozer,
    /// C++ `KINDOF_HARVESTER` (KindOfType ordinal 13).  Retail Chinooks,
    /// Supply Trucks, and GLA Workers carry this capability and may gather
    /// supplies; it is not the same as a `Harvestable` supply source.
    Harvester,
    /// C++ `KINDOF_UNATTACKABLE`: an object may exist for lifetime, vision,
    /// or presentation while never being a legal WeaponSet target (bridges,
    /// props, parachutes, and special objects use it heavily in retail INIs).
    /// The compact presentation bit bank is full; its distinct targetability
    /// meaning is carried through the dedicated presentation Boolean
    /// instead of changing any established bit positions.
    Unattackable,
    /// C++ `KINDOF_MINE`: a mine is targetable by either AntiMine or
    /// AntiGround weapons while retaining its own targeting category.
    Mine,
    /// C++ `KINDOF_DEMOTRAP`, which shares the mine target anti-mask.
    DemoTrap,
    /// C++ `KINDOF_SMALL_MISSILE`: the most specific WeaponSet victim mask.
    SmallMissile,
    /// C++ `KINDOF_BALLISTIC_MISSILE`: distinct from normal projectiles for
    /// PointDefenseLaser and WeaponSet targeting.
    BallisticMissile,
    /// C++ `KINDOF_PARACHUTE`, used when an airborne parachute is a legal
    /// AntiParachute target.
    Parachute,
    /// C++ `KINDOF_DISGUISER`.  This is separate from the temporary
    /// DISGUISED object status: WeaponSet grants the force-attack stealth
    /// exception only to objects whose authored template carries this bit.
    Disguiser,
    /// C++ `KINDOF_REPAIR_PAD`.  `ActionManager::canGetRepairedAt` uses this
    /// authored capability for ground vehicles; it is not inferred from a
    /// producer/building basename or a `RepairDockUpdate` approximation.
    RepairPad,
    /// C++ `KINDOF_HEAL_PAD`.  `ActionManager::canGetHealedAt` uses this
    /// authored capability for infantry; a medical-looking template without
    /// this source tag must not become a service destination.
    HealPad,
    /// C++ `KINDOF_MONEY_HACKER`.  `InternetHackContain` admits this exact
    /// source category; a template name containing "hacker" must never
    /// fabricate either the Hacker command or Internet Center access.
    ///
    /// This is gameplay-only metadata.  It intentionally has no presentation
    /// bit assignment because that packed 32-bit order is already full.
    MoneyHacker,
    /// C++ `KINDOF_SUPPLY_SOURCE`.  Retail SupplyDock/SupplyPile objects
    /// advertise this explicitly; a template spelling containing "supply"
    /// must not create a gather target or construction-exclusion source.
    ///
    /// This stays outside the compact presentation KindOf bank.  Parsed
    /// supply sources also retain the existing Resource/Harvestable bridge
    /// bits used by the host gather UI, while this bit preserves the exact
    /// C++ distinction for supply-source-only authority.
    SupplySource,
    /// C++ `KINDOF_CANNOT_BUILD_NEAR_SUPPLIES`.  BuildAssistant applies its
    /// expanded-footprint check only when the *requested building* has this
    /// authored capability; SupplyCenter-like spelling is not sufficient.
    ///
    /// Gameplay-only and append-only for the same compact-presentation-bank
    /// reason as [`Self::SupplySource`].
    CannotBuildNearSupplies,
    /// C++ `KINDOF_CRATE`. Gameplay-only and append-only: the compact
    /// presentation KindOf bank is already full, so crate identity is also
    /// frozen as dedicated presentation booleans for physical click routing.
    Crate,
    /// C++ `KINDOF_IGNORES_SELECT_ALL` — excluded from SELECT_ALL / matching.
    IgnoresSelectAll,
    /// C++ `KINDOF_ALWAYS_SELECTABLE` (KindOf.h:86). Never unselectable,
    /// even if effectively dead — UI feedback objects / rubble clicks.
    AlwaysSelectable,
    /// C++ `KINDOF_MP_COUNT_FOR_VICTORY` (KindOf.h:63). Radar/EVA/victory
    /// count this bit, not an FS-kind union.
    MpCountForVictory,
    /// C++ `KINDOF_SCORE` (KindOf.h:65). Multiplayer score + short-game.
    Score,
    /// C++ `KINDOF_SCORE_CREATE` (KindOf.h:66).
    ScoreCreate,
    /// C++ `KINDOF_SCORE_DESTROY` (KindOf.h:67).
    ScoreDestroy,
    /// C++ `KINDOF_CAN_SEE_THROUGH` — transparent obstacles skipped in attack LOS.
    CanSeeThrough,
    /// C++ `KINDOF_NO_GARRISON`. Heroes / special infantry cannot enter GarrisonContain.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    NoGarrison,
    /// C++ `KINDOF_GARRISONABLE_UNTIL_DESTROYED`. Firebase/bunker stays occupied
    /// through BODY_REALLYDAMAGED until death.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    GarrisonableUntilDestroyed,
    /// C++ `KINDOF_BOAT`. Hijack / ConvertToCarBomb reject boats.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Boat,
    /// C++ `KINDOF_TRANSPORT`. Occupied transports cannot be hijacked.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Transport,
    /// C++ `KINDOF_IMMUNE_TO_CAPTURE` (Patch 1.03 battle bus).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    ImmuneToCapture,
    /// C++ `KINDOF_DEFENSIVE_WALL`. FenceWidth objects with this bit stay solid walls.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    DefensiveWall,
    /// C++ `KINDOF_WALK_ON_TOP_OF_WALL`. Infantry path on these structure decks.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    WalkOnTopOfWall,
    /// C++ `KINDOF_BRIDGE`. Map-span GenericBridge objects are targetable.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Bridge,
    /// C++ `KINDOF_BRIDGE_TOWER`. Water waves skip towers (hit the span).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    BridgeTower,
    /// C++ `KINDOF_STEALTH_GARRISON`. Ninja / stealth-garrison infantry hide
    /// civilian buildings from non-allies and can be kicked by enemy Enter.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    StealthGarrison,
    /// C++ `KINDOF_TECH_BUILDING`. Captured oil/tech reverts to Neutral on killPlayer.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    TechBuilding,
    /// C++ `KINDOF_LANDMARK_BRIDGE`. Map-placed non-resizable spans (TsingMa).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    LandmarkBridge,
    /// C++ `KINDOF_AUTO_RALLYPOINT`. Factories/producers that accept
    /// ACTIONTYPE_SET_RALLY_POINT. Gameplay-only: the compact presentation
    /// KindOf bank is full.
    AutoRallypoint,
    /// C++ `KINDOF_MOB_NEXUS`. Angry Mob coordinator. ImmuneToGPS strips the
    /// default OverrideableByLikeKind StealthUpdate (ThingTemplate.cpp:391).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    MobNexus,
    /// C++ `KINDOF_NO_COLLIDE` (KindOf.h:56). Partition never collides
    /// these objects. Gameplay-only: the compact presentation KindOf bank
    /// is full.
    NoCollide,
    /// C++ `KINDOF_FORCEATTACKABLE` (KindOf.h:63). Always attackable via
    /// force-attack / hover pick even when not Selectable (civ fences,
    /// cargo planes). Gameplay-only: the compact presentation KindOf bank
    /// is full, so pick uses a dedicated presentation boolean.
    ForceAttackable,
    /// C++ `KINDOF_SHRUBBERY` (KindOf.h:32). Trees/bushes auto-cleared
    /// by construction. Gameplay-only: the compact presentation KindOf bank
    /// is full.
    Shrubbery,
    /// C++ `KINDOF_CLEARED_BY_BUILD` (KindOf.h:72). Debris/rubble under a
    /// footprint. Gameplay-only: the compact presentation KindOf bank is full.
    ClearedByBuild,
    /// C++ `KINDOF_INERT` (KindOf.h:110). Never removable for construction.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Inert,
    /// C++ `KINDOF_BLAST_CRATER` (KindOf.h). Permanent pathfind footprints
    /// that also skip the airborne height gate. Gameplay-only: the compact
    /// presentation KindOf bank is full.
    BlastCrater,
    /// C++ `KINDOF_HUGE_VEHICLE` (KindOf.h:35). Overlord / Helix class.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    HugeVehicle,
    /// C++ `KINDOF_CLIFF_JUMPER`. Combat bikes still get an individual move
    /// after a vehicle column (`AIGroup.cpp:1640-1649`).
    CliffJumper,
    /// C++ `KINDOF_CLEANUP_HAZARD` (KindOf.h, bit 51). Ambulance
    /// CleanupHazardUpdate partition targets; retail hazard fields
    /// (NukeRadiationFieldWeapon / PoisonField*) carry it.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    CleanupHazard,
    /// C++ KINDOF_LINEBUILD (bit 15), instant-completed wall segments.
    /// Append-only to preserve every existing serde/bincode variant index.
    LineBuild,
    /// C++ KINDOF_HULK; append-only save ordinal.
    Hulk,
    /// C++ KINDOF_REJECT_UNMANNED (bit109); append-only Rust save ordinal87.
    RejectUnmanned,
    /// C++ KINDOF_SPAWNS_ARE_THE_WEAPONS (bit79); append-only gameplay
    /// capability and save ordinal88.
    SpawnsAreTheWeapons,
    /// C++ KINDOF_PORTABLE_STRUCTURE (bit52); append-only host capability.
    PortableStructure,
    /// C++ KINDOF_BOOBY_TRAP (bit97); append-only host capability.
    BoobyTrap,
    /// C++ `KINDOF_PRODUCED_AT_HELIPAD` — helicopter airframe (Comanche etc.).
    /// Append-only host KindOf; save ID 91 (do not reshape earlier ordinals).
    ProducedAtHelipad,
    /// C++ KINDOF_EMP_HARDENED (bit 112). Airborne aircraft ignore EMP.
    EmpHardened,
    /// C++ KINDOF_REVEALS_ENEMY_PATHS (bit 96).
    RevealsEnemyPaths,
}

impl KindOf {
    /// All host `KindOf` variants in declaration order.
    ///
    /// The save/load xfer ordinal bijection test
    /// (`save_load::xfer::every_kind_of_xfer_ordinal_round_trips`) walks this
    /// census: when adding a variant, add it here AND to both
    /// `write_kind_of_variant` / `read_kind_of_variant` arms.
    pub const ALL_KIND_OF: &'static [KindOf] = &[
        KindOf::Structure,
        KindOf::Infantry,
        KindOf::Vehicle,
        KindOf::Aircraft,
        KindOf::Projectile,
        KindOf::Resource,
        KindOf::Selectable,
        KindOf::Attackable,
        KindOf::CommandCenter,
        KindOf::Worker,
        KindOf::Hero,
        KindOf::SupplyCenter,
        KindOf::PowerPlant,
        KindOf::FSBarracks,
        KindOf::FSWarFactory,
        KindOf::FSAirfield,
        KindOf::FSInternetCenter,
        KindOf::FSPower,
        KindOf::FSBaseDefense,
        KindOf::FSSupplyDropzone,
        KindOf::FSSupplyCenter,
        KindOf::FSSuperweapon,
        KindOf::FSStrategyCenter,
        KindOf::FSFake,
        KindOf::FSTechnology,
        KindOf::FSBlackMarket,
        KindOf::FSAdvancedTech,
        KindOf::Harvestable,
        KindOf::Powered,
        KindOf::AttackNeedsLineOfSight,
        KindOf::Immobile,
        KindOf::CanBeRepulsed,
        KindOf::CannotRetaliate,
        KindOf::Drone,
        KindOf::IgnoredInGui,
        KindOf::Salvager,
        KindOf::WeaponSalvager,
        KindOf::ArmorSalvager,
        KindOf::AircraftPathAround,
        KindOf::WaveGuide,
        KindOf::Dozer,
        KindOf::Harvester,
        KindOf::Unattackable,
        KindOf::Mine,
        KindOf::DemoTrap,
        KindOf::SmallMissile,
        KindOf::BallisticMissile,
        KindOf::Parachute,
        KindOf::Disguiser,
        KindOf::RepairPad,
        KindOf::HealPad,
        KindOf::MoneyHacker,
        KindOf::SupplySource,
        KindOf::CannotBuildNearSupplies,
        KindOf::Crate,
        KindOf::IgnoresSelectAll,
        KindOf::AlwaysSelectable,
        KindOf::MpCountForVictory,
        KindOf::Score,
        KindOf::ScoreCreate,
        KindOf::ScoreDestroy,
        KindOf::CanSeeThrough,
        KindOf::NoGarrison,
        KindOf::GarrisonableUntilDestroyed,
        KindOf::Boat,
        KindOf::Transport,
        KindOf::ImmuneToCapture,
        KindOf::DefensiveWall,
        KindOf::WalkOnTopOfWall,
        KindOf::Bridge,
        KindOf::BridgeTower,
        KindOf::StealthGarrison,
        KindOf::TechBuilding,
        KindOf::LandmarkBridge,
        KindOf::AutoRallypoint,
        KindOf::MobNexus,
        KindOf::NoCollide,
        KindOf::ForceAttackable,
        KindOf::Shrubbery,
        KindOf::ClearedByBuild,
        KindOf::Inert,
        KindOf::BlastCrater,
        KindOf::HugeVehicle,
        KindOf::CliffJumper,
        KindOf::CleanupHazard,
        KindOf::LineBuild,
        KindOf::Hulk,
        KindOf::RejectUnmanned,
        KindOf::SpawnsAreTheWeapons,
        KindOf::PortableStructure,
        KindOf::BoobyTrap,
        KindOf::ProducedAtHelipad,
        KindOf::EmpHardened,
        KindOf::RevealsEnemyPaths,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Infantry => "Infantry",
            Self::Vehicle => "Vehicle",
            Self::Aircraft => "Aircraft",
            Self::Projectile => "Projectile",
            Self::Resource => "Resource",
            Self::Selectable => "Selectable",
            Self::Attackable => "Attackable",
            Self::CommandCenter => "CommandCenter",
            Self::Worker => "Worker",
            Self::Hero => "Hero",
            Self::SupplyCenter => "SupplyCenter",
            Self::PowerPlant => "PowerPlant",
            Self::FSBarracks => "FSBarracks",
            Self::FSWarFactory => "FSWarFactory",
            Self::FSAirfield => "FSAirfield",
            Self::FSInternetCenter => "FSInternetCenter",
            Self::FSPower => "FSPower",
            Self::FSBaseDefense => "FSBaseDefense",
            Self::FSSupplyDropzone => "FSSupplyDropzone",
            Self::FSSupplyCenter => "FSSupplyCenter",
            Self::FSSuperweapon => "FSSuperweapon",
            Self::FSStrategyCenter => "FSStrategyCenter",
            Self::FSFake => "FSFake",
            Self::FSTechnology => "FSTechnology",
            Self::FSBlackMarket => "FSBlackMarket",
            Self::FSAdvancedTech => "FSAdvancedTech",
            Self::Harvestable => "Harvestable",
            Self::Powered => "Powered",
            Self::AttackNeedsLineOfSight => "AttackNeedsLineOfSight",
            Self::Immobile => "Immobile",
            Self::CanBeRepulsed => "CanBeRepulsed",
            Self::CannotRetaliate => "CannotRetaliate",
            Self::Drone => "Drone",
            Self::IgnoredInGui => "IgnoredInGui",
            Self::Salvager => "Salvager",
            Self::WeaponSalvager => "WeaponSalvager",
            Self::ArmorSalvager => "ArmorSalvager",
            Self::AircraftPathAround => "AircraftPathAround",
            Self::WaveGuide => "WaveGuide",
            Self::Dozer => "Dozer",
            Self::Harvester => "Harvester",
            Self::Unattackable => "Unattackable",
            Self::Mine => "Mine",
            Self::DemoTrap => "DemoTrap",
            Self::SmallMissile => "SmallMissile",
            Self::BallisticMissile => "BallisticMissile",
            Self::Parachute => "Parachute",
            Self::Disguiser => "Disguiser",
            Self::RepairPad => "RepairPad",
            Self::HealPad => "HealPad",
            Self::MoneyHacker => "MoneyHacker",
            Self::SupplySource => "SupplySource",
            Self::CannotBuildNearSupplies => "CannotBuildNearSupplies",
            Self::Crate => "Crate",
            Self::IgnoresSelectAll => "IgnoresSelectAll",
            Self::AlwaysSelectable => "AlwaysSelectable",
            Self::MpCountForVictory => "MpCountForVictory",
            Self::Score => "Score",
            Self::ScoreCreate => "ScoreCreate",
            Self::ScoreDestroy => "ScoreDestroy",
            Self::CanSeeThrough => "CanSeeThrough",
            Self::NoGarrison => "NoGarrison",
            Self::GarrisonableUntilDestroyed => "GarrisonableUntilDestroyed",
            Self::Boat => "Boat",
            Self::Transport => "Transport",
            Self::ImmuneToCapture => "ImmuneToCapture",
            Self::DefensiveWall => "DefensiveWall",
            Self::WalkOnTopOfWall => "WalkOnTopOfWall",
            Self::Bridge => "Bridge",
            Self::BridgeTower => "BridgeTower",
            Self::StealthGarrison => "StealthGarrison",
            Self::TechBuilding => "TechBuilding",
            Self::LandmarkBridge => "LandmarkBridge",
            Self::AutoRallypoint => "AutoRallypoint",
            Self::MobNexus => "MobNexus",
            Self::NoCollide => "NoCollide",
            Self::ForceAttackable => "ForceAttackable",
            Self::Shrubbery => "Shrubbery",
            Self::ClearedByBuild => "ClearedByBuild",
            Self::Inert => "Inert",
            Self::BlastCrater => "BlastCrater",
            Self::HugeVehicle => "HugeVehicle",
            Self::CliffJumper => "CliffJumper",
            Self::CleanupHazard => "CleanupHazard",
            Self::LineBuild => "LineBuild",
            Self::Hulk => "Hulk",
            Self::RejectUnmanned => "RejectUnmanned",
            Self::SpawnsAreTheWeapons => "SpawnsAreTheWeapons",
            Self::PortableStructure => "PortableStructure",
            Self::BoobyTrap => "BoobyTrap",
            Self::ProducedAtHelipad => "ProducedAtHelipad",
            Self::EmpHardened => "EmpHardened",
            Self::RevealsEnemyPaths => "RevealsEnemyPaths",
        }
    }

    /// Map one C++ KindOf.h token onto the live host capability set.
    pub fn from_ini_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_uppercase().as_str() {
            "ALWAYS_SELECTABLE" => Some(Self::AlwaysSelectable),
            "MP_COUNT_FOR_VICTORY" => Some(Self::MpCountForVictory),
            "SCORE" => Some(Self::Score),
            "SCORE_CREATE" => Some(Self::ScoreCreate),
            "SCORE_DESTROY" => Some(Self::ScoreDestroy),
            "CAN_SEE_THROUGH" | "CAN_SEE_THROUGH_STRUCTURE" => Some(Self::CanSeeThrough),
            "NO_GARRISON" | "NOGARRISON" => Some(Self::NoGarrison),
            "GARRISONABLE_UNTIL_DESTROYED" => Some(Self::GarrisonableUntilDestroyed),
            "BOAT" => Some(Self::Boat),
            "TRANSPORT" => Some(Self::Transport),
            "IMMUNE_TO_CAPTURE" | "IMMUNETOCAPTURE" => Some(Self::ImmuneToCapture),
            "DEFENSIVE_WALL" | "DEFENSIVEWALL" => Some(Self::DefensiveWall),
            "WALK_ON_TOP_OF_WALL" | "WALKONTOPOFWALL" => Some(Self::WalkOnTopOfWall),
            "BRIDGE" => Some(Self::Bridge),
            "LANDMARK_BRIDGE" | "LANDMARKBRIDGE" => Some(Self::LandmarkBridge),
            "BRIDGE_TOWER" | "BRIDGETOWER" => Some(Self::BridgeTower),
            "STEALTH_GARRISON" | "STEALTHGARRISON" => Some(Self::StealthGarrison),
            "TECH_BUILDING" | "TECHBUILDING" => Some(Self::TechBuilding),
            "AUTO_RALLYPOINT" | "AUTO_RALLY_POINT" => Some(Self::AutoRallypoint),
            "MOB_NEXUS" | "MOBNEXUS" => Some(Self::MobNexus),
            "NO_COLLIDE" | "NOCOLLIDE" => Some(Self::NoCollide),
            "FORCEATTACKABLE" | "FORCE_ATTACKABLE" => Some(Self::ForceAttackable),
            "SHRUBBERY" => Some(Self::Shrubbery),
            "AIRCRAFT_PATH_AROUND" => Some(Self::AircraftPathAround),
            "CLEARED_BY_BUILD" | "CLEAREDBYBUILD" => Some(Self::ClearedByBuild),
            "INERT" => Some(Self::Inert),
            "BLAST_CRATER" | "BLASTCRATER" => Some(Self::BlastCrater),
            "CLEANUP_HAZARD" | "CLEANUPHAZARD" => Some(Self::CleanupHazard),
            "HUGE_VEHICLE" | "HUGEVEHICLE" => Some(Self::HugeVehicle),
            "CLIFF_JUMPER" | "CLIFFJUMPER" => Some(Self::CliffJumper),
            "LINEBUILD" | "LINE_BUILD" => Some(Self::LineBuild),
            "HULK" => Some(Self::Hulk),
            "REJECT_UNMANNED" | "REJECTUNMANNED" => Some(Self::RejectUnmanned),
            "SPAWNS_ARE_THE_WEAPONS" | "SPAWNSARETHEWEAPONS" => Some(Self::SpawnsAreTheWeapons),
            "PORTABLE_STRUCTURE" | "PORTABLESTRUCTURE" => Some(Self::PortableStructure),
            "BOOBY_TRAP" | "BOOBYTRAP" => Some(Self::BoobyTrap),
            "PRODUCED_AT_HELIPAD" | "PRODUCEDATHELIPAD" => Some(Self::ProducedAtHelipad),
            "EMP_HARDENED" | "EMPHARDENED" => Some(Self::EmpHardened),
            "REVEALS_ENEMY_PATHS" | "REVEALSENEMYPATHS" => Some(Self::RevealsEnemyPaths),

            "DRONE" => Some(Self::Drone),
            _ => None,
        }
    }
}
