//! Residual-pack honesty helpers (fail-closed vs full retail Object stacks).
use super::types::*;
use super::*;
/// Honesty: SupW ParticleUplink magenta OuterColor residual vs normal blue.
pub fn honesty_particle_supw_outer_color() -> bool {
    let (r, g, b, a) = PARTICLE_SUPW_CONNECTOR_OUTER_COLOR;
    let (nr, ng, nb, na) = PARTICLE_CONNECTOR_OUTER_COLOR;
    (r - 1.0).abs() < 0.01
        && (g - 0.0).abs() < 0.01
        && (b - 1.0).abs() < 0.01
        && (a - 150.0 / 255.0).abs() < 0.01
        && (nr - 0.0).abs() < 0.01
        && (ng - 0.0).abs() < 0.01
        && (nb - 1.0).abs() < 0.01
        && (na - a).abs() < 0.01
        && PARTICLE_SUPW_CONNECTOR_OUTER_COLOR == PARTICLE_SUPW_ORBITAL_OUTER_COLOR
        && PARTICLE_SUPW_MEDIUM_CONNECTOR.contains("SupW_")
        && PARTICLE_SUPW_INTENSE_CONNECTOR.contains("SupW_")
        && PARTICLE_SUPW_ORBITAL_LASER.contains("SupW_")
        && PARTICLE_CONNECTOR_MEDIUM_LASER.starts_with("ParticleUplink")
}

/// Honesty: PUC sound residual pack name + BeamLaunchFX / GroundHitFX constants.
///
/// Fail-closed: not full Miles audio event playback / 3D positional loop stop.
pub fn honesty_particle_sound_loops() -> bool {
    PARTICLE_POWERUP_AUDIO == "ParticleUplinkCannon_PowerupSoundLoop"
        && PARTICLE_UNPACK_AUDIO == "ParticleUplinkCannon_UnpackToIdleSoundLoop"
        && PARTICLE_FIRING_TO_PACK_AUDIO == "ParticleUplinkCannon_FiringToPackSoundLoop"
        && PARTICLE_BEAM_AUDIO == "ParticleUplinkCannon_GroundAnnihilationSoundLoop"
        && PARTICLE_BEAM_LAUNCH_FX == "FX_ParticleUplinkCannon_BeamLaunchIteration"
        && PARTICLE_LAUNCH_FX_INTERVAL_FRAMES == 30
        && PARTICLE_GROUND_HIT_FX == "FX_ParticleUplinkCannon_BeamHitsGround"
}

/// Honesty: Scorch residual pack constants (scalar / swath / manual drive).
///
/// Fail-closed: not full TheGameClient::addScorch GPU decal / partition shroud.
pub fn honesty_particle_scorch_pack() -> bool {
    PARTICLE_TOTAL_SCORCH_MARKS == 20
        && (PARTICLE_SCORCH_MARK_SCALAR - 2.4).abs() < 0.01
        && (PARTICLE_SWATH_OF_DEATH_DISTANCE - 200.0).abs() < 0.01
        && (PARTICLE_SWATH_OF_DEATH_AMPLITUDE - 50.0).abs() < 0.01
        && (PARTICLE_MANUAL_DRIVING_SPEED - 20.0).abs() < 0.01
        && (PARTICLE_MANUAL_FAST_DRIVING_SPEED - 40.0).abs() < 0.01
        && PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES == 15
        && PARTICLE_GROUND_HIT_FX.contains("BeamHitsGround")
}

/// Honesty: SupW PointDefenseDroneLaserBeam LifetimeUpdate residual (95 ms → 3).
///
/// Fail-closed: not full LifetimeUpdate destroyObject / ThingFactory laser Object.
pub fn honesty_point_defense_laser_lifetime() -> bool {
    POINT_DEFENSE_DRONE_LASER_BEAM == "SupW_PointDefenseDroneLaserBeam"
        && POINT_DEFENSE_LASER_BEAM == "PointDefenseLaserBeam"
        && POINT_DEFENSE_LASER_MIN_LIFETIME_MS == 95
        && POINT_DEFENSE_LASER_MAX_LIFETIME_MS == 95
        && POINT_DEFENSE_LASER_LIFETIME_FRAMES == 3
        && duration_ms_to_logic_frames(95) == 3
        && lifetime_update_fixed_frames(
            POINT_DEFENSE_LASER_MIN_LIFETIME_MS,
            POINT_DEFENSE_LASER_MAX_LIFETIME_MS,
        ) == 3
}

/// Honesty: PUC building FlammableUpdate residual pack.
///
/// Fail-closed: not full aflame object status bit / live damage-over-time module.
pub fn honesty_particle_uplink_flammable() -> bool {
    PARTICLE_UPLINK_AFLAME_DURATION_MS == 5000
        && PARTICLE_UPLINK_AFLAME_DURATION_FRAMES == 150
        && (PARTICLE_UPLINK_AFLAME_DAMAGE_AMOUNT - 5.0).abs() < 0.01
        && PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_MS == 500
        && PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_FRAMES == 15
        && duration_ms_to_logic_frames(5000) == 150
        && duration_ms_to_logic_frames(500) == 15
}

/// Honesty: PUC OuterNodes flare particle system residual pack.
///
/// Retail FactionBuilding.ini ParticleUplinkCannonUpdate:
/// OuterNodesLight/Medium/Intense + LaserBaseLightFlare + Connector laser names.
/// Fail-closed: not full ParticleSystemManager spawn / W3D bone-world FX attach.
pub fn honesty_particle_outer_node_flare_pack() -> bool {
    PARTICLE_OUTER_NODE_LIGHT_FLARE == "ParticleUplinkCannon_OuterNodeLightFlare"
        && PARTICLE_OUTER_NODE_MEDIUM_FLARE == "ParticleUplinkCannon_OuterNodeMediumFlare"
        && PARTICLE_OUTER_NODE_INTENSE_FLARE == "ParticleUplinkCannon_OuterNodeIntenseFlare"
        && PARTICLE_LASER_BASE_READY_FLARE == "ParticleUplinkCannon_LaserBaseReadyToFire"
        && PARTICLE_CONNECTOR_MEDIUM_LASER == "ParticleUplinkCannon_MediumConnectorLaser"
        && PARTICLE_CONNECTOR_INTENSE_LASER == "ParticleUplinkCannon_IntenseConnectorLaser"
        && PARTICLE_ORBITAL_LASER_NAME == "ParticleUplinkCannon_OrbitalLaser"
        && PARTICLE_OUTER_EFFECT_NUM_BONES == 5
        && ParticleIntensity::Light.outer_flare_name() == PARTICLE_OUTER_NODE_LIGHT_FLARE
        && ParticleIntensity::Medium.outer_flare_name() == PARTICLE_OUTER_NODE_MEDIUM_FLARE
        && ParticleIntensity::Intense.outer_flare_name() == PARTICLE_OUTER_NODE_INTENSE_FLARE
        && ParticleIntensity::None.outer_flare_name().is_empty()
        && ParticleIntensity::Medium.connector_laser_name() == PARTICLE_CONNECTOR_MEDIUM_LASER
        && ParticleIntensity::Intense.connector_laser_name() == PARTICLE_CONNECTOR_INTENSE_LASER
        && ParticleIntensity::Light.connector_laser_name().is_empty()
}

/// Wave 81 residual honesty: PUC outer-node flare particle system name tables deepen.
///
/// Extends Wave 50 flare pack with structured intensity/name residual tables and
/// commented ConnectorMedium/Intense flare residual names from FactionBuilding.ini.
/// Fail-closed: not full ParticleSystemManager spawn / W3D bone-world FX attach.
pub fn honesty_particle_outer_node_flare_name_table_wave81() -> bool {
    honesty_particle_outer_node_flare_pack()
        && PARTICLE_OUTER_NODE_FLARE_NAME_TABLE.len() == 3
        && PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[0]
            == ("Light", PARTICLE_OUTER_NODE_LIGHT_FLARE)
        && PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[1]
            == ("Medium", PARTICLE_OUTER_NODE_MEDIUM_FLARE)
        && PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[2]
            == ("Intense", PARTICLE_OUTER_NODE_INTENSE_FLARE)
        && PARTICLE_UPLINK_FLARE_LASER_NAME_TABLE.len() == 6
        && PARTICLE_UPLINK_FLARE_LASER_NAME_TABLE
            .iter()
            .any(|(k, v)| *k == "LaserBaseReady" && *v == PARTICLE_LASER_BASE_READY_FLARE)
        && PARTICLE_UPLINK_FLARE_LASER_NAME_TABLE
            .iter()
            .any(|(k, v)| *k == "OrbitalLaser" && *v == PARTICLE_ORBITAL_LASER_NAME)
        && PARTICLE_CONNECTOR_MEDIUM_FLARE
            == "ParticleUplinkCannon_InnerConnectorMediumFlare"
        && PARTICLE_CONNECTOR_INTENSE_FLARE
            == "ParticleUplinkCannon_InnerConnectorIntenseFlare"
        // Intensity enum residual maps through the name table.
        && ParticleIntensity::Light.outer_flare_name()
            == PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[0].1
        && ParticleIntensity::Medium.outer_flare_name()
            == PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[1].1
        && ParticleIntensity::Intense.outer_flare_name()
            == PARTICLE_OUTER_NODE_FLARE_NAME_TABLE[2].1
        && PARTICLE_OUTER_EFFECT_BONE_NAME == "FX"
        && particle_outer_node_bone_name(0) == "FX01"
        && particle_outer_node_bone_name(4) == "FX05"
}

/// Honesty: PUC SlowDeath / InstantDeath residual pack (FactionBuilding.ini).
///
/// Complete building: SlowDeath ExemptStatus UNDER_CONSTRUCTION, DestructionDelay
/// 2000 ms → 60 frames, INITIAL FX/OCL then FINAL FX/OCL. Under construction:
/// InstantDeath RequiredStatus UNDER_CONSTRUCTION + OCL_ABPowerPlantExplode.
/// Fail-closed: not full SlowDeathBehavior multi-stage / Object die matrix.
pub fn honesty_particle_uplink_death_pack() -> bool {
    PARTICLE_UPLINK_SLOW_DEATH_EXEMPT_STATUS == "UNDER_CONSTRUCTION"
        && PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_MS == 2000
        && PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_FRAMES == 60
        && duration_ms_to_logic_frames(2000) == 60
        && PARTICLE_UPLINK_SLOW_DEATH_FX_INITIAL == "FX_ParticleUplinkDeathInitial"
        && PARTICLE_UPLINK_SLOW_DEATH_OCL_INITIAL == "OCL_SDILinkLasers"
        && PARTICLE_UPLINK_SLOW_DEATH_FX_FINAL == "FX_StructureMediumDeath"
        && PARTICLE_UPLINK_SLOW_DEATH_OCL_FINAL == "OCL_ParticleUplinkDeathFinal"
        && PARTICLE_UPLINK_INSTANT_DEATH_REQUIRED_STATUS == "UNDER_CONSTRUCTION"
        && PARTICLE_UPLINK_INSTANT_DEATH_OCL == "OCL_ABPowerPlantExplode"
        && PARTICLE_UPLINK_INSTANT_DEATH_FX == "FX_StructureMediumDeath"
        // InstantDeath and SlowDeath FINAL share StructureMediumDeath FX residual.
        && PARTICLE_UPLINK_INSTANT_DEATH_FX == PARTICLE_UPLINK_SLOW_DEATH_FX_FINAL
}

/// Honesty: SpectreGattlingGun ContinuousFire WeaponBonus ROF residual constants.
///
/// Retail WeaponBonus: CONTINUOUS_FIRE_MEAN RATE_OF_FIRE **200%**,
/// CONTINUOUS_FIRE_FAST RATE_OF_FIRE **300%**; ContinuousFireOne=1 / Two=2.
/// Fail-closed: not full FiringTracker WeaponBonusConditionFlags combat matrix.
pub fn honesty_gattling_weapon_bonus_rof() -> bool {
    SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE == 1
        && SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO == 2
        && (SPECTRE_GATTLING_ROF_MEAN - 2.0).abs() < 0.01
        && (SPECTRE_GATTLING_ROF_FAST - 3.0).abs() < 0.01
        && SpectreGattlingFireStage::Normal.rate_of_fire() == 1.0
        && (SpectreGattlingFireStage::Mean.rate_of_fire() - 2.0).abs() < 0.01
        && (SpectreGattlingFireStage::Fast.rate_of_fire() - 3.0).abs() < 0.01
        // Base 3 frames / 200% → floor(1.5)=1; / 300% → floor(1.0)=1.
        && spectre_gattling_interval_frames(0) == 3
        && spectre_gattling_interval_frames(1) == 3
        && spectre_gattling_interval_frames(2) == 1
        && spectre_gattling_interval_frames(3) == 1
}

