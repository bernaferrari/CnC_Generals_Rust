// FILE: ini_blocks.rs
// Author: Ported from C++ by Claude Code
// Desc: INI Block Parsers
//
// This module contains the block parsing system for different INI block types.
// Each block type (Object, Weapon, ParticleSystem, etc.) has its own parser.

use super::ini::*;

/// Block parse table entry
#[derive(Clone)]
pub struct BlockParse {
    pub token: &'static str,
    pub parse: INIBlockParse,
}

impl BlockParse {
    pub const fn new(token: &'static str, parse: INIBlockParse) -> Self {
        Self { token, parse }
    }
}

/// The main type table for all INI block types
/// This matches the theTypeTable in INI.cpp
pub fn get_type_table() -> &'static [BlockParse] {
    &TYPE_TABLE
}

/// Find a block parser by token name
pub fn find_block_parse(token: &str) -> Option<INIBlockParse> {
    for entry in TYPE_TABLE.iter() {
        if entry.token.eq_ignore_ascii_case(token) {
            return Some(entry.parse);
        }
    }
    None
}

// Placeholder block parsers - these would be implemented based on the actual game data structures

pub fn parse_ai_data_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse AIData block
    skip_to_end(ini)
}

pub fn parse_anim_2d_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Animation block
    skip_to_end(ini)
}

pub fn parse_armor_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Armor block
    skip_to_end(ini)
}

pub fn parse_audio_event_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse AudioEvent block
    skip_to_end(ini)
}

pub fn parse_audio_settings_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse AudioSettings block
    skip_to_end(ini)
}

pub fn parse_terrain_bridge_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Bridge block
    skip_to_end(ini)
}

pub fn parse_campaign_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Campaign block
    skip_to_end(ini)
}

pub fn parse_challenge_mode_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ChallengeGenerals block
    skip_to_end(ini)
}

pub fn parse_command_button_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse CommandButton block
    skip_to_end(ini)
}

pub fn parse_meta_map_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse CommandMap block
    skip_to_end(ini)
}

pub fn parse_command_set_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse CommandSet block
    skip_to_end(ini)
}

pub fn parse_control_bar_scheme_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ControlBarScheme block
    skip_to_end(ini)
}

pub fn parse_control_bar_resizer_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ControlBarResizer block
    skip_to_end(ini)
}

pub fn parse_crate_template_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse CrateData block
    skip_to_end(ini)
}

pub fn parse_credits(ini: &mut INI) -> INIResult<()> {
    // Would parse Credits block
    skip_to_end(ini)
}

pub fn parse_window_transitions(ini: &mut INI) -> INIResult<()> {
    // Would parse WindowTransition block
    skip_to_end(ini)
}

pub fn parse_damage_fx_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse DamageFX block
    skip_to_end(ini)
}

pub fn parse_dialog_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse DialogEvent block
    skip_to_end(ini)
}

pub fn parse_draw_group_number_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse DrawGroupInfo block
    skip_to_end(ini)
}

pub fn parse_eva_event(ini: &mut INI) -> INIResult<()> {
    // Would parse EvaEvent block
    skip_to_end(ini)
}

pub fn parse_fx_list_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse FXList block
    skip_to_end(ini)
}

pub fn parse_game_data_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse GameData block
    skip_to_end(ini)
}

pub fn parse_in_game_ui_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse InGameUI block
    skip_to_end(ini)
}

pub fn parse_locomotor_template_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Locomotor block
    skip_to_end(ini)
}

pub fn parse_language_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Language block
    skip_to_end(ini)
}

pub fn parse_map_cache_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MapCache block
    skip_to_end(ini)
}

pub fn parse_map_data_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MapData block
    skip_to_end(ini)
}

pub fn parse_mapped_image_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MappedImage block
    skip_to_end(ini)
}

pub fn parse_misc_audio(ini: &mut INI) -> INIResult<()> {
    // Would parse MiscAudio block
    skip_to_end(ini)
}

