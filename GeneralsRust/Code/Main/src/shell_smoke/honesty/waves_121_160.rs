//! Early residual honesty band (waves 121–160). No playable_claim flip.
//!
//! Owns this band's field subset and evaluate assignments.
//! Public `WaveHonesty`/`EarlyHonesty` stay flat via `from_parts`.

#![allow(unused_imports, unused_variables)]

use super::super::imports::*;

pub(super) struct Waves121160 {
    pub save_load_layout_wave121_ok: bool,
    pub save_load_control_stems_wave121_ok: bool,
    pub save_load_nav_commands_wave121_ok: bool,
    pub replay_control_names_wave122_ok: bool,
    pub replay_nav_commands_wave122_ok: bool,
    pub quit_control_names_wave123_ok: bool,
    pub quit_nav_commands_wave123_ok: bool,
    pub keyboard_control_names_wave124_ok: bool,
    pub keyboard_nav_commands_wave124_ok: bool,
    pub score_control_names_wave125_ok: bool,
    pub score_nav_commands_wave125_ok: bool,
    pub options_control_names_wave126_ok: bool,
    pub options_nav_commands_wave126_ok: bool,
    pub credits_control_names_wave127_ok: bool,
    pub credits_nav_commands_wave127_ok: bool,
    pub message_box_control_names_wave128_ok: bool,
    pub message_box_nav_commands_wave128_ok: bool,
    pub diplomacy_control_names_wave129_ok: bool,
    pub diplomacy_nav_commands_wave129_ok: bool,
    pub popup_replay_control_names_wave130_ok: bool,
    pub popup_replay_nav_commands_wave130_ok: bool,
    pub single_player_control_names_wave131_ok: bool,
    pub single_player_nav_commands_wave131_ok: bool,
    pub map_select_control_names_wave132_ok: bool,
    pub map_select_nav_commands_wave132_ok: bool,
    pub control_bar_control_names_wave133_ok: bool,
    pub control_bar_nav_commands_wave133_ok: bool,
    pub difficulty_select_control_names_wave134_ok: bool,
    pub difficulty_select_nav_commands_wave134_ok: bool,
    pub loading_screen_stages_wave135_ok: bool,
    pub loading_screen_nav_commands_wave135_ok: bool,
    pub in_game_chat_control_names_wave136_ok: bool,
    pub in_game_chat_nav_commands_wave136_ok: bool,
    pub idle_worker_control_names_wave137_ok: bool,
    pub idle_worker_nav_commands_wave137_ok: bool,
    pub generals_exp_control_names_wave138_ok: bool,
    pub generals_exp_nav_commands_wave138_ok: bool,
    pub popup_communicator_control_names_wave139_ok: bool,
    pub popup_communicator_nav_commands_wave139_ok: bool,
    pub replay_control_control_names_wave140_ok: bool,
    pub replay_control_nav_commands_wave140_ok: bool,
    pub shell_map_names_wave141_ok: bool,
    pub shell_map_nav_commands_wave141_ok: bool,
    pub beacon_control_names_wave142_ok: bool,
    pub beacon_nav_commands_wave142_ok: bool,
    pub eva_message_names_wave143_ok: bool,
    pub eva_nav_commands_wave143_ok: bool,
    pub ime_message_names_wave144_ok: bool,
    pub ime_nav_commands_wave144_ok: bool,
    pub smudge_method_names_wave145_ok: bool,
    pub smudge_nav_commands_wave145_ok: bool,
    pub ocl_timer_method_names_wave146_ok: bool,
    pub ocl_timer_nav_commands_wave146_ok: bool,
    pub control_bar_resizer_method_names_wave147_ok: bool,
    pub control_bar_resizer_nav_commands_wave147_ok: bool,
    pub under_construction_method_names_wave148_ok: bool,
    pub under_construction_nav_commands_wave148_ok: bool,
    pub structure_inventory_command_names_wave149_ok: bool,
    pub structure_inventory_nav_commands_wave149_ok: bool,
    pub multi_select_method_names_wave150_ok: bool,
    pub multi_select_nav_commands_wave150_ok: bool,
    pub credits_style_method_names_wave151_ok: bool,
    pub credits_nav_commands_wave151_ok: bool,
    pub challenge_generals_method_names_wave152_ok: bool,
    pub challenge_generals_nav_commands_wave152_ok: bool,
    pub gameworld_authority_env_names_wave153_ok: bool,
    pub gameworld_authority_method_names_wave153_ok: bool,
    pub gameworld_authority_nav_commands_wave153_ok: bool,
    pub window_video_type_state_names_wave154_ok: bool,
    pub window_video_method_names_wave154_ok: bool,
    pub window_video_nav_commands_wave154_ok: bool,
    pub main_menu_layout_names_wave155_ok: bool,
    pub main_menu_layout_nav_commands_wave155_ok: bool,
    pub control_bar_scheme_names_wave156_ok: bool,
    pub control_bar_scheme_method_names_wave156_ok: bool,
    pub control_bar_scheme_nav_commands_wave156_ok: bool,
    pub presentation_boundary_method_names_wave157_ok: bool,
    pub presentation_boundary_source_markers_wave157_ok: bool,
    pub presentation_boundary_nav_commands_wave157_ok: bool,
    pub presentation_boundary_live_wave157_ok: bool,
    pub control_bar_print_names_wave158_ok: bool,
    pub control_bar_print_nav_commands_wave158_ok: bool,
    pub terrain_env_boundary_method_names_wave159_ok: bool,
    pub terrain_env_boundary_source_markers_wave159_ok: bool,
    pub terrain_env_boundary_nav_commands_wave159_ok: bool,
    pub terrain_env_boundary_live_wave159_ok: bool,
    pub main_menu_wnd_names_wave160_ok: bool,
    pub main_menu_wnd_nav_commands_wave160_ok: bool,
    pub main_menu_wnd_live_wave160_ok: bool,
}

