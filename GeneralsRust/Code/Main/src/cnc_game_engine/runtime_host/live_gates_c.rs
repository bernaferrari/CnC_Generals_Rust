#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_live_economy_dual_world_empty_gate(
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
            format!("click_live_economy_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_economy_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_turret_ai_dual_world_empty_gate(
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
            format!("click_live_turret_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_turret_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_detector_module_dual_world_empty_gate(
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
            format!("click_live_stealth_detector_module_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_detector_module_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_modules_dual_world_empty_gate(
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
            format!("click_live_modules_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_modules_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_terrain_dual_world_empty_gate(
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
            format!("click_live_terrain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_terrain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_special_power_template_dual_world_empty_gate(
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
            format!("click_live_special_power_template_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_special_power_template_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_script_evaluator_dual_world_empty_gate(
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
            format!("click_live_script_evaluator_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_script_evaluator_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_system_game_logic_dual_world_empty_gate(
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
            format!("click_live_system_game_logic_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_system_game_logic_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_meta_event_dual_world_empty_gate(
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
            format!("click_live_meta_event_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_meta_event_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_spawn_behavior_dual_world_empty_gate(
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
            format!("click_live_spawn_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_spawn_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_action_manager_dual_world_empty_gate(
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
            format!("click_live_action_manager_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_action_manager_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_script_engine_dual_world_empty_gate(
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
            format!("click_live_script_engine_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_script_engine_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_chinook_ai_dual_world_empty_gate(
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
            format!("click_live_chinook_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_chinook_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_missile_ai_dual_world_empty_gate(
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
            format!("click_live_missile_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_missile_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_dozer_ai_dual_world_empty_gate(
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
            format!("click_live_dozer_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_dozer_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_deliver_payload_ai_dual_world_empty_gate(
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
            format!("click_live_deliver_payload_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_deliver_payload_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_special_power_module_dual_world_empty_gate(
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
            format!("click_live_special_power_module_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_special_power_module_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_pow_truck_ai_dual_world_empty_gate(
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
            format!("click_live_pow_truck_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_pow_truck_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_dock_update_dual_world_empty_gate(
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
            format!("click_live_dock_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_dock_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_weapon_template_dual_world_empty_gate(
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
            format!("click_live_weapon_template_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_weapon_template_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_railroad_guide_ai_dual_world_empty_gate(
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
            format!("click_live_railroad_guide_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_railroad_guide_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_hack_internet_ai_dual_world_empty_gate(
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
            format!("click_live_hack_internet_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_hack_internet_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_spectre_gunship_deployment_dual_world_empty_gate(
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
            format!("click_live_spectre_gunship_deployment_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_spectre_gunship_deployment_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_radius_decal_update_dual_world_empty_gate(
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
            format!("click_live_radius_decal_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_radius_decal_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_railed_transport_dock_dual_world_empty_gate(
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
            format!("click_live_railed_transport_dock_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_railed_transport_dock_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_structure_collapse_update_dual_world_empty_gate(
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
            format!("click_live_structure_collapse_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_structure_collapse_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_propaganda_tower_behavior_dual_world_empty_gate(
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
            format!("click_live_propaganda_tower_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_propaganda_tower_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_propaganda_center_behavior_dual_world_empty_gate(
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
            format!("click_live_propaganda_center_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_propaganda_center_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_production_update_complete_dual_world_empty_gate(
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
            format!("click_live_production_update_complete_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_production_update_complete_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_pow_truck_behavior_dual_world_empty_gate(
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
            format!("click_live_pow_truck_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_pow_truck_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate(
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
            format!(
                "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_veterancy_crate_collide_dual_world_empty_gate(
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
            format!("click_live_veterancy_crate_collide_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_veterancy_crate_collide_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_assault_transport_ai_update_dual_world_empty_gate(
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
            format!("click_live_assault_transport_ai_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_assault_transport_ai_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_heal_contain_dual_world_empty_gate(
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
            format!("click_live_heal_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_heal_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_topple_update_dual_world_empty_gate(
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
            format!("click_live_topple_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_topple_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_projectile_stream_update_dual_world_empty_gate(
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
            format!("click_live_projectile_stream_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_projectile_stream_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_demo_trap_update_dual_world_empty_gate(
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
            format!("click_live_demo_trap_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_demo_trap_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_mob_member_slaved_update_dual_world_empty_gate(
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
            format!("click_live_mob_member_slaved_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_mob_member_slaved_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_tn_guard_dual_world_empty_gate(
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
            format!("click_live_tn_guard_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_tn_guard_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_production_update_dual_world_empty_gate_wave376(
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
            format!("click_live_production_update_dual_world_empty_gate_wave376_ok_{action}")
        } else {
            format!("click_live_production_update_dual_world_empty_gate_wave376_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_poisoned_behavior_dual_world_empty_gate(
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
            format!("click_live_poisoned_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_poisoned_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_horde_update_dual_world_empty_gate(
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
            format!("click_live_horde_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_horde_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_flammable_update_dual_world_empty_gate(
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
            format!("click_live_flammable_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_flammable_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_base_regenerate_update_dual_world_empty_gate(
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
            format!("click_live_base_regenerate_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_base_regenerate_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_queue_production_exit_behavior_dual_world_empty_gate(
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
            format!("click_live_queue_production_exit_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_queue_production_exit_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_missile_launcher_building_update_dual_world_empty_gate(
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
            format!("click_live_missile_launcher_building_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!(
                "click_live_missile_launcher_building_update_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate(
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
            format!(
                "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_command_button_hunt_update_dual_world_empty_gate(
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
            format!("click_live_command_button_hunt_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_command_button_hunt_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_prison_behavior_dual_world_empty_gate(
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
            format!("click_live_prison_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_prison_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_generate_minefield_behavior_dual_world_empty_gate(
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
            format!("click_live_generate_minefield_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_generate_minefield_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_demoralize_special_power_dual_world_empty_gate(
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
            format!("click_live_demoralize_special_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_demoralize_special_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_detector_update_dual_world_empty_gate(
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
            format!("click_live_stealth_detector_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_detector_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_hive_structure_body_dual_world_empty_gate(
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
            format!("click_live_hive_structure_body_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_hive_structure_body_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_salvage_crate_collide_dual_world_empty_gate(
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
            format!("click_live_salvage_crate_collide_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_salvage_crate_collide_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate(
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
            format!(
                "click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_power_plant_update_dual_world_empty_gate(
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
            format!("click_live_power_plant_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_power_plant_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_leaflet_drop_behavior_dual_world_empty_gate(
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
            format!("click_live_leaflet_drop_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_leaflet_drop_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_auto_deposit_update_dual_world_empty_gate(
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
            format!("click_live_auto_deposit_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_auto_deposit_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate(
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
            format!(
                "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_neutron_missile_slow_death_update_dual_world_empty_gate(
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
            format!(
                "click_live_neutron_missile_slow_death_update_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_neutron_missile_slow_death_update_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_dock_dual_world_empty_gate(
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
            format!("click_live_ai_dock_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_dock_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_groups_dual_world_empty_gate(
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
            format!("click_live_ai_groups_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_groups_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_artillery_barrage_power_dual_world_empty_gate(
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
            format!("click_live_artillery_barrage_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_artillery_barrage_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_baikonur_launch_power_dual_world_empty_gate(
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
            format!("click_live_baikonur_launch_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_baikonur_launch_power_dual_world_empty_gate_miss_{action}")
        };
    }
}
