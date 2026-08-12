#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_replay_control(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "play".to_string());
        let position = args
            .get("position")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::callbacks::{
                drive_os_wnd_replay_control_fast_forward_like_cpp,
                drive_os_wnd_replay_control_pause_like_cpp,
                drive_os_wnd_replay_control_play_like_cpp,
                drive_os_wnd_replay_control_prepare_play_at_like_cpp,
                drive_os_wnd_replay_control_stop_like_cpp, simulate_replay_control_pause,
                simulate_replay_control_play, simulate_replay_control_prepare_play_at,
                simulate_replay_control_seek, simulate_replay_control_stop,
                simulate_replay_control_toggle_fast_forward,
            };
            wnd_ok = match action.as_str() {
                "pause" => {
                    drive_os_wnd_replay_control_pause_like_cpp() || simulate_replay_control_pause()
                }
                "stop" => {
                    drive_os_wnd_replay_control_stop_like_cpp() || simulate_replay_control_stop()
                }
                "ff" | "fast_forward" => {
                    drive_os_wnd_replay_control_fast_forward_like_cpp()
                        || simulate_replay_control_toggle_fast_forward()
                }
                "seek" => simulate_replay_control_seek(position),
                "prepare" | "prepare_play" => {
                    drive_os_wnd_replay_control_prepare_play_at_like_cpp(position)
                        || simulate_replay_control_prepare_play_at(position)
                }
                _ => drive_os_wnd_replay_control_play_like_cpp() || simulate_replay_control_play(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_replay_control_ok_wnd_{action}")
        } else {
            "click_replay_control_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_toggle_shell_map(&mut self, _args: &HashMap<String, String>) {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = game_client::gui::simulate_shell_map_toggle();
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "toggle_shell_map_ok_wnd".into()
        } else {
            "toggle_shell_map_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_shell_map(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "show".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::{
                simulate_shell_map_hide, simulate_shell_map_prepare_cycle, simulate_shell_map_show,
                simulate_shell_map_toggle,
            };
            wnd_ok = match action.as_str() {
                "hide" => simulate_shell_map_hide(),
                "toggle" => simulate_shell_map_toggle(),
                "prepare_cycle" | "cycle" => simulate_shell_map_prepare_cycle(),
                _ => simulate_shell_map_show(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_shell_map_ok_wnd_{action}")
        } else {
            "click_shell_map_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_beacon(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "place".to_string());
        let player = args
            .get("player")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let x = args
            .get("x")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        let y = args
            .get("y")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        let z = args
            .get("z")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.0);
        let text = args
            .get("text")
            .cloned()
            .unwrap_or_else(|| "beacon".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::system::{
                simulate_beacon_drain_notifications, simulate_beacon_place,
                simulate_beacon_prepare_place_with_text, simulate_beacon_remove,
                simulate_beacon_set_text,
            };
            wnd_ok = match action.as_str() {
                "remove" => simulate_beacon_remove(player, x, y, z),
                "text" => simulate_beacon_set_text(player, x, y, z, &text),
                "drain" => {
                    let _ = simulate_beacon_drain_notifications();
                    true
                }
                "prepare" | "prepare_text" => {
                    simulate_beacon_prepare_place_with_text(player, x, y, z, &text)
                }
                _ => simulate_beacon_place(player, x, y, z, Some(text.as_str())),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_beacon_ok_wnd_{action}")
        } else {
            "click_beacon_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_eva(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let message = args
            .get("message")
            .cloned()
            .unwrap_or_else(|| "LOWPOWER".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::eva::{
                simulate_eva_disable, simulate_eva_enable, simulate_eva_prepare_low_power_alert,
                simulate_eva_reset, simulate_eva_set_should_play_by_name, simulate_eva_update,
            };
            wnd_ok = match action.as_str() {
                "enable" => simulate_eva_enable(),
                "disable" => simulate_eva_disable(),
                "reset" => simulate_eva_reset(),
                "update" => simulate_eva_update(),
                "should_play" | "play" => simulate_eva_set_should_play_by_name(&message),
                _ => simulate_eva_prepare_low_power_alert(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_eva_ok_wnd_{action}")
        } else {
            "click_eva_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_ime(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let text = args
            .get("text")
            .cloned()
            .unwrap_or_else(|| "nihao".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::{
                drive_os_wnd_ime_clear_candidates_like_cpp,
                drive_os_wnd_ime_prepare_composition_cycle_like_cpp,
                drive_os_wnd_ime_result_like_cpp, simulate_ime_clear_candidates,
                simulate_ime_disable, simulate_ime_enable, simulate_ime_end_composition,
                simulate_ime_prepare_composition_cycle, simulate_ime_reset,
                simulate_ime_result_string, simulate_ime_start_composition,
                simulate_ime_update_composition,
            };
            wnd_ok = match action.as_str() {
                "enable" => simulate_ime_enable(),
                "disable" => simulate_ime_disable(),
                "start" => simulate_ime_start_composition(),
                "update" => simulate_ime_update_composition(&text, text.chars().count()),
                "result" => {
                    drive_os_wnd_ime_result_like_cpp(&text) || simulate_ime_result_string(&text)
                }
                "clear" => {
                    drive_os_wnd_ime_clear_candidates_like_cpp() || simulate_ime_clear_candidates()
                }
                "end" => simulate_ime_end_composition(),
                "reset" => simulate_ime_reset(),
                _ => {
                    drive_os_wnd_ime_prepare_composition_cycle_like_cpp(&text)
                        || simulate_ime_prepare_composition_cycle(&text)
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_ime_ok_wnd_{action}")
        } else {
            "click_ime_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_smudge(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let size = args
            .get("size")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(12.0);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::system::{
                simulate_smudge_add, simulate_smudge_add_set,
                simulate_smudge_prepare_set_with_smudge, simulate_smudge_remove_first,
                simulate_smudge_remove_set, simulate_smudge_reset,
                simulate_smudge_set_count_last_frame,
            };
            wnd_ok = match action.as_str() {
                "add_set" => simulate_smudge_add_set(),
                "add" => simulate_smudge_add(size, 1.0),
                "remove" => simulate_smudge_remove_first(),
                "remove_set" => simulate_smudge_remove_set(),
                "reset" => simulate_smudge_reset(),
                "count" => simulate_smudge_set_count_last_frame(size as i32),
                _ => simulate_smudge_prepare_set_with_smudge(size),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_smudge_ok_wnd_{action}")
        } else {
            "click_smudge_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_ocl_timer(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let remaining = args
            .get("remaining")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(870);
        let total = args
            .get("total")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(900);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_ocl_timer_format, simulate_ocl_timer_frames_to_display,
                simulate_ocl_timer_prepare_display, simulate_ocl_timer_should_update,
            };
            wnd_ok = match action.as_str() {
                "format" => {
                    let _ = simulate_ocl_timer_format(remaining, 0.5);
                    true
                }
                "frames" => {
                    let _ = simulate_ocl_timer_frames_to_display(remaining, total);
                    true
                }
                "should_update" => simulate_ocl_timer_should_update(0, remaining),
                _ => simulate_ocl_timer_prepare_display(remaining, total).is_some(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_ocl_timer_ok_wnd_{action}")
        } else {
            "click_ocl_timer_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_control_bar_resizer(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let name = args
            .get("name")
            .cloned()
            .unwrap_or_else(|| "ControlBar.wnd:ControlBarParent".to_string());
        let width = args
            .get("width")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1024);
        let height = args
            .get("height")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(768);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_control_bar_resizer_add_window, simulate_control_bar_resizer_clear,
                simulate_control_bar_resizer_get_optimal_size,
                simulate_control_bar_resizer_prepare_default, simulate_control_bar_resizer_resize,
                simulate_control_bar_resizer_set_base_resolution,
            };
            wnd_ok = match action.as_str() {
                "add" => simulate_control_bar_resizer_add_window(&name),
                "clear" => simulate_control_bar_resizer_clear(),
                "base" => simulate_control_bar_resizer_set_base_resolution(width, height),
                "resize" => simulate_control_bar_resizer_resize(width, height),
                "optimal" => {
                    let _ = simulate_control_bar_resizer_get_optimal_size();
                    true
                }
                _ => simulate_control_bar_resizer_prepare_default(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_control_bar_resizer_ok_wnd_{action}")
        } else {
            "click_control_bar_resizer_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_under_construction(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let name = args
            .get("name")
            .cloned()
            .unwrap_or_else(|| "Strategy Center".to_string());
        let percent = args
            .get("percent")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(66);
        let next = args
            .get("next")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(75);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_under_construction_cancel_command_name,
                simulate_under_construction_complete, simulate_under_construction_populate,
                simulate_under_construction_prepare_cycle,
                simulate_under_construction_update_percent,
            };
            wnd_ok = match action.as_str() {
                "populate" => simulate_under_construction_populate(&name, percent),
                "update" => {
                    let _ = simulate_under_construction_update_percent(percent);
                    true
                }
                "complete" => simulate_under_construction_complete(),
                "cancel" => {
                    simulate_under_construction_cancel_command_name()
                        == "Command_CancelConstruction"
                }
                _ => simulate_under_construction_prepare_cycle(&name, percent, next),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_under_construction_ok_wnd_{action}")
        } else {
            "click_under_construction_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_structure_inventory(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let max_g = args
            .get("max")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);
        let count = args
            .get("count")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(3);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_structure_inventory_clear,
                simulate_structure_inventory_evacuate_command_name,
                simulate_structure_inventory_exit_command_name,
                simulate_structure_inventory_populate,
                simulate_structure_inventory_prepare_occupied,
                simulate_structure_inventory_stop_command_name,
            };
            wnd_ok = match action.as_str() {
                "populate" => simulate_structure_inventory_populate(max_g, count),
                "clear" => simulate_structure_inventory_clear(),
                "exit" => {
                    simulate_structure_inventory_exit_command_name() == "Command_StructureExit"
                }
                "evacuate" => {
                    simulate_structure_inventory_evacuate_command_name() == "Command_Evacuate"
                }
                "stop" => simulate_structure_inventory_stop_command_name() == "Command_Stop",
                _ => simulate_structure_inventory_prepare_occupied(max_g, count),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_structure_inventory_ok_wnd_{action}")
        } else {
            "click_structure_inventory_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_multi_select(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_multi_select_clear, simulate_multi_select_prepare_divergent,
                simulate_multi_select_prepare_same_commands,
            };
            wnd_ok = match action.as_str() {
                "clear" => simulate_multi_select_clear(),
                "divergent" => simulate_multi_select_prepare_divergent(),
                _ => simulate_multi_select_prepare_same_commands(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_multi_select_ok_wnd_{action}")
        } else {
            "click_multi_select_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_credits_roll(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let text = args
            .get("text")
            .cloned()
            .unwrap_or_else(|| "COMMAND & CONQUER".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::credits::{
                drive_os_wnd_credits_roll_finished_like_cpp,
                drive_os_wnd_credits_roll_prepare_like_cpp,
                drive_os_wnd_credits_roll_update_like_cpp, simulate_credits_add_blank,
                simulate_credits_add_text, simulate_credits_init,
                simulate_credits_is_finished_probe, simulate_credits_prepare_short_roll,
                simulate_credits_reset, simulate_credits_update,
            };
            wnd_ok = match action.as_str() {
                "init" => simulate_credits_init(),
                "reset" => simulate_credits_reset(),
                "add" => simulate_credits_add_text(&text),
                "blank" => simulate_credits_add_blank(),
                "update" => {
                    drive_os_wnd_credits_roll_update_like_cpp() || simulate_credits_update()
                }
                "finished" => {
                    drive_os_wnd_credits_roll_finished_like_cpp()
                        || simulate_credits_is_finished_probe()
                }
                _ => {
                    drive_os_wnd_credits_roll_prepare_like_cpp()
                        || simulate_credits_prepare_short_roll()
                }
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_credits_roll_ok_wnd_{action}")
        } else {
            "click_credits_roll_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_challenge_generals(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let name = args
            .get("name")
            .cloned()
            .unwrap_or_else(|| "General Alexander".to_string());
        let difficulty = args
            .get("difficulty")
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(2);
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::{
                simulate_challenge_generals_init, simulate_challenge_generals_prepare_default,
                simulate_challenge_generals_set_bio_name,
                simulate_challenge_generals_set_difficulty,
                simulate_challenge_generals_set_starts_enabled,
                simulate_challenge_generals_set_template_num,
            };
            wnd_ok = match action.as_str() {
                "init" => simulate_challenge_generals_init(),
                "starts" => simulate_challenge_generals_set_starts_enabled(0, true),
                "bio" => simulate_challenge_generals_set_bio_name(0, &name),
                "difficulty" => simulate_challenge_generals_set_difficulty(difficulty),
                "template" => simulate_challenge_generals_set_template_num(0),
                _ => simulate_challenge_generals_prepare_default(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_challenge_generals_ok_wnd_{action}")
        } else {
            "click_challenge_generals_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_window_video(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::{
                simulate_window_video_init, simulate_window_video_pause_all,
                simulate_window_video_prepare_control_cycle, simulate_window_video_reset,
                simulate_window_video_resume_all, simulate_window_video_stop_all,
                simulate_window_video_update,
            };
            wnd_ok = match action.as_str() {
                "init" => simulate_window_video_init(),
                "reset" => simulate_window_video_reset(),
                "pause" => simulate_window_video_pause_all(),
                "resume" => simulate_window_video_resume_all(),
                "stop" => simulate_window_video_stop_all(),
                "update" => simulate_window_video_update(),
                _ => simulate_window_video_prepare_control_cycle(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_window_video_ok_wnd_{action}")
        } else {
            "click_window_video_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_main_menu_layout(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let wnd_ok = match action.as_str() {
            "create" => crate::ui::simulate_main_menu_layout_create(),
            "clear" => crate::ui::simulate_main_menu_layout_clear(),
            "hit" => crate::ui::simulate_main_menu_layout_hit_test_single_player(),
            _ => crate::ui::simulate_main_menu_layout_prepare_default(),
        };
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_main_menu_layout_ok_wnd_{action}")
        } else {
            "click_main_menu_layout_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_control_bar_scheme(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let name = args
            .get("name")
            .cloned()
            .unwrap_or_else(|| "America8x6".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_control_bar_scheme_clear, simulate_control_bar_scheme_get_current,
                simulate_control_bar_scheme_load, simulate_control_bar_scheme_prepare_default,
            };
            wnd_ok = match action.as_str() {
                "load" => simulate_control_bar_scheme_load(&name),
                "get" => simulate_control_bar_scheme_get_current(),
                "clear" => simulate_control_bar_scheme_clear(),
                _ => simulate_control_bar_scheme_prepare_default(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_control_bar_scheme_ok_wnd_{action}")
        } else {
            "click_control_bar_scheme_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_control_bar_print_positions(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                simulate_control_bar_print_positions_format_line,
                simulate_control_bar_print_positions_parent_name,
                simulate_control_bar_print_positions_prepare_sample,
                simulate_control_bar_print_positions_script_names,
            };
            wnd_ok = match action.as_str() {
                "parent" => simulate_control_bar_print_positions_parent_name(),
                "script" => simulate_control_bar_print_positions_script_names(),
                "format" => {
                    let block = simulate_control_bar_print_positions_format_line(
                        "ControlBar.wnd:ControlBarParent",
                        0,
                        450,
                        800,
                        150,
                    );
                    block.contains("END")
                }
                _ => simulate_control_bar_print_positions_prepare_sample(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_control_bar_print_positions_ok_wnd_{action}")
        } else {
            "click_control_bar_print_positions_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_terrain_env_boundary(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let wnd_ok = match action.as_str() {
            "heightmap" => crate::game_logic::simulate_terrain_env_boundary_heightmap_source(),
            "skybox" => crate::game_logic::simulate_terrain_env_boundary_skybox_source(),
            "sync" => crate::game_logic::simulate_terrain_env_boundary_sync_source(),
            _ => false,
        };
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_terrain_env_boundary_ok_wnd_{action}")
        } else {
            "click_terrain_env_boundary_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_main_menu_wnd(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let wnd_ok = match action.as_str() {
            "resolve" => {
                crate::gameplay_layout::resolve_main_menu_wnd_path().is_some()
                    || crate::gameplay_layout::main_menu_wnd_honesty().assets_unavailable
            }
            "validate" => {
                let h = crate::gameplay_layout::main_menu_wnd_honesty();
                h.shell_residual_ok()
            }
            "load" => crate::gameplay_layout::simulate_main_menu_wnd_prepare_load_honesty(),
            "materialise" => false,
            _ => crate::gameplay_layout::simulate_main_menu_wnd_prepare_honesty(),
        };
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_main_menu_wnd_ok_wnd_{action}")
        } else {
            "click_main_menu_wnd_miss".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_shell_stack(&mut self, args: &HashMap<String, String>) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "push".to_string());
        let ok = match action.as_str() {
            "init" => crate::game_logic::honesty_show_shell_menu_init_before_push_source(),
            "snapshot" => crate::game_logic::honesty_shell_snapshot_no_invented_stack_source(),
            "push" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_shell_stack_ok_wnd_{action}")
        } else {
            format!("click_shell_stack_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_shell_skirmish_nav(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "push".to_string());
        let ok = match action.as_str() {
            "windows" => {
                #[cfg(feature = "game_client")]
                {
                    false && game_client::gui::with_window_manager_ref(|wm| wm.window_count() > 0)
                }
                #[cfg(not(feature = "game_client"))]
                {
                    true
                }
            }
            "push" | "prepare" | "skirmish" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_shell_skirmish_nav_ok_wnd_{action}")
        } else {
            format!("click_shell_skirmish_nav_miss_{action}")
        };
    }
}