/// Honesty: CarpetBomb residual pack deepen (Wave 56).
///
/// DropDelay/DropVariance/PreferredHeight/bomb-count tiers/FireFX/DeliveryDistance.
/// Fail-closed: not full AmericaJetB52 pathfinder / DeliverPayloadAIUpdate flight.
pub fn honesty_carpet_bomb_residual_pack() -> bool {
    CARPET_BOMB_COUNT == 15
        && CARPET_BOMB_COUNT_AIRF == 12
        && CARPET_BOMB_COUNT_CHINA == 10
        && CARPET_BOMB_DROP_DELAY_FRAMES == 9
        && CARPET_BOMB_DROP_DELAY_MS == 300
        && duration_ms_to_logic_frames(CARPET_BOMB_DROP_DELAY_MS) == CARPET_BOMB_DROP_DELAY_FRAMES
        && CARPET_BOMB_DROP_DELAY_AIRF_MS == 130
        && CARPET_BOMB_DROP_DELAY_AIRF_FRAMES == 4
        && duration_ms_to_logic_frames(CARPET_BOMB_DROP_DELAY_AIRF_MS)
            == CARPET_BOMB_DROP_DELAY_AIRF_FRAMES
        && (CARPET_BOMB_DROP_VARIANCE_X - 30.0).abs() < 0.01
        && (CARPET_BOMB_DROP_VARIANCE_Y - 40.0).abs() < 0.01
        && (CARPET_BOMB_DROP_VARIANCE_Z - 0.0).abs() < 0.01
        && (CARPET_BOMB_PREFERRED_HEIGHT - 100.0).abs() < 0.01
        && (CARPET_BOMB_DELIVERY_DISTANCE - 400.0).abs() < 0.01
        && (CARPET_BOMB_DELIVERY_DISTANCE_AIRF - 500.0).abs() < 0.01
        && (CARPET_BOMB_DELIVERY_DISTANCE_CHINA - 350.0).abs() < 0.01
        && CARPET_BOMB_TRANSPORT == "AmericaJetB52"
        && CARPET_BOMB_TRANSPORT_AIRF == "AirF_AmericaJetB3"
        && CARPET_BOMB_TRANSPORT_CHINA == "ChinaJetCarpetBomber"
        && CARPET_BOMB_FIRE_FX == "FX_CarpetBomb"
        && CARPET_BOMB_EXPLOSION_AUDIO == "ExplosionCarpetBomb"
        && CARPET_BOMB_WEAPON_NAME == "CarpetBombWeapon"
        && CARPET_BOMB_PAYLOAD_OBJECT == "CarpetBomb"
        && CARPET_BOMB_LOCOMOTOR == "B52Locomotor"
        && (CARPET_BOMB_LOCOMOTOR_SPEED - 125.0).abs() < 0.01
        && (CARPET_BOMB_DROP_OFFSET_Z - (-2.0)).abs() < 0.01
        && CarpetBombFactionTier::America.bomb_count() == 15
        && CarpetBombFactionTier::AirForce.bomb_count() == 12
        && CarpetBombFactionTier::China.bomb_count() == 10
        && CarpetBombFactionTier::America.drop_delay_frames() == 9
        && CarpetBombFactionTier::AirForce.drop_delay_frames() == 4
        && CarpetBombFactionTier::China.drop_delay_frames() == 9
        && (CarpetBombFactionTier::America.line_length() - 14.0 * CARPET_BOMB_SPACING).abs() < 0.01
        && (CarpetBombFactionTier::AirForce.line_length() - 11.0 * CARPET_BOMB_SPACING).abs() < 0.01
        && (CarpetBombFactionTier::China.line_length() - 9.0 * CARPET_BOMB_SPACING).abs() < 0.01
        && CarpetBombFactionTier::from_science_or_ocl_name("AirF_SUPERWEAPON_CarpetBomb")
            == Some(CarpetBombFactionTier::AirForce)
        && CarpetBombFactionTier::from_science_or_ocl_name("SUPERWEAPON_ChinaCarpetBomb")
            == Some(CarpetBombFactionTier::China)
        && CarpetBombFactionTier::from_science_or_ocl_name("SUPERWEAPON_CarpetBomb")
            == Some(CarpetBombFactionTier::America)
}

/// Honesty: CruiseMissile / MOAB residual pack deepen (Wave 56).
///
/// Loft SpecialSpeedTime/DistanceToTravelBeforeTurning/HeightDie + projectile
/// object + MOAB damage/radius/ShockWave/FireFX residual.
/// Fail-closed: not full NeutronMissileUpdate door/loft physics Object.
pub fn honesty_cruise_missile_residual_pack() -> bool {
    CRUISE_MISSILE_PROJECTILE_OBJECT == "CruiseMissile"
        && CRUISE_MISSILE_WEAPON_NAME == "CruiseMissileWeapon"
        && CRUISE_MISSILE_OCL == "SUPERWEAPON_CruiseMissile"
        && CRUISE_MISSILE_DEATH_WEAPON == "MOABDetonationWeapon"
        && CRUISE_MISSILE_MOAB_FIRE_FX == "WeaponFX_MOAB_Blast"
        && CRUISE_MISSILE_LAUNCH_FIRE_FX == "WeaponFX_NeutronMissile"
        && CRUISE_MISSILE_LAUNCH_FX == "FX_NeutronMissileLaunch"
        && CRUISE_MISSILE_IGNITION_FX == "FX_NeutronMissileIgnition"
        && CRUISE_MISSILE_EXHAUST == "NeutronMissileExhaust"
        && (CRUISE_MISSILE_DISTANCE_BEFORE_TURNING - 200.0).abs() < 0.01
        && CRUISE_MISSILE_SPECIAL_SPEED_TIME_MS == 1500
        && CRUISE_MISSILE_SPECIAL_SPEED_TIME_FRAMES == 45
        && duration_ms_to_logic_frames(CRUISE_MISSILE_SPECIAL_SPEED_TIME_MS)
            == CRUISE_MISSILE_SPECIAL_SPEED_TIME_FRAMES
        && (CRUISE_MISSILE_SPECIAL_SPEED_HEIGHT - 160.0).abs() < 0.01
        && (CRUISE_MISSILE_HEIGHT_DIE_TARGET - 10.0).abs() < 0.01
        && CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_MS == 1000
        && CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
        && duration_ms_to_logic_frames(CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_MS)
            == CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES
        && CRUISE_MISSILE_LOFT_COMPOSITE_FRAMES == 75
        && (CRUISE_MISSILE_DAMAGE - 2000.0).abs() < 0.1
        && (CRUISE_MISSILE_RADIUS - 150.0).abs() < 0.1
        && (MOAB_FLAME_DAMAGE - 5.0).abs() < 0.01
        && (MOAB_FLAME_RADIUS - 100.0).abs() < 0.1
        && (MOAB_SHOCKWAVE_AMOUNT - 250.0).abs() < 0.1
        && (MOAB_SHOCKWAVE_RADIUS - 200.0).abs() < 0.1
        && (MOAB_SHOCKWAVE_TAPER_OFF - 0.33).abs() < 0.01
        && MOAB_DAMAGE_TYPE == "EXPLOSION"
        && MOAB_DEATH_TYPE == "EXPLODED"
        && MOAB_FLAME_DAMAGE_TYPE == "FLAME"
        && MOAB_FLAME_DEATH_TYPE == "BURNED"
        && CRUISE_MISSILE_DOOR_OPEN_TIME_FRAMES == 240
        && duration_ms_to_logic_frames(CRUISE_MISSILE_DOOR_OPEN_TIME_MS)
            == CRUISE_MISSILE_DOOR_OPEN_TIME_FRAMES
        && CRUISE_MISSILE_INITIATE_SOUND == "AirRaidSiren"
        && (CRUISE_MISSILE_GEOMETRY_MAJOR_RADIUS - 7.0).abs() < 0.01
        && (CRUISE_MISSILE_GEOMETRY_HEIGHT - 60.0).abs() < 0.01
        && CRUISE_MISSILE_IMPACT_DELAY_FRAMES == 180
}

/// Honesty: ArtilleryBarrage residual pack deepen (Wave 56).
///
/// FormationSize tiers + DelayDeliveryMin/Max + WeaponErrorRadius +
/// ChinaArtilleryCannon transport honesty.
/// Fail-closed: not full ChinaArtilleryCannon DeliverPayload transport Object.
pub fn honesty_artillery_barrage_residual_pack() -> bool {
    ARTILLERY_BARRAGE_SHELL_COUNT == 12
        && ARTILLERY_BARRAGE_SHELL_COUNT_L2 == 24
        && ARTILLERY_BARRAGE_SHELL_COUNT_L3 == 36
        && ArtilleryBarrageScienceTier::Level1.formation_size() == 12
        && ArtilleryBarrageScienceTier::Level2.formation_size() == 24
        && ArtilleryBarrageScienceTier::Level3.formation_size() == 36
        && (ARTILLERY_BARRAGE_ERROR_RADIUS - 100.0).abs() < 0.1
        && ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES == 90
        && ARTILLERY_BARRAGE_DELAY_DELIVERY_MAX_MS == 3000
        && duration_ms_to_logic_frames(ARTILLERY_BARRAGE_DELAY_DELIVERY_MAX_MS)
            == ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
        && ARTILLERY_BARRAGE_DELAY_DELIVERY_MIN_FRAMES == 0
        && ARTILLERY_BARRAGE_TRANSPORT == "ChinaArtilleryCannon"
        && ARTILLERY_BARRAGE_SHELL_OBJECT == "ChinaArtilleryBarrageShell"
        && ARTILLERY_BARRAGE_WEAPON_NAME == "ArtilleryBarrageDamageWeapon"
        && (ARTILLERY_BARRAGE_PREFERRED_HEIGHT - 500.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_DELIVERY_DISTANCE - 250.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_DECAL_RADIUS - 125.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_FORMATION_SPACING - 1.0).abs() < 0.01
        && (ARTILLERY_BARRAGE_EXIT_PITCH_RATE - 30.0).abs() < 0.01
        && ARTILLERY_BARRAGE_LOCOMOTOR == "ChinaArtilleryBarrageCannonLocomotor"
        && (ARTILLERY_BARRAGE_LOCOMOTOR_SPEED - 150.0).abs() < 0.1
        && ARTILLERY_BARRAGE_FIRE_FX == "FX_ArtilleryBarrage"
        && ARTILLERY_BARRAGE_INITIATE_SOUND == "FireArtilleryCannonSound"
        && (ARTILLERY_BARRAGE_DAMAGE - 105.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_RADIUS - 50.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_CANNON_MAX_HEALTH - 200.0).abs() < 0.1
        && ARTILLERY_BARRAGE_CANNON_KIND_OF.contains("EMP_HARDENED")
        && ARTILLERY_BARRAGE_CANNON_KIND_OF.contains("UNATTACKABLE")
}

/// Honesty: NuclearMissile radiation field residual pack deepen (Wave 56).
pub fn honesty_nuke_radiation_residual_pack() -> bool {
    (NUKE_RADIATION_DAMAGE_PER_TICK - 25.0).abs() < 0.01
        && (NUKE_RADIATION_RADIUS - 200.0).abs() < 0.1
        && NUKE_RADIATION_TICK_INTERVAL_FRAMES == 23
        && NUKE_RADIATION_DURATION_FRAMES == 900
        && NUKE_RADIATION_FIRE_FX == "WeaponFX_LargeRadiationFieldWeapon"
        && NUKE_RADIATION_DAMAGE_TYPE == "RADIATION"
        && NUKE_RADIATION_DEATH_TYPE == "NORMAL"
        && (NUKE_RADIATION_WEAPON_SPEED - 600.0).abs() < 0.1
        && NUKE_RADIATION_SUSPEND_FX_DELAY_MS == 10000
        && NUKE_RADIATION_SUSPEND_FX_DELAY_FRAMES == 300
        && duration_ms_to_logic_frames(NUKE_RADIATION_SUSPEND_FX_DELAY_MS)
            == NUKE_RADIATION_SUSPEND_FX_DELAY_FRAMES
        && NUKE_RADIATION_OCL == "OCL_NukeRadiationField"
        && NUKE_RADIATION_OBJECT_NAME == "NukeRadiationFieldWeapon"
        && NUKE_RADIATION_WEAPON_NAME == "NukeRadiationFieldWeapon"
        && NUKE_RADIATION_LIFETIME_MS == 30000
        && duration_ms_to_logic_frames(NUKE_RADIATION_LIFETIME_MS) == NUKE_RADIATION_DURATION_FRAMES
        && NUKE_RADIATION_DELAY_BETWEEN_SHOTS_MS == 750
        && NUKE_RADIATION_RADIUS_DAMAGE_AFFECTS.contains("NOT_AIRBORNE")
        && (NUKE_RADIATION_FIELD_MAX_HEALTH - 150.0).abs() < 0.1
        && (NUKE_RADIATION_GEOMETRY_MAJOR_RADIUS - 100.0).abs() < 0.1
        && NUKE_RADIATION_AUDIO == "RadiationPoolAmbientLoop"
}

