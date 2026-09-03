#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    /// Publish Main's real Rust snapshot inventory to the retail
    /// `PopupSaveLoad.wnd` callback.
    ///
    /// The popup itself stays responsible for its WND/listbox/confirmation
    /// sequence.  Main is the sole owner of the live `GameLogic` snapshot, so
    /// it must be the source of rows as well as the consumer of a confirmed
    /// Save/Load request.  In particular, an empty Main inventory is not a
    /// reason to fall back to Common's separate `TheGameState` persistence.
    #[cfg(feature = "game_client")]
    fn host_publish_popup_save_load_inventory(&mut self) {
        use game_client::gui::callbacks::{
            PopupSaveLoadEntry, publish_host_popup_save_load_entries,
        };

        let entries = match self.save_file_manager.list_saves() {
            Ok(saves) => saves
                .into_iter()
                .map(|save| {
                    let description = if save.save_info.description.trim().is_empty() {
                        save.save_info.display_name
                    } else {
                        save.save_info.description
                    };
                    let (display_time, display_date) =
                        crate::save_load::format_save_list_date_time(save.save_info.save_date);
                    PopupSaveLoadEntry {
                        filename: save.filename,
                        description,
                        display_time,
                        display_date,
                        is_mission: save.save_info.save_type == SaveFileType::Mission,
                    }
                })
                .collect(),
            Err(err) => {
                // Fail closed: Common may happen to share the directory, but
                // it is not the authority capable of restoring Main's world.
                warn!("Unable to list Main save games for PopupSaveLoad: {err}");
                Vec::new()
            }
        };
        publish_host_popup_save_load_entries(entries);
    }

    /// C++ `GameState::findNextSaveFilename` LOWEST_NUMBER `%08d.sav`.
    #[cfg(feature = "game_client")]
    fn next_popup_save_slot(&self) -> String {
        for index in 0..=99_999_999 {
            let slot = format!("{index:08}");
            if !self.save_file_manager.save_exists(&slot) {
                return slot;
            }
        }
        "00000000".to_string()
    }

    /// Complete confirmed retail PopupSaveLoad actions against Main's one
    /// snapshot authority.  It runs after UI callbacks have unwound, which
    /// avoids retaining a WindowManager/RefCell borrow across serialization or
    /// world teardown.
    #[cfg(feature = "game_client")]
    pub(crate) fn host_tick_popup_save_load_bridge(&mut self) {
        use game_client::gui::callbacks::{
            PopupSaveLoadRequest, take_host_popup_save_load_published_requests,
        };

        if !self.popup_save_load_bridge_initialized {
            self.host_publish_popup_save_load_inventory();
            self.popup_save_load_bridge_initialized = true;
        }

        for published in take_host_popup_save_load_published_requests() {
            let physical_os_input = published.input_provenance.is_physical_window_mouse_input();
            match published.request {
                PopupSaveLoadRequest::Save {
                    filename,
                    description,
                    save_file_type,
                } => {
                    // Snapshot eligibility before serialization.  The save
                    // authority itself remains unchanged; this is only a
                    // fail-closed proof attached after it succeeds.
                    let physical_evidence_eligible =
                        self.host_popup_save_load_evidence_eligible(physical_os_input);
                    // A selected existing row carries its exact filename.  A
                    // `New Save Game` pseudo-row deliberately carries none,
                    // so use a runtime-provided acceptance slot if present or
                    // allocate one without overwriting an unselected row.
                    let slot = if filename.trim().is_empty() {
                        self.pending_popup_save_slot
                            .take()
                            .filter(|slot| !slot.trim().is_empty())
                            .unwrap_or_else(|| self.next_popup_save_slot())
                    } else {
                        filename.trim().to_string()
                    };
                    let display_name = self
                        .pending_popup_save_display_name
                        .take()
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| {
                            if !description.trim().is_empty() {
                                description.clone()
                            } else if matches!(save_file_type, ::game_engine::SaveFileType::Mission)
                            {
                                crate::save_load::current_mission_save_description()
                            } else {
                                crate::save_load::default_save_edit_description(
                                    &self.presentation_or_boot_map_name(),
                                )
                            }
                        });
                    let description = if description.trim().is_empty() {
                        if matches!(save_file_type, ::game_engine::SaveFileType::Mission) {
                            crate::save_load::current_mission_save_description()
                        } else if !display_name.trim().is_empty() {
                            display_name.clone()
                        } else {
                            crate::save_load::default_save_edit_description(
                                &self.presentation_or_boot_map_name(),
                            )
                        }
                    } else {
                        description
                    };
                    let save_type = match save_file_type {
                        ::game_engine::SaveFileType::Normal => SaveFileType::Normal,
                        ::game_engine::SaveFileType::Mission => SaveFileType::Mission,
                    };
                    let save_info =
                        self.build_save_info(&slot, &display_name, &description, save_type);

                    match self.host_save_game_authority(&slot, &save_info) {
                        Ok(()) => {
                            self.runtime_host_last_gameplay_cmd =
                                format!("save_ok:wnd_popup:{slot}");
                            self.interactive_playability
                                .note_popup_save_confirmation_succeeded(physical_evidence_eligible);
                            self.host_publish_popup_save_load_inventory();
                        }
                        Err(err) => {
                            warn!("PopupSaveLoad save failed for '{slot}': {err}");
                            self.runtime_host_last_gameplay_cmd =
                                format!("save_fail:wnd_popup:{slot}:{err}");
                        }
                    }
                }
                PopupSaveLoadRequest::Load { filename } => {
                    // The post-load authority clears/rebuilds match residuals,
                    // so prove that the physical confirmation occurred in the
                    // visible offline match before starting restoration.
                    let physical_evidence_eligible =
                        self.host_popup_save_load_evidence_eligible(physical_os_input);
                    let slot = filename.trim();
                    if slot.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "load_fail_wnd_no_slot".into();
                        continue;
                    }
                    self.set_runtime_host_ui_screen_override(None);
                    match self.load_game_from_ui(slot) {
                        Ok(()) => {
                            if !matches!(self.current_state, GameState::InGame | GameState::Paused)
                            {
                                self.request_state_change(GameState::InGame);
                            }
                            self.runtime_host_last_gameplay_cmd =
                                format!("load_ok:wnd_popup:{slot}");
                            self.interactive_playability
                                .note_popup_load_confirmation_succeeded(physical_evidence_eligible);
                            self.host_publish_popup_save_load_inventory();
                        }
                        Err(err) => {
                            warn!("PopupSaveLoad load failed for '{slot}': {err}");
                            self.runtime_host_last_gameplay_cmd =
                                format!("load_fail:wnd_popup:{slot}:{err}");
                        }
                    }
                }
            }
        }
    }

    /// A PopupSaveLoad proof is meaningful only for an explicit physical WND
    /// confirmation in a visible offline match.  Runtime-host commands,
    /// control-file injection, headless use, menu loads, and online modes all
    /// fail closed before the sticky evidence can advance.
    #[cfg(feature = "game_client")]
    fn host_popup_save_load_evidence_eligible(&self, physical_os_input: bool) -> bool {
        physical_os_input
            && !self.runtime_host_headless
            && self.runtime_host_window_visible()
            && matches!(self.current_state, GameState::InGame | GameState::Paused)
            && matches!(
                self.host_match_game_mode,
                Some(
                    crate::game_logic::GameMode::SinglePlayer
                        | crate::game_logic::GameMode::Skirmish
                )
            )
    }

    pub(super) fn runtime_host_cmd_save_game(&mut self, args: &HashMap<String, String>) {
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

    pub(super) fn runtime_host_cmd_quickload(&mut self, _args: &HashMap<String, String>) {
        if !self.save_file_manager.save_exists("quicksave") {
            self.runtime_host_last_gameplay_cmd = "load_fail_no_quicksave".into();
        } else {
            self.set_runtime_host_ui_screen_override(None);
            // Host residual: report real load Result (do not claim ok on deserialize fail).
            match self.load_game_from_ui("quicksave") {
                Ok(()) => {
                    if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                        self.request_state_change(GameState::InGame);
                    }
                    self.runtime_host_last_gameplay_cmd = "load_ok:quicksave".into();
                }
                Err(err) => {
                    warn!("quickload failed: {err}");
                    self.runtime_host_last_gameplay_cmd = format!("load_fail:quicksave:{err}");
                }
            }
        }
    }

    /// Windowed sit-through step 8: Pause/QuitMenu → PopupSaveLoad.wnd Save.
    ///
    /// Does **not** fake a pass when the WND gadget/layout is missing. Headless
    /// smoke keeps `quicksave`; this command drives the live Pause Save/Load
    /// controls through their real confirmation path.  The confirmed callback
    /// is then drained by Main's popup bridge into `SaveFileManager`.
    pub(super) fn runtime_host_cmd_pause_save(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "save_fail_not_ingame".into();
            return;
        }
        let slot = args
            .get("slot")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "wnd_pause".to_string());
        let display = args
            .get("name")
            .cloned()
            .unwrap_or_else(|| format!("PauseSave-{slot}"));
        if matches!(self.current_state, GameState::InGame) {
            self.toggle_pause();
        }
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::callbacks::{
                drive_os_wnd_popup_save_load_save_and_confirm_like_cpp,
                drive_os_wnd_quit_menu_save_load_like_cpp, ensure_live_popup_save_load_layout,
                ensure_live_quit_menu_layout,
            };
            // Publish Main's actual snapshots before PopupSaveLoad initializes;
            // an installed but empty inventory intentionally contains only its
            // retail New Save pseudo-row.
            self.host_publish_popup_save_load_inventory();
            self.popup_save_load_bridge_initialized = true;
            self.pending_popup_save_slot = Some(slot.clone());
            self.pending_popup_save_display_name = Some(display.clone());

            // C++ pause parity: the human pause SHOWS QuitMenu.wnd before
            // its Save/Load button can be clicked (ToggleQuitMenu owns the
            // visible layout; GameState pause alone never reveals it). The
            // click dispatch below fail-closes on a hidden gadget, so show
            // the live quit layout first (idempotent: only when hidden).
            if !game_client::gui::callbacks::is_quit_menu_visible() {
                game_client::gui::callbacks::toggle_quit_menu();
            }
            let quit_layout = ensure_live_quit_menu_layout();
            let quit_save_load = quit_layout && drive_os_wnd_quit_menu_save_load_like_cpp();
            let bind = ensure_live_popup_save_load_layout();
            let save_confirm = drive_os_wnd_popup_save_load_save_and_confirm_like_cpp();
            wnd_ok = quit_save_load && bind && save_confirm;
        }
        if !wnd_ok {
            self.pending_popup_save_slot = None;
            self.pending_popup_save_display_name = None;
            // Layout, gadget, or confirmation rejected — fail closed rather
            // than treating a ButtonSave click as a completed snapshot.
            self.runtime_host_last_gameplay_cmd = "save_fail_wnd_missing".into();
            return;
        }
        self.runtime_host_last_gameplay_cmd = format!("save_pending_wnd_popup:{slot}");
    }

    /// Windowed sit-through step 8: Pause/QuitMenu → PopupSaveLoad.wnd Load.
    pub(super) fn runtime_host_cmd_pause_load(&mut self, args: &HashMap<String, String>) {
        let slot = args
            .get("slot")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "wnd_pause".to_string());
        if matches!(self.current_state, GameState::InGame) {
            self.toggle_pause();
        }
        if !self.save_file_manager.save_exists(&slot) {
            self.runtime_host_last_gameplay_cmd = format!("load_fail_wnd_no_slot:{slot}");
            return;
        }
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::callbacks::{
                drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp,
                drive_os_wnd_quit_menu_save_load_like_cpp, ensure_live_popup_save_load_layout,
                ensure_live_quit_menu_layout,
            };
            self.host_publish_popup_save_load_inventory();
            self.popup_save_load_bridge_initialized = true;
            // C++ pause parity (same as pause_save): show QuitMenu.wnd so
            // the SaveLoad button dispatch hits a visible gadget.
            if !game_client::gui::callbacks::is_quit_menu_visible() {
                game_client::gui::callbacks::toggle_quit_menu();
            }
            let quit_layout = ensure_live_quit_menu_layout();
            let quit_save_load = quit_layout && drive_os_wnd_quit_menu_save_load_like_cpp();
            let bind = ensure_live_popup_save_load_layout();
            let load_confirm = drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp(&slot);
            // A named selection, real Load click, and real confirmation are
            // mandatory.  Preparing a layout is deliberately not success.
            wnd_ok = quit_save_load && bind && load_confirm;
        }
        if !wnd_ok {
            self.runtime_host_last_gameplay_cmd = "load_fail_wnd_missing".into();
            return;
        }
        self.runtime_host_last_gameplay_cmd = format!("load_pending_wnd_popup:{slot}");
    }

    pub(super) fn runtime_host_cmd_load_game(&mut self, args: &HashMap<String, String>) {
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

    pub(super) fn runtime_host_cmd_replay(&mut self, args: &HashMap<String, String>) {
        let slot = args
            .get("slot")
            .cloned()
            .unwrap_or_else(|| "latest".to_string());
        warn!(
            "Runtime host replay command requested for slot '{slot}', replay startup path is not wired yet"
        );
        self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
    }

    pub(super) fn runtime_host_cmd_enqueue_production(&mut self, args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "train_fail_not_ingame".into();
        } else {
            self.runtime_host_last_gameplay_cmd = "train_begin".into();
            // Wave 727: missing template is fail-closed (no free default unit).
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
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_TRAIN_FORCE_COMPLETE").is_some_and(
                    |v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    },
                );
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
                    let classified = if let Some(frame) = self.last_presentation_frame.as_ref() {
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
                            == Some(crate::presentation_frame::PresentationBuildingType::Barracks)
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
                        for bname in ["AmericaBarracks", "USA_Barracks", "AmericaBarracks"] {
                            if let Some(id) = self.host_create_object(bname, team, spawn_at) {
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
                // Infantry (Ranger) must train at a completed barracks — never the
                // Command Center fallback that produced train_fail_enqueue:prod=CC.
                let infantry_unit = {
                    let n = requested.to_ascii_lowercase();
                    n.contains("ranger") || n.contains("infantry")
                };
                let pick = if infantry_unit {
                    barracks.into_iter().next()
                } else {
                    barracks
                        .into_iter()
                        .next()
                        .or_else(|| any.into_iter().next())
                };
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
                        || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_BARRACKS").is_some_and(
                            |v| {
                                let s = v.to_string_lossy();
                                !(s.is_empty()
                                    || s == "0"
                                    || s.eq_ignore_ascii_case("false")
                                    || s.eq_ignore_ascii_case("no"))
                            },
                        );
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
                if allow_auto_target && !infantry_unit {
                    pick.or_else(|| {
                        self.last_presentation_frame
                            .as_ref()
                            .and_then(|f| f.first_constructed_producer_id(team))
                    })
                } else if infantry_unit && pick.is_none() && allow_auto_target {
                    // Presentation freeze can omit a just-built barracks.
                    // Prefer GameWorld construction when mapped — HashMap UC/percent
                    // can lag writeback mid-frame.
                    let candidates: Vec<_> = self
                        .game_logic
                        .host_objects()
                        .values()
                        .filter(|o| o.team == team && o.is_alive() && !o.status.destroyed)
                        .filter(|o| {
                            o.template_name.to_ascii_lowercase().contains("barracks")
                                || o.is_kind_of(crate::game_logic::KindOf::FSBarracks)
                        })
                        .map(|o| (o.id, o.construction_percent, o.status.under_construction))
                        .collect();
                    candidates.into_iter().find_map(|(id, hpct, huc)| {
                        let (pct, uc) = self
                            .game_logic
                            .host_authoritative_construction(id)
                            .unwrap_or((hpct, huc));
                        // Finished (or stuck at 100% with UC still set) is trainable.
                        if !uc || pct + 1e-6 >= 1.0 {
                            Some(id)
                        } else {
                            None
                        }
                    })
                } else {
                    pick
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
            // Wave 722: synthetic GoldenRanger template insert is opt-in only.
            // Default fail-closed: train only against real retail/map templates.
            // Smoke/harness may set golden_template=1 /
            // GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER=1.
            let allow_golden_template = args
                .get("golden_template")
                .or_else(|| args.get("ensure_golden_ranger"))
                .map(|v| {
                    let s = v.trim();
                    s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                })
                .unwrap_or(false)
                || std::env::var_os("GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER").is_some_and(
                    |v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    },
                );
            let mut unit_candidates = vec![requested.as_str()];
            if allow_alias_fallback {
                unit_candidates.extend(["AmericaInfantryRanger", "USA_Ranger", "USARanger"]);
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
                // Percent==1 with UC still set is a stuck host complete
                // (unmapped sole-tick hole). Flush via the same ready-log
                // helper the writeback path uses — not force_complete cheat.
                if let Some((pct, uc)) = self.game_logic.host_authoritative_construction(pid) {
                    if uc && pct + 1e-6 >= 1.0 {
                        crate::game_logic::host_construction_ready_log::record(pid, 1.0);
                        self.game_logic.apply_post_writeback_complete_op(
                            crate::game_logic::PostWritebackCompleteOp::ConstructionCompletionsAfterReadyWriteback,
                        );
                    } else if uc && pct + 1e-6 < 1.0 {
                        // Presentation freeze can show a barracks as done while
                        // host/GW is still building. Fail-closed so smoke retries
                        // (train_fail_no_ready_barracks) instead of enqueue-fail.
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_ready_barracks".into();
                        return;
                    }
                }
                let host_ready = self
                    .game_logic
                    .host_object(pid)
                    .is_some_and(|o| o.is_constructed() && o.building_data.is_some());
                if !host_ready {
                    self.runtime_host_last_gameplay_cmd = "train_fail_no_ready_barracks".into();
                    return;
                }
                // Wave 722: only insert GoldenRanger host template when opted in.
                if allow_golden_template {
                    self.host_ensure_golden_ranger_template();
                }
                // Wave 721: free supplies floor is opt-in only (default fail-closed).
                // Retail cash comes from skirmish/map starting resources.
                // Smoke may set grant_supplies=1 / GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES=1.
                let allow_grant_supplies =
                    args.get("grant_supplies")
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
                let mut try_names = vec![template.clone()];
                if allow_alias_fallback {
                    for n in ["AmericaInfantryRanger", "USA_Ranger", "USARanger"] {
                        if !try_names.iter().any(|s| s == n) {
                            try_names.push(n.to_string());
                        }
                    }
                }
                if allow_golden_template {
                    try_names.push("GoldenRanger".into());
                }
                // Freeze/residual name lists can contain AmericaInfantryRanger
                // while GameLogic.templates only has the INI-loaded key
                // (case / USA_ alias). Resolve to a real host template.
                if let Some(resolved) = self.host_resolve_unit_template(&template) {
                    if !try_names.iter().any(|s| s == &resolved) {
                        try_names.insert(0, resolved);
                    }
                }
                let mut ok_name = None;
                let mut last_fail = template.clone();
                for name in &try_names {
                    if !self.presentation_or_live_has_template(name)
                        && !self.game_logic.templates.contains_key(name)
                    {
                        continue;
                    }
                    if self.host_enqueue_production(pid, name.clone()) {
                        ok_name = Some(name.clone());
                        break;
                    }
                    last_fail = name.clone();
                }
                if let Some(name) = ok_name {
                    self.runtime_host_last_gameplay_cmd = format!("train_ok:{}:{}", pid.0, name);
                } else {
                    let can = self.game_logic.can_make_unit(pid, &last_fail);
                    self.runtime_host_last_gameplay_cmd = format!(
                        "train_fail_enqueue:{}:prod={}:can={}",
                        last_fail, pid.0, can
                    );
                }
            } else if template.to_ascii_lowercase().contains("ranger")
                || template.to_ascii_lowercase().contains("infantry")
            {
                self.runtime_host_last_gameplay_cmd = "train_fail_no_ready_barracks".into();
            } else {
                self.runtime_host_last_gameplay_cmd = "train_fail_no_producer".into();
            }
        }
    }

    /// Map a freeze/residual unit name onto a key that exists in
    /// `GameLogic.templates` (enqueue reads that map, not the freeze list).
    fn host_resolve_unit_template(&self, requested: &str) -> Option<String> {
        if self.game_logic.templates.contains_key(requested) {
            return Some(requested.to_string());
        }
        if let Some(k) = self
            .game_logic
            .templates
            .keys()
            .find(|k| k.eq_ignore_ascii_case(requested))
        {
            return Some(k.clone());
        }
        let want = requested.to_ascii_lowercase();
        if !(want.contains("ranger") || want.contains("infantry")) {
            return None;
        }
        self.game_logic
            .templates
            .keys()
            .find(|k| {
                let n = k.to_ascii_lowercase();
                n.contains("ranger")
            })
            .cloned()
            .or_else(|| {
                self.game_logic
                    .templates
                    .keys()
                    .find(|k| {
                        let n = k.to_ascii_lowercase();
                        n.contains("infantry") && (n.contains("america") || n.contains("usa"))
                    })
                    .cloned()
            })
    }

    pub(super) fn runtime_host_cmd_select_local_unit(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "select_fail_not_ingame".into();
        } else {
            // Prefer presentation-frame local team (same residual as status
            // local_mobile_units / sample_unit_pos). Falling back only to the UI
            // team residual can desync and leave select_fail_no_mobile while
            // local_mobile_units=1 — which blocks RMB interactive_gameplay.
            let pick = if let Some(frame) = self.last_presentation_frame.as_ref() {
                let frame_team = frame.local_team();
                let ui_team = self.local_team_for_ui();
                frame
                    .first_mobile_friendly_id(frame_team)
                    .or_else(|| {
                        frame
                            .alive_selectable_friendly_mobile_ids(frame_team)
                            .into_iter()
                            .next()
                    })
                    .or_else(|| {
                        if ui_team != frame_team {
                            frame.first_mobile_friendly_id(ui_team).or_else(|| {
                                frame
                                    .alive_selectable_friendly_mobile_ids(ui_team)
                                    .into_iter()
                                    .next()
                            })
                        } else {
                            None
                        }
                    })
            } else {
                // Presentation required (no live get_objects dual-read).
                None
            };
            if let Some(id) = pick {
                self.host_set_selection(self.current_player_id, vec![id]);
                self.runtime_host_last_gameplay_cmd = format!("select_ok:{}", id.0);
            } else {
                self.runtime_host_last_gameplay_cmd = "select_fail_no_mobile".into();
            }
        }
    }

    pub(super) fn runtime_host_cmd_move(&mut self, args: &HashMap<String, String>) {
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

    pub(super) fn runtime_host_cmd_attack_nearest_enemy(
        &mut self,
        _args: &HashMap<String, String>,
    ) {
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
                                o.id == *id && o.team == team && !o.destroyed && o.has_weapon
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

    pub(super) fn runtime_host_cmd_stop_all(&mut self, _args: &HashMap<String, String>) {
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

    pub(super) fn runtime_host_cmd_sell(&mut self, args: &HashMap<String, String>) {
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
                        s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
                    })
                    .unwrap_or(false)
                    || std::env::var_os("GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET").is_some_and(
                        |v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        },
                    );
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
