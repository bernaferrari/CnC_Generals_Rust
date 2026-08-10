//! Host smoke residual assertions: waves 548–608.

use super::ShellSmokeResult;

pub(super) fn assert_waves_548_608(r: &ShellSmokeResult) {
    assert!(
        r.camera_follow_presentation_fail_closed_method_names_wave548_ok,
        "camera follow presentation fail-closed method names residual pack wave548: {}",
        r.detail
    );
    assert!(
        r.camera_follow_presentation_fail_closed_nav_commands_wave548_ok,
        "camera follow presentation fail-closed nav commands residual pack wave548: {}",
        r.detail
    );
    assert!(
        r.camera_follow_presentation_fail_closed_live_wave548_ok,
        "camera follow presentation fail-closed live residual wave548: {}",
        r.detail
    );
    assert!(
        r.ui_player_info_presentation_fail_closed_method_names_wave549_ok,
        "ui_player_info presentation fail-closed method names residual pack wave549: {}",
        r.detail
    );
    assert!(
        r.ui_player_info_presentation_fail_closed_nav_commands_wave549_ok,
        "ui_player_info presentation fail-closed nav commands residual pack wave549: {}",
        r.detail
    );
    assert!(
        r.ui_player_info_presentation_fail_closed_live_wave549_ok,
        "ui_player_info presentation fail-closed live residual wave549: {}",
        r.detail
    );
    assert!(
        r.visual_speed_presentation_helper_method_names_wave550_ok,
        "visual speed presentation helper method names residual pack wave550: {}",
        r.detail
    );
    assert!(
        r.visual_speed_presentation_helper_nav_commands_wave550_ok,
        "visual speed presentation helper nav commands residual pack wave550: {}",
        r.detail
    );
    assert!(
        r.visual_speed_presentation_helper_live_wave550_ok,
        "visual speed presentation helper live residual wave550: {}",
        r.detail
    );
    assert!(
        r.time_frozen_presentation_helper_method_names_wave551_ok,
        "time frozen presentation helper method names residual pack wave551: {}",
        r.detail
    );
    assert!(
        r.time_frozen_presentation_helper_nav_commands_wave551_ok,
        "time frozen presentation helper nav commands residual pack wave551: {}",
        r.detail
    );
    assert!(
        r.time_frozen_presentation_helper_live_wave551_ok,
        "time frozen presentation helper live residual wave551: {}",
        r.detail
    );
    assert!(
        r.shell_bypass_presentation_helper_method_names_wave552_ok,
        "shell bypass presentation helper method names residual pack wave552: {}",
        r.detail
    );
    assert!(
        r.shell_bypass_presentation_helper_nav_commands_wave552_ok,
        "shell bypass presentation helper nav commands residual pack wave552: {}",
        r.detail
    );
    assert!(
        r.shell_bypass_presentation_helper_live_wave552_ok,
        "shell bypass presentation helper live residual wave552: {}",
        r.detail
    );
    assert!(
        r.play_time_local_player_presentation_helper_method_names_wave553_ok,
        "play-time/local-player presentation helper method names residual pack wave553: {}",
        r.detail
    );
    assert!(
        r.play_time_local_player_presentation_helper_nav_commands_wave553_ok,
        "play-time/local-player presentation helper nav commands residual pack wave553: {}",
        r.detail
    );
    assert!(
        r.play_time_local_player_presentation_helper_live_wave553_ok,
        "play-time/local-player presentation helper live residual wave553: {}",
        r.detail
    );
    assert!(
        r.map_difficulty_presentation_helper_method_names_wave554_ok,
        "map/difficulty presentation helper method names residual pack wave554: {}",
        r.detail
    );
    assert!(
        r.map_difficulty_presentation_helper_nav_commands_wave554_ok,
        "map/difficulty presentation helper nav commands residual pack wave554: {}",
        r.detail
    );
    assert!(
        r.map_difficulty_presentation_helper_live_wave554_ok,
        "map/difficulty presentation helper live residual wave554: {}",
        r.detail
    );
    assert!(
        r.science_team_presentation_helper_method_names_wave555_ok,
        "science/team presentation helper method names residual pack wave555: {}",
        r.detail
    );
    assert!(
        r.science_team_presentation_helper_nav_commands_wave555_ok,
        "science/team presentation helper nav commands residual pack wave555: {}",
        r.detail
    );
    assert!(
        r.science_team_presentation_helper_live_wave555_ok,
        "science/team presentation helper live residual wave555: {}",
        r.detail
    );
    assert!(
        r.victory_presentation_helper_method_names_wave556_ok,
        "victory presentation helper method names residual pack wave556: {}",
        r.detail
    );
    assert!(
        r.victory_presentation_helper_nav_commands_wave556_ok,
        "victory presentation helper nav commands residual pack wave556: {}",
        r.detail
    );
    assert!(
        r.victory_presentation_helper_live_wave556_ok,
        "victory presentation helper live residual wave556: {}",
        r.detail
    );
    assert!(
        r.replay_presentation_helper_method_names_wave557_ok,
        "replay presentation helper method names residual pack wave557: {}",
        r.detail
    );
    assert!(
        r.replay_presentation_helper_nav_commands_wave557_ok,
        "replay presentation helper nav commands residual pack wave557: {}",
        r.detail
    );
    assert!(
        r.replay_presentation_helper_live_wave557_ok,
        "replay presentation helper live residual wave557: {}",
        r.detail
    );
    assert!(
        r.diplomacy_presentation_helper_method_names_wave558_ok,
        "diplomacy presentation helper method names residual pack wave558: {}",
        r.detail
    );
    assert!(
        r.diplomacy_presentation_helper_nav_commands_wave558_ok,
        "diplomacy presentation helper nav commands residual pack wave558: {}",
        r.detail
    );
    assert!(
        r.diplomacy_presentation_helper_live_wave558_ok,
        "diplomacy presentation helper live residual wave558: {}",
        r.detail
    );
    assert!(
        r.presentation_honesty_align_method_names_wave559_ok,
        "presentation honesty align method names residual pack wave559: {}",
        r.detail
    );
    assert!(
        r.presentation_honesty_align_nav_commands_wave559_ok,
        "presentation honesty align nav commands residual pack wave559: {}",
        r.detail
    );
    assert!(
        r.presentation_honesty_align_live_wave559_ok,
        "presentation honesty align live residual wave559: {}",
        r.detail
    );
    assert!(
        r.logic_frame_presentation_helper_method_names_wave560_ok,
        "logic frame presentation helper method names residual pack wave560: {}",
        r.detail
    );
    assert!(
        r.logic_frame_presentation_helper_nav_commands_wave560_ok,
        "logic frame presentation helper nav commands residual pack wave560: {}",
        r.detail
    );
    assert!(
        r.logic_frame_presentation_helper_live_wave560_ok,
        "logic frame presentation helper live residual wave560: {}",
        r.detail
    );
    assert!(
        r.logic_steps_presentation_helper_method_names_wave561_ok,
        "logic steps presentation helper method names residual pack wave561: {}",
        r.detail
    );
    assert!(
        r.logic_steps_presentation_helper_nav_commands_wave561_ok,
        "logic steps presentation helper nav commands residual pack wave561: {}",
        r.detail
    );
    assert!(
        r.logic_steps_presentation_helper_live_wave561_ok,
        "logic steps presentation helper live residual wave561: {}",
        r.detail
    );
    assert!(
        r.combat_kill_particle_observe_method_names_wave562_ok,
        "combat kill particle observe method names residual pack wave562: {}",
        r.detail
    );
    assert!(
        r.combat_kill_particle_observe_nav_commands_wave562_ok,
        "combat kill particle observe nav commands residual pack wave562: {}",
        r.detail
    );
    assert!(
        r.combat_kill_particle_observe_live_wave562_ok,
        "combat kill particle observe live residual wave562: {}",
        r.detail
    );
    assert!(
        r.template_name_presentation_helper_method_names_wave563_ok,
        "template name presentation helper method names residual pack wave563: {}",
        r.detail
    );
    assert!(
        r.template_name_presentation_helper_nav_commands_wave563_ok,
        "template name presentation helper nav commands residual pack wave563: {}",
        r.detail
    );
    assert!(
        r.template_name_presentation_helper_live_wave563_ok,
        "template name presentation helper live residual wave563: {}",
        r.detail
    );
    assert!(
        r.fixed_step_diag_presentation_helper_method_names_wave564_ok,
        "fixed-step diag presentation helper method names residual pack wave564: {}",
        r.detail
    );
    assert!(
        r.fixed_step_diag_presentation_helper_nav_commands_wave564_ok,
        "fixed-step diag presentation helper nav commands residual pack wave564: {}",
        r.detail
    );
    assert!(
        r.fixed_step_diag_presentation_helper_live_wave564_ok,
        "fixed-step diag presentation helper live residual wave564: {}",
        r.detail
    );
    assert!(
        r.construct_template_presentation_helper_method_names_wave565_ok,
        "construct template presentation helper method names residual pack wave565: {}",
        r.detail
    );
    assert!(
        r.construct_template_presentation_helper_nav_commands_wave565_ok,
        "construct template presentation helper nav commands residual pack wave565: {}",
        r.detail
    );
    assert!(
        r.construct_template_presentation_helper_live_wave565_ok,
        "construct template presentation helper live residual wave565: {}",
        r.detail
    );
    assert!(
        r.boot_ui_message_helper_method_names_wave566_ok,
        "boot UI message helper method names residual pack wave566: {}",
        r.detail
    );
    assert!(
        r.boot_ui_message_helper_nav_commands_wave566_ok,
        "boot UI message helper nav commands residual pack wave566: {}",
        r.detail
    );
    assert!(
        r.boot_ui_message_helper_live_wave566_ok,
        "boot UI message helper live residual wave566: {}",
        r.detail
    );
    assert!(
        r.boot_movie_helper_method_names_wave567_ok,
        "boot movie helper method names residual pack wave567: {}",
        r.detail
    );
    assert!(
        r.boot_movie_helper_nav_commands_wave567_ok,
        "boot movie helper nav commands residual pack wave567: {}",
        r.detail
    );
    assert!(
        r.boot_movie_helper_live_wave567_ok,
        "boot movie helper live residual wave567: {}",
        r.detail
    );
    assert!(
        r.script_fps_helper_method_names_wave568_ok,
        "script FPS helper method names residual pack wave568: {}",
        r.detail
    );
    assert!(
        r.script_fps_helper_nav_commands_wave568_ok,
        "script FPS helper nav commands residual pack wave568: {}",
        r.detail
    );
    assert!(
        r.script_fps_helper_live_wave568_ok,
        "script FPS helper live residual wave568: {}",
        r.detail
    );
    assert!(
        r.defeat_alliance_helper_method_names_wave569_ok,
        "defeat/alliance helper method names residual pack wave569: {}",
        r.detail
    );
    assert!(
        r.defeat_alliance_helper_nav_commands_wave569_ok,
        "defeat/alliance helper nav commands residual pack wave569: {}",
        r.detail
    );
    assert!(
        r.defeat_alliance_helper_live_wave569_ok,
        "defeat/alliance helper live residual wave569: {}",
        r.detail
    );
    assert!(
        r.script_msg_helper_method_names_wave570_ok,
        "script msg helper method names residual pack wave570: {}",
        r.detail
    );
    assert!(
        r.script_msg_helper_nav_commands_wave570_ok,
        "script msg helper nav commands residual pack wave570: {}",
        r.detail
    );
    assert!(
        r.script_msg_helper_live_wave570_ok,
        "script msg helper live residual wave570: {}",
        r.detail
    );
    assert!(
        r.popup_music_helper_method_names_wave571_ok,
        "popup/music helper method names residual pack wave571: {}",
        r.detail
    );
    assert!(
        r.popup_music_helper_nav_commands_wave571_ok,
        "popup/music helper nav commands residual pack wave571: {}",
        r.detail
    );
    assert!(
        r.popup_music_helper_live_wave571_ok,
        "popup/music helper live residual wave571: {}",
        r.detail
    );
    assert!(
        r.boot_camera_helper_method_names_wave572_ok,
        "boot camera helper method names residual pack wave572: {}",
        r.detail
    );
    assert!(
        r.boot_camera_helper_nav_commands_wave572_ok,
        "boot camera helper nav commands residual pack wave572: {}",
        r.detail
    );
    assert!(
        r.boot_camera_helper_live_wave572_ok,
        "boot camera helper live residual wave572: {}",
        r.detail
    );
    assert!(
        r.boot_player_info_helper_method_names_wave573_ok,
        "boot player info helper method names residual pack wave573: {}",
        r.detail
    );
    assert!(
        r.boot_player_info_helper_nav_commands_wave573_ok,
        "boot player info helper nav commands residual pack wave573: {}",
        r.detail
    );
    assert!(
        r.boot_player_info_helper_live_wave573_ok,
        "boot player info helper live residual wave573: {}",
        r.detail
    );
    assert!(
        r.boot_local_player_helper_method_names_wave574_ok,
        "boot local player helper method names residual pack wave574: {}",
        r.detail
    );
    assert!(
        r.boot_local_player_helper_nav_commands_wave574_ok,
        "boot local player helper nav commands residual pack wave574: {}",
        r.detail
    );
    assert!(
        r.boot_local_player_helper_live_wave574_ok,
        "boot local player helper live residual wave574: {}",
        r.detail
    );
    assert!(
        r.host_pause_team_helper_method_names_wave575_ok,
        "host pause/team helper method names residual pack wave575: {}",
        r.detail
    );
    assert!(
        r.host_pause_team_helper_nav_commands_wave575_ok,
        "host pause/team helper nav commands residual pack wave575: {}",
        r.detail
    );
    assert!(
        r.host_pause_team_helper_live_wave575_ok,
        "host pause/team helper live residual wave575: {}",
        r.detail
    );
    assert!(
        r.host_command_flush_helper_method_names_wave576_ok,
        "host command flush helper method names residual pack wave576: {}",
        r.detail
    );
    assert!(
        r.host_command_flush_helper_nav_commands_wave576_ok,
        "host command flush helper nav commands residual pack wave576: {}",
        r.detail
    );
    assert!(
        r.host_command_flush_helper_live_wave576_ok,
        "host command flush helper live residual wave576: {}",
        r.detail
    );
    assert!(
        r.host_camera_start_helper_method_names_wave577_ok,
        "host camera/start helper method names residual pack wave577: {}",
        r.detail
    );
    assert!(
        r.host_camera_start_helper_nav_commands_wave577_ok,
        "host camera/start helper nav commands residual pack wave577: {}",
        r.detail
    );
    assert!(
        r.host_camera_start_helper_live_wave577_ok,
        "host camera/start helper live residual wave577: {}",
        r.detail
    );
    assert!(
        r.host_silent_command_peel_method_names_wave578_ok,
        "host silent command peel method names residual pack wave578: {}",
        r.detail
    );
    assert!(
        r.host_silent_command_peel_nav_commands_wave578_ok,
        "host silent command peel nav commands residual pack wave578: {}",
        r.detail
    );
    assert!(
        r.host_silent_command_peel_live_wave578_ok,
        "host silent command peel live residual wave578: {}",
        r.detail
    );
    assert!(
        r.host_selection_map_helper_method_names_wave579_ok,
        "host selection/map helper method names residual pack wave579: {}",
        r.detail
    );
    assert!(
        r.host_selection_map_helper_nav_commands_wave579_ok,
        "host selection/map helper nav commands residual pack wave579: {}",
        r.detail
    );
    assert!(
        r.host_selection_map_helper_live_wave579_ok,
        "host selection/map helper live residual wave579: {}",
        r.detail
    );
    assert!(
        r.host_cancel_selection_helper_method_names_wave580_ok,
        "host cancel/selection helper method names residual pack wave580: {}",
        r.detail
    );
    assert!(
        r.host_cancel_selection_helper_nav_commands_wave580_ok,
        "host cancel/selection helper nav commands residual pack wave580: {}",
        r.detail
    );
    assert!(
        r.host_cancel_selection_helper_live_wave580_ok,
        "host cancel/selection helper live residual wave580: {}",
        r.detail
    );
    assert!(
        r.host_template_spawn_helper_method_names_wave581_ok,
        "host template/spawn helper method names residual pack wave581: {}",
        r.detail
    );
    assert!(
        r.host_template_spawn_helper_nav_commands_wave581_ok,
        "host template/spawn helper nav commands residual pack wave581: {}",
        r.detail
    );
    assert!(
        r.host_template_spawn_helper_live_wave581_ok,
        "host template/spawn helper live residual wave581: {}",
        r.detail
    );
    assert!(
        r.host_enqueue_shell_cmd_helper_method_names_wave582_ok,
        "host enqueue/shell cmd helper method names residual pack wave582: {}",
        r.detail
    );
    assert!(
        r.host_enqueue_shell_cmd_helper_nav_commands_wave582_ok,
        "host enqueue/shell cmd helper nav commands residual pack wave582: {}",
        r.detail
    );
    assert!(
        r.host_enqueue_shell_cmd_helper_live_wave582_ok,
        "host enqueue/shell cmd helper live residual wave582: {}",
        r.detail
    );
    assert!(
        r.host_runtime_cmd_helper_method_names_wave583_ok,
        "host runtime cmd helper method names residual pack wave583: {}",
        r.detail
    );
    assert!(
        r.host_runtime_cmd_helper_nav_commands_wave583_ok,
        "host runtime cmd helper nav commands residual pack wave583: {}",
        r.detail
    );
    assert!(
        r.host_runtime_cmd_helper_live_wave583_ok,
        "host runtime cmd helper live residual wave583: {}",
        r.detail
    );
    assert!(
        r.host_tick_mutation_helper_method_names_wave584_ok,
        "host tick/mutation helper method names residual pack wave584: {}",
        r.detail
    );
    assert!(
        r.host_tick_mutation_helper_nav_commands_wave584_ok,
        "host tick/mutation helper nav commands residual pack wave584: {}",
        r.detail
    );
    assert!(
        r.host_tick_mutation_helper_live_wave584_ok,
        "host tick/mutation helper live residual wave584: {}",
        r.detail
    );
    assert!(
        r.host_ui_shell_world_helper_method_names_wave585_ok,
        "host UI/shell/world helper method names residual pack wave585: {}",
        r.detail
    );
    assert!(
        r.host_ui_shell_world_helper_nav_commands_wave585_ok,
        "host UI/shell/world helper nav commands residual pack wave585: {}",
        r.detail
    );
    assert!(
        r.host_ui_shell_world_helper_live_wave585_ok,
        "host UI/shell/world helper live residual wave585: {}",
        r.detail
    );
    assert!(
        r.host_game_client_shell_tick_helper_method_names_wave586_ok,
        "host GameClient shell tick helper method names residual pack wave586: {}",
        r.detail
    );
    assert!(
        r.host_game_client_shell_tick_helper_nav_commands_wave586_ok,
        "host GameClient shell tick helper nav commands residual pack wave586: {}",
        r.detail
    );
    assert!(
        r.host_game_client_shell_tick_helper_live_wave586_ok,
        "host GameClient shell tick helper live residual wave586: {}",
        r.detail
    );
    assert!(
        r.host_game_client_device_tick_helper_method_names_wave587_ok,
        "host GameClient device tick helper method names residual pack wave587: {}",
        r.detail
    );
    assert!(
        r.host_game_client_device_tick_helper_nav_commands_wave587_ok,
        "host GameClient device tick helper nav commands residual pack wave587: {}",
        r.detail
    );
    assert!(
        r.host_game_client_device_tick_helper_live_wave587_ok,
        "host GameClient device tick helper live residual wave587: {}",
        r.detail
    );
    assert!(
        r.host_game_client_menu_shell_helper_method_names_wave588_ok,
        "host GameClient menu shell helper method names residual pack wave588: {}",
        r.detail
    );
    assert!(
        r.host_game_client_menu_shell_helper_nav_commands_wave588_ok,
        "host GameClient menu shell helper nav commands residual pack wave588: {}",
        r.detail
    );
    assert!(
        r.host_game_client_menu_shell_helper_live_wave588_ok,
        "host GameClient menu shell helper live residual wave588: {}",
        r.detail
    );
    assert!(
        r.host_presentation_finalize_helper_method_names_wave589_ok,
        "host presentation finalize helper method names residual pack wave589: {}",
        r.detail
    );
    assert!(
        r.host_presentation_finalize_helper_nav_commands_wave589_ok,
        "host presentation finalize helper nav commands residual pack wave589: {}",
        r.detail
    );
    assert!(
        r.host_presentation_finalize_helper_live_wave589_ok,
        "host presentation finalize helper live residual wave589: {}",
        r.detail
    );
    assert!(
        r.host_presentation_seed_helper_method_names_wave590_ok,
        "host presentation seed helper method names residual pack wave590: {}",
        r.detail
    );
    assert!(
        r.host_presentation_seed_helper_nav_commands_wave590_ok,
        "host presentation seed helper nav commands residual pack wave590: {}",
        r.detail
    );
    assert!(
        r.host_presentation_seed_helper_live_wave590_ok,
        "host presentation seed helper live residual wave590: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_presentation_helper_method_names_wave591_ok,
        "host render UI presentation helper method names residual pack wave591: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_presentation_helper_nav_commands_wave591_ok,
        "host render UI presentation helper nav commands residual pack wave591: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_presentation_helper_live_wave591_ok,
        "host render UI presentation helper live residual wave591: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_overlays_helper_method_names_wave592_ok,
        "host render UI overlays helper method names residual pack wave592: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_overlays_helper_nav_commands_wave592_ok,
        "host render UI overlays helper nav commands residual pack wave592: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_overlays_helper_live_wave592_ok,
        "host render UI overlays helper live residual wave592: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_finalize_helper_method_names_wave593_ok,
        "host render UI finalize helper method names residual pack wave593: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_finalize_helper_nav_commands_wave593_ok,
        "host render UI finalize helper nav commands residual pack wave593: {}",
        r.detail
    );
    assert!(
        r.host_render_ui_finalize_helper_live_wave593_ok,
        "host render UI finalize helper live residual wave593: {}",
        r.detail
    );
    assert!(
        r.host_minimap_bounds_repair_helper_method_names_wave594_ok,
        "host minimap bounds repair helper method names residual pack wave594: {}",
        r.detail
    );
    assert!(
        r.host_minimap_bounds_repair_helper_nav_commands_wave594_ok,
        "host minimap bounds repair helper nav commands residual pack wave594: {}",
        r.detail
    );
    assert!(
        r.host_minimap_bounds_repair_helper_live_wave594_ok,
        "host minimap bounds repair helper live residual wave594: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_apply_helper_method_names_wave595_ok,
        "host production complete apply helper method names residual pack wave595: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_apply_helper_nav_commands_wave595_ok,
        "host production complete apply helper nav commands residual pack wave595: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_apply_helper_live_wave595_ok,
        "host production complete apply helper live residual wave595: {}",
        r.detail
    );
    assert!(
        r.host_camera_queue_drain_helper_method_names_wave596_ok,
        "host camera queue drain helper method names residual pack wave596: {}",
        r.detail
    );
    assert!(
        r.host_camera_queue_drain_helper_nav_commands_wave596_ok,
        "host camera queue drain helper nav commands residual pack wave596: {}",
        r.detail
    );
    assert!(
        r.host_camera_queue_drain_helper_live_wave596_ok,
        "host camera queue drain helper live residual wave596: {}",
        r.detail
    );
    assert!(
        r.host_gameworld_shadow_session_helper_method_names_wave597_ok,
        "host gameworld shadow session helper method names residual pack wave597: {}",
        r.detail
    );
    assert!(
        r.host_gameworld_shadow_session_helper_nav_commands_wave597_ok,
        "host gameworld shadow session helper nav commands residual pack wave597: {}",
        r.detail
    );
    assert!(
        r.host_gameworld_shadow_session_helper_live_wave597_ok,
        "host gameworld shadow session helper live residual wave597: {}",
        r.detail
    );
    assert!(
        r.host_ingame_hud_helper_method_names_wave598_ok,
        "host ingame hud helper method names residual pack wave598: {}",
        r.detail
    );
    assert!(
        r.host_ingame_hud_helper_nav_commands_wave598_ok,
        "host ingame hud helper nav commands residual pack wave598: {}",
        r.detail
    );
    assert!(
        r.host_ingame_hud_helper_live_wave598_ok,
        "host ingame hud helper live residual wave598: {}",
        r.detail
    );
    assert!(
        r.host_match_outcome_helper_method_names_wave599_ok,
        "host match outcome helper method names residual pack wave599: {}",
        r.detail
    );
    assert!(
        r.host_match_outcome_helper_nav_commands_wave599_ok,
        "host match outcome helper nav commands residual pack wave599: {}",
        r.detail
    );
    assert!(
        r.host_match_outcome_helper_live_wave599_ok,
        "host match outcome helper live residual wave599: {}",
        r.detail
    );
    assert!(
        r.host_post_presentation_client_helper_method_names_wave600_ok,
        "host post presentation client helper method names residual pack wave600: {}",
        r.detail
    );
    assert!(
        r.host_post_presentation_client_helper_nav_commands_wave600_ok,
        "host post presentation client helper nav commands residual pack wave600: {}",
        r.detail
    );
    assert!(
        r.host_post_presentation_client_helper_live_wave600_ok,
        "host post presentation client helper live residual wave600: {}",
        r.detail
    );
    assert!(
        r.host_restart_pause_helper_method_names_wave601_ok,
        "host restart pause helper method names residual pack wave601: {}",
        r.detail
    );
    assert!(
        r.host_restart_pause_helper_nav_commands_wave601_ok,
        "host restart pause helper nav commands residual pack wave601: {}",
        r.detail
    );
    assert!(
        r.host_restart_pause_helper_live_wave601_ok,
        "host restart pause helper live residual wave601: {}",
        r.detail
    );
    assert!(
        r.host_ingame_logic_shell_helper_method_names_wave602_ok,
        "host ingame logic shell helper method names residual pack wave602: {}",
        r.detail
    );
    assert!(
        r.host_ingame_logic_shell_helper_nav_commands_wave602_ok,
        "host ingame logic shell helper nav commands residual pack wave602: {}",
        r.detail
    );
    assert!(
        r.host_ingame_logic_shell_helper_live_wave602_ok,
        "host ingame logic shell helper live residual wave602: {}",
        r.detail
    );
    assert!(
        r.host_paused_endgame_boot_ui_helper_method_names_wave603_ok,
        "host paused endgame boot ui helper method names residual pack wave603: {}",
        r.detail
    );
    assert!(
        r.host_paused_endgame_boot_ui_helper_nav_commands_wave603_ok,
        "host paused endgame boot ui helper nav commands residual pack wave603: {}",
        r.detail
    );
    assert!(
        r.host_paused_endgame_boot_ui_helper_live_wave603_ok,
        "host paused endgame boot ui helper live residual wave603: {}",
        r.detail
    );
    assert!(
        r.host_loading_sfx_helper_method_names_wave604_ok,
        "host loading sfx helper method names residual pack wave604: {}",
        r.detail
    );
    assert!(
        r.host_loading_sfx_helper_nav_commands_wave604_ok,
        "host loading sfx helper nav commands residual pack wave604: {}",
        r.detail
    );
    assert!(
        r.host_loading_sfx_helper_live_wave604_ok,
        "host loading sfx helper live residual wave604: {}",
        r.detail
    );
    assert!(
        r.host_menu_client_helper_method_names_wave605_ok,
        "host menu client helper method names residual pack wave605: {}",
        r.detail
    );
    assert!(
        r.host_menu_client_helper_nav_commands_wave605_ok,
        "host menu client helper nav commands residual pack wave605: {}",
        r.detail
    );
    assert!(
        r.host_menu_client_helper_live_wave605_ok,
        "host menu client helper live residual wave605: {}",
        r.detail
    );
    assert!(
        r.host_os_inject_presentation_notify_helper_method_names_wave606_ok,
        "host os inject presentation notify helper method names residual pack wave606: {}",
        r.detail
    );
    assert!(
        r.host_os_inject_presentation_notify_helper_nav_commands_wave606_ok,
        "host os inject presentation notify helper nav commands residual pack wave606: {}",
        r.detail
    );
    assert!(
        r.host_os_inject_presentation_notify_helper_live_wave606_ok,
        "host os inject presentation notify helper live residual wave606: {}",
        r.detail
    );
    assert!(
        r.host_ui_presentation_drain_helper_method_names_wave607_ok,
        "host ui presentation drain helper method names residual pack wave607: {}",
        r.detail
    );
    assert!(
        r.host_ui_presentation_drain_helper_nav_commands_wave607_ok,
        "host ui presentation drain helper nav commands residual pack wave607: {}",
        r.detail
    );
    assert!(
        r.host_ui_presentation_drain_helper_live_wave607_ok,
        "host ui presentation drain helper live residual wave607: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_host_apply_helper_method_names_wave608_ok,
        "host production complete host apply helper method names residual pack wave608: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_host_apply_helper_nav_commands_wave608_ok,
        "host production complete host apply helper nav commands residual pack wave608: {}",
        r.detail
    );
    assert!(
        r.host_production_complete_host_apply_helper_live_wave608_ok,
        "host production complete host apply helper live residual wave608: {}",
        r.detail
    );
}
