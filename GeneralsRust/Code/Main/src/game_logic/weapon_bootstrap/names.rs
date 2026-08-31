use super::*;

/// Retail primary weapon names used by host golden / skirmish unit templates.
pub const RANGER_PRIMARY_WEAPON: &str = "RangerAdvancedCombatRifle";
pub const GLA_REBEL_PRIMARY_WEAPON: &str = "GLARebelMachineGun";
pub const REDGUARD_PRIMARY_WEAPON: &str = "RedguardMachineGun";
pub const TANK_HUNTER_PRIMARY_WEAPON: &str = "ChinaInfantryTankHunterMissileLauncher";
/// China Troop Crawler residual DEPLOY primary.
pub const TROOP_CRAWLER_ASSAULT_WEAPON: &str = "TroopCrawlerAssault";
pub const HUMVEE_PRIMARY_WEAPON: &str = "HumveeGun";

/// Retail secondary weapon names used by host golden / skirmish unit templates.
/// Fail-closed residual: only units that need SECONDARY in host combat probes.
pub const RANGER_SECONDARY_WEAPON: &str = "RangerFlashBangGrenadeWeapon";
pub const HUMVEE_SECONDARY_WEAPON: &str = "HumveeMissileWeapon";

/// Retail base-defense primary weapons (Patriot / Gattling / Stinger structure residual).
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";
/// Retail Patriot secondary AA residual.
pub const PATRIOT_SECONDARY_WEAPON: &str = "PatriotMissileWeaponAir";
/// Retail Stinger Site residual (SPAWNS_ARE_THE_WEAPONS abstraction).
pub const STINGER_PRIMARY_WEAPON: &str = "StingerMissileWeapon";
pub const STINGER_SECONDARY_WEAPON: &str = "StingerMissileWeaponAir";
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";
/// Retail China Gattling Cannon secondary AA residual.
pub const GATTLING_BUILDING_SECONDARY_WEAPON: &str = "GattlingBuildingGunAir";

/// Retail Nuke Cannon primary / neutron secondary residual weapons.
pub const NUKE_CANNON_PRIMARY_WEAPON: &str = "NukeCannonGun";
pub const NUKE_CANNON_NEUTRON_WEAPON: &str = "NukeCannonNeutronWeapon";

/// Retail Inferno Cannon primary residual weapon (FireFieldSmall on impact).
pub const INFERNO_CANNON_PRIMARY_WEAPON: &str = "InfernoCannonGun";

/// Retail AuroraBombWeapon residual (AmericaJetAurora dive bomb).
pub const AURORA_BOMB_PRIMARY_WEAPON: &str = "AuroraBombWeapon";
/// Retail AirF_AuroraBombWeapon residual (FuelAir detonation path).
pub const AIRF_AURORA_BOMB_PRIMARY_WEAPON: &str = "AirF_AuroraBombWeapon";
/// Retail SupW_AuroraFuelBombWeapon residual.
pub const SUPW_AURORA_FUEL_BOMB_WEAPON: &str = "SupW_AuroraFuelBombWeapon";

/// Retail PointDefenseLaser residual weapons (Paladin / Avenger).
pub const PALADIN_POINT_DEFENSE_LASER: &str = "PaladinPointDefenseLaser";
pub const AVENGER_POINT_DEFENSE_LASER: &str = "AvengerPointDefenseLaserOne";

/// Retail America main battle tank guns + Avenger residual weapons.
pub const CRUSADER_TANK_GUN: &str = "CrusaderTankGun";
pub const PALADIN_TANK_GUN: &str = "PaladinTankGun";
/// Retail Laser General tank laser residual weapons.
pub const LAZR_CRUSADER_TANK_GUN: &str = "Lazr_CrusaderTankGun";
pub const LAZR_PALADIN_TANK_GUN: &str = "Lazr_PaladinTankGun";
pub const AVENGER_TARGET_DESIGNATOR: &str = "AvengerTargetDesignator";
pub const AVENGER_AIR_LASER: &str = "AvengerAirLaserOne";
/// Retail Humvee TOW air tertiary residual.
pub const HUMVEE_MISSILE_WEAPON_AIR: &str = "HumveeMissileWeaponAir";
/// Retail Laser General Patriot residual weapons.
pub const LAZR_PATRIOT_PRIMARY_WEAPON: &str = "Lazr_PatriotMissileWeapon";
pub const LAZR_PATRIOT_SECONDARY_WEAPON: &str = "Lazr_PatriotMissileWeaponAir";
/// Retail Superweapon General EMP Patriot residual weapons.
pub const SUPW_PATRIOT_PRIMARY_WEAPON: &str = "SupW_PatriotMissileWeapon";
pub const SUPW_PATRIOT_SECONDARY_WEAPON: &str = "SupW_PatriotMissileWeaponAir";
/// Retail GLA Tunnel Network structure gun residual.
pub const TUNNEL_NETWORK_GUN: &str = "TunnelNetworkGun";

