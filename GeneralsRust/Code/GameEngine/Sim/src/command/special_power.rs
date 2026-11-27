//! Retail special-power command names and targets.

use crate::special_power::{self, SpecialPowerType as CppSpecialPowerType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecialPowerType {
    Airstrike,
    /// China Artillery Barrage residual (SPECIAL_ARTILLERY_BARRAGE /
    /// SuperweaponArtilleryBarrage family). Delayed multi-shell scatter area
    /// damage (`ArtilleryBarrageDamageWeapon` residual within WeaponErrorRadius).
    /// Fail-closed: not full ChinaArtilleryCannon OCL DeliverPayload /
    /// random scatter draw / science-tier FormationSize 12/24/36 path.
    Artillery,
    /// Carpet Bomb residual (SPECIAL_CARPET_BOMB / SuperweaponCarpetBomb family).
    /// Delayed bomber approach then multi-point line/area explosive damage
    /// (`CarpetBombWeapon` residual along drop line).
    /// Fail-closed: not full B52/ChinaJet OCL DeliverPayload / DropVariance /
    /// staggered DropDelay / science-tier upgrade path.
    CarpetBomb,
    /// China Airforce Early Carpet Bomb residual (EARLY_SPECIAL_CHINA_CARPET_BOMB /
    /// Early_SuperweaponChinaCarpetBomb). China payload matrix + Early science gate.
    EarlyChinaCarpetBomb,
    /// USA Airforce Carpet Bomb residual (AIRF_SPECIAL_CARPET_BOMB /
    /// AirF_SuperweaponCarpetBomb). AirForce payload matrix (12 bombs / 130ms delay).
    AirForceCarpetBomb,
    ClusterMines,
    /// USA Superweapon Crate Drop residual (SPECIAL_CRATE_DROP / SuperweaponCrateDrop).
    /// Spawns residual 200DollarCrate × 10 near target.
    /// Fail-closed vs full AmericaJetCargoPlane OCL DeliverPayload Object.
    CrateDrop,
    DaisyCutter,
    EmergencyRepair,
    /// Early Emergency Repair residual (EARLY_SPECIAL_REPAIR_VEHICLES).
    EarlyEmergencyRepair,
    FuelAirBomb,
    Healing,
    IonCannon,
    NapalmStrike,
    NuclearMissile,
    /// GLA Black Market Nuke residual (SPECIAL_BLACK_MARKET_NUKE /
    /// SuperweaponBlackMarketNuke). Host maps to NuclearMissile radiation residual.
    /// Fail-closed vs full BlackMarketNuke OCL / smaller dirty yield matrix.
    BlackMarketNuke,
    /// GLA Detonate Dirty Nuke residual (SPECIAL_DETONATE_DIRTY_NUKE /
    /// SuperweaponDetonateDirtyNuke). Short-reload host NuclearMissile residual.
    DetonateDirtyNuke,
    /// USA America Airborne / SuperweaponParadropAmerica residual.
    Paradrop,
    /// Infantry General Paradrop residual (Infa_SuperweaponInfantryParadrop).
    /// Host uses Infa_AmericaInfantryRanger + Infa_SCIENCE_InfantryParadrop tiers.
    InfantryParadrop,
    /// Tank General Tank Paradrop residual (Tank_SuperweaponTankParadrop).
    /// Host uses Tank_AmericaTankCrusader + SCIENCE_TankParadrop 1/2/3 counts.
    TankParadrop,
    /// GLA Rebel Ambush / SuperweaponRebelAmbush residual (SPECIAL_AMBUSH).
    /// Synchronous leftover OCLSpecialPower UpgradeOCL spawn at the click.
    Ambush,
    /// GLA Terror Cell residual (SPECIAL_TERROR_CELL / SuperweaponTerrorCell).
    /// Host maps to Ambush infantry spawn residual (fail-closed vs full OCL cell).
    TerrorCell,
    ParticleCannon,
    RadarScan,
    ScudStorm,
    /// USA Spy Satellite Scan residual — temporary FOW reveal at location.
    SpySatellite,
    /// USA CIA Intelligence residual (SpyVision / setUnitsVisionSpied).
    /// Temporarily reveals enemy units (vision-spied + FOW + DETECTED residual).
    /// Fail-closed: not full SpyVisionUpdate module / kindof filter / sabotage path.
    CiaIntelligence,
    /// USA Communications Download residual (SPECIAL_COMMUNICATIONS_DOWNLOAD).
    /// Host maps to CIA Intelligence SpyVision residual (shared activate audio).
    /// Fail-closed vs full Pathfinder module duration matrix.
    CommunicationsDownload,
    SpyDrone,
    SuperweaponCountermeasures,
    /// China Dragon Tank FireWall / Firestorm residual (FIRE_WEAPON secondary path).
    /// Creates a line of fire damage zones toward the target location.
    FireWall,
    /// GLA Anthrax Bomb residual (SPECIAL_ANTHRAX_BOMB / SuperweaponAnthraxBomb).
    /// Delayed plane-drop blast + residual toxin field ticks.
    /// Leftover findOCL: Chem_GLACommandCenter / Gamma upgrade → SUPERWEAPON_AnthraxBombGamma.
    AnthraxBomb,
    /// USA Spectre Gunship residual (SPECIAL_SPECTRE_GUNSHIP / SuperweaponSpectreGunship).
    /// Delayed orbit insertion at target + periodic howitzer residual damage ticks
    /// in AttackAreaRadius for OrbitTime.
    /// Fail-closed: not full SpectreGunshipUpdate OCL aircraft / gattling strafe /
    /// howitzer projectile / decal / contain gunner path.
    SpectreGunship,
    /// China EMP Pulse residual (SPECIAL_EMP_PULSE / SuperweaponEMPPulse).
    /// Temporarily disables vehicles/structures in radius (DISABLED_EMP).
    /// Fail-closed: not full OCL EMPPulseBomb / EMPPulseEffectSpheroid drawable path.
    EmpPulse,
    /// China Frenzy ("Rage") residual (SPECIAL_FRENZY / SuperweaponFrenzy).
    /// Temporary ally attack buff in radius (WEAPONBONUSCONDITION_FRENZY_*).
    /// Fail-closed: not full OCL Frenzy_InvisibleMarker / FrenzyCloud particle path.
    Frenzy,
    /// China Airforce Early Frenzy residual (EARLY_SPECIAL_FRENZY /
    /// Early_SuperweaponFrenzy). Same residual levels; Early science gate.
    EarlyFrenzy,
    /// USA Strategy Center Bombardment battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_ONE / PLANSTATUS_BOMBARDMENT).
    /// Army-wide DAMAGE 120% residual for legal members.
    /// Fail-closed: not full BattlePlanUpdate pack/unpack / turret enable matrix.
    BattlePlanBombardment,
    /// USA Strategy Center HoldTheLine battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_TWO / PLANSTATUS_HOLDTHELINE).
    /// Army armor damage scalar 0.9 + Strategy Center max-health 2.0 residual.
    /// Fail-closed: not full paralyze / door animation matrix.
    BattlePlanHoldTheLine,
    /// USA Strategy Center SearchAndDestroy battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_THREE / PLANSTATUS_SEARCHANDDESTROY).
    /// Army RANGE 120% + sight 1.2 residual; center detect residual.
    /// Fail-closed: not full StealthDetectorUpdate module / vision object path.
    BattlePlanSearchAndDestroy,
    /// GLA GPS Scrambler residual (SPECIAL_GPS_SCRAMBLER / SuperweaponGPSScrambler).
    /// Grants temporary STEALTHED to ally vehicles/infantry in radius (GrantStealth).
    /// Fail-closed: not full OCL GPSScrambler_InvisibleMarker grow-radius pulse path.
    GpsScrambler,
    /// USA Leaflet Drop residual (SPECIAL_LEAFLET_DROP / SuperweaponLeafletDrop).
    /// Delayed disable of enemy infantry/vehicles in radius (DISABLED_EMP residual).
    /// Fail-closed: not full OCL B52 / LeafletContainer / LeafletFX particle path.
    LeafletDrop,
    /// USA Airforce Early Leaflet Drop residual (EARLY_SPECIAL_LEAFLET_DROP /
    /// Early_SuperweaponLeafletDrop). Same delay/radius/reload; Early science gate.
    EarlyLeafletDrop,
    /// GLA Sneak Attack residual (SPECIAL_SNEAK_ATTACK / SuperweaponSneakAttack).
    /// Delayed tunnel structure spawn at target + residual shockwave damage.
    /// Fail-closed: not full OCL Start animation / multi-shockwave / TunnelContain path.
    SneakAttack,
    /// USA Superweapon General Cruise Missile residual
    /// (SUPR_SPECIAL_CRUISE_MISSILE / SupW_CruiseMissile / SUPERWEAPON_CruiseMissile).
    /// Delayed loft-to-target strike with `MOABDetonationWeapon` area damage.
    /// Fail-closed: not full NeutronMissileUpdate loft / door animation /
    /// OCL FireWeapon projectile path / MOABFlameWeapon secondary.
    CruiseMissile,
    /// USA Ambulance Cleanup Area residual
    /// (`SpecialAbilityAmbulanceCleanupArea` / CleanupAreaPower).
    /// Clears residual toxin/radiation fields and mines at a world location.
    /// Fail-closed: not full CleanupHazardUpdate projectile stream / scan loop /
    /// HazardousMaterialArmor object stack / rubble pathfind clear.
    CleanupArea,
    /// China Helix NapalmBomb residual (`SpecialAbilityHelixNapalmBomb` /
    /// SPECIAL_HELIX_NAPALM_BOMB). Instant NapalmBomb blast + FirestormSmall DoT
    /// at target location (requires Upgrade_HelixNapalmBomb residual unlock).
    /// Fail-closed: not full SpecialObject NapalmBomb fall / HeightDieUpdate /
    /// FirestormDynamicGeometryInfoUpdate expand animation.
    HelixNapalmBomb,
    /// Nuke General Helix NukeBomb residual (`Nuke_SpecialAbilityHelixNukeBomb`).
    /// Same SPECIAL_HELIX_NAPALM_BOMB enum; host maps to Helix napalm residual path
    /// with Nuke_Upgrade_HelixNukeBomb unlock residual.
    HelixNukeBomb,
    /// China SuperweaponCashHack residual (SPECIAL_CASH_HACK / SuperweaponCashHack).
    /// Instant steal from richest enemy player by SCIENCE_CashHack1/2/3 amount
    /// (1000/2000/4000). Fail-closed vs full CashHackSpecialPower victim clamp /
    /// floating text / multiplayer academy path.
    CashHack,
    /// USA Airforce DaisyCutter residual (AIRF_SPECIAL_DAISY_CUTTER).
    AirForceDaisyCutter,
    /// USA Airforce A10 residual (AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE).
    AirForceAirstrike,
    /// USA Airforce Spectre residual (AIRF_SPECIAL_SPECTRE_GUNSHIP).
    AirForceSpectreGunship,
    /// Superweapon General PUC residual (SUPW_SPECIAL_PARTICLE_UPLINK_CANNON).
    SuperweaponParticleCannon,
    /// Nuke General NeutronMissile residual (NUKE_SPECIAL_NEUTRON_MISSILE).
    NukeNeutronMissile,
    /// Superweapon General NeutronMissile residual (SUPW path alias).
    SuperweaponNeutronMissile,
    /// Nuke General China CarpetBomb residual.
    NukeChinaCarpetBomb,
    /// Stealth General GPS Scrambler residual (Slth_SuperweaponGPSScrambler).
    StealthGpsScrambler,
    /// Launch Baikonur Rocket residual (SPECIAL_LAUNCH_BAIKONUR_ROCKET).
    BaikonurRocket,
    /// Defector special power residual (SPECIAL_DEFECTOR / SpecialPowerDefector).
    /// Clicks enemy unit/structure → defects to caster with undetected timer.
    Defector,
    /// Nuke General NukeDrop residual (NUKE_SPECIAL_CLUSTER_MINES).
    NukeDrop,
    /// Battleship Bombardment residual (SPECIAL_BATTLESHIP_BOMBARDMENT).
    BattleshipBombardment,
    /// Laser General Particle Uplink residual (LAZR_SPECIAL_PARTICLE_UPLINK_CANNON).
    /// Host maps to ParticleCannon residual path.
    LaserCannon,
    /// USA Missile Defender laser guided special residual
    /// (SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES).
    /// Locks secondary laser weapon + attack target (StartAbilityRange 200).
    MissileDefenderLaserGuided,
    /// China Tank Hunter TNT special residual (SPECIAL_TANKHUNTER_TNT_ATTACK).
    /// Plants sticky timed charge (StartAbilityRange 5, Reload 7500ms).
    TankHunterTnt,
    /// Laser General Howitzer laser guided residual
    /// (shares SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES enum).
    /// Host maps to secondary laser-lock attack residual.
    LaserGuidedHowitzer,
    /// Demo Rebel timed charges residual (SPECIAL_TIMED_CHARGES / Reload 30000ms).
    DemoRebelTimedCharges,
    /// Demo Kell timed charges residual (SPECIAL_TIMED_CHARGES).
    DemoKellTimedCharges,
    /// Demo Kell sticky charges residual (SPECIAL_TIMED_CHARGES).
    DemoKellStickyCharges,
    /// Demo Kell remote charges residual (SPECIAL_REMOTE_CHARGES).
    DemoKellRemoteCharges,
    /// Battle Bus demo trap rollout residual (SPECIAL_TIMED_CHARGES / Reload 7500ms).
    BattleBusDemoTrapRollout,
    /// Colonel Burton timed charges residual (SPECIAL_TIMED_CHARGES).
    BurtonTimedCharges,
    /// Colonel Burton remote charges residual (SPECIAL_REMOTE_CHARGES).
    BurtonRemoteCharges,
    /// China Hacker disable building residual (SPECIAL_HACKER_DISABLE_BUILDING).
    HackerDisableBuilding,
    /// Black Lotus disable vehicle residual (SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK).
    BlackLotusDisableVehicle,
    /// Black Lotus steal cash residual (SPECIAL_BLACKLOTUS_STEAL_CASH_HACK).
    BlackLotusStealCash,
    /// Black Lotus capture building residual (SPECIAL_BLACKLOTUS_CAPTURE_BUILDING).
    BlackLotusCaptureBuilding,
    /// Microwave Tank disable building (C++ Enum SPECIAL_HACKER_DISABLE_BUILDING).
    MicrowaveDisableBuilding,
    /// Ranger capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RangerCaptureBuilding,
    /// Red Guard capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RedGuardCaptureBuilding,
    /// Rebel capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RebelCaptureBuilding,
    /// Bomb Truck disguise residual (SPECIAL_DISGUISE_AS_VEHICLE).
    DisguiseAsVehiclePower,
    Invalid,
}

/// Resolve an exact retail `CommandButton.ini` `SpecialPower` name.
///
/// Command buttons carry the INI identity as text, whereas the authoritative
/// host uses [`SpecialPowerType`].  Keeping this conversion in one explicit
/// table avoids treating an arbitrary legacy numeric ID, condition suffix, or
/// fuzzy name match as a valid power.  Separator/case differences are benign;
/// every accepted key is otherwise a real retail name from CommandButton.ini.
pub fn special_power_type_from_template_name(name: &str) -> Option<SpecialPowerType> {
    let key: String = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();

    use SpecialPowerType as Power;
    Some(match key.as_str() {
        "airfsuperweapona10thunderboltmissilestrike" => Power::AirForceAirstrike,
        "airfsuperweaponcarpetbomb" => Power::AirForceCarpetBomb,
        "airfsuperweaponspectregunship" => Power::AirForceSpectreGunship,
        "demospecialabilitydemokelltimedcharges" => Power::DemoKellTimedCharges,
        "demospecialabilitydemorebeltimedcharges" => Power::DemoRebelTimedCharges,
        "demospecialabilitykellremotecharges" => Power::DemoKellRemoteCharges,
        "earlysuperweaponchinacarpetbomb" => Power::EarlyChinaCarpetBomb,
        "earlysuperweaponemergencyrepair" => Power::EarlyEmergencyRepair,
        "earlysuperweaponfrenzy" => Power::EarlyFrenzy,
        "earlysuperweaponleafletdrop" => Power::EarlyLeafletDrop,
        "infasuperweaponinfantryparadrop" => Power::InfantryParadrop,
        "lazrlasercannon" => Power::LaserCannon,
        "lazrspecialabilitylaserguidedhowitzer" => Power::LaserGuidedHowitzer,
        "nukespecialabilityhelixnukebomb" => Power::HelixNukeBomb,
        "nukesuperweaponchinacarpetbomb" => Power::NukeChinaCarpetBomb,
        "nukesuperweaponneutronmissile" => Power::NukeNeutronMissile,
        "nukesuperweaponnukedrop" => Power::NukeDrop,
        "slthsuperweapongpsscrambler" => Power::StealthGpsScrambler,
        "specialabilityambulancecleanuparea" => Power::CleanupArea,
        "specialabilityblacklotuscapturebuilding" => Power::BlackLotusCaptureBuilding,
        "specialabilityblacklotusdisablevehiclehack" => Power::BlackLotusDisableVehicle,
        "specialabilityblacklotusstealcashhack" => Power::BlackLotusStealCash,
        "specialabilitycolonelburtonremotecharges" => Power::BurtonRemoteCharges,
        "specialabilitycolonelburtontimedcharges" => Power::BurtonTimedCharges,
        "specialabilitydisguiseasvehicle" => Power::DisguiseAsVehiclePower,
        "specialabilityhackerdisablebuilding" => Power::HackerDisableBuilding,
        "specialabilityhelixnapalmbomb" => Power::HelixNapalmBomb,
        "specialabilitymicrowavedisablebuilding" => Power::MicrowaveDisableBuilding,
        "specialabilitymissiledefenderlaserguidedmissiles" => Power::MissileDefenderLaserGuided,
        "specialabilityrangercapturebuilding" => Power::RangerCaptureBuilding,
        "specialabilityrebelcapturebuilding" => Power::RebelCaptureBuilding,
        "specialabilityredguardcapturebuilding" => Power::RedGuardCaptureBuilding,
        "specialabilitytankhuntertntattack" => Power::TankHunterTnt,
        "specialpowerbattleshipbombardment" => Power::BattleshipBombardment,
        "specialpowercommunicationsdownload" => Power::CommunicationsDownload,
        "specialpowerradarvanscan" => Power::RadarScan,
        "specialpowerspydrone" => Power::SpyDrone,
        "specialpowerspysatellite" => Power::SpySatellite,
        "superweapona10thunderboltmissilestrike" => Power::Airstrike,
        "superweaponanthraxbomb" => Power::AnthraxBomb,
        "superweaponartillerybarrage" => Power::Artillery,
        "superweaponcarpetbomb" | "superweaponchinacarpetbomb" => Power::CarpetBomb,
        "superweaponcashhack" => Power::CashHack,
        "superweaponciaintelligence" => Power::CiaIntelligence,
        "superweaponclustermines" => Power::ClusterMines,
        "superweaponcratedrop" => Power::CrateDrop,
        "superweapondaisycutter" => Power::DaisyCutter,
        "superweaponemergencyrepair" => Power::EmergencyRepair,
        "superweaponemppulse" => Power::EmpPulse,
        "superweaponfrenzy" => Power::Frenzy,
        "superweapongpsscrambler" => Power::GpsScrambler,
        "superweaponlaunchbaikonurrocket" => Power::BaikonurRocket,
        "superweaponleafletdrop" => Power::LeafletDrop,
        "superweaponnapalmstrike" => Power::NapalmStrike,
        "superweaponneutronmissile" => Power::NuclearMissile,
        "superweaponparadropamerica" => Power::Paradrop,
        "superweaponparticleuplinkcannon" => Power::ParticleCannon,
        "superweaponrebelambush" => Power::Ambush,
        "superweaponscudstorm" => Power::ScudStorm,
        "superweaponsneakattack" => Power::SneakAttack,
        "superweaponspectregunship" => Power::SpectreGunship,
        "superweaponterrorcell" => Power::TerrorCell,
        "supwcruisemissile" => Power::CruiseMissile,
        "supwsuperweaponparticleuplinkcannon" => Power::SuperweaponParticleCannon,
        "supwsuperweaponneutronmissile" => Power::SuperweaponNeutronMissile,
        "tanksuperweapontankparadrop" => Power::TankParadrop,
        // These two retail names are represented by target-command variants
        // rather than a SpecialPowerType.  The Control Bar host bridge handles
        // them separately and must not guess a nearby power here.
        "specialabilityboobytrap" | "specialabilitychangebattleplans" => return None,
        _ => return None,
    })
}

/// Weapon slot identifiers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponSlot {
    Primary,
    Secondary,
    Tertiary,
    AntiAir,
    Slot(u32),
}

