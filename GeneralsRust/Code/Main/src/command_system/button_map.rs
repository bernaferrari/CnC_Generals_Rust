use super::*;

/// CommandSet INI matrix / context-sensitive ControlBar.cpp.
pub fn command_type_from_button_name(name: &str) -> Option<CommandType> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    let lower: String = n
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    // Drop optional "command" prefix residual.
    let key = lower.strip_prefix("command").unwrap_or(lower.as_str());

    match key {
        "stop" => Some(CommandType::Stop),
        "scatter" => Some(CommandType::Scatter),
        "tighten" | "tightentoposition" | "gatherclick" => Some(CommandType::TightenToPosition {
            destination: glam::Vec3::ZERO,
        }),
        "attackteam" => Some(CommandType::AttackTeam {
            team: 0,
            max_shots: -1,
        }),
        "overridespecialpowerdestination" | "overridespdest" => {
            Some(CommandType::OverrideSpecialPowerDestination {
                location: glam::Vec3::ZERO,
            })
        }
        "setweaponsetflag" | "weaponsetflag" => Some(CommandType::SetWeaponSetFlag {
            flag: 0,
            enabled: true,
        }),
        "followwaypointpath" | "followwaypoints" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: false,
            as_team: false,
        }),
        "followwaypointpathexact" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: false,
        }),
        "followwaypointpathasteam" | "followwaypointsasteam" => {
            Some(CommandType::FollowWaypointPath {
                waypoints: Vec::new(),
                exact: false,
                as_team: true,
            })
        }
        "followwaypointpathasteamexact" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: true,
        }),
        "attackfollowwaypointpath" | "attackfollowwaypoints" => {
            Some(CommandType::AttackFollowWaypointPath {
                waypoints: Vec::new(),
                exact: false,
                as_team: false,
            })
        }
        "attackfollowwaypointpathasteam" => Some(CommandType::AttackFollowWaypointPath {
            waypoints: Vec::new(),
            exact: false,
            as_team: true,
        }),
        "attackfollowwaypointpathexact" => Some(CommandType::AttackFollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: false,
        }),
        "surrender" => Some(CommandType::Surrender { surrendered: true }),
        "unsurrender" => Some(CommandType::Surrender { surrendered: false }),
        "docommandbutton" => Some(CommandType::DoCommandButton {
            button: String::new(),
        }),
        "executerailedtransport" | "railedtransport" => Some(CommandType::ExecuteRailedTransport),
        "deploy" => Some(CommandType::Deploy),
        "cheer" | "allcheer" | "groupcheer" => Some(CommandType::Cheer),
        "createformation" | "formation" => Some(CommandType::CreateFormation),
        "viewcommandcenter" | "centerbase" => Some(CommandType::ViewCommandCenter),
        "viewlastradarevent" | "gotoradarevent" => Some(CommandType::ViewLastRadarEvent),
        "placebeacon" | "beacon" => Some(CommandType::PlaceBeacon {
            location: glam::Vec3::ZERO, // filled by map click
            text: String::new(),
        }),
        "removebeacon" | "deletebeacon" | "beacondelete" => Some(CommandType::RemoveBeacon),
        "setbeacontext" | "beacontext" | "clearbeacontext" => Some(CommandType::SetBeaconText {
            text: String::new(),
        }),
        "selfdestruct" | "surrenderobserver" => Some(CommandType::SelfDestruct {
            transfer_to_ally: true,
        }),
        "enableretaliation" | "retaliationmode" => Some(CommandType::EnableRetaliationMode {
            player_index: 0,
            enabled: true,
        }),
        "attackmove" | "attackmoveto" => Some(CommandType::AttackMoveTo {
            destination: glam::Vec3::ZERO, // filled by dispatch from cursor/world,
            max_shots: -1,
        }),
        "setrallypoint" => Some(CommandType::SetRallyPoint {
            location: glam::Vec3::ZERO, // filled by dispatch
        }),
        "guard" | "guardarea" | "guardposition" => Some(CommandType::Guard {
            target: GuardTarget::Position(glam::Vec3::ZERO),
            mode: crate::game_logic::GuardMode::Normal,
        }),
        "guardwithoutpursuit" | "guard_without_pursuit" => Some(CommandType::Guard {
            target: GuardTarget::Position(glam::Vec3::ZERO),
            mode: crate::game_logic::GuardMode::WithoutPursuit,
        }),
        "guardflying" | "guardflyingunits" | "guardflyingunitsonly" | "guard_flying_units_only" => {
            Some(CommandType::Guard {
                target: GuardTarget::Position(glam::Vec3::ZERO),
                mode: crate::game_logic::GuardMode::FlyingUnitsOnly,
            })
        }
        "patrol" | "hunt" => Some(CommandType::Patrol),
        "attackposition" => Some(CommandType::AttackPosition {
            location: Some(glam::Vec3::ZERO),
            max_shots: -1,
        }),
        "attackself" | "attackownposition" => Some(CommandType::AttackPosition {
            location: None,
            max_shots: -1,
        }),
        "attitudesleep" | "sleep" | "holdfire" => Some(CommandType::AttitudeSleep),
        "attitudepassive" | "passive" | "defend" => Some(CommandType::AttitudePassive),
        "attitudenormal" | "normal" | "normalstance" => Some(CommandType::AttitudeNormal),
        "attitudeaggressive" | "aggressive" | "attackmoveaggression" => {
            Some(CommandType::AttitudeAggressive)
        }
        "evacuate" | "structureexit" => Some(CommandType::Evacuate),
        "movetoandevacuate" | "moveevacuate" => Some(CommandType::MoveToAndEvacuate {
            destination: glam::Vec3::ZERO,
            and_exit: false,
        }),
        "movetoandevacuateandexit" | "moveevacuateexit" => Some(CommandType::MoveToAndEvacuate {
            destination: glam::Vec3::ZERO,
            and_exit: true,
        }),
        "repair" => Some(CommandType::Repair {
            target_id: ObjectId(0),
        }),
        "exit" => Some(CommandType::Exit),
        "sell" => Some(CommandType::Sell {
            object_id: crate::game_logic::ObjectId(0), // filled by dispatch
        }),
        // GeneralsExperience science purchase residual (name filled by UI/hotkey).
        "purchasescience" | "buyscience" => Some(CommandType::PurchaseScience {
            science_name: String::new(),
        }),
        "switchweapons" | "switchweapon" | "toggleweapon" => {
            Some(CommandType::SwitchWeapons { slot: 0 })
        }
        "combatdrop" | "rappell" | "rappel" => Some(CommandType::CombatDrop {
            target: crate::command_system::DropTarget::Location(glam::Vec3::ZERO),
        }),
        "hackinternet" | "internet" | "starthacking" => Some(CommandType::HackInternet),
        "returntobase" | "rtb" | "land" => Some(CommandType::ReturnToBase),
        "resumeconstruction" | "resume" => Some(CommandType::ResumeConstruction {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "returnsupplies" | "returncargo" | "forcesupplyreturn" => Some(CommandType::ReturnSupplies),
        "cleanuparea" | "detox" | "clearhazards" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::None,
        }),
        "clearmines" | "disarmmines" => Some(CommandType::ClearMines),
        "setmineclearingdetail" | "mineclearingdetail" | "mineclearing" => {
            Some(CommandType::SetMineClearingDetail { enabled: true })
        }
        "clearmineclearingdetail" | "nomineclearing" => {
            Some(CommandType::SetMineClearingDetail { enabled: false })
        }
        "goprone" | "prone" | "hitthedirt" => Some(CommandType::GoProne),
        "setweaponlock" | "weaponlock" | "lockweapon" => Some(CommandType::SetWeaponLock {
            slot: 1,      // default secondary lock residual
            lock_type: 2, // permanent
        }),
        "releaseweaponlock" | "unlockweapon" => {
            Some(CommandType::ReleaseWeaponLock { lock_type: 2 })
        }
        "setemoticon" | "emoticon" => Some(CommandType::SetEmoticon {
            name: "Emoticon_Smile".into(),
            duration_frames: 90, // 3s @ 30Hz
        }),
        "attackarea" => Some(CommandType::AttackArea {
            center: glam::Vec3::ZERO,
            radius: 150.0,
            polygon_name: None,
        }),
        // Generic ControlBar SW button residual — power type resolved at arm time.
        "specialpower" | "dospecialpower" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon, // placeholder; engine resolves
            target: PowerTarget::None,
        }),
        // USA Strategy Center battle plans residual (immediate, no map click).
        "initiatebattleplanbombardment" | "battleplanbombardment" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanBombardment,
                target: PowerTarget::None,
            })
        }
        "initiatebattleplanholdtheline" | "battleplanholdtheline" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanHoldTheLine,
                target: PowerTarget::None,
            })
        }
        "initiatebattleplansearchanddestroy" | "battleplansearchanddestroy" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanSearchAndDestroy,
                target: PowerTarget::None,
            })
        }
        // Named superweapon / intel residual button names (map-click arm).
        "spysatellitescan" | "spysatellite" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::None,
        }),
        "ciaintelligence" | "ciaintel" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        }),
        "spydrone" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpyDrone,
            target: PowerTarget::None,
        }),
        "particlecannon" | "fireparticlecannon" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon,
            target: PowerTarget::None,
        }),
        "nuclearmissile" | "launchnuclearmissile" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::NuclearMissile,
            target: PowerTarget::None,
        }),
        "scudstorm" | "launchesudstorm" | "launchscudstorm" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ScudStorm,
            target: PowerTarget::None,
        }),
        "carpetbomb" | "chinacarpetbomb" | "americacarpetbomb" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::CarpetBomb,
                target: PowerTarget::None,
            })
        }
        "artillerybarrage" | "artillery" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Artillery,
            target: PowerTarget::None,
        }),
        "emergencyrepair" | "repairvehicles" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmergencyRepair,
            target: PowerTarget::None,
        }),
        "airstrike" | "spectreairstrike" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Airstrike,
            target: PowerTarget::None,
        }),
        "ambush" | "rebelambush" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Ambush,
            target: PowerTarget::None,
        }),
        "sneakattack" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SneakAttack,
            target: PowerTarget::None,
        }),
        "leafletdrop" | "leaflet" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::LeafletDrop,
            target: PowerTarget::None,
        }),
        "gpsscrambler" | "gps" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::GpsScrambler,
            target: PowerTarget::None,
        }),
        "spectregunship" | "spectre" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpectreGunship,
            target: PowerTarget::None,
        }),
        "anthraxbomb" | "anthrax" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::AnthraxBomb,
            target: PowerTarget::None,
        }),
        "cancelupgrade" => Some(CommandType::CancelUpgrade {
            upgrade_name: String::new(),
        }),
        "cancelunit" | "cancelunitcreate" => Some(CommandType::CancelUnitCreate {
            template_name: String::new(),
        }),
        "cancelconstruction" => Some(CommandType::DozerCancelConstruct {
            object_id: crate::game_logic::ObjectId(0),
        }),
        // Unit special-ability residual (target filled by map click).
        "hijack" | "hijackvehicle" => Some(CommandType::Hijack {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "sabotage" | "sabotagebuilding" => Some(CommandType::Sabotage {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "capturebuilding"
        | "rangercapturebuilding"
        | "redguardcapturebuilding"
        | "rebelcapturebuilding"
        | "blacklotuscapturebuilding" => Some(CommandType::CaptureBuilding {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "snipevehicle" | "jarmenkellsnipe" | "snipe" => Some(CommandType::SnipeVehicle {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "planttimeddemocharge" | "timeddemocharge" | "planttimedcharge" => {
            Some(CommandType::PlantTimedDemoCharge {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "plantremotedemocharge" | "remotedemocharge" | "plantremotecharge" => {
            Some(CommandType::PlantRemoteDemoCharge {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "detonateremotedemocharges" | "detonateremotecharges" => {
            Some(CommandType::DetonateRemoteDemoCharges)
        }
        "stealcashhack" | "blacklotusstealcash" => Some(CommandType::StealCashHack {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "disablevehiclehack" | "blacklotusdisablevehicle" => {
            Some(CommandType::DisableVehicleHack {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "hackerdisablebuilding" | "disablebuilding" => Some(CommandType::HackerDisableBuilding {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "disguiseasvehicle" | "disguise" => Some(CommandType::DisguiseAsVehicle {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "plantboobytrap" | "boobytrap" => Some(CommandType::PlantBoobyTrap {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "converttocarbomb" | "carbomb" => Some(CommandType::ConvertToCarbomb {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "demotertiarysuicide" | "suicidebomb" | "tertiarysuicide" => {
            Some(CommandType::DemoTertiarySuicide)
        }
        "toggleovercharge" | "overcharge" => Some(CommandType::ToggleOvercharge),
        _ => {
            // Command_UpgradeAmericaX / Command_Upgrade_GLA… → Upgrade_AmericaX
            if let Some(rest) = key.strip_prefix("upgrade") {
                if rest.is_empty() {
                    return None;
                }
                // Rebuild from original casing after Command_/Upgrade prefix.
                let stripped = n
                    .trim()
                    .trim_start_matches("Command_")
                    .trim_start_matches("command_")
                    .trim_start_matches("Command")
                    .trim_start_matches("command");
                let body = stripped
                    .strip_prefix("Upgrade_")
                    .or_else(|| stripped.strip_prefix("Upgrade"))
                    .or_else(|| stripped.strip_prefix("upgrade_"))
                    .or_else(|| stripped.strip_prefix("upgrade"))
                    .unwrap_or(rest)
                    .trim_start_matches('_');
                let upgrade_name = format!("Upgrade_{body}");
                Some(CommandType::QueueUpgrade { upgrade_name })
            } else {
                None
            }
        }
    }
}
