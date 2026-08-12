use super::*;

/// Look up the retail primary weapon template name for a host unit template.
pub fn primary_weapon_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(RANGER_PRIMARY_WEAPON),
        "GLA_Soldier" | "GLA_Rebel" | "GLAInfantryRebel" => Some(GLA_REBEL_PRIMARY_WEAPON),
        "GLA_Terrorist"
        | "GLAInfantryTerrorist"
        | "TestTerrorist"
        | "Chem_GLAInfantryTerrorist"
        | "Demo_GLAInfantryTerrorist"
        | "Slth_GLAInfantryTerrorist" => Some(TERRORIST_SUICIDE_WEAPON),
        "USA_MissileDefender"
        | "AmericaInfantryMissileDefender"
        | "TestMissileDefender"
        | "SupW_AmericaInfantryMissileDefender" => Some(MISSILE_DEFENDER_MISSILE_WEAPON),
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => {
            Some(REDGUARD_PRIMARY_WEAPON)
        }
        "China_TankHunter"
        | "ChinaInfantryTankHunter"
        | "Tank_ChinaInfantryTankHunter"
        | "Nuke_ChinaInfantryTankHunter"
        | "Infa_ChinaInfantryTankHunter"
        | "TestTankHunter" => Some(TANK_HUNTER_PRIMARY_WEAPON),
        "ChinaVehicleTroopCrawler"
        | "China_TroopCrawler"
        | "Tank_ChinaVehicleTroopCrawler"
        | "Nuke_ChinaVehicleTroopCrawler"
        | "TestTroopCrawler" => Some(TROOP_CRAWLER_ASSAULT_WEAPON),
        "USA_Humvee" | "AmericaVehicleHumvee" | "TestHumvee" | "GoldenHumvee" => {
            Some(HUMVEE_PRIMARY_WEAPON)
        }
        "USA_Crusader" | "USA_CrusaderTank" | "AmericaTankCrusader" | "TestCrusader" => {
            Some(CRUSADER_TANK_GUN)
        }
        "Lazr_AmericaTankCrusader" | "TestLazrCrusader" => Some(LAZR_CRUSADER_TANK_GUN),
        "USA_Paladin" | "USA_PaladinTank" | "AmericaTankPaladin" | "TestPaladin" => {
            Some(PALADIN_TANK_GUN)
        }
        "Lazr_AmericaTankPaladin" | "TestLazrPaladin" => Some(LAZR_PALADIN_TANK_GUN),
        "USA_Avenger" | "AmericaTankAvenger" | "AmericaVehicleAvenger" | "TestAvenger" => {
            Some(AVENGER_TARGET_DESIGNATOR)
        }
        // Base-defense structures (Patriot / Gattling / Stinger residual auto-fire).
        "USA_Patriot"
        | "USA_PatriotMissile"
        | "AmericaPatriotBattery"
        | "PatriotMissile"
        | "TestPatriot" => Some(PATRIOT_PRIMARY_WEAPON),
        "Lazr_AmericaPatriotBattery" | "Lazr_PatriotMissileSystem" | "TestLazrPatriot" => {
            Some(LAZR_PATRIOT_PRIMARY_WEAPON)
        }
        "SupW_AmericaPatriotBattery"
        | "SupW_PatriotMissileSystem"
        | "TestSupWPatriot"
        | "TestEmpPatriot" => Some(SUPW_PATRIOT_PRIMARY_WEAPON),
        "GLATunnelNetwork"
        | "GLA_TunnelNetwork"
        | "Demo_GLATunnelNetwork"
        | "Chem_GLATunnelNetwork"
        | "Slth_GLATunnelNetwork"
        | "GLASneakAttackTunnelNetwork"
        | "TestTunnelNetwork" => Some(TUNNEL_NETWORK_GUN),
        "GLA_StingerSite"
        | "GLAStingerSite"
        | "Chem_GLAStingerSite"
        | "Demo_GLAStingerSite"
        | "Slth_GLAStingerSite"
        | "GC_Slth_GLAStingerSite"
        | "GC_Chem_GLAStingerSite"
        | "TestStingerSite" => Some(STINGER_PRIMARY_WEAPON),
        "China_GattlingCannon"
        | "ChinaGattlingCannon"
        | "Nuke_ChinaGattlingCannon"
        | "Tank_ChinaGattlingCannon"
        | "Infa_ChinaGattlingCannon"
        | "TestGattlingCannon" => Some(GATTLING_BUILDING_PRIMARY_WEAPON),
        "China_NukeCannon"
        | "ChinaVehicleNukeCannon"
        | "Nuke_ChinaVehicleNukeCannon"
        | "TestNukeCannon" => Some(NUKE_CANNON_PRIMARY_WEAPON),
        "China_InfernoCannon"
        | "ChinaVehicleInfernoCannon"
        | "Nuke_ChinaVehicleInfernoCannon"
        | "TestInfernoCannon" => Some(INFERNO_CANNON_PRIMARY_WEAPON),
        // AmericaJetAurora residual dive bomb.
        "AmericaJetAurora" | "USA_Aurora" | "TestAurora" => Some(AURORA_BOMB_PRIMARY_WEAPON),
        "AirF_AmericaJetAurora" | "TestAuroraFuelAir" => Some(AIRF_AURORA_BOMB_PRIMARY_WEAPON),
        "SupW_AmericaJetAurora" => Some(SUPW_AURORA_FUEL_BOMB_WEAPON),
        "Lazr_AmericaJetAurora" => Some(AURORA_BOMB_PRIMARY_WEAPON),
        "AmericaJetStealthFighter"
        | "USA_StealthFighter"
        | "TestStealthFighter"
        | "SupW_AmericaJetStealthFighter"
        | "Lazr_AmericaJetStealthFighter"
        | "AirF_AmericaJetStealthFighter" => Some(STEALTH_JET_MISSILE_WEAPON),
        "AmericaTankMicrowave"
        | "AmericaVehicleMicrowaveTank"
        | "USA_MicrowaveTank"
        | "Lazr_AmericaTankMicrowave"
        | "AirF_AmericaTankMicrowave"
        | "SupW_AmericaTankMicrowave"
        | "TestMicrowave"
        | "TestMicrowaveTank" => Some(MICROWAVE_BUILDING_CLEARER_WEAPON),
        // AmericaVehicleComanche residual primary cannon.
        "AmericaVehicleComanche"
        | "USA_Comanche"
        | "TestComanche"
        | "AirF_AmericaVehicleComanche"
        | "SupW_AmericaVehicleComanche"
        | "Lazr_AmericaVehicleComanche" => Some(COMANCHE_PRIMARY_WEAPON),

        // China MiG residual napalm missiles.
        "ChinaJetMIG" | "China_MiG" | "TestMiG" | "Tank_ChinaJetMIG" | "Infa_ChinaJetMIG"
        | "Boss_JetMIG" => Some(NAPALM_MISSILE_WEAPON),
        "Nuke_ChinaJetMIG" => Some(NUKE_MIG_MISSILE_WEAPON),
        // America Fire Base residual howitzer.
        "AmericaFireBase"
        | "USA_FireBase"
        | "TestFireBase"
        | "AirF_AmericaFireBase"
        | "SupW_AmericaFireBase"
        | "Lazr_AmericaFireBase" => Some(FIRE_BASE_HOWITZER_WEAPON),
        // China Helix residual primary minigun.
        "ChinaVehicleHelix"
        | "China_Helix"
        | "Nuke_ChinaVehicleHelix"
        | "Infa_ChinaVehicleHelix"
        | "Tank_ChinaVehicleHelix"
        | "TestHelix" => Some(HELIX_MINIGUN_WEAPON),
        // Pathfinder sniper residual.
        "AmericaInfantryPathfinder"
        | "USA_Pathfinder"
        | "TestPathfinder"
        | "AirF_AmericaInfantryPathfinder"
        | "SupW_AmericaInfantryPathfinder"
        | "Lazr_AmericaInfantryPathfinder" => Some(PATHFINDER_SNIPER_WEAPON),
        // Hellfire drone residual primary.
        "AmericaVehicleHellfireDrone"
        | "USA_HellfireDrone"
        | "TestHellfireDrone"
        | "AirF_AmericaVehicleHellfireDrone"
        | "SupW_AmericaVehicleHellfireDrone"
        | "Lazr_AmericaVehicleHellfireDrone" => Some(HELLFIRE_MISSILE_WEAPON),
        // Scout drone has no primary weapon (sensor only).
        "AmericaVehicleScoutDrone"
        | "USA_ScoutDrone"
        | "TestScoutDrone"
        | "AirF_AmericaVehicleScoutDrone"
        | "SupW_AmericaVehicleScoutDrone"
        | "Lazr_AmericaVehicleScoutDrone" => None,
        // Sentry gun is PLAYER_UPGRADE only — no primary until research residual.
        "AmericaVehicleSentryDrone"
        | "USA_SentryDrone"
        | "TestSentryDrone"
        | "AirF_AmericaVehicleSentryDrone"
        | "SupW_AmericaVehicleSentryDrone"
        | "Lazr_AmericaVehicleSentryDrone" => None,
        // GLA Rocket Buggy residual long-range rockets.
        "GLAVehicleRocketBuggy"
        | "GLA_RocketBuggy"
        | "TestRocketBuggy"
        | "Chem_GLAVehicleRocketBuggy"
        | "Demo_GLAVehicleRocketBuggy"
        | "Slth_GLAVehicleRocketBuggy" => Some(BUGGY_ROCKET_WEAPON),
        // GLA Quad Cannon residual ground gun.
        "GLAVehicleQuadCannon"
        | "GLA_QuadCannon"
        | "TestQuadCannon"
        | "Chem_GLAVehicleQuadCannon"
        | "Demo_GLAVehicleQuadCannon"
        | "Slth_GLAVehicleQuadCannon" => Some(QUAD_CANNON_GUN),
        // GLA SCUD launcher residual explosive primary.
        "GLAVehicleScudLauncher"
        | "GLA_ScudLauncher"
        | "TestScudLauncher"
        | "Chem_GLAVehicleScudLauncher"
        | "Demo_GLAVehicleScudLauncher"
        | "Slth_GLAVehicleScudLauncher" => Some(SCUD_GUN_EXPLOSIVE),
        // GLA Technical residual machine gun (salvage tiers swap residual).
        "GLAVehicleTechnical"
        | "GLA_Technical"
        | "TestTechnical"
        | "Chem_GLAVehicleTechnical"
        | "Demo_GLAVehicleTechnical"
        | "Slth_GLAVehicleTechnical"
        | "GLAVehicleTechnicalChassisOne"
        | "GLAVehicleTechnicalChassisTwo"
        | "GLAVehicleTechnicalChassisThree" => Some(TECHNICAL_MACHINE_GUN),
        // GLA Toxin Tractor residual poison stream primary.
        "GLAVehicleToxinTruck"
        | "GLA_ToxinTruck"
        | "TestToxinTruck"
        | "Chem_GLAVehicleToxinTruck"
        | "Demo_GLAVehicleToxinTruck"
        | "Slth_GLAVehicleToxinTruck" => Some(TOXIN_TRUCK_GUN),
        // GLA Scorpion residual tank gun (salvage + rocket residual).
        "GLATankScorpion"
        | "GLA_Scorpion"
        | "GLA_ScorpionTank"
        | "TestScorpion"
        | "Chem_GLATankScorpion"
        | "Demo_GLATankScorpion"
        | "Slth_GLATankScorpion" => Some(SCORPION_TANK_GUN),
        // America Tomahawk residual missile.
        "AmericaVehicleTomahawk"
        | "USA_Tomahawk"
        | "USA_TomahawkLauncher"
        | "TestTomahawk"
        | "SupW_AmericaVehicleTomahawk" => Some(TOMAHAWK_MISSILE_WEAPON),
        // AmericaJetRaptor residual missiles (King Raptor uses AirF weapon).
        "AmericaJetRaptor"
        | "USA_Raptor"
        | "TestRaptor"
        | "SupW_AmericaJetRaptor"
        | "Lazr_AmericaJetRaptor" => Some(RAPTOR_JET_MISSILE_WEAPON),
        "AirF_AmericaJetRaptor" | "TestKingRaptor" => Some(AIRF_RAPTOR_JET_MISSILE_WEAPON),
        // AmericaVehicleBattleDrone residual MG.
        "AmericaVehicleBattleDrone"
        | "USA_BattleDrone"
        | "TestBattleDrone"
        | "SupW_AmericaVehicleBattleDrone"
        | "AirF_AmericaVehicleBattleDrone"
        | "Lazr_AmericaVehicleBattleDrone" => Some(BATTLE_DRONE_MACHINE_GUN),
        // China Overlord / Emperor residual main gun.
        "ChinaTankOverlord"
        | "China_OverlordTank"
        | "TestOverlord"
        | "Nuke_ChinaTankOverlord"
        | "Tank_ChinaTankOverlord"
        | "Infa_ChinaTankOverlord"
        | "Tank_ChinaTankEmperor"
        | "TestEmperor" => Some(OVERLORD_TANK_GUN),
        // GLA Jarmen Kell residual sniper.
        "GLAInfantryJarmenKell"
        | "GLA_JarmenKell"
        | "TestJarmenKell"
        | "Chem_GLAInfantryJarmenKell"
        | "Demo_GLAInfantryJarmenKell"
        | "Slth_GLAInfantryJarmenKell"
        | "GC_Slth_GLAInfantryJarmenKell" => Some(JARMEN_KELL_RIFLE),
        // GLA Marauder residual tank gun (salvage tiers swap fire-rate residual).
        "GLATankMarauder"
        | "GLA_MarauderTank"
        | "TestMarauder"
        | "Chem_GLATankMarauder"
        | "Demo_GLATankMarauder"
        | "Slth_GLATankMarauder" => Some(MARAUDER_TANK_GUN),
        // China Battlemaster residual main gun (Uranium / horde ROF residual).
        "ChinaTankBattleMaster"
        | "China_BattlemasterTank"
        | "China_BattleTank"
        | "TestBattlemaster"
        | "Tank_ChinaTankBattleMaster"
        | "Nuke_ChinaTankBattleMaster" => Some(BATTLE_MASTER_TANK_GUN),
        // China Dragon Tank residual flame.
        "ChinaTankDragon"
        | "China_DragonTank"
        | "TestDragonTank"
        | "Tank_ChinaTankDragon"
        | "Nuke_ChinaTankDragon"
        | "Infa_ChinaTankDragon" => Some(DRAGON_TANK_FLAME_WEAPON),
        // China Gattling Tank residual ground gun (AA secondary separate).
        "ChinaTankGattling"
        | "China_GattlingTank"
        | "TestGattlingTank"
        | "Tank_ChinaTankGattling"
        | "Nuke_ChinaTankGattling"
        | "Infa_ChinaTankGattling" => Some(GATTLING_TANK_GUN),
        // China Infantry General MiniGunner residual ground gun.
        "Infa_ChinaInfantryMiniGunner"
        | "China_MiniGunner"
        | "TestMiniGunner"
        | "ChinaInfantryMiniGunner" => Some(MINIGUNNER_GUN),
        // GLA RPG Trooper / Tunnel Defender residual rocket.
        "GLAInfantryTunnelDefender"
        | "GLA_RPGTrooper"
        | "GLA_RPG"
        | "GLA_TunnelDefender"
        | "TestRPGTrooper"
        | "TestRPG"
        | "TestTunnelDefender"
        | "Chem_GLAInfantryTunnelDefender"
        | "Demo_GLAInfantryTunnelDefender"
        | "Slth_GLAInfantryTunnelDefender"
        | "GC_Slth_GLAInfantryTunnelDefender"
        | "GC_Chem_GLAInfantryTunnelDefender" => Some(TUNNEL_DEFENDER_ROCKET_WEAPON),
        // Generals leftover GLALightTank residual (retail PRIMARY CrusaderTankGun).
        "GLALightTank" | "GLA_LightTank" | "TestLightTank" => Some(CRUSADER_TANK_GUN),
        // GLA Combat Cycle residual: default InitialPayload Rebel MG.
        // Empty bike is PRIMARY NONE; spawn residual binds rebel weapon.
        "GLAVehicleCombatBike"
        | "GLA_CombatBike"
        | "TestCombatBike"
        | "TestCombatCycle"
        | "Chem_GLAVehicleCombatBike"
        | "Demo_GLAVehicleCombatBike"
        | "Slth_GLAVehicleCombatBike"
        | "GC_Slth_GLAVehicleCombatBike" => Some(REBEL_BIKER_MG),
        "GLAVehicleCombatBikeRocket"
        | "Chem_GLAVehicleCombatBikeRocket"
        | "Demo_GLAVehicleCombatBikeRocket"
        | "Slth_GLAVehicleCombatBikeRocket"
        | "GC_Slth_GLAVehicleCombatBikeRocket" => Some(TUNNEL_DEFENDER_BIKER_ROCKET),
        "GLAVehicleCombatBikeTerrorist"
        | "Chem_GLAVehicleCombatBikeTerrorist"
        | "Demo_GLAVehicleCombatBikeTerrorist"
        | "Slth_GLAVehicleCombatBikeTerrorist"
        | "Boss_VehicleCombatBikeTerrorist" => Some(TERRORIST_SUICIDE_WEAPON),
        _ => {
            // Name residual for Laser/Superweapon general variants + Nuke Cannon.
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                return Some(NUKE_CANNON_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_inferno_cannon::is_inferno_cannon_template(template_name) {
                return Some(INFERNO_CANNON_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(template_name) {
                return Some(
                    match crate::game_logic::host_aurora_bomb::aurora_bomb_kind_for_template(
                        template_name,
                    ) {
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::FuelAir => {
                            AIRF_AURORA_BOMB_PRIMARY_WEAPON
                        }
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::FuelAirSupW => {
                            SUPW_AURORA_FUEL_BOMB_WEAPON
                        }
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::Standard => {
                            AURORA_BOMB_PRIMARY_WEAPON
                        }
                    },
                );
            }
            if crate::game_logic::host_bunker_buster::is_bunker_buster_carrier(template_name) {
                return Some(STEALTH_JET_MISSILE_WEAPON);
            }
            if crate::game_logic::host_bunker_buster::is_kill_garrisoned_clearer(template_name) {
                return Some(MICROWAVE_BUILDING_CLEARER_WEAPON);
            }
            if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
                return Some(COMANCHE_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_overlord_addons::is_helix_template(template_name) {
                return Some(HELIX_MINIGUN_WEAPON);
            }
            if crate::game_logic::host_pathfinder::is_pathfinder_template(template_name) {
                return Some(PATHFINDER_SNIPER_WEAPON);
            }
            if crate::game_logic::host_slave_drones::is_hellfire_drone_template(template_name) {
                return Some(HELLFIRE_MISSILE_WEAPON);
            }
            if crate::game_logic::host_slave_drones::is_scout_drone_template(template_name) {
                return None;
            }
            // Sentry without gun upgrade has no residual primary (fail-closed).
            if crate::game_logic::host_sentry_drone::is_sentry_drone_template(template_name) {
                return None;
            }
            // Angry Mob nexus residual aggregate fire weapon (AI range residual).
            if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(template_name) {
                return Some(ANGRY_MOB_RESIDUAL_WEAPON);
            }
            if crate::game_logic::host_rocket_buggy::is_rocket_buggy_template(template_name) {
                return Some(BUGGY_ROCKET_WEAPON);
            }
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                return Some(QUAD_CANNON_GUN);
            }
            if crate::game_logic::host_scud_launcher::is_scud_launcher_template(template_name) {
                return Some(SCUD_GUN_EXPLOSIVE);
            }
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                return Some(TECHNICAL_MACHINE_GUN);
            }
            if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name) {
                return Some(TOXIN_TRUCK_GUN);
            }
            if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
                return Some(SCORPION_TANK_GUN);
            }
            if crate::game_logic::host_tomahawk::is_tomahawk_template(template_name) {
                return Some(TOMAHAWK_MISSILE_WEAPON);
            }
            if crate::game_logic::host_raptor::is_raptor_template(template_name) {
                let king = crate::game_logic::host_raptor::is_king_raptor_template(template_name);
                return Some(crate::game_logic::host_raptor::raptor_weapon_name(king));
            }
            if crate::game_logic::host_slave_drones::is_battle_drone_template(template_name) {
                return Some(BATTLE_DRONE_MACHINE_GUN);
            }
            if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
                return Some(OVERLORD_TANK_GUN);
            }
            if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
                return Some(JARMEN_KELL_RIFLE);
            }
            if crate::game_logic::host_marauder::is_marauder_template(template_name) {
                return Some(MARAUDER_TANK_GUN);
            }
            if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
                return Some(TERRORIST_SUICIDE_WEAPON);
            }
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                return Some(MISSILE_DEFENDER_MISSILE_WEAPON);
            }
            if crate::game_logic::host_battlemaster::is_battlemaster_template(template_name) {
                return Some(BATTLE_MASTER_TANK_GUN);
            }
            if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
                return Some(DRAGON_TANK_FLAME_WEAPON);
            }
            if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
                return Some(GATTLING_TANK_GUN);
            }
            if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                return Some(MINIGUNNER_GUN);
            }
            if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
                return Some(TUNNEL_DEFENDER_ROCKET_WEAPON);
            }
            if let Some(w) =
                crate::game_logic::host_usa_tanks::primary_weapon_name_for_usa_tank(template_name)
            {
                return Some(w);
            }
            if crate::game_logic::host_avenger::is_avenger_template(template_name) {
                return Some(AVENGER_TARGET_DESIGNATOR);
            }
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                return Some(HUMVEE_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                return Some(
                    match crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                        template_name,
                    ) {
                        crate::game_logic::host_combat_cycle::CombatCycleRider::TunnelDefender => {
                            TUNNEL_DEFENDER_BIKER_ROCKET
                        }
                        crate::game_logic::host_combat_cycle::CombatCycleRider::Terrorist => {
                            TERRORIST_SUICIDE_WEAPON
                        }
                        _ => REBEL_BIKER_MG,
                    },
                );
            }
            crate::game_logic::host_base_defense::primary_weapon_name_for_defense(template_name)
        }
    }
}