/// Honesty: AnthraxBomb poison field residual pack deepen (Wave 56).
pub fn honesty_anthrax_toxin_residual_pack() -> bool {
    (ANTHRAX_TOXIN_DAMAGE_PER_TICK - 40.0).abs() < 0.01
        && (ANTHRAX_TOXIN_RADIUS - 300.0).abs() < 0.1
        && ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES == 15
        && ANTHRAX_TOXIN_DURATION_FRAMES == 1800
        && ANTHRAX_TOXIN_FIRE_FX == "WeaponFX_LargePoisonFieldWeaponUpgraded"
        && ANTHRAX_TOXIN_DAMAGE_TYPE == "POISON"
        && ANTHRAX_TOXIN_DEATH_TYPE == "POISONED_BETA"
        && (ANTHRAX_TOXIN_WEAPON_SPEED - 600.0).abs() < 0.1
        && ANTHRAX_TOXIN_OCL == "OCL_PoisonFieldAnthraxBomb"
        && ANTHRAX_BOMB_WEAPON_NAME == "AnthraxBombWeapon"
        && (ANTHRAX_BOMB_IMPACT_DAMAGE - 200.0).abs() < 0.1
        && (ANTHRAX_BOMB_IMPACT_RADIUS - 100.0).abs() < 0.1
        && ANTHRAX_TOXIN_DELAY_BETWEEN_SHOTS_MS == 500
        && duration_ms_to_logic_frames(ANTHRAX_TOXIN_DELAY_BETWEEN_SHOTS_MS)
            == ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES
        && ANTHRAX_TOXIN_LIFETIME_MS == 60000
        && duration_ms_to_logic_frames(ANTHRAX_TOXIN_LIFETIME_MS) == ANTHRAX_TOXIN_DURATION_FRAMES
        && ANTHRAX_TOXIN_RADIUS_DAMAGE_AFFECTS.contains("NOT_AIRBORNE")
        && (ANTHRAX_TOXIN_ATTACK_RANGE - 15.0).abs() < 0.01
        && (ANTHRAX_TOXIN_MINIMUM_ATTACK_RANGE - 10.0).abs() < 0.01
        && ANTHRAX_TOXIN_AUDIO == "AnthraxPoolAmbientLoop"
}

/// Honesty: DaisyCutter / MOAB special-power residual pack deepen (Wave 72).
///
/// SpecialPower.ini ReloadTime / RadiusCursor / science / ViewObject +
/// DaisyCutterDetonationWeapon + DaisyCutterFlameWeapon residual.
/// Fail-closed: not full FuelAirBombPower OCL cargo plane DeliverPayload Object.
pub fn honesty_daisy_cutter_residual_pack() -> bool {
    DAISY_CUTTER_RELOAD_MS == 360_000
        && DAISY_CUTTER_RELOAD_FRAMES == 10_800
        && duration_ms_to_logic_frames(DAISY_CUTTER_RELOAD_MS) == DAISY_CUTTER_RELOAD_FRAMES
        && (DAISY_CUTTER_RADIUS_CURSOR - 170.0).abs() < 0.01
        && DAISY_CUTTER_REQUIRED_SCIENCE == "SCIENCE_DaisyCutter"
        && DAISY_CUTTER_SPECIAL_POWER == "SuperweaponDaisyCutter"
        && DAISY_CUTTER_MOAB_SPECIAL_POWER == "SuperweaponMOAB"
        && DAISY_CUTTER_VIEW_OBJECT_DURATION_MS == 30_000
        && DAISY_CUTTER_VIEW_OBJECT_DURATION_FRAMES == 900
        && duration_ms_to_logic_frames(DAISY_CUTTER_VIEW_OBJECT_DURATION_MS)
            == DAISY_CUTTER_VIEW_OBJECT_DURATION_FRAMES
        && (DAISY_CUTTER_VIEW_OBJECT_RANGE - 250.0).abs() < 0.1
        && DAISY_CUTTER_SHARED_SYNCED_TIMER
        && !DAISY_CUTTER_PUBLIC_TIMER
        && DAISY_CUTTER_SHORTCUT_POWER
        && (DAISY_CUTTER_PRIMARY_DAMAGE - 2000.0).abs() < 0.1
        && (DAISY_CUTTER_PRIMARY_RADIUS - 100.0).abs() < 0.1
        && (DAISY_CUTTER_OUTER_RADIUS - 170.0).abs() < 0.1
        && DAISY_CUTTER_IMPACT_DELAY_FRAMES == 90
        && DAISY_CUTTER_DAMAGE_TYPE == "EXPLOSION"
        && DAISY_CUTTER_DEATH_TYPE == "EXPLODED"
        && (DAISY_CUTTER_FLAME_DAMAGE - 5.0).abs() < 0.01
        && (DAISY_CUTTER_FLAME_RADIUS - 100.0).abs() < 0.1
        && DAISY_CUTTER_EXPLOSION_AUDIO == "DaisyCutterExplosion"
        // Host kind residual parity with pack constants.
        && HostSuperweaponKind::DaisyCutter.impact_delay_frames()
            == DAISY_CUTTER_IMPACT_DELAY_FRAMES
        && (HostSuperweaponKind::DaisyCutter.max_damage() - DAISY_CUTTER_PRIMARY_DAMAGE).abs()
            < 0.1
        && (HostSuperweaponKind::DaisyCutter.damage_radius() - DAISY_CUTTER_OUTER_RADIUS).abs()
            < 0.1
        && HostSuperweaponKind::DaisyCutter.spawns_moab_flame()
}

/// Honesty: A-10 Thunderbolt special-power residual pack deepen (Wave 72).
///
/// SpecialPower.ini ReloadTime / RadiusCursor / science / ViewObject +
/// A10ThunderboltMissileWeapon / Vulcan residual.
/// Fail-closed: not full AmericaJetA10Thunderbolt DeliverPayload flight Object.
pub fn honesty_a10_strike_residual_pack() -> bool {
    A10_STRIKE_RELOAD_MS == 240_000
        && A10_STRIKE_RELOAD_FRAMES == 7_200
        && duration_ms_to_logic_frames(A10_STRIKE_RELOAD_MS) == A10_STRIKE_RELOAD_FRAMES
        && (A10_STRIKE_RADIUS_CURSOR - 50.0).abs() < 0.01
        && A10_STRIKE_REQUIRED_SCIENCE == "SCIENCE_A10ThunderboltMissileStrike1"
        && A10_STRIKE_SPECIAL_POWER == "SuperweaponA10ThunderboltMissileStrike"
        && A10_STRIKE_VIEW_OBJECT_DURATION_MS == 30_000
        && A10_STRIKE_VIEW_OBJECT_DURATION_FRAMES == 900
        && duration_ms_to_logic_frames(A10_STRIKE_VIEW_OBJECT_DURATION_MS)
            == A10_STRIKE_VIEW_OBJECT_DURATION_FRAMES
        && (A10_STRIKE_VIEW_OBJECT_RANGE - 250.0).abs() < 0.1
        && A10_STRIKE_SHARED_SYNCED_TIMER
        && !A10_STRIKE_PUBLIC_TIMER
        && A10_STRIKE_SHORTCUT_POWER
        && (A10_STRIKE_HOST_MAX_DAMAGE - 500.0).abs() < 0.1
        && (A10_STRIKE_HOST_RADIUS - 100.0).abs() < 0.1
        && (A10_STRIKE_HOST_INNER_RADIUS - 40.0).abs() < 0.1
        && A10_STRIKE_IMPACT_DELAY_FRAMES == 60
        && (A10_MISSILE_PRIMARY_DAMAGE - 200.0).abs() < 0.1
        && (A10_MISSILE_PRIMARY_RADIUS - 50.0).abs() < 0.1
        && A10_MISSILE_CLIP_RELOAD_MS == 20_000
        && A10_MISSILE_CLIP_RELOAD_FRAMES == 600
        && duration_ms_to_logic_frames(A10_MISSILE_CLIP_RELOAD_MS) == A10_MISSILE_CLIP_RELOAD_FRAMES
        && (A10_VULCAN_PRIMARY_DAMAGE - 10.0).abs() < 0.01
        && (A10_VULCAN_PRIMARY_RADIUS - 4.0).abs() < 0.01
        && A10_VULCAN_DELAY_BETWEEN_SHOTS_MS == 60
        && A10_STRIKE_IMPACT_AUDIO.is_empty()
        && HostSuperweaponKind::A10Strike.impact_audio() == A10_STRIKE_IMPACT_AUDIO
        && HostSuperweaponKind::A10Strike.impact_delay_frames() == A10_STRIKE_IMPACT_DELAY_FRAMES
        && (HostSuperweaponKind::A10Strike.max_damage() - A10_STRIKE_HOST_MAX_DAMAGE).abs() < 0.1
        && (HostSuperweaponKind::A10Strike.damage_radius() - A10_STRIKE_HOST_RADIUS).abs() < 0.1
}

/// Combined Wave 72 special-power residual honesty pack (free constant packs).
///
/// Consolidates carpet/cruise/artillery/nuke/anthrax + DaisyCutter/A10 deepen.

/// Wave residual honesty: Early China CarpetBomb science/enum/reload pack.
/// Wave residual honesty: AirF CarpetBomb science/enum/reload pack.
pub fn honesty_airf_carpet_bomb_residual_pack_ok() -> bool {
    AIRF_CARPET_REQUIRED_SCIENCE == "SCIENCE_AirF_CarpetBomb"
        && AIRF_CARPET_SPECIAL_ENUM == "AIRF_SPECIAL_CARPET_BOMB"
        && AIRF_CARPET_SPECIAL_POWER == "AirF_SuperweaponCarpetBomb"
        && AIRF_CARPET_RELOAD_MS == 240_000
        && AIRF_CARPET_RELOAD_FRAMES == 7_200
        && CarpetBombFactionTier::from_science_or_ocl_name("SCIENCE_AirF_CarpetBomb")
            == Some(CarpetBombFactionTier::AirForce)
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::AirForceCarpetBomb,
        ) == Some(HostSuperweaponKind::CarpetBomb)
}

/// Retail SuperweaponBlackMarketNuke ReloadTime residual (msec).
pub const BLACK_MARKET_NUKE_RELOAD_MS: u32 = 600_000;
/// BlackMarketNuke ReloadTime frames residual.
pub const BLACK_MARKET_NUKE_RELOAD_FRAMES: u32 = 18_000;
/// Retail RequiredScience residual.
pub const BLACK_MARKET_NUKE_REQUIRED_SCIENCE: &str = "SCIENCE_BlackMarketNuke";
/// Retail Enum residual.
pub const BLACK_MARKET_NUKE_SPECIAL_ENUM: &str = "SPECIAL_BLACK_MARKET_NUKE";
/// Retail SuperweaponBlackMarketNuke name residual.
pub const BLACK_MARKET_NUKE_SPECIAL_POWER: &str = "SuperweaponBlackMarketNuke";
/// Retail SuperweaponDetonateDirtyNuke ReloadTime residual (msec).
pub const DIRTY_NUKE_RELOAD_MS: u32 = 30_000;
/// DirtyNuke ReloadTime frames residual.
pub const DIRTY_NUKE_RELOAD_FRAMES: u32 = 900;
/// Retail Enum residual.
pub const DIRTY_NUKE_SPECIAL_ENUM: &str = "SPECIAL_DETONATE_DIRTY_NUKE";
/// Retail SuperweaponDetonateDirtyNuke name residual.
pub const DIRTY_NUKE_SPECIAL_POWER: &str = "SuperweaponDetonateDirtyNuke";

/// Wave residual honesty: BlackMarketNuke + DirtyNuke map onto NuclearMissile host path.
/// Retail SuperweaponNapalmStrike ReloadTime residual (msec).
pub const NAPALM_STRIKE_RELOAD_MS: u32 = 600_000;
/// NapalmStrike ReloadTime frames residual.
pub const NAPALM_STRIKE_RELOAD_FRAMES: u32 = 18_000;
/// Retail RequiredScience residual.
pub const NAPALM_STRIKE_REQUIRED_SCIENCE: &str = "SCIENCE_NapalmStrike";
/// Retail Enum residual.
pub const NAPALM_STRIKE_SPECIAL_ENUM: &str = "SPECIAL_NAPALM_STRIKE";
/// Retail SuperweaponNapalmStrike name residual.
pub const NAPALM_STRIKE_SPECIAL_POWER: &str = "SuperweaponNapalmStrike";

