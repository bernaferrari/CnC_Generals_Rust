#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_special_power(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "special_power_fail_not_ingame".into();
        } else {
            self.issue_named_command_from_ui("Command_DoSpecialPower");
            self.runtime_host_last_gameplay_cmd = "special_power_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_remove_beacon(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "remove_beacon_fail_not_ingame".into();
        } else {
            self.issue_named_command_from_ui("Command_RemoveBeacon");
            self.runtime_host_last_gameplay_cmd = "remove_beacon_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_demo_suicide(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "demo_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            self.issue_named_command_from_ui("Command_DemoTertiarySuicide");
            self.runtime_host_last_gameplay_cmd = "demo_suicide_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_detonate_remote(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "detonate_remote_fail_not_ingame".into();
        } else {
            self.issue_named_command_from_ui("Command_DetonateRemoteDemoCharges");
            self.runtime_host_last_gameplay_cmd = "detonate_remote_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_view_last_radar(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "view_radar_fail_not_ingame".into();
        } else {
            self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
            self.runtime_host_last_gameplay_cmd = "view_radar_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_force_attack(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "force_attack_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            // Wave 218: selection via presentation-first ui_selected_ids.
            let selected = self.ui_selected_ids(self.current_player_id);
            if selected.is_empty() {
                self.runtime_host_last_gameplay_cmd = "force_attack_fail_no_selection".into();
            } else {
                let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                let loc = glam::Vec3::new(x, y, z);
                self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
                    command_type: crate::command_system::CommandType::ForceAttackGround {
                        location: loc,
                    },
                    player_id: self.current_player_id,
                    command_id: 0,
                    timestamp: std::time::SystemTime::now(),
                    selected_units: selected.clone(),
                    modifier_keys: crate::command_system::ModifierKeys {
                        ctrl: true,
                        shift: false,
                        alt: false,
                    },
                });
                self.runtime_host_last_gameplay_cmd =
                    format!("force_attack_ok:{},{},{}:{}", x, y, z, selected.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_force_attack_object(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "force_attack_object_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            // Wave 218: selection via presentation-first ui_selected_ids.
            let selected = self.ui_selected_ids(self.current_player_id);
            if selected.is_empty() {
                self.runtime_host_last_gameplay_cmd =
                    "force_attack_object_fail_no_selection".into();
            } else {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                let target_id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    team.and_then(|t| frame.first_enemy_force_attack_id(t))
                } else {
                    // Presentation required (no live get_objects dual-read).
                    None
                };
                if let Some(target_id) = target_id {
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::ForceAttackObject {
                                target_id,
                            },
                            player_id: self.current_player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected.clone(),
                            modifier_keys: crate::command_system::ModifierKeys {
                                ctrl: true,
                                shift: false,
                                alt: false,
                            },
                        },
                    );
                    self.runtime_host_last_gameplay_cmd =
                        format!("force_attack_object_ok:{}", target_id.0);
                } else {
                    self.runtime_host_last_gameplay_cmd =
                        "force_attack_object_fail_no_enemy".into();
                }
            }
        }
    }

    pub(super) fn runtime_host_cmd_select_all(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_all_fail_not_ingame".into();
        } else {
            self.select_all_friendly_units();
            // Wave 218: count via presentation-first ui_selected_ids.
            let n = self.ui_selected_ids(self.current_player_id).len();
            self.runtime_host_last_gameplay_cmd = format!("select_all_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_select_all_combat(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_all_combat_fail_not_ingame".into();
        } else {
            self.select_all_friendly_combat();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_all_combat_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_assign_control_group(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "control_group_assign_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            let group: u8 = args
                .get("group")
                .or_else(|| args.get("n"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1)
                .clamp(0, 9);
            // Wave 216: selection identity via presentation-first ui_selected_ids.
            let selected = self.ui_selected_ids(self.current_player_id);
            if selected.is_empty() {
                self.runtime_host_last_gameplay_cmd =
                    "control_group_assign_fail_no_selection".into();
            } else {
                self.evict_ids_from_other_control_groups(&selected, group);
                self.write_leftover_player_create_team(group, &selected);
                self.control_groups.insert(group, selected.clone());
                self.runtime_host_last_gameplay_cmd =
                    format!("control_group_assign_ok:{}:{}", group, selected.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_recall_control_group(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "control_group_recall_fail_not_ingame".into();
        } else {
            let group: u8 = args
                .get("group")
                .or_else(|| args.get("n"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1)
                .clamp(0, 9);
            if let Some(ids) = self.control_groups.get(&group).cloned() {
                // Wave 216: presentation-only alive identity when freeze installed.
                let alive: Vec<_> = ids
                    .into_iter()
                    .filter(|id| self.ui_object_alive(*id))
                    .collect();
                if alive.is_empty() {
                    self.runtime_host_last_gameplay_cmd =
                        format!("control_group_recall_fail_empty:{}", group);
                } else {
                    // Wave 583: selection residual via host_set_selection.
                    let alive_n = alive.len();
                    self.host_set_selection(self.current_player_id, alive);
                    self.last_control_group_select = Some((group, Instant::now()));
                    self.runtime_host_last_gameplay_cmd =
                        format!("control_group_recall_ok:{}:{}", group, alive_n);
                }
            } else {
                self.runtime_host_last_gameplay_cmd =
                    format!("control_group_recall_fail_unset:{}", group);
            }
        }
    }

    pub(super) fn runtime_host_cmd_waypoint_mode(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "waypoint_mode_fail_not_ingame".into();
        } else {
            let enable = match args
                .get("on")
                .or_else(|| args.get("enabled"))
                .map(|s| s.trim().to_ascii_lowercase())
                .as_deref()
            {
                Some("1") | Some("true") | Some("on") | Some("yes") => true,
                Some("0") | Some("false") | Some("off") | Some("no") => false,
                _ => !self.sticky_waypoint_mode,
            };
            self.sticky_waypoint_mode = enable;
            // Keep command-system sticky in sync when available.
            // CommandProcessor path uses alt/sticky on click; host sets engine sticky.
            self.runtime_host_last_gameplay_cmd = if enable {
                "waypoint_mode_ok:on".into()
            } else {
                "waypoint_mode_ok:off".into()
            };
        }
    }

    pub(super) fn runtime_host_cmd_add_waypoint(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "waypoint_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            // Wave 218: selection via presentation-first ui_selected_ids.
            let selected = self.ui_selected_ids(self.current_player_id);
            if selected.is_empty() {
                self.runtime_host_last_gameplay_cmd = "waypoint_fail_no_selection".into();
            } else {
                let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                let dest = glam::Vec3::new(x, y, z);
                self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
                    command_type: crate::command_system::CommandType::AddWaypoint {
                        destination: dest,
                    },
                    player_id: self.current_player_id,
                    command_id: 0,
                    timestamp: std::time::SystemTime::now(),
                    selected_units: selected.clone(),
                    modifier_keys: crate::command_system::ModifierKeys {
                        ctrl: false,
                        shift: true,
                        alt: true,
                    },
                });
                self.runtime_host_last_gameplay_cmd =
                    format!("waypoint_ok:{},{},{}:{}", x, y, z, selected.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_box_select(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "box_select_fail_not_ingame".into();
        } else {
            // World-space AABB box select residual (same path as drag-select release).
            let min_x: f32 = args
                .get("min_x")
                .or_else(|| args.get("x0"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(-5000.0);
            let max_x: f32 = args
                .get("max_x")
                .or_else(|| args.get("x1"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(5000.0);
            let min_z: f32 = args
                .get("min_z")
                .or_else(|| args.get("z0"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(-5000.0);
            let max_z: f32 = args
                .get("max_z")
                .or_else(|| args.get("z1"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(5000.0);
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                self.runtime_host_last_gameplay_cmd = "box_select_fail_no_presentation".into();
                return;
            };
            let player_team = frame.local_team();
            let boxed: Vec<ObjectId> =
                frame.box_select_unit_ids(player_team, min_x, max_x, min_z, max_z);
            // Wave 583: selection residual via host_set_selection.
            let boxed_n = boxed.len();
            self.host_set_selection(self.current_player_id, boxed);
            self.runtime_host_last_gameplay_cmd = format!("box_select_ok:{}", boxed_n);
        }
    }

    pub(super) fn runtime_host_cmd_select_similar(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_similar_fail_not_ingame".into();
        } else {
            // Wave 218: seed/count via presentation-first ui_selected_ids.
            let seed = self
                .ui_selected_ids(self.current_player_id)
                .first()
                .copied()
                .or_else(|| {
                    if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                            .alive_selectable_friendly_mobile_ids(frame.local_team())
                            .into_iter()
                            .next()
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        None
                    }
                });
            if let Some(seed) = seed {
                self.select_similar_units(seed);
                let n = self.ui_selected_ids(self.current_player_id).len();
                self.runtime_host_last_gameplay_cmd = format!("select_similar_ok:{}", n);
            } else {
                self.runtime_host_last_gameplay_cmd = "select_similar_fail_no_seed".into();
            }
        }
    }

    pub(super) fn runtime_host_cmd_select_on_screen(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_on_screen_fail_not_ingame".into();
        } else {
            self.select_all_friendly_on_screen();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_on_screen_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_select_aircraft(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_aircraft_fail_not_ingame".into();
        } else {
            self.select_all_friendly_aircraft();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_aircraft_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_select_idle_harvesters(
        &mut self,
        _args: &HashMap<String, String>,
    ) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_idle_fail_not_ingame".into();
        } else {
            self.select_idle_harvesters();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_idle_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_select_structures(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_structures_fail_not_ingame".into();
        } else {
            self.select_all_friendly_structures();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_structures_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_select_moving(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_moving_fail_not_ingame".into();
        } else {
            self.select_all_friendly_moving();
            let n = self.selected_objects.len();
            self.runtime_host_last_gameplay_cmd = format!("select_moving_ok:{}", n);
        }
    }

    pub(super) fn runtime_host_cmd_camera_reset(&mut self, _args: &HashMap<String, String>) {
        if !matches!(
            self.current_state,
            GameState::InGame | GameState::Paused | GameState::Menu
        ) {
            self.runtime_host_last_gameplay_cmd = "camera_reset_fail_bad_state".into();
        } else {
            self.reset_camera_view_hotkey();
            self.runtime_host_last_gameplay_cmd = format!(
                "camera_reset_ok:{:.1},{:.1},{:.1}",
                self.camera_target.x, self.camera_target.y, self.camera_target.z
            );
        }
    }

    pub(super) fn runtime_host_cmd_camera_look_at(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "camera_look_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let target = self.clamp_to_world_bounds(glam::Vec3::new(x, y, z));
            // Use the same W3D-orbit rebuild as physical centering.  Writing
            // only `camera_position` here left the actual view matrix stale
            // and made the next input update snap the command back.
            self.host_player_look_at(target);
            self.runtime_host_last_gameplay_cmd = format!(
                "camera_look_ok:{:.1},{:.1},{:.1}",
                target.x, target.y, target.z
            );
        }
    }

    pub(super) fn runtime_host_cmd_camera_zoom(&mut self, args: &HashMap<String, String>) {
        if !matches!(
            self.current_state,
            GameState::InGame | GameState::Paused | GameState::Menu
        ) {
            self.runtime_host_last_gameplay_cmd = "camera_zoom_fail_bad_state".into();
        } else {
            let z: f32 = args
                .get("z")
                .or_else(|| args.get("zoom"))
                .or_else(|| args.get("level"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0_f32)
                .clamp(0.2_f32, 4.0_f32);
            self.camera_zoom = z;
            self.cancel_scripted_camera_from_player_set();
            self.apply_camera_orbit_transform();
            self.runtime_host_last_gameplay_cmd = format!("camera_zoom_ok:{:.3}", z);
        }
    }

    pub(super) fn runtime_host_cmd_camera_track(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "camera_track_fail_not_ingame".into();
        } else {
            self.toggle_camera_tracking_drawable_hotkey();
            // C++ CommandXlat.cpp:3216-3218 only enables; no OFF path.
            self.runtime_host_last_gameplay_cmd = "camera_track_ok:on".into();
        }
    }

    pub(super) fn runtime_host_cmd_cancel_production(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "cancel_production_fail_not_ingame".into();
        } else {
            // Wave 731: empty-selection auto-pick is opt-in only (default fail-closed).
            // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
            let allow_auto_target = args
                .get("auto_target")
                .or_else(|| args.get("pick_any"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                    let s = v.to_string_lossy();
                    !(s.is_empty()
                        || s == "0"
                        || s.eq_ignore_ascii_case("false")
                        || s.eq_ignore_ascii_case("no"))
                });
            // Prefer structure with production queue only when auto_target opted in.
            if self.selected_objects.is_empty() && allow_auto_target {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                            .alive_upgrade_producer_structure_ids(team)
                            .into_iter()
                            .next()
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        None
                    };
                    if let Some(id) = id {
                        self.host_set_selection(self.current_player_id, vec![id]);
                    }
                }
            }
            let all = args
                .get("all")
                .map(|s| matches!(s.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
                .unwrap_or(false);
            let ok = if all {
                self.cancel_all_selected_production()
            } else {
                self.cancel_selected_production_queue_head()
            };
            self.runtime_host_last_gameplay_cmd = if ok {
                format!("cancel_production_ok:{}", if all { "all" } else { "head" })
            } else if self.ui_selected_ids(self.current_player_id).is_empty() {
                // Wave 218: empty selection via presentation-first ui_selected_ids.
                "cancel_production_fail_no_selection".into()
            } else {
                // Empty queue is a valid residual — command path exercised.
                "cancel_production_ok:empty".into()
            };
        }
    }

    pub(super) fn runtime_host_cmd_open_diplomacy(&mut self, _args: &HashMap<String, String>) {
        // C++ in-match diplomacy is the Diplomacy.wnd overlay (Diplomacy.cpp
        // TheDiplomacy toggle), NOT a shell screen. Only Menu-state drives use
        // the shell push; mid-match must not strand the shell stack over the
        // live world (the old enter_shell_menu call wedged state=Menu).
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            #[cfg(feature = "game_client")]
            {
                let shown = game_client::gui::callbacks::toggle_diplomacy(false).is_ok();
                self.runtime_host_last_gameplay_cmd = if shown {
                    "diplomacy_ok_wnd".into()
                } else {
                    "diplomacy_fail_wnd".into()
                };
                return;
            }
        }
        #[allow(unreachable_code)]
        {
            self.enter_shell_menu_from_runtime_host(Some("Diplomacy"));
            self.runtime_host_last_gameplay_cmd = "diplomacy_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_auto_attack(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "auto_attack_fail_not_ingame".into();
        } else {
            let enable = match args
                .get("on")
                .or_else(|| args.get("enabled"))
                .map(|s| s.trim().to_ascii_lowercase())
                .as_deref()
            {
                Some("1") | Some("true") | Some("on") | Some("yes") => true,
                Some("0") | Some("false") | Some("off") | Some("no") => false,
                _ => !self.sticky_auto_attack,
            };
            self.sticky_auto_attack = enable;
            self.runtime_host_last_gameplay_cmd = if enable {
                "auto_attack_ok:on".into()
            } else {
                "auto_attack_ok:off".into()
            };
        }
    }

    pub(super) fn runtime_host_cmd_request_capture(&mut self, _args: &HashMap<String, String>) {
        self.runtime_host_pending_capture = true;
        self.runtime_host_last_gameplay_cmd = "request_capture_ok".into();
    }

    /// G3EnemyHunt temp diagnostic: on-demand env-gated roster dump for the
    /// DozerConstruct — the previous residual reported ok on a queued command
    /// that never created a building.
    fn construct_result_after_place(&self, template: &str, p: glam::Vec3, force: bool) -> String {
        let team = self.local_team_for_ui();
        let uc = self
            .game_logic
            .host_objects()
            .values()
            .filter(|o| {
                o.team == team
                    && o.is_alive()
                    && o.status.under_construction
                    && (o.template_name.eq_ignore_ascii_case(template)
                        || o.template_name.to_ascii_lowercase().contains("barracks"))
            })
            .count();
        if uc == 0 {
            return format!("construct_fail_no_building:{template}");
        }
        if force {
            format!("construct_ok_force:{template}@{},{}", p.x, p.z)
        } else {
            format!("construct_ok:{template}@{},{}", p.x, p.z)
        }
    }

    pub(super) fn runtime_host_cmd_construct(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "construct_fail_not_ingame".into();
        } else {
            self.runtime_host_last_gameplay_cmd = "construct_begin".into();
            // Wave 727: missing template is fail-closed (no free default structure).
            // Smoke always passes template=...; harness may set default_template=1
            // / GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1 for legacy bare commands.
            let allow_default_template = args
                .get("default_template")
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE").is_some_and(|v| {
                    let s = v.to_string_lossy();
                    !(s.is_empty()
                        || s == "0"
                        || s.eq_ignore_ascii_case("false")
                        || s.eq_ignore_ascii_case("no"))
                });
            let requested = match args
                .get("template")
                .cloned()
                .or_else(|| args.get("name").cloned())
            {
                Some(t) if !t.trim().is_empty() => t,
                _ if allow_default_template => "USA_Barracks".to_string(),
                _ => {
                    self.runtime_host_last_gameplay_cmd = "construct_fail_no_template".into();
                    return;
                }
            };
            // Wave 725: soft template alias fallbacks are opt-in only (default fail-closed).
            // Retail commands use the exact requested template name.
            // Smoke/harness may set alias_fallback=1 / GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1.
            let allow_alias_fallback = args
                .get("alias_fallback")
                .or_else(|| args.get("soft_alias"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK").is_some_and(|v| {
                    let s = v.to_string_lossy();
                    !(s.is_empty()
                        || s == "0"
                        || s.eq_ignore_ascii_case("false")
                        || s.eq_ignore_ascii_case("no"))
                });
            // Prefer exact requested; common barracks aliases only when opted in.
            let mut candidates = vec![requested.as_str()];
            if allow_alias_fallback {
                candidates.extend(["USA_Barracks", "AmericaBarracks", "Barracks"]);
            }
            // Wave 565: template residual prefers presentation freeze names.
            let template = candidates
                .iter()
                .find(|n| self.presentation_or_boot_has_template(**n))
                .map(|s| (*s).to_string())
                .unwrap_or(requested);
            // Wave 220: team via presentation-first local_team_for_ui.
            let team = Some(self.local_team_for_ui());
            let Some(team) = team else {
                self.runtime_host_last_gameplay_cmd = "construct_fail_no_player".into();
                return;
            };
            // Prefer selected worker; else first friendly dozer/worker.
            let mut builders: Vec<crate::game_logic::ObjectId> = self
                .selected_objects
                .iter()
                .copied()
                .filter(|id| {
                    if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.alive_construct_builder_ids(team).contains(id)
                    } else {
                        // Wave 217: presentation required for construct builder identity.
                        false
                    }
                })
                .collect();
            // Wave 729: auto-pick producer/builder when selection empty is opt-in only.
            // Default fail-closed: retail requires a real selection.
            // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
            let allow_auto_target = args
                .get("auto_target")
                .or_else(|| args.get("pick_any"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                    let s = v.to_string_lossy();
                    !(s.is_empty()
                        || s == "0"
                        || s.eq_ignore_ascii_case("false")
                        || s.eq_ignore_ascii_case("no"))
                });
            if builders.is_empty() && allow_auto_target {
                builders = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.alive_construct_builder_ids(team)
                } else {
                    // Presentation required (no live get_objects dual-read).
                    Vec::new()
                };
            }
            // Wave 719: free dozer spawn is opt-in only (default fail-closed).
            // Retail construct requires an existing builder. Vertical-slice smoke
            // may set spawn_dozer=1 / GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER=1.
            // Wave 227: remember spawn pose so construct location needs no live get_object.
            let mut spawned_builder_pose: Option<(crate::game_logic::ObjectId, glam::Vec3)> = None;
            let allow_spawn_dozer = args
                .get("spawn_dozer")
                .or_else(|| args.get("force_spawn_dozer"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER").is_some_and(
                    |v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    },
                );
            if builders.is_empty() && allow_spawn_dozer {
                let spawn_at = {
                    let cc = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.first_friendly_command_center_position(team)
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        None
                    };
                    cc.unwrap_or(glam::Vec3::new(100.0, 0.0, 100.0))
                        + glam::Vec3::new(25.0, 0.0, 0.0)
                };
                for name in ["USA_Dozer", "AmericaVehicleDozer", "GoldenDozer"] {
                    // Wave 565: freeze owns known names; host still sees live inserts.
                    // Wave 581: freeze OR live host insert residual.
                    if !self.presentation_or_live_has_template(name) {
                        continue;
                    }
                    if let Some(id) = self.host_create_object(name, team, spawn_at) {
                        builders.push(id);
                        spawned_builder_pose = Some((id, spawn_at));
                        break;
                    }
                }
            }
            let Some(builder) = builders.first().copied() else {
                // Wave 217: presentation required for construct builder identity.
                self.runtime_host_last_gameplay_cmd = "construct_fail_no_dozer".into();
                return;
            };
            self.host_set_selection(self.current_player_id, vec![builder]);

            // Location: explicit xyz, else near builder / local CC.
            let loc = if let (Some(x), Some(z)) = (
                args.get("x").and_then(|s| s.parse::<f32>().ok()),
                args.get("z").and_then(|s| s.parse::<f32>().ok()),
            ) {
                let y = args
                    .get("y")
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(0.0);
                glam::Vec3::new(x, y, z)
            } else {
                let base = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .first_friendly_command_center_position(team)
                        .or_else(|| {
                            frame
                                .objects
                                .iter()
                                .find(|o| o.id == builder && !o.destroyed)
                                .map(|o| o.position)
                        })
                } else {
                    // Presentation required (no live get_objects dual-read).
                    None
                }
                .or_else(|| {
                    // Wave 227: newly spawned dozer pose from spawn residual (no live get_object).
                    spawned_builder_pose
                        .filter(|(id, _)| *id == builder)
                        .map(|(_, pos)| pos)
                })
                .unwrap_or(glam::Vec3::ZERO);
                base + glam::Vec3::new(40.0, 0.0, 0.0)
            };

            // FOW residual: load_map + per-frame update_main_crate_vision already ran.
            let lbc =
                self.host_legal_build_code_at_for_builder(team, loc, &template, Some(builder));
            if lbc != 0 {
                let mut found = None;
                // Scan nearby pads (same residual as golden FOW recovery).
                'scan: for step in [15.0_f32, 25.0, 40.0] {
                    let extent = if step <= 15.0 {
                        8
                    } else if step <= 25.0 {
                        10
                    } else {
                        12
                    };
                    for dx in -extent..=extent {
                        for dz in -extent..=extent {
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            let p = loc + glam::Vec3::new(dx as f32 * step, 0.0, dz as f32 * step);
                            let code = self.host_legal_build_code_at_for_builder(
                                team,
                                p,
                                &template,
                                Some(builder),
                            );
                            
                            if code
                                == crate::game_logic::host_production_buildable_command_residual::LBC_OK
                            {
                                found = Some(p);
                                break 'scan;
                            }
                        }
                    }
                }
                
                if let Some(p) = found {
                    self.place_structure_from_ui(&template, p);
                    self.runtime_host_last_gameplay_cmd =
                        self.construct_result_after_place(&template, p, false);
                } else if args
                    .get("auto_target")
                    .or_else(|| args.get("force_place"))
                    .map(|v| {
                        let s = v.trim();
                        s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                    })
                    .unwrap_or(false)
                {
                    // Opt-in residual: place at builder-forward offset when LBC
                    // rejects the whole search (map clutter / footprint mismatch).
                    let p = loc + glam::Vec3::new(80.0, 0.0, 80.0);
                    self.place_structure_from_ui(&template, p);
                    self.runtime_host_last_gameplay_cmd =
                        self.construct_result_after_place(&template, p, true);
                } else {
                    self.runtime_host_last_gameplay_cmd = format!("construct_fail_lbc:{lbc}");
                }
            } else {
                self.place_structure_from_ui(&template, loc);
                self.runtime_host_last_gameplay_cmd =
                    self.construct_result_after_place(&template, loc, false);
            }
        }
    }
}
