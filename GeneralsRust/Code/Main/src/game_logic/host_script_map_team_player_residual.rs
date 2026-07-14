//! Wave 95 residual peels: Script action/condition name tables / MapObject /
//! Waypoint / Team / Player residual deepen.
//!
//! Orthogonal to Waves 76 (ScriptEngine timers), 84 (Relationship enum),
//! 85 (faction/player template/starting cash), 89 (rank/skill points).
//! Host-testable packs for script/map/team/player residual honesty.
//!
//! Sources (retail ZH C++):
//! - Scripts.h ScriptActionType / ConditionType enums + ScriptEngine.cpp
//!   m_internalName residual tables (NUM_ITEMS action 344 / condition 109)
//! - Scripts.h THIS_TEAM / TEAM_THE_PLAYER / skirmish area name tokens
//! - ScriptEngine.h MAX_COUNTERS / MAX_FLAGS / MAX_ATTACK_PRIORITIES **256**
//! - MapObject.h MAP_XY_FACTOR / MAP_HEIGHT_SCALE / FLAG_* / MO_* residual
//! - WellKnownKeys.h object*/waypoint*/team*/player* Dict key residual
//! - TerrainLogic.h Waypoint::MAX_LINKS **8** / INVALID_WAYPOINT_ID
//! - Team.h TEAM_ID_INVALID / MAX_UNIT_TYPES **7** / MAX_GENERIC_SCRIPTS **16**
//! - Player.cpp / PlayerList.h / GameCommon.h player residual deepen
//!
//! Fail-closed:
//! - Not full ScriptAction executor / Condition evaluator residual
//! - Not full MapObject WB validate / WorldDict xfer residual
//! - Not full TerrainLogic waypoint pathfind residual
//! - Not full TeamFactory production / AI recruit residual
//! - Not full Player science purchase / energy matrix residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact C++ SCREAMING_SNAKE match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

