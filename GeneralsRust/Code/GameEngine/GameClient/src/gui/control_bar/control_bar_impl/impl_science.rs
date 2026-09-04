// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.


/// ShowControlBar can request DEFAULT before the host `ControlBar` tick
/// (C++ ControlBarCallback.cpp:489). Applied on the live instance.
static PENDING_CONTROL_BAR_STAGE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0xFF);

fn control_bar_stage_to_u8(stage: ControlBarStage) -> u8 {
    match stage {
        ControlBarStage::Default => 0,
        ControlBarStage::Low => 1,
        ControlBarStage::Hidden => 2,
    }
}

fn control_bar_stage_from_u8(value: u8) -> Option<ControlBarStage> {
    match value {
        0 => Some(ControlBarStage::Default),
        1 => Some(ControlBarStage::Low),
        2 => Some(ControlBarStage::Hidden),
        _ => None,
    }
}

fn take_pending_control_bar_stage() -> Option<ControlBarStage> {
    control_bar_stage_from_u8(
        PENDING_CONTROL_BAR_STAGE.swap(0xFF, std::sync::atomic::Ordering::SeqCst),
    )
}

/// C++ `setDefaultControlBarConfig` (ControlBar.cpp:3001-3014) does
/// `m_contextParent[CP_MASTER]->winHide(FALSE)`. CommandWindow is authored
/// `STATUS = ENABLED+HIDDEN+SEE_THRU` (ControlBar.wnd); C++ then shows it and
/// the 14 `ButtonCommand` children from `switchToContext(CB_CONTEXT_COMMAND)`
/// (ControlBar.cpp:2177-2188) + `populateCommand` (ControlBarCommand.cpp:317).
/// Live InGame DEFAULT is that dozer/command-set context, so clear the bits.
fn reveal_ingame_command_window() {
    with_window_manager(|manager| {
        let set_hidden = |name: &str, hidden: bool| {
            if let Some(win) = manager.find_window_by_name(name) {
                let _ = win.borrow_mut().hide(hidden);
            }
        };
        set_hidden("ControlBar.wnd:ControlBarParent", false);
        set_hidden("ControlBar.wnd:CommandWindow", false);
        set_hidden("ControlBar.wnd:UnderConstructionWindow", true);
        set_hidden("ControlBar.wnd:OCLTimerWindow", true);
        set_hidden("ControlBar.wnd:ObserverPlayerListWindow", true);
        set_hidden("ControlBar.wnd:BeaconWindow", true);
        for i in 1..=14 {
            set_hidden(&format!("ControlBar.wnd:ButtonCommand{:02}", i), false);
        }
        // C++ setDefaultControlBarConfig: view 80% height, parent at default
        // WND pos. Authored 800x600 y=480 is offscreen on 640x480. Pin the
        // parent to the bottom 20% of the live display.
        if let Some(parent) = manager.find_window_by_name("ControlBar.wnd:ControlBarParent") {
            let (_sw, sh) = manager.screen_size();
            let mut win = parent.borrow_mut();
            let (_w, h) = win.get_size();
            let bar_h = if h >= 32 { h.min(sh / 4).max(64) } else { (sh as f32 * 0.20) as i32 };
            let y = (sh - bar_h).max(0);
            let _ = win.set_position(0, y);
        }
    });
}

fn leftover_find_window(name: &str) -> Option<std::rc::Rc<std::cell::RefCell<GameWindow>>> {
    with_window_manager(|manager| manager.find_window_by_name(name))
}

fn leftover_hide_window(name: &str, hidden: bool) {
    if let Some(win) = leftover_find_window(name) {
        let _ = win.borrow_mut().hide(hidden);
    }
}

fn leftover_enable_window(name: &str, enabled: bool) {
    if let Some(win) = leftover_find_window(name) {
        let _ = win.borrow_mut().enable(enabled);
    }
}

fn leftover_window_is_hidden(name: &str) -> bool {
    leftover_find_window(name)
        .map(|win| win.borrow().is_hidden())
        .unwrap_or(true)
}

fn leftover_mapped_image(name: &str) -> Option<crate::gui::game_window::Image> {
    let collection = crate::display::image::get_mapped_image_collection();
    let guard = collection.try_read()?;
    let found = guard.find_image_by_name(name)?;
    let size = found.get_image_size();
    Some(crate::gui::game_window::Image {
        name: name.to_string(),
        width: size.x,
        height: size.y,
    })
}

fn leftover_set_enabled_image(window_name: &str, image_name: &str) {
    let Some(image) = leftover_mapped_image(image_name) else {
        return;
    };
    if let Some(win) = leftover_find_window(window_name) {
        let _ = win.borrow_mut().set_enabled_image(0, image);
    }
}

fn leftover_set_progress(window_name: &str, progress: i32) {
    if let Some(win) = leftover_find_window(window_name) {
        let mut win = win.borrow_mut();
        if let Some(bar) = win.progress_bar_mut() {
            bar.set_progress(progress.clamp(0, 100) as f32);
        }
    }
}

fn leftover_set_button_text(window_name: &str, text: &str) {
    if let Some(win) = leftover_find_window(window_name) {
        let _ = win.borrow_mut().set_text(text);
    }
}

fn leftover_animate_windows_enabled() -> bool {
    game_engine::common::ini::ini_game_data::get_global_data()
        .map(|data| data.read().animate_windows)
        .unwrap_or(true)
}

static LAST_SCIENCE_TRANSITION_GROUP: std::sync::Mutex<Option<&'static str>> =
    std::sync::Mutex::new(None);

fn leftover_transition_group(group: &'static str) {
    // C++ ControlBar.cpp:2929-2930 gates GenExpFade on m_animateWindows.
    // ControlBarArrow (1658) only needs TheTransitionHandler + input enabled.
    if group == "GenExpFade" && !leftover_animate_windows_enabled() {
        return;
    }
    with_window_manager(|manager| manager.transition_set_group(group, false));
    if group == "GenExpFade" {
        if let Ok(mut slot) = LAST_SCIENCE_TRANSITION_GROUP.lock() {
            *slot = Some(group);
        }
    }
}

fn leftover_ensure_named_window(name: &str) {
    if leftover_find_window(name).is_some() {
        return;
    }
    with_window_manager(|manager| {
        if let Ok(win) = manager.create_window(None, 0, 0, 200, 200) {
            win.borrow_mut().set_name(name);
            let _ = win.borrow_mut().hide(true);
        }
    });
}

fn leftover_take_last_science_transition_group() -> Option<&'static str> {
    LAST_SCIENCE_TRANSITION_GROUP
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

fn leftover_display_size() -> (i32, i32) {
    with_window_manager(|manager| manager.screen_size())
}

fn leftover_set_tactical_view_height(height: i32) {
    let (_sw, sh) = leftover_display_size();
    let frac = if sh > 0 {
        (height as f32 / sh as f32).clamp(0.05, 1.0)
    } else {
        1.0
    };
    crate::display::view::set_tactical_view_height_frac(frac);
    crate::display::view::with_tactical_view(|view| {
        view.set_height(height);
    });
}

const GEN_EXP_PARENT: &str = "GeneralsExpPoints.wnd:GenExpParent";
const GEN_EXP_PROGRESS: &str = "GeneralsExpPoints.wnd:ProgressBarExperience";
const WIN_U_ATTACK: &str = "ControlBar.wnd:WinUAttack";
const BUTTON_GENERAL: &str = "ControlBar.wnd:ButtonGeneral";
const BUTTON_LARGE: &str = "ControlBar.wnd:ButtonLarge";
const CONTROL_BAR_PARENT: &str = "ControlBar.wnd:ControlBarParent";
const GEN_EXP_POINTS_AVAILABLE: &str = "GeneralsExpPoints.wnd:StaticTextRankPointsAvailable";
const GEN_EXP_TITLE: &str = "GeneralsExpPoints.wnd:StaticTextTitle";

fn leftover_hide_all_purchase_science_windows() {
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_1 {
        leftover_hide_window(&format!("GeneralsExpPoints.wnd:ButtonRank1Number{i}"), true);
    }
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_3 {
        leftover_hide_window(&format!("GeneralsExpPoints.wnd:ButtonRank3Number{i}"), true);
    }
    for i in 0..MAX_PURCHASE_SCIENCE_RANK_8 {
        leftover_hide_window(&format!("GeneralsExpPoints.wnd:ButtonRank8Number{i}"), true);
    }
}

