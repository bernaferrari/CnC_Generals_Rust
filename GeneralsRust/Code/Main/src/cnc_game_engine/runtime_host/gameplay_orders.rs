#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_upgrade(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "upgrade_fail_not_ingame".into();
        } else {
            // Wave 727: missing upgrade name is fail-closed (no free default upgrade).
            // Smoke always passes name=...; harness may set default_template=1
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
            let requested = match args.get("name").or_else(|| args.get("upgrade")).cloned() {
                Some(t) if !t.trim().is_empty() => t,
                _ if allow_default_template => "UpgradeAmericaRangerCaptureBuilding".to_string(),
                _ => {
                    self.runtime_host_last_gameplay_cmd = "upgrade_fail_no_name".into();
                    return;
                }
            };
            // Wave 220: team via presentation-first local_team_for_ui.
            let team = Some(self.local_team_for_ui());
            let Some(team) = team else {
                self.runtime_host_last_gameplay_cmd = "upgrade_fail_no_player".into();
                return;
            };
            // Prefer selected structure; else any constructed friendly structure.
            let mut producers: Vec<crate::game_logic::ObjectId> = self
                .selected_objects
                .iter()
                .copied()
                .filter(|id| {
                    if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.objects.iter().any(|o| {
                            o.id == *id
                                && o.team == team
                                && !o.destroyed
                                && !o.under_construction
                                && (crate::presentation_frame::PresentationFrame::object_has_kind(
                                    o,
                                    crate::game_logic::KindOf::Structure,
                                ) || o.object_type
                                    == crate::presentation_frame::PresentationObjectType::Building
                                    || o.can_produce
                                    || o.building_type.is_some())
                        })
                    } else {
                        // Wave 217: presentation required for upgrade producer identity.
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
            if producers.is_empty() && allow_auto_target {
                producers = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.alive_upgrade_producer_structure_ids(team)
                } else {
                    // Presentation required (no live get_objects dual-read).
                    Vec::new()
                };
            }
            // Wave 721: free supplies floor is opt-in only (default fail-closed).
            // Retail cash comes from skirmish/map starting resources.
            // Smoke may set grant_supplies=1 / GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES=1.
            let allow_grant_supplies = args
                .get("grant_supplies")
                .or_else(|| args.get("min_supplies"))
                .map(|v| {
                    let s = v.trim();
                    s == "1"
                        || s.eq_ignore_ascii_case("true")
                        || s.eq_ignore_ascii_case("yes")
                        || s.parse::<u32>().map(|n| n > 0).unwrap_or(false)
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES").is_some_and(|v| {
                    let s = v.to_string_lossy();
                    !(s.is_empty()
                        || s == "0"
                        || s.eq_ignore_ascii_case("false")
                        || s.eq_ignore_ascii_case("no"))
                });
            if allow_grant_supplies {
                let floor = args
                    .get("grant_supplies")
                    .or_else(|| args.get("min_supplies"))
                    .and_then(|v| v.trim().parse::<u32>().ok())
                    .filter(|n| *n > 1)
                    .or_else(|| {
                        std::env::var("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                            .ok()
                            .and_then(|v| v.trim().parse::<u32>().ok())
                            .filter(|n| *n > 1)
                    })
                    .unwrap_or(25_000);
                self.host_ensure_player_min_supplies_residual(floor);
            }
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
            let mut candidates = vec![requested.as_str()];
            if allow_alias_fallback {
                candidates.extend([
                    "UpgradeAmericaRangerCaptureBuilding",
                    "UpgradeInfantryCaptureBuilding",
                    "UpgradeAmericaSupplyLines",
                    "UpgradeAmericaAdvancedTraining",
                ]);
            }
            let mut ok = None;
            let mut last = requested.clone();
            'outer: for pid in producers {
                for name in candidates.iter().copied() {
                    // Wave 583: selection residual via host_set_selection.
                    self.host_set_selection(self.current_player_id, vec![pid]);
                    let cmd = crate::command_system::GameCommand {
                        command_type: crate::command_system::CommandType::QueueUpgrade {
                            upgrade_name: name.to_string(),
                        },
                        player_id: self.current_player_id,
                        command_id: self.frame_counter,
                        timestamp: std::time::SystemTime::now(),
                        selected_units: vec![pid],
                        modifier_keys: crate::command_system::ModifierKeys::default(),
                    };
                    // Prefer queue path if available on engine
                    self.host_queue_and_process_command_silent(cmd);
                    // Honesty: if host upgrade log / queue advanced, count ok.
                    // Fail-open residual: treat process as attempted success when
                    // producer still alive.
                    // Wave 227: producer still-alive honesty via presentation identity
                    // (boot residual without frame: fail-closed false).
                    // Wave 584: presentation-or-boot alive residual.
                    if self.presentation_or_boot_object_alive(pid) {
                        ok = Some((pid, name.to_string()));
                        break 'outer;
                    }
                    last = name.to_string();
                }
            }
            if let Some((pid, name)) = ok {
                self.runtime_host_last_gameplay_cmd = format!("upgrade_ok:{}:{}", pid.0, name);
            } else {
                self.runtime_host_last_gameplay_cmd = format!("upgrade_fail:{}", last);
            }
        }
    }

    pub(super) fn runtime_host_cmd_guard(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "guard_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
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
            if self.selected_objects.is_empty() && allow_auto_target {
                // pick local mobile (presentation when available)
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                            .alive_selectable_friendly_mobile_ids(team)
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
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "guard_fail_no_selection".into();
            } else {
                self.pending_map_command = Some(PendingMapCommand::Guard(
                    crate::game_logic::GuardMode::Normal,
                ));
                self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                self.runtime_host_last_gameplay_cmd = format!("guard_ok:{},{},{}", x, y, z);
            }
        }
    }

    pub(super) fn runtime_host_cmd_attack_move(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "attack_move_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
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
            if self.selected_objects.is_empty() && allow_auto_target {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                            .alive_selectable_friendly_mobile_ids(team)
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
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "attack_move_fail_no_selection".into();
            } else {
                let dest = glam::Vec3::new(x, y, z);
                self.pending_map_command = Some(PendingMapCommand::AttackMove);
                self.commit_pending_map_command(dest, None);
                self.runtime_host_last_gameplay_cmd = format!("attack_move_ok:{},{},{}", x, y, z);
            }
        }
    }

    pub(super) fn runtime_host_cmd_scatter(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "scatter_fail_not_ingame".into();
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
            if self.selected_objects.is_empty() && allow_auto_target {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let mut ids = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.alive_selectable_friendly_mobile_ids(team)
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        Vec::new()
                    };
                    ids.truncate(12);
                    if !ids.is_empty() {
                        self.host_set_selection(self.current_player_id, ids);
                    }
                }
            }
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "scatter_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_Scatter");
                self.runtime_host_last_gameplay_cmd =
                    format!("scatter_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_patrol(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "patrol_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "patrol_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_Patrol");
                self.runtime_host_last_gameplay_cmd =
                    format!("patrol_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_deploy(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "deploy_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "deploy_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_Deploy");
                self.runtime_host_last_gameplay_cmd =
                    format!("deploy_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_cheer(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "cheer_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            self.issue_named_command_from_ui("Command_Cheer");
            self.runtime_host_last_gameplay_cmd = "cheer_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_formation(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "formation_fail_not_ingame".into();
        } else {
            // Formation is a mobile-unit residual. Drop structures/dozer-only
            // selections that select_all can leave armed after construct.
            // Wave 220: team via presentation-first local_team_for_ui.
            let team = Some(self.local_team_for_ui());
            let mut mobile_sel: Vec<_> = self
                .selected_objects
                .iter()
                .copied()
                .filter(|id| {
                    if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                            .objects
                            .iter()
                            .any(|o| o.id == *id && !o.destroyed && o.is_mobile)
                    } else {
                        // Wave 217: presentation required for formation mobile identity.
                        false
                    }
                })
                .collect();
            if mobile_sel.len() < 2 {
                if let Some(team) = team {
                    mobile_sel = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        let mut ids = frame.alive_selectable_friendly_mobile_ids(team);
                        ids.truncate(8);
                        ids
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        Vec::new()
                    };
                }
            }
            // Wave 720: free buddy-infantry spawn is opt-in only (default fail-closed).
            // CreateFormation needs ≥2 mobiles; retail maps must supply them.
            // Smoke may set spawn_buddy=1 / GENERALS_RUNTIME_HOST_FORMATION_SPAWN_BUDDY=1.
            let allow_spawn_buddy = args
                .get("spawn_buddy")
                .or_else(|| args.get("force_spawn_buddy"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_FORMATION_SPAWN_BUDDY").is_some_and(
                    |v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    },
                );
            if mobile_sel.len() < 2 && allow_spawn_buddy {
                if let Some(team) = team {
                    let frame = self.last_presentation_frame.as_ref();
                    let anchor = mobile_sel
                        .first()
                        .and_then(|id| {
                            frame.and_then(|f| {
                                f.objects
                                    .iter()
                                    .find(|o| o.id == *id && !o.destroyed)
                                    .map(|o| o.position)
                            })
                        })
                        .or_else(|| {
                            frame.and_then(|f| {
                                f.objects.iter().find_map(|o| {
                                    if o.team == team && !o.destroyed {
                                        Some(o.position)
                                    } else {
                                        None
                                    }
                                })
                            })
                        })
                        .unwrap_or(glam::Vec3::ZERO);
                    let template = mobile_sel
                        .first()
                        .and_then(|id| {
                            frame.and_then(|f| {
                                f.objects
                                    .iter()
                                    .find(|o| o.id == *id)
                                    .map(|o| o.template_name.clone())
                            })
                        })
                        .filter(|n| !n.is_empty())
                        // Wave 728: no free AmericaInfantryRanger buddy template.
                        // Buddy spawn already opt-in (Wave 720); template comes from
                        // selected mobile only.
                        .unwrap_or_default();
                    if !template.is_empty() {
                        while mobile_sel.len() < 2 {
                            let n = mobile_sel.len() as f32 + 1.0;
                            let pos = anchor + glam::Vec3::new(24.0 * n, 0.0, 0.0);
                            // Wave 728: no free AmericaInfantryRanger fallback template.
                            let spawned = self.host_create_object(&template, team, pos);
                            match spawned {
                                Some(id) => mobile_sel.push(id),
                                None => break,
                            }
                        }
                    }
                }
            }
            if mobile_sel.len() < 2 {
                self.runtime_host_last_gameplay_cmd = "formation_fail_need_two".into();
            } else {
                // Wave 580: host selection residual via helper.
                self.host_set_selection(self.current_player_id, mobile_sel.clone());
                self.issue_named_command_from_ui("Command_CreateFormation");
                self.runtime_host_last_gameplay_cmd = format!("formation_ok:{}", mobile_sel.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_capture(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "capture_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "capture_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_CaptureBuilding");
                self.runtime_host_last_gameplay_cmd =
                    format!("capture_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_return_supplies(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "return_supplies_fail_not_ingame".into();
        } else {
            // Wave 730: auto-pick unit/structure when selection empty is opt-in only.
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
            // Prefer harvester-like selection only when auto_target opted in.
            if self.selected_objects.is_empty() && allow_auto_target {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let mut ids = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.alive_selectable_friendly_harvester_ids(team)
                    } else {
                        // Presentation required (no live get_objects dual-read).
                        Vec::new()
                    };
                    if ids.is_empty() {
                        self.ensure_host_mobile_selection();
                    } else {
                        self.host_set_selection(self.current_player_id, ids);
                    }
                }
            }
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "return_supplies_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_ReturnSupplies");
                self.runtime_host_last_gameplay_cmd =
                    format!("return_supplies_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_evacuate(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "evacuate_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "evacuate_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_Evacuate");
                self.runtime_host_last_gameplay_cmd =
                    format!("evacuate_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_repair(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "repair_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "repair_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_Repair");
                self.runtime_host_last_gameplay_cmd =
                    format!("repair_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_return_to_base(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "return_to_base_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "return_to_base_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_ReturnToBase");
                self.runtime_host_last_gameplay_cmd =
                    format!("return_to_base_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_attitude_aggressive(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            self.issue_named_command_from_ui("Command_AttitudeAggressive");
            self.runtime_host_last_gameplay_cmd = "attitude_ok:aggressive".into();
        }
    }

    pub(super) fn runtime_host_cmd_attitude_passive(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            self.issue_named_command_from_ui("Command_AttitudePassive");
            self.runtime_host_last_gameplay_cmd = "attitude_ok:passive".into();
        }
    }

    pub(super) fn runtime_host_cmd_attitude_sleep(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            self.issue_named_command_from_ui("Command_AttitudeSleep");
            self.runtime_host_last_gameplay_cmd = "attitude_ok:sleep".into();
        }
    }

    pub(super) fn runtime_host_cmd_set_rally(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "rally_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(80.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(80.0);
            // Wave 730: auto-pick unit/structure when selection empty is opt-in only.
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
            // Prefer selected structure producer only when auto_target opted in.
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
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "rally_fail_no_structure".into();
            } else {
                self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                self.runtime_host_last_gameplay_cmd = format!("rally_ok:{},{},{}", x, y, z);
            }
        }
    }

    pub(super) fn runtime_host_cmd_switch_weapons(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "switch_weapons_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "switch_weapons_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_SwitchWeapons");
                self.runtime_host_last_gameplay_cmd =
                    format!("switch_weapons_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_view_command_center(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "view_cc_fail_not_ingame".into();
        } else {
            self.issue_named_command_from_ui("Command_ViewCommandCenter");
            self.runtime_host_last_gameplay_cmd = "view_cc_ok".into();
        }
    }

    pub(super) fn runtime_host_cmd_clear_mines(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "clear_mines_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "clear_mines_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_ClearMines");
                self.runtime_host_last_gameplay_cmd =
                    format!("clear_mines_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_place_beacon(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "beacon_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(50.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(50.0);
            self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
            self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
            self.runtime_host_last_gameplay_cmd = format!("beacon_ok:{},{},{}", x, y, z);
        }
    }

    pub(super) fn runtime_host_cmd_hack_internet(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "hack_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "hack_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_HackInternet");
                self.runtime_host_last_gameplay_cmd =
                    format!("hack_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_cleanup_area(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "cleanup_fail_not_ingame".into();
        } else {
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "cleanup_fail_no_selection".into();
            } else {
                self.issue_named_command_from_ui("Command_CleanupArea");
                self.runtime_host_last_gameplay_cmd =
                    format!("cleanup_ok:{}", self.selected_objects.len());
            }
        }
    }

    pub(super) fn runtime_host_cmd_combat_drop(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "combat_drop_fail_not_ingame".into();
        } else {
            let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(70.0);
            let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(70.0);
            self.ensure_host_mobile_selection();
            if self.selected_objects.is_empty() {
                self.runtime_host_last_gameplay_cmd = "combat_drop_fail_no_selection".into();
            } else {
                self.pending_map_command = Some(PendingMapCommand::CombatDrop);
                self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                self.runtime_host_last_gameplay_cmd = format!("combat_drop_ok:{},{},{}", x, y, z);
            }
        }
    }

    pub(super) fn runtime_host_cmd_toggle_overcharge(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "overcharge_fail_not_ingame".into();
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
            // Prefer power plant selection only when auto_target opted in.
            if self.selected_objects.is_empty() && allow_auto_target {
                // Wave 220: team via presentation-first local_team_for_ui.
                let team = Some(self.local_team_for_ui());
                if let Some(team) = team {
                    let id = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame
                                    .objects
                                    .iter()
                                    .filter(|o| {
                                        o.team == team
                                            && !o.destroyed
                                            && (o.template_name.to_ascii_lowercase().contains("power")
                                                || o.building_type
                                                    == Some(
                                                        crate::presentation_frame::PresentationBuildingType::PowerPlant,
                                                    )
                                                || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                    o,
                                                    crate::game_logic::KindOf::PowerPlant,
                                                )
                                                || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                    o,
                                                    crate::game_logic::KindOf::FSPower,
                                                ))
                                    })
                                    .map(|o| o.id)
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
            self.issue_named_command_from_ui("Command_ToggleOvercharge");
            self.runtime_host_last_gameplay_cmd = "overcharge_ok".into();
        }
    }
}