pub fn parse_mouse_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Mouse block
    skip_to_end(ini)
}

pub fn parse_mouse_cursor_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MouseCursor block
    skip_to_end(ini)
}

pub fn parse_multiplayer_color_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MultiplayerColor block
    skip_to_end(ini)
}

pub fn parse_multiplayer_starting_money_choice_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MultiplayerStartingMoneyChoice block
    skip_to_end(ini)
}

pub fn parse_online_chat_color_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse OnlineChatColors block
    skip_to_end(ini)
}

pub fn parse_multiplayer_settings_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MultiplayerSettings block
    skip_to_end(ini)
}

pub fn parse_music_track_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse MusicTrack block
    skip_to_end(ini)
}

pub fn parse_object_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Object block
    skip_to_end(ini)
}

pub fn parse_object_creation_list_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ObjectCreationList block
    skip_to_end(ini)
}

pub fn parse_object_reskin_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ObjectReskin block
    skip_to_end(ini)
}

pub fn parse_particle_system_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ParticleSystem block
    skip_to_end(ini)
}

pub fn parse_player_template_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse PlayerTemplate block
    skip_to_end(ini)
}

pub fn parse_terrain_road_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Road block
    skip_to_end(ini)
}

pub fn parse_science_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Science block
    skip_to_end(ini)
}

pub fn parse_rank_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Rank block
    skip_to_end(ini)
}

pub fn parse_special_power_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse SpecialPower block
    skip_to_end(ini)
}

pub fn parse_shell_menu_scheme_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse ShellMenuScheme block
    skip_to_end(ini)
}

pub fn parse_terrain_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Terrain block
    skip_to_end(ini)
}

pub fn parse_upgrade_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Upgrade block
    skip_to_end(ini)
}

pub fn parse_video_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Video block
    skip_to_end(ini)
}

pub fn parse_water_setting_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse WaterSet block
    skip_to_end(ini)
}

pub fn parse_water_transparency_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse WaterTransparency block
    skip_to_end(ini)
}

pub fn parse_weather_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Weather block
    skip_to_end(ini)
}

pub fn parse_weapon_template_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse Weapon block
    skip_to_end(ini)
}

pub fn parse_webpage_url_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse WebpageURL block
    skip_to_end(ini)
}

pub fn parse_header_template_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse HeaderTemplate block
    skip_to_end(ini)
}

pub fn parse_static_game_lod_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse StaticGameLOD block
    skip_to_end(ini)
}

pub fn parse_dynamic_game_lod_definition(ini: &mut INI) -> INIResult<()> {
    // Would parse DynamicGameLOD block
    skip_to_end(ini)
}

pub fn parse_lod_preset(ini: &mut INI) -> INIResult<()> {
    // Would parse LODPreset block
    skip_to_end(ini)
}

pub fn parse_bench_profile(ini: &mut INI) -> INIResult<()> {
    // Would parse BenchProfile block
    skip_to_end(ini)
}

pub fn parse_really_low_mhz(ini: &mut INI) -> INIResult<()> {
    // Would parse ReallyLowMHz block
    skip_to_end(ini)
}

/// Helper function to skip to end of block
fn skip_to_end(ini: &mut INI) -> INIResult<()> {
    let mut done = false;

    while !done {
        ini.read_line()?;

        if INI::is_end_of_block(&ini.buffer) {
            done = true;
        }

        if !done && ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }
    }

    Ok(())
}