/// Look up the retail secondary weapon template name for a host unit template.
/// Fail-closed: only known multi-slot units; not full WeaponSet upgrade matrices.
pub fn secondary_weapon_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(RANGER_SECONDARY_WEAPON),
        "USA_MissileDefender"
        | "AmericaInfantryMissileDefender"
        | "TestMissileDefender"
        | "SupW_AmericaInfantryMissileDefender" => Some(MISSILE_DEFENDER_LASER_GUIDED_WEAPON),
        "USA_Humvee" | "AmericaVehicleHumvee" | "TestHumvee" | "GoldenHumvee" => {
            Some(HUMVEE_SECONDARY_WEAPON)
        }
        "USA_Avenger" | "AmericaTankAvenger" | "AmericaVehicleAvenger" | "TestAvenger" => {
            Some(AVENGER_AIR_LASER)
        }
        "USA_Patriot"
        | "USA_PatriotMissile"
        | "AmericaPatriotBattery"
        | "PatriotMissile"
        | "TestPatriot" => Some(PATRIOT_SECONDARY_WEAPON),
        "Lazr_AmericaPatriotBattery" | "Lazr_PatriotMissileSystem" | "TestLazrPatriot" => {
            Some(LAZR_PATRIOT_SECONDARY_WEAPON)
        }
        "SupW_AmericaPatriotBattery"
        | "SupW_PatriotMissileSystem"
        | "TestSupWPatriot"
        | "TestEmpPatriot" => Some(SUPW_PATRIOT_SECONDARY_WEAPON),
        "GLA_StingerSite"
        | "GLAStingerSite"
        | "Chem_GLAStingerSite"
        | "Demo_GLAStingerSite"
        | "Slth_GLAStingerSite"
        | "GC_Slth_GLAStingerSite"
        | "GC_Chem_GLAStingerSite"
        | "TestStingerSite" => Some(STINGER_SECONDARY_WEAPON),
        "China_GattlingCannon"
        | "ChinaGattlingCannon"
        | "Nuke_ChinaGattlingCannon"
        | "Tank_ChinaGattlingCannon"
        | "Infa_ChinaGattlingCannon"
        | "TestGattlingCannon" => Some(GATTLING_BUILDING_SECONDARY_WEAPON),
        // Neutron shells are PLAYER_UPGRADE residual (Upgrade_ChinaNeutronShells) —
        // not bound at create; research equips secondary (parity with rocket pods).
        // Rocket pods are PLAYER_UPGRADE residual — not bound at create; research equips.
        // Comanche residual SECONDARY anti-tank until rocket-pods upgrade replaces slot.
        "AmericaVehicleComanche"
        | "USA_Comanche"
        | "TestComanche"
        | "AirF_AmericaVehicleComanche"
        | "SupW_AmericaVehicleComanche"
        | "Lazr_AmericaVehicleComanche" => Some(COMANCHE_ANTITANK_WEAPON),
        // Quad Cannon residual AA secondary (ground primary + air secondary).
        "GLAVehicleQuadCannon"
        | "GLA_QuadCannon"
        | "TestQuadCannon"
        | "Chem_GLAVehicleQuadCannon"
        | "Demo_GLAVehicleQuadCannon"
        | "Slth_GLAVehicleQuadCannon" => Some(QUAD_CANNON_GUN_AIR),
        // SCUD residual toxin secondary (Anthrax swaps warhead residual).
        "GLAVehicleScudLauncher"
        | "GLA_ScudLauncher"
        | "TestScudLauncher"
        | "Chem_GLAVehicleScudLauncher"
        | "Demo_GLAVehicleScudLauncher"
        | "Slth_GLAVehicleScudLauncher" => Some(SCUD_GUN_TOXIN),
        // Toxin Tractor residual contaminate spray secondary.
        "GLAVehicleToxinTruck"
        | "GLA_ToxinTruck"
        | "TestToxinTruck"
        | "Chem_GLAVehicleToxinTruck"
        | "Demo_GLAVehicleToxinTruck"
        | "Slth_GLAVehicleToxinTruck" => Some(TOXIN_TRUCK_SPRAYER),
        // China Gattling Tank residual AA secondary.
        "ChinaTankGattling"
        | "China_GattlingTank"
        | "TestGattlingTank"
        | "Tank_ChinaTankGattling"
        | "Nuke_ChinaTankGattling"
        | "Infa_ChinaTankGattling" => Some(GATTLING_TANK_GUN_AIR),
        // China MiniGunner residual AA secondary.
        "Infa_ChinaInfantryMiniGunner"
        | "China_MiniGunner"
        | "TestMiniGunner"
        | "ChinaInfantryMiniGunner" => Some(MINIGUNNER_GUN_AIR),
        _ => {
            // Nuke Cannon neutron secondary is upgrade-gated — see comment above.
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                Some(QUAD_CANNON_GUN_AIR)
            } else if crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                template_name,
            ) {
                Some(SCUD_GUN_TOXIN)
            } else if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(
                template_name,
            ) {
                Some(TOXIN_TRUCK_SPRAYER)
            } else if crate::game_logic::host_base_defense::is_gattling_cannon_structure(
                template_name,
            ) {
                Some(GATTLING_BUILDING_SECONDARY_WEAPON)
            } else if crate::game_logic::host_base_defense::is_patriot_battery_structure(
                template_name,
            ) {
                Some(
                    crate::game_logic::host_base_defense::secondary_weapon_name_for_defense(
                        template_name,
                    )
                    .unwrap_or(PATRIOT_SECONDARY_WEAPON),
                )
            } else if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name)
            {
                Some(STINGER_SECONDARY_WEAPON)
            } else if crate::game_logic::host_missile_defender::is_missile_defender_template(
                template_name,
            ) {
                Some(MISSILE_DEFENDER_LASER_GUIDED_WEAPON)
            } else if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(
                template_name,
            ) {
                // Comanche residual SECONDARY anti-tank; rocket pods replace after upgrade.
                Some(COMANCHE_ANTITANK_WEAPON)
            } else if crate::game_logic::host_gattling_tank::is_gattling_tank_template(
                template_name,
            ) {
                Some(GATTLING_TANK_GUN_AIR)
            } else if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                Some(MINIGUNNER_GUN_AIR)
            } else {
                None
            }
        }
    }
}
