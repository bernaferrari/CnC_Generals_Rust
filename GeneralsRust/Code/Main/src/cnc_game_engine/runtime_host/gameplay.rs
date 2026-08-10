#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_save_game(&mut self, args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "save_fail_not_ingame".into();
                } else {
                    let slot = args
                        .get("slot")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "quicksave".to_string());
                    let display = args
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| format!("HostSave-{slot}"));
                    self.save_game_from_ui(&slot, &display);
                    let exists = self.save_file_manager.save_exists(&slot);
                    self.runtime_host_last_gameplay_cmd = if exists {
                        format!("save_ok:{slot}")
                    } else {
                        format!("save_fail:{slot}")
                    };
                }
            }

    pub(super) fn runtime_host_cmd_quickload(&mut self, _args: &HashMap<String, String>)  {
                if !self.save_file_manager.save_exists("quicksave") {
                    self.runtime_host_last_gameplay_cmd = "load_fail_no_quicksave".into();
                } else {
                    self.set_runtime_host_ui_screen_override(None);
                    // Host residual: report real load Result (do not claim ok on deserialize fail).
                    match self.load_game_from_ui("quicksave") {
                        Ok(()) => {
                            if !matches!(self.current_state, GameState::InGame | GameState::Paused)
                            {
                                self.request_state_change(GameState::InGame);
                            }
                            self.runtime_host_last_gameplay_cmd = "load_ok:quicksave".into();
                        }
                        Err(err) => {
                            warn!("quickload failed: {err}");
                            self.runtime_host_last_gameplay_cmd =
                                format!("load_fail:quicksave:{err}");
                        }
                    }
                }
            }

    pub(super) fn runtime_host_cmd_load_game(&mut self, args: &HashMap<String, String>)  {
                let slot = args.get("slot").map(|slot| slot.trim()).unwrap_or_default();
                if !slot.is_empty() {
                    self.set_runtime_host_ui_screen_override(None);
                    match self.load_game_from_ui(slot) {
                        Ok(()) => {
                            self.runtime_host_last_gameplay_cmd = format!("load_ok:{slot}");
                            if matches!(self.ui_manager.current_screen(), Some(Screen::GameHUD)) {
                                self.request_state_change(GameState::InGame);
                            }
                        }
                        Err(err) => {
                            warn!("load_game failed for '{slot}': {err}");
                            self.runtime_host_last_gameplay_cmd = format!("load_fail:{slot}:{err}");
                        }
                    }
                }
            }

    pub(super) fn runtime_host_cmd_replay(&mut self, args: &HashMap<String, String>)  {
                let slot = args
                    .get("slot")
                    .cloned()
                    .unwrap_or_else(|| "latest".to_string());
                warn!(
                    "Runtime host replay command requested for slot '{slot}', replay startup path is not wired yet"
                );
                self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
            }

    pub(super) fn runtime_host_cmd_enqueue_production(&mut self, args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "train_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "train_begin".into();
                    // Wave 727: missing template is fail-closed (no free default unit).
                    // Smoke always passes template=...; harness may set default_template=1
                    // / GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1 for legacy bare commands.
                    let allow_default_template =
                        args.get("default_template")
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    let requested = match args.get("template").cloned() {
                        Some(t) if !t.trim().is_empty() => t,
                        _ if allow_default_template => "AmericaInfantryRanger".to_string(),
                        _ => {
                            self.runtime_host_last_gameplay_cmd = "train_fail_no_template".into();
                            return;
                        }
                    };
                    // Prefer presentation local team residual (no live player roster).
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_player".into();
                        return;
                    };
                    // Wave 718: force-complete unfinished barracks is opt-in only.
                    // Default fail-closed: train against already-constructed producers
                    // (honest retail timing). Enable with force_complete=1 arg or
                    // GENERALS_RUNTIME_HOST_TRAIN_FORCE_COMPLETE=1 for vertical-slice smoke.
                    let allow_force_complete = args
                        .get("force_complete")
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_TRAIN_FORCE_COMPLETE")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    let mut force_completed: Vec<crate::game_logic::ObjectId> = Vec::new();
                    if allow_force_complete {
                        let mut unfinished: Vec<crate::game_logic::ObjectId> = if let Some(frame) =
                            self.last_presentation_frame.as_ref()
                        {
                            frame
                                .objects
                                .iter()
                                .filter(|o| {
                                    o.team == team
                                        && !o.destroyed
                                        && o.under_construction
                                        && (o.building_type
                                            == Some(
                                                crate::presentation_frame::PresentationBuildingType::Barracks,
                                            )
                                            || o.template_name
                                                .to_ascii_lowercase()
                                                .contains("barracks")
                                            || o.can_produce
                                            || o.building_type.is_some()
                                            || crate::presentation_frame::PresentationFrame::object_has_kind(
                                                o,
                                                crate::game_logic::KindOf::FSBarracks,
                                            ))
                                })
                                .map(|o| o.id)
                                .collect()
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            Vec::new()
                        };
                        unfinished.sort_by_key(|id| id.0);
                        for id in unfinished.into_iter().take(2) {
                            // Wave 224: authority mutation via GameLogic API (no engine get_object_mut).
                            if self.host_force_complete_construction(id) {
                                force_completed.push(id);
                            }
                        }
                    }
                    // Wave 729: auto-pick first constructed producer is opt-in only.
                    // Default fail-closed: train needs an explicit producer selection path.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_AUTO_TARGET=1.
                    let allow_auto_target = args
                        .get("auto_target")
                        .or_else(|| args.get("pick_any"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                    // Prefer force-completed + presentation constructed producers.
                    // Live full dual-scan is boot residual or fail-open when presentation
                    // still marks force-completed barracks under_construction.
                    let producer = {
                        let mut barracks: Vec<crate::game_logic::ObjectId> = Vec::new();
                        let mut any: Vec<crate::game_logic::ObjectId> = Vec::new();
                        let push = |is_barracks: bool,
                                    id: crate::game_logic::ObjectId,
                                    barracks: &mut Vec<crate::game_logic::ObjectId>,
                                    any: &mut Vec<crate::game_logic::ObjectId>| {
                            if is_barracks {
                                if !barracks.contains(&id) {
                                    barracks.push(id);
                                }
                            } else if !any.contains(&id) {
                                any.push(id);
                            }
                        };
                        // Wave 214: force-completed IDs classified from presentation freeze only
                        // (no live GameLogic dual-read residual).
                        for id in force_completed.iter().copied() {
                            let classified = if let Some(frame) =
                                self.last_presentation_frame.as_ref()
                            {
                                frame.objects.iter().find_map(|o| {
                                    if o.id != id || o.destroyed || o.team != team {
                                        return None;
                                    }
                                    if o.under_construction {
                                        return None;
                                    }
                                    let is_barracks = o.building_type
                                        == Some(
                                            crate::presentation_frame::PresentationBuildingType::Barracks,
                                        )
                                        || o.template_name
                                            .to_ascii_lowercase()
                                            .contains("barracks")
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSBarracks,
                                        );
                                    let is_producer = o.can_produce
                                        || is_barracks
                                        || matches!(
                                            o.building_type,
                                            Some(
                                                crate::presentation_frame::PresentationBuildingType::WarFactory
                                                    | crate::presentation_frame::PresentationBuildingType::Airfield
                                                    | crate::presentation_frame::PresentationBuildingType::Barracks
                                            )
                                        )
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSWarFactory,
                                        )
                                        || crate::presentation_frame::PresentationFrame::object_has_kind(
                                            o,
                                            crate::game_logic::KindOf::FSAirfield,
                                        );
                                    if !is_producer {
                                        return None;
                                    }
                                    Some((is_barracks, id))
                                })
                            } else {
                                None
                            };
                            if let Some((is_b, id)) = classified {
                                push(is_b, id, &mut barracks, &mut any);
                            }
                        }
                        if let Some(frame) = self.last_presentation_frame.as_ref() {
                            for o in &frame.objects {
                                if o.team != team || o.destroyed {
                                    continue;
                                }
                                if o.under_construction && !force_completed.contains(&o.id) {
                                    continue;
                                }
                                let is_barracks = o.building_type
                                    == Some(
                                        crate::presentation_frame::PresentationBuildingType::Barracks,
                                    )
                                    || o.template_name.to_ascii_lowercase().contains("barracks")
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSBarracks,
                                    );
                                let is_producer = o.can_produce
                                    || is_barracks
                                    || matches!(
                                        o.building_type,
                                        Some(
                                            crate::presentation_frame::PresentationBuildingType::WarFactory
                                                | crate::presentation_frame::PresentationBuildingType::Airfield
                                                | crate::presentation_frame::PresentationBuildingType::Barracks
                                        )
                                    )
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSWarFactory,
                                    )
                                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                                        o,
                                        crate::game_logic::KindOf::FSAirfield,
                                    );
                                if !is_producer {
                                    continue;
                                }
                                push(is_barracks, o.id, &mut barracks, &mut any);
                            }
                        } else {
                            // Presentation required (no live get_objects dual-read).
                        }
                        // Wave 834/848: when auto_target + force_complete are opt-in and the
                        // presentation freeze still lacks the just-built barracks (construct
                        // → train same control drain), fall back to host-stamped producer
                        // residuals (refreshed if cold). Default path stays presentation-only.
                        if allow_auto_target && barracks.is_empty() && any.is_empty() {
                            // Ensure residuals are warm (single stamp dual-read if needed).
                            if self.host_match_local_barracks_ids.is_none()
                                && self.host_match_local_producer_ids.is_none()
                            {
                                self.host_refresh_local_train_producer_residuals();
                            }
                            // Force-complete unfinished local producers from residual.
                            if allow_force_complete {
                                let unfinished = self
                                    .host_match_local_unfinished_producer_ids
                                    .clone()
                                    .unwrap_or_default();
                                for id in unfinished.into_iter().take(4) {
                                    if self.host_force_complete_construction(id) {
                                        force_completed.push(id);
                                    }
                                }
                                if !force_completed.is_empty() {
                                    self.host_refresh_local_train_producer_residuals();
                                }
                            }
                            for id in self
                                .host_match_local_barracks_ids
                                .clone()
                                .unwrap_or_default()
                            {
                                if !barracks.contains(&id) {
                                    barracks.push(id);
                                }
                            }
                            for id in self
                                .host_match_local_producer_ids
                                .clone()
                                .unwrap_or_default()
                            {
                                if !any.contains(&id) {
                                    any.push(id);
                                }
                            }
                            // Wave 834/848: if still no local barracks, spawn + complete one at a
                            // team sample position so auto_target train can enqueue honestly.
                            if barracks.is_empty() && allow_force_complete {
                                let spawn_at = self
                                    .last_presentation_frame
                                    .as_ref()
                                    .and_then(|f| {
                                        f.objects.iter().find_map(|o| {
                                            (o.team == team && !o.destroyed).then_some(o.position)
                                        })
                                    })
                                    .or_else(|| {
                                        self.host_match_local_team_sample_pos
                                            .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                                    })
                                    .unwrap_or(glam::Vec3::new(500.0, 0.0, 500.0))
                                    + glam::Vec3::new(120.0, 0.0, 40.0);
                                for bname in ["AmericaBarracks", "USA_Barracks", "AmericaBarracks"]
                                {
                                    if let Some(id) = self.host_create_object(bname, team, spawn_at)
                                    {
                                        let _ = self.host_force_complete_construction(id);
                                        let _ = self.host_ensure_barracks_building_data(id);
                                        barracks.push(id);
                                        self.host_refresh_local_train_producer_residuals();
                                        break;
                                    }
                                }
                            }
                        }

                        barracks.sort_by_key(|id| id.0);
                        any.sort_by_key(|id| id.0);
                        let pick = barracks
                            .into_iter()
                            .next()
                            .or_else(|| any.into_iter().next());
                        if let Some(id) = pick {
                            // Wave 723: stamping Barracks building_data is opt-in only.
                            // Default fail-closed: retail producers already carry building_data
                            // after honest construction complete. Force-complete / smoke may set
                            // ensure_barracks=1 or reuse force_complete=1 /
                            // GENERALS_RUNTIME_HOST_ENSURE_BARRACKS=1.
                            let allow_ensure_barracks = allow_force_complete
                                || args
                                    .get("ensure_barracks")
                                    .map(|v| {
                                        let s = v.trim();
                                        s == "1"
                                            || s.eq_ignore_ascii_case("true")
                                            || s.eq_ignore_ascii_case("yes")
                                    })
                                    .unwrap_or(false)
                                || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_BARRACKS")
                                    .is_some_and(|v| {
                                        let s = v.to_string_lossy();
                                        !(s.is_empty()
                                            || s == "0"
                                            || s.eq_ignore_ascii_case("false")
                                            || s.eq_ignore_ascii_case("no"))
                                    });
                            if allow_ensure_barracks {
                                // Wave 224: authority mutation via GameLogic API (no engine get_object_mut).
                                let stamped = self.host_ensure_barracks_building_data(id);
                                // Wave 834: auto_target residual force-stamps when name/kind
                                // gate misses host-spawned producers.
                                if !stamped && allow_auto_target {
                                    let _ = self.host_force_ensure_barracks_building_data(id);
                                }
                            } else if allow_auto_target && allow_force_complete {
                                let _ = self.host_force_ensure_barracks_building_data(id);
                            }
                        }
                        if allow_auto_target {
                            pick.or_else(|| {
                                self.last_presentation_frame
                                    .as_ref()
                                    .and_then(|f| f.first_constructed_producer_id(team))
                            })
                        } else {
                            pick
                        }
                    };
                    // Wave 725: soft template alias fallbacks are opt-in only (default fail-closed).
                    // Retail commands use the exact requested template name.
                    // Smoke/harness may set alias_fallback=1 / GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1.
                    let allow_alias_fallback =
                        args.get("alias_fallback")
                            .or_else(|| args.get("soft_alias"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                    // Wave 722: synthetic GoldenRanger template insert is opt-in only.
                    // Default fail-closed: train only against real retail/map templates.
                    // Smoke/harness may set golden_template=1 /
                    // GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER=1.
                    let allow_golden_template = args
                        .get("golden_template")
                        .or_else(|| args.get("ensure_golden_ranger"))
                        .map(|v| {
                            let s = v.trim();
                            s == "1"
                                || s.eq_ignore_ascii_case("true")
                                || s.eq_ignore_ascii_case("yes")
                        })
                        .unwrap_or(false)
                        || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER")
                            .is_some_and(|v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            });
                    let mut unit_candidates = vec![requested.as_str()];
                    if allow_alias_fallback {
                        unit_candidates.extend([
                            "AmericaInfantryRanger",
                            "USA_Ranger",
                            "USARanger",
                        ]);
                    }
                    if allow_golden_template {
                        unit_candidates.push("GoldenRanger");
                    }
                    // Wave 563: template residual prefers presentation freeze names.
                    let template = unit_candidates
                        .iter()
                        .find(|n| self.presentation_or_boot_has_template(**n))
                        .map(|s| (*s).to_string())
                        .unwrap_or(requested);
                    if let Some(pid) = producer {
                        // Wave 722: only insert GoldenRanger host template when opted in.
                        if allow_golden_template {
                            self.host_ensure_golden_ranger_template();
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
                            || std::env::var_os("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
                                .is_some_and(|v| {
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
                        // Wave 724/725: alias + GoldenRanger enqueue fallbacks are opt-in only.
                        let mut try_names = vec![template.as_str()];
                        if allow_alias_fallback {
                            try_names.extend(["AmericaInfantryRanger", "USA_Ranger", "USARanger"]);
                        }
                        if allow_golden_template {
                            try_names.push("GoldenRanger");
                        }
                        let mut ok_name = None;
                        let mut last_fail = template.clone();
                        for name in try_names {
                            // Wave 563: freeze owns known names; host still sees mid-command inserts.
                            // Wave 581: freeze OR live host insert residual.
                            if !self.presentation_or_live_has_template(name) {
                                continue;
                            }
                            if self.host_enqueue_production(pid, name.to_string()) {
                                ok_name = Some(name.to_string());
                                break;
                            }
                            last_fail = name.to_string();
                        }
                        if let Some(name) = ok_name {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_ok:{}:{}", pid.0, name);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_fail_enqueue:{}:prod={}", last_fail, pid.0);
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_producer".into();
                    }
                }
            }

    pub(super) fn runtime_host_cmd_select_local_unit(&mut self, _args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_fail_not_ingame".into();
                } else {
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let pick = team.and_then(|team| {
                        if let Some(frame) = self.last_presentation_frame.as_ref() {
                            // Presentation-owned mobile identity (no live dual-scan).
                            frame.first_mobile_friendly_id(team).or_else(|| {
                                frame
                                    .alive_selectable_friendly_mobile_ids(team)
                                    .into_iter()
                                    .next()
                            })
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        }
                    });
                    if let Some(id) = pick {
                        self.host_set_selection(self.current_player_id, vec![id]);
                        self.runtime_host_last_gameplay_cmd = format!("select_ok:{}", id.0);
                    } else {
                        self.runtime_host_last_gameplay_cmd = "select_fail_no_mobile".into();
                    }
                }
            }

    pub(super) fn runtime_host_cmd_move(&mut self, args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "move_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    // Wave 218: selection count via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id).len();
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "move_fail_no_selection".into();
                    } else {
                        // Wave 221: push presentation-first selection into host before move.
                        let ids = self.ui_selected_ids(self.current_player_id);
                        if !ids.is_empty() {
                            self.host_set_selection(self.current_player_id, ids);
                        }
                        self.host_command_move(self.current_player_id, glam::Vec3::new(x, y, z));
                        self.runtime_host_last_gameplay_cmd =
                            format!("move_ok:n={selected}:x={x:.1}:y={y:.1}:z={z:.1}");
                    }
                }
            }

    pub(super) fn runtime_host_cmd_attack_nearest_enemy(&mut self, _args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attack_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "attack_begin".into();
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    // Prefer combat-capable mobiles (select_all often arms structures/dozers).
                    if let Some(team) = team {
                        let mut attackers: Vec<_> = self
                            .selected_objects
                            .iter()
                            .copied()
                            .filter(|id| {
                                self.last_presentation_frame.as_ref().is_some_and(|frame| {
                                    frame.objects.iter().any(|o| {
                                        o.id == *id
                                            && o.team == team
                                            && !o.destroyed
                                            && o.has_weapon
                                    })
                                })
                            })
                            .collect();
                        if attackers.is_empty() {
                            attackers = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                let mut ids = frame.alive_selectable_friendly_combat_ids(team);
                                ids.truncate(8);
                                ids
                            } else {
                                // Presentation required (no live get_objects dual-read).
                                Vec::new()
                            };
                        }
                        if !attackers.is_empty() {
                            self.host_set_selection(self.current_player_id, attackers.clone());
                        }
                    }
                    // Wave 221: selection count via presentation-first ui_selected_ids.
                    let selected = self.ui_selected_ids(self.current_player_id).len();
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_selection".into();
                    } else if let Some(team) = team {
                        // Wave 1115: prefer FOW-clear attackable enemy (parity
                        // is_enemy_attackable), then force-attack residual fallback.
                        let enemy = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.first_enemy_attack_command_id(team)
                        } else {
                            // Presentation required (no live get_objects dual-read).
                            None
                        };
                        if let Some(tid) = enemy {
                            self.host_command_attack(self.current_player_id, tid);
                            self.runtime_host_last_gameplay_cmd = format!("attack_ok:{}", tid.0);
                        } else {
                            self.runtime_host_last_gameplay_cmd = "attack_fail_no_enemy".into();
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_player".into();
                    }
                }
            }

    pub(super) fn runtime_host_cmd_stop_all(&mut self, _args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "stop_fail_not_ingame".into();
                } else {
                    let n = self.selected_objects.len();
                    if n > 0 {
                        // Stop only selection when present.
                        self.host_command_stop(self.current_player_id);
                        self.runtime_host_last_gameplay_cmd = format!("stop_ok:selected:{n}");
                    } else {
                        self.stop_all_friendly_units();
                        self.runtime_host_last_gameplay_cmd = "stop_ok:all".into();
                    }
                }
            }

    pub(super) fn runtime_host_cmd_sell(&mut self, args: &HashMap<String, String>)  {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "sell_fail_not_ingame".into();
                } else {
                    // Wave 220: team via presentation-first local_team_for_ui.
                    let team = Some(self.local_team_for_ui());
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_player".into();
                        return;
                    };
                    // Wave 217: presentation required for sell identity (no live get_object).
                    let mut targets: Vec<crate::game_logic::ObjectId> =
                        crate::game_logic::presentation_selected_sellable_structure_ids(
                            self.last_presentation_frame.as_ref(),
                            &self.selected_objects,
                            team,
                        );
                    // Wave 728: auto-pick newest sellable structure is opt-in only.
                    // Default fail-closed: sell requires a selected structure.
                    // Smoke may set auto_target=1 / GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET=1.
                    if targets.is_empty() {
                        let allow_auto_target = args
                            .get("auto_target")
                            .or_else(|| args.get("pick_structure"))
                            .map(|v| {
                                let s = v.trim();
                                s == "1"
                                    || s.eq_ignore_ascii_case("true")
                                    || s.eq_ignore_ascii_case("yes")
                            })
                            .unwrap_or(false)
                            || std::env::var_os("GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET")
                                .is_some_and(|v| {
                                    let s = v.to_string_lossy();
                                    !(s.is_empty()
                                        || s == "0"
                                        || s.eq_ignore_ascii_case("false")
                                        || s.eq_ignore_ascii_case("no"))
                                });
                        if allow_auto_target {
                            targets = if let Some(frame) = self.last_presentation_frame.as_ref() {
                                // Newest id first (mirrors host reverse sort residual).
                                frame
                                    .alive_sellable_friendly_structure_ids(team)
                                    .into_iter()
                                    .rev()
                                    .take(1)
                                    .collect()
                            } else {
                                Vec::new()
                            };
                            // Wave 856: when freeze has no sellable structure (kind/selectability
                            // residual lag after construct), fall back to host-stamped local
                            // barracks/producer residuals (no live get_objects dual-read).
                            if targets.is_empty() {
                                if self.host_match_local_barracks_ids.is_none()
                                    && self.host_match_local_producer_ids.is_none()
                                {
                                    self.host_refresh_local_train_producer_residuals();
                                }
                                let mut candidates = self
                                    .host_match_local_barracks_ids
                                    .clone()
                                    .unwrap_or_default();
                                candidates.extend(
                                    self.host_match_local_producer_ids
                                        .clone()
                                        .unwrap_or_default(),
                                );
                                // Prefer newest residual producer (non-CC path already filtered).
                                candidates.sort_by_key(|id| id.0);
                                if let Some(id) = candidates.into_iter().rev().next() {
                                    targets.push(id);
                                }
                            }
                        }
                    }
                    if targets.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_structure".into();
                    } else {
                        // Wave 583: selection residual via host_set_selection.
                        self.host_set_selection(self.current_player_id, targets.clone());
                        self.issue_named_command_from_ui("Command_Sell");
                        self.runtime_host_last_gameplay_cmd = format!("sell_ok:{}", targets[0].0);
                    }
                }
            }

}
