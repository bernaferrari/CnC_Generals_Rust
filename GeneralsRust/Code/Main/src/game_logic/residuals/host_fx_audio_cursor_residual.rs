//! Wave 88 residual peels: superweapon FX/OCL/particle/audio name tables + cursor tables.
//!
//! Freezes retail C++ / INI residual name tables used by host superweapon FX, OCL
//! spawn, particle systems, Miles audio events, radius cursors, and mouse cursors.
//!
//! Sources (retail ZH INI + C++):
//! - InGameUI.h `RadiusCursorType` + `TheRadiusCursorNames` (RADIUSCURSOR_COUNT)
//! - Mouse.h `MouseCursor` + Mouse.cpp `CursorININames` (ALLOW_SURRENDER/DEMORALIZE off)
//! - FXList.ini superweapon FX residual names
//! - ObjectCreationList.ini SUPERWEAPON_* / related OCL residual names
//! - ParticleSystem.ini PUC + superweapon particle residual names
//! - SoundEffects.ini special-power / superweapon AudioEvent residual names
//!
//! Fail-closed:
//! - Not full FXListExecutor particle spawn / OCL DeliverPayload flight matrix
//! - Not full ParticleSystemManager GPU / Miles positional playback residual
//! - Not full CursorManager / RadiusCursor GPU draw residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// 1. RadiusCursor residual name table (InGameUI.h TheRadiusCursorNames)
// ---------------------------------------------------------------------------

/// C++ `RADIUSCURSOR_COUNT` residual (keep-last sentinel).
pub const RADIUSCURSOR_COUNT: u32 = 30;

/// Ordered C++ `TheRadiusCursorNames` residual (excluding trailing NULL).
/// Discriminants 0..29 match RadiusCursorType enum order.
pub const RADIUS_CURSOR_NAME_LIST: &[&str] = &[
    "NONE",                     // 0  RADIUSCURSOR_NONE
    "ATTACK_DAMAGE_AREA",       // 1
    "ATTACK_SCATTER_AREA",      // 2
    "ATTACK_CONTINUE_AREA",     // 3
    "GUARD_AREA",               // 4
    "EMERGENCY_REPAIR",         // 5
    "FRIENDLY_SPECIALPOWER",    // 6 green
    "OFFENSIVE_SPECIALPOWER",   // 7 red
    "SUPERWEAPON_SCATTER_AREA", // 8 red
    "PARTICLECANNON",           // 9
    "A10STRIKE",                // 10
    "CARPETBOMB",               // 11
    "DAISYCUTTER",              // 12
    "PARADROP",                 // 13
    "SPYSATELLITE",             // 14
    "SPECTREGUNSHIP",           // 15
    "HELIX_NAPALM_BOMB",        // 16
    "NUCLEARMISSILE",           // 17
    "EMPPULSE",                 // 18
    "ARTILLERYBARRAGE",         // 19
    "NAPALMSTRIKE",             // 20
    "CLUSTERMINES",             // 21
    "SCUDSTORM",                // 22
    "ANTHRAXBOMB",              // 23
    "AMBUSH",                   // 24
    "RADAR",                    // 25
    "SPYDRONE",                 // 26
    "FRENZY",                   // 27
    "CLEARMINES",               // 28
    "AMBULANCE",                // 29
];