/// Retail StealthJetMissileWeapon residual (Bunker Buster carrier primary).
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";

/// Retail MicrowaveTankBuildingClearer residual (KILL_GARRISONED damage type).
pub const MICROWAVE_BUILDING_CLEARER_WEAPON: &str = "MicrowaveTankBuildingClearer";

/// Retail Comanche primary / anti-tank / rocket-pod residual weapons.
pub const COMANCHE_PRIMARY_WEAPON: &str = "Comanche20mmCannonWeapon";
pub const COMANCHE_ANTITANK_WEAPON: &str = "ComancheAntiTankMissileWeapon";
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";

/// Retail Helix PRIMARY minigun residual weapon.
pub const HELIX_MINIGUN_WEAPON: &str = "HelixMinigunWeapon";

/// Retail China MiG napalm / BlackNapalm / Nuke MiG residual weapons.
pub const NAPALM_MISSILE_WEAPON: &str = "NapalmMissileWeapon";
pub const BLACK_NAPALM_MISSILE_WEAPON: &str = "BlackNapalmMissileWeapon";
pub const NUKE_MIG_MISSILE_WEAPON: &str = "Nuke_MiGMissileWeapon";
pub const NUKE_NUKE_MISSILE_WEAPON: &str = "Nuke_NukeMissileWeapon";

/// Retail America Fire Base howitzer residual weapon.
pub const FIRE_BASE_HOWITZER_WEAPON: &str = "FireBaseHowitzerGun";

/// Retail Sentry Drone gun residual weapon (PLAYER_UPGRADE primary).
pub const SENTRY_DRONE_GUN_WEAPON: &str = "SentryDroneGun";

/// Retail Pathfinder sniper residual weapon.
pub const PATHFINDER_SNIPER_WEAPON: &str = "USAPathfinderSniperRifle";

/// Retail Hellfire drone residual weapon.
pub const HELLFIRE_MISSILE_WEAPON: &str = "HellfireMissileWeapon";

/// Host GLA Angry Mob residual aggregate fire weapon (nexus residual).
pub const ANGRY_MOB_RESIDUAL_WEAPON: &str = "GLAAngryMobResidualWeapon";

/// Retail GLA Rocket Buggy primary residual weapons.
pub const BUGGY_ROCKET_WEAPON: &str = "BuggyRocketWeapon";
pub const BUGGY_ROCKET_WEAPON_UPGRADED: &str = "BuggyRocketWeaponUpgraded";

/// Retail GLA Quad Cannon ground / anti-air residual weapons.
pub const QUAD_CANNON_GUN: &str = "QuadCannonGun";
pub const QUAD_CANNON_GUN_AIR: &str = "QuadCannonGunAir";
pub const QUAD_CANNON_GUN_UPGRADE_ONE: &str = "QuadCannonGunUpgradeOne";
pub const QUAD_CANNON_GUN_UPGRADE_ONE_AIR: &str = "QuadCannonGunUpgradeOneAir";
pub const QUAD_CANNON_GUN_UPGRADE_TWO: &str = "QuadCannonGunUpgradeTwo";
pub const QUAD_CANNON_GUN_UPGRADE_TWO_AIR: &str = "QuadCannonGunUpgradeTwoAir";

/// Retail GLA Technical residual weapons (salvage tiers).
pub const TECHNICAL_MACHINE_GUN: &str = "TechnicalMachineGunWeapon";
pub const TECHNICAL_CANNON: &str = "TechnicalCannonWeapon";
pub const TECHNICAL_RPG: &str = "TechnicalRPGWeapon";

/// Retail GLA Toxin Tractor residual weapons.
pub const TOXIN_TRUCK_GUN: &str = "ToxinTruckGun";
pub const TOXIN_TRUCK_GUN_UPGRADED: &str = "ToxinTruckGunUpgraded";
pub const TOXIN_TRUCK_SPRAYER: &str = "ToxinTruckSprayer";
pub const TOXIN_TRUCK_SPRAYER_UPGRADED: &str = "ToxinTruckSprayerUpgraded";