/// Wave residual honesty: NapalmStrike uses own fire table / OCL, not DaisyCutter FAB.
/// Wave residual honesty: general special-power aliases map onto host residual kinds.
pub fn honesty_general_special_power_alias_pack_ok() -> bool {
    use crate::command_system::SpecialPowerType as P;
    HostSuperweaponKind::from_command_power(&P::AirForceDaisyCutter)
        == Some(HostSuperweaponKind::DaisyCutter)
        && HostSuperweaponKind::from_command_power(&P::AirForceAirstrike)
            == Some(HostSuperweaponKind::A10Strike)
        && HostSuperweaponKind::from_command_power(&P::AirForceSpectreGunship)
            == Some(HostSuperweaponKind::SpectreGunship)
        && HostSuperweaponKind::from_command_power(&P::SuperweaponParticleCannon)
            == Some(HostSuperweaponKind::ParticleCannon)
        && HostSuperweaponKind::from_command_power(&P::LaserCannon)
            == Some(HostSuperweaponKind::ParticleCannon)
        && HostSuperweaponKind::from_command_power(&P::NukeNeutronMissile)
            == Some(HostSuperweaponKind::NuclearMissile)
        && HostSuperweaponKind::from_command_power(&P::SuperweaponNeutronMissile)
            == Some(HostSuperweaponKind::NuclearMissile)
        && HostSuperweaponKind::from_command_power(&P::BaikonurRocket)
            == Some(HostSuperweaponKind::NuclearMissile)
        && HostSuperweaponKind::from_command_power(&P::NukeChinaCarpetBomb)
            == Some(HostSuperweaponKind::CarpetBomb)
        && HostSuperweaponKind::from_command_power(&P::BattleshipBombardment).is_none()
}

pub fn honesty_napalm_strike_residual_pack_ok() -> bool {
    use crate::game_logic::combat::DamageType;
    NAPALM_STRIKE_RELOAD_MS == 600_000
        && NAPALM_STRIKE_RELOAD_FRAMES == 18_000
        && NAPALM_STRIKE_REQUIRED_SCIENCE == "SCIENCE_NapalmStrike"
        && NAPALM_STRIKE_SPECIAL_ENUM == "SPECIAL_NAPALM_STRIKE"
        && NAPALM_STRIKE_SPECIAL_POWER == "SuperweaponNapalmStrike"
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::NapalmStrike,
        ) == Some(HostSuperweaponKind::NapalmStrike)
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::NapalmStrike,
        ) != Some(HostSuperweaponKind::DaisyCutter)
        && (HostSuperweaponKind::NapalmStrike.max_damage() - NAPALM_STRIKE_PRIMARY_DAMAGE).abs()
            < 0.1
        && (HostSuperweaponKind::NapalmStrike.max_damage() - DAISY_CUTTER_PRIMARY_DAMAGE).abs()
            > 1.0
        && (HostSuperweaponKind::NapalmStrike.damage_radius() - NAPALM_STRIKE_OUTER_RADIUS).abs()
            < 0.1
        && (HostSuperweaponKind::NapalmStrike.falloff_inner() - NAPALM_STRIKE_PRIMARY_RADIUS).abs()
            < 0.1
        && HostSuperweaponKind::NapalmStrike.authored_damage_type() == DamageType::Fire
        && crate::game_logic::host_ocl_special_power::special_power_template_for_host_kind(
            "NapalmStrike",
        ) == Some("SuperweaponNapalmStrike")
        && crate::game_logic::host_ocl_special_power::peel_for_special_power(
            "SuperweaponNapalmStrike",
        )
        .is_some_and(|p| p.default_ocl == NAPALM_STRIKE_OCL)
}

pub fn honesty_black_market_and_dirty_nuke_residual_pack_ok() -> bool {
    BLACK_MARKET_NUKE_RELOAD_MS == 600_000
        && BLACK_MARKET_NUKE_RELOAD_FRAMES == 18_000
        && BLACK_MARKET_NUKE_REQUIRED_SCIENCE == "SCIENCE_BlackMarketNuke"
        && BLACK_MARKET_NUKE_SPECIAL_ENUM == "SPECIAL_BLACK_MARKET_NUKE"
        && BLACK_MARKET_NUKE_SPECIAL_POWER == "SuperweaponBlackMarketNuke"
        && DIRTY_NUKE_RELOAD_MS == 30_000
        && DIRTY_NUKE_RELOAD_FRAMES == 900
        && DIRTY_NUKE_SPECIAL_ENUM == "SPECIAL_DETONATE_DIRTY_NUKE"
        && DIRTY_NUKE_SPECIAL_POWER == "SuperweaponDetonateDirtyNuke"
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::BlackMarketNuke,
        ) == Some(HostSuperweaponKind::NuclearMissile)
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::DetonateDirtyNuke,
        ) == Some(HostSuperweaponKind::NuclearMissile)
}

pub fn honesty_early_china_carpet_bomb_residual_pack_ok() -> bool {
    EARLY_CHINA_CARPET_REQUIRED_SCIENCE == "Early_SCIENCE_ChinaCarpetBomb"
        && EARLY_CHINA_CARPET_SPECIAL_ENUM == "EARLY_SPECIAL_CHINA_CARPET_BOMB"
        && EARLY_CHINA_CARPET_SPECIAL_POWER == "Early_SuperweaponChinaCarpetBomb"
        && EARLY_CHINA_CARPET_RELOAD_MS == 150_000
        && EARLY_CHINA_CARPET_RELOAD_FRAMES == 4_500
        && CarpetBombFactionTier::from_science_or_ocl_name("Early_SCIENCE_ChinaCarpetBomb")
            == Some(CarpetBombFactionTier::China)
        && HostSuperweaponKind::from_command_power(
            &crate::command_system::SpecialPowerType::EarlyChinaCarpetBomb,
        ) == Some(HostSuperweaponKind::CarpetBomb)
}

pub fn honesty_special_power_residual_pack_ok() -> bool {
    honesty_carpet_bomb_residual_pack()
        && honesty_cruise_missile_residual_pack()
        && honesty_artillery_barrage_residual_pack()
        && honesty_nuke_radiation_residual_pack()
        && honesty_anthrax_toxin_residual_pack()
        && honesty_daisy_cutter_residual_pack()
        && honesty_a10_strike_residual_pack()
}

// --- Wave 73 residual honesty packs ---

/// Honesty: SpectreGunship orbit residual pack deepen (Wave 73).
///
/// HowitzerFiringRate / FollowLag / GunshipOrbitRadius vs AttackAreaRadius /
/// science-tier AttackAreaRadius table / dual-weapon ROF schedule / SpecialPower
/// Reload+ViewObject / AttackAreaDecal residual.
/// Fail-closed: not full SpectreGunshipUpdate OCL aircraft / live gattling strafe.
pub fn honesty_spectre_orbit_residual_pack_wave73() -> bool {
    SPECTRE_HOWITZER_FIRING_RATE_MS == 300
        && SPECTRE_ORBIT_TICK_INTERVAL_FRAMES == 9
        && duration_ms_to_logic_frames(SPECTRE_HOWITZER_FIRING_RATE_MS)
            == SPECTRE_ORBIT_TICK_INTERVAL_FRAMES
        && SPECTRE_HOWITZER_FOLLOW_LAG_MS == 400
        && SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES == 12
        && duration_ms_to_logic_frames(SPECTRE_HOWITZER_FOLLOW_LAG_MS)
            == SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES
        && !spectre_howitzer_follow_ready(SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES)
        && spectre_howitzer_follow_ready(SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES + 1)
        && spectre_wind_gattling_aim(
            Vec3::ZERO,
            Vec3::new(SPECTRE_STRAFING_INCREMENT * 3.0, 0.0, 0.0),
            SPECTRE_STRAFING_INCREMENT,
            7,
        )
        .1
            == 0

        && (SPECTRE_GUNSHIP_ORBIT_RADIUS - 250.0).abs() < 0.01
        && (SPECTRE_ORBIT_RADIUS - 200.0).abs() < 0.01
        // Gunship flight circle is larger than damage/cursor AttackAreaRadius.
        && SPECTRE_GUNSHIP_ORBIT_RADIUS > SPECTRE_ORBIT_RADIUS
        && (SPECTRE_TARGETING_RETICLE_RADIUS - 25.0).abs() < 0.01
        && (SPECTRE_OVERRIDE_CONSTRAINT_RADIUS - 175.0).abs() < 0.01
        && (SPECTRE_ORBIT_RADIUS - SPECTRE_TARGETING_RETICLE_RADIUS
            - SPECTRE_OVERRIDE_CONSTRAINT_RADIUS)
            .abs()
            < 0.01

        && (SPECTRE_STRAFING_INCREMENT - 20.0).abs() < 0.01
        && (SPECTRE_ORBIT_INSERTION_SLOPE - 0.7).abs() < 0.001
        && SPECTRE_GATTLING_STRAFE_FX == "SpectreGattlingArmsSmoke"
        && SPECTRE_ATTACK_AREA_DECAL_TEXTURE == "SCCSpecTarg"
        && SPECTRE_TARGETING_RETICLE_DECAL_TEXTURE == "SCCSpecRet"
        && SPECTRE_DECAL_COLOR == (127, 177, 222, 255)
        && SPECTRE_ATTACK_AREA_DECAL_THROB_MS == 1500
        && SPECTRE_TARGETING_RETICLE_DECAL_THROB_MS == 300
        && SPECTRE_RELOAD_MS == 240_000
        && SPECTRE_RELOAD_FRAMES == 7_200
        && duration_ms_to_logic_frames(SPECTRE_RELOAD_MS) == SPECTRE_RELOAD_FRAMES
        && SPECTRE_AIRF_RELOAD_MS == 180_000
        && SPECTRE_AIRF_RELOAD_FRAMES == 5_400
        && duration_ms_to_logic_frames(SPECTRE_AIRF_RELOAD_MS) == SPECTRE_AIRF_RELOAD_FRAMES
        && SPECTRE_VIEW_OBJECT_DURATION_MS == 30_000
        && SPECTRE_VIEW_OBJECT_DURATION_FRAMES == 900
        && duration_ms_to_logic_frames(SPECTRE_VIEW_OBJECT_DURATION_MS)
            == SPECTRE_VIEW_OBJECT_DURATION_FRAMES
        && (SPECTRE_VIEW_OBJECT_RANGE - 250.0).abs() < 0.1
        && SPECTRE_SPECIAL_POWER_TEMPLATE == "SuperweaponSpectreGunship"
        && SPECTRE_AIRF_SPECIAL_POWER_TEMPLATE == "AirF_SuperweaponSpectreGunship"
        // Science tiers only change OrbitTime; AttackAreaRadius stays 200.
        && SpectreGunshipScienceTier::Level1.attack_area_radius() == SPECTRE_ORBIT_RADIUS
        && SpectreGunshipScienceTier::Level2.attack_area_radius() == SPECTRE_ORBIT_RADIUS
        && SpectreGunshipScienceTier::Level3.attack_area_radius() == SPECTRE_ORBIT_RADIUS
        && SpectreGunshipScienceTier::Level1.orbit_duration_frames() == 300
        && SpectreGunshipScienceTier::Level2.orbit_duration_frames() == 450
        && SpectreGunshipScienceTier::Level3.orbit_duration_frames() == 600
        // Dual-weapon ROF residual schedule.
        && SPECTRE_DUAL_HOWITZER_BASE_INTERVAL == 9
        && SPECTRE_DUAL_HOWITZER_MEAN_INTERVAL == 6
        && SPECTRE_DUAL_HOWITZER_FAST_INTERVAL == 4
        && SPECTRE_DUAL_GATTLING_BASE_INTERVAL == 3
        && SPECTRE_DUAL_GATTLING_MEAN_INTERVAL == 1
        && SPECTRE_DUAL_GATTLING_FAST_INTERVAL == 1
        && spectre_howitzer_interval_frames(0) == SPECTRE_DUAL_HOWITZER_BASE_INTERVAL
        && spectre_howitzer_interval_frames(2) == SPECTRE_DUAL_HOWITZER_MEAN_INTERVAL
        && spectre_howitzer_interval_frames(3) == SPECTRE_DUAL_HOWITZER_FAST_INTERVAL
        && spectre_gattling_interval_frames(0) == SPECTRE_DUAL_GATTLING_BASE_INTERVAL
        && spectre_gattling_interval_frames(2) == SPECTRE_DUAL_GATTLING_MEAN_INTERVAL
        && spectre_gattling_interval_frames(3) == SPECTRE_DUAL_GATTLING_FAST_INTERVAL
        && (SPECTRE_HOWITZER_RANDOM_OFFSET - 20.0).abs() < 0.01
        && (SPECTRE_HOWITZER_RADIUS - 25.0).abs() < 0.01
        && HostSuperweaponKind::SpectreGunship.damage_radius() == SPECTRE_ORBIT_RADIUS
}