impl WeaponSlot {
    /// Resolve a command slot to one of the original Object weapon-set slots.
    pub fn concrete(&self) -> Option<crate::weapon::WeaponSlotType> {
        use crate::weapon::WeaponSlotType;

        match *self {
            Self::Primary => Some(WeaponSlotType::Primary),
            Self::Secondary => Some(WeaponSlotType::Secondary),
            Self::Tertiary => Some(WeaponSlotType::Tertiary),
            Self::AntiAir => None,
            Self::Slot(ordinal) => WeaponSlotType::from_u32(ordinal),
        }
    }
}

/// Resolve an authored command to its C++ special-power kind.
pub fn cpp_special_power(power: &SpecialPowerType) -> Option<CppSpecialPowerType> {
    match power {
        SpecialPowerType::DaisyCutter | SpecialPowerType::FuelAirBomb => {
            Some(CppSpecialPowerType::DaisyCutter)
        }
        SpecialPowerType::Airstrike => Some(CppSpecialPowerType::A10ThunderboltStrike),
        SpecialPowerType::Artillery => Some(CppSpecialPowerType::ArtilleryBarrage),
        SpecialPowerType::CarpetBomb => Some(CppSpecialPowerType::CarpetBomb),
        SpecialPowerType::EarlyChinaCarpetBomb => Some(CppSpecialPowerType::EarlyChinaCarpetBomb),
        SpecialPowerType::AirForceCarpetBomb => Some(CppSpecialPowerType::AirfCarpetBomb),
        SpecialPowerType::ClusterMines => Some(CppSpecialPowerType::ClusterMines),
        SpecialPowerType::CrateDrop => Some(CppSpecialPowerType::CrateDrop),
        SpecialPowerType::EmergencyRepair => Some(CppSpecialPowerType::RepairVehicles),
        SpecialPowerType::EarlyEmergencyRepair => Some(CppSpecialPowerType::EarlyRepairVehicles),
        SpecialPowerType::NapalmStrike => Some(CppSpecialPowerType::NapalmStrike),
        SpecialPowerType::NuclearMissile => Some(CppSpecialPowerType::NeutronMissile),
        SpecialPowerType::BlackMarketNuke => Some(CppSpecialPowerType::BlackMarketNuke),
        SpecialPowerType::DetonateDirtyNuke => Some(CppSpecialPowerType::DetonateDirtyNuke),
        SpecialPowerType::Paradrop => Some(CppSpecialPowerType::ParadropAmerica),
        SpecialPowerType::InfantryParadrop => Some(CppSpecialPowerType::InfaParadropAmerica),
        SpecialPowerType::TankParadrop => Some(CppSpecialPowerType::TankParadrop),
        SpecialPowerType::Ambush => Some(CppSpecialPowerType::Ambush),
        SpecialPowerType::TerrorCell => Some(CppSpecialPowerType::TerrorCell),
        SpecialPowerType::ParticleCannon => Some(CppSpecialPowerType::ParticleUplinkCannon),
        SpecialPowerType::RadarScan => Some(CppSpecialPowerType::RadarVanScan),
        SpecialPowerType::ScudStorm => Some(CppSpecialPowerType::ScudStorm),
        SpecialPowerType::SpySatellite => Some(CppSpecialPowerType::SpySatellite),
        SpecialPowerType::CiaIntelligence => Some(CppSpecialPowerType::CiaIntelligence),
        SpecialPowerType::CommunicationsDownload => {
            Some(CppSpecialPowerType::CommunicationsDownload)
        }
        SpecialPowerType::SpyDrone => Some(CppSpecialPowerType::SpyDrone),
        SpecialPowerType::AnthraxBomb => Some(CppSpecialPowerType::AnthraxBomb),
        SpecialPowerType::SpectreGunship => Some(CppSpecialPowerType::SpectreGunship),
        SpecialPowerType::EmpPulse => Some(CppSpecialPowerType::EmpPulse),
        SpecialPowerType::Frenzy => Some(CppSpecialPowerType::Frenzy),
        SpecialPowerType::EarlyFrenzy => Some(CppSpecialPowerType::EarlyFrenzy),
        SpecialPowerType::BattlePlanBombardment
        | SpecialPowerType::BattlePlanHoldTheLine
        | SpecialPowerType::BattlePlanSearchAndDestroy => {
            Some(CppSpecialPowerType::ChangeBattlePlans)
        }
        SpecialPowerType::GpsScrambler => Some(CppSpecialPowerType::GpsScrambler),
        SpecialPowerType::LeafletDrop => Some(CppSpecialPowerType::LeafletDrop),
        SpecialPowerType::EarlyLeafletDrop => Some(CppSpecialPowerType::EarlyLeafletDrop),
        SpecialPowerType::SneakAttack => Some(CppSpecialPowerType::SneakAttack),
        SpecialPowerType::CruiseMissile => Some(CppSpecialPowerType::SuprCruiseMissile),
        SpecialPowerType::CleanupArea => Some(CppSpecialPowerType::CleanupArea),
        SpecialPowerType::HelixNapalmBomb => Some(CppSpecialPowerType::HelixNapalmBomb),
        // Retail Nuke_SpecialAbilityHelixNukeBomb shares SPECIAL_HELIX_NAPALM_BOMB enum.
        SpecialPowerType::HelixNukeBomb => Some(CppSpecialPowerType::HelixNapalmBomb),
        SpecialPowerType::CashHack => Some(CppSpecialPowerType::CashHack),
        SpecialPowerType::AirForceDaisyCutter => Some(CppSpecialPowerType::AirfDaisyCutter),
        SpecialPowerType::AirForceAirstrike => Some(CppSpecialPowerType::AirfA10ThunderboltStrike),
        SpecialPowerType::AirForceSpectreGunship => Some(CppSpecialPowerType::AirfSpectreGunship),
        SpecialPowerType::SuperweaponParticleCannon => {
            Some(CppSpecialPowerType::SupwParticleUplinkCannon)
        }
        SpecialPowerType::NukeNeutronMissile => Some(CppSpecialPowerType::NukeNeutronMissile),
        SpecialPowerType::SuperweaponNeutronMissile => {
            Some(CppSpecialPowerType::SupwNeutronMissile)
        }
        SpecialPowerType::NukeChinaCarpetBomb => Some(CppSpecialPowerType::EarlyChinaCarpetBomb),
        SpecialPowerType::StealthGpsScrambler => Some(CppSpecialPowerType::SlthGpsScrambler),
        SpecialPowerType::BaikonurRocket => Some(CppSpecialPowerType::LaunchBaikonurRocket),
        SpecialPowerType::Defector => Some(CppSpecialPowerType::Defector),
        SpecialPowerType::NukeDrop => Some(CppSpecialPowerType::NukeClusterMines),
        SpecialPowerType::BattleshipBombardment => Some(CppSpecialPowerType::BattleshipBombardment),
        SpecialPowerType::LaserCannon => Some(CppSpecialPowerType::LazrParticleUplinkCannon),
        SpecialPowerType::MissileDefenderLaserGuided => {
            Some(CppSpecialPowerType::MissileDefenderLaserGuidedMissiles)
        }
        SpecialPowerType::TankHunterTnt => Some(CppSpecialPowerType::TankHunterTntAttack),
        SpecialPowerType::LaserGuidedHowitzer => {
            Some(CppSpecialPowerType::MissileDefenderLaserGuidedMissiles)
        }
        SpecialPowerType::DemoRebelTimedCharges
        | SpecialPowerType::DemoKellTimedCharges
        | SpecialPowerType::DemoKellStickyCharges
        | SpecialPowerType::BattleBusDemoTrapRollout
        | SpecialPowerType::BurtonTimedCharges => Some(CppSpecialPowerType::TimedCharges),
        SpecialPowerType::DemoKellRemoteCharges | SpecialPowerType::BurtonRemoteCharges => {
            Some(CppSpecialPowerType::RemoteCharges)
        }
        SpecialPowerType::HackerDisableBuilding | SpecialPowerType::MicrowaveDisableBuilding => {
            Some(CppSpecialPowerType::HackerDisableBuilding)
        }
        SpecialPowerType::BlackLotusDisableVehicle => {
            Some(CppSpecialPowerType::BlackLotusDisableVehicleHack)
        }
        SpecialPowerType::BlackLotusStealCash => Some(CppSpecialPowerType::BlackLotusStealCashHack),
        SpecialPowerType::BlackLotusCaptureBuilding => {
            Some(CppSpecialPowerType::BlackLotusCaptureBuilding)
        }
        SpecialPowerType::RangerCaptureBuilding
        | SpecialPowerType::RedGuardCaptureBuilding
        | SpecialPowerType::RebelCaptureBuilding => {
            Some(CppSpecialPowerType::InfantryCaptureBuilding)
        }
        SpecialPowerType::DisguiseAsVehiclePower => Some(CppSpecialPowerType::DisguiseAsVehicle),
        SpecialPowerType::Invalid => Some(CppSpecialPowerType::Invalid),
        // FireWall is a host FIRE_WEAPON residual (Dragon Tank), not a SpecialPowerType
        // discriminant in C++ SpecialPowerType.h — no unique SPECIAL_* name residual.
        SpecialPowerType::FireWall => None,
        // Host residual placeholders without unique C++ SPECIAL_* discriminant.
        SpecialPowerType::Healing
        | SpecialPowerType::IonCannon
        | SpecialPowerType::SuperweaponCountermeasures => None,
    }
}