/// C++ ControlBar.cpp:1118-1141 — bind rank cameo windows and USE_OVERLAY_STATES.
fn leftover_bind_purchase_science_windows_init() {
    leftover_ensure_named_window(GEN_EXP_POINTS_AVAILABLE);
    leftover_ensure_named_window(GEN_EXP_TITLE);
    let bind = |prefix: &str, count: usize| {
        for i in 0..count {
            let name = format!("GeneralsExpPoints.wnd:{prefix}{i}");
            leftover_ensure_named_window(&name);
            if let Some(win) = leftover_find_window(&name) {
                win.borrow_mut()
                    .set_status(crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES);
            }
        }
    };
    bind("ButtonRank1Number", MAX_PURCHASE_SCIENCE_RANK_1);
    bind("ButtonRank3Number", MAX_PURCHASE_SCIENCE_RANK_3);
    bind("ButtonRank8Number", MAX_PURCHASE_SCIENCE_RANK_8);
}




/// C++ ControlBarCommand.cpp:1219-1261 — COMMAND_RESTRICTED unless ScienceVec owned.
pub fn command_button_science_vec_owned(
    crate_player_has: impl Fn(ScienceType) -> bool,
    host_unlocked_names: &[String],
    required_ids: &[ScienceType],
    required_names: &[String],
) -> bool {
    if !required_ids.is_empty() {
        let ids_ok = required_ids
            .iter()
            .all(|&st| st == SCIENCE_INVALID || crate_player_has(st));
        if ids_ok {
            return true;
        }
    }
    if !required_names.is_empty() {
        return required_names.iter().all(|req| {
            let want = req.trim();
            if want.is_empty() {
                return true;
            }
            host_unlocked_names
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(want))
        });
    }
    required_ids.is_empty()
}





impl ControlBar {
    // ---------------------------------------------------------------------------
    // Science purchase system
    // C++ ControlBar.cpp:143-485, 2907-2966
    // ---------------------------------------------------------------------------

    fn ensure_generals_exp_layout(&mut self) {
        if leftover_find_window(GEN_EXP_PARENT).is_none() {
            // C++ ControlBar.cpp:1058-1062 winCreateLayout("GeneralsExpPoints.wnd")
            // then bind GenExpParent as CP_PURCHASE_SCIENCE. Retail .wnd may be
            // absent in this tree — fail-closed stub windows still receive hide.
            with_window_manager(|manager| {
                let _ = manager.create_layout_with_windows("GeneralsExpPoints.wnd");
            });
            leftover_ensure_named_window(GEN_EXP_PARENT);
            leftover_ensure_named_window(GEN_EXP_PROGRESS);
            leftover_ensure_named_window("GeneralsExpPoints.wnd:ButtonExit");
            leftover_ensure_named_window(BUTTON_GENERAL);
            leftover_hide_window(GEN_EXP_PARENT, true);
            leftover_hide_window("GeneralsExpPoints.wnd", true);
        }
        leftover_bind_purchase_science_windows_init();
        self.science_layout_loaded = leftover_find_window(GEN_EXP_PARENT).is_some();
    }

    /// Resolve a CommandButton by name for leftover setControlCommand / click.
    fn leftover_lookup_command_button(command_name: &str) -> Option<CommandButton> {
        Self::leftover_command_button_by_name(command_name)
    }

    fn leftover_apply_purchase_science_windows(&self) {
        leftover_hide_all_purchase_science_windows();
        let apply_rank = |prefix: &str, buttons: &[ScienceButtonState]| {
            for (i, button) in buttons.iter().enumerate() {
                let name = format!("GeneralsExpPoints.wnd:{prefix}{i}");
                leftover_hide_window(&name, button.is_hidden || button.command_name.is_empty());
                leftover_enable_window(&name, button.is_enabled && !button.is_purchased);
                if let Some(win) = leftover_find_window(&name) {
                    {
                        let mut win = win.borrow_mut();
                        win.set_status(crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES);
                        if button.is_purchased {
                            win.set_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                        } else {
                            win.clear_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                        }
                    }
                    if !button.command_name.is_empty() {
                        let cmd = Self::leftover_command_button_by_name(&button.command_name)
                            .unwrap_or_else(|| {
                                let mut fallback = CommandButton::default();
                                fallback.command_name = button.command_name.clone();
                                fallback.command_type = CommandType::PurchaseScience;
                                fallback
                            });
                        self.set_control_command(&win, &cmd);
                    }

                }
            }
        };
        apply_rank("ButtonRank1Number", &self.science_state.rank1_buttons);
        apply_rank("ButtonRank3Number", &self.science_state.rank3_buttons);
        apply_rank("ButtonRank8Number", &self.science_state.rank8_buttons);

        leftover_set_button_text(
            GEN_EXP_POINTS_AVAILABLE,
            &self.science_state.available_points.to_string(),
        );
        let title_key = format!("SCIENCE:Rank{}", self.science_state.rank_level);
        leftover_set_button_text(GEN_EXP_TITLE, &GameText::fetch(&title_key));
        leftover_set_progress(
            GEN_EXP_PROGRESS,
            (self.science_state.experience_progress * 100.0) as i32,
        );
    }


    pub fn show_purchase_science(&mut self) {
        if gamelogic::helpers::TheScriptEngine::is_game_ending() {
            return;
        }
        self.ensure_generals_exp_layout();
        self.populate_purchase_science();
        self.gen_star_flash = false;
        self.leftover_apply_purchase_science_windows();
        self.science_state.is_visible = true;
        if leftover_find_window(GEN_EXP_PARENT).is_some() {
            if !leftover_window_is_hidden(GEN_EXP_PARENT) {
                return;
            }
            leftover_hide_window(GEN_EXP_PARENT, false);
            leftover_hide_window("GeneralsExpPoints.wnd", false);
        }
        leftover_transition_group("GenExpFade");
    }

    pub fn hide_purchase_science(&mut self) {
        if leftover_find_window(GEN_EXP_PARENT).is_some() {
            if leftover_window_is_hidden(GEN_EXP_PARENT) {
                self.science_state.is_visible = false;
                return;
            }
            leftover_hide_window(GEN_EXP_PARENT, true);
        }
        leftover_hide_window("GeneralsExpPoints.wnd", true);
        self.science_state.is_visible = false;
    }

    pub fn toggle_purchase_science(&mut self) {
        let hidden = leftover_find_window(GEN_EXP_PARENT)
            .map(|_| leftover_window_is_hidden(GEN_EXP_PARENT))
            .unwrap_or(!self.science_state.is_visible);
        if hidden {
            self.show_purchase_science();
        } else {
            self.hide_purchase_science();
        }
    }

    /// C++ ControlBarCallback.cpp:396-399 — ButtonGeneral (and CommandMap
    /// Alt+G via TheControlBar::toggle_purchase_science) opens/closes the
    /// GeneralsExpPoints window.
    pub fn on_generals_button(&mut self) {
        self.toggle_purchase_science();
    }

    fn populate_purchase_science(&mut self) {
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };

        let Some(store) = get_science_store() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
        self.science_state.rank_level = player.get_rank_level();
        self.science_state.experience_progress = 0.0;
        self.science_state.rank_title_label = format!("SCIENCE:Rank{}", player.get_rank_level());

        self.science_state.rank1_buttons.clear();
        self.science_state.rank3_buttons.clear();
        self.science_state.rank8_buttons.clear();