/// Honesty: NuclearMissile radiation residual pack deepen (Wave 73).
///
/// NukeRadiationFieldWeapon AttackRange / KindOf / Armor / Geometry / DeathFX /
/// HazardFieldCore + SuperweaponNeutronMissile Reload/ViewObject residual.
/// Fail-closed: not full HazardousMaterialArmor cleanup-hazard stack.
pub fn honesty_nuke_radiation_residual_pack_wave73() -> bool {
    honesty_nuke_radiation_residual_pack()
        && (NUKE_RADIATION_ATTACK_RANGE - 15.0).abs() < 0.01
        && (NUKE_RADIATION_MINIMUM_ATTACK_RANGE - 10.0).abs() < 0.01
        && NUKE_RADIATION_KIND_OF.contains("IMMOBILE")
        && NUKE_RADIATION_KIND_OF.contains("CLEANUP_HAZARD")
        && NUKE_RADIATION_KIND_OF.contains("INERT")
        && NUKE_RADIATION_KIND_OF.contains("NO_COLLIDE")
        && NUKE_RADIATION_ARMOR == "HazardousMaterialArmor"
        && NUKE_RADIATION_GEOMETRY == "CYLINDER"
        && (NUKE_RADIATION_GEOMETRY_HEIGHT - 1.0).abs() < 0.01
        && !NUKE_RADIATION_GEOMETRY_IS_SMALL
        && (NUKE_RADIATION_FIELD_INITIAL_HEALTH - 150.0).abs() < 0.1
        && (NUKE_RADIATION_FIELD_INITIAL_HEALTH - NUKE_RADIATION_FIELD_MAX_HEALTH).abs() < 0.01
        && NUKE_RADIATION_EDITOR_SORTING == "SYSTEM"
        && NUKE_RADIATION_HAZARD_FIELD_CORE_WEAPON == "HazardFieldCoreWeapon"
        && NUKE_RADIATION_DEATH_FX == "FX_RadiationPoolDie"
        && NUCLEAR_MISSILE_RELOAD_MS == 360_000
        && NUCLEAR_MISSILE_RELOAD_FRAMES == 10_800
        && duration_ms_to_logic_frames(NUCLEAR_MISSILE_RELOAD_MS) == NUCLEAR_MISSILE_RELOAD_FRAMES
        && (NUCLEAR_MISSILE_RADIUS_CURSOR - 210.0).abs() < 0.01
        && NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_MS == 40_000
        && NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_FRAMES == 1_200
        && duration_ms_to_logic_frames(NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_MS)
            == NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_FRAMES
        && (NUCLEAR_MISSILE_VIEW_OBJECT_RANGE - 250.0).abs() < 0.1
        && NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND == "AirRaidSiren"
        && (HostSuperweaponKind::NuclearMissile.damage_radius() - NUCLEAR_MISSILE_RADIUS_CURSOR)
            .abs()
            < 0.01
}

/// Honesty: SupW / AirF / Nuke_ special-power variant residual pack (Wave 73).
///
/// SupW_SuperweaponNeutronMissile / SupW_ParticleUplink / Nuke_ Neutron /
/// AirF Spectre ReloadTime + RadiusCursor residual table.
/// Fail-closed: not full SupW ThingFactory Object / general faction select.
pub fn honesty_supw_variants_residual_pack_wave73() -> bool {
    SUPW_NEUTRON_MISSILE_RELOAD_MS == 240_000
        && SUPW_NEUTRON_MISSILE_RELOAD_FRAMES == 7_200
        && duration_ms_to_logic_frames(SUPW_NEUTRON_MISSILE_RELOAD_MS)
            == SUPW_NEUTRON_MISSILE_RELOAD_FRAMES
        && (SUPW_NEUTRON_MISSILE_RADIUS_CURSOR - 210.0).abs() < 0.01
        && SUPW_NEUTRON_MISSILE_VIEW_OBJECT_DURATION_MS == 40_000
        && SUPW_NEUTRON_MISSILE_VIEW_OBJECT_DURATION_FRAMES == 1_200
        && (SUPW_NEUTRON_MISSILE_VIEW_OBJECT_RANGE - 250.0).abs() < 0.1
        && SUPW_NEUTRON_MISSILE_SPECIAL_POWER == "SupW_SuperweaponNeutronMissile"
        && SUPW_PUC_RELOAD_MS == 180_000
        && SUPW_PUC_RELOAD_FRAMES == 5_400
        && duration_ms_to_logic_frames(SUPW_PUC_RELOAD_MS) == SUPW_PUC_RELOAD_FRAMES
        && SUPW_PUC_SPECIAL_POWER == "SupW_SuperweaponParticleUplinkCannon"
        && NUKE_GENERAL_NEUTRON_RELOAD_MS == 300_000
        && NUKE_GENERAL_NEUTRON_RELOAD_FRAMES == 9_000
        && duration_ms_to_logic_frames(NUKE_GENERAL_NEUTRON_RELOAD_MS)
            == NUKE_GENERAL_NEUTRON_RELOAD_FRAMES
        && NUKE_GENERAL_NEUTRON_SPECIAL_POWER == "Nuke_SuperweaponNeutronMissile"
        && (NUKE_GENERAL_NEUTRON_RADIUS_CURSOR - 210.0).abs() < 0.01
        // SupW Neutron is faster than standard China NeutronMissile (360s).
        && SUPW_NEUTRON_MISSILE_RELOAD_MS < NUCLEAR_MISSILE_RELOAD_MS
        // Nuke general is between SupW (240s) and standard (360s).
        && NUKE_GENERAL_NEUTRON_RELOAD_MS > SUPW_NEUTRON_MISSILE_RELOAD_MS
        && NUKE_GENERAL_NEUTRON_RELOAD_MS < NUCLEAR_MISSILE_RELOAD_MS
        // SupW PUC is faster than standard ParticleUplink (240s).
        && SUPW_PUC_RELOAD_MS < 240_000
        // AirF Spectre is faster than USA Spectre (240s).
        && SPECTRE_AIRF_RELOAD_MS < SPECTRE_RELOAD_MS
        && CRUISE_MISSILE_RELOAD_MS == 120_000
        && CRUISE_MISSILE_RELOAD_FRAMES == 3_600
}

/// Combined Wave 73 residual honesty pack.
pub fn honesty_special_power_residual_pack_wave73_ok() -> bool {
    honesty_spectre_orbit_residual_pack_wave73()
        && honesty_nuke_radiation_residual_pack_wave73()
        && honesty_supw_variants_residual_pack_wave73()
}

/// Honesty: A10 science-tier FormationSize residual pack (Wave 76).
///
/// ObjectCreationList.ini SUPERWEAPON_A10ThunderboltMissileStrike1/2/3:
/// FormationSize **1/2/3**, FormationSpacing **35**, DeliveryDistance **450**,
/// DropDelay **500**ms → **15**f, VisibleNumBones **6**, DeliveryDecal residual.
/// Fail-closed: not full AmericaJetA10Thunderbolt DeliverPayload flight Object.
pub fn honesty_a10_science_tier_residual_pack_wave76() -> bool {
    A10_SCIENCE_TIER1 == "SCIENCE_A10ThunderboltMissileStrike1"
        && A10_SCIENCE_TIER2 == "SCIENCE_A10ThunderboltMissileStrike2"
        && A10_SCIENCE_TIER3 == "SCIENCE_A10ThunderboltMissileStrike3"
        && A10_OCL_TIER1 == "SUPERWEAPON_A10ThunderboltMissileStrike1"
        && A10_OCL_TIER2 == "SUPERWEAPON_A10ThunderboltMissileStrike2"
        && A10_OCL_TIER3 == "SUPERWEAPON_A10ThunderboltMissileStrike3"
        && A10StrikeScienceTier::Level1.formation_size() == 1
        && A10StrikeScienceTier::Level2.formation_size() == 2
        && A10StrikeScienceTier::Level3.formation_size() == 3
        && A10StrikeScienceTier::Level1.science_name() == A10_SCIENCE_TIER1
        && A10StrikeScienceTier::Level2.science_name() == A10_SCIENCE_TIER2
        && A10StrikeScienceTier::Level3.science_name() == A10_SCIENCE_TIER3
        && A10StrikeScienceTier::Level1.ocl_name() == A10_OCL_TIER1
        && A10StrikeScienceTier::Level2.ocl_name() == A10_OCL_TIER2
        && A10StrikeScienceTier::Level3.ocl_name() == A10_OCL_TIER3
        && A10_FORMATIONION_SIZE_L1 == 1
        && A10_FORMATIONION_SIZE_L2 == 2
        && A10_FORMATIONION_SIZE_L3 == 3
        && A10_FORMATIONION_SIZE_L3 > A10_FORMATIONION_SIZE_L2
        && A10_FORMATIONION_SIZE_L2 > A10_FORMATIONION_SIZE_L1
        && (A10_FORMATIONION_SPACING - 35.0).abs() < 0.01
        && (A10_DELIVERY_DISTANCE - 450.0).abs() < 0.01
        && A10_DROP_DELAY_MS == 500
        && A10_DROP_DELAY_FRAMES == 15
        && duration_ms_to_logic_frames(A10_DROP_DELAY_MS) == A10_DROP_DELAY_FRAMES
        && A10_VISIBLE_NUM_BONES == 6
        && A10_VISIBLE_ITEMS_DROPPED_PER_INTERVAL == 2
        && (A10_DIVE_START_DISTANCE - 500.0).abs() < 0.01
        && (A10_DIVE_END_DISTANCE - 300.0).abs() < 0.01
        && (A10_STRAFE_LENGTH - 450.0).abs() < 0.01
        && A10_DELIVERY_DECAL_TEXTURE == "SCCA10Strike_USA"
        && A10_DELIVERY_DECAL_STYLE == "SHADOW_ALPHA_DECAL"
        && A10_DELIVERY_DECAL_OPACITY_MIN_PCT == 25
        && A10_DELIVERY_DECAL_OPACITY_MAX_PCT == 50
        && A10_DELIVERY_DECAL_THROB_MS == 500
        && A10_DELIVERY_DECAL_COLOR == (255, 156, 0, 255)
        && (A10_DELIVERY_DECAL_RADIUS - 50.0).abs() < 0.01
        && (A10_DELIVERY_DECAL_RADIUS - A10_STRIKE_RADIUS_CURSOR).abs() < 0.01
        && A10_TRANSPORT == "AmericaJetA10Thunderbolt"
        && A10_PAYLOAD_TEMPLATE == "A10ThunderboltMissile"
        && A10_PAYLOAD_WEAPON == "A10ThunderboltMissileWeapon"
        && A10StrikeScienceTier::from_science_name("SCIENCE_A10ThunderboltMissileStrike1")
            == Some(A10StrikeScienceTier::Level1)
        && A10StrikeScienceTier::from_science_name("SCIENCE_A10ThunderboltMissileStrike2")
            == Some(A10StrikeScienceTier::Level2)
        && A10StrikeScienceTier::from_science_name("SCIENCE_A10ThunderboltMissileStrike3")
            == Some(A10StrikeScienceTier::Level3)
        && A10StrikeScienceTier::from_science_name("AirF_SCIENCE_A10ThunderboltMissileStrike2")
            == Some(A10StrikeScienceTier::Level2)
        && A10StrikeScienceTier::highest_from_sciences([
            "SCIENCE_A10ThunderboltMissileStrike1",
            "SCIENCE_A10ThunderboltMissileStrike3",
        ]) == A10StrikeScienceTier::Level3
        // Tier1 science name residual still matches Wave 72 pack constant.
        && A10_STRIKE_REQUIRED_SCIENCE == A10_SCIENCE_TIER1
}

/// Combined Wave 76 special-power residual honesty (A10 science-tier pack).
pub fn honesty_special_power_residual_pack_wave76_ok() -> bool {
    honesty_a10_science_tier_residual_pack_wave76() && honesty_a10_strike_residual_pack()
}

// --- Wave 77: SpecialPower.ini InitiateSound / InitiateAtLocationSound residual name tables ---