/// Case-insensitive residual name lookup (editor UI sometimes varies case).
pub fn residual_name_index_ci(table: &[&str], name: &str) -> Option<usize> {
    table
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------
// 1. Script residual action/condition name tables + engine caps
// ---------------------------------------------------------------------------

/// C++ `ScriptAction::NUM_ITEMS` residual (keep-last sentinel; not a live action).
pub const SCRIPT_ACTION_NUM_ITEMS_RESIDUAL: u32 = 344;
/// C++ `Condition::NUM_ITEMS` residual (keep-last sentinel; not a live condition).
pub const SCRIPT_CONDITION_NUM_ITEMS_RESIDUAL: u32 = 109;
/// C++ `MAX_PARMS` residual (Scripts.h).
pub const SCRIPT_MAX_PARMS_RESIDUAL: usize = 12;
/// C++ `ScriptEngine.h` MAX_COUNTERS residual.
pub const SCRIPT_MAX_COUNTERS_RESIDUAL: usize = 256;
/// C++ `ScriptEngine.h` MAX_FLAGS residual.
pub const SCRIPT_MAX_FLAGS_RESIDUAL: usize = 256;
/// C++ `ScriptEngine.h` MAX_ATTACK_PRIORITIES residual.
pub const SCRIPT_MAX_ATTACK_PRIORITIES_RESIDUAL: usize = 256;

/// C++ Scripts.h well-known script token residual.
pub const SCRIPT_THIS_TEAM: &str = "<This Team>";
pub const SCRIPT_ANY_TEAM: &str = "<Any Team>";
pub const SCRIPT_THIS_OBJECT: &str = "<This Object>";
pub const SCRIPT_ANY_OBJECT: &str = "<Any Object>";
pub const SCRIPT_THIS_PLAYER: &str = "<This Player>";
pub const SCRIPT_LOCAL_PLAYER: &str = "<Local Player>";
pub const SCRIPT_THE_PLAYER: &str = "ThePlayer";
pub const SCRIPT_TEAM_THE_PLAYER: &str = "teamThePlayer";
pub const SCRIPT_THIS_PLAYER_ENEMY: &str = "<This Player's Enemy>";
pub const SCRIPT_WATER_GRID: &str = "Water Grid";
pub const SCRIPT_SKIRMISH_CENTER: &str = "Center";
pub const SCRIPT_SKIRMISH_FLANK: &str = "Flank";
pub const SCRIPT_SKIRMISH_BACKDOOR: &str = "Backdoor";
pub const SCRIPT_SKIRMISH_SPECIAL: &str = "Special";
pub const SCRIPT_INNER_PERIMETER: &str = "InnerPerimeter";
pub const SCRIPT_OUTER_PERIMETER: &str = "OuterPerimeter";

/// Ordered C++ `ScriptActionType` internal names residual (Scripts.h enum order; index = discriminant).
pub const SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL: &[&str] = &[
    "DEBUG_MESSAGE_BOX", // 0
    "SET_FLAG", // 1
    "SET_COUNTER", // 2
    "VICTORY", // 3
    "DEFEAT", // 4
    "NO_OP", // 5
    "SET_TIMER", // 6
    "PLAY_SOUND_EFFECT", // 7
    "ENABLE_SCRIPT", // 8
    "DISABLE_SCRIPT", // 9
    "CALL_SUBROUTINE", // 10
    "PLAY_SOUND_EFFECT_AT", // 11
    "DAMAGE_MEMBERS_OF_TEAM", // 12
    "MOVE_TEAM_TO", // 13
    "MOVE_CAMERA_TO", // 14
    "INCREMENT_COUNTER", // 15
    "DECREMENT_COUNTER", // 16
    "MOVE_CAMERA_ALONG_WAYPOINT_PATH", // 17
    "ROTATE_CAMERA", // 18
    "RESET_CAMERA", // 19
    "SET_MILLISECOND_TIMER", // 20
    "CAMERA_MOD_FREEZE_TIME", // 21
    "SET_VISUAL_SPEED_MULTIPLIER", // 22
    "CREATE_OBJECT", // 23
    "SUSPEND_BACKGROUND_SOUNDS", // 24
    "RESUME_BACKGROUND_SOUNDS", // 25
    "CAMERA_MOD_SET_FINAL_ZOOM", // 26
    "CAMERA_MOD_SET_FINAL_PITCH", // 27
    "CAMERA_MOD_FREEZE_ANGLE", // 28
    "CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER", // 29
    "CAMERA_MOD_SET_ROLLING_AVERAGE", // 30
    "CAMERA_MOD_FINAL_LOOK_TOWARD", // 31
    "CAMERA_MOD_LOOK_TOWARD", // 32
    "TEAM_ATTACK_TEAM", // 33
    "CREATE_REINFORCEMENT_TEAM", // 34
    "MOVE_CAMERA_TO_SELECTION", // 35
    "TEAM_FOLLOW_WAYPOINTS", // 36
    "TEAM_SET_STATE", // 37
    "MOVE_NAMED_UNIT_TO", // 38
    "NAMED_ATTACK_NAMED", // 39
    "CREATE_NAMED_ON_TEAM_AT_WAYPOINT", // 40
    "CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT", // 41
    "NAMED_APPLY_ATTACK_PRIORITY_SET", // 42
    "TEAM_APPLY_ATTACK_PRIORITY_SET", // 43
    "SET_BASE_CONSTRUCTION_SPEED", // 44
    "NAMED_SET_ATTITUDE", // 45
    "TEAM_SET_ATTITUDE", // 46
    "NAMED_ATTACK_AREA", // 47
    "NAMED_ATTACK_TEAM", // 48
    "TEAM_ATTACK_AREA", // 49
    "TEAM_ATTACK_NAMED", // 50
    "TEAM_LOAD_TRANSPORTS", // 51
    "NAMED_ENTER_NAMED", // 52
    "TEAM_ENTER_NAMED", // 53
    "NAMED_EXIT_ALL", // 54
    "TEAM_EXIT_ALL", // 55
    "NAMED_FOLLOW_WAYPOINTS", // 56
    "NAMED_GUARD", // 57
    "TEAM_GUARD", // 58
    "NAMED_HUNT", // 59
    "TEAM_HUNT", // 60
    "PLAYER_SELL_EVERYTHING", // 61
    "PLAYER_DISABLE_BASE_CONSTRUCTION", // 62
    "PLAYER_DISABLE_FACTORIES", // 63
    "PLAYER_DISABLE_UNIT_CONSTRUCTION", // 64
    "PLAYER_ENABLE_BASE_CONSTRUCTION", // 65
    "PLAYER_ENABLE_FACTORIES", // 66
    "PLAYER_ENABLE_UNIT_CONSTRUCTION", // 67
    "CAMERA_MOVE_HOME", // 68
    "BUILD_TEAM", // 69
    "NAMED_DAMAGE", // 70
    "NAMED_DELETE", // 71
    "TEAM_DELETE", // 72
    "NAMED_KILL", // 73
    "TEAM_KILL", // 74
    "PLAYER_KILL", // 75
    "DISPLAY_TEXT", // 76
    "CAMEO_FLASH", // 77
    "NAMED_FLASH", // 78
    "TEAM_FLASH", // 79
    "MOVIE_PLAY_FULLSCREEN", // 80
    "MOVIE_PLAY_RADAR", // 81
    "SOUND_PLAY_NAMED", // 82
    "SPEECH_PLAY", // 83
    "PLAYER_TRANSFER_OWNERSHIP_PLAYER", // 84
    "NAMED_TRANSFER_OWNERSHIP_PLAYER", // 85
    "PLAYER_RELATES_PLAYER", // 86
    "RADAR_CREATE_EVENT", // 87
    "RADAR_DISABLE", // 88
    "RADAR_ENABLE", // 89
    "MAP_REVEAL_AT_WAYPOINT", // 90
    "TEAM_AVAILABLE_FOR_RECRUITMENT", // 91
    "TEAM_COLLECT_NEARBY_FOR_TEAM", // 92
    "TEAM_MERGE_INTO_TEAM", // 93
    "DISABLE_INPUT", // 94
    "ENABLE_INPUT", // 95
    "PLAYER_HUNT", // 96
    "SOUND_AMBIENT_PAUSE", // 97
    "SOUND_AMBIENT_RESUME", // 98
    "MUSIC_SET_TRACK", // 99
    "SET_TREE_SWAY", // 100
    "DEBUG_STRING", // 101
    "MAP_REVEAL_ALL", // 102
    "TEAM_GARRISON_SPECIFIC_BUILDING", // 103
    "EXIT_SPECIFIC_BUILDING", // 104
    "TEAM_GARRISON_NEAREST_BUILDING", // 105
    "TEAM_EXIT_ALL_BUILDINGS", // 106
    "NAMED_GARRISON_SPECIFIC_BUILDING", // 107
    "NAMED_GARRISON_NEAREST_BUILDING", // 108
    "NAMED_EXIT_BUILDING", // 109
    "PLAYER_GARRISON_ALL_BUILDINGS", // 110
    "PLAYER_EXIT_ALL_BUILDINGS", // 111
    "TEAM_WANDER", // 112
    "TEAM_PANIC", // 113
    "SETUP_CAMERA", // 114
    "CAMERA_LETTERBOX_BEGIN", // 115
    "CAMERA_LETTERBOX_END", // 116
    "ZOOM_CAMERA", // 117
    "PITCH_CAMERA", // 118
    "CAMERA_FOLLOW_NAMED", // 119
    "OVERSIZE_TERRAIN", // 120
    "CAMERA_FADE_ADD", // 121
    "CAMERA_FADE_SUBTRACT", // 122
    "CAMERA_FADE_SATURATE", // 123
    "CAMERA_FADE_MULTIPLY", // 124
    "CAMERA_BW_MODE_BEGIN", // 125
    "CAMERA_BW_MODE_END", // 126
    "DRAW_SKYBOX_BEGIN", // 127
    "DRAW_SKYBOX_END", // 128
    "SET_ATTACK_PRIORITY_THING", // 129
    "SET_ATTACK_PRIORITY_KIND_OF", // 130
    "SET_DEFAULT_ATTACK_PRIORITY", // 131
    "CAMERA_STOP_FOLLOW", // 132
    "CAMERA_MOTION_BLUR", // 133
    "CAMERA_MOTION_BLUR_JUMP", // 134
    "CAMERA_MOTION_BLUR_FOLLOW", // 135
    "CAMERA_MOTION_BLUR_END_FOLLOW", // 136
    "FREEZE_TIME", // 137
    "UNFREEZE_TIME", // 138
    "SHOW_MILITARY_CAPTION", // 139
    "CAMERA_SET_AUDIBLE_DISTANCE", // 140
    "SET_STOPPING_DISTANCE", // 141
    "NAMED_SET_STOPPING_DISTANCE", // 142
    "SET_FPS_LIMIT", // 143
    "MUSIC_SET_VOLUME", // 144
    "MAP_SHROUD_AT_WAYPOINT", // 145
    "MAP_SHROUD_ALL", // 146
    "SET_RANDOM_TIMER", // 147
    "SET_RANDOM_MSEC_TIMER", // 148
    "STOP_TIMER", // 149
    "RESTART_TIMER", // 150
    "ADD_TO_MSEC_TIMER", // 151
    "SUB_FROM_MSEC_TIMER", // 152
    "TEAM_TRANSFER_TO_PLAYER", // 153
    "PLAYER_SET_MONEY", // 154
    "PLAYER_GIVE_MONEY", // 155
    "DISABLE_SPECIAL_POWER_DISPLAY", // 156
    "ENABLE_SPECIAL_POWER_DISPLAY", // 157
    "NAMED_HIDE_SPECIAL_POWER_DISPLAY", // 158
    "NAMED_SHOW_SPECIAL_POWER_DISPLAY", // 159
    "DISPLAY_COUNTDOWN_TIMER", // 160
    "HIDE_COUNTDOWN_TIMER", // 161
    "ENABLE_COUNTDOWN_TIMER_DISPLAY", // 162
    "DISABLE_COUNTDOWN_TIMER_DISPLAY", // 163
    "NAMED_STOP_SPECIAL_POWER_COUNTDOWN", // 164
    "NAMED_START_SPECIAL_POWER_COUNTDOWN", // 165
    "NAMED_SET_SPECIAL_POWER_COUNTDOWN", // 166
    "NAMED_ADD_SPECIAL_POWER_COUNTDOWN", // 167
    "NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT", // 168
    "NAMED_FIRE_SPECIAL_POWER_AT_NAMED", // 169
    "REFRESH_RADAR", // 170
    "CAMERA_TETHER_NAMED", // 171
    "CAMERA_STOP_TETHER_NAMED", // 172
    "CAMERA_SET_DEFAULT", // 173
    "NAMED_STOP", // 174
    "TEAM_STOP", // 175
    "TEAM_STOP_AND_DISBAND", // 176
    "RECRUIT_TEAM", // 177
    "TEAM_SET_OVERRIDE_RELATION_TO_TEAM", // 178
    "TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM", // 179
    "TEAM_REMOVE_ALL_OVERRIDE_RELATIONS", // 180
    "CAMERA_LOOK_TOWARD_OBJECT", // 181
    "NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH", // 182
    "TEAM_SET_OVERRIDE_RELATION_TO_PLAYER", // 183
    "TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER", // 184
    "PLAYER_SET_OVERRIDE_RELATION_TO_TEAM", // 185
    "PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM", // 186
    "UNIT_EXECUTE_SEQUENTIAL_SCRIPT", // 187
    "UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING", // 188
    "UNIT_STOP_SEQUENTIAL_SCRIPT", // 189
    "TEAM_EXECUTE_SEQUENTIAL_SCRIPT", // 190
    "TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING", // 191
    "TEAM_STOP_SEQUENTIAL_SCRIPT", // 192
    "UNIT_GUARD_FOR_FRAMECOUNT", // 193
    "UNIT_IDLE_FOR_FRAMECOUNT", // 194
    "TEAM_GUARD_FOR_FRAMECOUNT", // 195
    "TEAM_IDLE_FOR_FRAMECOUNT", // 196
    "WATER_CHANGE_HEIGHT", // 197
    "NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED", // 198
    "NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT", // 199
    "WATER_CHANGE_HEIGHT_OVER_TIME", // 200
    "MAP_SWITCH_BORDER", // 201
    "TEAM_GUARD_POSITION", // 202
    "TEAM_GUARD_OBJECT", // 203
    "TEAM_GUARD_AREA", // 204
    "OBJECT_FORCE_SELECT", // 205
    "CAMERA_LOOK_TOWARD_WAYPOINT", // 206
    "UNIT_DESTROY_ALL_CONTAINED", // 207
    "RADAR_FORCE_ENABLE", // 208
    "RADAR_REVERT_TO_NORMAL", // 209
    "SCREEN_SHAKE", // 210
    "TECHTREE_MODIFY_BUILDABILITY_OBJECT", // 211
    "WAREHOUSE_SET_VALUE", // 212
    "OBJECT_CREATE_RADAR_EVENT", // 213
    "TEAM_CREATE_RADAR_EVENT", // 214
    "DISPLAY_CINEMATIC_TEXT", // 215
    "DEBUG_CRASH_BOX", // 216
    "SOUND_DISABLE_TYPE", // 217
    "SOUND_ENABLE_TYPE", // 218
    "SOUND_ENABLE_ALL", // 219
    "AUDIO_OVERRIDE_VOLUME_TYPE", // 220
    "AUDIO_RESTORE_VOLUME_TYPE", // 221
    "AUDIO_RESTORE_VOLUME_ALL_TYPE", // 222
    "INGAME_POPUP_MESSAGE", // 223
    "SET_CAVE_INDEX", // 224
    "NAMED_SET_HELD", // 225
    "NAMED_SET_TOPPLE_DIRECTION", // 226
    "UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE", // 227
    "TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE", // 228
    "MAP_REVEAL_ALL_PERM", // 229
    "MAP_REVEAL_ALL_UNDO_PERM", // 230
    "NAMED_SET_REPULSOR", // 231
    "TEAM_SET_REPULSOR", // 232
    "TEAM_WANDER_IN_PLACE", // 233
    "TEAM_INCREASE_PRIORITY", // 234
    "TEAM_DECREASE_PRIORITY", // 235
    "DISPLAY_COUNTER", // 236
    "HIDE_COUNTER", // 237
    "TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED", // 238
    "TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT", // 239
    "NAMED_USE_COMMANDBUTTON_ABILITY", // 240
    "TEAM_USE_COMMANDBUTTON_ABILITY", // 241
    "NAMED_FLASH_WHITE", // 242
    "TEAM_FLASH_WHITE", // 243
    "SKIRMISH_BUILD_BUILDING", // 244
    "SKIRMISH_FOLLOW_APPROACH_PATH", // 245
    "IDLE_ALL_UNITS", // 246
    "RESUME_SUPPLY_TRUCKING", // 247
    "NAMED_CUSTOM_COLOR", // 248
    "SKIRMISH_MOVE_TO_APPROACH_PATH", // 249
    "SKIRMISH_BUILD_BASE_DEFENSE_FRONT", // 250
    "SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST", // 251
    "NAMED_RECEIVE_UPGRADE", // 252
    "PLAYER_REPAIR_NAMED_STRUCTURE", // 253
    "SKIRMISH_BUILD_BASE_DEFENSE_FLANK", // 254
    "SKIRMISH_BUILD_STRUCTURE_FRONT", // 255
    "SKIRMISH_BUILD_STRUCTURE_FLANK", // 256
    "SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE", // 257
    "SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT", // 258
    "SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL", // 259
    "SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL", // 260
    "TEAM_SPIN_FOR_FRAMECOUNT", // 261
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED", // 262
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT", // 263
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING", // 264
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF", // 265
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING", // 266
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS", // 267
    "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE", // 268
    "TEAM_PARTIAL_USE_COMMANDBUTTON", // 269
    "TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT", // 270
    "PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS", // 271
    "PLAYER_ADD_SKILLPOINTS", // 272
    "PLAYER_ADD_RANKLEVEL", // 273
    "PLAYER_SET_RANKLEVEL", // 274
    "PLAYER_SET_RANKLEVELLIMIT", // 275
    "PLAYER_GRANT_SCIENCE", // 276
    "PLAYER_PURCHASE_SCIENCE", // 277
    "TEAM_HUNT_WITH_COMMAND_BUTTON", // 278
    "TEAM_WAIT_FOR_NOT_CONTAINED_ALL", // 279
    "TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL", // 280
    "TEAM_FOLLOW_WAYPOINTS_EXACT", // 281
    "NAMED_FOLLOW_WAYPOINTS_EXACT", // 282
    "TEAM_SET_EMOTICON", // 283
    "NAMED_SET_EMOTICON", // 284
    "AI_PLAYER_BUILD_SUPPLY_CENTER", // 285
    "AI_PLAYER_BUILD_UPGRADE", // 286
    "OBJECTLIST_ADDOBJECTTYPE", // 287
    "OBJECTLIST_REMOVEOBJECTTYPE", // 288
    "MAP_REVEAL_PERMANENTLY_AT_WAYPOINT", // 289
    "MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT", // 290
    "NAMED_SET_STEALTH_ENABLED", // 291
    "TEAM_SET_STEALTH_ENABLED", // 292
    "EVA_SET_ENABLED_DISABLED", // 293
    "OPTIONS_SET_OCCLUSION_MODE", // 294
    "LOCALDEFEAT", // 295
    "OPTIONS_SET_DRAWICON_UI_MODE", // 296
    "OPTIONS_SET_PARTICLE_CAP_MODE", // 297
    "PLAYER_SCIENCE_AVAILABILITY", // 298
    "UNIT_AFFECT_OBJECT_PANEL_FLAGS", // 299
    "TEAM_AFFECT_OBJECT_PANEL_FLAGS", // 300
    "PLAYER_SELECT_SKILLSET", // 301
    "SCRIPTING_OVERRIDE_HULK_LIFETIME", // 302
    "NAMED_FACE_NAMED", // 303
    "NAMED_FACE_WAYPOINT", // 304
    "TEAM_FACE_NAMED", // 305
    "TEAM_FACE_WAYPOINT", // 306
    "COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE", // 307
    "COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT", // 308
    "UNIT_SPAWN_NAMED_LOCATION_ORIENTATION", // 309
    "PLAYER_AFFECT_RECEIVING_EXPERIENCE", // 310
    "PLAYER_EXCLUDE_FROM_SCORE_SCREEN", // 311
    "TEAM_GUARD_SUPPLY_CENTER", // 312
    "ENABLE_SCORING", // 313
    "DISABLE_SCORING", // 314
    "SOUND_SET_VOLUME", // 315
    "SPEECH_SET_VOLUME", // 316
    "DISABLE_BORDER_SHROUD", // 317
    "ENABLE_BORDER_SHROUD", // 318
    "OBJECT_ALLOW_BONUSES", // 319
    "SOUND_REMOVE_ALL_DISABLED", // 320
    "SOUND_REMOVE_TYPE", // 321
    "TEAM_GUARD_IN_TUNNEL_NETWORK", // 322
    "QUICKVICTORY", // 323
    "SET_INFANTRY_LIGHTING_OVERRIDE", // 324
    "RESET_INFANTRY_LIGHTING_OVERRIDE", // 325
    "TEAM_DELETE_LIVING", // 326
    "RESIZE_VIEW_GUARDBAND", // 327
    "DELETE_ALL_UNMANNED", // 328
    "CHOOSE_VICTIM_ALWAYS_USES_NORMAL", // 329
    "CAMERA_ENABLE_SLAVE_MODE", // 330
    "CAMERA_DISABLE_SLAVE_MODE", // 331
    "CAMERA_ADD_SHAKER_AT", // 332
    "SET_TRAIN_HELD", // 333
    "NAMED_SET_EVAC_LEFT_OR_RIGHT", // 334
    "ENABLE_OBJECT_SOUND", // 335
    "DISABLE_OBJECT_SOUND", // 336
    "NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH", // 337
    "NAMED_SET_UNMANNED_STATUS", // 338
    "TEAM_SET_UNMANNED_STATUS", // 339
    "NAMED_SET_BOOBYTRAPPED", // 340
    "TEAM_SET_BOOBYTRAPPED", // 341
    "SHOW_WEATHER", // 342
    "AI_PLAYER_BUILD_TYPE_NEAREST_TEAM", // 343
];
/// Ordered C++ `ConditionType` internal names residual (Scripts.h enum order; index = discriminant).
pub const SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL: &[&str] = &[
    "CONDITION_FALSE", // 0
    "COUNTER", // 1
    "FLAG", // 2
    "CONDITION_TRUE", // 3
    "TIMER_EXPIRED", // 4
    "PLAYER_ALL_DESTROYED", // 5
    "PLAYER_ALL_BUILDFACILITIES_DESTROYED", // 6
    "TEAM_INSIDE_AREA_PARTIALLY", // 7
    "TEAM_DESTROYED", // 8
    "CAMERA_MOVEMENT_FINISHED", // 9
    "TEAM_HAS_UNITS", // 10
    "TEAM_STATE_IS", // 11
    "TEAM_STATE_IS_NOT", // 12
    "NAMED_INSIDE_AREA", // 13
    "NAMED_OUTSIDE_AREA", // 14
    "NAMED_DESTROYED", // 15
    "NAMED_NOT_DESTROYED", // 16
    "TEAM_INSIDE_AREA_ENTIRELY", // 17
    "TEAM_OUTSIDE_AREA_ENTIRELY", // 18
    "NAMED_ATTACKED_BY_OBJECTTYPE", // 19
    "TEAM_ATTACKED_BY_OBJECTTYPE", // 20
    "NAMED_ATTACKED_BY_PLAYER", // 21
    "TEAM_ATTACKED_BY_PLAYER", // 22
    "BUILT_BY_PLAYER", // 23
    "NAMED_CREATED", // 24
    "TEAM_CREATED", // 25
    "PLAYER_HAS_CREDITS", // 26
    "NAMED_DISCOVERED", // 27
    "TEAM_DISCOVERED", // 28
    "MISSION_ATTEMPTS", // 29
    "NAMED_OWNED_BY_PLAYER", // 30
    "TEAM_OWNED_BY_PLAYER", // 31
    "PLAYER_HAS_N_OR_FEWER_BUILDINGS", // 32
    "PLAYER_HAS_POWER", // 33
    "NAMED_REACHED_WAYPOINTS_END", // 34
    "TEAM_REACHED_WAYPOINTS_END", // 35
    "NAMED_SELECTED", // 36
    "NAMED_ENTERED_AREA", // 37
    "NAMED_EXITED_AREA", // 38
    "TEAM_ENTERED_AREA_ENTIRELY", // 39
    "TEAM_ENTERED_AREA_PARTIALLY", // 40
    "TEAM_EXITED_AREA_ENTIRELY", // 41
    "TEAM_EXITED_AREA_PARTIALLY", // 42
    "MULTIPLAYER_ALLIED_VICTORY", // 43
    "MULTIPLAYER_ALLIED_DEFEAT", // 44
    "MULTIPLAYER_PLAYER_DEFEAT", // 45
    "PLAYER_HAS_NO_POWER", // 46
    "HAS_FINISHED_VIDEO", // 47
    "HAS_FINISHED_SPEECH", // 48
    "HAS_FINISHED_AUDIO", // 49
    "BUILDING_ENTERED_BY_PLAYER", // 50
    "ENEMY_SIGHTED", // 51
    "UNIT_HEALTH", // 52
    "BRIDGE_REPAIRED", // 53
    "BRIDGE_BROKEN", // 54
    "NAMED_DYING", // 55
    "NAMED_TOTALLY_DEAD", // 56
    "PLAYER_HAS_OBJECT_COMPARISON", // 57
    "OBSOLETE_SCRIPT_1", // 58
    "OBSOLETE_SCRIPT_2", // 59
    "PLAYER_TRIGGERED_SPECIAL_POWER", // 60
    "PLAYER_COMPLETED_SPECIAL_POWER", // 61
    "PLAYER_MIDWAY_SPECIAL_POWER", // 62
    "PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED", // 63
    "PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED", // 64
    "PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED", // 65
    "DEFUNCT_PLAYER_SELECTED_GENERAL", // 66
    "DEFUNCT_PLAYER_SELECTED_GENERAL_FROM_NAMED", // 67
    "PLAYER_BUILT_UPGRADE", // 68
    "PLAYER_BUILT_UPGRADE_FROM_NAMED", // 69
    "PLAYER_DESTROYED_N_BUILDINGS_PLAYER", // 70
    "UNIT_COMPLETED_SEQUENTIAL_EXECUTION", // 71
    "TEAM_COMPLETED_SEQUENTIAL_EXECUTION", // 72
    "PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA", // 73
    "PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA", // 74
    "UNIT_EMPTIED", // 75
    "TYPE_SIGHTED", // 76
    "NAMED_BUILDING_IS_EMPTY", // 77
    "PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS", // 78
    "UNIT_HAS_OBJECT_STATUS", // 79
    "TEAM_ALL_HAS_OBJECT_STATUS", // 80
    "TEAM_SOME_HAVE_OBJECT_STATUS", // 81
    "PLAYER_POWER_COMPARE_PERCENT", // 82
    "PLAYER_EXCESS_POWER_COMPARE_VALUE", // 83
    "SKIRMISH_SPECIAL_POWER_READY", // 84
    "SKIRMISH_VALUE_IN_AREA", // 85
    "SKIRMISH_PLAYER_FACTION", // 86
    "SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE", // 87
    "SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE", // 88
    "SKIRMISH_COMMAND_BUTTON_READY_ALL", // 89
    "SKIRMISH_COMMAND_BUTTON_READY_PARTIAL", // 90
    "SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS", // 91
    "SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD", // 92
    "SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED", // 93
    "SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS", // 94
    "SKIRMISH_NAMED_AREA_EXIST", // 95
    "SKIRMISH_PLAYER_HAS_UNITS_IN_AREA", // 96
    "SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER", // 97
    "SKIRMISH_PLAYER_IS_OUTSIDE_AREA", // 98
    "SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER", // 99
    "PLAYER_ACQUIRED_SCIENCE", // 100
    "PLAYER_HAS_SCIENCEPURCHASEPOINTS", // 101
    "PLAYER_CAN_PURCHASE_SCIENCE", // 102
    "MUSIC_TRACK_HAS_COMPLETED", // 103
    "PLAYER_LOST_OBJECT_TYPE", // 104
    "SUPPLY_SOURCE_SAFE", // 105
    "SUPPLY_SOURCE_ATTACKED", // 106
    "START_POSITION_IS", // 107
    "NAMED_HAS_FREE_CONTAINER_SLOTS", // 108
];
/// Wave 95 honesty: Script action residual name table (full NUM_ITEMS table).
///
/// Fail-closed: not full ActionTemplate UI/help text residual / executor path.
pub fn honesty_script_action_name_table_residual_wave95() -> bool {
    SCRIPT_ACTION_NUM_ITEMS_RESIDUAL == 344
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL.len() == 344
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[0] == "DEBUG_MESSAGE_BOX"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[1] == "SET_FLAG"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[3] == "VICTORY"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[4] == "DEFEAT"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[5] == "NO_OP"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[10] == "CALL_SUBROUTINE"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[23] == "CREATE_OBJECT"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[36] == "TEAM_FOLLOW_WAYPOINTS"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[76] == "DISPLAY_TEXT"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[99] == "MUSIC_SET_TRACK"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[102] == "MAP_REVEAL_ALL"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[342] == "SHOW_WEATHER"
        && SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL[343] == "AI_PLAYER_BUILD_TYPE_NEAREST_TEAM"
        && residual_name_index(SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL, "VICTORY") == Some(3)
        && residual_name_index(SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL, "CALL_SUBROUTINE")
            == Some(10)
        && residual_name_index(SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL, "QUICKVICTORY").is_some()
        && residual_name_index(SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL, "not_an_action")
            .is_none()
        && SCRIPT_MAX_PARMS_RESIDUAL == 12
        && SCRIPT_MAX_COUNTERS_RESIDUAL == 256
        && SCRIPT_MAX_FLAGS_RESIDUAL == 256
        && SCRIPT_MAX_ATTACK_PRIORITIES_RESIDUAL == 256
        && SCRIPT_THIS_TEAM == "<This Team>"
        && SCRIPT_TEAM_THE_PLAYER == "teamThePlayer"
        && SCRIPT_THE_PLAYER == "ThePlayer"
        && SCRIPT_LOCAL_PLAYER == "<Local Player>"
        && SCRIPT_SKIRMISH_CENTER == "Center"
        && SCRIPT_INNER_PERIMETER == "InnerPerimeter"
        && {
            // Uniqueness residual across full action table.
            let mut names: Vec<&str> = SCRIPT_ACTION_INTERNAL_NAME_TABLE_RESIDUAL.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

/// Wave 95 honesty: Script condition residual name table (full NUM_ITEMS table).
///
/// Fail-closed: not full ConditionTemplate UI residual / evaluator path.
pub fn honesty_script_condition_name_table_residual_wave95() -> bool {
    SCRIPT_CONDITION_NUM_ITEMS_RESIDUAL == 109
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL.len() == 109
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[0] == "CONDITION_FALSE"
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[1] == "COUNTER"
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[2] == "FLAG"
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[3] == "CONDITION_TRUE"
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[4] == "TIMER_EXPIRED"
        && SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL[108]
            == "NAMED_HAS_FREE_CONTAINER_SLOTS"
        && residual_name_index(SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL, "CONDITION_TRUE")
            == Some(3)
        && residual_name_index(SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL, "COUNTER")
            == Some(1)
        && residual_name_index(SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL, "not_a_cond")
            .is_none()
        && residual_name_index_ci(SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL, "condition_true")
            == Some(3)
        && {
            let mut names: Vec<&str> = SCRIPT_CONDITION_INTERNAL_NAME_TABLE_RESIDUAL.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// 2. MapObject residual peels (MapObject.h + object* WellKnownKeys)
// ---------------------------------------------------------------------------

/// C++ `MAP_XY_FACTOR` residual.
pub const MAP_XY_FACTOR_RESIDUAL: f32 = 10.0;
/// C++ `MAP_HEIGHT_SCALE` residual (`MAP_XY_FACTOR/16`).
pub const MAP_HEIGHT_SCALE_RESIDUAL: f32 = MAP_XY_FACTOR_RESIDUAL / 16.0;

/// C++ MapObject FLAG_* residual bits.
pub const MAP_FLAG_DRAWS_IN_MIRROR: u32 = 0x0000_0001;
pub const MAP_FLAG_ROAD_POINT1: u32 = 0x0000_0002;
pub const MAP_FLAG_ROAD_POINT2: u32 = 0x0000_0004;
pub const MAP_FLAG_ROAD_FLAGS: u32 = MAP_FLAG_ROAD_POINT1 | MAP_FLAG_ROAD_POINT2;
pub const MAP_FLAG_ROAD_CORNER_ANGLED: u32 = 0x0000_0008;
pub const MAP_FLAG_BRIDGE_POINT1: u32 = 0x0000_0010;
pub const MAP_FLAG_BRIDGE_POINT2: u32 = 0x0000_0020;
pub const MAP_FLAG_BRIDGE_FLAGS: u32 = MAP_FLAG_BRIDGE_POINT1 | MAP_FLAG_BRIDGE_POINT2;
pub const MAP_FLAG_ROAD_CORNER_TIGHT: u32 = 0x0000_0040;
pub const MAP_FLAG_ROAD_JOIN: u32 = 0x0000_0080;
pub const MAP_FLAG_DONT_RENDER: u32 = 0x0000_0100;

/// C++ MapObject runtime MO_* residual bits.
pub const MAP_MO_SELECTED: u32 = 0x01;
pub const MAP_MO_LIGHT: u32 = 0x02;
pub const MAP_MO_WAYPOINT: u32 = 0x04;
pub const MAP_MO_SCORCH: u32 = 0x08;

/// C++ WellKnownKeys object* Dict key residual (MapObject property sheet).
pub const MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL: &[&str] = &[
    "objectName",
    "objectInitialHealth",
    "objectMaxHPs",
    "objectEnabled",
    "objectIndestructible",
    "objectUnsellable",
    "objectTargetable",
    "objectPowered",
    "objectScriptAttachment",
    "objectAggressiveness",
    "objectVisualRange",
    "objectShroudClearingDistance",
    "objectGroupNumber",
    "objectRecruitableAI",
    "objectSelectable",
    "objectVeterancy",
    "objectTime",
    "objectWeather",
    "objectRadius",
    "objectStoppingDistance",
    "objectLayer",
    "objectGrantUpgrade",
    "objectSoundAmbient",
    "objectSoundAmbientCustomized",
    "objectSoundAmbientEnabled",
    "objectSoundAmbientLooping",
    "objectSoundAmbientLoopCount",
    "objectSoundAmbientMinVolume",
    "objectSoundAmbientVolume",
    "objectSoundAmbientMinRange",
    "objectSoundAmbientMaxRange",
    "objectSoundAmbientPriority",
];

/// World-dict / map-level residual keys used with MapObject list.
pub const MAP_WORLD_DICT_KEY_MAP_NAME: &str = "mapName";
pub const MAP_WORLD_DICT_KEY_INITIAL_CAMERA: &str = "InitialCameraPosition";
pub const MAP_OBJECT_DICT_KEY_ORIGINAL_OWNER: &str = "originalOwner";
pub const MAP_OBJECT_DICT_KEY_UNIQUE_ID: &str = "uniqueID";

/// Wave 95 honesty: MapObject residual peels pack.
///
/// Fail-closed: not full WB validate / bridge tower render residual.
pub fn honesty_map_object_residual_pack_wave95() -> bool {
    MAP_XY_FACTOR_RESIDUAL == 10.0
        && (MAP_HEIGHT_SCALE_RESIDUAL - 0.625).abs() < 1e-6
        && MAP_FLAG_DRAWS_IN_MIRROR == 0x1
        && MAP_FLAG_ROAD_POINT1 == 0x2
        && MAP_FLAG_ROAD_POINT2 == 0x4
        && MAP_FLAG_ROAD_FLAGS == 0x6
        && MAP_FLAG_ROAD_CORNER_ANGLED == 0x8
        && MAP_FLAG_BRIDGE_POINT1 == 0x10
        && MAP_FLAG_BRIDGE_POINT2 == 0x20
        && MAP_FLAG_BRIDGE_FLAGS == 0x30
        && MAP_FLAG_ROAD_CORNER_TIGHT == 0x40
        && MAP_FLAG_ROAD_JOIN == 0x80
        && MAP_FLAG_DONT_RENDER == 0x100
        && MAP_MO_SELECTED == 0x01
        && MAP_MO_LIGHT == 0x02
        && MAP_MO_WAYPOINT == 0x04
        && MAP_MO_SCORCH == 0x08
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL.len() == 32
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL[0] == "objectName"
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL[1] == "objectInitialHealth"
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL.contains(&"objectVeterancy")
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL.contains(&"objectScriptAttachment")
        && MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL.contains(&"objectSelectable")
        && MAP_WORLD_DICT_KEY_MAP_NAME == "mapName"
        && MAP_WORLD_DICT_KEY_INITIAL_CAMERA == "InitialCameraPosition"
        && MAP_OBJECT_DICT_KEY_ORIGINAL_OWNER == "originalOwner"
        && MAP_OBJECT_DICT_KEY_UNIQUE_ID == "uniqueID"
        && residual_name_index(MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL, "objectName") == Some(0)
        && residual_name_index(MAP_OBJECT_DICT_KEY_TABLE_RESIDUAL, "not_a_key").is_none()
}

// ---------------------------------------------------------------------------
// 3. Waypoint residual peels (TerrainLogic.h + waypoint* WellKnownKeys)
// ---------------------------------------------------------------------------

/// C++ `INVALID_WAYPOINT_ID` residual (`0x7FFFFFFF`).
pub const INVALID_WAYPOINT_ID_RESIDUAL: u32 = 0x7FFF_FFFF;
/// C++ `Waypoint::MAX_LINKS` residual.
pub const WAYPOINT_MAX_LINKS_RESIDUAL: usize = 8;

/// C++ WellKnownKeys waypoint* Dict key residual.
pub const WAYPOINT_DICT_KEY_TABLE_RESIDUAL: &[&str] = &[
    "waypointName",
    "waypointID",
    "waypointPathLabel1",
    "waypointPathLabel2",
    "waypointPathLabel3",
    "waypointPathBiDirectional",
];

/// Wave 95 honesty: Waypoint residual peels pack.
///
/// Fail-closed: not full TerrainLogic path label walk / bi-directional link residual.
pub fn honesty_waypoint_residual_pack_wave95() -> bool {
    INVALID_WAYPOINT_ID_RESIDUAL == 0x7FFF_FFFF
        && WAYPOINT_MAX_LINKS_RESIDUAL == 8
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL.len() == 6
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[0] == "waypointName"
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[1] == "waypointID"
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[2] == "waypointPathLabel1"
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[3] == "waypointPathLabel2"
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[4] == "waypointPathLabel3"
        && WAYPOINT_DICT_KEY_TABLE_RESIDUAL[5] == "waypointPathBiDirectional"
        && residual_name_index(WAYPOINT_DICT_KEY_TABLE_RESIDUAL, "waypointID") == Some(1)
        && residual_name_index(WAYPOINT_DICT_KEY_TABLE_RESIDUAL, "waypointPathLabel3")
            == Some(4)
        && residual_name_index(WAYPOINT_DICT_KEY_TABLE_RESIDUAL, "not_a_wp").is_none()
        // Cross-link: MapObject MO_WAYPOINT bit residual for waypoint map objects.
        && MAP_MO_WAYPOINT == 0x04
}

// ---------------------------------------------------------------------------
// 4. Team residual peels (Team.h + team* WellKnownKeys)
// ---------------------------------------------------------------------------

/// C++ `TEAM_ID_INVALID` residual.
pub const TEAM_ID_INVALID_RESIDUAL: u32 = 0;
/// C++ `TEAM_PROTOTYPE_ID_INVALID` residual.
pub const TEAM_PROTOTYPE_ID_INVALID_RESIDUAL: u32 = 0;
/// C++ `TeamTemplateInfo::MAX_UNIT_TYPES` residual.
pub const TEAM_MAX_UNIT_TYPES_RESIDUAL: usize = 7;
/// C++ `MAX_GENERIC_SCRIPTS` residual.
pub const TEAM_MAX_GENERIC_SCRIPTS_RESIDUAL: usize = 16;

/// C++ `TeamTemplateInfo::TBehavior` residual ordinals.
pub const TEAM_BEHAVIOR_NORMAL: u32 = 0;
pub const TEAM_BEHAVIOR_IGNORE_DISTRACTIONS: u32 = 1;
pub const TEAM_BEHAVIOR_DEAL_AGGRESSIVELY: u32 = 2;

/// Default team name residual: `"team" + playerName` (Player/Team namespace).
pub fn default_player_team_name_residual(player_name: &str) -> String {
    format!("team{player_name}")
}

/// C++ WellKnownKeys team* Dict key residual (TeamTemplate / map team dict).
pub const TEAM_DICT_KEY_TABLE_RESIDUAL: &[&str] = &[
    "teamName",
    "teamOwner",
    "teamIsSingleton",
    "teamHome",
    "teamUnitType1",
    "teamUnitMinCount1",
    "teamUnitMaxCount1",
    "teamUnitType2",
    "teamUnitMinCount2",
    "teamUnitMaxCount2",
    "teamUnitType3",
    "teamUnitMinCount3",
    "teamUnitMaxCount3",
    "teamUnitType4",
    "teamUnitMinCount4",
    "teamUnitMaxCount4",
    "teamUnitType5",
    "teamUnitMinCount5",
    "teamUnitMaxCount5",
    "teamUnitType6",
    "teamUnitMinCount6",
    "teamUnitMaxCount6",
    "teamUnitType7",
    "teamUnitMinCount7",
    "teamUnitMaxCount7",
    "teamOnCreateScript",
    "teamOnIdleScript",
    "teamInitialIdleFrames",
    "teamOnUnitDestroyedScript",
    "teamOnDestroyedScript",
    "teamDestroyedThreshold",
    "teamEnemySightedScript",
    "teamAllClearScript",
    "teamAutoReinforce",
    "teamIsAIRecruitable",
    "teamIsBaseDefense",
    "teamIsPerimeterDefense",
    "teamAggressiveness",
    "teamTransportsReturn",
    "teamAvoidThreats",
    "teamAttackCommonTarget",
    "teamMaxInstances",
    "teamDescription",
    "teamProductionCondition",
    "teamProductionPriority",
    "teamProductionPrioritySuccessIncrease",
    "teamProductionPriorityFailureDecrease",
    "teamTransport",
    "teamReinforcementOrigin",
    "teamStartsFull",
    "teamTransportsExit",
    "teamVeterancy",
    "teamExecutesActionsOnCreate",
    "teamGenericScriptHook",
];

/// Wave 95 honesty: Team residual peels pack.
///
/// Fail-closed: not full TeamFactory production / recruit / generic script residual.
pub fn honesty_team_residual_pack_wave95() -> bool {
    TEAM_ID_INVALID_RESIDUAL == 0
        && TEAM_PROTOTYPE_ID_INVALID_RESIDUAL == 0
        && TEAM_MAX_UNIT_TYPES_RESIDUAL == 7
        && TEAM_MAX_GENERIC_SCRIPTS_RESIDUAL == 16
        && TEAM_BEHAVIOR_NORMAL == 0
        && TEAM_BEHAVIOR_IGNORE_DISTRACTIONS == 1
        && TEAM_BEHAVIOR_DEAL_AGGRESSIVELY == 2
        && TEAM_DICT_KEY_TABLE_RESIDUAL.len() == 54
        && TEAM_DICT_KEY_TABLE_RESIDUAL[0] == "teamName"
        && TEAM_DICT_KEY_TABLE_RESIDUAL[1] == "teamOwner"
        && TEAM_DICT_KEY_TABLE_RESIDUAL[2] == "teamIsSingleton"
        && TEAM_DICT_KEY_TABLE_RESIDUAL[3] == "teamHome"
        && TEAM_DICT_KEY_TABLE_RESIDUAL[4] == "teamUnitType1"
        && TEAM_DICT_KEY_TABLE_RESIDUAL.contains(&"teamUnitType7")
        && TEAM_DICT_KEY_TABLE_RESIDUAL.contains(&"teamOnCreateScript")
        && TEAM_DICT_KEY_TABLE_RESIDUAL.contains(&"teamProductionCondition")
        && TEAM_DICT_KEY_TABLE_RESIDUAL.contains(&"teamGenericScriptHook")
        && TEAM_DICT_KEY_TABLE_RESIDUAL.last() == Some(&"teamGenericScriptHook")
        && residual_name_index(TEAM_DICT_KEY_TABLE_RESIDUAL, "teamName") == Some(0)
        && residual_name_index(TEAM_DICT_KEY_TABLE_RESIDUAL, "teamUnitMaxCount7").is_some()
        && residual_name_index(TEAM_DICT_KEY_TABLE_RESIDUAL, "not_a_team").is_none()
        && default_player_team_name_residual("PlyrCivilian") == "teamPlyrCivilian"
        && default_player_team_name_residual("America") == "teamAmerica"
        && SCRIPT_TEAM_THE_PLAYER == "teamThePlayer"
}

// ---------------------------------------------------------------------------
// 5. Player residual peels deepen (beyond Wave 85 faction/template/cash)
// ---------------------------------------------------------------------------

/// C++ `GameCommon.h MAX_PLAYER_COUNT` residual (cross-link Wave 85).
pub const PLAYER_MAX_COUNT_RESIDUAL: usize = 16;
/// C++ neutral player list slot residual (`PlayerList::getNeutralPlayer` → index 0).
pub const PLAYER_NEUTRAL_INDEX_RESIDUAL: usize = 0;
/// C++ `NEUTRAL_PLAYER_COLOR` residual (`Player.cpp`).
pub const PLAYER_NEUTRAL_COLOR_ARGB_RESIDUAL: u32 = 0xFFFF_FFFF;
/// C++ `Player::m_skillPointsModifier` ctor residual.
pub const PLAYER_SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ `Player::m_rankLevel` ctor residual.
pub const PLAYER_RANK_LEVEL_DEFAULT_RESIDUAL: i32 = 0;
/// C++ `Player::m_skillPoints` ctor residual.
pub const PLAYER_SKILL_POINTS_DEFAULT_RESIDUAL: i32 = 0;
/// C++ self-relationship residual (`setPlayerRelationship(this, ALLIES)`).
pub const PLAYER_SELF_RELATIONSHIP_ALLIES_RESIDUAL: u32 = 2;
/// C++ default unknown-team relationship residual (`getRelationship` → NEUTRAL).
pub const PLAYER_DEFAULT_RELATIONSHIP_NEUTRAL_RESIDUAL: u32 = 1;
/// C++ Relationship ENEMIES residual (cross-link Wave 84).
pub const PLAYER_RELATIONSHIP_ENEMIES_RESIDUAL: u32 = 0;

/// C++ WellKnownKeys player* Dict key residual (SidesList / map player dict).
pub const PLAYER_DICT_KEY_TABLE_RESIDUAL: &[&str] = &[
    "playerName",
    "playerIsHuman",
    "playerIsSkirmish",
    "playerDisplayName",
    "playerFaction",
    "playerEnemies",
    "playerAllies",
    "playerStartMoney",
    "playerColor",
    "playerNightColor",
    "playerIsPreorder",
];

/// C++ player mask residual: `1 << playerIndex`.
pub fn player_mask_residual(player_index: u32) -> u32 {
    1u32 << player_index
}

/// Wave 95 honesty: Player residual peels deepen pack.
///
/// Fail-closed: not full science purchase / energy / multipayload residual.
pub fn honesty_player_residual_deepen_pack_wave95() -> bool {
    PLAYER_MAX_COUNT_RESIDUAL == 16
        && PLAYER_NEUTRAL_INDEX_RESIDUAL == 0
        && PLAYER_NEUTRAL_COLOR_ARGB_RESIDUAL == 0xFFFF_FFFF
        && (PLAYER_SKILL_POINTS_MODIFIER_DEFAULT_RESIDUAL - 1.0).abs() < 1e-6
        && PLAYER_RANK_LEVEL_DEFAULT_RESIDUAL == 0
        && PLAYER_SKILL_POINTS_DEFAULT_RESIDUAL == 0
        && PLAYER_SELF_RELATIONSHIP_ALLIES_RESIDUAL == 2
        && PLAYER_DEFAULT_RELATIONSHIP_NEUTRAL_RESIDUAL == 1
        && PLAYER_RELATIONSHIP_ENEMIES_RESIDUAL == 0
        && player_mask_residual(0) == 1
        && player_mask_residual(1) == 2
        && player_mask_residual(3) == 8
        && player_mask_residual(15) == 0x8000
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.len() == 11
        && PLAYER_DICT_KEY_TABLE_RESIDUAL[0] == "playerName"
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerIsHuman")
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerFaction")
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerStartMoney")
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerColor")
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerAllies")
        && PLAYER_DICT_KEY_TABLE_RESIDUAL.contains(&"playerEnemies")
        && residual_name_index(PLAYER_DICT_KEY_TABLE_RESIDUAL, "playerName") == Some(0)
        && residual_name_index(PLAYER_DICT_KEY_TABLE_RESIDUAL, "not_a_player").is_none()
        // Empty string playerName residual denotes Neutral (WellKnownKeys comment).
        && SCRIPT_THE_PLAYER == "ThePlayer"
        && default_player_team_name_residual("") == "team"
}

// ---------------------------------------------------------------------------
// Combined Wave 95 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 95 residual honesty pack.
pub fn honesty_script_map_team_player_residual_pack_wave95() -> bool {
    honesty_script_action_name_table_residual_wave95()
        && honesty_script_condition_name_table_residual_wave95()
        && honesty_map_object_residual_pack_wave95()
        && honesty_waypoint_residual_pack_wave95()
        && honesty_team_residual_pack_wave95()
        && honesty_player_residual_deepen_pack_wave95()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_action_name_table_wave95_honesty() {
        assert!(honesty_script_action_name_table_residual_wave95());
    }

    #[test]
    fn script_condition_name_table_wave95_honesty() {
        assert!(honesty_script_condition_name_table_residual_wave95());
    }

    #[test]
    fn map_object_residual_wave95_honesty() {
        assert!(honesty_map_object_residual_pack_wave95());
    }

    #[test]
    fn waypoint_residual_wave95_honesty() {
        assert!(honesty_waypoint_residual_pack_wave95());
    }

    #[test]
    fn team_residual_wave95_honesty() {
        assert!(honesty_team_residual_pack_wave95());
    }

    #[test]
    fn player_residual_deepen_wave95_honesty() {
        assert!(honesty_player_residual_deepen_pack_wave95());
    }

    #[test]
    fn script_map_team_player_residual_pack_wave95_honesty() {
        assert!(honesty_script_map_team_player_residual_pack_wave95());
    }
}