        let Some(template) = player.get_player_template() else {
            return;
        };
        let name1 = template.get_purchase_science_command_set_rank1();
        let name3 = template.get_purchase_science_command_set_rank3();
        let name8 = template.get_purchase_science_command_set_rank8();
        if name1.is_empty() || name3.is_empty() || name8.is_empty() {
            return;
        }
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let find_set = |name: &str| {
            control_bar
                .find_command_set_by_name(name)
                .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()))
        };
        let (Some(set1), Some(set3), Some(set8)) =
            (find_set(name1), find_set(name3), find_set(name8))
        else {
            return;
        };

        self.science_state.rank1_buttons = Self::science_buttons_from_set(
            &player,
            &store,
            &set1,
            MAX_PURCHASE_SCIENCE_RANK_1,
        );
        self.science_state.rank3_buttons = Self::science_buttons_from_set(
            &player,
            &store,
            &set3,
            MAX_PURCHASE_SCIENCE_RANK_3,
        );
        self.science_state.rank8_buttons = Self::science_buttons_from_set(
            &player,
            &store,
            &set8,
            MAX_PURCHASE_SCIENCE_RANK_8,
        );

        self.update_context_purchase_science();
    }

    fn science_buttons_from_set(
        player: &gamelogic::player::Player,
        store: &game_engine::common::rts::ScienceStore,
        command_set: &gamelogic::command_button::CommandSet,
        limit: usize,
    ) -> Vec<ScienceButtonState> {
        let mut buttons = Vec::new();
        for slot in 0..limit {
            let Some(button) = command_set.buttons.get(slot).and_then(|b| b.as_ref()) else {
                buttons.push(ScienceButtonState {
                    command_name: String::new(),
                    science_type: SCIENCE_INVALID,
                    is_hidden: true,
                    is_enabled: false,
                    is_purchased: false,
                });
                continue;
            };
            if (button.get_options_bits() & CommandOption::ScriptOnly as u32) != 0 {
                buttons.push(ScienceButtonState {
                    command_name: String::new(),
                    science_type: SCIENCE_INVALID,
                    is_hidden: true,
                    is_enabled: false,
                    is_purchased: false,
                });
                continue;
            }
            let science = button
                .science_vec()
                .first()
                .copied()
                .unwrap_or(SCIENCE_INVALID);
            let mut is_hidden = false;
            let mut is_enabled = false;
            let mut is_purchased = false;
            if science != SCIENCE_INVALID {
                if player.is_science_disabled(science) {
                    is_enabled = false;
                } else if player.is_science_hidden(science) {
                    is_hidden = true;
                } else {
                    let cost = store.get_science_purchase_cost(science);
                    if !player.has_science(science)
                        && store.player_has_prereqs_for_science(player, science)
                        && cost <= player.get_science_purchase_points()
                    {
                        is_enabled = true;
                    }
                    is_purchased = player.has_science(science);
                    if !store.player_has_root_prereqs_for_science(player, science) {
                        is_hidden = true;
                    }
                }
            }
            buttons.push(ScienceButtonState {
                command_name: button.get_name().to_string(),
                science_type: science,
                is_hidden,
                is_enabled,
                is_purchased,
            });
        }
        buttons
    }

    /// First enabled purchase-science button (C++ populatePurchaseScience order).
    pub fn first_capable_purchase_science_name(&self) -> Option<String> {
        for buttons in [
            &self.science_state.rank1_buttons,
            &self.science_state.rank3_buttons,
            &self.science_state.rank8_buttons,
        ] {
            for btn in buttons {
                if btn.is_enabled && !btn.is_purchased && !btn.command_name.is_empty() {
                    return Some(btn.command_name.clone());
                }
            }
        }
        None
    }



    fn leftover_experience_progress_percent(&self, player: &gamelogic::player::Player) -> i32 {
        if let Some(percent) = self.science_state.live_rank_progress_percent {
            return percent.clamp(0, 100);
        }
        let skill = player.get_skill_points();
        let rank = player.get_rank_level();
        let (down, up) = gamelogic::system::rank_info::the_rank_info_store()
            .and_then(|store| {
                let down = store
                    .get_rank_info(rank as usize)
                    .map(|info| info.skill_points_needed)
                    .unwrap_or(0);
                let up = store
                    .get_rank_info((rank + 1) as usize)
                    .map(|info| info.skill_points_needed)
                    .unwrap_or(down);
                Some((down, up))
            })
            .unwrap_or((0, skill.max(1)));
        let denom = up - down;
        if denom == 0 {
            return 0;
        }
        ((skill - down) * 100) / denom
    }

    fn update_context_purchase_science(&mut self) {
        if let Some(progress) = self.science_state.live_rank_progress_percent {
            if let Some(spp) = self.science_state.live_science_purchase_points {
                self.science_state.available_points = spp;
            }
            if let Some(rank) = self.science_state.live_rank_level {
                self.science_state.rank_level = rank;
            }
            self.science_state.experience_progress = (progress as f32 / 100.0).clamp(0.0, 1.0);
            leftover_set_progress(GEN_EXP_PROGRESS, progress);
            return;
        }
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
        let progress = self.leftover_experience_progress_percent(&player);
        self.science_state.experience_progress = (progress as f32 / 100.0).clamp(0.0, 1.0);
        leftover_set_progress(GEN_EXP_PROGRESS, progress);
    }

    /// Bind live host rank / skill to GeneralsExpPoints ProgressBarExperience.
    ///
    /// C++ ControlBar::updateContextPurchaseScience reads the local player's
    /// skill vs rank thresholds. Leftover PlayerList skill is never written by
    /// Main, so presentation pushes the already-correct live percent.
    pub fn sync_rank_progress_from_presentation(
        &mut self,
        percent: i32,
        skill_points: i32,
        rank_level: i32,
        science_purchase_points: i32,
    ) {
        let percent = percent.clamp(0, 100);
        self.science_state.live_rank_progress_percent = Some(percent);
        self.science_state.live_skill_points = Some(skill_points);
        self.science_state.live_rank_level = Some(rank_level);
        self.science_state.live_science_purchase_points = Some(science_purchase_points);
        self.science_state.rank_level = rank_level;
        self.science_state.available_points = science_purchase_points;
        self.science_state.rank_title_label = format!("SCIENCE:Rank{rank_level}");
        self.science_state.experience_progress = (percent as f32 / 100.0).clamp(0.0, 1.0);
        leftover_set_progress(GEN_EXP_PROGRESS, percent);
        // C++ getStarImage / onPlayerSciencePurchasePointsChanged.
        if science_purchase_points > 0 {
            self.gen_star_flash = true;
            if self.last_flashed_at_point_value < science_purchase_points {
                self.last_flashed_at_point_value = science_purchase_points;
            }
        }
        self.leftover_apply_purchase_science_windows();
    }

    /// Feed unlocked science names from PresentationFrame into purchase UI residual.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Marks matching rank buttons purchased; stores the full unlocked name list.
    /// Fail-closed: does not claim full ScienceStore / rank-point parity.
    pub fn sync_sciences_from_presentation(&mut self, unlocked_sciences: &[String]) {
        let mut unlocked: Vec<String> = unlocked_sciences.to_vec();
        unlocked.sort();
        unlocked.dedup();
        self.science_state.unlocked_sciences = unlocked.clone();

        let mark = |buttons: &mut Vec<ScienceButtonState>| {
            for btn in buttons.iter_mut() {
                let purchased = unlocked.iter().any(|name| {
                    name.eq_ignore_ascii_case(&btn.command_name)
                        || (!btn.command_name.is_empty()
                            && name
                                .to_ascii_lowercase()
                                .contains(&btn.command_name.to_ascii_lowercase()))
                        || name.eq_ignore_ascii_case(&format!("{:?}", btn.science_type))
                });
                if purchased {
                    btn.is_purchased = true;
                    btn.is_enabled = false;
                }
            }
        };
        mark(&mut self.science_state.rank1_buttons);
        mark(&mut self.science_state.rank3_buttons);
        mark(&mut self.science_state.rank8_buttons);

        // When no INI buttons are loaded yet, seed purchased placeholders so HUD
        // residual still reflects snapshot sciences (fail-closed CommandSet art).
        if self.science_state.rank1_buttons.is_empty()
            && self.science_state.rank3_buttons.is_empty()
            && self.science_state.rank8_buttons.is_empty()
            && !unlocked.is_empty()
        {
            self.science_state.rank1_buttons = unlocked
                .iter()
                .take(MAX_PURCHASE_SCIENCE_RANK_1)
                .map(|name| ScienceButtonState {
                    command_name: name.clone(),
                    science_type: SCIENCE_INVALID,
                    is_hidden: false,
                    is_enabled: false,
                    is_purchased: true,
                })
                .collect();
        }
        self.leftover_apply_purchase_science_windows();
        self.mark_ui_dirty();

    }

    pub fn get_science_state(&self) -> &SciencePurchaseState {
        &self.science_state
    }

    // ---------------------------------------------------------------------------
    // Control bar stage management
    // C++ ControlBar.cpp:2968-3053
    // ---------------------------------------------------------------------------

    fn leftover_capture_default_control_bar_position(&mut self) {
        if self.default_control_bar_captured {
            return;
        }
        if let Some(parent) = leftover_find_window(CONTROL_BAR_PARENT) {
            let (x, y) = parent.borrow().get_position();
            self.default_control_bar_x = x;
            self.default_control_bar_y = y;
            self.default_control_bar_captured = true;
        }
    }

    fn leftover_set_up_down_images(&self) {
        // C++ ControlBar::setUpDownImages (ControlBar.cpp:3128-3146): the
        // up/down toggle button shows the scheme's ToggleButtonDown* art in the
        // default stage and ToggleButtonUp* art when collapsed to the low
        // stage. Unresolved mapped images leave the slots unbound (the image
        // draw paints nothing, W3DPushButton.cpp:288-368).
        let Some(manager) = game_engine::common::ini::get_control_bar_scheme_manager()
        else {
            return;
        };
        let Some(scheme) = manager.read().get_active_scheme().cloned() else {
            return;
        };
        let (on, inside, pushed) = if self.control_bar_stage == ControlBarStage::Low {
            (
                &scheme.toggle_button_up_on,
                &scheme.toggle_button_up_in,
                &scheme.toggle_button_up_pushed,
            )
        } else {
            (
                &scheme.toggle_button_down_on,
                &scheme.toggle_button_down_in,
                &scheme.toggle_button_down_pushed,
            )
        };
        let Some(win) = leftover_find_window(BUTTON_LARGE) else {
            return;
        };
        let mut guard = win.borrow_mut();
        let data = guard.instance_data_mut();
        if let Some(image) = leftover_mapped_image(on) {
            data.enabled_draw_data[0].image = Some(image);
        }
        if let Some(image) = leftover_mapped_image(inside) {
            data.hilite_draw_data[0].image = Some(image);
        }
        if let Some(image) = leftover_mapped_image(pushed) {
            data.hilite_draw_data[1].image = Some(image);
        }
    }

    fn leftover_set_default_control_bar_config(&mut self) {
        self.control_bar_stage = ControlBarStage::Default;
        reveal_ingame_command_window();
        self.leftover_capture_default_control_bar_position();
        let (_sw, sh) = leftover_display_size();
        leftover_set_tactical_view_height(((sh as f32) * 0.80) as i32);
        if let Some(parent) = leftover_find_window(CONTROL_BAR_PARENT) {
            let _ = parent.borrow_mut().set_position(
                self.default_control_bar_x,
                self.default_control_bar_y,
            );
            let _ = parent.borrow_mut().hide(false);
        }
        self.leftover_set_up_down_images();
    }

    fn leftover_set_low_control_bar_config(&mut self) {
        self.leftover_capture_default_control_bar_position();
        self.control_bar_stage = ControlBarStage::Low;
        let (_sw, sh) = leftover_display_size();
        leftover_set_tactical_view_height(sh);
        let y = sh - ((sh as f32) * 0.10) as i32;
        if let Some(parent) = leftover_find_window(CONTROL_BAR_PARENT) {
            let _ = parent
                .borrow_mut()
                .set_position(self.default_control_bar_x, y);
            let _ = parent.borrow_mut().hide(false);
        }
        self.leftover_set_up_down_images();
    }

    pub fn switch_control_bar_stage(&mut self, stage: ControlBarStage) {
        PENDING_CONTROL_BAR_STAGE.store(0xFF, std::sync::atomic::Ordering::SeqCst);
        if TheGameLogic::is_in_replay_game() {
            return;
        }
        match stage {
            ControlBarStage::Default => self.leftover_set_default_control_bar_config(),
            ControlBarStage::Low => self.leftover_set_low_control_bar_config(),
            ControlBarStage::Hidden => {
                self.control_bar_stage = ControlBarStage::Hidden;
                leftover_hide_window(CONTROL_BAR_PARENT, true);
            }
        }
    }

    /// C++ `ShowControlBar` (ControlBarCallback.cpp:489) when ControlBarParent is live.
    pub fn request_switch_control_bar_stage(stage: ControlBarStage) {
        PENDING_CONTROL_BAR_STAGE.store(
            control_bar_stage_to_u8(stage),
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    pub fn apply_pending_control_bar_stage(&mut self) {
        if let Some(stage) = take_pending_control_bar_stage() {
            self.switch_control_bar_stage(stage);
        }
    }

    pub fn toggle_control_bar_stage(&mut self) {
        if self.control_bar_stage == ControlBarStage::Default {
            self.switch_control_bar_stage(ControlBarStage::Low);
        } else {
            self.switch_control_bar_stage(ControlBarStage::Default);
        }
    }

    pub fn get_control_bar_stage(&self) -> ControlBarStage {
        self.control_bar_stage
    }

    // ---------------------------------------------------------------------------
    // Control bar scheme
    // C++ ControlBar.cpp:2726-2824
    // ---------------------------------------------------------------------------

    fn local_player_side_name() -> String {
        logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_side().clone()))
            .filter(|side| !side.is_empty())
            .unwrap_or_else(|| "Observer".to_string())
    }

    /// Apply the existing scheme manager for `player_side` (C++ ControlBarScheme.cpp:1139).
    fn apply_control_bar_scheme_for_side(&self, player_side: &str) {
        let side = if player_side.is_empty() {
            "Observer"
        } else {
            player_side
        };
        if let Some(manager) = &self.scheme_manager {
            let _ = manager.load_scheme(side);
        }
        if let Some(manager) = game_engine::common::ini::get_control_bar_scheme_manager() {
            let mut manager = manager.write();
            let _ = manager.set_active_scheme_for_side(side);
            // C++ ControlBarScheme.cpp:1124-1125: m_multiplyer is
            // display_size / scheme ScreenCreationRes, set at scheme select.
            let (creation_w, creation_h) = manager
                .get_active_scheme()
                .map(|scheme| (scheme.screen_creation_res.x, scheme.screen_creation_res.y))
                .unwrap_or((800, 600));
            if creation_w > 0 && creation_h > 0 {
                let (screen_w, screen_h) = leftover_display_size();
                manager.set_multiplier(
                    screen_w as f32 / creation_w as f32,
                    screen_h as f32 / creation_h as f32,
                );
            }
        }
        self.apply_scheme_button_art();
    }

    /// C++ ControlBarScheme::init (ControlBarScheme.cpp:401-662): bind the
    /// active scheme's fixed-HUD art to the ControlBar.wnd windows and move
    /// them to the scheme slot rects. Button art (options / idle worker /
    /// beacon / communicator / general / up-down toggle / queue slots / exp
    /// bar / u-attack) is stored on the same draw-data slots C++
    /// GadgetButtonSetEnabledImage / GadgetButtonSetHiliteImage /
    /// GadgetButtonSetHiliteSelectedImage / GadgetButtonSetDisabledImage write
    /// (enabled[0], hilite[0], hilite[1], disabled[0]). Image names that do
    /// not resolve through the mapped-image collection leave the slot unbound
    /// so the image draw paints nothing (W3DPushButton.cpp:288-368) — never a
    /// color fill from the authored placeholder.
    fn apply_scheme_button_art(&self) {
        let Some(manager) = game_engine::common::ini::get_control_bar_scheme_manager()
        else {
            return;
        };
        let Some(scheme) = manager.read().get_active_scheme().cloned() else {
            return;
        };

        // C++ resMultiplier = display / m_ScreenCreationRes
        // (ControlBarScheme.cpp:417-419).
        let (screen_w, screen_h) = leftover_display_size();
        let creation_w = scheme.screen_creation_res.x.max(1) as f32;
        let creation_h = scheme.screen_creation_res.y.max(1) as f32;
        let mult_x = screen_w as f32 / creation_w;
        let mult_y = screen_h as f32 / creation_h;
        let scale = |v: i32, mult: f32| (v as f32 * mult).round() as i32;

        let bind_slot =
            |slot: &mut Option<crate::gui::game_window::Image>, name: &str| {
                if !name.is_empty() {
                    if let Some(image) = leftover_mapped_image(name) {
                        *slot = Some(image);
                    }
                }
            };

        let art_slots = [
            (
                "ControlBar.wnd:ButtonOptions",
                &scheme.options_button_enable,
                &scheme.options_button_hightlited,
                &scheme.options_button_pushed,
                &scheme.options_button_disabled,
                (scheme.options_ul, scheme.options_lr),
            ),
            (
                "ControlBar.wnd:ButtonIdleWorker",
                &scheme.idle_worker_button_enable,
                &scheme.idle_worker_button_hightlited,
                &scheme.idle_worker_button_pushed,
                &scheme.idle_worker_button_disabled,
                (scheme.worker_ul, scheme.worker_lr),
            ),
            (
                "ControlBar.wnd:ButtonPlaceBeacon",
                &scheme.beacon_button_enable,
                &scheme.beacon_button_hightlited,
                &scheme.beacon_button_pushed,
                &scheme.beacon_button_disabled,
                (scheme.beacon_ul, scheme.beacon_lr),
            ),
            (
                "ControlBar.wnd:PopupCommunicator",
                &scheme.buddy_button_enable,
                &scheme.buddy_button_hightlited,
                &scheme.buddy_button_pushed,
                &scheme.buddy_button_disabled,
                (scheme.chat_ul, scheme.chat_lr),
            ),
            (
                BUTTON_GENERAL,
                &scheme.general_button_enable,
                &scheme.general_button_hightlited,
                &scheme.general_button_pushed,
                &scheme.general_button_disabled,
                (scheme.general_ul, scheme.general_lr),
            ),
        ];
        for (window_name, enable, hilite, pushed, disabled, (ul, lr)) in art_slots {
            let Some(win) = leftover_find_window(window_name) else {
                continue;
            };
            let mut guard = win.borrow_mut();
            {
                let data = guard.instance_data_mut();
                bind_slot(&mut data.enabled_draw_data[0].image, enable);
                bind_slot(&mut data.hilite_draw_data[0].image, hilite);
                bind_slot(&mut data.hilite_draw_data[1].image, pushed);
                bind_slot(&mut data.disabled_draw_data[0].image, disabled);
            }
            // C++ positions each button at scheme UL * resMultiplier minus the
            // parent screen position, and sizes it LR-UL * resMultiplier
            // (ControlBarScheme.cpp:432-447 and siblings).
            let (screen_x, screen_y) = guard.get_screen_position();
            let (rel_x, rel_y) = guard.get_position();
            let (parent_x, parent_y) = (screen_x - rel_x, screen_y - rel_y);
            let width = scale(lr.x, mult_x) - scale(ul.x, mult_x);
            let height = scale(lr.y, mult_y) - scale(ul.y, mult_y);
            let _ = guard.set_position(scale(ul.x, mult_x) - parent_x, scale(ul.y, mult_y) - parent_y);
            let _ = guard.set_size(width.max(1), height.max(1));
        }

        // ExpBarForeground: C++ winSetEnabledImage(0, m_expBarForeground)
        // (ControlBarScheme.cpp:476-480).
        if let Some(win) = leftover_find_window("ControlBar.wnd:ExpBarForeground") {
            if let Some(image) = leftover_mapped_image(&scheme.exp_bar_foreground_image) {
                let _ = win.borrow_mut().set_enabled_image(0, image);
            }
        }

        // WinUAttack: enabled + disabled(=highlight) images and slot reposition
        // (ControlBarScheme.cpp:629-651).
        if let Some(win) = leftover_find_window(WIN_U_ATTACK) {
            let mut guard = win.borrow_mut();
            if let Some(image) = leftover_mapped_image(&scheme.u_attack_button_enable) {
                let _ = guard.set_enabled_image(0, image);
            }
            if let Some(image) = leftover_mapped_image(&scheme.u_attack_button_hightlited) {
                let data = guard.instance_data_mut();
                data.disabled_draw_data[0].image = Some(image);
            }
            let (screen_x, screen_y) = guard.get_screen_position();
            let (rel_x, rel_y) = guard.get_position();
            let (parent_x, parent_y) = (screen_x - rel_x, screen_y - rel_y);
            let _ = guard.set_position(
                scale(scheme.u_attack_ul.x, mult_x) - parent_x,
                scale(scheme.u_attack_ul.y, mult_y) - parent_y,
            );
            let _ = guard.set_size(
                (scale(scheme.u_attack_lr.x, mult_x) - scale(scheme.u_attack_ul.x, mult_x)).max(1),
                (scale(scheme.u_attack_lr.y, mult_y) - scale(scheme.u_attack_ul.y, mult_y)).max(1),
            );
        }

        // Build queue slots: disabled image = scheme queue art
        // (C++ updateBuildQueueDisabledImages, ControlBar.cpp:2832-2870).
        if !scheme.queue_button_image.is_empty() {
            if let Some(image) = leftover_mapped_image(&scheme.queue_button_image) {
                for index in 1..=9 {
                    if let Some(win) =
                        leftover_find_window(&format!("ControlBar.wnd:ButtonQueue{index:02}"))
                    {
                        let mut guard = win.borrow_mut();
                        let data = guard.instance_data_mut();
                        data.disabled_draw_data[0].image = Some(image.clone());
                    }
                }
            }
        }

        // Min/max toggle (ButtonLarge): scheme slot position; art comes from
        // setUpDownImages on every stage switch
        // (ControlBarScheme.cpp:603-627, ControlBar.cpp:3128-3146).
        if let Some(win) = leftover_find_window(BUTTON_LARGE) {
            let mut guard = win.borrow_mut();
            let (screen_x, screen_y) = guard.get_screen_position();
            let (rel_x, rel_y) = guard.get_position();
            let (parent_x, parent_y) = (screen_x - rel_x, screen_y - rel_y);
            let _ = guard.set_position(
                scale(scheme.min_max_ul.x, mult_x) - parent_x,
                scale(scheme.min_max_ul.y, mult_y) - parent_y,
            );
            let _ = guard.set_size(
                (scale(scheme.min_max_lr.x, mult_x) - scale(scheme.min_max_ul.x, mult_x)).max(1),
                (scale(scheme.min_max_lr.y, mult_y) - scale(scheme.min_max_ul.y, mult_y)).max(1),
            );
        }
        self.leftover_set_up_down_images();
    }

    pub fn set_control_bar_scheme_by_player_side(&mut self, player_side: &str) {
        self.apply_control_bar_scheme_for_side(player_side);
        self.apply_scheme_context_and_default_stage();
    }

    pub fn set_control_bar_scheme_by_player(&mut self) {
        self.apply_control_bar_scheme_for_side(&Self::local_player_side_name());
        self.apply_scheme_context_and_default_stage();
    }

    fn apply_scheme_context_and_default_stage(&mut self) {
        let is_observer = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);

        if is_observer {
            self.observer_mode = true;
            super::control_bar_observer::init_observer_controls();
            super::control_bar_observer::ensure_gen_arrow_from_mapped_images();
            let _ = self.switch_to_context(ControlBarState::Observer, None);
        } else {
            self.observer_mode = false;
            let _ = self.switch_to_context(ControlBarState::None, None);
        }

        // C++ setControlBarSchemeByPlayer (ControlBar.cpp:2756-2761): the
        // beacon-placement button only exists in LAN/Internet multiplayer;
        // hide it in skirmish, single-player, and observer sessions.
        let game_mode = gamelogic::helpers::TheGameLogic::get_game_mode();
        leftover_hide_window(
            "ControlBar.wnd:ButtonPlaceBeacon",
            game_mode != gamelogic::system::game_logic::GAME_LAN
                && game_mode != gamelogic::system::game_logic::GAME_INTERNET,
        );

        self.switch_control_bar_stage(ControlBarStage::Default);
        self.mark_ui_dirty();
    }

    // ---------------------------------------------------------------------------
    // Player event handlers
    // C++ ControlBar.cpp:1651-1682
    // ---------------------------------------------------------------------------

    fn leftover_maybe_flash_control_bar_arrow(&self) {
        let current_points = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_science_purchase_points()))
            .unwrap_or(0);
        if self.last_flashed_at_point_value > current_points {
            return;
        }
        if crate::helpers::TheInGameUI::get_input_enabled() {
            leftover_transition_group("ControlBarArrow");
        }
    }

    pub fn on_player_rank_changed(&mut self) {
        self.leftover_maybe_flash_control_bar_arrow();
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    pub fn on_player_science_purchase_points_changed(&mut self) {
        self.leftover_maybe_flash_control_bar_arrow();
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    // ---------------------------------------------------------------------------
    // Special power shortcut bar
    // C++ ControlBar.cpp:3198-3747
    // ---------------------------------------------------------------------------

    fn leftover_shortcut_button_name(&self, index: usize) -> String {
        if self.special_power_shortcut_layout.is_empty() {
            format!("ControlBar.wnd:ButtonCommand{}", index + 1)
        } else {
            format!("{}:ButtonCommand{}", self.special_power_shortcut_layout, index + 1)
        }
    }

    fn leftover_shortcut_parent_name(&self, index: usize) -> String {
        if self.special_power_shortcut_layout.is_empty() {
            format!("ControlBar.wnd:ButtonParent{}", index + 1)
        } else {
            format!("{}:ButtonParent{}", self.special_power_shortcut_layout, index + 1)
        }
    }

    fn leftover_shortcut_bar_parent_name(&self) -> String {
        if self.special_power_shortcut_layout.is_empty() {
            "ControlBar.wnd:GenPowersShortcutBarParent".to_string()
        } else {
            format!("{}:GenPowersShortcutBarParent", self.special_power_shortcut_layout)
        }
    }

    pub fn init_special_power_shortcut_bar(&mut self) {
        self.special_power_shortcuts.clear();
        self.special_power_shortcut_count = 0;
        self.special_power_shortcut_layout.clear();

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };
        if !player.is_player_active() {
            return;
        }
        let Some(template) = player.get_player_template() else {
            return;
        };
        let button_count = template.get_special_power_shortcut_button_count();
        let win_name = template.get_special_power_shortcut_win_name().to_string();
        if button_count <= 0 || win_name.is_empty() {
            return;
        }
        self.special_power_shortcut_count =
            (button_count as usize).min(MAX_SPECIAL_POWER_SHORTCUTS);
        self.special_power_shortcut_layout = win_name.clone();

        with_window_manager(|manager| {
            let _ = manager.create_layout_with_windows(&win_name);
        });
        leftover_hide_window(&self.leftover_shortcut_bar_parent_name(), true);
        leftover_hide_window(&win_name, leftover_find_window(&win_name).is_some());


        for i in 0..self.special_power_shortcut_count {
            self.special_power_shortcuts
                .push(SpecialPowerShortcutState {
                    command_name: String::new(),
                    availability: CommandAvailability::Hidden,
                    multiplier_count: 1,
                    is_hidden: true,
                });
            let button_name = self.leftover_shortcut_button_name(i);
            if let Some(win) = leftover_find_window(&button_name) {
                let mut win = win.borrow_mut();
                win.set_status(crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES);
                win.set_status(crate::gui::game_window::WindowStatus::SHORTCUT_BUTTON);
                let _ = win.hide(true);
            }
            leftover_hide_window(&self.leftover_shortcut_parent_name(i), true);
        }
    }

    fn leftover_player_has_object_template(
        player: &gamelogic::player::Player,
        template_name: &str,
    ) -> bool {
        if template_name.is_empty() {
            return false;
        }
        let mut found = false;
        let _ = player.iterate_objects(|obj_arc| {
            if found {
                return Ok(());
            }
            if let Ok(obj) = obj_arc.read() {
                if obj.get_template_name().eq_ignore_ascii_case(template_name) {
                    found = true;
                }
            }
            Ok(())
        });
        found
    }

    fn leftover_has_any_shortcut_special_power(player: &gamelogic::player::Player) -> bool {
        let mut found = false;
        let _ = player.iterate_objects(|obj_arc| {
            if found {
                return Ok(());
            }
            if let Ok(obj) = obj_arc.read() {
                if obj
                    .find_any_shortcut_special_power_module_interface()
                    .is_some()
                {
                    found = true;
                }
            }
            Ok(())
        });
        found
    }

    fn leftover_shortcut_module_name_and_ready(
        obj: &gamelogic::object::Object,
    ) -> Option<(String, bool, f32)> {
        let behavior = obj.find_any_shortcut_special_power_module_interface()?;
        let mut guard = behavior.lock().ok()?;
        let sp = guard.get_special_power_module_interface()?;
        let template_any = sp.get_special_power_template()?;
        let template = template_any.downcast_ref::<std::sync::Arc<
            gamelogic::object::special_power_template::SpecialPowerTemplate,
        >>()?;
        Some((
            template.get_name().to_string(),
            sp.is_ready(),
            sp.get_percent_ready(),
        ))
    }

    fn leftover_count_ready_shortcut_special_powers(
        player: &gamelogic::player::Player,
        special_power_name: &str,
    ) -> i32 {
        if special_power_name.is_empty() {
            return 0;
        }
        let mut count = 0;
        let _ = player.iterate_objects(|obj_arc| {
            let Ok(obj) = obj_arc.read() else {
                return Ok(());
            };
            if let Some((name, ready, _)) = Self::leftover_shortcut_module_name_and_ready(&obj) {
                if name.eq_ignore_ascii_case(special_power_name) && ready {
                    count += 1;
                }
            }
            Ok(())
        });
        count
    }

    fn leftover_find_most_ready_shortcut_object(
        player: &gamelogic::player::Player,
        special_power_name: &str,
    ) -> Option<u32> {
        if special_power_name.is_empty() {
            return None;
        }
        let mut best_id = None;
        let mut best_ready = -1.0f32;
        let _ = player.iterate_objects(|obj_arc| {
            let Ok(obj) = obj_arc.read() else {
                return Ok(());
            };
            if let Some((name, _, ready)) = Self::leftover_shortcut_module_name_and_ready(&obj) {
                if name.eq_ignore_ascii_case(special_power_name) && ready > best_ready {
                    best_ready = ready;
                    best_id = Some(obj.get_id());
                }
            }
            Ok(())
        });
        best_id
    }

    fn leftover_apply_shortcut_window(
        &self,
        index: usize,
        hidden: bool,
        availability: CommandAvailability,
        ready_count: i32,
    ) {
        let button_name = self.leftover_shortcut_button_name(index);
        let parent_name = self.leftover_shortcut_parent_name(index);
        leftover_hide_window(&button_name, hidden);
        leftover_hide_window(&parent_name, hidden);
        if hidden {
            leftover_set_button_text(&button_name, "");
            return;
        }
        if let Some(win) = leftover_find_window(&button_name) {
            let mut win = win.borrow_mut();
            win.clear_status(crate::gui::game_window::WindowStatus::NOT_READY);
            win.clear_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
            match availability {
                CommandAvailability::Hidden => {
                    let _ = win.hide(true);
                }
                CommandAvailability::Restricted => {
                    let _ = win.enable(false);
                }
                CommandAvailability::NotReady => {
                    let _ = win.enable(false);
                    win.set_status(crate::gui::game_window::WindowStatus::NOT_READY);
                }
                CommandAvailability::CantAfford => {
                    let _ = win.enable(false);
                    win.set_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                }
                _ => {
                    let _ = win.enable(true);
                }
            }
        }
        if ready_count > 1 {
            leftover_set_button_text(&button_name, &ready_count.to_string());
        } else {
            leftover_set_button_text(&button_name, "");
        }
    }

    fn populate_special_power_shortcut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.special_power_shortcut_count == 0 {
            return Ok(());
        }

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };
        if !player.is_local_player() {
            return Ok(());
        }
        let Some(template) = player.get_player_template() else {
            return Ok(());
        };
        let set_name = template.get_special_power_shortcut_command_set();
        if set_name.is_empty() {
            return Ok(());
        }

        let control_bar = get_control_bar_bridge();
        let Some(control_bar) = control_bar else {
            return Ok(());
        };

        let command_set = control_bar
            .find_command_set_by_name(set_name)
            .or_else(|| control_bar.find_command_set_by_name(&set_name.to_ascii_uppercase()));

        let Some(command_set) = command_set else {
            return Ok(());
        };

        for i in 0..self.special_power_shortcut_count {
            leftover_hide_window(&self.leftover_shortcut_button_name(i), true);
            leftover_hide_window(&self.leftover_shortcut_parent_name(i), true);
        }

        let mut current_button = 0;
        for i in 0..self
            .special_power_shortcut_count
            .min(command_set.buttons.len())
        {
            let Some(logic_button) = command_set.buttons.get(i).and_then(|b| b.as_ref()) else {
                continue;
            };

            if (logic_button.get_options_bits() & CommandOption::NeedUpgrade as u32) != 0 {
                if let Some(upgrade) = logic_button.get_upgrade_template() {
                    if !player.has_upgrade_complete(upgrade) {
                        continue;
                    }
                }
            }

            if (logic_button.get_options_bits() & CommandOption::NeedSpecialPowerScience as u32) != 0
            {
                let Some(power) = logic_button.get_special_power_template() else {
                    continue;
                };
                if Self::leftover_find_most_ready_shortcut_object(&player, power.get_name())
                    .is_none()
                {
                    continue;
                }
                let required = power.get_required_science();
                if required != SCIENCE_INVALID && !player.has_science(required) {
                    continue;
                }
            } else if logic_button.get_command_type() == CommandType::MetaSelectMatchingUnits {
                let template_name = logic_button
                    .get_thing_template()
                    .map(|t| t.get_name().as_str().to_string())
                    .unwrap_or_default();
                if !Self::leftover_player_has_object_template(&player, &template_name) {
                    continue;
                }
            }

            if current_button < self.special_power_shortcuts.len() {
                let mut cmd = Self::command_from_logic_button(logic_button);
                if let Some(common_bar) = get_ini_control_bar() {
                    cmd = Self::command_from_set_slot(&common_bar, Some(logic_button));
                } else {
                    Self::apply_need_special_power_science(&mut cmd, logic_button);
                }
                self.special_power_shortcuts[current_button].command_name =
                    logic_button.get_name().to_string();
                self.special_power_shortcuts[current_button].is_hidden = false;
                self.special_power_shortcuts[current_button].availability =
                    CommandAvailability::Available;
                leftover_hide_window(&self.leftover_shortcut_button_name(current_button), false);
                leftover_hide_window(&self.leftover_shortcut_parent_name(current_button), false);
                leftover_enable_window(&self.leftover_shortcut_button_name(current_button), true);
                leftover_enable_window(&self.leftover_shortcut_parent_name(current_button), true);
                if !cmd.button_image.is_empty() {
                    leftover_set_enabled_image(
                        &self.leftover_shortcut_button_name(current_button),
                        &cmd.button_image,
                    );
                }
                // C++ ControlBar.cpp:2472-2476 also binds shortcut buttons.
                if !cmd.text_label.is_empty() {
                    if let Some(win) =
                        leftover_find_window(&self.leftover_shortcut_button_name(current_button))
                    {
                        let hot_key = with_hot_key_manager(|manager| {
                            manager.search_hot_key(&cmd.text_label)
                        });
                        if !hot_key.is_empty() {
                            with_hot_key_manager(|manager| manager.add_hot_key(win, &hot_key));
                        }
                    }
                }
                current_button += 1;
            }
        }

        for i in current_button..self.special_power_shortcuts.len() {
            self.special_power_shortcuts[i].is_hidden = true;
            self.special_power_shortcuts[i].command_name.clear();
            leftover_hide_window(&self.leftover_shortcut_button_name(i), true);
            leftover_hide_window(&self.leftover_shortcut_parent_name(i), true);
        }

        let parent_name = self.leftover_shortcut_bar_parent_name();
        if leftover_find_window(CONTROL_BAR_PARENT).is_some()
            && !leftover_window_is_hidden(CONTROL_BAR_PARENT)
            && leftover_window_is_hidden(&parent_name)
        {
            leftover_hide_window(&parent_name, false);
            self.animate_special_power_shortcut(true);
        }
        self.update_special_power_shortcut_availability();

        Ok(())
    }

    fn update_special_power_shortcut_availability(&mut self) {
        if self.special_power_shortcut_count == 0 || self.special_power_shortcuts.is_empty() {
            return;
        }

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };

        let has_select = self.has_any_shortcut_selection();
        let has_power = Self::leftover_has_any_shortcut_special_power(&player);
        let parent_name = self.leftover_shortcut_bar_parent_name();
        let master_visible = leftover_find_window(CONTROL_BAR_PARENT)
            .map(|w| !w.borrow().is_hidden())
            .unwrap_or(true);

        if (has_select || has_power)
            && leftover_window_is_hidden(&parent_name)
            && master_visible
        {
            leftover_hide_window(&parent_name, false);
            self.animate_special_power_shortcut(true);
        } else if !has_select && !has_power && !leftover_window_is_hidden(&parent_name) {
            self.animate_special_power_shortcut(false);
        }

        if leftover_window_is_hidden(&parent_name) {
            return;
        }

        if !player.is_player_active() {
            self.hide_special_power_shortcut();
            return;
        }

        let control_bar = get_control_bar_bridge();
        for i in 0..self.special_power_shortcuts.len() {
            if self.special_power_shortcuts[i].is_hidden {
                continue;
            }
            let command_name = self.special_power_shortcuts[i].command_name.clone();
            if command_name.is_empty() {
                continue;
            }
            let Some(control_bar) = control_bar.as_ref() else {
                self.special_power_shortcuts[i].availability = CommandAvailability::Available;
                continue;
            };
            let Some(logic_button) = control_bar.find_command_button_by_name(&command_name) else {
                // Presentation-seeded ready shortcuts stay Available.
                self.special_power_shortcuts[i].availability = CommandAvailability::Available;
                continue;
            };

            let mut availability = CommandAvailability::Restricted;
            let mut ready_count = 1;
            if let Some(power) = logic_button.get_special_power_template() {
                ready_count = Self::leftover_count_ready_shortcut_special_powers(
                    &player,
                    power.get_name(),
                )
                .max(1);
                if let Some(obj_id) =
                    Self::leftover_find_most_ready_shortcut_object(&player, power.get_name())
                {
                    let cmd = Self::command_from_logic_button(&logic_button);
                    availability = self
                        .get_command_availability(&cmd, obj_id, player.get_player_index() as u32)
                        .unwrap_or(CommandAvailability::Restricted);
                }
            } else if logic_button.get_command_type() == CommandType::MetaSelectMatchingUnits {
                let template_name = logic_button
                    .get_thing_template()
                    .map(|t| t.get_name().as_str().to_string())
                    .unwrap_or_default();
                if Self::leftover_player_has_object_template(&player, &template_name) {
                    availability = CommandAvailability::Available;
                } else {
                    availability = CommandAvailability::Hidden;
                }
            }

            self.special_power_shortcuts[i].availability = availability;
            self.special_power_shortcuts[i].multiplier_count = ready_count;
            if availability == CommandAvailability::Hidden {
                self.special_power_shortcuts[i].is_hidden = true;
            }
            self.leftover_apply_shortcut_window(i, self.special_power_shortcuts[i].is_hidden, availability, ready_count);
        }
    }

    pub fn hide_special_power_shortcut(&mut self) {
        leftover_hide_window(&self.leftover_shortcut_bar_parent_name(), true);
        for (i, shortcut) in self.special_power_shortcuts.iter_mut().enumerate() {
            shortcut.is_hidden = true;
            leftover_hide_window(
                &if self.special_power_shortcut_layout.is_empty() {
                    format!("ControlBar.wnd:ButtonCommand{}", i + 1)
                } else {
                    format!("{}:ButtonCommand{}", self.special_power_shortcut_layout, i + 1)
                },
                true,
            );
        }
    }

    pub fn show_special_power_shortcut(&mut self) {
        if gamelogic::helpers::TheScriptEngine::is_game_ending() {
            return;
        }
        if self.special_power_shortcut_count == 0 {
            self.init_special_power_shortcut_bar();
        }
        leftover_hide_window(&self.leftover_shortcut_bar_parent_name(), false);
        for shortcut in &mut self.special_power_shortcuts {
            if !shortcut.command_name.is_empty() {
                shortcut.is_hidden = false;
            }
        }
        let _ = self.populate_special_power_shortcut();
    }

    pub fn animate_special_power_shortcut(&mut self, enabled: bool) {
        self.is_animating = enabled;
        if enabled {
            self.animation_start_time = Instant::now();
        }
    }

    pub fn get_observer_look_at_player_index(&self) -> Option<i32> {
        if !self.observer_mode {
            return None;
        }
        if let Some(index) = super::control_bar_observer::observer_look_at_player_index() {
            return Some(index);
        }
        self.context
            .read()
            .ok()
            .and_then(|ctx| ctx.observer_look_at_player)
    }

    pub fn set_observer_look_at_player_index(&mut self, index: Option<i32>) {
        super::control_bar_observer::set_observer_look_at_player(index);
        if let Ok(mut ctx) = self.context.write() {
            ctx.observer_look_at_player = index;
        }
    }

    pub fn get_arrow_image(&self) -> Option<crate::gui::game_window::Image> {
        super::control_bar_observer::get_gen_arrow_image()
    }

    pub fn set_arrow_image(&mut self, image: Option<crate::gui::game_window::Image>) {
        super::control_bar_observer::set_gen_arrow_image(image);
    }

    pub fn has_any_shortcut_selection(&self) -> bool {
        self.special_power_shortcuts
            .iter()
            .any(|s| !s.is_hidden && s.availability != CommandAvailability::Hidden)
    }

    /// Feed radar, queued upgrades, and ready special-power shortcuts from PresentationFrame.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Fail-closed: shortcut command names are template placeholders (not full CommandSet art).
    pub fn sync_radar_queues_and_specials_from_presentation(
        &mut self,
        radar_count: i32,
        radar_disabled: bool,
        queued_upgrades: &[String],
        ready_special_power_templates: &[String],
    ) {
        self.presentation_radar_count = radar_count;
        self.presentation_radar_disabled = radar_disabled;
        let mut queued = queued_upgrades.to_vec();
        queued.sort();
        queued.dedup();
        self.presentation_queued_upgrades = queued;

        // Seed shortcuts from presentation-ready special powers when empty.
        if self.special_power_shortcuts.is_empty() && !ready_special_power_templates.is_empty() {
            let mut names = ready_special_power_templates.to_vec();
            names.sort();
            names.dedup();
            self.special_power_shortcuts = names
                .into_iter()
                .take(8)
                .map(|command_name| SpecialPowerShortcutState {
                    command_name,
                    availability: CommandAvailability::Available,
                    multiplier_count: 1,
                    is_hidden: false,
                })
                .collect();
            self.special_power_shortcut_count = self.special_power_shortcuts.len();
        } else if !ready_special_power_templates.is_empty() {
            // Mark matching shortcuts available.
            for sc in self.special_power_shortcuts.iter_mut() {
                if ready_special_power_templates.iter().any(|n| {
                    n.eq_ignore_ascii_case(&sc.command_name)
                        || sc.command_name.eq_ignore_ascii_case(n)
                }) {
                    sc.availability = CommandAvailability::Available;
                    sc.is_hidden = false;
                }
            }
        }

        // Gen-star flash is SPP-driven (sync_rank_progress + update_star_image).
        // Queued upgrades are not C++ getStarImage.
        self.mark_ui_dirty();
    }

    pub fn presentation_radar_count(&self) -> i32 {
        self.presentation_radar_count
    }

    pub fn presentation_radar_disabled(&self) -> bool {
        self.presentation_radar_disabled
    }

    pub fn presentation_queued_upgrades(&self) -> &[String] {
        &self.presentation_queued_upgrades
    }

    pub fn get_special_power_shortcuts(&self) -> &[SpecialPowerShortcutState] {
        &self.special_power_shortcuts
    }

    // ---------------------------------------------------------------------------
    // Command bar border colors
    // C++ ControlBar.cpp:2361-2397, 2887-2893
    // ---------------------------------------------------------------------------

    pub fn set_command_bar_border_colors(
        &mut self,
        build: Option<u32>,
        action: Option<u32>,
        upgrade: Option<u32>,
        system: Option<u32>,
    ) {
        self.border_colors.build = build;
        self.border_colors.action = action;
        self.border_colors.upgrade = upgrade;
        self.border_colors.system = system;

    }

    pub fn get_border_color_for_type(
        &self,
        border_type: CommandButtonMappedBorderType,
    ) -> Option<u32> {
        match border_type {
            CommandButtonMappedBorderType::Build => self.border_colors.build,
            CommandButtonMappedBorderType::Upgrade => self.border_colors.upgrade,
            CommandButtonMappedBorderType::Action => self.border_colors.action,
            CommandButtonMappedBorderType::System => self.border_colors.system,
            CommandButtonMappedBorderType::None => None,
        }
    }

    // ---------------------------------------------------------------------------
    // Build queue display helpers
    // C++ ControlBar.cpp:2832-2870
    // ---------------------------------------------------------------------------

    pub fn get_displayed_construct_percent(&self) -> f32 {
        self.displayed_construct_percent
    }

    pub fn get_displayed_ocl_timer_seconds(&self) -> u32 {
        self.displayed_ocl_timer_seconds
    }

    pub fn get_displayed_queue_count(&self) -> usize {
        self.displayed_queue_count
    }

    // ---------------------------------------------------------------------------
    // Communicator hide
    // C++ ControlBar.cpp:2897-2902
    // ---------------------------------------------------------------------------

    pub fn hide_communicator(&mut self, hide: bool) {
        if let Some(wm) = &self.window_manager {
            if let Some(win) = wm.find_window_by_name("ControlBar.wnd:PopupCommunicator") {
                let _ = win.borrow_mut().hide(hide);
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Selection update (external entry point)
    // ---------------------------------------------------------------------------

    pub fn update_for_selection(
        &mut self,
        selected_objects: Vec<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut context = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            context.selected_objects = selected_objects;
        }
        self.mark_ui_dirty();
        self.update(Duration::from_millis(33))?;
        self.apply_pending_control_bar_stage();
        Ok(())
    }

    pub fn set_observer_mode(&mut self, observer: bool) {
        self.observer_mode = observer;
        self.mark_ui_dirty();
    }

    fn refresh_button_states(&mut self, context: &ControlBarContext, player_id: u32) {
        let mut refreshed = HashMap::new();
        for button in &context.available_commands {
            let mut state = self
                .button_states
                .get(&button.command_name)
                .cloned()
                .unwrap_or_default();
            state.visible = true;
            refreshed.insert(button.command_name.clone(), state);
        }

        let crate_has = |science: ScienceType| -> bool {
            logic_player_list()
                .read()
                .ok()
                .and_then(|list| {
                    list.get_player(player_id as PlayerIndex)
                        .cloned()
                        .or_else(|| list.get_local_player().cloned())
                })
                .and_then(|arc| arc.read().ok().map(|p| p.has_science(science)))
                .unwrap_or(false)
        };

        for (name, state) in refreshed.iter_mut() {
            if let Some(button) = context
                .available_commands
                .iter()
                .find(|b| &b.command_name == name)
            {
                if let Some(&first_id) = context.selected_objects.first() {
                    match self.get_command_availability(button, first_id, player_id) {
                        Ok(availability) => {
                            state.availability = availability;
                            state.enabled = matches!(
                                availability,
                                CommandAvailability::Available | CommandAvailability::Active
                            );
                            state.check_like_active = availability == CommandAvailability::Active;
                        }
                        Err(_) => {
                            state.enabled = false;
                            state.availability = CommandAvailability::Restricted;
                        }
                    }
                }
                if matches!(button.command_type, CommandType::QueueUpgrade)
                    && !command_button_science_vec_owned(
                        &crate_has,
                        &self.science_state.unlocked_sciences,
                        &button.sciences_ids,
                        &button.sciences,
                    )
                {
                    state.enabled = false;
                    state.availability = CommandAvailability::Restricted;
                }

            }
        }

        self.button_states = refreshed;
    }


    pub fn get_context(&self) -> Arc<RwLock<ControlBarContext>> {
        self.context.clone()
    }

    pub fn get_build_queue_data(&self) -> &[BuildQueueEntry] {
        &self.build_queue_data
    }

    pub fn get_button_state(&self, command_name: &str) -> Option<&ButtonState> {
        self.button_states.get(command_name)
    }

    pub fn is_ui_dirty(&self) -> bool {
        self.ui_dirty
    }

    pub fn get_current_state(&self) -> ControlBarState {
        self.context
            .read()
            .map(|c| c.current_state)
            .unwrap_or(ControlBarState::None)
    }
}

#[cfg(test)]
mod science_vec_gate_tests {
    use super::*;

    #[test]
    fn science_vec_restricted_until_host_or_crate_owns() {
        // C++ ControlBarCommand.cpp:1219-1261
        let required_ids = [42];
        let required_names = vec!["SCIENCE_StealthFighter".to_string()];
        assert!(!command_button_science_vec_owned(
            |_| false,
            &[],
            &required_ids,
            &required_names
        ));
        assert!(command_button_science_vec_owned(
            |st| st == 42,
            &[],
            &required_ids,
            &required_names
        ));
        assert!(command_button_science_vec_owned(
            |_| false,
            &["SCIENCE_StealthFighter".to_string()],
            &required_ids,
            &required_names
        ));
    }

    #[test]
    fn show_purchase_science_unhides_gen_exp_parent_and_sets_fade() {
        // C++ ControlBar.cpp:2918-2933 showPurchaseScience — winHide(FALSE)
        // on GeneralsExpPoints.wnd:GenExpParent + setGroup("GenExpFade").
        let _ = leftover_take_last_science_transition_group();
        let mut bar = ControlBar::new();
        bar.show_purchase_science();
        assert!(
            leftover_find_window(GEN_EXP_PARENT).is_some(),
            "showPurchaseScience must bind GeneralsExpPoints.wnd:GenExpParent"
        );
        assert!(
            !leftover_window_is_hidden(GEN_EXP_PARENT),
            "showPurchaseScience must winHide(FALSE) GenExpParent"
        );
        assert!(bar.science_state.is_visible);
        assert_eq!(
            leftover_take_last_science_transition_group(),
            Some("GenExpFade")
        );

        bar.hide_purchase_science();
        assert!(
            leftover_window_is_hidden(GEN_EXP_PARENT),
            "hidePurchaseScience must winHide(TRUE) GenExpParent"
        );
        assert!(!bar.science_state.is_visible);
    }

    #[test]
    fn progress_bar_experience_uses_live_host_skill() {
        // C++ ControlBar::updateContextPurchaseScience from live local skill.
        let mut bar = ControlBar::new();
        bar.sync_rank_progress_from_presentation(50, 1150, 2, 3);
        assert_eq!(bar.science_state.live_rank_progress_percent, Some(50));
        assert!((bar.science_state.experience_progress - 0.5).abs() < 0.001);
        assert_eq!(bar.science_state.available_points, 3);
        assert_eq!(bar.science_state.rank_level, 2);
        assert_eq!(bar.science_state.live_skill_points, Some(1150));
    }

    #[test]
    fn generals_button_and_alt_g_toggle_purchase_science_window() {
        // C++ ControlBarCallback.cpp:396-399 ButtonGeneral → togglePurchaseScience.
        // Host Alt+G routes through TheControlBar::toggle_purchase_science.
        let mut bar = ControlBar::new();
        bar.on_generals_button();
        assert!(!leftover_window_is_hidden(GEN_EXP_PARENT));
        bar.on_generals_button();
        assert!(leftover_window_is_hidden(GEN_EXP_PARENT));
    }
}