/// Retail SuperweaponScudStorm InitiateSound residual (`ScudStormInitiated`).
pub const SCUD_STORM_INITIATE_SOUND: &str = "ScudStormInitiated";
/// Retail SuperweaponScudStorm ReloadTime residual (msec).
pub const SCUD_STORM_RELOAD_MS: u32 = 300_000;
/// SuperweaponScudStorm ReloadTime 300000ms → 9000 frames @ 30 FPS.
pub const SCUD_STORM_RELOAD_FRAMES: u32 = 9_000;
/// Retail SuperweaponParticleUplinkCannon ReloadTime residual (msec).
pub const PARTICLE_CANNON_RELOAD_MS: u32 = 240_000;
/// SuperweaponParticleUplinkCannon ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const PARTICLE_CANNON_RELOAD_FRAMES: u32 = 7_200;
/// Retail SuperweaponAnthraxBomb ReloadTime residual (msec).
pub const ANTHRAX_BOMB_RELOAD_MS: u32 = 360_000;
/// SuperweaponAnthraxBomb ReloadTime 360000ms → 10800 frames @ 30 FPS.
pub const ANTHRAX_BOMB_RELOAD_FRAMES: u32 = 10_800;

/// Wave 77: ScudStorm has no InitiateAtLocationSound residual in SpecialPower.ini.
pub const SCUD_STORM_INITIATE_AT_LOCATION_SOUND: &str = "";
/// Wave 77: powers with no retail InitiateSound leave empty residual (not special-power name).
pub const EMPTY_SPECIAL_POWER_INITIATE_SOUND: &str = "";

/// Honesty: HostSuperweaponKind retail InitiateSound / InitiateAtLocationSound name tables.
///
/// SpecialPower.ini residual only — `activate_audio()` still returns special-power
/// template labels for host residual queues; this pack freezes the retail Miles
/// event names when present (or empty when retail omits the field).
/// Fail-closed: not full Miles positional playback / sound event INI load.
pub fn honesty_special_power_audio_name_table_wave77() -> bool {
    // Powers with retail InitiateSound residual.
    SCUD_STORM_INITIATE_SOUND == "ScudStormInitiated"
        && ARTILLERY_BARRAGE_INITIATE_SOUND == "FireArtilleryCannonSound"
        && CRUISE_MISSILE_INITIATE_SOUND == "AirRaidSiren"
        // Powers with retail InitiateAtLocationSound residual.
        && NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND == "AirRaidSiren"
        && CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND == "AirRaidSiren"
        // Empty residual honesty (retail omits field or comments it out).
        && NUCLEAR_MISSILE_INITIATE_SOUND == EMPTY_SPECIAL_POWER_INITIATE_SOUND
        && ARTILLERY_BARRAGE_INITIATE_AT_LOCATION_SOUND == EMPTY_SPECIAL_POWER_INITIATE_SOUND
        && SCUD_STORM_INITIATE_AT_LOCATION_SOUND == EMPTY_SPECIAL_POWER_INITIATE_SOUND
        // Kind table residual: retail initiate / at-location names.
        && HostSuperweaponKind::ScudStorm.retail_initiate_sound() == SCUD_STORM_INITIATE_SOUND
        && HostSuperweaponKind::ArtilleryBarrage.retail_initiate_sound()
            == ARTILLERY_BARRAGE_INITIATE_SOUND
        && HostSuperweaponKind::CruiseMissile.retail_initiate_sound()
            == CRUISE_MISSILE_INITIATE_SOUND
        && HostSuperweaponKind::NuclearMissile.retail_initiate_at_location_sound()
            == NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND
        && HostSuperweaponKind::CruiseMissile.retail_initiate_at_location_sound()
            == CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND
        && HostSuperweaponKind::DaisyCutter.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::A10Strike.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::ParticleCannon.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::AnthraxBomb.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::SpectreGunship.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::CarpetBomb.retail_initiate_sound().is_empty()
        && HostSuperweaponKind::NuclearMissile.retail_initiate_sound().is_empty()
        // Host residual queue labels remain special-power template names.
        && HostSuperweaponKind::ScudStorm.activate_audio() == "SuperweaponScudStorm"
        && HostSuperweaponKind::CruiseMissile.activate_audio() == "SuperweaponCruiseMissile"
}

/// Combined Wave 77 special-power residual honesty (audio name tables).
pub fn honesty_special_power_residual_pack_wave77_ok() -> bool {
    honesty_special_power_audio_name_table_wave77()
}

// --- Wave 78: HostSuperweaponKind reload table + CarpetBomb/Artillery science residual ---

/// Honesty: complete HostSuperweaponKind retail ReloadTime residual table (all kinds).
///
/// Fail-closed: not full SpecialPower template SharedSyncedTimer / PublicTimer UI path.
pub fn honesty_host_superweapon_reload_table_wave78() -> bool {
    SCUD_STORM_RELOAD_MS == 300_000
        && SCUD_STORM_RELOAD_FRAMES == 9_000
        && duration_ms_to_logic_frames(SCUD_STORM_RELOAD_MS) == SCUD_STORM_RELOAD_FRAMES
        && PARTICLE_CANNON_RELOAD_MS == 240_000
        && PARTICLE_CANNON_RELOAD_FRAMES == 7_200
        && duration_ms_to_logic_frames(PARTICLE_CANNON_RELOAD_MS) == PARTICLE_CANNON_RELOAD_FRAMES
        && ANTHRAX_BOMB_RELOAD_MS == 360_000
        && ANTHRAX_BOMB_RELOAD_FRAMES == 10_800
        && duration_ms_to_logic_frames(ANTHRAX_BOMB_RELOAD_MS) == ANTHRAX_BOMB_RELOAD_FRAMES
        && HostSuperweaponKind::DaisyCutter.reload_ms() == DAISY_CUTTER_RELOAD_MS
        && HostSuperweaponKind::DaisyCutter.reload_frames() == DAISY_CUTTER_RELOAD_FRAMES
        && HostSuperweaponKind::A10Strike.reload_ms() == A10_STRIKE_RELOAD_MS
        && HostSuperweaponKind::A10Strike.reload_frames() == A10_STRIKE_RELOAD_FRAMES
        && HostSuperweaponKind::ScudStorm.reload_ms() == SCUD_STORM_RELOAD_MS
        && HostSuperweaponKind::ScudStorm.reload_frames() == SCUD_STORM_RELOAD_FRAMES
        && HostSuperweaponKind::ParticleCannon.reload_ms() == PARTICLE_CANNON_RELOAD_MS
        && HostSuperweaponKind::ParticleCannon.reload_frames() == PARTICLE_CANNON_RELOAD_FRAMES
        && HostSuperweaponKind::NuclearMissile.reload_ms() == NUCLEAR_MISSILE_RELOAD_MS
        && HostSuperweaponKind::NuclearMissile.reload_frames() == NUCLEAR_MISSILE_RELOAD_FRAMES
        && HostSuperweaponKind::AnthraxBomb.reload_ms() == ANTHRAX_BOMB_RELOAD_MS
        && HostSuperweaponKind::AnthraxBomb.reload_frames() == ANTHRAX_BOMB_RELOAD_FRAMES
        && HostSuperweaponKind::SpectreGunship.reload_ms() == SPECTRE_RELOAD_MS
        && HostSuperweaponKind::SpectreGunship.reload_frames() == SPECTRE_RELOAD_FRAMES
        && HostSuperweaponKind::CarpetBomb.reload_ms() == CARPET_BOMB_RELOAD_MS
        && HostSuperweaponKind::CarpetBomb.reload_frames() == CARPET_BOMB_RELOAD_FRAMES
        && HostSuperweaponKind::ArtilleryBarrage.reload_ms() == ARTILLERY_BARRAGE_RELOAD_MS
        && HostSuperweaponKind::ArtilleryBarrage.reload_frames() == ARTILLERY_BARRAGE_RELOAD_FRAMES
        && HostSuperweaponKind::CruiseMissile.reload_ms() == CRUISE_MISSILE_RELOAD_MS
        && HostSuperweaponKind::CruiseMissile.reload_frames() == CRUISE_MISSILE_RELOAD_FRAMES
        // Ordering residual: Cruise (120s) < Carpet (150s) < A10/Spectre/PUC (240s)
        // < Scud/Artillery (300s) < Daisy/Nuke/Anthrax (360s).
        && CRUISE_MISSILE_RELOAD_MS < CARPET_BOMB_RELOAD_MS
        && CARPET_BOMB_RELOAD_MS < A10_STRIKE_RELOAD_MS
        && A10_STRIKE_RELOAD_MS == SPECTRE_RELOAD_MS
        && A10_STRIKE_RELOAD_MS == PARTICLE_CANNON_RELOAD_MS
        && SCUD_STORM_RELOAD_MS == ARTILLERY_BARRAGE_RELOAD_MS
        && SCUD_STORM_RELOAD_MS > A10_STRIKE_RELOAD_MS
        && DAISY_CUTTER_RELOAD_MS == NUCLEAR_MISSILE_RELOAD_MS
        && DAISY_CUTTER_RELOAD_MS == ANTHRAX_BOMB_RELOAD_MS
        && DAISY_CUTTER_RELOAD_MS > SCUD_STORM_RELOAD_MS
}

/// Honesty: CarpetBomb faction-tier reload / cursor / OCL / DeliveryDecal residual deepen.
///
/// Fail-closed: not full AmericaJetB52 / AirF_AmericaJetB3 / ChinaJetCarpetBomber flight Object.
pub fn honesty_carpet_bomb_science_tier_residual_pack_wave78() -> bool {
    CARPET_BOMB_RELOAD_MS == 150_000
        && CARPET_BOMB_RELOAD_FRAMES == 4_500
        && duration_ms_to_logic_frames(CARPET_BOMB_RELOAD_MS) == CARPET_BOMB_RELOAD_FRAMES
        && CARPET_BOMB_RELOAD_AIRF_MS == 240_000
        && CARPET_BOMB_RELOAD_AIRF_FRAMES == 7_200
        && duration_ms_to_logic_frames(CARPET_BOMB_RELOAD_AIRF_MS) == CARPET_BOMB_RELOAD_AIRF_FRAMES
        && CARPET_BOMB_RELOAD_NUKE_MS == 180_000
        && CARPET_BOMB_RELOAD_NUKE_FRAMES == 5_400
        && duration_ms_to_logic_frames(CARPET_BOMB_RELOAD_NUKE_MS) == CARPET_BOMB_RELOAD_NUKE_FRAMES
        && CarpetBombFactionTier::America.reload_ms() == 150_000
        && CarpetBombFactionTier::China.reload_ms() == 150_000
        && CarpetBombFactionTier::AirForce.reload_ms() == 240_000
        && CarpetBombFactionTier::America.reload_frames() == 4_500
        && CarpetBombFactionTier::AirForce.reload_frames() == 7_200
        && (CarpetBombFactionTier::America.radius_cursor() - 100.0).abs() < 0.01
        && (CarpetBombFactionTier::AirForce.radius_cursor() - 180.0).abs() < 0.01
        && (CarpetBombFactionTier::China.radius_cursor() - 180.0).abs() < 0.01
        && (CarpetBombFactionTier::America.delivery_decal_radius() - 100.0).abs() < 0.01
        && (CarpetBombFactionTier::AirForce.delivery_decal_radius() - 180.0).abs() < 0.01
        && (CarpetBombFactionTier::China.delivery_decal_radius() - 180.0).abs() < 0.01
        && CarpetBombFactionTier::America.ocl_name() == CARPET_BOMB_OCL_AMERICA
        && CarpetBombFactionTier::AirForce.ocl_name() == CARPET_BOMB_OCL_AIRF
        && CarpetBombFactionTier::China.ocl_name() == CARPET_BOMB_OCL_CHINA
        && CarpetBombFactionTier::America.science_name() == CARPET_BOMB_SCIENCE_AMERICA
        && CarpetBombFactionTier::AirForce.science_name() == CARPET_BOMB_SCIENCE_AIRF
        && CarpetBombFactionTier::China.science_name() == CARPET_BOMB_SCIENCE_CHINA
        && CarpetBombFactionTier::America.delivery_decal_texture()
            == CARPET_BOMB_DECAL_TEXTURE_AMERICA
        && CarpetBombFactionTier::China.delivery_decal_texture()
            == CARPET_BOMB_DECAL_TEXTURE_CHINA_AIRF
        && CarpetBombFactionTier::America.delivery_decal_color()
            == CARPET_BOMB_DECAL_COLOR_AMERICA
        && CarpetBombFactionTier::AirForce.delivery_decal_color()
            == CARPET_BOMB_DECAL_COLOR_CHINA_AIRF
        && CARPET_BOMB_DECAL_STYLE == "SHADOW_ALPHA_DECAL"
        && CARPET_BOMB_DECAL_OPACITY_MIN_PCT == 25
        && CARPET_BOMB_DECAL_OPACITY_MAX_PCT == 50
        && CARPET_BOMB_DECAL_THROB_MS == 500
        && CARPET_BOMB_VIEW_OBJECT_DURATION_MS == 40_000
        && CARPET_BOMB_VIEW_OBJECT_DURATION_FRAMES == 1_200
        && duration_ms_to_logic_frames(CARPET_BOMB_VIEW_OBJECT_DURATION_MS)
            == CARPET_BOMB_VIEW_OBJECT_DURATION_FRAMES
        && (CARPET_BOMB_VIEW_OBJECT_RANGE - 250.0).abs() < 0.01
        // Reload ordering: USA/China 150s < Nuke 180s < AirF 240s.
        && CARPET_BOMB_RELOAD_MS < CARPET_BOMB_RELOAD_NUKE_MS
        && CARPET_BOMB_RELOAD_NUKE_MS < CARPET_BOMB_RELOAD_AIRF_MS
        && honesty_carpet_bomb_residual_pack()
}