pub(super) fn evaluate(
    pres: &crate::presentation_frame::PresentationFrame,
    presentation_ok: bool,
) -> Waves121160 {
    let _ = (pres, presentation_ok);
    Waves121160 {
        save_load_layout_wave121_ok: honesty_save_load_layout_residual_wave121(),
        save_load_control_stems_wave121_ok: honesty_save_load_control_stems_residual_wave121(),
        save_load_nav_commands_wave121_ok: honesty_save_load_nav_commands_residual_wave121(),
        replay_control_names_wave122_ok: honesty_replay_menu_control_names_residual_wave122(),
        replay_nav_commands_wave122_ok: honesty_replay_menu_nav_commands_residual_wave122(),
        quit_control_names_wave123_ok: honesty_quit_menu_control_names_residual_wave123(),
        quit_nav_commands_wave123_ok: honesty_quit_menu_nav_commands_residual_wave123(),
        keyboard_control_names_wave124_ok: honesty_keyboard_options_control_names_residual_wave124(
        ),
        keyboard_nav_commands_wave124_ok: honesty_keyboard_options_nav_commands_residual_wave124(),
        score_control_names_wave125_ok: honesty_score_screen_control_names_residual_wave125(),
        score_nav_commands_wave125_ok: honesty_score_screen_nav_commands_residual_wave125(),
        options_control_names_wave126_ok: honesty_options_menu_control_names_residual_wave126(),
        options_nav_commands_wave126_ok: honesty_options_menu_nav_commands_residual_wave126(),
        credits_control_names_wave127_ok: honesty_credits_menu_control_names_residual_wave127(),
        credits_nav_commands_wave127_ok: honesty_credits_menu_nav_commands_residual_wave127(),
        message_box_control_names_wave128_ok: honesty_message_box_control_names_residual_wave128(),
        message_box_nav_commands_wave128_ok: honesty_message_box_nav_commands_residual_wave128(),
        diplomacy_control_names_wave129_ok: honesty_diplomacy_control_names_residual_wave129(),
        diplomacy_nav_commands_wave129_ok: honesty_diplomacy_nav_commands_residual_wave129(),
        popup_replay_control_names_wave130_ok: honesty_popup_replay_control_names_residual_wave130(
        ),
        popup_replay_nav_commands_wave130_ok: honesty_popup_replay_nav_commands_residual_wave130(),
        single_player_control_names_wave131_ok:
            honesty_single_player_menu_control_names_residual_wave131(),
        single_player_nav_commands_wave131_ok:
            honesty_single_player_menu_nav_commands_residual_wave131(),
        map_select_control_names_wave132_ok: honesty_map_select_menu_control_names_residual_wave132(
        ),
        map_select_nav_commands_wave132_ok: honesty_map_select_menu_nav_commands_residual_wave132(),
        control_bar_control_names_wave133_ok: honesty_control_bar_control_names_residual_wave133(),
        control_bar_nav_commands_wave133_ok: honesty_control_bar_nav_commands_residual_wave133(),
        difficulty_select_control_names_wave134_ok:
            honesty_difficulty_select_control_names_residual_wave134(),
        difficulty_select_nav_commands_wave134_ok:
            honesty_difficulty_select_nav_commands_residual_wave134(),
        loading_screen_stages_wave135_ok: honesty_loading_screen_stages_residual_wave135(),
        loading_screen_nav_commands_wave135_ok:
            honesty_loading_screen_nav_commands_residual_wave135(),
        in_game_chat_control_names_wave136_ok: honesty_in_game_chat_control_names_residual_wave136(
        ),
        in_game_chat_nav_commands_wave136_ok: honesty_in_game_chat_nav_commands_residual_wave136(),
        idle_worker_control_names_wave137_ok: honesty_idle_worker_control_names_residual_wave137(),
        idle_worker_nav_commands_wave137_ok: honesty_idle_worker_nav_commands_residual_wave137(),
        generals_exp_control_names_wave138_ok: honesty_generals_exp_control_names_residual_wave138(
        ),
        generals_exp_nav_commands_wave138_ok: honesty_generals_exp_nav_commands_residual_wave138(),
        popup_communicator_control_names_wave139_ok:
            honesty_popup_communicator_control_names_residual_wave139(),
        popup_communicator_nav_commands_wave139_ok:
            honesty_popup_communicator_nav_commands_residual_wave139(),
        replay_control_control_names_wave140_ok:
            honesty_replay_control_control_names_residual_wave140(),
        replay_control_nav_commands_wave140_ok:
            honesty_replay_control_nav_commands_residual_wave140(),
        shell_map_names_wave141_ok: honesty_shell_map_names_residual_wave141(),
        shell_map_nav_commands_wave141_ok: honesty_shell_map_nav_commands_residual_wave141(),
        beacon_control_names_wave142_ok: honesty_beacon_control_names_residual_wave142(),
        beacon_nav_commands_wave142_ok: honesty_beacon_nav_commands_residual_wave142(),
        eva_message_names_wave143_ok: honesty_eva_message_names_residual_wave143(),
        eva_nav_commands_wave143_ok: honesty_eva_nav_commands_residual_wave143(),
        ime_message_names_wave144_ok: honesty_ime_message_names_residual_wave144(),
        ime_nav_commands_wave144_ok: honesty_ime_nav_commands_residual_wave144(),
        smudge_method_names_wave145_ok: honesty_smudge_method_names_residual_wave145(),
        smudge_nav_commands_wave145_ok: honesty_smudge_nav_commands_residual_wave145(),
        ocl_timer_method_names_wave146_ok: honesty_ocl_timer_method_names_residual_wave146(),
        ocl_timer_nav_commands_wave146_ok: honesty_ocl_timer_nav_commands_residual_wave146(),
        control_bar_resizer_method_names_wave147_ok:
            honesty_control_bar_resizer_method_names_residual_wave147(),
        control_bar_resizer_nav_commands_wave147_ok:
            honesty_control_bar_resizer_nav_commands_residual_wave147(),
        under_construction_method_names_wave148_ok:
            honesty_under_construction_method_names_residual_wave148(),
        under_construction_nav_commands_wave148_ok:
            honesty_under_construction_nav_commands_residual_wave148(),
        structure_inventory_command_names_wave149_ok:
            honesty_structure_inventory_command_names_residual_wave149(),
        structure_inventory_nav_commands_wave149_ok:
            honesty_structure_inventory_nav_commands_residual_wave149(),
        multi_select_method_names_wave150_ok: honesty_multi_select_method_names_residual_wave150(),
        multi_select_nav_commands_wave150_ok: honesty_multi_select_nav_commands_residual_wave150(),
        credits_style_method_names_wave151_ok: honesty_credits_style_method_names_residual_wave151(
        ),
        credits_nav_commands_wave151_ok: honesty_credits_nav_commands_residual_wave151(),
        challenge_generals_method_names_wave152_ok:
            honesty_challenge_generals_method_names_residual_wave152(),
        challenge_generals_nav_commands_wave152_ok:
            honesty_challenge_generals_nav_commands_residual_wave152(),
        gameworld_authority_env_names_wave153_ok:
            honesty_gameworld_authority_env_names_residual_wave153(),
        gameworld_authority_method_names_wave153_ok:
            honesty_gameworld_authority_method_names_residual_wave153(),
        gameworld_authority_nav_commands_wave153_ok:
            honesty_gameworld_authority_nav_commands_residual_wave153(),
        window_video_type_state_names_wave154_ok:
            honesty_window_video_type_state_names_residual_wave154(),
        window_video_method_names_wave154_ok: honesty_window_video_method_names_residual_wave154(),
        window_video_nav_commands_wave154_ok: honesty_window_video_nav_commands_residual_wave154(),
        main_menu_layout_names_wave155_ok: honesty_main_menu_layout_names_residual_wave155(),
        main_menu_layout_nav_commands_wave155_ok:
            honesty_main_menu_layout_nav_commands_residual_wave155(),
        control_bar_scheme_names_wave156_ok: honesty_control_bar_scheme_names_residual_wave156(),
        control_bar_scheme_method_names_wave156_ok:
            honesty_control_bar_scheme_method_names_residual_wave156(),
        control_bar_scheme_nav_commands_wave156_ok:
            honesty_control_bar_scheme_nav_commands_residual_wave156(),
        presentation_boundary_method_names_wave157_ok:
            honesty_presentation_boundary_method_names_residual_wave157(),
        presentation_boundary_source_markers_wave157_ok:
            honesty_presentation_boundary_source_markers_residual_wave157(),
        presentation_boundary_nav_commands_wave157_ok:
            honesty_presentation_boundary_nav_commands_residual_wave157(),
        presentation_boundary_live_wave157_ok: simulate_presentation_boundary_prepare_honesty(),
        control_bar_print_names_wave158_ok: honesty_control_bar_print_names_residual_wave158(),
        control_bar_print_nav_commands_wave158_ok:
            honesty_control_bar_print_nav_commands_residual_wave158(),
        terrain_env_boundary_method_names_wave159_ok:
            honesty_terrain_env_boundary_method_names_residual_wave159(),
        terrain_env_boundary_source_markers_wave159_ok:
            honesty_terrain_env_boundary_source_markers_residual_wave159(),
        terrain_env_boundary_nav_commands_wave159_ok:
            honesty_terrain_env_boundary_nav_commands_residual_wave159(),
        terrain_env_boundary_live_wave159_ok: simulate_terrain_env_boundary_prepare_honesty(),
        main_menu_wnd_names_wave160_ok: honesty_main_menu_wnd_names_residual_wave160(),
        main_menu_wnd_nav_commands_wave160_ok: honesty_main_menu_wnd_nav_commands_residual_wave160(
        ),
        main_menu_wnd_live_wave160_ok: simulate_main_menu_wnd_prepare_honesty(),
    }
}
