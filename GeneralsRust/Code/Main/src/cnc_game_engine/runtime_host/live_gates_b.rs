#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_live_game_client_dual_world_empty_gate(
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
            format!("click_live_game_client_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_game_client_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_drawable_dual_world_empty_gate(
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
            format!("click_live_drawable_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_drawable_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_script_conditions_dual_world_empty_gate(
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
            format!("click_live_script_conditions_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_script_conditions_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_transport_contain_dual_world_empty_gate(
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
            format!("click_live_transport_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_transport_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ingame_ui_dual_world_empty_gate(
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
            format!("click_live_ingame_ui_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ingame_ui_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_helix_contain_dual_world_empty_gate(
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
            format!("click_live_helix_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_helix_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_command_processor_dual_world_empty_gate(
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
            format!("click_live_command_processor_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_command_processor_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_turret_dual_world_empty_gate(
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
            format!("click_live_turret_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_turret_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_rider_change_contain_dual_world_empty_gate(
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
            format!("click_live_rider_change_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_rider_change_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_selection_dual_world_empty_gate(
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
            format!("click_live_selection_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_selection_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_cave_contain_dual_world_empty_gate(
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
            format!("click_live_cave_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_cave_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_tunnel_contain_dual_world_empty_gate(
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
            format!("click_live_tunnel_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_tunnel_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_helpers_dual_world_empty_gate(
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
            format!("click_live_helpers_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_helpers_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_update_interface_dual_world_empty_gate(
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
            format!("click_live_ai_update_interface_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_update_interface_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_update_dual_world_empty_gate(
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
            format!("click_live_stealth_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_script_executor_dual_world_empty_gate(
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
            format!("click_live_script_executor_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_script_executor_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_integration_dual_world_empty_gate(
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
            format!("click_live_ai_integration_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_integration_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_dumb_projectile_dual_world_empty_gate(
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
            format!("click_live_dumb_projectile_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_dumb_projectile_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_enhanced_player_dual_world_empty_gate(
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
            format!("click_live_enhanced_player_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_enhanced_player_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_hijacker_update_dual_world_empty_gate(
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
            format!("click_live_hijacker_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_hijacker_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_weapon_impl_dual_world_empty_gate(
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
            format!("click_live_weapon_impl_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_weapon_impl_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_async_player_dual_world_empty_gate(
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
            format!("click_live_async_player_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_async_player_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_active_body_dual_world_empty_gate(
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
            format!("click_live_active_body_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_active_body_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_skirmish_conditions_dual_world_empty_gate(
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
            format!("click_live_skirmish_conditions_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_skirmish_conditions_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_build_list_dual_world_empty_gate(
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
            format!("click_live_ai_build_list_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_build_list_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_victory_dual_world_empty_gate(
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
            format!("click_live_victory_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_victory_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_script_actions_dual_world_empty_gate(
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
            format!("click_live_script_actions_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_script_actions_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_special_ability_dual_world_empty_gate(
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
            format!("click_live_special_ability_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_special_ability_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_detector_dual_world_empty_gate(
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
            format!("click_live_stealth_detector_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_detector_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_supply_system_dual_world_empty_gate(
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
            format!("click_live_supply_system_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_supply_system_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_particle_uplink_dual_world_empty_gate(
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
            format!("click_live_particle_uplink_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_particle_uplink_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_overlord_contain_dual_world_empty_gate(
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
            format!("click_live_overlord_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_overlord_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_bridge_behavior_dual_world_empty_gate(
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
            format!("click_live_bridge_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_bridge_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_behavior_dual_world_empty_gate(
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
            format!("click_live_stealth_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_crate_collide_dual_world_empty_gate(
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
            format!("click_live_crate_collide_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_crate_collide_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_object_manager_dual_world_empty_gate(
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
            format!("click_live_object_manager_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_object_manager_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_sticky_bomb_dual_world_empty_gate(
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
            format!("click_live_sticky_bomb_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_sticky_bomb_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_auto_heal_dual_world_empty_gate(
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
            format!("click_live_auto_heal_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_auto_heal_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_grant_stealth_dual_world_empty_gate(
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
            format!("click_live_grant_stealth_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_grant_stealth_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_status_bits_upgrade_dual_world_empty_gate(
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
            format!("click_live_status_bits_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_status_bits_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_jet_ai_dual_world_empty_gate(
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
            format!("click_live_jet_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_jet_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_parking_place_dual_world_empty_gate(
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
            format!("click_live_parking_place_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_parking_place_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_flight_deck_dual_world_empty_gate(
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
            format!("click_live_flight_deck_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_flight_deck_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_exit_strategies_dual_world_empty_gate(
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
            format!("click_live_exit_strategies_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_exit_strategies_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_collision_system_dual_world_empty_gate(
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
            format!("click_live_collision_system_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_collision_system_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_max_health_upgrade_dual_world_empty_gate(
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
            format!("click_live_max_health_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_max_health_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_structure_topple_dual_world_empty_gate(
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
            format!("click_live_structure_topple_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_structure_topple_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_physics_update_dual_world_empty_gate(
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
            format!("click_live_physics_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_physics_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_cleanup_hazard_dual_world_empty_gate(
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
            format!("click_live_cleanup_hazard_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_cleanup_hazard_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_bridge_tower_dual_world_empty_gate(
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
            format!("click_live_bridge_tower_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_bridge_tower_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_armor_upgrade_dual_world_empty_gate(
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
            format!("click_live_armor_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_armor_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_paradrop_power_dual_world_empty_gate(
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
            format!("click_live_paradrop_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_paradrop_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_fuel_air_bomb_dual_world_empty_gate(
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
            format!("click_live_fuel_air_bomb_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_fuel_air_bomb_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_tensile_formation_dual_world_empty_gate(
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
            format!("click_live_tensile_formation_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_tensile_formation_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_die_mod_dual_world_empty_gate(
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
            format!("click_live_die_mod_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_die_mod_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_partition_manager_dual_world_empty_gate(
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
            format!("click_live_partition_manager_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_partition_manager_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_spectre_gunship_dual_world_empty_gate(
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
            format!("click_live_spectre_gunship_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_spectre_gunship_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_production_update_dual_world_empty_gate(
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
            format!("click_live_production_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_production_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_neutron_blast_dual_world_empty_gate(
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
            format!("click_live_neutron_blast_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_neutron_blast_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_countermeasures_dual_world_empty_gate(
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
            format!("click_live_countermeasures_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_countermeasures_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_skirmish_player_dual_world_empty_gate(
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
            format!("click_live_skirmish_player_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_skirmish_player_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_a10_strike_dual_world_empty_gate(
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
            format!("click_live_a10_strike_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_a10_strike_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_rebuild_hole_dual_world_empty_gate(
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
            format!("click_live_rebuild_hole_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_rebuild_hole_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_wave_guide_dual_world_empty_gate(
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
            format!("click_live_wave_guide_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_wave_guide_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_emp_update_dual_world_empty_gate(
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
            format!("click_live_emp_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_emp_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_bunker_buster_dual_world_empty_gate(
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
            format!("click_live_bunker_buster_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_bunker_buster_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_bridge_scaffold_dual_world_empty_gate(
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
            format!("click_live_bridge_scaffold_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_bridge_scaffold_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_assisted_targeting_dual_world_empty_gate(
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
            format!("click_live_assisted_targeting_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_assisted_targeting_dual_world_empty_gate_miss_{action}")
        };
    }
}