/// Honesty: ArtilleryBarrage science-tier OCL / DeliveryDecal residual deepen (Wave 78).
///
/// Fail-closed: not full ChinaArtilleryCannon DeliverPayload transport Object.
pub fn honesty_artillery_science_tier_residual_pack_wave78() -> bool {
    ARTILLERY_SCIENCE_TIER1 == "SCIENCE_ArtilleryBarrage1"
        && ARTILLERY_SCIENCE_TIER2 == "SCIENCE_ArtilleryBarrage2"
        && ARTILLERY_SCIENCE_TIER3 == "SCIENCE_ArtilleryBarrage3"
        && ARTILLERY_OCL_TIER1 == "SUPERWEAPON_ArtilleryBarrage1"
        && ARTILLERY_OCL_TIER2 == "SUPERWEAPON_ArtilleryBarrage2"
        && ARTILLERY_OCL_TIER3 == "SUPERWEAPON_ArtilleryBarrage3"
        && ArtilleryBarrageScienceTier::Level1.formation_size() == 12
        && ArtilleryBarrageScienceTier::Level2.formation_size() == 24
        && ArtilleryBarrageScienceTier::Level3.formation_size() == 36
        && ArtilleryBarrageScienceTier::Level1.science_name() == ARTILLERY_SCIENCE_TIER1
        && ArtilleryBarrageScienceTier::Level2.science_name() == ARTILLERY_SCIENCE_TIER2
        && ArtilleryBarrageScienceTier::Level3.science_name() == ARTILLERY_SCIENCE_TIER3
        && ArtilleryBarrageScienceTier::Level1.ocl_name() == ARTILLERY_OCL_TIER1
        && ArtilleryBarrageScienceTier::Level2.ocl_name() == ARTILLERY_OCL_TIER2
        && ArtilleryBarrageScienceTier::Level3.ocl_name() == ARTILLERY_OCL_TIER3
        && ARTILLERY_SCIENCE_POINT_COST == 1
        && ARTILLERY_SCIENCE1_PREREQ == ["SCIENCE_CHINA", "SCIENCE_Rank3"]
        && ARTILLERY_SCIENCE2_PREREQ == ["SCIENCE_ArtilleryBarrage1", "SCIENCE_Rank3"]
        && ARTILLERY_SCIENCE3_PREREQ == ["SCIENCE_ArtilleryBarrage2", "SCIENCE_Rank3"]
        && ARTILLERY_DELIVERY_DECAL_TEXTURE == "SCCArtilleryBarrage_China"
        && ARTILLERY_DELIVERY_DECAL_STYLE == "SHADOW_ALPHA_DECAL"
        && ARTILLERY_DELIVERY_DECAL_OPACITY_MIN_PCT == 25
        && ARTILLERY_DELIVERY_DECAL_OPACITY_MAX_PCT == 50
        && ARTILLERY_DELIVERY_DECAL_THROB_MS == 500
        && ARTILLERY_DELIVERY_DECAL_COLOR == (255, 156, 0, 255)
        && (ARTILLERY_BARRAGE_DECAL_RADIUS - 125.0).abs() < 0.01
        && (ARTILLERY_BARRAGE_RADIUS_CURSOR - 125.0).abs() < 0.01
        && ARTILLERY_VISIBLE_NUM_BONES == 1
        && ARTILLERY_VISIBLE_ITEMS_DROPPED_PER_INTERVAL == 1
        && ARTILLERY_VIEW_OBJECT_DURATION_MS == 30_000
        && ARTILLERY_VIEW_OBJECT_DURATION_FRAMES == 900
        && duration_ms_to_logic_frames(ARTILLERY_VIEW_OBJECT_DURATION_MS)
            == ARTILLERY_VIEW_OBJECT_DURATION_FRAMES
        && (ARTILLERY_VIEW_OBJECT_RANGE - 250.0).abs() < 0.01
        && ARTILLERY_BARRAGE_RELOAD_MS == 300_000
        && ARTILLERY_BARRAGE_RELOAD_FRAMES == 9_000
        && duration_ms_to_logic_frames(ARTILLERY_BARRAGE_RELOAD_MS)
            == ARTILLERY_BARRAGE_RELOAD_FRAMES
        && ArtilleryBarrageScienceTier::from_science_name("SCIENCE_ArtilleryBarrage1")
            == Some(ArtilleryBarrageScienceTier::Level1)
        && ArtilleryBarrageScienceTier::from_science_name("SCIENCE_ArtilleryBarrage3")
            == Some(ArtilleryBarrageScienceTier::Level3)
        && ArtilleryBarrageScienceTier::highest_from_sciences([
            "SCIENCE_ArtilleryBarrage1",
            "SCIENCE_ArtilleryBarrage2",
        ]) == ArtilleryBarrageScienceTier::Level2
        && honesty_artillery_barrage_residual_pack()
}

/// Combined Wave 78 special-power residual honesty pack.
pub fn honesty_special_power_residual_pack_wave78_ok() -> bool {
    honesty_host_superweapon_reload_table_wave78()
        && honesty_carpet_bomb_science_tier_residual_pack_wave78()
        && honesty_artillery_science_tier_residual_pack_wave78()
}

/// Honesty: DeletionUpdate calcSleepDelay residual (remnant fixed 120; clamp ≥1).
pub fn honesty_deletion_update_sleep_delay() -> bool {
    PARTICLE_REMNANT_DELETION_MIN_FRAMES == 120
        && PARTICLE_REMNANT_DELETION_MAX_FRAMES == 120
        && PARTICLE_REMNANT_DELETION_MIN_FRAMES == PARTICLE_REMNANT_DURATION_FRAMES
        && particle_remnant_deletion_sleep_frames() == 120
        && deletion_update_calc_sleep_delay(0, 0, 0) == 1
        && deletion_update_calc_sleep_delay(5, 5, 99) == 5
        && {
            let d = deletion_update_calc_sleep_delay(3, 7, 1);
            d >= 3 && d <= 7
        }
}

