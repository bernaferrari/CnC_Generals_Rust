#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_gameworld_authority(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "refresh" => {
                        crate::gameworld_shadow::simulate_gameworld_authority_refresh_env()
                    }
                    "shadow" => crate::gameworld_shadow::simulate_gameworld_shadow_enable_check(),
                    "probe" => self.host_simulate_gameworld_authority_probe(),
                    _ => crate::gameworld_shadow::simulate_gameworld_authority_prepare_defaults(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_gameworld_authority_ok_wnd_{action}")
                } else {
                    "click_gameworld_authority_miss".into()
                };
            }

    pub(super) fn runtime_host_cmd_click_presentation_boundary(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let wnd_ok = match action.as_str() {
                    "execute" => crate::graphics::simulate_presentation_boundary_execute_source(),
                    "collect" => crate::graphics::simulate_presentation_boundary_collect_source(),
                    "fallback" => {
                        crate::graphics::simulate_presentation_boundary_fallback_counter_source()
                    }
                    "cnc" => crate::graphics::simulate_presentation_boundary_cnc_execute_source(),
                    _ => crate::graphics::simulate_presentation_boundary_prepare_honesty(),
                };
                self.runtime_host_last_gameplay_cmd = if wnd_ok {
                    format!("click_presentation_boundary_ok_wnd_{action}")
                } else {
                    "click_presentation_boundary_miss".into()
                };
            }

    pub(super) fn runtime_host_cmd_click_live_map_load(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "defcon" | "lone_eagle" | "prepare" | "load" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_map_load_ok_{action}")
                } else {
                    format!("click_live_map_load_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_seed(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "source" => {
                        crate::game_logic::honesty_seed_presentation_after_match_start_source()
                            && crate::game_logic::honesty_render_execute_presentation_only_source()
                    }
                    "build" | "prepare" | "seed" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_seed_ok_{action}")
                } else {
                    format!("click_live_presentation_seed_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_shadow(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "source" => {
                        crate::game_logic::honesty_seed_presentation_shadow_overlay_source()
                    }
                    "sync" | "overlay" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_shadow_ok_{action}")
                } else {
                    format!("click_live_gameworld_shadow_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_single_authority(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "policy" | "teleport" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_single_authority_ok_{action}")
                } else {
                    format!("click_single_authority_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_presentation_client_boundary(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "execute" | "client" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_presentation_client_boundary_ok_{action}")
                } else {
                    format!("click_presentation_client_boundary_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_golden_map_host_victory(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "clear" | "formula" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_golden_map_host_victory_ok_{action}")
                } else {
                    format!("click_golden_map_host_victory_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_executable_presentation_boundary(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "vertical" | "fallback" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_executable_presentation_boundary_ok_{action}")
                } else {
                    format!("click_executable_presentation_boundary_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_gameworld_production_authority(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "enabled" | "sole" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_production_authority_ok_{action}")
                } else {
                    format!("click_gameworld_production_authority_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_gameworld_sole_tick_coupling(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "couple" | "uncouple" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_sole_tick_coupling_ok_{action}")
                } else {
                    format!("click_gameworld_sole_tick_coupling_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_gameworld_authority_matrix(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "all" | "couple" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_gameworld_authority_matrix_ok_{action}")
                } else {
                    format!("click_gameworld_authority_matrix_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_production_writeback(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "progress" | "writeback" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_production_writeback_ok_{action}")
                } else {
                    format!("click_live_gameworld_production_writeback_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_construction_writeback(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "progress" | "sole" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_construction_writeback_ok_{action}")
                } else {
                    format!("click_live_gameworld_construction_writeback_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_damage_channel(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "log" | "parity" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_damage_channel_ok_{action}")
                } else {
                    format!("click_live_gameworld_damage_channel_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_economy_movement(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "economy" | "movement" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_economy_movement_ok_{action}")
                } else {
                    format!("click_live_gameworld_economy_movement_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_projectile_ai(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "projectile" | "ai" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_projectile_ai_ok_{action}")
                } else {
                    format!("click_live_gameworld_projectile_ai_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_fire_special_power(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "special" | "fire" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_fire_special_power_ok_{action}")
                } else {
                    format!("click_live_gameworld_fire_special_power_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_presentation_view(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "view" | "engine" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_presentation_view_ok_{action}")
                } else {
                    format!("click_live_gameworld_presentation_view_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_gameworld_overlay(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "overlay" | "stamp" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_gameworld_overlay_ok_{action}")
                } else {
                    format!("click_live_presentation_gameworld_overlay_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_executable_gameworld_presentation(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "status" | "vertical" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_executable_gameworld_presentation_ok_{action}")
                } else {
                    format!("click_executable_gameworld_presentation_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_overlay_deepen(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "velocity" | "selection" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_overlay_deepen_ok_{action}")
                } else {
                    format!("click_live_presentation_overlay_deepen_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_overlay_stamp(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "field" | "status" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_overlay_stamp_ok_{action}")
                } else {
                    format!("click_live_presentation_overlay_stamp_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_gameworld_entity_view_deepen(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "view" | "exec" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_gameworld_entity_view_deepen_ok_{action}")
                } else {
                    format!("click_live_gameworld_entity_view_deepen_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_append_missing(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_append_missing_ok_{action}")
                } else {
                    format!("click_live_presentation_append_missing_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_build_from_gameworld(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_build_from_gameworld_ok_{action}")
                } else {
                    format!("click_live_presentation_build_from_gameworld_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_from_gameworld_default(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_from_gameworld_default_ok_{action}")
                } else {
                    format!("click_live_presentation_from_gameworld_default_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_build_for_engine(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_build_for_engine_ok_{action}")
                } else {
                    format!("click_live_presentation_build_for_engine_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_rebuilt_vertical_gate(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_rebuilt_vertical_gate_ok_{action}")
                } else {
                    format!("click_live_presentation_rebuilt_vertical_gate_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_env_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_env_only_ok_{action}")
                } else {
                    format!("click_live_presentation_env_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_os_input_command_path(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_os_input_command_path_ok_{action}")
                } else {
                    format!("click_live_os_input_command_path_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_host_beacon_presentation(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_host_beacon_presentation_ok_{action}")
                } else {
                    format!("click_live_host_beacon_presentation_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_presentation_fow_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => {
                        false
                    }
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_presentation_fow_only_ok_{action}")
                } else {
                    format!("click_live_presentation_fow_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_ui_producer_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_producer_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_producer_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_ui_helpers_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_helpers_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_helpers_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_control_group_camera_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_control_group_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_control_group_camera_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_cmd_filter_env_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_cmd_filter_env_presentation_only_ok_{action}")
                } else {
                    format!("click_live_cmd_filter_env_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_selection_commands_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_selection_commands_presentation_only_ok_{action}")
                } else {
                    format!("click_live_selection_commands_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_ui_command_selection_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_ui_command_selection_presentation_only_ok_{action}")
                } else {
                    format!("click_live_ui_command_selection_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_local_team_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_local_team_presentation_only_ok_{action}")
                } else {
                    format!("click_live_local_team_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_hotkey_move_attack_selection_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hotkey_move_attack_selection_presentation_only_ok_{action}")
                } else {
                    format!(
                        "click_live_hotkey_move_attack_selection_presentation_only_miss_{action}"
                    )
                };
            }

    pub(super) fn runtime_host_cmd_click_live_pick_object_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_pick_object_presentation_only_ok_{action}")
                } else {
                    format!("click_live_pick_object_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_bootstrap_camera_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_bootstrap_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_bootstrap_camera_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_force_complete_authority_api(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_force_complete_authority_api_ok_{action}")
                } else {
                    format!("click_live_force_complete_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_path_guard_authority_api(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_path_guard_authority_api_ok_{action}")
                } else {
                    format!("click_live_path_guard_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_hotkey_selection_camera_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_hotkey_selection_camera_presentation_only_ok_{action}")
                } else {
                    format!("click_live_hotkey_selection_camera_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_construct_spawn_pose_authority_api(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_construct_spawn_pose_authority_api_ok_{action}")
                } else {
                    format!("click_live_construct_spawn_pose_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_rmb_target_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_target_presentation_only_ok_{action}")
                } else {
                    format!("click_live_rmb_target_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_rmb_selected_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_selected_presentation_only_ok_{action}")
                } else {
                    format!("click_live_rmb_selected_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_engine_presentation_player_ui(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_engine_presentation_player_ui_ok_{action}")
                } else {
                    format!("click_live_engine_presentation_player_ui_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_rmb_presentation_full_classify(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_rmb_presentation_full_classify_ok_{action}")
                } else {
                    format!("click_live_rmb_presentation_full_classify_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_mouse_input_presentation_only(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_mouse_input_presentation_only_ok_{action}")
                } else {
                    format!("click_live_mouse_input_presentation_only_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_engine_player_ui_boot_peel(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_engine_player_ui_boot_peel_ok_{action}")
                } else {
                    format!("click_live_engine_player_ui_boot_peel_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_contain_module_overrides_fail_closed(&mut self, args: &HashMap<String, String>)  {
                let action = args
                    .get("action")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "prepare".to_string());
                let ok = match action.as_str() {
                    "live" | "prepare" => {
                        false
                    }
                    _ => self.host_unknown_action_fail_closed(false),
                };
                self.runtime_host_last_gameplay_cmd = if ok {
                    format!("click_live_contain_module_overrides_fail_closed_ok_{action}")
                } else {
                    format!("click_live_contain_module_overrides_fail_closed_miss_{action}")
                };
            }

}