/// The main type table - matches C++ theTypeTable
static TYPE_TABLE: &[BlockParse] = &[
    BlockParse::new("AIData", parse_ai_data_definition),
    BlockParse::new("Animation", parse_anim_2d_definition),
    BlockParse::new("Armor", parse_armor_definition),
    BlockParse::new("AudioEvent", parse_audio_event_definition),
    BlockParse::new("AudioSettings", parse_audio_settings_definition),
    BlockParse::new("Bridge", parse_terrain_bridge_definition),
    BlockParse::new("Campaign", parse_campaign_definition),
    BlockParse::new("ChallengeGenerals", parse_challenge_mode_definition),
    BlockParse::new("CommandButton", parse_command_button_definition),
    BlockParse::new("CommandMap", parse_meta_map_definition),
    BlockParse::new("CommandSet", parse_command_set_definition),
    BlockParse::new("ControlBarScheme", parse_control_bar_scheme_definition),
    BlockParse::new("ControlBarResizer", parse_control_bar_resizer_definition),
    BlockParse::new("CrateData", parse_crate_template_definition),
    BlockParse::new("Credits", parse_credits),
    BlockParse::new("WindowTransition", parse_window_transitions),
    BlockParse::new("DamageFX", parse_damage_fx_definition),
    BlockParse::new("DialogEvent", parse_dialog_definition),
    BlockParse::new("DrawGroupInfo", parse_draw_group_number_definition),
    BlockParse::new("EvaEvent", parse_eva_event),
    BlockParse::new("FXList", parse_fx_list_definition),
    BlockParse::new("GameData", parse_game_data_definition),
    BlockParse::new("InGameUI", parse_in_game_ui_definition),
    BlockParse::new("Locomotor", parse_locomotor_template_definition),
    BlockParse::new("Language", parse_language_definition),
    BlockParse::new("MapCache", parse_map_cache_definition),
    BlockParse::new("MapData", parse_map_data_definition),
    BlockParse::new("MappedImage", parse_mapped_image_definition),
    BlockParse::new("MiscAudio", parse_misc_audio),
    BlockParse::new("Mouse", parse_mouse_definition),
    BlockParse::new("MouseCursor", parse_mouse_cursor_definition),
    BlockParse::new("MultiplayerColor", parse_multiplayer_color_definition),
    BlockParse::new("MultiplayerStartingMoneyChoice", parse_multiplayer_starting_money_choice_definition),
    BlockParse::new("OnlineChatColors", parse_online_chat_color_definition),
    BlockParse::new("MultiplayerSettings", parse_multiplayer_settings_definition),
    BlockParse::new("MusicTrack", parse_music_track_definition),
    BlockParse::new("Object", parse_object_definition),
    BlockParse::new("ObjectCreationList", parse_object_creation_list_definition),
    BlockParse::new("ObjectReskin", parse_object_reskin_definition),
    BlockParse::new("ParticleSystem", parse_particle_system_definition),
    BlockParse::new("PlayerTemplate", parse_player_template_definition),
    BlockParse::new("Road", parse_terrain_road_definition),
    BlockParse::new("Science", parse_science_definition),
    BlockParse::new("Rank", parse_rank_definition),
    BlockParse::new("SpecialPower", parse_special_power_definition),
    BlockParse::new("ShellMenuScheme", parse_shell_menu_scheme_definition),
    BlockParse::new("Terrain", parse_terrain_definition),
    BlockParse::new("Upgrade", parse_upgrade_definition),
    BlockParse::new("Video", parse_video_definition),
    BlockParse::new("WaterSet", parse_water_setting_definition),
    BlockParse::new("WaterTransparency", parse_water_transparency_definition),
    BlockParse::new("Weather", parse_weather_definition),
    BlockParse::new("Weapon", parse_weapon_template_definition),
    BlockParse::new("WebpageURL", parse_webpage_url_definition),
    BlockParse::new("HeaderTemplate", parse_header_template_definition),
    BlockParse::new("StaticGameLOD", parse_static_game_lod_definition),
    BlockParse::new("DynamicGameLOD", parse_dynamic_game_lod_definition),
    BlockParse::new("LODPreset", parse_lod_preset),
    BlockParse::new("BenchProfile", parse_bench_profile),
    BlockParse::new("ReallyLowMHz", parse_really_low_mhz),
];