/// Honesty: ScudStormMissile ThingFactory object residual pack (Wave 65).
///
/// Consolidates WeaponObjects.ini `Object ScudStormMissile` residual fields not
/// already closed as a single host-testable pack: Physics Mass **500**,
/// TransportSlotCount **10**, ShroudClearingRange **0**, Armor ProjectileArmor,
/// SpecialPowerCompletionDie SuperweaponScudStorm, HeightDie
/// TargetHeightIncludesStructures **Yes**, DAMAGED/REALLYDAMAGED/RUBBLE model
/// **NONE**. Fail-closed: not full ThingFactory Object / live MissileAIUpdate
/// physics flight / partition KindOf matrix.
pub fn honesty_scud_storm_missile_thing_factory_pack() -> bool {
    SCUD_STORM_MISSILE_OBJECT == "ScudStormMissile"
        && SCUD_STORM_PROJECTILE_OBJECT == "ScudStormMissile"
        && (SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01
        && SCUD_STORM_MISSILE_TRANSPORT_SLOT_COUNT == 10
        && (SCUD_STORM_MISSILE_SHROUD_CLEARING_RANGE - 0.0).abs() < 0.01
        && SCUD_STORM_MISSILE_ARMOR == "ProjectileArmor"
        && SCUD_STORM_MISSILE_DAMAGE_FX == "None"
        && SCUD_STORM_MISSILE_SPECIAL_POWER == "SuperweaponScudStorm"
        && SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES
        && (SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET - 15.0).abs() < 0.01
        && SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN
        && SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH
        && SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
        && SCUD_STORM_MISSILE_DAMAGED_MODEL == "NONE"
        && SCUD_STORM_MISSILE_MODEL == "UBScudStrm_M"
        && SCUD_STORM_MISSILE_OK_TO_CHANGE_MODEL_COLOR
        && SCUD_STORM_MISSILE_KIND_OF == "PROJECTILE"
        && (SCUD_STORM_MISSILE_VISION_RANGE - 300.0).abs() < 0.01
        && (SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01
        && (SCUD_STORM_MISSILE_INITIAL_HEALTH - 10000.0).abs() < 0.01
        && SCUD_STORM_MISSILE_GEOMETRY == "Cylinder"
        && SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL
        && (SCUD_STORM_MISSILE_GEOMETRY_RADIUS - 7.0).abs() < 0.01
        && (SCUD_STORM_MISSILE_GEOMETRY_HEIGHT - 30.0).abs() < 0.01
        && SCUD_STORM_MISSILE_DESTROY_DIE
        && SCUD_STORM_MISSILE_EDITOR_SORTING == "SYSTEM"
        && SCUD_STORM_MISSILE_LOCOMOTOR_NAME == "SCUDStormMissileLocomotor"
}

/// Honesty: SpectreHowitzerShell ThingFactory InstantDeath + geometry pack (Wave 65).
///
/// Full InstantDeath death-type residual table:
/// - DETONATED: DeathTypes `NONE +DETONATED` / FX_NukeGLA
/// - LASERED: DeathTypes `NONE +LASERED` / FX + OCL_GenericMissileDisintegrate
/// - GENERIC: DeathTypes `ALL -LASERED -DETONATED` / FX_GenericMissileDeath
/// Plus Scale **0.6**, Geometry Cylinder r**4**/h**4**, Shadow SHADOW_DECAL.
/// Fail-closed: not full InstantDeathBehavior Object / W3D ModelDraw shell drawable.
pub fn honesty_spectre_howitzer_shell_thing_factory_pack() -> bool {
    SPECTRE_HOWITZER_SHELL_OBJECT == "SpectreHowitzerShell"
        && SPECTRE_HOWITZER_PROJECTILE_OBJECT == "SpectreHowitzerShell"
        && SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_TYPES == "NONE +DETONATED"
        && SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX == "FX_NukeGLA"
        && SPECTRE_HOWITZER_SHELL_DEATH_LASERED_TYPES == "NONE +LASERED"
        && SPECTRE_HOWITZER_SHELL_DEATH_LASERED_FX == "FX_GenericMissileDisintegrate"
        && SPECTRE_HOWITZER_SHELL_DEATH_LASERED_OCL == "OCL_GenericMissileDisintegrate"
        && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_TYPES == "ALL -LASERED -DETONATED"
        && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX == "FX_GenericMissileDeath"
        && SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_TYPES.contains("DETONATED")
        && SPECTRE_HOWITZER_SHELL_DEATH_LASERED_TYPES.contains("LASERED")
        && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_TYPES.contains("-LASERED")
        && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_TYPES.contains("-DETONATED")
        && (SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01
        && SPECTRE_HOWITZER_SHELL_GEOMETRY == "Cylinder"
        && SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL
        && (SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS - 4.0).abs() < 0.01
        && (SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT - 4.0).abs() < 0.01
        && SPECTRE_HOWITZER_SHELL_SHADOW == "SHADOW_DECAL"
        && SPECTRE_HOWITZER_SHELL_MODEL == "AVSpectreShell1"
        && (SPECTRE_HOWITZER_SHELL_MASS - 1.0).abs() < 0.01
        && (SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
        && (SPECTRE_HOWITZER_SHELL_INITIAL_HEALTH - 100.0).abs() < 0.01
        && SPECTRE_HOWITZER_SHELL_KIND_OF == "PROJECTILE"
        && SPECTRE_HOWITZER_SHELL_ARMOR == "ProjectileArmor"
        && SPECTRE_HOWITZER_SHELL_DAMAGE_FX == "None"
        && (SPECTRE_HOWITZER_SHELL_VISION_RANGE - 0.0).abs() < 0.01
        && !SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_INCLUDES_STRUCTURES
        && SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN
        && SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
        && (SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT - 1.0).abs() < 0.01
        && SPECTRE_HOWITZER_SHELL_DISPLAY_NAME == "OBJECT:Missile"
        && SPECTRE_HOWITZER_SHELL_EDITOR_SORTING == "SYSTEM"
        && SPECTRE_HOWITZER_SHELL_OK_TO_CHANGE_MODEL_COLOR
}

/// Honesty: ParticleUplinkCannonTrailRemnant ThingFactory residual pack (Wave 65).
///
/// KindOf residual pack honesty: **NO_COLLIDE UNATTACKABLE IMMOBILE** (individual
/// bit residual) + ImmortalBody MaxHealth/InitialHealth **50** + EditorSorting
/// SYSTEM + FireWeaponUpdate/DeletionUpdate module presence. Fail-closed: not
/// full ThingFactory ImmortalBody / live DeletionUpdate destroyObject stack.
pub fn honesty_trail_remnant_thing_factory_pack() -> bool {
    PARTICLE_REMNANT_OBJECT_NAME == "ParticleUplinkCannonTrailRemnant"
        && PARTICLE_REMNANT_KIND_OF == "NO_COLLIDE UNATTACKABLE IMMOBILE"
        && PARTICLE_REMNANT_KIND_OF.contains("NO_COLLIDE")
        && PARTICLE_REMNANT_KIND_OF.contains("UNATTACKABLE")
        && PARTICLE_REMNANT_KIND_OF.contains("IMMOBILE")
        && PARTICLE_REMNANT_KIND_OF_NO_COLLIDE
        && PARTICLE_REMNANT_KIND_OF_UNATTACKABLE
        && PARTICLE_REMNANT_KIND_OF_IMMOBILE
        && (PARTICLE_REMNANT_MAX_HEALTH - 50.0).abs() < 0.01
        && (PARTICLE_REMNANT_INITIAL_HEALTH - 50.0).abs() < 0.01
        && (PARTICLE_REMNANT_INITIAL_HEALTH - PARTICLE_REMNANT_MAX_HEALTH).abs() < 0.01
        && PARTICLE_REMNANT_BODY == "ImmortalBody"
        && PARTICLE_REMNANT_EDITOR_SORTING == "SYSTEM"
        && PARTICLE_REMNANT_FIRE_WEAPON_UPDATE
        && PARTICLE_REMNANT_DELETION_UPDATE
        && PARTICLE_REMNANT_WEAPON_NAME == "ParticleUplinkCannonBeamTrailRemnantWeapon"
        && (PARTICLE_REMNANT_DAMAGE_PER_TICK - 15.0).abs() < 0.01
        && (PARTICLE_REMNANT_RADIUS - 10.0).abs() < 0.01
        && PARTICLE_REMNANT_TICK_INTERVAL_FRAMES == 7
        && PARTICLE_REMNANT_DURATION_FRAMES == 120
        && PARTICLE_REMNANT_MIN_LIFETIME_MS == 4000
        && PARTICLE_REMNANT_MAX_LIFETIME_MS == 4000
        && PARTICLE_REMNANT_MIN_LIFETIME_MS == PARTICLE_REMNANT_MAX_LIFETIME_MS
        && PARTICLE_REMNANT_DAMAGE_TYPE == "PARTICLE_BEAM"
        && PARTICLE_REMNANT_DEATH_TYPE == "BURNED"
        && (PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR - 1.0).abs() < 0.01
        && PARTICLE_REMNANT_IMMORTAL_NEVER_DEAD
        && honesty_immortal_body_health_floor(50.0, -100.0, 1.0)
        && honesty_deletion_update_sleep_delay()
}

// ---------------------------------------------------------------------------
// Wave 74: ThingFactory residual spawn bookkeeping (fail-closed vs live Object)
// ---------------------------------------------------------------------------

/// Host residual ThingFactory object spawn ledger entry (Wave 74).
///
/// Records object identity + pack fields at residual spawn/impact time without
/// creating a full GameLogic Object. Fail-closed: not full ThingFactory
/// `newObject` / partition KindOf matrix / live Physics module stack.
#[derive(Debug, Clone, PartialEq)]
pub struct ThingFactoryObjectSpawnResidual {
    /// Retail Object name residual (`ScudStormMissile` / shell / remnant).
    pub object_name: &'static str,
    /// Physics Mass residual.
    pub mass: f32,
    /// ActiveBody / ImmortalBody MaxHealth residual.
    pub max_health: f32,
    /// Geometry type residual (`Cylinder` or empty for remnant).
    pub geometry: &'static str,
    /// KindOf residual pack string.
    pub kind_of: &'static str,
    /// Armor residual name.
    pub armor: &'static str,
    /// Body module residual name (`ActiveBody` / `ImmortalBody` honesty).
    pub body_module: &'static str,
    /// Absolute spawn/impact frame residual.
    pub spawn_frame: u32,
    /// Spawn position residual (impact / orbit / pulse epicenter).
    pub position: Vec3,
    /// True when ImmortalBody residual is armed (TrailRemnant only).
    pub immortal_body: bool,
    /// True when DeletionUpdate residual is armed (TrailRemnant only).
    pub deletion_update: bool,
}

/// Build ScudStormMissile ThingFactory residual spawn ledger on impact.
///
/// Host residual only — not full MissileAIUpdate Object spawn.
#[inline]
pub fn scud_storm_missile_spawn_residual(
    spawn_frame: u32,
    position: Vec3,
) -> ThingFactoryObjectSpawnResidual {
    ThingFactoryObjectSpawnResidual {
        object_name: SCUD_STORM_MISSILE_OBJECT,
        mass: SCUD_STORM_MISSILE_MASS,
        max_health: SCUD_STORM_MISSILE_MAX_HEALTH,
        geometry: SCUD_STORM_MISSILE_GEOMETRY,
        kind_of: SCUD_STORM_MISSILE_KIND_OF,
        armor: SCUD_STORM_MISSILE_ARMOR,
        body_module: "ActiveBody",
        spawn_frame,
        position,
        immortal_body: false,
        deletion_update: false,
    }
}

/// Build SpectreHowitzerShell ThingFactory residual spawn ledger.
///
/// Host residual only — not full DumbProjectileBehavior Object spawn.
#[inline]
pub fn spectre_howitzer_shell_spawn_residual(
    spawn_frame: u32,
    position: Vec3,
) -> ThingFactoryObjectSpawnResidual {
    ThingFactoryObjectSpawnResidual {
        object_name: SPECTRE_HOWITZER_SHELL_OBJECT,
        mass: SPECTRE_HOWITZER_SHELL_MASS,
        max_health: SPECTRE_HOWITZER_SHELL_MAX_HEALTH,
        geometry: SPECTRE_HOWITZER_SHELL_GEOMETRY,
        kind_of: SPECTRE_HOWITZER_SHELL_KIND_OF,
        armor: SPECTRE_HOWITZER_SHELL_ARMOR,
        body_module: "ActiveBody",
        spawn_frame,
        position,
        immortal_body: false,
        deletion_update: false,
    }
}

/// Build TrailRemnant ThingFactory residual spawn ledger.
///
/// ImmortalBody + DeletionUpdate residual already closed (Wave 43/44/65) —
/// spawn bookkeeping records that pack identity on remnant field spawn.
#[inline]
pub fn trail_remnant_spawn_residual(
    spawn_frame: u32,
    position: Vec3,
) -> ThingFactoryObjectSpawnResidual {
    ThingFactoryObjectSpawnResidual {
        object_name: PARTICLE_REMNANT_OBJECT_NAME,
        mass: 0.0, // remnant has no Physics Mass residual
        max_health: PARTICLE_REMNANT_MAX_HEALTH,
        geometry: "", // remnant has no Geometry residual
        kind_of: PARTICLE_REMNANT_KIND_OF,
        armor: "", // remnant has no Armor residual
        body_module: PARTICLE_REMNANT_BODY,
        spawn_frame,
        position,
        immortal_body: true,
        deletion_update: PARTICLE_REMNANT_DELETION_UPDATE,
    }
}

/// Honesty: residual spawn ledger matches ThingFactory object pack constants.
pub fn honesty_thing_factory_spawn_residual(spawn: &ThingFactoryObjectSpawnResidual) -> bool {
    match spawn.object_name {
        name if name == SCUD_STORM_MISSILE_OBJECT => {
            (spawn.mass - SCUD_STORM_MISSILE_MASS).abs() < 0.01
                && (spawn.max_health - SCUD_STORM_MISSILE_MAX_HEALTH).abs() < 0.01
                && spawn.geometry == SCUD_STORM_MISSILE_GEOMETRY
                && spawn.kind_of == SCUD_STORM_MISSILE_KIND_OF
                && spawn.armor == SCUD_STORM_MISSILE_ARMOR
                && spawn.body_module == "ActiveBody"
                && !spawn.immortal_body
                && !spawn.deletion_update
                && honesty_scud_storm_missile_thing_factory_pack()
        }
        name if name == SPECTRE_HOWITZER_SHELL_OBJECT => {
            (spawn.mass - SPECTRE_HOWITZER_SHELL_MASS).abs() < 0.01
                && (spawn.max_health - SPECTRE_HOWITZER_SHELL_MAX_HEALTH).abs() < 0.01
                && spawn.geometry == SPECTRE_HOWITZER_SHELL_GEOMETRY
                && spawn.kind_of == SPECTRE_HOWITZER_SHELL_KIND_OF
                && spawn.armor == SPECTRE_HOWITZER_SHELL_ARMOR
                && spawn.body_module == "ActiveBody"
                && !spawn.immortal_body
                && !spawn.deletion_update
                && honesty_spectre_howitzer_shell_thing_factory_pack()
        }
        name if name == PARTICLE_REMNANT_OBJECT_NAME => {
            (spawn.max_health - PARTICLE_REMNANT_MAX_HEALTH).abs() < 0.01
                && spawn.kind_of == PARTICLE_REMNANT_KIND_OF
                && spawn.body_module == PARTICLE_REMNANT_BODY
                && spawn.immortal_body
                && spawn.deletion_update
                && honesty_trail_remnant_thing_factory_pack()
        }
        _ => false,
    }
}

/// Wave 74 residual honesty: ThingFactory spawn bookkeeping pack for Scud /
/// Howitzer shell / TrailRemnant residual objects.
///
/// Fail-closed: not full ThingFactory Object / live module stack / partition.
pub fn honesty_thing_factory_spawn_bookkeeping_wave74() -> bool {
    let scud = scud_storm_missile_spawn_residual(10, Vec3::new(1.0, 0.0, 2.0));
    let shell = spectre_howitzer_shell_spawn_residual(20, Vec3::new(3.0, 80.0, 4.0));
    let remnant = trail_remnant_spawn_residual(30, Vec3::new(5.0, 0.0, 6.0));
    honesty_thing_factory_spawn_residual(&scud)
        && honesty_thing_factory_spawn_residual(&shell)
        && honesty_thing_factory_spawn_residual(&remnant)
        && scud.spawn_frame == 10
        && shell.spawn_frame == 20
        && remnant.spawn_frame == 30
        && (scud.position.x - 1.0).abs() < 0.01
        && (shell.position.y - 80.0).abs() < 0.01
        && (remnant.position.z - 6.0).abs() < 0.01
        && scud.object_name == "ScudStormMissile"
        && shell.object_name == "SpectreHowitzerShell"
        && remnant.object_name == "ParticleUplinkCannonTrailRemnant"
        && remnant.immortal_body
        && remnant.deletion_update
        && !scud.immortal_body
        && !shell.deletion_update
}

/// Retail ImmortalBody health floor residual (never drop below 1 HP).
pub const PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR: f32 = 1.0;
/// Retail ImmortalBody never-dead residual (never mark effectively dead).
pub const PARTICLE_REMNANT_IMMORTAL_NEVER_DEAD: bool = true;

/// Apply ImmortalBody `internalChangeHealth` residual clamp.
///
/// C++: `delta = max(delta, -getHealth() + 1)` then ActiveBody change — health
/// never falls below 1 and object is never marked dead. Host residual is pure
/// arithmetic (fail-closed vs full BodyModule / Object death flag matrix).
#[inline]
pub fn immortal_body_apply_health_delta(current_health: f32, delta: f32) -> f32 {
    let floor = PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR;
    let clamped_delta = delta.max(-current_health + floor);
    (current_health + clamped_delta).max(floor)
}

/// Honesty: ImmortalBody health-floor residual never drops below 1 / never dead.
#[inline]
pub fn honesty_immortal_body_health_floor(
    current_health: f32,
    delta: f32,
    result_health: f32,
) -> bool {
    immortal_body_apply_health_delta(current_health, delta) == result_health
        && result_health >= PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR
        && PARTICLE_REMNANT_IMMORTAL_NEVER_DEAD
}
