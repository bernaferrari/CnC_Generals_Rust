// C++ ScriptEngine::init, in condition enum ordinal order.
use super::{Definition, ParameterType};
pub(super) const DEFINITIONS: [Definition; super::ConditionType::NumItems as usize] = [
    // 0: CONDITION_FALSE
    Definition::new("Scripting_/ False.", &[], &["False."]),
    // 1: COUNTER
    Definition::new(
        "Scripting_/ Counter compared to a value.",
        &[
            ParameterType::Counter,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &["Counter ", " IS ", " "],
    ),
    // 2: FLAG
    Definition::new(
        "Scripting_/ Flag compared to a value.",
        &[ParameterType::Flag, ParameterType::Boolean],
        &[" ", " IS "],
    ),
    // 3: CONDITION_TRUE
    Definition::new("Scripting_/ True.", &[], &["True."]),
    // 4: TIMER_EXPIRED
    Definition::new(
        "Scripting_/ Timer expired.",
        &[ParameterType::Counter],
        &["Timer ", " has expired."],
    ),
    // 5: PLAYER_ALL_DESTROYED
    Definition::new(
        "Player_/ All destroyed.",
        &[ParameterType::Side],
        &["Everything belonging to  ", " has been destroyed."],
    ),
    // 6: PLAYER_ALL_BUILDFACILITIES_DESTROYED
    Definition::new(
        "Player_/ All factories destroyed.",
        &[ParameterType::Side],
        &["All factories belonging to  ", " have been destroyed."],
    ),
    // 7: TEAM_INSIDE_AREA_PARTIALLY
    Definition::new(
        "Team_/ Team has units in an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &[" ", " has one or more units in ", " (", ")."],
    ),
    // 8: TEAM_DESTROYED
    Definition::new(
        "Team_/ Team is destroyed.",
        &[ParameterType::Team],
        &[" ", " has been destroyed."],
    ),
    // 9: CAMERA_MOVEMENT_FINISHED
    Definition::new(
        "Camera_/ Camera movement finished.",
        &[],
        &["The camera movement has finished."],
    ),
    // 10: TEAM_HAS_UNITS
    Definition::new(
        "Team_/ Team has units.",
        &[ParameterType::Team],
        &[" ", " has one or more units."],
    ),
    // 11: TEAM_STATE_IS
    Definition::new(
        "Team_/ Team state is.",
        &[ParameterType::Team, ParameterType::TeamState],
        &[" ", " state IS "],
    ),
    // 12: TEAM_STATE_IS_NOT
    Definition::new(
        "Team_/ Team state is not.",
        &[ParameterType::Team, ParameterType::TeamState],
        &[" ", " state IS NOT "],
    ),
    // 13: NAMED_INSIDE_AREA
    Definition::new(
        "Unit_/ Unit inside an area.",
        &[ParameterType::Unit, ParameterType::TriggerArea],
        &[" ", " is inside "],
    ),
    // 14: NAMED_OUTSIDE_AREA
    Definition::new(
        "Unit_/ Unit outside an area.",
        &[ParameterType::Unit, ParameterType::TriggerArea],
        &[" ", " is outside "],
    ),
    // 15: NAMED_DESTROYED
    Definition::new(
        "Unit_/ Unit is destroyed.",
        &[ParameterType::Unit],
        &[" ", " has been destroyed."],
    ),
    // 16: NAMED_NOT_DESTROYED
    Definition::new(
        "Unit_/ Unit exists and is alive.",
        &[ParameterType::Unit],
        &[" ", " exists and is alive."],
    ),
    // 17: TEAM_INSIDE_AREA_ENTIRELY
    Definition::new(
        "Team_/ Team completely inside an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &[" ", " is all inside ", " (", ")."],
    ),
    // 18: TEAM_OUTSIDE_AREA_ENTIRELY
    Definition::new(
        "Team_/ Team is completely outside an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &[" ", " is completely outside ", " (", ")."],
    ),
    // 19: NAMED_ATTACKED_BY_OBJECTTYPE
    Definition::new(
        "Unit_/ Unit is attacked by a specific unit type.",
        &[ParameterType::Unit, ParameterType::ObjectType],
        &[" ", " has been attacked by a(n) "],
    ),
    // 20: TEAM_ATTACKED_BY_OBJECTTYPE
    Definition::new(
        "Team_/ Team is attacked by a specific unit type.",
        &[ParameterType::Team, ParameterType::ObjectType],
        &[" ", " has been attacked by a(n) "],
    ),
    // 21: NAMED_ATTACKED_BY_PLAYER
    Definition::new(
        "Unit_/ Unit has been attacked by a player.",
        &[ParameterType::Unit, ParameterType::Side],
        &[" ", " has been attacked by "],
    ),
    // 22: TEAM_ATTACKED_BY_PLAYER
    Definition::new(
        "Team_/ Team has been attacked by a player.",
        &[ParameterType::Team, ParameterType::Side],
        &[" ", " has been attacked by "],
    ),
    // 23: BUILT_BY_PLAYER
    Definition::new(
        "Player_/ Player has built an object type.",
        &[ParameterType::ObjectType, ParameterType::Side],
        &[" ", " has been built by "],
    ),
    // 24: NAMED_CREATED
    Definition::new(
        "Unit_/ Unit has been created.",
        &[ParameterType::Unit],
        &[" ", " has been created."],
    ),
    // 25: TEAM_CREATED
    Definition::new(
        "Team_/ Team has been created.",
        &[ParameterType::Team],
        &[" ", " has been created."],
    ),
    // 26: PLAYER_HAS_CREDITS
    Definition::new(
        "Player_/ Player has (comparison) to a number of credits.",
        &[
            ParameterType::Int,
            ParameterType::Comparison,
            ParameterType::Side,
        ],
        &[" ", " is ", " the number of credits possessed by "],
    ),
    // 27: NAMED_DISCOVERED
    Definition::new(
        "Player_/ Player has discovered a specific unit.",
        &[ParameterType::Unit, ParameterType::Side],
        &[" ", " has been discovered by "],
    ),
    // 28: TEAM_DISCOVERED
    Definition::new(
        "Player_/ Player has discovered a team.",
        &[ParameterType::Team, ParameterType::Side],
        &[" ", " has been discovered by "],
    ),
    // 29: MISSION_ATTEMPTS
    Definition::new(
        "Player_/ Player has attempted the mission a number of times.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " has attempted the mission ", " ", " times."],
    ),
    // 30: NAMED_OWNED_BY_PLAYER
    Definition::new(
        "Player_/ Player owns the specific Unit.",
        &[ParameterType::Unit, ParameterType::Side],
        &[" ", " is owned by "],
    ),
    // 31: TEAM_OWNED_BY_PLAYER
    Definition::new(
        "Player_/ Player owns a specific team.",
        &[ParameterType::Team, ParameterType::Side],
        &[" ", " is owned by "],
    ),
    // 32: PLAYER_HAS_N_OR_FEWER_BUILDINGS
    Definition::new(
        "Player_/ Player currently owns N or fewer buildings.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " currently owns ", " or fewer buildings."],
    ),
    // 33: PLAYER_HAS_POWER
    Definition::new(
        "Player_/ Player's base currently has power.",
        &[ParameterType::Side],
        &[" ", " buildings are powered."],
    ),
    // 34: NAMED_REACHED_WAYPOINTS_END
    Definition::new(
        "Unit_/ Unit has reached the end of a specific waypoint path.",
        &[ParameterType::Unit, ParameterType::WaypointPath],
        &[" ", " has reached the end of "],
    ),
    // 35: TEAM_REACHED_WAYPOINTS_END
    Definition::new(
        "Team_/ Team has reached the end of a specific waypoint path.",
        &[ParameterType::Team, ParameterType::WaypointPath],
        &[" ", " has reached the end of "],
    ),
    // 36: NAMED_SELECTED
    Definition::new(
        "Unit_/ Unit currently selected.",
        &[ParameterType::Unit],
        &[" ", " is currently selected."],
    ),
    // 37: NAMED_ENTERED_AREA
    Definition::new(
        "Unit_/ Unit enters an area.",
        &[ParameterType::Unit, ParameterType::TriggerArea],
        &[" ", " enters "],
    ),
    // 38: NAMED_EXITED_AREA
    Definition::new(
        "Unit_/ Unit exits an area.",
        &[ParameterType::Unit, ParameterType::TriggerArea],
        &[" ", " exits "],
    ),
    // 39: TEAM_ENTERED_AREA_ENTIRELY
    Definition::new(
        "Team_/ Team entirely enters an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &[" ", " all enter ", " (", ")."],
    ),
    // 40: TEAM_ENTERED_AREA_PARTIALLY
    Definition::new(
        "Team_/ One unit enters an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &["One unit from ", " enters ", " (", ")."],
    ),
    // 41: TEAM_EXITED_AREA_ENTIRELY
    Definition::new(
        "Team_/ Team entirely exits an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &[" ", " all exit ", " (", ")."],
    ),
    // 42: TEAM_EXITED_AREA_PARTIALLY
    Definition::new(
        "Team_/ One unit exits an area.",
        &[
            ParameterType::Team,
            ParameterType::TriggerArea,
            ParameterType::SurfacesAllowed,
        ],
        &["One unit from ", " exits ", " (", ")."],
    ),
    // 43: MULTIPLAYER_ALLIED_VICTORY
    Definition::new(
        "Multiplayer_/ Multiplayer allied victory.",
        &[],
        &["The multiplayer game has ended in victory for the local player and his allies."],
    ),
    // 44: MULTIPLAYER_ALLIED_DEFEAT
    Definition::new(
        "Multiplayer_/ Multiplayer allied defeat.",
        &[],
        &["The multiplayer game has ended in defeat for the local player and his allies."],
    ),
    // 45: MULTIPLAYER_PLAYER_DEFEAT
    Definition::new(
        "Multiplayer_/ Multiplayer local player defeat check.",
        &[],
        &[
            "Everything belonging to the local player has been destroyed, but his allies may or may not have been defeated.",
        ],
    ),
    // 46: PLAYER_HAS_NO_POWER
    Definition::new(
        "Player_/ Player's base currently has no power.",
        &[ParameterType::Side],
        &[" ", " buildings are not powered."],
    ),
    // 47: HAS_FINISHED_VIDEO
    Definition::new(
        "Multimedia_/ Video has completed playing.",
        &[ParameterType::Movie],
        &[" ", " has completed playing."],
    ),
    // 48: HAS_FINISHED_SPEECH
    Definition::new(
        "Multimedia_/ Speech has completed playing.",
        &[ParameterType::Dialog],
        &[" ", " has completed playing."],
    ),
    // 49: HAS_FINISHED_AUDIO
    Definition::new(
        "Multimedia_/ Sound has completed playing.",
        &[ParameterType::Sound],
        &[" ", " has completed playing."],
    ),
    // 50: BUILDING_ENTERED_BY_PLAYER
    Definition::new(
        "Player_/ Player has entered a specific building.",
        &[ParameterType::Side, ParameterType::Unit],
        &[" ", " has entered building named "],
    ),
    // 51: ENEMY_SIGHTED
    Definition::new(
        "Unit_/ Unit has sighted a(n) friendly/neutral/enemy unit belonging to a side.",
        &[
            ParameterType::Unit,
            ParameterType::Relation,
            ParameterType::Side,
        ],
        &[" ", " sees a(n) ", " unit belonging to ", "."],
    ),
    // 52: UNIT_HEALTH
    Definition::new(
        "Unit_/ Unit health % compared to a value.",
        &[
            ParameterType::Unit,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " Health IS ", " ", " percent."],
    ),
    // 53: BRIDGE_REPAIRED
    Definition::new(
        "Unit_/ Bridge is repaired.",
        &[ParameterType::Bridge],
        &[" ", " has been repaired."],
    ),
    // 54: BRIDGE_BROKEN
    Definition::new(
        "Unit_/ Bridge is broken.",
        &[ParameterType::Bridge],
        &[" ", " has been broken."],
    ),
    // 55: NAMED_DYING
    Definition::new(
        "Unit_/ Unit is dying.",
        &[ParameterType::Unit],
        &[" ", " has been killed, but still on screen."],
    ),
    // 56: NAMED_TOTALLY_DEAD
    Definition::new(
        "Unit_/ Unit is finished dying.",
        &[ParameterType::Unit],
        &[" ", " has been killed, and is finished dying."],
    ),
    // 57: PLAYER_HAS_OBJECT_COMPARISON
    Definition::new(
        "Player_/ Player has (comparison) unit type.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
            ParameterType::ObjectType,
        ],
        &[" ", " has ", " ", " unit or structure of type "],
    ),
    // 58: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 59: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 60: PLAYER_TRIGGERED_SPECIAL_POWER
    Definition::new(
        "Player_/ Player starts using a special power.",
        &[ParameterType::Side, ParameterType::SpecialPower],
        &["Player ", " starts using ", "."],
    ),
    // 61: PLAYER_COMPLETED_SPECIAL_POWER
    Definition::new(
        "Player_/ Player completed using a special power.",
        &[ParameterType::Side, ParameterType::SpecialPower],
        &["Player ", " completed using ", "."],
    ),
    // 62: PLAYER_MIDWAY_SPECIAL_POWER
    Definition::new(
        "Player_/ Player is midway through using a special power.",
        &[ParameterType::Side, ParameterType::SpecialPower],
        &["Player ", " is midway using ", "."],
    ),
    // 63: PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED
    Definition::new(
        "Player_/ Player start using a special power from a named unit.",
        &[
            ParameterType::Side,
            ParameterType::SpecialPower,
            ParameterType::Unit,
        ],
        &["Player ", " starts using ", " from ", "."],
    ),
    // 64: PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED
    Definition::new(
        "Player_/ Player completed using a special power from a named unit.",
        &[
            ParameterType::Side,
            ParameterType::SpecialPower,
            ParameterType::Unit,
        ],
        &["Player ", " completed using ", " from ", "."],
    ),
    // 65: PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED
    Definition::new(
        "Player_/ Player is midway through using a special power from a named unit.",
        &[
            ParameterType::Side,
            ParameterType::SpecialPower,
            ParameterType::Unit,
        ],
        &["Player ", " is midway using ", " from ", "."],
    ),
    // 66: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 67: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 68: PLAYER_BUILT_UPGRADE
    Definition::new(
        "Player_/ Player built an upgrade.",
        &[ParameterType::Side, ParameterType::Upgrade],
        &["Player ", " built ", "."],
    ),
    // 69: PLAYER_BUILT_UPGRADE_FROM_NAMED
    Definition::new(
        "Player_/ Player built an upgrade from a named unit.",
        &[
            ParameterType::Side,
            ParameterType::Upgrade,
            ParameterType::Unit,
        ],
        &["Player ", " built ", " from ", "."],
    ),
    // 70: PLAYER_DESTROYED_N_BUILDINGS_PLAYER
    Definition::new(
        "Player_/ Player destroyed N or more of an opponent's buildings.",
        &[ParameterType::Side, ParameterType::Int, ParameterType::Side],
        &[
            "Player ",
            " destroyed ",
            " or more buildings owned by ",
            ".",
        ],
    ),
    // 71: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 72: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 73: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA
    Definition::new(
        "Player_/ Player has (comparison) unit type in an area.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
            ParameterType::ObjectType,
            ParameterType::TriggerArea,
        ],
        &[" ", " has ", " ", " unit or structure of type ", " in the "],
    ),
    // 74: PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA
    Definition::new(
        "Player_/ Player has (comparison) kind of unit or structure in an area.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
            ParameterType::KindOfParam,
            ParameterType::TriggerArea,
        ],
        &[" ", " has ", " ", " unit or structure with ", " in the "],
    ),
    // 75: UNIT_EMPTIED
    Definition::new(
        "Unit_/ Unit has emptied its contents.",
        &[ParameterType::Unit],
        &[" ", " emptied its contents."],
    ),
    // 76: TYPE_SIGHTED
    Definition::new(
        "Unit_/ Unit has sighted a type of unit belonging to a side.",
        &[
            ParameterType::Unit,
            ParameterType::ObjectType,
            ParameterType::Side,
        ],
        &[" ", " sees a(n) ", " belonging to ", "."],
    ),
    // 77: NAMED_BUILDING_IS_EMPTY
    Definition::new(
        "Unit_/ A specific building is empty.",
        &[ParameterType::Unit],
        &[" ", " is empty."],
    ),
    // 78: PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS
    Definition::new(
        "Player_/ Player currently owns N or fewer faction buildings.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " currently owns ", " or fewer faction buildings."],
    ),
    // 79: UNIT_HAS_OBJECT_STATUS
    Definition::new(
        "Unit_/ Unit has object status.",
        &[ParameterType::Unit, ParameterType::ObjectStatus],
        &[" ", " has "],
    ),
    // 80: TEAM_ALL_HAS_OBJECT_STATUS
    Definition::new(
        "Team_/ Team has object status - all.",
        &[ParameterType::Team, ParameterType::ObjectStatus],
        &[" ", " has "],
    ),
    // 81: TEAM_SOME_HAVE_OBJECT_STATUS
    Definition::new(
        "Team_/ Team has object status - partial.",
        &[ParameterType::Team, ParameterType::ObjectStatus],
        &[" ", " has "],
    ),
    // 82: PLAYER_POWER_COMPARE_PERCENT
    Definition::new(
        "Player_/ Player has (comparison) percent power supply to consumption.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " has ", " ", " percent power supply ratio."],
    ),
    // 83: PLAYER_EXCESS_POWER_COMPARE_VALUE
    Definition::new(
        "Player_/ Player has (comparison) kilowatts excess power supply.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " has ", " ", " excess kilowatts power supply."],
    ),
    // 84: SKIRMISH_SPECIAL_POWER_READY
    Definition::new(
        "Skirmish_/ Player's special power is ready to fire.",
        &[ParameterType::Side, ParameterType::SpecialPower],
        &[" ", " is ready to fire ", "."],
    ),
    // 85: SKIRMISH_VALUE_IN_AREA
    Definition::new(
        "Skirmish Only_/ Player has total value in area.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
            ParameterType::TriggerArea,
        ],
        &[" ", " has ", " ", " within area "],
    ),
    // 86: SKIRMISH_PLAYER_FACTION
    Definition::new(
        "Skirmish_/ Player is faction. - untested",
        &[ParameterType::Side, ParameterType::FactionName],
        &[" ", " is "],
    ),
    // 87: SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE
    Definition::new(
        "Skirmish Only_/ Supplies are within specified distance.",
        &[
            ParameterType::Side,
            ParameterType::Real,
            ParameterType::TriggerArea,
            ParameterType::Real,
        ],
        &[" ", " has supplies within ", " of ", " worth at least "],
    ),
    // 88: SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE
    Definition::new(
        "Skirmish Only_/ Tech building is within specified distance.",
        &[
            ParameterType::Side,
            ParameterType::Real,
            ParameterType::TriggerArea,
        ],
        &[" ", " has a tech building within ", " of "],
    ),
    // 89: SKIRMISH_COMMAND_BUTTON_READY_ALL
    Definition::new(
        "Skirmish_/ Command Ability is ready - all.",
        &[
            ParameterType::Side,
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[
            " ",
            "'s ",
            " are ready to use ",
            " (all applicable members).",
        ],
    ),
    // 90: SKIRMISH_COMMAND_BUTTON_READY_PARTIAL
    Definition::new(
        "Skirmish_/ Command Ability is ready - partial",
        &[
            ParameterType::Side,
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", "'s ", " are ready to use ", " (at least one member)."],
    ),
    // 91: SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS
    Definition::new(
        "Skirmish_/ Unowned faction unit -- comparison.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", ". There are ", " ", " unowned faction units."],
    ),
    // 92: SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD
    Definition::new(
        "Skirmish_/ Player has prerequisites to build an object type.",
        &[ParameterType::Side, ParameterType::ObjectType],
        &[" ", " can build ", "."],
    ),
    // 93: SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED
    Definition::new(
        "Skirmish_/ Player has garrisoned buildings -- comparison.",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " has ", " ", " garrisoned buildings."],
    ),
    // 94: SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS
    Definition::new(
        "Skirmish_/ Player has captured units -- comparison",
        &[
            ParameterType::Side,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " has captured ", " ", " units."],
    ),
    // 95: SKIRMISH_NAMED_AREA_EXIST
    Definition::new(
        "Skirmish_/ Area exists.",
        &[ParameterType::Side, ParameterType::TriggerArea],
        &[" ", ". ", " exists."],
    ),
    // 96: SKIRMISH_PLAYER_HAS_UNITS_IN_AREA
    Definition::new(
        "Skirmish_/ Player has units in an area",
        &[ParameterType::Side, ParameterType::TriggerArea],
        &[" ", " has units in ", "."],
    ),
    // 97: SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER
    Definition::new(
        "Skirmish_/ Player has been attacked by player.",
        &[ParameterType::Side, ParameterType::Side],
        &[" ", " has been attacked by ", "."],
    ),
    // 98: SKIRMISH_PLAYER_IS_OUTSIDE_AREA
    Definition::new(
        "Skirmish_/ Player doesn't have units in an area.",
        &[ParameterType::Side, ParameterType::TriggerArea],
        &[" ", " has doesn't have units in ", "."],
    ),
    // 99: SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER
    Definition::new(
        "Skirmish_/ Player has discovered another player.",
        &[ParameterType::Side, ParameterType::Side],
        &[" ", " has discovered ", "."],
    ),
    // 100: PLAYER_ACQUIRED_SCIENCE
    Definition::new(
        "Player_/ Player acquired a Science.",
        &[ParameterType::Side, ParameterType::Science],
        &["Player ", " acquired ", "."],
    ),
    // 101: PLAYER_HAS_SCIENCEPURCHASEPOINTS
    Definition::new(
        "Player_/ Player has a certain number of Science Purchase Points available.",
        &[ParameterType::Side, ParameterType::Int],
        &[
            "Player ",
            " has at least ",
            " Science Purchase Points available.",
        ],
    ),
    // 102: PLAYER_CAN_PURCHASE_SCIENCE
    Definition::new(
        "Player_/ Player can purchase a particular Science (has all prereqs & points).",
        &[ParameterType::Side, ParameterType::Science],
        &["Player ", " can purchase ", "."],
    ),
    // 103: MUSIC_TRACK_HAS_COMPLETED
    Definition::new(
        "Multimedia_/ Music track has completed some number of times.",
        &[ParameterType::Music, ParameterType::Int],
        &[
            " ",
            " has completed at least ",
            " times. (NOTE: This can only be used to start other music. USING THIS SCRIPT IN ANY OTHER WAY WILL CAUSE REPLAYS TO NOT WORK.)",
        ],
    ),
    // 104: PLAYER_LOST_OBJECT_TYPE
    Definition::new(
        "Player_/ Player has lost an object of type.",
        &[ParameterType::Side, ParameterType::ObjectType],
        &[
            " ",
            " has lost an object of type ",
            " (can be an object type list).",
        ],
    ),
    // 105: SUPPLY_SOURCE_SAFE
    Definition::new(
        "Skirmish_/ Supply source is safe.",
        &[ParameterType::Side, ParameterType::Int],
        &[
            " ",
            " closest supply src with at least ",
            " available resources is SAFE from enemy influence.",
        ],
    ),
    // 106: SUPPLY_SOURCE_ATTACKED
    Definition::new(
        "Skirmish_/ Supply source is attacked.",
        &[ParameterType::Side],
        &[" ", " supply source is under attack."],
    ),
    // 107: START_POSITION_IS
    Definition::new(
        "Skirmish_/ Start position.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " starting position is ", " ."],
    ),
    // 108: NAMED_HAS_FREE_CONTAINER_SLOTS
    Definition::new(
        "Unit_/ Unit has free container slots.",
        &[ParameterType::Unit],
        &[" ", " has free container slots."],
    ),
];