/// Retail GLA SCUD launcher residual weapons.
pub const SCUD_GUN_EXPLOSIVE: &str = "SCUDLauncherGunExplosive";
pub const SCUD_GUN_TOXIN: &str = "SCUDLauncherGunToxin";
pub const SCUD_GUN_ANTHRAX: &str = "SCUDLauncherGunAnthrax";

/// Retail GLA Marauder salvage fire-rate residual weapons.
pub const MARAUDER_TANK_GUN: &str = "MarauderTankGun";
pub const MARAUDER_TANK_GUN_UPGRADE_ONE: &str = "MarauderTankGunUpgradeOne";
pub const MARAUDER_TANK_GUN_UPGRADE_TWO: &str = "MarauderTankGunUpgradeTwo";

/// Retail China Battlemaster primary residual weapon.
pub const BATTLE_MASTER_TANK_GUN: &str = "BattleMasterTankGun";

/// Retail GLA Scorpion residual weapons (gun + rocket secondary).
pub const SCORPION_TANK_GUN: &str = "ScorpionTankGun";
pub const SCORPION_TANK_GUN_PLUS_ONE: &str = "ScorpionTankGunPlusOne";
pub const SCORPION_MISSILE_WEAPON: &str = "ScorpionMissileWeapon";

/// Retail America Tomahawk residual weapon.
pub const TOMAHAWK_MISSILE_WEAPON: &str = "TomahawkMissileWeapon";

/// USA Raptor jet residual.
pub const RAPTOR_JET_MISSILE_WEAPON: &str = "RaptorJetMissileWeapon";
pub const AIRF_RAPTOR_JET_MISSILE_WEAPON: &str = "AirF_RaptorJetMissileWeapon";

/// USA Battle Drone residual.
pub const BATTLE_DRONE_MACHINE_GUN: &str = "BattleDroneMachineGun";

/// Retail China Overlord / Emperor residual main gun.
pub const OVERLORD_TANK_GUN: &str = "OverlordTankGun";

/// Retail GLA Jarmen Kell residual primary sniper.
pub const JARMEN_KELL_RIFLE: &str = "GLAJarmenKellRifle";

/// Retail GLA Combat Cycle rider residual weapons.
pub const REBEL_BIKER_MG: &str = "GLARebelBikerMachineGun";
pub const TUNNEL_DEFENDER_BIKER_ROCKET: &str = "TunnelDefenderBikerRocketWeapon";
pub const BIKER_KELL_SNIPER: &str = "GLABikerKellSniperRifle";
pub const TERRORIST_SUICIDE_WEAPON: &str = "TerroristSuicideWeapon";
/// Retail FireWeaponWhenDead residual for infantry Terrorist.
pub const SUICIDE_DYNAMITE_PACK: &str = "SuicideDynamitePack";

/// Retail USA Missile Defender residual weapons.
pub const MISSILE_DEFENDER_MISSILE_WEAPON: &str = "MissileDefenderMissileWeapon";
pub const MISSILE_DEFENDER_LASER_GUIDED_WEAPON: &str = "MissileDefenderLaserGuidedMissileWeapon";

/// Retail China Dragon Tank flame residual weapons.
pub const DRAGON_TANK_FLAME_WEAPON: &str = "DragonTankFlameWeapon";
pub const DRAGON_TANK_FLAME_WEAPON_UPGRADED: &str = "DragonTankFlameWeaponUpgraded";

/// Retail China Gattling Tank residual weapons.
pub const GATTLING_TANK_GUN: &str = "GattlingTankGun";
pub const GATTLING_TANK_GUN_AIR: &str = "GattlingTankGunAir";

/// Retail China MiniGunner residual weapons (Infantry General).
pub const MINIGUNNER_GUN: &str = "Infa_MiniGunnerGun";
pub const MINIGUNNER_GUN_AIR: &str = "Infa_MiniGunnerGunAir";

/// Retail GLA RPG Trooper / Tunnel Defender residual rocket.
pub const TUNNEL_DEFENDER_ROCKET_WEAPON: &str = "TunnelDefenderRocketWeapon";

/// Retail USA Dozer mine-clearing detail weapon (MINE_CLEARING_DETAIL residual).
pub const DOZER_MINE_DISARMING_WEAPON: &str = "DozerMineDisarmingWeapon";
/// Retail GLA Worker mine-clearing detail weapon (MINE_CLEARING_DETAIL residual).
pub const WORKER_MINE_DISARMING_WEAPON: &str = "WorkerMineDisarmingWeapon";
