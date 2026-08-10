#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_live_command_attack_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_attack_log_ok_{action}")
                } else {
                    format!("click_live_command_attack_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_guard_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_guard_log_ok_{action}")
                } else {
                    format!("click_live_command_guard_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_production_construction_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_production_construction_log_ok_{action}")
                } else {
                    format!("click_live_command_production_construction_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_rally_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_rally_log_ok_{action}")
                } else {
                    format!("click_live_command_rally_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_evacuate_contain_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_evacuate_contain_log_ok_{action}")
                } else {
                    format!("click_live_evacuate_contain_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_cheer_science_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_cheer_science_log_ok_{action}")
                } else {
                    format!("click_live_command_cheer_science_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_deploy_status_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_deploy_status_log_ok_{action}")
                } else {
                    format!("click_live_command_deploy_status_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_formation_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_formation_log_ok_{action}")
                } else {
                    format!("click_live_command_formation_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_order_target_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_order_target_log_ok_{action}")
                } else {
                    format!("click_live_command_order_target_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_selection_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_selection_log_ok_{action}")
                } else {
                    format!("click_live_command_selection_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_non_attack_order_target(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_non_attack_order_target_ok_{action}")
                } else {
                    format!("click_live_command_non_attack_order_target_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_golden_mopup_default_off(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_golden_mopup_default_off_ok_{action}")
                } else {
                    format!("click_live_golden_mopup_default_off_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_beacon_note(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_beacon_note_ok_{action}")
                } else {
                    format!("click_live_command_beacon_note_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_sell_deselect_log(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_sell_deselect_log_ok_{action}")
                } else {
                    format!("click_live_command_sell_deselect_log_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_unit_authority_api(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_unit_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_unit_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_unit_more_authority_api(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_unit_more_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_unit_more_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_executor_authority_api(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_executor_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_executor_authority_api_miss_{action}")
                };
            }

    pub(super) fn runtime_host_cmd_click_live_command_executor_more_authority_api(&mut self, args: &HashMap<String, String>)  {
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
                    format!("click_live_command_executor_more_authority_api_ok_{action}")
                } else {
                    format!("click_live_command_executor_more_authority_api_miss_{action}")
                };
            }

}
