#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_click_live_die_command_dual_world_empty_gate(
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
            format!("click_live_die_command_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_die_command_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_upgrade_behavior_dual_world_empty_gate(
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
            format!("click_live_upgrade_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_upgrade_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_construction_placement_dual_world_empty_gate(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "prepare".to_string());
        // World-click path is live: selected dozer + pending template +
        // LMB on terrain → place_structure_from_ui (real build time).
        let ok = match action.as_str() {
            "live" | "prepare" => true,
            _ => self.host_unknown_action_fail_closed(false),
        };
        self.runtime_host_last_gameplay_cmd = if ok {
            format!("click_live_construction_placement_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_construction_placement_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_client_dual_world_empty_gate(
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
            format!("click_live_client_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_client_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_group_dual_world_empty_gate(
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
            format!("click_live_ai_group_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_group_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_slaved_update_dual_world_empty_gate(
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
            format!("click_live_slaved_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_slaved_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_demoralize_power_dual_world_empty_gate(
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
            format!("click_live_demoralize_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_demoralize_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_bone_fx_update_dual_world_empty_gate(
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
            format!("click_live_bone_fx_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_bone_fx_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_supply_warehouse_dock_dual_world_empty_gate(
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
            format!("click_live_supply_warehouse_dock_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_supply_warehouse_dock_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ocl_special_power_dual_world_empty_gate(
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
            format!("click_live_ocl_special_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ocl_special_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_railed_transport_ai_update_dual_world_empty_gate(
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
            format!("click_live_railed_transport_ai_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_railed_transport_ai_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_squish_collide_dual_world_empty_gate(
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
            format!("click_live_squish_collide_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_squish_collide_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_weapon_bonus_update_dual_world_empty_gate(
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
            format!("click_live_weapon_bonus_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_weapon_bonus_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_minefield_behavior_dual_world_empty_gate(
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
            format!("click_live_minefield_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_minefield_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_point_defense_laser_update_dual_world_empty_gate(
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
            format!("click_live_point_defense_laser_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_point_defense_laser_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_lifetime_update_dual_world_empty_gate(
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
            format!("click_live_lifetime_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_lifetime_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_slow_death_behavior_dual_world_empty_gate(
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
            format!("click_live_slow_death_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_slow_death_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_battle_bus_slow_death_behavior_dual_world_empty_gate(
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
            format!("click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_damage_module_dual_world_empty_gate(
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
            format!("click_live_damage_module_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_damage_module_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_transition_damage_fx_dual_world_empty_gate(
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
            format!("click_live_transition_damage_fx_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_transition_damage_fx_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_spawn_point_production_exit_behavior_dual_world_empty_gate(
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
                "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_ok_{action}"
            )
        } else {
            format!(
                "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_miss_{action}"
            )
        };
    }

    pub(super) fn runtime_host_cmd_click_live_build_placement_dual_world_empty_gate(
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
            format!("click_live_build_placement_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_build_placement_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_weapon_set_dual_world_empty_gate(
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
            format!("click_live_weapon_set_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_weapon_set_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_experience_tracker_dual_world_empty_gate(
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
            format!("click_live_experience_tracker_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_experience_tracker_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_targeting_dual_world_empty_gate(
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
            format!("click_live_ai_targeting_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_targeting_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_move_to_state_dual_world_empty_gate(
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
            format!("click_live_move_to_state_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_move_to_state_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_locomotor_core_dual_world_empty_gate(
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
            format!("click_live_locomotor_core_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_locomotor_core_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_path_following_dual_world_empty_gate(
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
            format!("click_live_path_following_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_path_following_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_manager_dual_world_empty_gate(
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
            format!("click_live_ai_manager_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_manager_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_states_dual_world_empty_gate(
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
            format!("click_live_ai_states_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_states_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_player_dual_world_empty_gate(
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
            format!("click_live_ai_player_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_player_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_team_dual_world_empty_gate(
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
            format!("click_live_team_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_team_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_legacy_states_dual_world_empty_gate(
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
            format!("click_live_ai_legacy_states_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_legacy_states_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_unit_dual_world_empty_gate(
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
            format!("click_live_unit_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_unit_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_dual_world_empty_gate(
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
            format!("click_live_stealth_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_garrison_dual_world_empty_gate(
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
            format!("click_live_garrison_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_garrison_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_open_contain_dual_world_empty_gate(
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
            format!("click_live_open_contain_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_open_contain_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_pathfind_dual_world_empty_gate(
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
            format!("click_live_pathfind_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_pathfind_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate(
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
            format!("click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_guard_dual_world_empty_gate(
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
            format!("click_live_guard_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_guard_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_guard_retaliate_dual_world_empty_gate(
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
            format!("click_live_guard_retaliate_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_guard_retaliate_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_wander_ai_dual_world_empty_gate(
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
            format!("click_live_wander_ai_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_wander_ai_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_subobjects_upgrade_dual_world_empty_gate(
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
            format!("click_live_subobjects_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_subobjects_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_unit_exit_dual_world_empty_gate(
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
            format!("click_live_unit_exit_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_unit_exit_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_owner_resolve_dual_world_empty_gate(
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
            format!("click_live_owner_resolve_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_owner_resolve_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_spy_vision_update_dual_world_empty_gate(
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
            format!("click_live_spy_vision_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_spy_vision_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_overcharge_behavior_dual_world_empty_gate(
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
            format!("click_live_overcharge_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_overcharge_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_tech_building_behavior_dual_world_empty_gate(
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
            format!("click_live_tech_building_behavior_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_tech_building_behavior_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_power_plant_upgrade_dual_world_empty_gate(
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
            format!("click_live_power_plant_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_power_plant_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_upgrade_dual_world_empty_gate(
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
            format!("click_live_stealth_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_aurora_strike_power_dual_world_empty_gate(
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
            format!("click_live_aurora_strike_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_aurora_strike_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_carpet_bomb_power_dual_world_empty_gate(
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
            format!("click_live_carpet_bomb_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_carpet_bomb_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_nuclear_missile_power_dual_world_empty_gate(
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
            format!("click_live_nuclear_missile_power_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_nuclear_missile_power_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_overlord_draw_dual_world_empty_gate(
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
            format!("click_live_overlord_draw_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_overlord_draw_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_stealth_integration_dual_world_empty_gate(
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
            format!("click_live_stealth_integration_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_stealth_integration_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_player_upgrade_manager_dual_world_empty_gate(
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
            format!("click_live_player_upgrade_manager_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_player_upgrade_manager_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_advanced_nuggets_dual_world_empty_gate(
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
            format!("click_live_advanced_nuggets_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_advanced_nuggets_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_replace_object_upgrade_dual_world_empty_gate(
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
            format!("click_live_replace_object_upgrade_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_replace_object_upgrade_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_fire_spread_update_dual_world_empty_gate(
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
            format!("click_live_fire_spread_update_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_fire_spread_update_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_object_upgrade_batch_dual_world_empty_gate(
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
            format!("click_live_object_upgrade_batch_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_object_upgrade_batch_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_core_sim_dual_world_empty_gate(
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
            format!("click_live_core_sim_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_core_sim_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_mod_dual_world_empty_gate(
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
            format!("click_live_ai_mod_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_mod_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_object_mod_dual_world_empty_gate(
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
            format!("click_live_object_mod_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_object_mod_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_weapon_dual_world_empty_gate(
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
            format!("click_live_weapon_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_weapon_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_partition_filters_dual_world_empty_gate(
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
            format!("click_live_partition_filters_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_partition_filters_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_ai_state_machine_dual_world_empty_gate(
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
            format!("click_live_ai_state_machine_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_ai_state_machine_dual_world_empty_gate_miss_{action}")
        };
    }

    pub(super) fn runtime_host_cmd_click_live_player_dual_world_empty_gate(
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
            format!("click_live_player_dual_world_empty_gate_ok_{action}")
        } else {
            format!("click_live_player_dual_world_empty_gate_miss_{action}")
        };
    }
}
