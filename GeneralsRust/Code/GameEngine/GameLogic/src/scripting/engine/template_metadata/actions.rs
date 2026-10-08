// C++ ScriptEngine::init, in action enum ordinal order.
use super::{Definition, ParameterType};
pub(super) const DEFINITIONS: [Definition; super::ScriptActionType::NumItems as usize] = [
    // 0: DEBUG_MESSAGE_BOX
    Definition::new(
        "Scripting_/Debug/Display message and pause",
        &[ParameterType::TextString],
        &["Show debug string and pause: "],
    ),
    // 1: SET_FLAG
    Definition::new(
        "Scripting_/Flags/Set flag to value",
        &[ParameterType::Flag, ParameterType::Boolean],
        &["Set ", " to "],
    ),
    // 2: SET_COUNTER
    Definition::new(
        "Scripting_/Counters/Set counter to a value",
        &[ParameterType::Counter, ParameterType::Int],
        &["Set ", " to "],
    ),
    // 3: VICTORY
    Definition::new(
        "Multiplayer_/ Announce victory.",
        &[],
        &["Show 'Victorious' window and end game"],
    ),
    // 4: DEFEAT
    Definition::new(
        "Multiplayer_/ Announce defeat.",
        &[],
        &["Show 'Defeated' window and end game"],
    ),
    // 5: NO_OP
    Definition::new(
        "Scripting_/Debug/Null operation.",
        &[],
        &["Null operation. (Does nothing.)"],
    ),
    // 6: SET_TIMER
    Definition::new(
        "Scripting_/Timer/Frame countdown timer -- set.",
        &[ParameterType::Counter, ParameterType::Int],
        &["Set timer ", " to expire in ", " frames."],
    ),
    // 7: PLAY_SOUND_EFFECT
    Definition::new(
        "Multimedia_/Sound Effect/Play sound effect.",
        &[ParameterType::Sound],
        &["Play ", "."],
    ),
    // 8: ENABLE_SCRIPT
    Definition::new(
        "Scripting_/Script/Enable Script.",
        &[ParameterType::Script],
        &["Enable ", "."],
    ),
    // 9: DISABLE_SCRIPT
    Definition::new(
        "Scripting_/Script/Disable script.",
        &[ParameterType::Script],
        &["Disable ", "."],
    ),
    // 10: CALL_SUBROUTINE
    Definition::new(
        "Scripting_/Script/Run subroutine script.",
        &[ParameterType::ScriptSubroutine],
        &["Run ", "."],
    ),
    // 11: PLAY_SOUND_EFFECT_AT
    Definition::new(
        "Multimedia_/Sound Effect/Play sound effect at waypoint.",
        &[ParameterType::Sound, ParameterType::Waypoint],
        &["Play ", " at ", "."],
    ),
    // 12: DAMAGE_MEMBERS_OF_TEAM
    Definition::new(
        "Team_/Damage/Damage the members of a team.",
        &[ParameterType::Team, ParameterType::Real],
        &["Damage ", ", amount=", " (-1==kill)."],
    ),
    // 13: MOVE_TEAM_TO
    Definition::new(
        "Team_/Move/Set to move to a location.",
        &[ParameterType::Team, ParameterType::Waypoint],
        &["Move ", " to ", "."],
    ),
    // 14: MOVE_CAMERA_TO
    Definition::new(
        "Camera_/Move/Move the camera to a location.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Move camera to ",
            " in ",
            " seconds, camera shutter ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 15: INCREMENT_COUNTER
    Definition::new(
        "Scripting_/Counters/Increment counter.",
        &[ParameterType::Int, ParameterType::Counter],
        &["Add ", " to counter "],
    ),
    // 16: DECREMENT_COUNTER
    Definition::new(
        "Scripting_/Counters/Decrement counter.",
        &[ParameterType::Int, ParameterType::Counter],
        &["Subtract ", " from counter "],
    ),
    // 17: MOVE_CAMERA_ALONG_WAYPOINT_PATH
    Definition::new(
        "Camera_/Move/Move along a waypoint path.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Move along path starting with ",
            " in ",
            " seconds, camera shutter ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 18: ROTATE_CAMERA
    Definition::new(
        "Camera_/Rotate/ Rotate around the current viewpoint.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Rotate ",
            " times, taking ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 19: RESET_CAMERA
    Definition::new(
        "Camera_/Move/ Reset to the default view.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Reset to ",
            ", taking ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 20: SET_MILLISECOND_TIMER
    Definition::new(
        "Scripting_/Timer/Seconds countdown timer -- set.",
        &[ParameterType::Counter, ParameterType::Real],
        &["Set timer ", " to expire in ", " seconds."],
    ),
    // 21: CAMERA_MOD_FREEZE_TIME
    Definition::new(
        "Camera_/Move/Modify/ Freeze time during the camera movement.",
        &[],
        &["Freeze time during the camera movement."],
    ),
    // 22: SET_VISUAL_SPEED_MULTIPLIER
    Definition::new(
        "{Compatibility}_/Multimedia/Modify visual game time.",
        &[ParameterType::Int],
        &[
            "Make visual time ",
            " time normal (1=normal, 2 = twice as fast, ...).",
        ],
    ),
    // 23: CREATE_OBJECT
    Definition::new(
        "Unit_/Spawn/Spawn object.",
        &[
            ParameterType::ObjectType,
            ParameterType::Team,
            ParameterType::Coord3D,
            ParameterType::Angle,
        ],
        &[
            "Spawn object ",
            " in ",
            " at position (",
            "), rotated ",
            " .",
        ],
    ),
    // 24: SUSPEND_BACKGROUND_SOUNDS
    Definition::new(
        "Multimedia_/All Sounds/Suspend all sounds.",
        &[],
        &["Suspend background sounds."],
    ),
    // 25: RESUME_BACKGROUND_SOUNDS
    Definition::new(
        "Multimedia_/All Sounds/Resume all sounds.",
        &[],
        &["Resume background sounds."],
    ),
    // 26: CAMERA_MOD_SET_FINAL_ZOOM
    Definition::new(
        "Camera_/Move/Modify/Set Final zoom for camera movement.",
        &[
            ParameterType::Real,
            ParameterType::Percent,
            ParameterType::Percent,
        ],
        &[
            "Adjust zoom to ",
            " (1.0==max height, 0.0==in the ground) ",
            " ease-in ",
            " ease-out.",
        ],
    ),
    // 27: CAMERA_MOD_SET_FINAL_PITCH
    Definition::new(
        "Camera_/Move/Modify/Set Final pitch for camera movement.",
        &[
            ParameterType::Real,
            ParameterType::Percent,
            ParameterType::Percent,
        ],
        &[
            "Adjust pitch to ",
            " (1.0==default, 0.0==toward horizon, >1 = toward ground) ",
            " ease-in ",
            " ease-out.",
        ],
    ),
    // 28: CAMERA_MOD_FREEZE_ANGLE
    Definition::new(
        "Camera_/Move/Modify/ Freeze camera angle during the camera movement.",
        &[],
        &["Freeze camera angle during the camera movement."],
    ),
    // 29: CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER
    Definition::new(
        "{Compatibility}_/Camera/Modify/Final visual game time for camera movement.",
        &[ParameterType::Int],
        &[
            "Adjust game time to",
            " times normal (1=normal, 2 = twice as fast, ...).",
        ],
    ),
    // 30: CAMERA_MOD_SET_ROLLING_AVERAGE
    Definition::new(
        "Camera_/Move/Modify/ Number of frames to average movements.",
        &[ParameterType::Int],
        &[
            "Average position and angle changes over",
            " frames. (1=no smoothing, 5 = very smooth)",
        ],
    ),
    // 31: CAMERA_MOD_FINAL_LOOK_TOWARD
    Definition::new(
        "{Compatibility}_/Camera/Modify/Move/ Final camera look toward point.",
        &[ParameterType::Waypoint],
        &["Look toward", " at the end of the camera movement."],
    ),
    // 32: CAMERA_MOD_LOOK_TOWARD
    Definition::new(
        "Camera_/Modify/Move/Camera look toward point while moving.",
        &[ParameterType::Waypoint],
        &["Look toward", " during the camera movement."],
    ),
    // 33: TEAM_ATTACK_TEAM
    Definition::new(
        "Team_/Attack/Set to attack -- another team.",
        &[ParameterType::Team, ParameterType::Team],
        &[" ", " begin attack on "],
    ),
    // 34: CREATE_REINFORCEMENT_TEAM
    Definition::new(
        "Team_/ Spawn a reinforcement team.",
        &[ParameterType::Team, ParameterType::Waypoint],
        &["Spawn an instance of ", " at ", "."],
    ),
    // 35: MOVE_CAMERA_TO_SELECTION
    Definition::new(
        "Camera_/Move/Modify/ End movement at selected unit.",
        &[],
        &["End movement at selected unit."],
    ),
    // 36: TEAM_FOLLOW_WAYPOINTS
    Definition::new(
        "Team_/Move/Set to follow a waypoint path.",
        &[
            ParameterType::Team,
            ParameterType::WaypointPath,
            ParameterType::Boolean,
        ],
        &["Have ", " follow ", " , as a team is "],
    ),
    // 37: TEAM_SET_STATE
    Definition::new(
        "Team_/Misc/Team custom state - set state.",
        &[ParameterType::Team, ParameterType::TeamState],
        &["Set ", " to ", "."],
    ),
    // 38: MOVE_NAMED_UNIT_TO
    Definition::new(
        "Unit_/Move/Move a specific unit to a location.",
        &[ParameterType::Unit, ParameterType::Waypoint],
        &["Move ", " to ", "."],
    ),
    // 39: NAMED_ATTACK_NAMED
    Definition::new(
        "Unit_/Attack/Set unit to attack another unit.",
        &[ParameterType::Unit, ParameterType::Unit],
        &[" ", " begin attack on "],
    ),
    // 40: CREATE_NAMED_ON_TEAM_AT_WAYPOINT
    Definition::new(
        "Unit_/Spawn/Spawn -- named unit on a team at a waypoint.",
        &[
            ParameterType::Unit,
            ParameterType::ObjectType,
            ParameterType::Team,
            ParameterType::Waypoint,
        ],
        &["Spawn ", " of type ", " on ", " at waypoint "],
    ),
    // 41: CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT
    Definition::new(
        "Unit_/Spawn/Spawn -- unnamed unit on a team at a waypoint.",
        &[
            ParameterType::ObjectType,
            ParameterType::Team,
            ParameterType::Waypoint,
        ],
        &["Spawn unit of type ", " on ", " at waypoint "],
    ),
    // 42: NAMED_APPLY_ATTACK_PRIORITY_SET
    Definition::new(
        "AttackPrioritySet_/Apply/Unit/Apply unit's attack priority set.",
        &[ParameterType::Unit, ParameterType::AttackPrioritySet],
        &["Have ", " use ", "."],
    ),
    // 43: TEAM_APPLY_ATTACK_PRIORITY_SET
    Definition::new(
        "AttackPrioritySet_/Apply/Team/Apply a team's attack priority set.",
        &[ParameterType::Team, ParameterType::AttackPrioritySet],
        &["Have ", " use ", "."],
    ),
    // 44: SET_BASE_CONSTRUCTION_SPEED
    Definition::new(
        "Player_/AI/Set the delay between building teams.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " seconds between building teams.", ""],
    ),
    // 45: NAMED_SET_ATTITUDE
    Definition::new(
        "Unit_/Mood/Set the general attitude of a specific unit.",
        &[ParameterType::Unit, ParameterType::AiMood],
        &[" ", " changes his attitude to "],
    ),
    // 46: TEAM_SET_ATTITUDE
    Definition::new(
        "Team_/Mood/Set the general attitude of a team.",
        &[ParameterType::Team, ParameterType::AiMood],
        &[" ", " change their attitude to "],
    ),
    // 47: NAMED_ATTACK_AREA
    Definition::new(
        "Unit_/Attack/Set a specific unit to attack a specific trigger area.",
        &[ParameterType::Unit, ParameterType::TriggerArea],
        &[" ", " attacks anything in "],
    ),
    // 48: NAMED_ATTACK_TEAM
    Definition::new(
        "Unit_/Attack/Set a specific unit to attack a team.",
        &[ParameterType::Unit, ParameterType::Team],
        &[" ", " attacks "],
    ),
    // 49: TEAM_ATTACK_AREA
    Definition::new(
        "Team_/Attack/Set to attack -- trigger area.",
        &[ParameterType::Team, ParameterType::TriggerArea],
        &[" ", " attack anything in "],
    ),
    // 50: TEAM_ATTACK_NAMED
    Definition::new(
        "Team_/Attack/Set to attack -- specific unit.",
        &[ParameterType::Team, ParameterType::Unit],
        &[" ", " attacks "],
    ),
    // 51: TEAM_LOAD_TRANSPORTS
    Definition::new(
        "Team_/Transport/Transport -- automatically load.",
        &[ParameterType::Team],
        &[" ", " load into transports."],
    ),
    // 52: NAMED_ENTER_NAMED
    Definition::new(
        "Unit_/Transport/Transport -- load unit into specific.",
        &[ParameterType::Unit, ParameterType::Unit],
        &[" ", " loads into "],
    ),
    // 53: TEAM_ENTER_NAMED
    Definition::new(
        "Team_/Transport/Transport -- load team into specific.",
        &[ParameterType::Team, ParameterType::Unit],
        &[" ", " attempt to load into "],
    ),
    // 54: NAMED_EXIT_ALL
    Definition::new(
        "Unit_/Transport/Transport -- unload units from specific.",
        &[ParameterType::Unit],
        &[" ", " unloads."],
    ),
    // 55: TEAM_EXIT_ALL
    Definition::new(
        "Team_/Transport/Transport -- unload team from all.",
        &[ParameterType::Team],
        &[" ", " unload."],
    ),
    // 56: NAMED_FOLLOW_WAYPOINTS
    Definition::new(
        "Unit_/Move/Set a specific unit to follow a waypoint path.",
        &[ParameterType::Unit, ParameterType::WaypointPath],
        &[" ", " follows waypoints, beginning at "],
    ),
    // 57: NAMED_GUARD
    Definition::new(
        "Unit_/Move/Set to guard.",
        &[ParameterType::Unit],
        &[" ", " begins guarding."],
    ),
    // 58: TEAM_GUARD
    Definition::new(
        "Team_/Guard/Set to guard -- current location.",
        &[ParameterType::Team],
        &[" ", " begins guarding."],
    ),
    // 59: NAMED_HUNT
    Definition::new(
        "Unit_/Hunt/Set a specific unit to hunt.",
        &[ParameterType::Unit],
        &[" ", " begins hunting."],
    ),
    // 60: TEAM_HUNT
    Definition::new(
        "Team_/Hunt/Set to hunt.",
        &[ParameterType::Team],
        &[" ", " begins hunting."],
    ),
    // 61: PLAYER_SELL_EVERYTHING
    Definition::new(
        "Player_/Set/Set a player to sell everything.",
        &[ParameterType::Side],
        &[" ", " sells everything."],
    ),
    // 62: PLAYER_DISABLE_BASE_CONSTRUCTION
    Definition::new(
        "Player_/Build/Set a player to be unable to build buildings.",
        &[ParameterType::Side],
        &[" ", " is unable to build buildings."],
    ),
    // 63: PLAYER_DISABLE_FACTORIES
    Definition::new(
        "Player_/Build/Set a player to be unable to build from a specific building.",
        &[ParameterType::Side, ParameterType::ObjectType],
        &[" ", " is unable to build from "],
    ),
    // 64: PLAYER_DISABLE_UNIT_CONSTRUCTION
    Definition::new(
        "Player_/Build/Set a player to be unable to build units.",
        &[ParameterType::Side],
        &[" ", " is unable to build units."],
    ),
    // 65: PLAYER_ENABLE_BASE_CONSTRUCTION
    Definition::new(
        "Player_/Build/Set a player to be able to build buildings.",
        &[ParameterType::Side],
        &[" ", " is able to build buildings."],
    ),
    // 66: PLAYER_ENABLE_FACTORIES
    Definition::new(
        "Player_/Build/Set a player to be able to build from a specific building.",
        &[ParameterType::Side, ParameterType::ObjectType],
        &[" ", " is able to build from "],
    ),
    // 67: PLAYER_ENABLE_UNIT_CONSTRUCTION
    Definition::new(
        "Player_/Build/Set a player to be able to build units.",
        &[ParameterType::Side],
        &[" ", " is able to build units."],
    ),
    // 68: CAMERA_MOVE_HOME
    Definition::new(
        "Camera_/Move/Move the camera to the home position.",
        &[],
        &["The camera moves to the home base."],
    ),
    // 69: BUILD_TEAM
    Definition::new(
        "Team_/AI/Start building a team.",
        &[ParameterType::Team],
        &["Start building team "],
    ),
    // 70: NAMED_DAMAGE
    Definition::new(
        "Unit_/Damage/Deal damage to a specific unit.",
        &[ParameterType::Unit, ParameterType::Int],
        &[" ", " takes ", " points of damage."],
    ),
    // 71: NAMED_DELETE
    Definition::new(
        "Unit_/Damage or Remove/Delete a specific unit.",
        &[ParameterType::Unit],
        &[" ", " is removed from the world."],
    ),
    // 72: TEAM_DELETE
    Definition::new(
        "Team_/Damage or Remove/Delete a team.",
        &[ParameterType::Team],
        &[" ", " is removed from the world."],
    ),
    // 73: NAMED_KILL
    Definition::new(
        "Unit_/Damage or Remove/Kill a specific unit.",
        &[ParameterType::Unit],
        &[" ", "is dealt a lethal amount of damage."],
    ),
    // 74: TEAM_KILL
    Definition::new(
        "Team_/Damage or Remove/Kill an entire team.",
        &[ParameterType::Team],
        &[" ", " is dealt a lethal amount of damage."],
    ),
    // 75: PLAYER_KILL
    Definition::new(
        "Player_/Damage or Remove/Kill a player.",
        &[ParameterType::Side],
        &[
            "All of ",
            "'s buildings and units are dealt a lethal amount of damage.",
        ],
    ),
    // 76: DISPLAY_TEXT
    Definition::new(
        "User_/String/Display a string.",
        &[ParameterType::LocalizedText],
        &["Displays ", " in the text log and message area."],
    ),
    // 77: CAMEO_FLASH
    Definition::new(
        "User_/Flash/Flash a cameo for a specified amount of time.",
        &[ParameterType::CommandButton, ParameterType::Int],
        &[" ", " flashes for ", " seconds."],
    ),
    // 78: NAMED_FLASH
    Definition::new(
        "User_/Flash/Flash a specific unit for a specified amount of time.",
        &[ParameterType::Unit, ParameterType::Int],
        &[" ", " flashes for ", " seconds."],
    ),
    // 79: TEAM_FLASH
    Definition::new(
        "User_/Flash/Flash a team for a specified amount of time.",
        &[ParameterType::Team, ParameterType::Int],
        &[" ", " flashes for ", " seconds."],
    ),
    // 80: MOVIE_PLAY_FULLSCREEN
    Definition::new(
        "Multimedia_/Movie/Play a movie in fullscreen mode.",
        &[ParameterType::Movie],
        &[" ", " plays fullscreen."],
    ),
    // 81: MOVIE_PLAY_RADAR
    Definition::new(
        "Multimedia_/Movie/Play a movie in the radar.",
        &[ParameterType::Movie],
        &[" ", " plays in the radar window."],
    ),
    // 82: SOUND_PLAY_NAMED
    Definition::new(
        "Multimedia_/Sound Effects/Play a sound as though coming from a specific unit.",
        &[ParameterType::TextString, ParameterType::Unit],
        &[" ", " plays as though coming from "],
    ),
    // 83: SPEECH_PLAY
    Definition::new(
        "Multimedia_/Sound Effects/Play a speech file.",
        &[ParameterType::Dialog, ParameterType::Boolean],
        &[
            " ",
            " plays, allowing overlap ",
            " (true to allow, false to disallow).",
        ],
    ),
    // 84: PLAYER_TRANSFER_OWNERSHIP_PLAYER
    Definition::new(
        "Player_/Transfer/Transfer assets from one player to another player.",
        &[ParameterType::Side, ParameterType::Side],
        &["All assets of ", " are transferred to "],
    ),
    // 85: NAMED_TRANSFER_OWNERSHIP_PLAYER
    Definition::new(
        "Player_/Transfer/Transfer a specific unit to the control of a player.",
        &[ParameterType::Unit, ParameterType::Side],
        &[" ", " is transferred to the command of "],
    ),
    // 86: PLAYER_RELATES_PLAYER
    Definition::new(
        "Player_/Alliances/Change how a player relates to another player.",
        &[
            ParameterType::Side,
            ParameterType::Side,
            ParameterType::Relation,
        ],
        &[" ", " considers ", " to be "],
    ),
    // 87: RADAR_CREATE_EVENT
    Definition::new(
        "Radar_/Create Event/Create a radar event at a specified location.",
        &[ParameterType::Coord3D, ParameterType::RadarEventType],
        &["A radar event occurs at ", " of type "],
    ),
    // 88: RADAR_DISABLE
    Definition::new(
        "Radar_/Control/Disable the radar.",
        &[],
        &["The radar is disabled."],
    ),
    // 89: RADAR_ENABLE
    Definition::new(
        "Radar_/Control/Enable the radar.",
        &[],
        &["The radar is enabled."],
    ),
    // 90: MAP_REVEAL_AT_WAYPOINT
    Definition::new(
        "Map_/Shroud or Reveal/Reveal map at waypoint -- fog.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Side,
        ],
        &[
            "The map is revealed at ",
            " with a radius of ",
            " feet for ",
            ".",
        ],
    ),
    // 91: TEAM_AVAILABLE_FOR_RECRUITMENT
    Definition::new(
        "Team_/AI/Set whether members of a team can be recruited into another team.",
        &[ParameterType::Team, ParameterType::Boolean],
        &[" ", " sets their willingness to join teams to "],
    ),
    // 92: TEAM_COLLECT_NEARBY_FOR_TEAM
    Definition::new(
        "Team_/AI/Set to collect nearby units.",
        &[ParameterType::Team],
        &[" ", " attempts to collect nearby units for a team."],
    ),
    // 93: TEAM_MERGE_INTO_TEAM
    Definition::new(
        "Team_/Merge/Merge a team into another team.",
        &[ParameterType::Team, ParameterType::Team],
        &[" ", " merges onto "],
    ),
    // 94: DISABLE_INPUT
    Definition::new(
        "User_/Input/User input -- disable.",
        &[],
        &["Disable mouse and keyboard input."],
    ),
    // 95: ENABLE_INPUT
    Definition::new(
        "User_/Input/User input -- enable.",
        &[],
        &["Enable mouse and keyboard input."],
    ),
    // 96: PLAYER_HUNT
    Definition::new(
        "Player_/Hunt/Set all of a player's units to hunt.",
        &[ParameterType::Side],
        &[" ", " begins hunting."],
    ),
    // 97: SOUND_AMBIENT_PAUSE
    Definition::new(
        "Multimedia_/SoundEffects/Pause the ambient sounds.",
        &[],
        &["Pause the ambient sounds."],
    ),
    // 98: SOUND_AMBIENT_RESUME
    Definition::new(
        "Multimedia_/SoundEffects/Resume the ambient sounds.",
        &[],
        &["Resume the ambient sounds."],
    ),
    // 99: MUSIC_SET_TRACK
    Definition::new(
        "Multimedia_/Music/Play a music track.",
        &[
            ParameterType::Music,
            ParameterType::Boolean,
            ParameterType::Boolean,
        ],
        &["Play ", " using fadeout (", ") and fadein (", ")."],
    ),
    // 100: SET_TREE_SWAY
    Definition::new(
        "Map_/Environment/Set wind sway amount and direction.",
        &[
            ParameterType::Angle,
            ParameterType::Angle,
            ParameterType::Angle,
            ParameterType::Int,
            ParameterType::Real,
        ],
        &[
            "Set wind direction to ",
            ", amount to sway ",
            ", amount to lean with the wind ",
            ", frames to take to sway once ",
            ", randomness ",
            "(0=lock step, 1=large random variation).",
        ],
    ),
    // 101: DEBUG_STRING
    Definition::new(
        "Scripting_/Debug/Display string",
        &[ParameterType::TextString],
        &["Show debug string without pausing: "],
    ),
    // 102: MAP_REVEAL_ALL
    Definition::new(
        "Map_/Shroud or Reveal/Reveal the entire map for a player.",
        &[ParameterType::Side],
        &["The world is revealed for ", "."],
    ),
    // 103: TEAM_GARRISON_SPECIFIC_BUILDING
    Definition::new(
        "Team_/Garrison/Garrison a specific building with a team.",
        &[ParameterType::Team, ParameterType::Unit],
        &[" ", " enters into building named "],
    ),
    // 104: EXIT_SPECIFIC_BUILDING
    Definition::new(
        "Unit_/Garrison/Empty a specific building.",
        &[ParameterType::Unit],
        &[" ", " empties."],
    ),
    // 105: TEAM_GARRISON_NEAREST_BUILDING
    Definition::new(
        "Team_/Garrison/Garrison a nearby building with a team.",
        &[ParameterType::Team],
        &[" ", " garrison a nearby building."],
    ),
    // 106: TEAM_EXIT_ALL_BUILDINGS
    Definition::new(
        "Team_/Garrison/Exit all buildings a team is in.",
        &[ParameterType::Team],
        &[" ", " exits all buildings."],
    ),
    // 107: NAMED_GARRISON_SPECIFIC_BUILDING
    Definition::new(
        "Unit_/Garrison/Garrison a specific building with a specific unit.",
        &[ParameterType::Unit, ParameterType::Unit],
        &[" ", " garrison building "],
    ),
    // 108: NAMED_GARRISON_NEAREST_BUILDING
    Definition::new(
        "Unit_/Garrison/Garrison a nearby building with a specific unit.",
        &[ParameterType::Unit],
        &[" ", " garrison a nearby building."],
    ),
    // 109: NAMED_EXIT_BUILDING
    Definition::new(
        "Unit_/Garrison/Exit the building the unit is in.",
        &[ParameterType::Unit],
        &[" ", " leaves the building."],
    ),
    // 110: PLAYER_GARRISON_ALL_BUILDINGS
    Definition::new(
        "Player_/Garrison/Garrison as many buildings as player has units for.",
        &[ParameterType::Side],
        &[" ", " garrison buildings."],
    ),
    // 111: PLAYER_EXIT_ALL_BUILDINGS
    Definition::new(
        "Player_/Garrison/All units leave their garrisons.",
        &[ParameterType::Side],
        &[" ", " evacuate."],
    ),
    // 112: TEAM_WANDER
    Definition::new(
        "Team_/Move/Set to follow a waypoint path -- wander.",
        &[ParameterType::Team, ParameterType::WaypointPath],
        &["Have ", " wander along "],
    ),
    // 113: TEAM_PANIC
    Definition::new(
        "Team_/Move/Set to follow a waypoint path -- panic.",
        &[ParameterType::Team, ParameterType::WaypointPath],
        &["Have ", " move in panic along "],
    ),
    // 114: SETUP_CAMERA
    Definition::new(
        "Camera_/Adjust/Set up the camera.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Waypoint,
        ],
        &[
            "Position camera at ",
            ", zoom = ",
            "(0.0 to 1.0), pitch = ",
            "(1.0==default), looking towards ",
            ".",
        ],
    ),
    // 115: CAMERA_LETTERBOX_BEGIN
    Definition::new(
        "Camera_/Letterbox/Start letterbox mode.",
        &[],
        &["Start letterbox mode (hide UI, add border)."],
    ),
    // 116: CAMERA_LETTERBOX_END
    Definition::new(
        "Camera_/ End letterbox mode.",
        &[],
        &["End letterbox mode (show UI, remove border)."],
    ),
    // 117: ZOOM_CAMERA
    Definition::new(
        "Camera_/Adjust/Change the camera zoom.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Change camera zoom to ",
            " in ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 118: PITCH_CAMERA
    Definition::new(
        "Camera_/Adjust/Change the camera pitch.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Change camera pitch to ",
            " in ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 119: CAMERA_FOLLOW_NAMED
    Definition::new(
        "Camera_/Move/Follow a specific unit.",
        &[ParameterType::Unit, ParameterType::Boolean],
        &[
            "Have the camera follow ",
            ".  Snap camera to object is ",
            ".",
        ],
    ),
    // 120: OVERSIZE_TERRAIN
    Definition::new(
        "Camera_/Terrain/Oversize the terrain.",
        &[ParameterType::Int],
        &[
            "Oversize the terrain ",
            " tiles on each side [0 = reset to normal].",
        ],
    ),
    // 121: CAMERA_FADE_ADD
    Definition::new(
        "Camera_/Fade Effects/Fade using an add blend to white.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Int,
        ],
        &[
            "Fade (0-1) from ",
            " to ",
            " adding toward white. Take ",
            " frames to increase, hold for ",
            " fames, and decrease ",
            " frames.",
        ],
    ),
    // 122: CAMERA_FADE_SUBTRACT
    Definition::new(
        "Camera_/Fade Effects/Fade using a subtractive blend to black.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Int,
        ],
        &[
            "Fade (0-1) from ",
            " to ",
            " subtracting toward black. Take ",
            " frames to increase, hold for ",
            " fames, and decrease ",
            " frames.",
        ],
    ),
    // 123: CAMERA_FADE_SATURATE
    Definition::new(
        "Camera_/Fade Effects/Fade using a saturate blend.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Int,
        ],
        &[
            "Fade (0.5-1) from ",
            " to ",
            " increasing saturation. Take ",
            " frames to increase, hold for ",
            " fames, and decrease ",
            " frames.",
        ],
    ),
    // 124: CAMERA_FADE_MULTIPLY
    Definition::new(
        "Camera_/Fade Effects/Fade using a multiply blend to black.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Int,
        ],
        &[
            "Fade (1-0) from ",
            " to ",
            " multiplying toward black. Take ",
            " frames to increase, hold for ",
            " fames, and decrease ",
            " frames.",
        ],
    ),
    // 125: CAMERA_BW_MODE_BEGIN
    Definition::new(
        "Camera_/Fade Effects/Start black & white mode.",
        &[ParameterType::Int],
        &["Frames to fade into black & white mode = "],
    ),
    // 126: CAMERA_BW_MODE_END
    // C++ leaves parameter 0 indeterminate; ScriptActions.cpp reads Int.
    Definition::new(
        "Camera_/Fade Effects/End black & white mode.",
        &[ParameterType::Int],
        &["Frames to fade into color mode = "],
    ),
    // 127: DRAW_SKYBOX_BEGIN
    Definition::new(
        "Camera_/Skybox/Start skybox mode.",
        &[],
        &["Start skybox mode (draw sky background)."],
    ),
    // 128: DRAW_SKYBOX_END
    Definition::new(
        "Camera_/Skybox/End skybox mode.",
        &[],
        &["End skybox mode (draw black background)."],
    ),
    // 129: SET_ATTACK_PRIORITY_THING
    Definition::new(
        "AttackPrioritySet_/Set/Modify a set's priority for a single unit type.",
        &[
            ParameterType::AttackPrioritySet,
            ParameterType::ObjectType,
            ParameterType::Int,
        ],
        &["For ", " set the priority of object type ", " to "],
    ),
    // 130: SET_ATTACK_PRIORITY_KIND_OF
    Definition::new(
        "AttackPrioritySet_/Set/Modify a set's priorities for all of a kind.",
        &[
            ParameterType::AttackPrioritySet,
            ParameterType::KindOfParam,
            ParameterType::Int,
        ],
        &["For ", " set the priority of object type ", " to "],
    ),
    // 131: SET_DEFAULT_ATTACK_PRIORITY
    Definition::new(
        "AttackPrioritySet_/Set/Specify the set's default priority.",
        &[ParameterType::AttackPrioritySet, ParameterType::Int],
        &["For ", " set the default priority to "],
    ),
    // 132: CAMERA_STOP_FOLLOW
    Definition::new(
        "Camera_/Move/Stop following any units.",
        &[],
        &["Stop following any units."],
    ),
    // 133: CAMERA_MOTION_BLUR
    Definition::new(
        "Camera_/Fade Effects/Motion blur zoom.",
        &[ParameterType::Boolean, ParameterType::Boolean],
        &[
            "Blur zoom, zoom in = ",
            " (true=zoom in, false = zoom out), saturate colors = ",
        ],
    ),
    // 134: CAMERA_MOTION_BLUR_JUMP
    Definition::new(
        "Camera_/Fade Effects/Motion blur zoom with jump cut.",
        &[ParameterType::Waypoint, ParameterType::Boolean],
        &[
            "Blur zoom, zoom in at current location, zoom out at ",
            ", saturate colors = ",
        ],
    ),
    // 135: CAMERA_MOTION_BLUR_FOLLOW
    // C++ leaves parameter 0 indeterminate; ScriptActions.cpp reads Int.
    Definition::new(
        "Camera_/Fade Effects/Start motion blur as the camera moves.",
        &[ParameterType::Int],
        &[
            "Start motion blur as the camera moves, amount= ",
            " (start with 30 and adjust up or down). ",
        ],
    ),
    // 136: CAMERA_MOTION_BLUR_END_FOLLOW
    Definition::new(
        "Camera_/Fade Effects/End motion blur as the camera moves.",
        &[],
        &["End motion blur as the camera moves."],
    ),
    // 137: FREEZE_TIME
    Definition::new("Scripting_/Time/Freeze time.", &[], &["Freeze time."]),
    // 138: UNFREEZE_TIME
    Definition::new("Scripting_/Time/Unfreeze time.", &[], &["Unfreeze time."]),
    // 139: SHOW_MILITARY_CAPTION
    Definition::new(
        "Scripting_/Briefing/Show military briefing caption.",
        &[ParameterType::TextString, ParameterType::Int],
        &["Show military briefing ", " for ", " milliseconds."],
    ),
    // 140: CAMERA_SET_AUDIBLE_DISTANCE
    Definition::new(
        "Camera_/Sounds/Set the audible distance for camera-up shots.",
        &[ParameterType::Real],
        &["Set the audible range during camera-up shots to ", ""],
    ),
    // 141: SET_STOPPING_DISTANCE
    Definition::new(
        "Team_/Move/Set stopping distance for each unit's current locomotor.",
        &[ParameterType::Team, ParameterType::Real],
        &["Set stopping distances for ", " to ", "."],
    ),
    // 142: NAMED_SET_STOPPING_DISTANCE
    Definition::new(
        "Unit_/Move/Set stopping distance for current locomotor.",
        &[ParameterType::Unit, ParameterType::Real],
        &["Set stopping distance for ", " to ", "."],
    ),
    // 143: SET_FPS_LIMIT
    Definition::new(
        "Scripting_/ Set max frames per second.",
        &[ParameterType::Int],
        &["Set max FPS to ", ".  (0 sets to default.)"],
    ),
    // 144: MUSIC_SET_VOLUME
    Definition::new(
        "Multimedia_/ Set the current music volume.",
        &[ParameterType::Real],
        &["Set the desired music volume to ", "%. (0-100)"],
    ),
    // 145: MAP_SHROUD_AT_WAYPOINT
    Definition::new(
        "Map_/Shroud or Reveal/Shroud map at waypoint -- add fog.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Side,
        ],
        &[
            "The map is shrouded at ",
            " with a radius of ",
            " feet for ",
            ".",
        ],
    ),
    // 146: MAP_SHROUD_ALL
    Definition::new(
        "Map_/Shroud or Reveal/Shroud the entire map for a player.",
        &[ParameterType::Side],
        &["The world is shrouded for ", "."],
    ),
    // 147: SET_RANDOM_TIMER
    Definition::new(
        "Scripting_/Timer/Frame countdown timer -- set random.",
        &[
            ParameterType::Counter,
            ParameterType::Int,
            ParameterType::Int,
        ],
        &["Set timer ", " to expire between ", " and ", " frames."],
    ),
    // 148: SET_RANDOM_MSEC_TIMER
    Definition::new(
        "Scripting_/Timer/Seconds countdown timer -- set random.",
        &[
            ParameterType::Counter,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &["Set timer ", " to expire between ", " and ", " seconds."],
    ),
    // 149: STOP_TIMER
    Definition::new(
        "Scripting_/Timer/Timer -- stop.",
        &[ParameterType::Counter],
        &["Stop timer "],
    ),
    // 150: RESTART_TIMER
    Definition::new(
        "Scripting_/Timer/Timer -- restart stopped.",
        &[ParameterType::Counter],
        &["Restart timer "],
    ),
    // 151: ADD_TO_MSEC_TIMER
    Definition::new(
        "Scripting_/Timer/Seconds countdown timer -- add seconds.",
        &[ParameterType::Real, ParameterType::Counter],
        &["Add ", " seconds to timer ", " ."],
    ),
    // 152: SUB_FROM_MSEC_TIMER
    Definition::new(
        "Scripting_/Timer/Seconds countdown timer -- subtract seconds.",
        &[ParameterType::Real, ParameterType::Counter],
        &["Subtract ", " seconds from timer ", " ."],
    ),
    // 153: TEAM_TRANSFER_TO_PLAYER
    Definition::new(
        "Team_/ Transfer control of a team to a player.",
        &[ParameterType::Team, ParameterType::Side],
        &["Control of ", " transfers to "],
    ),
    // 154: PLAYER_SET_MONEY
    Definition::new(
        "Player_/ Set player's money.",
        &[ParameterType::Side, ParameterType::Int],
        &["Set ", "'s money to $"],
    ),
    // 155: PLAYER_GIVE_MONEY
    Definition::new(
        "Player_/ Gives/takes from player's money.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " gets $"],
    ),
    // 156: DISABLE_SPECIAL_POWER_DISPLAY
    Definition::new(
        "Scripting_/ Special power countdown display -- disable.",
        &[],
        &["Disables special power countdown display."],
    ),
    // 157: ENABLE_SPECIAL_POWER_DISPLAY
    Definition::new(
        "Scripting_/ Special power countdown display -- enable.",
        &[],
        &["Enables special power countdown display."],
    ),
    // 158: NAMED_HIDE_SPECIAL_POWER_DISPLAY
    Definition::new(
        "Unit_/ Special power countdown timer -- hide.",
        &[ParameterType::Unit],
        &["Hides special power countdowns for ", "."],
    ),
    // 159: NAMED_SHOW_SPECIAL_POWER_DISPLAY
    Definition::new(
        "Unit_/ Special power countdown timer -- display.",
        &[ParameterType::Unit],
        &["Shows special power countdowns for ", "."],
    ),
    // 160: DISPLAY_COUNTDOWN_TIMER
    Definition::new(
        "Scripting_/ Timer -- display an individual timer to the user.",
        &[ParameterType::Counter, ParameterType::LocalizedText],
        &["Show ", " with text "],
    ),
    // 161: HIDE_COUNTDOWN_TIMER
    Definition::new(
        "Scripting_/ Timer -- hides an individual timer from the user.",
        &[ParameterType::Counter],
        &["Hide "],
    ),
    // 162: ENABLE_COUNTDOWN_TIMER_DISPLAY
    Definition::new(
        "Scripting_/ Timer -- display all timers to the user.",
        &[],
        &["Enables timer display."],
    ),
    // 163: DISABLE_COUNTDOWN_TIMER_DISPLAY
    Definition::new(
        "Scripting_/ Timer -- hide all timers from the user.",
        &[],
        &["Disables timer display."],
    ),
    // 164: NAMED_STOP_SPECIAL_POWER_COUNTDOWN
    Definition::new(
        "Unit_/ Special power countdown timer -- pause.",
        &[ParameterType::Unit, ParameterType::SpecialPower],
        &["Pause ", "'s ", " countdown."],
    ),
    // 165: NAMED_START_SPECIAL_POWER_COUNTDOWN
    Definition::new(
        "Unit_/ Special power countdown timer -- resume.",
        &[ParameterType::Unit, ParameterType::SpecialPower],
        &["Resume ", "'s ", " countdown."],
    ),
    // 166: NAMED_SET_SPECIAL_POWER_COUNTDOWN
    Definition::new(
        "Unit_/ Special power countdown timer -- set.",
        &[
            ParameterType::Unit,
            ParameterType::SpecialPower,
            ParameterType::Int,
        ],
        &["Set ", "'s ", " to ", " seconds."],
    ),
    // 167: NAMED_ADD_SPECIAL_POWER_COUNTDOWN
    Definition::new(
        "Unit_/ Special power countdown timer -- add seconds.",
        &[
            ParameterType::Unit,
            ParameterType::SpecialPower,
            ParameterType::Int,
        ],
        &[" ", "'s ", " has ", " seconds added to it."],
    ),
    // 168: NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT
    Definition::new(
        "Unit_/ Special power -- fire at location.",
        &[
            ParameterType::Unit,
            ParameterType::SpecialPower,
            ParameterType::Waypoint,
        ],
        &[" ", " fires ", " at ", "."],
    ),
    // 169: NAMED_FIRE_SPECIAL_POWER_AT_NAMED
    Definition::new(
        "Unit_/ Special power -- fire at unit.",
        &[
            ParameterType::Unit,
            ParameterType::SpecialPower,
            ParameterType::Unit,
        ],
        &[" ", " fires ", " at ", "."],
    ),
    // 170: REFRESH_RADAR
    Definition::new(
        "Scripting_/ Refresh radar terrain.",
        &[],
        &["Refresh radar terrain."],
    ),
    // 171: CAMERA_TETHER_NAMED
    Definition::new(
        "Camera_/ Tether camera to a specific unit.",
        &[
            ParameterType::Unit,
            ParameterType::Boolean,
            ParameterType::Real,
        ],
        &[
            "Have the camera tethered to ",
            ".  Snap camera to object is ",
            ".  Amount of play is ",
            ".",
        ],
    ),
    // 172: CAMERA_STOP_TETHER_NAMED
    Definition::new(
        "Camera_/ Stop tether to any units.",
        &[],
        &["Stop tether to any units."],
    ),
    // 173: CAMERA_SET_DEFAULT
    Definition::new(
        "Camera_/ Set default camera.",
        &[
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Camera Pitch = ",
            "(0.0==default), angle = ",
            "(0.0 is N, 90.0 is W, etc), height = ",
            "(1.0==default).",
        ],
    ),
    // 174: NAMED_STOP
    Definition::new(
        "Unit_/ Set a specific unit to stop.",
        &[ParameterType::Unit],
        &[" ", " stops."],
    ),
    // 175: TEAM_STOP
    Definition::new(
        "Team_/ Set to stop.",
        &[ParameterType::Team],
        &[" ", " stops."],
    ),
    // 176: TEAM_STOP_AND_DISBAND
    Definition::new(
        "Team_/ Set to stop, then disband.",
        &[ParameterType::Team],
        &[" ", " stops, then disbands."],
    ),
    // 177: RECRUIT_TEAM
    Definition::new(
        "Team_/Create/Recruit a team.",
        &[ParameterType::Team, ParameterType::Real],
        &[
            "Recruit an instance of ",
            ", maximum recruiting distance (feet):",
            ".",
        ],
    ),
    // 178: TEAM_SET_OVERRIDE_RELATION_TO_TEAM
    Definition::new(
        "Team_/ Override a team's relationship to another team.",
        &[
            ParameterType::Team,
            ParameterType::Team,
            ParameterType::Relation,
        ],
        &[
            " ",
            " considers ",
            " to be ",
            " (rather than using the the player relationship).",
        ],
    ),
    // 179: TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM
    Definition::new(
        "Team_/ Remove an override to a team's relationship to another team.",
        &[ParameterType::Team, ParameterType::Team],
        &[" ", " uses the player relationship to "],
    ),
    // 180: TEAM_REMOVE_ALL_OVERRIDE_RELATIONS
    Definition::new(
        "Team_/ Remove all overrides to team's relationship to teams and/or players.",
        &[ParameterType::Team],
        &[
            " ",
            " uses the player relationship to all other teams and players.",
        ],
    ),
    // 181: CAMERA_LOOK_TOWARD_OBJECT
    Definition::new(
        "Camera (R)_/ Rotate toward unit.",
        &[
            ParameterType::Unit,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Rotate toward ",
            ", taking ",
            " seconds and holding ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds.",
        ],
    ),
    // 182: NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH
    Definition::new(
        "Unit_/ Fire waypoint-weapon following waypoint path.",
        &[ParameterType::Unit, ParameterType::WaypointPath],
        &[
            " ",
            " fire waypoint-weapon following waypoints starting at ",
            ".",
        ],
    ),
    // 183: TEAM_SET_OVERRIDE_RELATION_TO_PLAYER
    Definition::new(
        "Team_/ Override a team's relationship to another player.",
        &[
            ParameterType::Team,
            ParameterType::Side,
            ParameterType::Relation,
        ],
        &[
            " ",
            " considers ",
            " to be ",
            " (rather than using the the player relationship).",
        ],
    ),
    // 184: TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER
    Definition::new(
        "Team_/ Remove an override to a team's relationship to another player.",
        &[ParameterType::Team, ParameterType::Side],
        &[" ", " uses the player relationship to "],
    ),
    // 185: PLAYER_SET_OVERRIDE_RELATION_TO_TEAM
    Definition::new(
        "Player_/ Override a player's relationship to another team.",
        &[
            ParameterType::Side,
            ParameterType::Team,
            ParameterType::Relation,
        ],
        &[
            " ",
            " considers ",
            " to be ",
            " (rather than using the the player relationship).",
        ],
    ),
    // 186: PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM
    Definition::new(
        "Player_/ Remove an override to a player's relationship to another team.",
        &[ParameterType::Side, ParameterType::Team],
        &[" ", " uses the player relationship to "],
    ),
    // 187: UNIT_EXECUTE_SEQUENTIAL_SCRIPT
    Definition::new(
        "Unit_/ Set a specific unit to execute a script sequentially.",
        &[ParameterType::Unit, ParameterType::Script],
        &[" ", " executes ", " sequentially."],
    ),
    // 188: UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING
    Definition::new(
        "Unit_/ Set a specific unit to execute a looping sequential script.",
        &[
            ParameterType::Unit,
            ParameterType::Script,
            ParameterType::Int,
        ],
        &[" ", " executes ", " sequentially, ", " times. (0=forever)"],
    ),
    // 189: UNIT_STOP_SEQUENTIAL_SCRIPT
    Definition::new(
        "Unit_/ Set a specific unit to stop executing a sequential script.",
        &[ParameterType::Unit],
        &[" ", " stops executing."],
    ),
    // 190: TEAM_EXECUTE_SEQUENTIAL_SCRIPT
    Definition::new(
        "Team_/ Execute script sequentially -- start.",
        &[ParameterType::Team, ParameterType::Script],
        &[" ", " executes ", " sequentially."],
    ),
    // 191: TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING
    Definition::new(
        "Team_/ Execute script sequentially -- looping.",
        &[
            ParameterType::Team,
            ParameterType::Script,
            ParameterType::Int,
        ],
        &[" ", " executes ", " sequentially, ", " times. (0=forever)"],
    ),
    // 192: TEAM_STOP_SEQUENTIAL_SCRIPT
    Definition::new(
        "Team_/ Execute script sequentially -- stop.",
        &[ParameterType::Team],
        &[" ", " stops executing."],
    ),
    // 193: UNIT_GUARD_FOR_FRAMECOUNT
    Definition::new(
        "Unit_/ Set to guard for some number of frames.",
        &[ParameterType::Unit, ParameterType::Int],
        &[" ", " guards for ", " frames."],
    ),
    // 194: UNIT_IDLE_FOR_FRAMECOUNT
    Definition::new(
        "Unit_/ Set to idle for some number of frames.",
        &[ParameterType::Unit, ParameterType::Int],
        &[" ", " idles for ", " frames."],
    ),
    // 195: TEAM_GUARD_FOR_FRAMECOUNT
    Definition::new(
        "Team_/ Set to guard -- number of frames.",
        &[ParameterType::Team, ParameterType::Int],
        &[" ", " guards for ", " frames."],
    ),
    // 196: TEAM_IDLE_FOR_FRAMECOUNT
    Definition::new(
        "Team_/ Set to idle for some number of frames.",
        &[ParameterType::Team, ParameterType::Int],
        &[" ", " idles for ", " frames."],
    ),
    // 197: WATER_CHANGE_HEIGHT
    Definition::new(
        "Map_/ Adjust water height to a new level",
        &[ParameterType::TriggerArea, ParameterType::Real],
        &[" ", " changes altitude to "],
    ),
    // 198: NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED
    Definition::new(
        "Unit_/ Use commandbutton ability on an object.",
        &[
            ParameterType::Unit,
            ParameterType::CommandbuttonAbility,
            ParameterType::Unit,
        ],
        &[" ", " use ", " on ", "."],
    ),
    // 199: NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT
    Definition::new(
        "Unit_/ Use commandbutton ability at a waypoint.",
        &[
            ParameterType::Unit,
            ParameterType::CommandbuttonAbility,
            ParameterType::Waypoint,
        ],
        &[" ", " use ", " at ", "."],
    ),
    // 200: WATER_CHANGE_HEIGHT_OVER_TIME
    Definition::new(
        "Map_/ Adjust water height to a new level with damage over time",
        &[
            ParameterType::TriggerArea,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            " ",
            " changes altitude to ",
            " in ",
            " seconds doing ",
            " dam_/sec.",
        ],
    ),
    // 201: MAP_SWITCH_BORDER
    Definition::new(
        "Map_/ Change the active boundary.",
        &[ParameterType::Boundary],
        &[" ", " becomes the active border."],
    ),
    // 202: TEAM_GUARD_POSITION
    Definition::new(
        "Team_/Guard/Set to guard -- location.",
        &[ParameterType::Team, ParameterType::Waypoint],
        &[" ", " begins guarding at "],
    ),
    // 203: TEAM_GUARD_OBJECT
    Definition::new(
        "Team_/Guard/Set to guard -- specific unit.",
        &[ParameterType::Team, ParameterType::Unit],
        &[" ", " begins guarding "],
    ),
    // 204: TEAM_GUARD_AREA
    Definition::new(
        "Team_/Guard/Set to guard -- area.",
        &[ParameterType::Team, ParameterType::TriggerArea],
        &[" ", " begins guarding "],
    ),
    // 205: OBJECT_FORCE_SELECT
    Definition::new(
        "Scripting_/ Select the first object type on a team.",
        &[
            ParameterType::Team,
            ParameterType::ObjectType,
            ParameterType::Boolean,
            ParameterType::Dialog,
        ],
        &[" ", " 's first ", ", centers in view (", ") while playing "],
    ),
    // 206: CAMERA_LOOK_TOWARD_WAYPOINT
    Definition::new(
        "Camera (R)_/ Rotate to look at a waypoint.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Boolean,
        ],
        &[
            "Rotate to look at ",
            ", taking ",
            " seconds, ease-in ",
            " seconds, ease-out ",
            " seconds, reverse rotation ",
            ".",
        ],
    ),
    // 207: UNIT_DESTROY_ALL_CONTAINED
    Definition::new(
        "Unit_/ Kill all units contained within a specific transport or structure.",
        &[ParameterType::Unit],
        &["All units inside ", " are killed."],
    ),
    // 208: RADAR_FORCE_ENABLE
    Definition::new(
        "Radar_/ Force enable the radar.",
        &[],
        &["The radar is now forced to be enabled."],
    ),
    // 209: RADAR_REVERT_TO_NORMAL
    Definition::new(
        "Radar_/ Revert radar to normal behavior.",
        &[],
        &["The radar is now reverting to its normal behavior."],
    ),
    // 210: SCREEN_SHAKE
    Definition::new(
        "Camera_/ Shake Screen.",
        &[ParameterType::ShakeIntensity],
        &["The screen will shake with "],
    ),
    // 211: TECHTREE_MODIFY_BUILDABILITY_OBJECT
    Definition::new(
        "Map_/ Adjust the tech tree for a specific object type.",
        &[ParameterType::ObjectType, ParameterType::Buildable],
        &[" ", " becomes "],
    ),
    // 212: WAREHOUSE_SET_VALUE
    Definition::new(
        "Unit_/ Set cash value of Warehouse.",
        &[ParameterType::Unit, ParameterType::Int],
        &[
            "Warehouse named ",
            " is set to having ",
            " dollars worth of boxes. ",
        ],
    ),
    // 213: OBJECT_CREATE_RADAR_EVENT
    Definition::new(
        "Radar_/Create Event/Create  a radar event at a specific object.",
        &[ParameterType::Unit, ParameterType::RadarEventType],
        &["A radar event occurs at ", " of type "],
    ),
    // 214: TEAM_CREATE_RADAR_EVENT
    Definition::new(
        "Radar_/Create Event/Create  a radar event at a specific team.",
        &[ParameterType::Team, ParameterType::RadarEventType],
        &["A radar event occurs at ", " of type "],
    ),
    // 215: DISPLAY_CINEMATIC_TEXT
    Definition::new(
        "User_/String/Display a cinematic string.",
        &[
            ParameterType::LocalizedText,
            ParameterType::FontName,
            ParameterType::Int,
        ],
        &[
            "Displays ",
            " with font type ",
            " in the bottom letterbox for ",
            " seconds.",
        ],
    ),
    // 216: DEBUG_CRASH_BOX
    Definition::new(
        "{INTERNAL}_/Debug/Display a crash box (debug/internal builds only).",
        &[ParameterType::TextString],
        &["Display a crash box with the text: "],
    ),
    // 217: SOUND_DISABLE_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- disable type.",
        &[ParameterType::Sound],
        &[" ", " is disabled."],
    ),
    // 218: SOUND_ENABLE_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- enable type.",
        &[ParameterType::Sound],
        &[" ", " is enabled."],
    ),
    // 219: SOUND_ENABLE_ALL
    Definition::new(
        "Multimedia_/ Sound Events -- enable all.",
        &[],
        &["Enable all sound events."],
    ),
    // 220: AUDIO_OVERRIDE_VOLUME_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- override volume -- type.",
        &[ParameterType::Sound, ParameterType::Real],
        &[" ", " play at ", "% of full volume."],
    ),
    // 221: AUDIO_RESTORE_VOLUME_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- restore volume -- type.",
        &[ParameterType::Sound],
        &[" ", " play at normal volume.", ""],
    ),
    // 222: AUDIO_RESTORE_VOLUME_ALL_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- restore volume -- all.",
        &[],
        &["All sound events play at normal volume."],
    ),
    // 223: INGAME_POPUP_MESSAGE
    Definition::new(
        "User_/String/Display Popup Message Box.",
        &[
            ParameterType::LocalizedText,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Int,
            ParameterType::Boolean,
        ],
        &[
            "Displays ",
            " at ",
            " percent of the screen Width ",
            " percent of the screen Height and a width of ",
            " pixels and pauses the game (",
            " )",
        ],
    ),
    // 224: SET_CAVE_INDEX
    Definition::new(
        "Unit_/ Set Cave connectivity index.",
        &[ParameterType::Unit, ParameterType::Int],
        &[
            "Cave named ",
            " is set to being connected to all caves of index ",
            ", but only if both Cave listings have no occupants. ",
        ],
    ),
    // 225: NAMED_SET_HELD
    Definition::new(
        "Unit_/Move/Set unit to be held in place, ignoring Physics, Locomotors, etc.",
        &[ParameterType::Unit, ParameterType::Boolean],
        &["Set Held status for ", " to ", "."],
    ),
    // 226: NAMED_SET_TOPPLE_DIRECTION
    Definition::new(
        "Unit_/ Set topple direction.",
        &[ParameterType::Unit, ParameterType::Coord3D],
        &[" ", " will topple towards ", " if destroyed."],
    ),
    // 227: UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE
    Definition::new(
        "Unit_/ Move unit towards the nearest object of a specific type.",
        &[
            ParameterType::Unit,
            ParameterType::ObjectType,
            ParameterType::TriggerArea,
        ],
        &[" ", " will move towards the nearest ", " within "],
    ),
    // 228: TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE
    Definition::new(
        "Team_/ Move team towards the nearest object of a specific type.",
        &[
            ParameterType::Team,
            ParameterType::ObjectType,
            ParameterType::TriggerArea,
        ],
        &[" ", " will move towards the nearest ", " within "],
    ),
    // 229: MAP_REVEAL_ALL_PERM
    Definition::new(
        "Map_/Shroud or Reveal/Reveal the entire map permanently for a player.",
        &[ParameterType::Side],
        &["The world is revealed permanently for ", "."],
    ),
    // 230: MAP_REVEAL_ALL_UNDO_PERM
    Definition::new(
        "Map_/Shroud or Reveal/Un-Reveal the entire map permanently for a player.",
        &[ParameterType::Side],
        &[
            "Undo the permanent reveal for ",
            ".  This will mess things up badly if called when there has been no permanent reveal.",
        ],
    ),
    // 231: NAMED_SET_REPULSOR
    Definition::new(
        "Unit_/Internal/Repulsor/Set the REPULSOR flag of a specific unit.",
        &[ParameterType::Unit, ParameterType::Boolean],
        &[" ", " REPULSOR flag is "],
    ),
    // 232: TEAM_SET_REPULSOR
    Definition::new(
        "Team_/Internal/Repulsor/Set the REPULSOR flag of a team.",
        &[ParameterType::Team, ParameterType::Boolean],
        &[" ", " REPULSOR flag is "],
    ),
    // 233: TEAM_WANDER_IN_PLACE
    Definition::new(
        "Team_/Move/Set to wander around current location.",
        &[ParameterType::Team],
        &["Have ", " wander around it's current location."],
    ),
    // 234: TEAM_INCREASE_PRIORITY
    Definition::new(
        "Team_/AI/Increase priority by Success Priority Increase amount.",
        &[ParameterType::Team],
        &[
            "Increase the AI priority for",
            "  by its Success Priority Increase amount.",
        ],
    ),
    // 235: TEAM_DECREASE_PRIORITY
    Definition::new(
        "Team_/AI/Reduce priority by Failure Priority Decrease amount.",
        &[ParameterType::Team],
        &[
            "Reduce the AI priority for",
            "  by its Failure Priority Decrease amount.",
        ],
    ),
    // 236: DISPLAY_COUNTER
    Definition::new(
        "Scripting_/ Counter -- display an individual counter to the user.",
        &[ParameterType::Counter, ParameterType::LocalizedText],
        &["Show ", " with text "],
    ),
    // 237: HIDE_COUNTER
    Definition::new(
        "Scripting_/ Counter -- hides an individual counter from the user.",
        &[ParameterType::Counter],
        &["Hide "],
    ),
    // 238: TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED
    Definition::new(
        "Team_/ Use commandbutton ability on an object.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::Unit,
        ],
        &[" ", " use ", " on ", "."],
    ),
    // 239: TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT
    Definition::new(
        "Team_/ Use commandbutton ability at a waypoint.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::Waypoint,
        ],
        &[" ", " use ", " at ", "."],
    ),
    // 240: NAMED_USE_COMMANDBUTTON_ABILITY
    Definition::new(
        "Unit_/ Use commandbutton ability.",
        &[ParameterType::Unit, ParameterType::CommandbuttonAbility],
        &[" ", " use ", "."],
    ),
    // 241: TEAM_USE_COMMANDBUTTON_ABILITY
    Definition::new(
        "Team_/ Use commandbutton ability.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " use ", "."],
    ),
    // 242: NAMED_FLASH_WHITE
    Definition::new(
        "User_/Flash/Flash a specific unit white for a specified amount of time.",
        &[ParameterType::Unit, ParameterType::Int],
        &[" ", " flashes white for ", " seconds."],
    ),
    // 243: TEAM_FLASH_WHITE
    Definition::new(
        "User_/Flash/Flash a team white for a specified amount of time.",
        &[ParameterType::Team, ParameterType::Int],
        &[" ", " flashes white for ", " seconds."],
    ),
    // 244: SKIRMISH_BUILD_BUILDING
    Definition::new(
        "Skirmish Only_/ Build a building.",
        &[ParameterType::ObjectType],
        &["Build a building of type "],
    ),
    // 245: SKIRMISH_FOLLOW_APPROACH_PATH
    Definition::new(
        "Skirmish Only_/Move/Team follow approach path.",
        &[
            ParameterType::Team,
            ParameterType::SkirmishWaypointPath,
            ParameterType::Boolean,
        ],
        &[
            "Have ",
            " approach the enemy using path ",
            ", as a team is ",
        ],
    ),
    // 246: IDLE_ALL_UNITS
    Definition::new(
        "Scripting_/Idle or Restart/Idle all units for all players.",
        &[],
        &["Idle all units for all players."],
    ),
    // 247: RESUME_SUPPLY_TRUCKING
    Definition::new(
        "Scripting_/Idle or Restart/All idle Supply Trucks attempt to resume supply routes.",
        &[],
        &["All idle Supply Trucks attempt to resume supply routes."],
    ),
    // 248: NAMED_CUSTOM_COLOR
    Definition::new(
        "User_/Flash/Set a specific unit to use a special indicator color.",
        &[ParameterType::Unit, ParameterType::Color],
        &[" ", " uses the color ", " ."],
    ),
    // 249: SKIRMISH_MOVE_TO_APPROACH_PATH
    Definition::new(
        "Skirmish Only_/Move/Team move to approach path.",
        &[ParameterType::Team, ParameterType::SkirmishWaypointPath],
        &["Have ", " move to the start of enemy path ", "."],
    ),
    // 250: SKIRMISH_BUILD_BASE_DEFENSE_FRONT
    Definition::new(
        "Skirmish Only_/Build/Build base defense on front perimeter.",
        &[],
        &["Build one additional perimeter base defenses, on the front."],
    ),
    // 251: SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST
    Definition::new(
        "Skirmish_/ Special power -- fire at enemy's highest cost area.",
        &[ParameterType::Side, ParameterType::SpecialPower],
        &[" ", " fire "],
    ),
    // 252: unused
    Definition::new("UNUSED/(placeholder)/placeholder", &[], &[]),
    // 253: PLAYER_REPAIR_NAMED_STRUCTURE
    Definition::new(
        "Player_/ Repair named bridge or structure.",
        &[ParameterType::Side, ParameterType::Unit],
        &["Have ", " repair ", "."],
    ),
    // 254: SKIRMISH_BUILD_BASE_DEFENSE_FLANK
    Definition::new(
        "Skirmish Only_/Build/Build base defense on flank perimeter.",
        &[],
        &["Build one additional perimeter base defenses, on the flank."],
    ),
    // 255: SKIRMISH_BUILD_STRUCTURE_FRONT
    Definition::new(
        "Skirmish Only_/Build/Build structure on front perimeter.",
        &[ParameterType::ObjectType],
        &["Build one additional ", ", on the front."],
    ),
    // 256: SKIRMISH_BUILD_STRUCTURE_FLANK
    Definition::new(
        "Skirmish Only_/Build/Build structure on flank perimeter.",
        &[ParameterType::ObjectType],
        &["Build one additional ", ", on the flank."],
    ),
    // 257: SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE
    Definition::new(
        "Skirmish_/ Team attacks nearest group matching value comparison.",
        &[
            ParameterType::Team,
            ParameterType::Comparison,
            ParameterType::Int,
        ],
        &[" ", " attacks nearest group worth ", " "],
    ),
    // 258: SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT
    Definition::new(
        "Skirmish_/ Team performs command ability on most valuable object.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::Real,
            ParameterType::Boolean,
        ],
        &[
            " ",
            " performs ",
            " on most expensive object within ",
            " ",
            " (true = all valid sources, false = first valid source).",
        ],
    ),
    // 259: SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL
    Definition::new(
        "Skirmish_/ Delay a sequential script until the specified command ability is ready - all.",
        &[
            ParameterType::Side,
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " 's ", " all wait until ", " is ready."],
    ),
    // 260: SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL
    Definition::new(
        "Skirmish_/ Delay a sequential script until the specified command ability is ready - partial.",
        &[
            ParameterType::Side,
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[
            " ",
            " 's ",
            " wait until at least one member is ",
            " ready.",
        ],
    ),
    // 261: TEAM_SPIN_FOR_FRAMECOUNT
    Definition::new(
        "Team_/ Set to continue current action for some number of frames.",
        &[ParameterType::Team, ParameterType::Int],
        &[
            " ",
            " continue their current action for at least ",
            " frames.",
        ],
    ),
    // 262: TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED
    Definition::new(
        "Team_/ Use command ability -- all -- named enemy",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " use ", "  on "],
    ),
    // 263: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT
    Definition::new(
        "Team_/ Use command ability -- all -- nearest enemy unit",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " use ", "  on nearest enemy unit."],
    ),
    // 264: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING
    Definition::new(
        "Team_/ Use command ability -- all -- nearest enemy garrisoned building.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " use ", "  on nearest enemy garrisoned building."],
    ),
    // 265: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF
    Definition::new(
        "Team_/ Use command ability -- all -- nearest enemy object with kind of.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::KindOfParam,
        ],
        &[" ", " use ", "  on nearest enemy with ", ""],
    ),
    // 266: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING
    Definition::new(
        "Team_/ Use command ability -- all -- nearest enemy building.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " use ", "  on nearest enemy building."],
    ),
    // 267: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS
    Definition::new(
        "Team_/ Use command ability -- all -- nearest enemy building kindof.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::KindOfParam,
        ],
        &[" ", " use ", "  on nearest enemy building with "],
    ),
    // 268: TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE
    Definition::new(
        "Team_/ Use command ability -- all -- nearest object type.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
            ParameterType::ObjectType,
        ],
        &[" ", " use ", " on nearest object of type ", "."],
    ),
    // 269: TEAM_PARTIAL_USE_COMMANDBUTTON
    Definition::new(
        "Team_/ Use command ability -- partial -- self.",
        &[
            ParameterType::Real,
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", "% of ", " perform ", "."],
    ),
    // 270: TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT
    Definition::new(
        "Team_/ Capture unowned faction unit -- nearest.",
        &[ParameterType::Team],
        &[" ", " capture the nearest unowned faction unit."],
    ),
    // 271: PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS
    Definition::new(
        "Player_/ Create team from all captured units.",
        &[ParameterType::Side, ParameterType::Team],
        &[
            " ",
            " creates a new ",
            " from units it has captured. (There's nothing quite like being assaulted by your own captured units!)",
        ],
    ),
    // 272: PLAYER_ADD_SKILLPOINTS
    Definition::new(
        "Player_/Experience/Add or Subtract Skill Points.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " is given ", " Skill Points."],
    ),
    // 273: PLAYER_ADD_RANKLEVEL
    Definition::new(
        "Player_/Experience/Add or Subtract Rank Levels.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " is given ", " Rank Levels."],
    ),
    // 274: PLAYER_SET_RANKLEVEL
    Definition::new(
        "Player_/Experience/Set Rank Level.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " is given a Rank Level of ", "."],
    ),
    // 275: PLAYER_SET_RANKLEVELLIMIT
    Definition::new(
        "Map_/Experience/Set Rank Level Limit for current Map.",
        &[ParameterType::Int],
        &["The current map is given a Rank Level Limit of ", "."],
    ),
    // 276: PLAYER_GRANT_SCIENCE
    Definition::new(
        "Player_/Science/Grant a Science to a given Player (ignoring prerequisites).",
        &[ParameterType::Side, ParameterType::Science],
        &[" ", " is granted ", "."],
    ),
    // 277: PLAYER_PURCHASE_SCIENCE
    Definition::new(
        "Player_/Science/Player attempts to purchase a Science.",
        &[ParameterType::Side, ParameterType::Science],
        &[" ", " attempts to purchase Science ", "."],
    ),
    // 278: TEAM_HUNT_WITH_COMMAND_BUTTON
    Definition::new(
        "Team_/Hunt/Set to hunt using commandbutton ability.",
        &[
            ParameterType::Team,
            ParameterType::CommandbuttonAllAbilities,
        ],
        &[" ", " begins hunting using ", "."],
    ),
    // 279: TEAM_WAIT_FOR_NOT_CONTAINED_ALL
    Definition::new(
        "Team_/ Delay a sequential script until the team is no longer contained - all",
        &[ParameterType::Team],
        &[" ", " all delay until they are no longer contained."],
    ),
    // 280: TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL
    Definition::new(
        "Team_/ Delay a sequential script until the team is no longer contained - partial",
        &[ParameterType::Team],
        &[
            " ",
            " delay until at least one of them is no longer contained.",
        ],
    ),
    // 281: TEAM_FOLLOW_WAYPOINTS_EXACT
    Definition::new(
        "Team_/Move/Set to EXACTLY follow a waypoint path.",
        &[
            ParameterType::Team,
            ParameterType::WaypointPath,
            ParameterType::Boolean,
        ],
        &["Have ", " EXACTLY follow ", " , as a team is "],
    ),
    // 282: NAMED_FOLLOW_WAYPOINTS_EXACT
    Definition::new(
        "Unit_/Move/Set a specific unit to EXACTLY follow a waypoint path.",
        &[ParameterType::Unit, ParameterType::WaypointPath],
        &[" ", " EXACTLY follows waypoints, beginning at "],
    ),
    // 283: TEAM_SET_EMOTICON
    Definition::new(
        "Team_/ Set emoticon for duration (-1.0 permanent, otherwise duration in sec).",
        &[
            ParameterType::Team,
            ParameterType::Emoticon,
            ParameterType::Real,
        ],
        &[" ", " use ", " emoticon for ", " seconds."],
    ),
    // 284: NAMED_SET_EMOTICON
    Definition::new(
        "Unit_/ Set emoticon for duration (-1.0 permanent, otherwise duration in sec).",
        &[
            ParameterType::Unit,
            ParameterType::Emoticon,
            ParameterType::Real,
        ],
        &[" ", " use ", " emoticon for ", " seconds."],
    ),
    // 285: AI_PLAYER_BUILD_SUPPLY_CENTER
    Definition::new(
        "Player_/AI/AI player build near a supply source.",
        &[
            ParameterType::Side,
            ParameterType::ObjectType,
            ParameterType::Int,
        ],
        &[
            "Have AI ",
            " build a ",
            " near a supply src with at least ",
            " available resources.",
        ],
    ),
    // 286: AI_PLAYER_BUILD_UPGRADE
    Definition::new(
        "Player_/AI/AI player build an upgrade.",
        &[ParameterType::Side, ParameterType::Upgrade],
        &["Have AI ", " build this upgrade: "],
    ),
    // 287: OBJECTLIST_ADDOBJECTTYPE
    Definition::new(
        "Scripting_/ Object Type List -- Add Object Type.",
        &[ParameterType::ObjectTypeList, ParameterType::ObjectType],
        &[" ", " : add "],
    ),
    // 288: OBJECTLIST_REMOVEOBJECTTYPE
    Definition::new(
        "Scripting_/ Object Type List -- Remove Object Type.",
        &[ParameterType::ObjectTypeList, ParameterType::ObjectType],
        &[" ", " : remove "],
    ),
    // 289: MAP_REVEAL_PERMANENTLY_AT_WAYPOINT
    Definition::new(
        "Map_/ Reveal map at waypoint -- permanently.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Side,
            ParameterType::Revealname,
        ],
        &[
            "The map is permanently revealed at ",
            " with a radius of ",
            " for ",
            ". (Afterwards referred to as ",
            ").",
        ],
    ),
    // 290: MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT
    Definition::new(
        "Map_/ Reveal map at waypoint -- undo permanently.",
        &[ParameterType::Revealname],
        &[" ", " is undone."],
    ),
    // 291: NAMED_SET_STEALTH_ENABLED
    Definition::new(
        "Unit_/Status/Stealth set enabled or disabled.",
        &[ParameterType::Unit, ParameterType::Boolean],
        &["Set ", " stealth ability to ", "."],
    ),
    // 292: TEAM_SET_STEALTH_ENABLED
    Definition::new(
        "Team_/Status/Stealth set enabled or disabled.",
        &[ParameterType::Team, ParameterType::Boolean],
        &["Set ", " stealth ability to ", "."],
    ),
    // 293: EVA_SET_ENABLED_DISABLED
    Definition::new(
        "Scripting_/ Enable or Disable EVA.",
        &[ParameterType::Boolean],
        &["Set EVA to be enabled ", " (False to disable.)"],
    ),
    // 294: OPTIONS_SET_OCCLUSION_MODE
    Definition::new(
        "Scripting_/ Enable or Disable Occlusion (Drawing Behind Buildings).",
        &[ParameterType::Boolean],
        &["Set Occlusion to be enabled ", " (False to disable.)"],
    ),
    // 295: LOCALDEFEAT
    Definition::new(
        "Multiplayer_/ Announce local defeat.",
        &[],
        &["Show 'Game Over' window"],
    ),
    // 296: OPTIONS_SET_DRAWICON_UI_MODE
    Definition::new(
        "Scripting_/ Enable or Disable Draw-icon UI.",
        &[ParameterType::Boolean],
        &["Set Draw-icon UI to be enabled ", " (False to disable.)"],
    ),
    // 297: OPTIONS_SET_PARTICLE_CAP_MODE
    Definition::new(
        "Scripting_/ Enable or Disable Particle Cap.",
        &[ParameterType::Boolean],
        &["Set Particle Cap to be enabled ", " (False to disable.)"],
    ),
    // 298: PLAYER_SCIENCE_AVAILABILITY
    Definition::new(
        "Player_/Science/Set science availability.",
        &[
            ParameterType::Side,
            ParameterType::Science,
            ParameterType::ScienceAvailability,
        ],
        &[" ", " set ", " availability to ", "."],
    ),
    // 299: UNIT_AFFECT_OBJECT_PANEL_FLAGS
    Definition::new(
        "Unit_/ Affect flags set on object panel.",
        &[
            ParameterType::Unit,
            ParameterType::ObjectPanelFlag,
            ParameterType::Boolean,
        ],
        &[" ", " changes the value of flag ", " to ", "."],
    ),
    // 300: TEAM_AFFECT_OBJECT_PANEL_FLAGS
    Definition::new(
        "Team_/ Affect flags set on object panel - all.",
        &[
            ParameterType::Team,
            ParameterType::ObjectPanelFlag,
            ParameterType::Boolean,
        ],
        &[" ", " change the value of flag ", " to ", "."],
    ),
    // 301: PLAYER_SELECT_SKILLSET
    Definition::new(
        "Player_/ Set the skillset for a computer player.",
        &[ParameterType::Side, ParameterType::Int],
        &[" ", " uses skillset number ", " (1-5)."],
    ),
    // 302: SCRIPTING_OVERRIDE_HULK_LIFETIME
    Definition::new(
        "Scripting_/ Hulk set override lifetime.",
        &[ParameterType::Real],
        &[
            "Override hulk lifetime to ",
            " seconds. Negative value reverts to normal behavior.",
        ],
    ),
    // 303: NAMED_FACE_NAMED
    Definition::new(
        "Unit_/ Set unit to face another unit.",
        &[ParameterType::Unit, ParameterType::Unit],
        &[" ", " begin facing "],
    ),
    // 304: NAMED_FACE_WAYPOINT
    Definition::new(
        "Unit_/ Set unit to face a waypoint.",
        &[ParameterType::Unit, ParameterType::Waypoint],
        &[" ", " begin facing "],
    ),
    // 305: TEAM_FACE_NAMED
    Definition::new(
        "Team_/ Set team to face another unit.",
        &[ParameterType::Team, ParameterType::Unit],
        &[" ", " begin facing "],
    ),
    // 306: TEAM_FACE_WAYPOINT
    Definition::new(
        "Team_/ Set team to face a waypoint.",
        &[ParameterType::Team, ParameterType::Waypoint],
        &[" ", " begin facing "],
    ),
    // 307: COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE
    Definition::new(
        "Scripting_/ Remove a command button from an object type.",
        &[ParameterType::CommandButton, ParameterType::ObjectType],
        &[" ", " is removed from all objects of type ", "."],
    ),
    // 308: COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT
    Definition::new(
        "Scripting_/ Add a command button to an object type.",
        &[
            ParameterType::CommandButton,
            ParameterType::ObjectType,
            ParameterType::Int,
        ],
        &[
            " ",
            " is added to all objects of type ",
            " in slot number ",
            " (1-12).",
        ],
    ),
    // 309: UNIT_SPAWN_NAMED_LOCATION_ORIENTATION
    Definition::new(
        "Unit_/ Spawn -- named unit on a team at a position with an orientation.",
        &[
            ParameterType::Unit,
            ParameterType::ObjectType,
            ParameterType::Team,
            ParameterType::Coord3D,
            ParameterType::Angle,
        ],
        &[
            "Spawn ",
            " of type ",
            " on team ",
            " at position (",
            "), rotated ",
            " .",
        ],
    ),
    // 310: PLAYER_AFFECT_RECEIVING_EXPERIENCE
    Definition::new(
        "Player_/ Change the modifier to generals experience that a player receives.",
        &[ParameterType::Side, ParameterType::Real],
        &[
            " ",
            " gains experience at ",
            " times the usual rate (0.0 for no gain, 1.0 for normal rate)",
        ],
    ),
    // 311: PLAYER_EXCLUDE_FROM_SCORE_SCREEN
    Definition::new(
        "Player_/Score/Exclude this player from the score screen.",
        &[ParameterType::Side],
        &["Exclude ", " from the score screen."],
    ),
    // 312: TEAM_GUARD_SUPPLY_CENTER
    Definition::new(
        "Team_/Guard/Set to guard a supply source.",
        &[ParameterType::Team, ParameterType::Int],
        &[
            "Have Team ",
            " guard attacked or closest supply src with at least ",
            " available resources",
        ],
    ),
    // 313: ENABLE_SCORING
    Definition::new("Player_/Score/Turn on scoring.", &[], &["Turn on scoring."]),
    // 314: DISABLE_SCORING
    Definition::new(
        "Player_/Score/Turn off scoring.",
        &[],
        &["Turn off scoring."],
    ),
    // 315: SOUND_SET_VOLUME
    Definition::new(
        "Multimedia_/ Set the current sound volume.",
        &[ParameterType::Real],
        &["Set the desired sound volume to ", "%. (0-100)"],
    ),
    // 316: SPEECH_SET_VOLUME
    Definition::new(
        "Multimedia_/ Set the current speech volume.",
        &[ParameterType::Real],
        &["Set the desired speech volume to ", "%. (0-100)"],
    ),
    // 317: DISABLE_BORDER_SHROUD
    Definition::new(
        "Map_/Shroud or Reveal/Border Shroud is turned off.",
        &[],
        &["Shroud off the map edges is turned off."],
    ),
    // 318: ENABLE_BORDER_SHROUD
    Definition::new(
        "Map_/Shroud or Reveal/Border Shroud is turned on.",
        &[],
        &["Shroud off the map edges is turned on."],
    ),
    // 319: OBJECT_ALLOW_BONUSES
    Definition::new(
        "Map_/ Adjust Object Bonuses based on difficulty.",
        &[ParameterType::Boolean],
        &[
            "Enable Object Bonuses based on difficulty ",
            " (true to enable, false to disable).",
        ],
    ),
    // 320: SOUND_REMOVE_ALL_DISABLED
    Definition::new(
        "Multimedia_/ Sound Events -- remove all disabled.",
        &[],
        &["Remove all disabled sound events."],
    ),
    // 321: SOUND_REMOVE_TYPE
    Definition::new(
        "Multimedia_/ Sound Events -- remove type.",
        &[ParameterType::Sound],
        &[" ", " is removed."],
    ),
    // 322: TEAM_GUARD_IN_TUNNEL_NETWORK
    Definition::new(
        "Team_/ Set to guard - from inside tunnel network.",
        &[ParameterType::Team],
        &[" ", " Enter and guard from tunnel network."],
    ),
    // 323: QUICKVICTORY
    Definition::new(
        "User_/ Announce quick win",
        &[],
        &["End game in victory immediately."],
    ),
    // 324: SET_INFANTRY_LIGHTING_OVERRIDE
    Definition::new(
        "Map_/Environment/Infantry Lighting - Set.",
        &[ParameterType::Real],
        &[
            "Set lighting percent on infantry to ",
            " (0.0==min, 1.0==normal day, 2.0==max (which is normal night).)",
        ],
    ),
    // 325: RESET_INFANTRY_LIGHTING_OVERRIDE
    Definition::new(
        "Map_/Environment/Infantry Lighting - Reset.",
        &[],
        &[
            "Reset infantry lighting to the normal setting. 1.0 for the two day states, 2.0 for the two night states. (Look in GamesData.ini)",
        ],
    ),
    // 326: TEAM_DELETE_LIVING
    Definition::new(
        "Team_/Damage or Remove/Delete a team, but ignore dead guys.",
        &[ParameterType::Team],
        &["Each living member of team ", " is removed from the world."],
    ),
    // 327: RESIZE_VIEW_GUARDBAND
    Definition::new(
        "Map_/ Resize view guardband.",
        &[ParameterType::Real, ParameterType::Real],
        &[
            "Allow bigger objects to be perceived as onscreen near the edge (",
            ",",
            ") Width then height, in world units.",
        ],
    ),
    // 328: DELETE_ALL_UNMANNED
    Definition::new(
        "Scripting_/ Delete all unmanned (sniped) vehicles.",
        &[],
        &["Delete all unmanned (sniped) vehicles."],
    ),
    // 329: CHOOSE_VICTIM_ALWAYS_USES_NORMAL
    Definition::new(
        "Map_/ Force ChooseVictim to ignore game difficulty and always use Normal setting.",
        &[ParameterType::Boolean],
        &[
            "Force ChooseVictim to ignore game difficulty and always use Normal setting ",
            " (true to enable, false to disable).",
        ],
    ),
    // 330: CAMERA_ENABLE_SLAVE_MODE
    Definition::new(
        "Camera_/Enable 3DSMax Camera Animation Playback mode.",
        &[ParameterType::TextString, ParameterType::TextString],
        &[
            "Enable 3DSMax Camera playback of animation with thing name ",
            " containing bone name ",
        ],
    ),
    // 331: CAMERA_DISABLE_SLAVE_MODE
    Definition::new(
        "Camera_/Disable 3DSMax Camera Animation Playback mode.",
        &[],
        &["Disable camera playback mode."],
    ),
    // 332: CAMERA_ADD_SHAKER_AT
    Definition::new(
        "Camera_/Add Camera Shaker Effect at.",
        &[
            ParameterType::Waypoint,
            ParameterType::Real,
            ParameterType::Real,
            ParameterType::Real,
        ],
        &[
            "Add Camera Shaker Effect at waypoint ",
            " with Amplitude ",
            " Duration (seconds) ",
            " Radius.",
        ],
    ),
    // 333: SET_TRAIN_HELD
    Definition::new(
        "Unit/ Set a train to stay at a station. TRUE = stay. FALSE = go-ahead.",
        &[ParameterType::Unit, ParameterType::Boolean],
        &[" ", " sets its held status to "],
    ),
    // 334: NAMED_SET_EVAC_LEFT_OR_RIGHT
    Definition::new(
        "Unit/ Set which side of a container (likely a train) you want the riders to exit on.",
        &[ParameterType::Unit, ParameterType::LeftOrRight],
        &[" ", " will exit its riders on its "],
    ),
    // 335: ENABLE_OBJECT_SOUND
    Definition::new(
        "Multimedia_/Sound Effect/Enable object's ambient sound",
        &[ParameterType::Unit],
        &["Enable (or trigger) ", "'s ambient sound."],
    ),
    // 336: DISABLE_OBJECT_SOUND
    Definition::new(
        "Multimedia_/Sound Effect/Disable object's ambient sound",
        &[ParameterType::Unit],
        &["Disable ", "'s ambient sound."],
    ),
    // 337: NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH
    Definition::new(
        "Unit_/ Use commandbutton ability using a waypoint path.",
        &[
            ParameterType::Unit,
            ParameterType::CommandbuttonAbility,
            ParameterType::WaypointPath,
        ],
        &[" ", " use ", " to follow ", " path."],
    ),
    // 338: NAMED_SET_UNMANNED_STATUS
    Definition::new(
        "Unit_/Status/Make unmanned.",
        &[ParameterType::Unit],
        &["Make ", " unmanned."],
    ),
    // 339: TEAM_SET_UNMANNED_STATUS
    Definition::new(
        "Team_/Status/Make unmanned.",
        &[ParameterType::Team],
        &["Make ", " unmanned."],
    ),
    // 340: NAMED_SET_BOOBYTRAPPED
    Definition::new(
        "Unit_/Status/Add boobytrap.",
        &[ParameterType::ObjectType, ParameterType::Unit],
        &["Add boobytrap of type ", " to ", "."],
    ),
    // 341: TEAM_SET_BOOBYTRAPPED
    Definition::new(
        "Team_/Status/Add boobytrap.",
        &[ParameterType::ObjectType, ParameterType::Team],
        &["Add boobytrap of type ", " to team ", "."],
    ),
    // 342: SHOW_WEATHER
    Definition::new(
        "Map/Environment/Show Weather.",
        &[ParameterType::Boolean],
        &["Show Weather = "],
    ),
    // 343: AI_PLAYER_BUILD_TYPE_NEAREST_TEAM
    Definition::new(
        "Player_/AI/AI player build nearest specified team.",
        &[
            ParameterType::Side,
            ParameterType::ObjectType,
            ParameterType::Team,
        ],
        &["Have AI ", " build a ", " nearest team ", "."],
    ),
];
