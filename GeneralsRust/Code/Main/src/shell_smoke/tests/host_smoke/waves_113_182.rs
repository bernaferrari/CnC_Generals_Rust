//! Host smoke residual assertions: waves 113–182.

use super::ShellSmokeResult;

pub(super) fn assert_waves_113_182(r: &ShellSmokeResult) {
    assert!(
        r.game_window_manager_wave113_ok,
        "game window manager residual pack wave113: {}",
        r.detail
    );
    assert!(
        r.window_style_wave113_ok,
        "window style residual pack wave113: {}",
        r.detail
    );
    assert!(
        r.gadget_wave113_ok,
        "gadget residual pack wave113: {}",
        r.detail
    );
    assert!(
        r.video_buffer_wave113_ok,
        "video buffer residual pack wave113: {}",
        r.detail
    );
    assert!(
        r.audio_event_wave113_ok,
        "audio event residual pack wave113: {}",
        r.detail
    );
    assert!(
        r.main_menu_skirmish_names_wave114_ok,
        "main menu skirmish names residual pack wave114: {}",
        r.detail
    );
    assert!(
        r.main_menu_skirmish_nav_steps_wave114_ok,
        "main menu skirmish nav steps residual pack wave114: {}",
        r.detail
    );
    assert!(
        r.main_menu_skirmish_message_wave114_ok,
        "main menu skirmish message residual pack wave114: {}",
        r.detail
    );
    assert!(
        r.map_select_names_wave115_ok,
        "map select names residual pack wave115: {}",
        r.detail
    );
    assert!(
        r.map_select_nav_steps_wave115_ok,
        "map select nav steps residual pack wave115: {}",
        r.detail
    );
    assert!(
        r.map_select_commands_wave115_ok,
        "map select commands residual pack wave115: {}",
        r.detail
    );
    assert!(
        r.slot_state_wave116_ok,
        "slot state residual pack wave116: {}",
        r.detail
    );
    assert!(
        r.slot_combo_names_wave116_ok,
        "slot combo names residual pack wave116: {}",
        r.detail
    );
    assert!(
        r.slot_nav_commands_wave116_ok,
        "slot nav commands residual pack wave116: {}",
        r.detail
    );
    assert!(
        r.starting_cash_wave117_ok,
        "starting cash residual pack wave117: {}",
        r.detail
    );
    assert!(
        r.game_speed_controls_wave117_ok,
        "game speed controls residual pack wave117: {}",
        r.detail
    );
    assert!(
        r.rules_nav_commands_wave117_ok,
        "rules nav commands residual pack wave117: {}",
        r.detail
    );
    assert!(
        r.main_menu_button_names_wave118_ok,
        "main menu button names residual pack wave118: {}",
        r.detail
    );
    assert!(
        r.main_menu_push_targets_wave118_ok,
        "main menu push targets residual pack wave118: {}",
        r.detail
    );
    assert!(
        r.main_menu_button_nav_commands_wave118_ok,
        "main menu button nav commands residual pack wave118: {}",
        r.detail
    );
    assert!(
        r.campaign_button_names_wave119_ok,
        "campaign button names residual pack wave119: {}",
        r.detail
    );
    assert!(
        r.campaign_enums_wave119_ok,
        "campaign enums residual pack wave119: {}",
        r.detail
    );
    assert!(
        r.campaign_nav_commands_wave119_ok,
        "campaign nav commands residual pack wave119: {}",
        r.detail
    );
    assert!(
        r.challenge_control_names_wave120_ok,
        "challenge control names residual pack wave120: {}",
        r.detail
    );
    assert!(
        r.challenge_nav_commands_wave120_ok,
        "challenge nav commands residual pack wave120: {}",
        r.detail
    );
    assert!(
        r.save_load_layout_wave121_ok,
        "save load layout residual pack wave121: {}",
        r.detail
    );
    assert!(
        r.save_load_control_stems_wave121_ok,
        "save load control stems residual pack wave121: {}",
        r.detail
    );
    assert!(
        r.save_load_nav_commands_wave121_ok,
        "save load nav commands residual pack wave121: {}",
        r.detail
    );
    assert!(
        r.replay_control_names_wave122_ok,
        "replay control names residual pack wave122: {}",
        r.detail
    );
    assert!(
        r.replay_nav_commands_wave122_ok,
        "replay nav commands residual pack wave122: {}",
        r.detail
    );
    assert!(
        r.quit_control_names_wave123_ok,
        "quit control names residual pack wave123: {}",
        r.detail
    );
    assert!(
        r.quit_nav_commands_wave123_ok,
        "quit nav commands residual pack wave123: {}",
        r.detail
    );
    assert!(
        r.keyboard_control_names_wave124_ok,
        "keyboard control names residual pack wave124: {}",
        r.detail
    );
    assert!(
        r.keyboard_nav_commands_wave124_ok,
        "keyboard nav commands residual pack wave124: {}",
        r.detail
    );
    assert!(
        r.score_control_names_wave125_ok,
        "score control names residual pack wave125: {}",
        r.detail
    );
    assert!(
        r.score_nav_commands_wave125_ok,
        "score nav commands residual pack wave125: {}",
        r.detail
    );
    assert!(
        r.options_control_names_wave126_ok,
        "options control names residual pack wave126: {}",
        r.detail
    );
    assert!(
        r.options_nav_commands_wave126_ok,
        "options nav commands residual pack wave126: {}",
        r.detail
    );
    assert!(
        r.credits_control_names_wave127_ok,
        "credits control names residual pack wave127: {}",
        r.detail
    );
    assert!(
        r.credits_nav_commands_wave127_ok,
        "credits nav commands residual pack wave127: {}",
        r.detail
    );
    assert!(
        r.message_box_control_names_wave128_ok,
        "message box control names residual pack wave128: {}",
        r.detail
    );
    assert!(
        r.message_box_nav_commands_wave128_ok,
        "message box nav commands residual pack wave128: {}",
        r.detail
    );
    assert!(
        r.diplomacy_control_names_wave129_ok,
        "diplomacy control names residual pack wave129: {}",
        r.detail
    );
    assert!(
        r.diplomacy_nav_commands_wave129_ok,
        "diplomacy nav commands residual pack wave129: {}",
        r.detail
    );
    assert!(
        r.popup_replay_control_names_wave130_ok,
        "popup replay control names residual pack wave130: {}",
        r.detail
    );
    assert!(
        r.popup_replay_nav_commands_wave130_ok,
        "popup replay nav commands residual pack wave130: {}",
        r.detail
    );
    assert!(
        r.single_player_control_names_wave131_ok,
        "single player control names residual pack wave131: {}",
        r.detail
    );
    assert!(
        r.single_player_nav_commands_wave131_ok,
        "single player nav commands residual pack wave131: {}",
        r.detail
    );
    assert!(
        r.map_select_control_names_wave132_ok,
        "map select control names residual pack wave132: {}",
        r.detail
    );
    assert!(
        r.map_select_nav_commands_wave132_ok,
        "map select nav commands residual pack wave132: {}",
        r.detail
    );
    assert!(
        r.control_bar_control_names_wave133_ok,
        "control bar control names residual pack wave133: {}",
        r.detail
    );
    assert!(
        r.control_bar_nav_commands_wave133_ok,
        "control bar nav commands residual pack wave133: {}",
        r.detail
    );
    assert!(
        r.difficulty_select_control_names_wave134_ok,
        "difficulty select control names residual pack wave134: {}",
        r.detail
    );
    assert!(
        r.difficulty_select_nav_commands_wave134_ok,
        "difficulty select nav commands residual pack wave134: {}",
        r.detail
    );
    assert!(
        r.loading_screen_stages_wave135_ok,
        "loading screen stages residual pack wave135: {}",
        r.detail
    );
    assert!(
        r.loading_screen_nav_commands_wave135_ok,
        "loading screen nav commands residual pack wave135: {}",
        r.detail
    );
    assert!(
        r.in_game_chat_control_names_wave136_ok,
        "in game chat control names residual pack wave136: {}",
        r.detail
    );
    assert!(
        r.in_game_chat_nav_commands_wave136_ok,
        "in game chat nav commands residual pack wave136: {}",
        r.detail
    );
    assert!(
        r.idle_worker_control_names_wave137_ok,
        "idle worker control names residual pack wave137: {}",
        r.detail
    );
    assert!(
        r.idle_worker_nav_commands_wave137_ok,
        "idle worker nav commands residual pack wave137: {}",
        r.detail
    );
    assert!(
        r.generals_exp_control_names_wave138_ok,
        "generals exp control names residual pack wave138: {}",
        r.detail
    );
    assert!(
        r.generals_exp_nav_commands_wave138_ok,
        "generals exp nav commands residual pack wave138: {}",
        r.detail
    );
    assert!(
        r.popup_communicator_control_names_wave139_ok,
        "popup communicator control names residual pack wave139: {}",
        r.detail
    );
    assert!(
        r.popup_communicator_nav_commands_wave139_ok,
        "popup communicator nav commands residual pack wave139: {}",
        r.detail
    );
    assert!(
        r.replay_control_control_names_wave140_ok,
        "replay control control names residual pack wave140: {}",
        r.detail
    );
    assert!(
        r.replay_control_nav_commands_wave140_ok,
        "replay control nav commands residual pack wave140: {}",
        r.detail
    );
    assert!(
        r.shell_map_names_wave141_ok,
        "shell map names residual pack wave141: {}",
        r.detail
    );
    assert!(
        r.shell_map_nav_commands_wave141_ok,
        "shell map nav commands residual pack wave141: {}",
        r.detail
    );
    assert!(
        r.beacon_control_names_wave142_ok,
        "beacon control names residual pack wave142: {}",
        r.detail
    );
    assert!(
        r.beacon_nav_commands_wave142_ok,
        "beacon nav commands residual pack wave142: {}",
        r.detail
    );
    assert!(
        r.eva_message_names_wave143_ok,
        "eva message names residual pack wave143: {}",
        r.detail
    );
    assert!(
        r.eva_nav_commands_wave143_ok,
        "eva nav commands residual pack wave143: {}",
        r.detail
    );
    assert!(
        r.ime_message_names_wave144_ok,
        "ime message names residual pack wave144: {}",
        r.detail
    );
    assert!(
        r.ime_nav_commands_wave144_ok,
        "ime nav commands residual pack wave144: {}",
        r.detail
    );
    assert!(
        r.smudge_method_names_wave145_ok,
        "smudge method names residual pack wave145: {}",
        r.detail
    );
    assert!(
        r.smudge_nav_commands_wave145_ok,
        "smudge nav commands residual pack wave145: {}",
        r.detail
    );
    assert!(
        r.ocl_timer_method_names_wave146_ok,
        "ocl timer method names residual pack wave146: {}",
        r.detail
    );
    assert!(
        r.ocl_timer_nav_commands_wave146_ok,
        "ocl timer nav commands residual pack wave146: {}",
        r.detail
    );
    assert!(
        r.control_bar_resizer_method_names_wave147_ok,
        "control bar resizer method names residual pack wave147: {}",
        r.detail
    );
    assert!(
        r.control_bar_resizer_nav_commands_wave147_ok,
        "control bar resizer nav commands residual pack wave147: {}",
        r.detail
    );
    assert!(
        r.under_construction_method_names_wave148_ok,
        "under construction method names residual pack wave148: {}",
        r.detail
    );
    assert!(
        r.under_construction_nav_commands_wave148_ok,
        "under construction nav commands residual pack wave148: {}",
        r.detail
    );
    assert!(
        r.structure_inventory_command_names_wave149_ok,
        "structure inventory command names residual pack wave149: {}",
        r.detail
    );
    assert!(
        r.structure_inventory_nav_commands_wave149_ok,
        "structure inventory nav commands residual pack wave149: {}",
        r.detail
    );
    assert!(
        r.multi_select_method_names_wave150_ok,
        "multi select method names residual pack wave150: {}",
        r.detail
    );
    assert!(
        r.multi_select_nav_commands_wave150_ok,
        "multi select nav commands residual pack wave150: {}",
        r.detail
    );
    assert!(
        r.credits_style_method_names_wave151_ok,
        "credits style method names residual pack wave151: {}",
        r.detail
    );
    assert!(
        r.credits_nav_commands_wave151_ok,
        "credits nav commands residual pack wave151: {}",
        r.detail
    );
    assert!(
        r.challenge_generals_method_names_wave152_ok,
        "challenge generals method names residual pack wave152: {}",
        r.detail
    );
    assert!(
        r.challenge_generals_nav_commands_wave152_ok,
        "challenge generals nav commands residual pack wave152: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_env_names_wave153_ok,
        "gameworld authority env names residual pack wave153: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_method_names_wave153_ok,
        "gameworld authority method names residual pack wave153: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_nav_commands_wave153_ok,
        "gameworld authority nav commands residual pack wave153: {}",
        r.detail
    );
    assert!(
        r.window_video_type_state_names_wave154_ok,
        "window video type state names residual pack wave154: {}",
        r.detail
    );
    assert!(
        r.window_video_method_names_wave154_ok,
        "window video method names residual pack wave154: {}",
        r.detail
    );
    assert!(
        r.window_video_nav_commands_wave154_ok,
        "window video nav commands residual pack wave154: {}",
        r.detail
    );
    assert!(
        r.main_menu_layout_names_wave155_ok,
        "main menu layout names residual pack wave155: {}",
        r.detail
    );
    assert!(
        r.main_menu_layout_nav_commands_wave155_ok,
        "main menu layout nav commands residual pack wave155: {}",
        r.detail
    );
    assert!(
        r.control_bar_scheme_names_wave156_ok,
        "control bar scheme names residual pack wave156: {}",
        r.detail
    );
    assert!(
        r.control_bar_scheme_method_names_wave156_ok,
        "control bar scheme method names residual pack wave156: {}",
        r.detail
    );
    assert!(
        r.control_bar_scheme_nav_commands_wave156_ok,
        "control bar scheme nav commands residual pack wave156: {}",
        r.detail
    );
    assert!(
        r.presentation_boundary_method_names_wave157_ok,
        "presentation boundary method names residual pack wave157: {}",
        r.detail
    );
    assert!(
        r.presentation_boundary_source_markers_wave157_ok,
        "presentation boundary source markers residual pack wave157: {}",
        r.detail
    );
    assert!(
        r.presentation_boundary_nav_commands_wave157_ok,
        "presentation boundary nav commands residual pack wave157: {}",
        r.detail
    );
    assert!(
        r.presentation_boundary_live_wave157_ok,
        "presentation boundary live residual wave157: {}",
        r.detail
    );
    assert!(
        r.control_bar_print_names_wave158_ok,
        "control bar print names residual pack wave158: {}",
        r.detail
    );
    assert!(
        r.control_bar_print_nav_commands_wave158_ok,
        "control bar print nav commands residual pack wave158: {}",
        r.detail
    );
    assert!(
        r.terrain_env_boundary_method_names_wave159_ok,
        "terrain env boundary method names residual pack wave159: {}",
        r.detail
    );
    assert!(
        r.terrain_env_boundary_source_markers_wave159_ok,
        "terrain env boundary source markers residual pack wave159: {}",
        r.detail
    );
    assert!(
        r.terrain_env_boundary_nav_commands_wave159_ok,
        "terrain env boundary nav commands residual pack wave159: {}",
        r.detail
    );
    assert!(
        r.terrain_env_boundary_live_wave159_ok,
        "terrain env boundary live residual wave159: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_names_wave160_ok,
        "main menu wnd names residual pack wave160: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_nav_commands_wave160_ok,
        "main menu wnd nav commands residual pack wave160: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_live_wave160_ok,
        "main menu wnd live residual wave160: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_load_method_names_wave161_ok,
        "main menu wnd load method names residual pack wave161: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_load_nav_commands_wave161_ok,
        "main menu wnd load nav commands residual pack wave161: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_load_live_wave161_ok,
        "main menu wnd load live residual wave161: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_materialise_method_names_wave162_ok,
        "main menu wnd materialise method names residual pack wave162: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_materialise_nav_commands_wave162_ok,
        "main menu wnd materialise nav commands residual pack wave162: {}",
        r.detail
    );
    assert!(
        r.main_menu_wnd_materialise_live_wave162_ok,
        "main menu wnd materialise live residual wave162: {}",
        r.detail
    );
    assert!(
        r.shell_stack_push_method_names_wave163_ok,
        "shell stack push method names residual pack wave163: {}",
        r.detail
    );
    assert!(
        r.shell_stack_push_nav_commands_wave163_ok,
        "shell stack push nav commands residual pack wave163: {}",
        r.detail
    );
    assert!(
        r.shell_stack_push_live_wave163_ok,
        "shell stack push live residual wave163: {}",
        r.detail
    );
    assert!(
        r.shell_skirmish_nav_method_names_wave164_ok,
        "shell skirmish nav method names residual pack wave164: {}",
        r.detail
    );
    assert!(
        r.shell_skirmish_nav_commands_wave164_ok,
        "shell skirmish nav commands residual pack wave164: {}",
        r.detail
    );
    assert!(
        r.shell_skirmish_nav_live_wave164_ok,
        "shell skirmish nav live residual wave164: {}",
        r.detail
    );
    assert!(
        r.control_bar_materialise_method_names_wave165_ok,
        "control bar materialise method names residual pack wave165: {}",
        r.detail
    );
    assert!(
        r.control_bar_materialise_nav_commands_wave165_ok,
        "control bar materialise nav commands residual pack wave165: {}",
        r.detail
    );
    assert!(
        r.control_bar_materialise_live_wave165_ok,
        "control bar materialise live residual wave165: {}",
        r.detail
    );
    assert!(
        r.skirmish_options_wnd_method_names_wave166_ok,
        "skirmish options wnd method names residual pack wave166: {}",
        r.detail
    );
    assert!(
        r.skirmish_options_wnd_nav_commands_wave166_ok,
        "skirmish options wnd nav commands residual pack wave166: {}",
        r.detail
    );
    assert!(
        r.skirmish_options_wnd_live_wave166_ok,
        "skirmish options wnd live residual wave166: {}",
        r.detail
    );
    assert!(
        r.new_game_stream_method_names_wave167_ok,
        "new game stream method names residual pack wave167: {}",
        r.detail
    );
    assert!(
        r.new_game_stream_nav_commands_wave167_ok,
        "new game stream nav commands residual pack wave167: {}",
        r.detail
    );
    assert!(
        r.new_game_stream_live_wave167_ok,
        "new game stream live residual wave167: {}",
        r.detail
    );
    assert!(
        r.w3d_main_menu_init_method_names_wave168_ok,
        "w3d main menu init method names residual pack wave168: {}",
        r.detail
    );
    assert!(
        r.w3d_main_menu_init_nav_commands_wave168_ok,
        "w3d main menu init nav commands residual pack wave168: {}",
        r.detail
    );
    assert!(
        r.w3d_main_menu_init_live_wave168_ok,
        "w3d main menu init live residual wave168: {}",
        r.detail
    );
    assert!(
        r.start_game_loading_method_names_wave169_ok,
        "start game loading method names residual pack wave169: {}",
        r.detail
    );
    assert!(
        r.start_game_loading_nav_commands_wave169_ok,
        "start game loading nav commands residual pack wave169: {}",
        r.detail
    );
    assert!(
        r.start_game_loading_live_wave169_ok,
        "start game loading live residual wave169: {}",
        r.detail
    );
    assert!(
        r.live_map_load_method_names_wave170_ok,
        "live map load method names residual pack wave170: {}",
        r.detail
    );
    assert!(
        r.live_map_load_nav_commands_wave170_ok,
        "live map load nav commands residual pack wave170: {}",
        r.detail
    );
    assert!(
        r.live_map_load_live_wave170_ok,
        "live map load live residual wave170: {}",
        r.detail
    );
    assert!(
        r.live_presentation_seed_method_names_wave171_ok,
        "live presentation seed method names residual pack wave171: {}",
        r.detail
    );
    assert!(
        r.live_presentation_seed_nav_commands_wave171_ok,
        "live presentation seed nav commands residual pack wave171: {}",
        r.detail
    );
    assert!(
        r.live_presentation_seed_live_wave171_ok,
        "live presentation seed live residual wave171: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_shadow_overlay_method_names_wave172_ok,
        "live gameworld shadow overlay method names residual pack wave172: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_shadow_overlay_nav_commands_wave172_ok,
        "live gameworld shadow overlay nav commands residual pack wave172: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_shadow_overlay_live_wave172_ok,
        "live gameworld shadow overlay live residual wave172: {}",
        r.detail
    );
    assert!(
        r.single_authority_combat_method_names_wave173_ok,
        "single authority combat method names residual pack wave173: {}",
        r.detail
    );
    assert!(
        r.single_authority_combat_nav_commands_wave173_ok,
        "single authority combat nav commands residual pack wave173: {}",
        r.detail
    );
    assert!(
        r.single_authority_combat_live_wave173_ok,
        "single authority combat live residual wave173: {}",
        r.detail
    );
    assert!(
        r.presentation_client_boundary_method_names_wave174_ok,
        "presentation client boundary method names residual pack wave174: {}",
        r.detail
    );
    assert!(
        r.presentation_client_boundary_nav_commands_wave174_ok,
        "presentation client boundary nav commands residual pack wave174: {}",
        r.detail
    );
    assert!(
        r.presentation_client_boundary_live_wave174_ok,
        "presentation client boundary live residual wave174: {}",
        r.detail
    );
    assert!(
        r.golden_map_host_victory_method_names_wave175_ok,
        "golden map host victory method names residual pack wave175: {}",
        r.detail
    );
    assert!(
        r.golden_map_host_victory_nav_commands_wave175_ok,
        "golden map host victory nav commands residual pack wave175: {}",
        r.detail
    );
    assert!(
        r.golden_map_host_victory_live_wave175_ok,
        "golden map host victory live residual wave175: {}",
        r.detail
    );
    assert!(
        r.executable_presentation_boundary_method_names_wave176_ok,
        "executable presentation boundary method names residual pack wave176: {}",
        r.detail
    );
    assert!(
        r.executable_presentation_boundary_nav_commands_wave176_ok,
        "executable presentation boundary nav commands residual pack wave176: {}",
        r.detail
    );
    assert!(
        r.executable_presentation_boundary_live_wave176_ok,
        "executable presentation boundary live residual wave176: {}",
        r.detail
    );
    assert!(
        r.production_authority_env_ok,
        "production authority env residual wave177: {}",
        r.detail
    );
    assert!(
        r.gameworld_production_authority_method_names_wave177_ok,
        "gameworld production authority method names residual pack wave177: {}",
        r.detail
    );
    assert!(
        r.gameworld_production_authority_nav_commands_wave177_ok,
        "gameworld production authority nav commands residual pack wave177: {}",
        r.detail
    );
    assert!(
        r.gameworld_production_authority_live_wave177_ok,
        "gameworld production authority live residual wave177: {}",
        r.detail
    );
    assert!(
        r.movement_authority_env_ok,
        "movement authority env residual wave178: {}",
        r.detail
    );
    assert!(
        r.gameworld_sole_tick_coupling_method_names_wave178_ok,
        "gameworld sole tick coupling method names residual pack wave178: {}",
        r.detail
    );
    assert!(
        r.gameworld_sole_tick_coupling_nav_commands_wave178_ok,
        "gameworld sole tick coupling nav commands residual pack wave178: {}",
        r.detail
    );
    assert!(
        r.gameworld_sole_tick_coupling_live_wave178_ok,
        "gameworld sole tick coupling live residual wave178: {}",
        r.detail
    );
    assert!(
        r.ai_fire_construction_authority_env_ok,
        "ai/fire/construction authority env residual wave179: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_matrix_method_names_wave179_ok,
        "gameworld authority matrix method names residual pack wave179: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_matrix_nav_commands_wave179_ok,
        "gameworld authority matrix nav commands residual pack wave179: {}",
        r.detail
    );
    assert!(
        r.gameworld_authority_matrix_live_wave179_ok,
        "gameworld authority matrix live residual wave179: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_production_writeback_method_names_wave180_ok,
        "live gameworld production writeback method names residual pack wave180: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_production_writeback_nav_commands_wave180_ok,
        "live gameworld production writeback nav commands residual pack wave180: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_production_writeback_live_wave180_ok,
        "live gameworld production writeback live residual wave180: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_construction_writeback_method_names_wave181_ok,
        "live gameworld construction writeback method names residual pack wave181: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_construction_writeback_nav_commands_wave181_ok,
        "live gameworld construction writeback nav commands residual pack wave181: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_construction_writeback_live_wave181_ok,
        "live gameworld construction writeback live residual wave181: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_damage_channel_method_names_wave182_ok,
        "live gameworld damage channel method names residual pack wave182: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_damage_channel_nav_commands_wave182_ok,
        "live gameworld damage channel nav commands residual pack wave182: {}",
        r.detail
    );
    assert!(
        r.live_gameworld_damage_channel_live_wave182_ok,
        "live gameworld damage channel live residual wave182: {}",
        r.detail
    );
}