/// The original SPECIAL_* name of the command's authored power.
pub fn cpp_special_power_name(power: &SpecialPowerType) -> Option<&'static str> {
    cpp_special_power(power).map(|power| power.as_str())
}

/// Whether a unit-under-cursor click still requires a location command.
pub fn special_power_is_location_target_only(power: &SpecialPowerType) -> bool {
    cpp_special_power(power).is_some_and(special_power::is_location_target_only)
}

/// Whether the command fires without either an object or a location target.
pub fn special_power_is_no_target(power: &SpecialPowerType) -> bool {
    cpp_special_power(power).is_some_and(special_power::allows_no_target)
}

/// Retail SpecialPower.ini template name used by ScriptEngine conditions.
///
/// C++ `SpecialPowerModule::aboutToDoSpecialPower` notifies
/// `getSpecialPowerTemplate()->getName()` (the INI block name).
pub fn special_power_ini_template_name(power: &SpecialPowerType) -> &'static str {
    use SpecialPowerType as P;
    match power {
        P::Airstrike => "SuperweaponA10ThunderboltMissileStrike",
        P::AirForceAirstrike => "AirF_SuperweaponA10ThunderboltMissileStrike",
        P::Artillery => "SuperweaponArtilleryBarrage",
        P::BattleshipBombardment => "SpecialPowerBattleshipBombardment",
        P::CarpetBomb => "SuperweaponCarpetBomb",
        P::AirForceCarpetBomb => "AirF_SuperweaponCarpetBomb",
        P::EarlyChinaCarpetBomb => "Early_SuperweaponChinaCarpetBomb",
        P::NukeChinaCarpetBomb => "Nuke_SuperweaponChinaCarpetBomb",
        P::DaisyCutter | P::FuelAirBomb => "SuperweaponDaisyCutter",
        P::AirForceDaisyCutter => "AirF_SuperweaponDaisyCutter",
        P::NuclearMissile | P::BaikonurRocket => "SuperweaponNeutronMissile",
        P::NukeNeutronMissile => "Nuke_SuperweaponNeutronMissile",
        P::SuperweaponNeutronMissile => "SupW_SuperweaponNeutronMissile",
        P::ParticleCannon => "SuperweaponParticleUplinkCannon",
        P::SuperweaponParticleCannon => "SupW_SuperweaponParticleUplinkCannon",
        P::LaserCannon => "Lazr_LaserCannon",
        P::ScudStorm => "SuperweaponScudStorm",
        P::AnthraxBomb => "SuperweaponAnthraxBomb",
        P::SpectreGunship => "SuperweaponSpectreGunship",
        P::AirForceSpectreGunship => "AirF_SuperweaponSpectreGunship",
        P::CruiseMissile => "SupW_CruiseMissile",
        P::SpySatellite => "SpecialPowerSpySatellite",
        P::RadarScan => "SpecialPowerRadarVanScan",
        P::SpyDrone => "SpecialPowerSpyDrone",
        P::CiaIntelligence => "SuperweaponCIAIntelligence",
        P::ClusterMines => "SuperweaponClusterMines",
        P::Paradrop => "SuperweaponParadropAmerica",
        P::Ambush => "SuperweaponRebelAmbush",
        P::EmpPulse => "SuperweaponEMPPulse",
        P::Frenzy => "SuperweaponFrenzy",
        P::EmergencyRepair => "SuperweaponEmergencyRepair",
        P::GpsScrambler => "SuperweaponGPSScrambler",
        P::StealthGpsScrambler => "Slth_SuperweaponGPSScrambler",
        P::LeafletDrop => "SuperweaponLeafletDrop",
        P::SneakAttack => "SuperweaponSneakAttack",
        P::CashHack => "SuperweaponCashHack",
        P::CrateDrop => "SuperweaponCrateDrop",
        P::NapalmStrike => "SuperweaponNapalmStrike",
        _ => cpp_special_power_name(power).unwrap_or("SPECIAL_INVALID"),
    }
}