/// Lookup RadiusCursorType name index residual (case-insensitive).
pub fn radius_cursor_name_index(name: &str) -> Option<usize> {
    RADIUS_CURSOR_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: full RadiusCursor residual name table.
///
/// Fail-closed: not full RadiusCursor GPU draw / ControlBar overlay path.
pub fn honesty_radius_cursor_name_table_wave88() -> bool {
    RADIUSCURSOR_COUNT == 30
        && RADIUS_CURSOR_NAME_LIST.len() == 30
        && RADIUS_CURSOR_NAME_LIST[0] == "NONE"
        && RADIUS_CURSOR_NAME_LIST[9] == "PARTICLECANNON"
        && RADIUS_CURSOR_NAME_LIST[10] == "A10STRIKE"
        && RADIUS_CURSOR_NAME_LIST[11] == "CARPETBOMB"
        && RADIUS_CURSOR_NAME_LIST[12] == "DAISYCUTTER"
        && RADIUS_CURSOR_NAME_LIST[15] == "SPECTREGUNSHIP"
        && RADIUS_CURSOR_NAME_LIST[17] == "NUCLEARMISSILE"
        && RADIUS_CURSOR_NAME_LIST[19] == "ARTILLERYBARRAGE"
        && RADIUS_CURSOR_NAME_LIST[22] == "SCUDSTORM"
        && RADIUS_CURSOR_NAME_LIST[23] == "ANTHRAXBOMB"
        && RADIUS_CURSOR_NAME_LIST[29] == "AMBULANCE"
        && radius_cursor_name_index("PARTICLECANNON") == Some(9)
        && radius_cursor_name_index("DAISYCUTTER") == Some(12)
        && radius_cursor_name_index("NUCLEARMISSILE") == Some(17)
        && radius_cursor_name_index("SCUDSTORM") == Some(22)
        && radius_cursor_name_index("ANTHRAXBOMB") == Some(23)
        && radius_cursor_name_index("not_a_cursor").is_none()
        // Superweapon radius cluster contiguous residual (PARTICLECANNON..HELIX).
        && radius_cursor_name_index("PARTICLECANNON") == Some(9)
        && radius_cursor_name_index("HELIX_NAPALM_BOMB") == Some(16)
        && {
            let mut names: Vec<&str> = RADIUS_CURSOR_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 2. MouseCursor residual name table (Mouse.cpp CursorININames)
// ---------------------------------------------------------------------------

/// C++ `NUM_MOUSE_CURSORS` residual with ALLOW_SURRENDER / ALLOW_DEMORALIZE off.
///
/// CursorININames[0] = "None" maps to MouseCursor::NONE = 0; last live entry is
/// "ParticleUplinkCannon" at index 39; NUM_MOUSE_CURSORS = 40.
pub const NUM_MOUSE_CURSORS: u32 = 40;

/// Ordered C++ `CursorININames` residual (ALLOW_SURRENDER / ALLOW_DEMORALIZE off).
pub const MOUSE_CURSOR_INI_NAME_LIST: &[&str] = &[
    "None",                // 0  NONE
    "Normal",              // 1  NORMAL = FIRST_CURSOR
    "Arrow",               // 2
    "Scroll",              // 3
    "Target",              // 4  CROSS
    "Move",                // 5  MOVETO
    "AttackMove",          // 6
    "AttackObj",           // 7
    "ForceAttackObj",      // 8
    "ForceAttackGround",   // 9
    "Build",               // 10
    "InvalidBuild",        // 11
    "GenericInvalid",      // 12  SUPERWEAPON_INVALID_CURSOR residual
    "Select",              // 13
    "EnterFriendly",       // 14
    "EnterAggressive",     // 15
    "SetRallyPoint",       // 16
    "GetRepaired",         // 17
    "GetHealed",           // 18
    "DoRepair",            // 19
    "ResumeConstruction",  // 20
    "CaptureBuilding",     // 21
    "SnipeVehicle",        // 22
    "LaserGuidedMissiles", // 23
    "TankHunterTNTAttack", // 24
    "StabAttack",          // 25
    "PlaceRemoteCharge",   // 26
    "PlaceTimedCharge",    // 27
    "Defector",            // 28
    // ALLOW_DEMORALIZE off — no Demoralize entry
    "Dock", // 29
    // ALLOW_SURRENDER off — no PickUpPrisoner / ReturnToPrison
    "FireFlame", // 30
    // ALLOW_SURRENDER off — no FireTranqDarts / FireStunBullets
    "FireBomb",             // 31
    "PlaceBeacon",          // 32
    "DisguiseAsVehicle",    // 33
    "Waypoint",             // 34
    "OutRange",             // 35
    "StabAttackInvalid",    // 36
    "PlaceChargeInvalid",   // 37
    "Hack",                 // 38
    "ParticleUplinkCannon", // 39  PARTICLE_UPLINK_CANNON
];

/// Lookup MouseCursor INI name index residual (case-insensitive).
pub fn mouse_cursor_ini_name_index(name: &str) -> Option<usize> {
    MOUSE_CURSOR_INI_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: full MouseCursor residual name table.
///
/// Fail-closed: not full CursorManager GPU / hardware cursor path.
pub fn honesty_mouse_cursor_name_table_wave88() -> bool {
    NUM_MOUSE_CURSORS == 40
        && MOUSE_CURSOR_INI_NAME_LIST.len() == 40
        && MOUSE_CURSOR_INI_NAME_LIST[0] == "None"
        && MOUSE_CURSOR_INI_NAME_LIST[1] == "Normal"
        && MOUSE_CURSOR_INI_NAME_LIST[4] == "Target"
        && MOUSE_CURSOR_INI_NAME_LIST[12] == "GenericInvalid"
        && MOUSE_CURSOR_INI_NAME_LIST[21] == "CaptureBuilding"
        && MOUSE_CURSOR_INI_NAME_LIST[39] == "ParticleUplinkCannon"
        && mouse_cursor_ini_name_index("GenericInvalid") == Some(12)
        && mouse_cursor_ini_name_index("ParticleUplinkCannon") == Some(39)
        && mouse_cursor_ini_name_index("CaptureBuilding") == Some(21)
        && mouse_cursor_ini_name_index("Demoralize").is_none()
        && mouse_cursor_ini_name_index("PickUpPrisoner").is_none()
        // Superweapon InvalidCursor residual matches shared GenericInvalid.
        && mouse_cursor_ini_name_index("GenericInvalid")
            == Some(MOUSE_CURSOR_INI_NAME_LIST
                .iter()
                .position(|&n| n == "GenericInvalid")
                .unwrap())
        && {
            let mut names: Vec<&str> = MOUSE_CURSOR_INI_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 3. FXList residual name table (superweapon-related FXList.ini names)
// ---------------------------------------------------------------------------

/// Retail FXList residual names for host superweapon / structure death FX.
pub const SUPERWEAPON_FXLIST_NAME_TABLE: &[&str] = &[
    // Daisy Cutter residual
    "FX_DaisyCutterExplode",
    "FX_DaisyCutterIgnite",
    "FX_DaisyCutterFinalExplosion",
    // A10 residual
    "FX_A10ThunderboltMissileIgnition",
    "FX_A10ThunderboltMissileExplosion",
    // Scud residual
    "FX_ScudStormIgnition",
    "FX_ScudLauncherIgnition",
    "FX_ScudLauncherExplosionOneFinal",
    "FX_ScudLauncherDamageTransition",
    // Particle Uplink residual
    "FX_ParticleUplinkCannon_BeamHitsGround",
    "FX_ParticleUplinkCannon_BeamLaunchIteration",
    "FX_ParticleUplinkDeathInitial",
    // Nuke residual
    "FX_Nuke",
    "FX_NukeGLA",
    "FX_BaikonurNuke",
    "FX_NukeCannonDamageTransition",
    // Anthrax residual
    "FX_AnthraxBomb",
    "FX_AnthraxGammaBomb",
    "FX_AnthraxPoolDie",
    // Carpet / Artillery residual
    "FX_CarpetBomb",
    "FX_ArtilleryBarrage",
    // Spectre residual
    "FX_SpectreHowitzerExplosion",
    "FX_SpectreGattlingImpacts",
    "FX_SpectreGunshipExplosionLight",
    // Structure death residual (shared by PUC Instant/SlowDeath FINAL)
    "FX_StructureLargeDeath",
    "FX_StructureMediumDeath",
    "FX_StructureSmallDeath",
    "FX_StructureTinyDeath",
];

/// Lookup superweapon FXList residual name presence.
pub fn superweapon_fxlist_name_known(name: &str) -> bool {
    SUPERWEAPON_FXLIST_NAME_TABLE
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: superweapon FXList residual name table.
///
/// Fail-closed: not full FXListExecutor particle/sound/decal application.
pub fn honesty_superweapon_fxlist_name_table_wave88() -> bool {
    SUPERWEAPON_FXLIST_NAME_TABLE.len() == 28
        && SUPERWEAPON_FXLIST_NAME_TABLE[0] == "FX_DaisyCutterExplode"
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_DaisyCutterFinalExplosion")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_A10ThunderboltMissileExplosion")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_ScudStormIgnition")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_ParticleUplinkCannon_BeamHitsGround")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_ParticleUplinkDeathInitial")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_Nuke")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_AnthraxBomb")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_CarpetBomb")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_ArtilleryBarrage")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_SpectreHowitzerExplosion")
        && SUPERWEAPON_FXLIST_NAME_TABLE.contains(&"FX_StructureMediumDeath")
        && superweapon_fxlist_name_known("FX_Nuke")
        && superweapon_fxlist_name_known("FX_ParticleUplinkDeathInitial")
        && !superweapon_fxlist_name_known("FX_NotARealSuperweapon")
        && {
            let mut names: Vec<&str> = SUPERWEAPON_FXLIST_NAME_TABLE.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 4. ObjectCreationList residual names (superweapon OCL table)
// ---------------------------------------------------------------------------

/// Retail ObjectCreationList residual names for host superweapons + related OCL.
pub const SUPERWEAPON_OCL_NAME_TABLE: &[&str] = &[
    // Core host superweapon OCL residual
    "SUPERWEAPON_DaisyCutter",
    "SUPERWEAPON_MOAB",
    "SUPERWEAPON_NeutronMissile",
    "SUPERWEAPON_ScudStorm",
    "SUPERWEAPON_ArtilleryBarrage1",
    "SUPERWEAPON_ArtilleryBarrage2",
    "SUPERWEAPON_ArtilleryBarrage3",
    "SUPERWEAPON_A10ThunderboltMissileStrike1",
    "SUPERWEAPON_A10ThunderboltMissileStrike2",
    "SUPERWEAPON_A10ThunderboltMissileStrike3",
    "SUPERWEAPON_AnthraxBomb",
    "SUPERWEAPON_AnthraxBombGamma",
    "SUPERWEAPON_CarpetBomb",
    "AirF_SUPERWEAPON_CarpetBomb",
    "SUPERWEAPON_ChinaCarpetBomb",
    "Nuke_SUPERWEAPON_ChinaCarpetBomb",
    "SUPERWEAPON_CruiseMissile",
    "SupW_SUPERWEAPON_NeutronMissile",
    // Related field / death OCL residual (special-power aftermath)
    "OCL_NukeRadiationField",
    "OCL_PoisonFieldAnthraxBomb",
    "OCL_PoisonFieldAnthraxGammaBomb",
    "OCL_PoisonFieldLarge",
    "OCL_ParticleUplinkDeathFinal",
    "OCL_ABPowerPlantExplode",
    "OCL_SDILinkLasers",
    "OCL_GenericMissileDisintegrate",
    // Support special-power OCL residual (host-adjacent)
    "SUPERWEAPON_Paradrop1",
    "SUPERWEAPON_ClusterMines",
    "SUPERWEAPON_EMPPulse",
    "SUPERWEAPON_SpySatellite",
    "OCL_SpectreDeathFinalBlowUp",
];

/// Lookup superweapon OCL residual name presence.
pub fn superweapon_ocl_name_known(name: &str) -> bool {
    SUPERWEAPON_OCL_NAME_TABLE
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: superweapon ObjectCreationList residual name table.
///
/// Fail-closed: not full OCL DeliverPayload / FireWeapon / Attack spawn matrix.
pub fn honesty_superweapon_ocl_name_table_wave88() -> bool {
    SUPERWEAPON_OCL_NAME_TABLE.len() == 31
        && SUPERWEAPON_OCL_NAME_TABLE[0] == "SUPERWEAPON_DaisyCutter"
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_NeutronMissile")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_ScudStorm")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_ArtilleryBarrage1")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_ArtilleryBarrage3")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_A10ThunderboltMissileStrike1")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_A10ThunderboltMissileStrike3")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_AnthraxBomb")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_CarpetBomb")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"AirF_SUPERWEAPON_CarpetBomb")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_ChinaCarpetBomb")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"SUPERWEAPON_CruiseMissile")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"OCL_NukeRadiationField")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"OCL_PoisonFieldAnthraxBomb")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"OCL_ParticleUplinkDeathFinal")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"OCL_SDILinkLasers")
        && SUPERWEAPON_OCL_NAME_TABLE.contains(&"OCL_ABPowerPlantExplode")
        && superweapon_ocl_name_known("SUPERWEAPON_DaisyCutter")
        && superweapon_ocl_name_known("SUPERWEAPON_CruiseMissile")
        && !superweapon_ocl_name_known("SUPERWEAPON_NotReal")
        // A10 / Artillery science tiers residual present (1/2/3).
        && SUPERWEAPON_OCL_NAME_TABLE
            .iter()
            .filter(|n| n.starts_with("SUPERWEAPON_A10"))
            .count()
            == 3
        && SUPERWEAPON_OCL_NAME_TABLE
            .iter()
            .filter(|n| n.starts_with("SUPERWEAPON_ArtilleryBarrage"))
            .count()
            == 3
        && {
            let mut names: Vec<&str> = SUPERWEAPON_OCL_NAME_TABLE.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 5. Particle system residual name table expand (superweapon + PUC)
// ---------------------------------------------------------------------------

/// Retail ParticleSystem residual names for PUC + host superweapon FX.
///
/// Expands Wave 81 outer-node flare residual with full PUC system set plus
/// Daisy / Nuke / Scud / Anthrax / Carpet / Artillery / Spectre anchors.
pub const SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE: &[&str] = &[
    // Particle Uplink Cannon residual (ParticleSystem.ini + WeaponObjects laser objs)
    "ParticleUplinkCannon_OuterNodeLightFlare",
    "ParticleUplinkCannon_OuterNodeMediumFlare",
    "ParticleUplinkCannon_OuterNodeIntenseFlare",
    "ParticleUplinkCannon_InnerConnectorMediumFlare",
    "ParticleUplinkCannon_InnerConnectorIntenseFlare",
    "ParticleUplinkCannon_LaserBaseReadyToFire",
    "ParticleUplinkCannon_LaunchFlare",
    "ParticleUplinkCannon_Fire",
    "ParticleUplinkCannon_Sparks",
    "ParticleUplinkCannon_Magma",
    "ParticleUplinkCannon_Shockwave",
    // Laser object residual names (WeaponObjects.ini; beam attach residual)
    "ParticleUplinkCannon_MediumConnectorLaser",
    "ParticleUplinkCannon_IntenseConnectorLaser",
    "ParticleUplinkCannon_OrbitalLaser",
    // SupW general PUC residual variants
    "SupW_ParticleUplinkCannon_OuterNodeLightFlare",
    "SupW_ParticleUplinkCannon_OuterNodeMediumFlare",
    "SupW_ParticleUplinkCannon_OuterNodeIntenseFlare",
    "SupW_ParticleUplinkCannon_LaunchFlare",
    // Daisy residual
    "DaisyExplosion",
    "DaisyExplosionGasSpray",
    "DaisyExplosionSmoke",
    // Nuke residual
    "NukeMushroomRing",
    "NukeMushroomStem",
    "NukeShockwave",
    "NukeShockwaveInverted",
    "NukeGLAMushroomRing",
    "NukeGLAMushroomStem",
    "NukeGLAFlare",
    // Scud residual
    "ScudMissleExplosion",
    "ScudMissleSmoke",
    "ScudMissleLenzFlare",
    "ScudMissleLauncherToxinSmoke",
    // Anthrax residual
    "AnthraxBombExplosion",
    "AnthraxBombLenzflare",
    "AnthraxGammaBombExplosion",
    // Carpet / Artillery residual
    "CarpetBombWave",
    "CarpetBombExplosionSmoke",
    "ArtilleryBarrageDust",
    "ArtilleryBarrageShockwave",
    "ArtilleryBarrageTrail",
    // Spectre residual
    "SpectreHowitzerExplosion",
    "SpectreGunshipExplosionLight",
    "SpectreAfterburnerTrail",
    "SpectreContrail",
    "SpectreEngineFlare",
];

/// Lookup superweapon particle system residual name presence.
pub fn superweapon_particle_system_name_known(name: &str) -> bool {
    SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: expanded superweapon particle system residual name table.
///
/// Fail-closed: not full ParticleSystemManager spawn / W3D bone-world FX attach.
pub fn honesty_superweapon_particle_name_table_wave88() -> bool {
    SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.len() == 45
        // Wave 81 outer-node flare residual still present.
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_OuterNodeLightFlare")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_OuterNodeMediumFlare")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_OuterNodeIntenseFlare")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_InnerConnectorMediumFlare")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_InnerConnectorIntenseFlare")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_LaserBaseReadyToFire")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_OrbitalLaser")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_MediumConnectorLaser")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .contains(&"ParticleUplinkCannon_IntenseConnectorLaser")
        // Beam ground-hit particle residual (via FX_ParticleUplinkCannon_BeamHitsGround).
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ParticleUplinkCannon_Magma")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ParticleUplinkCannon_Sparks")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ParticleUplinkCannon_Shockwave")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ParticleUplinkCannon_Fire")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ParticleUplinkCannon_LaunchFlare")
        // Non-PUC superweapon particle residual.
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"DaisyExplosion")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"NukeMushroomRing")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"NukeShockwave")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ScudMissleExplosion")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"AnthraxBombExplosion")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"CarpetBombWave")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"ArtilleryBarrageDust")
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.contains(&"SpectreHowitzerExplosion")
        && superweapon_particle_system_name_known("ParticleUplinkCannon_OrbitalLaser")
        && superweapon_particle_system_name_known("DaisyExplosion")
        && !superweapon_particle_system_name_known("ParticleSystem_NotReal")
        // Outer-node intensity residual cluster size.
        && SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE
            .iter()
            .filter(|n| n.contains("OuterNode") && n.contains("Flare") && !n.starts_with("SupW_"))
            .count()
            == 3
        && {
            let mut names: Vec<&str> = SUPERWEAPON_PARTICLE_SYSTEM_NAME_TABLE.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 6. Audio event residual name table expand (superweapon Miles events)
// ---------------------------------------------------------------------------

/// Retail SoundEffects.ini AudioEvent residual names for host superweapons.
///
/// Expands Wave 77 InitiateSound residual (ScudStormInitiated / FireArtillery /
/// AirRaidSiren) with full strike / building / explosion audio residual anchors.
pub const SUPERWEAPON_AUDIO_EVENT_NAME_TABLE: &[&str] = &[
    // Wave 77 initiate residual (SpecialPower.ini Miles names)
    "ScudStormInitiated",
    "FireArtilleryCannonSound",
    "AirRaidSiren",
    // Scud residual
    "ScudStormLaunch",
    "ScudStormSelect",
    // Daisy residual
    "DaisyCutterWeapon",
    "DaisyCutterGas",
    "DaisyCutterIgnite",
    "ExplosionDaisyCutter",
    // A10 residual
    "A10ThunderboltMissileWeaponSound",
    "A10ThunderboltAmbientLoop",
    "A10ThunderboltDive",
    "ExplosionA10ThunderboltMissile",
    // Neutron / Nuke residual
    "NeutronMissileRelease",
    "BuildingNeutronMissileOpen",
    "BuildingNeutronMissileLaunch",
    "BuildingNeutronMissileHiss",
    "ExplosionNeutron",
    "ExplosionMiniNuke",
    // Particle Uplink residual loops
    "ParticleUplinkCannon_PowerupSoundLoop",
    "ParticleUplinkCannon_UnpackToIdleSoundLoop",
    "ParticleUplinkCannon_FiringToPackSoundLoop",
    "ParticleUplinkCannon_GroundAnnihilationSoundLoop",
    // Anthrax residual
    "ExplosionAnthraxBomb",
    "AnthraxPoolAmbientLoop",
    "AnthraxPoolDie",
    // Carpet / Artillery residual
    "ExplosionCarpetBomb",
    "ArtilleryBarrageIncomingWhistle",
    "ExplosionArtilleryBarrage",
    // Spectre residual
    "SpectreGunshipAmbientLoop",
    "SpectreGunshipAfterburnerLoop",
    "SpectreGunshipGattlingWeapon",
    "SpectreHowitzerWeapon",
    // Cruise residual (cinematic host anchor)
    "Cin_CruiseMissileAmbientLoop",
];

/// Lookup superweapon audio event residual name presence.
pub fn superweapon_audio_event_name_known(name: &str) -> bool {
    SUPERWEAPON_AUDIO_EVENT_NAME_TABLE
        .iter()
        .any(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 88 honesty: expanded superweapon audio event residual name table.
///
/// Fail-closed: not full Miles positional playback / AudioEvent INI load.
pub fn honesty_superweapon_audio_event_name_table_wave88() -> bool {
    SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.len() == 34
        // Wave 77 initiate residual still present.
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ScudStormInitiated")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"FireArtilleryCannonSound")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"AirRaidSiren")
        // Expanded strike / building residual.
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ScudStormLaunch")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"DaisyCutterWeapon")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ExplosionDaisyCutter")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"A10ThunderboltDive")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ExplosionA10ThunderboltMissile")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"BuildingNeutronMissileLaunch")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"NeutronMissileRelease")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE
            .contains(&"ParticleUplinkCannon_GroundAnnihilationSoundLoop")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE
            .contains(&"ParticleUplinkCannon_PowerupSoundLoop")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ExplosionAnthraxBomb")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ExplosionCarpetBomb")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"ExplosionArtilleryBarrage")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"SpectreGunshipAmbientLoop")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"SpectreHowitzerWeapon")
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.contains(&"Cin_CruiseMissileAmbientLoop")
        && superweapon_audio_event_name_known("ScudStormInitiated")
        && superweapon_audio_event_name_known("AirRaidSiren")
        && !superweapon_audio_event_name_known("AudioEvent_NotReal")
        // PUC loop residual cluster size.
        && SUPERWEAPON_AUDIO_EVENT_NAME_TABLE
            .iter()
            .filter(|n| n.starts_with("ParticleUplinkCannon_"))
            .count()
            == 4
        && {
            let mut names: Vec<&str> = SUPERWEAPON_AUDIO_EVENT_NAME_TABLE.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 88 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 88 honesty: all FX/OCL/particle/audio/cursor residual packs.
pub fn honesty_fx_audio_cursor_residual_pack_wave88() -> bool {
    honesty_radius_cursor_name_table_wave88()
        && honesty_mouse_cursor_name_table_wave88()
        && honesty_superweapon_fxlist_name_table_wave88()
        && honesty_superweapon_ocl_name_table_wave88()
        && honesty_superweapon_particle_name_table_wave88()
        && honesty_superweapon_audio_event_name_table_wave88()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_cursor_name_table_wave88_honesty() {
        assert!(honesty_radius_cursor_name_table_wave88());
        assert_eq!(radius_cursor_name_index("DAISYCUTTER"), Some(12));
        assert_eq!(radius_cursor_name_index("SCUDSTORM"), Some(22));
    }

    #[test]
    fn mouse_cursor_name_table_wave88_honesty() {
        assert!(honesty_mouse_cursor_name_table_wave88());
        assert_eq!(
            mouse_cursor_ini_name_index("ParticleUplinkCannon"),
            Some(39)
        );
        assert_eq!(mouse_cursor_ini_name_index("GenericInvalid"), Some(12));
    }

    #[test]
    fn superweapon_fxlist_name_table_wave88_honesty() {
        assert!(honesty_superweapon_fxlist_name_table_wave88());
        assert!(superweapon_fxlist_name_known("FX_Nuke"));
        assert!(superweapon_fxlist_name_known("FX_StructureMediumDeath"));
    }

    #[test]
    fn superweapon_ocl_name_table_wave88_honesty() {
        assert!(honesty_superweapon_ocl_name_table_wave88());
        assert!(superweapon_ocl_name_known("SUPERWEAPON_DaisyCutter"));
        assert!(superweapon_ocl_name_known("OCL_ParticleUplinkDeathFinal"));
    }

    #[test]
    fn superweapon_particle_name_table_wave88_honesty() {
        assert!(honesty_superweapon_particle_name_table_wave88());
        assert!(superweapon_particle_system_name_known(
            "ParticleUplinkCannon_OuterNodeLightFlare"
        ));
        assert!(superweapon_particle_system_name_known("DaisyExplosion"));
    }

    #[test]
    fn superweapon_audio_event_name_table_wave88_honesty() {
        assert!(honesty_superweapon_audio_event_name_table_wave88());
        assert!(superweapon_audio_event_name_known("ScudStormInitiated"));
        assert!(superweapon_audio_event_name_known(
            "ParticleUplinkCannon_GroundAnnihilationSoundLoop"
        ));
    }

    #[test]
    fn fx_audio_cursor_residual_pack_wave88_honesty() {
        assert!(honesty_fx_audio_cursor_residual_pack_wave88());
    }
}
