#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_live_player_probe_api(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_player_probe_api_ok_{action}")
        } else {
            format!("click_live_player_probe_api_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_player_team_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_player_team_probe_ok_{action}")
        } else {
            format!("click_live_player_team_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_player_field_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_player_field_probe_ok_{action}")
        } else {
            format!("click_live_player_field_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_camera_height_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => false,
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_camera_height_probe_ok_{action}")
        } else {
            format!("click_live_camera_height_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_command_player_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => false,
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_command_player_probe_ok_{action}")
        } else {
            format!("click_live_command_player_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_construct_economy_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => false,
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_construct_economy_probe_ok_{action}")
        } else {
            format!("click_live_construct_economy_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_command_unit_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_command_unit_probe_ok_{action}")
        } else {
            format!("click_live_command_unit_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_selection_query_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => false,
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_selection_query_probe_ok_{action}")
        } else {
            format!("click_live_selection_query_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_world_pick_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_world_pick_probe_ok_{action}")
        } else {
            format!("click_live_world_pick_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_object_registry_empty_fastpath(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_object_registry_empty_fastpath_ok_{action}")
        } else {
            format!("click_live_object_registry_empty_fastpath_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_legacy_object_registry_fastpath(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_legacy_object_registry_fastpath_ok_{action}")
        } else {
            format!("click_live_legacy_object_registry_fastpath_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_presentation_time_frozen_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_presentation_time_frozen_probe_ok_{action}")
        } else {
            format!("click_live_presentation_time_frozen_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_presentation_visual_speed_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_presentation_visual_speed_probe_ok_{action}")
        } else {
            format!("click_live_presentation_visual_speed_probe_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_presentation_script_camera_probe(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        let ok = match action.as_str() {
            "live" | "prepare" => false,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_presentation_script_camera_probe_ok_{action}")
        } else {
            format!("click_live_presentation_script_camera_probe_miss_{action}")
        };
    }
}
