// Remaining InGameUI placement, hints, modes, movies, and helpers.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

use crate::video_buffer::VideoBuffer;
use crate::video_player::VideoPlayerInterface;
impl InGameUI {
    pub fn start_building_placement(&mut self, template_name: String, footprint: Vec2) {
        self.placement_preview = Some(PlacementPreview::new(template_name, footprint));
        let builder_id = self.find_selected_builder();
        let name = self
            .placement_preview
            .as_ref()
            .map(|preview| preview.template_name.clone());
        TheInGameUI::place_build_available(name.clone(), builder_id);
        self.place_build_available_icons(name, builder_id);
    }

    /// Cancel building placement
    pub fn cancel_building_placement(&mut self) {
        self.placement_preview = None;
        self.destroy_placement_icons();
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::set_placement_start(None);
    }

    /// Update resources display
    pub fn update_resources(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.resource_display.update(credits, power_available, power_used);
    }

    /// Set local player id used for selection and command routing.
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
    }

    /// Create a move hint line.
    pub fn create_move_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        self.expire_hint_for_source(HintType::Move, source_id);

        // C++ InGameUI.cpp:2152-2160 — single IMMOBILE selection suppresses the hint.
        if self.get_select_count() == 1 {
            if let Some(object_id) = self.get_selection().into_iter().next() {
                if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                    if obj
                        .read()
                        .ok()
                        .map(|guard| guard.is_kind_of(KindOf::Immobile))
                        .unwrap_or(false)
                    {
                        return;
                    }
                }
            }
        }

        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Move);
        }

        self.hints.push(HintData {
            hint_type: HintType::Move,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: MOVE_HINT_LIFETIME_FRAMES,
        });
    }

    /// C++ InGameUI.cpp:2176-2179 — intentionally empty.
    pub fn create_attack_hint(&mut self, _start: Coord3D, _end: Coord3D, _source_id: u32) {}

    /// C++ InGameUI.cpp:2184-2187 — intentionally empty.
    pub fn create_force_attack_hint(&mut self, _start: Coord3D, _end: Coord3D, _source_id: u32) {}

    /// C++ InGameUI.cpp:2192-2199 — notify the target drawable, no hint line.
    pub fn create_garrison_hint(&mut self, _start: Coord3D, _end: Coord3D, source_id: u32) {
        self.notify_drawable_on_selected(source_id);
    }

    fn notify_drawable_on_selected(&self, drawable_id: u32) {
        if drawable_id == 0 {
            return;
        }
        crate::drawable::drawable_manager::with_drawable_manager(|manager| {
            if let Some(drawable) = manager.get_drawable_mut(crate::core::DrawableId(drawable_id)) {
                drawable.set_selected(true);
            }
        });
    }

    /// C++ beginAreaSelectHint — drag-select flag only, no invented hint.
    pub fn begin_area_select_hint(&mut self) {
        self.is_selecting = true;
    }

    pub fn end_area_select_hint(&mut self) {
        self.is_selecting = false;
    }

    /// C++: InGameUI::expireHint() (InGameUI.cpp:3812) — expire a specific hint by type and index.
    pub fn expire_hint(&mut self, hint_type: HintType, hint_index: usize) {
        if hint_index >= self.hints.len() {
            return;
        }
        if self.hints[hint_index].hint_type == hint_type {
            self.hints.remove(hint_index);
        }
    }

    pub fn expire_hints(&mut self) {
        let current_frame = self.current_frame;
        self.hints.retain(|h| Self::hint_is_alive(h, current_frame));
    }

    pub fn clear_hints(&mut self) {
        self.hints.clear();
    }

    pub fn get_hints(&self) -> &[HintData] {
        &self.hints
    }

    fn hint_is_alive(hint: &HintData, current_frame: u32) -> bool {
        current_frame < hint.creation_frame.saturating_add(hint.lifetime_frames)
    }

    fn hint_draws_waypoint_line(hint: &HintData, current_frame: u32) -> bool {
        hint.hint_type == HintType::Command && Self::hint_is_alive(hint, current_frame)
    }

    // ── Named timer system ─────────────────────────────────────────────
    // C++: InGameUI::addNamedTimer() (InGameUI.cpp:709-727)
    // C++: InGameUI::removeNamedTimer() (InGameUI.cpp:731-741)
    // C++: InGameUI::showNamedTimerDisplay() (InGameUI.cpp:745-748)
    // C++: InGameUI::postDraw named-timer block (InGameUI.cpp:3699-3784)
    pub fn add_named_timer(&mut self, name: &str, text: String, is_countdown: bool) {
        crate::gui::ingame_ui::live_hud::add_named_timer(name, &text, is_countdown);
        self.named_timers.retain(|t| t.name != name);
        let remaining = Self::script_counter_value(name).unwrap_or(0);
        self.named_timers.push(NamedTimerData {
            name: name.to_string(),
            text,
            is_countdown,
            timestamp: -1,
            color: self.named_timer_normal_color,
            display_text: String::new(),
            use_ready_font: false,
            remaining_frames: remaining,
            last_tick_frame: 0,
            draw_x: 0.0,
            draw_y: 0.0,
            draw_color: self.named_timer_normal_color,
        });
    }

    pub fn remove_named_timer(&mut self, name: &str) {
        crate::gui::ingame_ui::live_hud::remove_named_timer(name);
        self.named_timers.retain(|t| t.name != name);
    }

    pub fn show_named_timer_display(&mut self, show: bool) {
        crate::gui::ingame_ui::live_hud::show_named_timer_display(show);
        self.show_named_timers = show;
    }

    pub fn get_named_timers(&self) -> &[NamedTimerData] {
        &self.named_timers
    }

    pub fn named_timer_display_shown(&self) -> bool {
        self.show_named_timers
    }

    /// Seed remaining frames when ScriptEngine has not allocated the counter yet.
    pub fn set_named_timer_remaining_frames(&mut self, name: &str, frames: i32) {
        if let Some(timer) = self.named_timers.iter_mut().find(|t| t.name == name) {
            timer.remaining_frames = frames;
            timer.timestamp = -1;
        }
    }

    /// C++ postDraw named-timer block (InGameUI.cpp:3699-3784).
    pub fn update_named_timers(&mut self) {
        if !self.show_named_timers || self.current_frame == 0 {
            return;
        }
        let flash_duration = self.named_timer_flash_duration as i32;
        let mut used_flash = self.named_timer_used_flash_color;
        let mut last_flash = self.named_timer_last_flash_frame;
        let current_frame = self.current_frame;
        let current_frame_i = current_frame as i32;
        let reverse_x = self.named_timer_position.0 >= 0.5;
        let start_x = self.named_timer_position.0 * self.screen_size.x;
        let mut start_y = self.named_timer_position.1 * self.screen_size.y;

        for timer in &mut self.named_timers {
            let script_frames = Self::script_counter_value(&timer.name);
            let (frames_left, last_tick) = Self::resolve_named_timer_frames(
                timer.remaining_frames,
                timer.is_countdown,
                current_frame,
                timer.last_tick_frame,
                script_frames,
            );
            timer.remaining_frames = frames_left;
            timer.last_tick_frame = last_tick;

            let ready_secs = Self::named_timer_ready_seconds(frames_left);
            let value_changed = if timer.is_countdown {
                ready_secs != timer.timestamp
            } else {
                frames_left != timer.timestamp
            };
            if value_changed {
                timer.use_ready_font = timer.is_countdown && ready_secs == 0;
                timer.timestamp = if timer.is_countdown {
                    ready_secs
                } else {
                    frames_left
                };
                timer.display_text =
                    Self::format_named_timer_line(&timer.text, frames_left, timer.is_countdown);
            }

            let draw_color = if timer.is_countdown && ready_secs == 0 && flash_duration != 0 {
                if current_frame_i >= last_flash + flash_duration {
                    used_flash = !used_flash;
                    last_flash = current_frame_i;
                }
                if used_flash {
                    timer.color
                } else {
                    self.named_timer_flash_color
                }
            } else {
                timer.color
            };

            let font_size = if timer.use_ready_font {
                self.named_timer_ready_point_size
            } else {
                self.named_timer_normal_point_size
            };
            let entry = Self::named_timer_draw_layout(
                &timer.display_text,
                start_x,
                start_y,
                reverse_x,
                font_size,
                draw_color,
                timer.use_ready_font,
            );
            timer.draw_x = entry.x;
            timer.draw_y = entry.y;
            timer.draw_color = entry.color;
            start_y -= font_size.max(1) as f32;
        }

        self.named_timer_used_flash_color = used_flash;
        self.named_timer_last_flash_frame = last_flash;

        if let Ok(mut renderer) = self.renderer.try_write() {
            self.draw_named_timers(&mut renderer);
        }
    }

    /// C++ `info->displayString->draw` (InGameUI.cpp:3768-3777).
    pub fn draw_named_timers(&self, renderer: &mut UIRenderer) {
        if !self.show_named_timers {
            return;
        }
        for entry in self.named_timer_draw_entries() {
            let (r, g, b, a) = Self::unpack_argb(entry.color);
            let _ = renderer.draw_text_simple(
                &entry.text,
                Vec2::new(entry.x, entry.y),
                entry.font_point_size.max(1) as f32,
                [
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0,
                ],
            );
        }
    }

    pub fn named_timer_draw_entries(&self) -> Vec<NamedTimerDrawEntry> {
        if !self.show_named_timers {
            return Vec::new();
        }
        self.named_timers
            .iter()
            .filter(|timer| !timer.display_text.is_empty())
            .map(|timer| NamedTimerDrawEntry {
                text: timer.display_text.clone(),
                x: timer.draw_x,
                y: timer.draw_y,
                color: timer.draw_color,
                font_point_size: if timer.use_ready_font {
                    self.named_timer_ready_point_size
                } else {
                    self.named_timer_normal_point_size
                },
                use_ready_font: timer.use_ready_font,
            })
            .collect()
    }

    fn script_counter_value(name: &str) -> Option<i32> {
        gamelogic::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .and_then(|engine| engine.get_counter(name).map(|c| c.value))
            })
    }

    /// ScriptEngine counter wins (InGameUI.cpp:3716). Otherwise residual
    /// countdown decrements once per logic frame (ScriptEngine.cpp:5547-5550).
    fn resolve_named_timer_frames(
        stored: i32,
        is_countdown: bool,
        current_frame: u32,
        last_tick_frame: u32,
        script_frames: Option<i32>,
    ) -> (i32, u32) {
        if let Some(frames) = script_frames {
            return (frames, current_frame);
        }
        if current_frame == last_tick_frame {
            return (stored, last_tick_frame);
        }
        if is_countdown && stored >= 0 {
            (stored - 1, current_frame)
        } else {
            (stored, current_frame)
        }
    }

    /// C++ `readySecs = SECONDS_PER_LOGICFRAME_REAL * framesLeft` (InGameUI.cpp:3718-3720).
    fn named_timer_ready_seconds(frames_left: i32) -> i32 {
        if frames_left > 0 {
            (frames_left as f32 * (1.0 / 30.0)) as i32
        } else {
            0
        }
    }

    fn named_timer_draw_layout(
        display_text: &str,
        start_x: f32,
        start_y: f32,
        reverse_x: bool,
        font_point_size: i32,
        color: u32,
        use_ready_font: bool,
    ) -> NamedTimerDrawEntry {
        let font_size = font_point_size.max(1) as f32;
        let text_width = display_text.len() as f32 * font_size * 0.6;
        NamedTimerDrawEntry {
            text: display_text.to_string(),
            x: if reverse_x {
                start_x - text_width
            } else {
                start_x
            },
            y: start_y,
            color,
            font_point_size,
            use_ready_font,
        }
    }

    fn format_named_timer_line(text: &str, frames_left: i32, is_countdown: bool) -> String {
        if !is_countdown {
            return format!("{text} {frames_left}");
        }
        let seconds = Self::named_timer_ready_seconds(frames_left);
        let min = seconds / 60;
        let sec = seconds - min * 60;
        if sec >= 10 {
            format!("{text} {min}:{sec}")
        } else {
            format!("{text} {min}:0{sec}")
        }
    }


    // ── GUI command system ─────────────────────────────────────────────
    // C++: InGameUI::setGUICommand() (InGameUI.cpp:2865)
    // C++: InGameUI::getGUICommand() (InGameUI.cpp:2923)

    pub fn command_option_need_target(options: u32) -> bool {
        options & COMMAND_OPTION_NEED_TARGET != 0
    }

    pub fn should_skip_gui_command_during_playback(playback_active: bool) -> bool {
        playback_active
    }

    pub fn mouse_mode_for_gui_command(has_command: bool, needs_target: bool) -> MouseMode {
        if has_command && needs_target {
            MouseMode::GuiCommand
        } else {
            MouseMode::Default
        }
    }

    pub fn set_gui_command(&mut self, command: Option<String>) {
        // C++ InGameUI::setGUICommand (InGameUI.cpp:2865-2918)
        if Self::should_skip_gui_command_during_playback(
            self.recorder_playback_active
                || with_recorder(|recorder| recorder.is_playback()).unwrap_or(false),
        ) {
            return;
        }

        let pending = TheInGameUI::get_pending_command();
        // String-only leftover API: no pending options means the caller already
        // chose a targeting command. Default to NEED_TARGET_POS (non-context).
        let options = pending
            .as_ref()
            .map(|cmd| cmd.options)
            .unwrap_or(if command.is_some() { 0x0000_0020 } else { 0 });
        let needs_target = Self::command_option_need_target(options);

        if command.is_some() && !needs_target {
            self.gui_command = None;
            self.set_mouse_mode(MouseMode::Default);
            TheInGameUI::set_mouse_mode(MouseMode::Default);
            return;
        }

        let mode = Self::mouse_mode_for_gui_command(command.is_some(), needs_target);
        self.set_mouse_mode(mode);
        TheInGameUI::set_mouse_mode(mode);
        self.gui_command = command;

        let is_context = options & CMD_CONTEXTMODE_COMMAND != 0;
        self.set_mouse_cursor(MouseCursor::Arrow);
        TheInGameUI::set_mouse_cursor(MouseCursor::Arrow);
        if self.gui_command.is_some() && needs_target && !is_context {
            if let Some(pending) = pending {
                if let Some(cursor_type) =
                    RadiusCursorType::from_name(&pending.radius_cursor_type)
                {
                    if cursor_type != RadiusCursorType::None {
                        TheInGameUI::set_radius_cursor_active_with_type(
                            &pending.radius_cursor_type,
                        );
                        self.set_radius_cursor(
                            cursor_type,
                            Coord3D::new(0.0, 0.0, 0.0),
                            1.0,
                        );
                    }
                }
            }
        } else {
            self.clear_radius_cursor();
            TheInGameUI::set_radius_cursor_none();
        }
        self.mouse_mode_cursor = self.current_cursor;
    }


    pub fn get_gui_command(&self) -> Option<&String> {
        self.gui_command.as_ref()
    }

    // ── Quit menu ──────────────────────────────────────────────────────
    // C++: InGameUI::setQuitMenuVisible() (InGameUI.h:460)
    // C++: InGameUI::isQuitMenuVisible() (InGameUI.h:461)

    pub fn set_quit_menu_visible(&mut self, visible: bool) {
        self.quit_menu_visible = visible;
    }

    pub fn is_quit_menu_visible(&self) -> bool {
        self.quit_menu_visible
    }

    // ── Window layout registration ─────────────────────────────────────
    // C++: InGameUI::registerWindowLayout() (InGameUI.cpp)
    // C++: InGameUI::unregisterWindowLayout() (InGameUI.cpp)

    pub fn register_window_layout(&mut self, name: &str) {
        self.window_layouts.insert(name.to_string(), true);
    }

    pub fn unregister_window_layout(&mut self, name: &str) {
        self.window_layouts.remove(name);
    }

    pub fn is_window_layout_registered(&self, name: &str) -> bool {
        self.window_layouts.get(name).copied().unwrap_or(false)
    }

    // ── Region builder helper ──────────────────────────────────────────

    pub fn build_region(&self, x: f32, y: f32, width: f32, height: f32) -> IRegion2D {
        IRegion2D::new(
            ICoord2D::new(x as i32, y as i32),
            ICoord2D::new((x + width) as i32, (y + height) as i32),
        )
    }

    fn expire_hint_for_source(&mut self, hint_type: HintType, source_id: u32) {
        if source_id == 0 {
            return;
        }
        self.hints
            .retain(|h| !(h.hint_type == hint_type && h.source_id == source_id));
    }

    fn expire_oldest_hint(&mut self, hint_type: HintType) {
        if let Some(pos) = self.hints.iter().position(|h| h.hint_type == hint_type) {
            self.hints.remove(pos);
        }
    }

    // ── Draw pipeline: main render entry ─────────────────────────────
    // C++: InGameUI::draw() (pure virtual in C++, implemented in subclasses)
    // C++ calls drawSelectionAnims(), drawPlacementCursor(), drawRadarBall(), etc.

    /// Main draw pipeline. Renders selection indicators, placement cursor,
    /// team/waypoint overlay lines, and minimap ping animations.
    /// C++: InGameUI::draw() — pure virtual, subclass calls sub-draw methods.
    pub fn set_placement_template(&mut self, template: Option<String>) {
        match template {
            Some(name) => {
                let footprint = TheThingFactory::find_template(&name)
                    .map(|t| {
                        let info = t.get_template_geometry_info();
                        let half_w = (info.bounds.max.x - info.bounds.min.x) / 2.0;
                        let half_h = (info.bounds.max.y - info.bounds.min.y) / 2.0;
                        Vec2::new(half_w.abs().max(1.0), half_h.abs().max(1.0))
                    })
                    .unwrap_or(Vec2::new(1.0, 1.0));
                self.start_building_placement(name, footprint);
            }
            None => {
                self.cancel_building_placement();
            }
        }
    }

    /// Get the current placement template name. C++: getPendingPlaceType()
    pub fn get_placement_template(&self) -> Option<&str> {
        self.placement_preview
            .as_ref()
            .map(|p| p.template_name.as_str())
    }

    /// Check if placement is legal at the given position.
    /// C++: BuildAssistant::isLocationLegalToBuild() with quick checks.
    /// Validates terrain passability, object overlap, and proximity.
    pub fn can_place_at(&self, pos: &Coord3D) -> bool {
        let Some(ref preview) = self.placement_preview else {
            return false;
        };

        let validator = FoundationValidator::new_strict();
        validator
            .validate_placement(
                pos,
                &preview.template_name,
                preview.rotation,
                self.player_id as ObjectID,
            )
            .is_ok()
    }

    /// Set/get superweapon display hidden by script.
    /// C++: setSuperweaponDisplayEnabledByScript() / getSuperweaponDisplayEnabledByScript()
    pub fn set_superweapon_hidden_by_script(&mut self, hidden: bool) {
        self.superweapon_hidden_by_script = hidden;
    }

    pub fn set_superweapon_display_enabled_by_script(&mut self, enabled: bool) {
        self.superweapon_hidden_by_script = !enabled;
    }

    pub fn is_superweapon_hidden_by_script(&self) -> bool {
        self.superweapon_hidden_by_script
    }

    pub fn hide_object_superweapon_display_by_script(&mut self, object_id: ObjectID) {
        for timer in &mut self.superweapon_timers {
            if timer.object_id == object_id {
                timer.hidden_by_script = true;
            }
        }
    }

    pub fn show_object_superweapon_display_by_script(&mut self, object_id: ObjectID) {
        for timer in &mut self.superweapon_timers {
            if timer.object_id == object_id {
                timer.hidden_by_script = false;
            }
        }
    }

    /// Render the in-game UI
    pub fn set_force_attack_mode(&mut self, enabled: bool) {
        self.force_attack_mode = enabled;
    }

    pub fn is_in_force_attack_mode(&self) -> bool {
        self.force_attack_mode
    }

    pub fn set_force_move_to_mode(&mut self, enabled: bool) {
        self.force_move_to_mode = enabled;
    }

    pub fn is_in_force_move_to_mode(&self) -> bool {
        self.force_move_to_mode
    }

    pub fn toggle_attack_move_to_mode(&mut self) -> bool {
        self.attack_move_to_mode = !self.attack_move_to_mode;
        self.attack_move_to_mode
    }

    pub fn is_in_attack_move_to_mode(&self) -> bool {
        self.attack_move_to_mode
    }

    pub fn clear_attack_move_to_mode(&mut self) {
        self.attack_move_to_mode = false;
    }

    pub fn set_waypoint_mode(&mut self, enabled: bool) {
        self.waypoint_mode = enabled;
    }

    pub fn is_in_waypoint_mode(&self) -> bool {
        self.waypoint_mode
    }

    pub fn set_prefer_selection_mode(&mut self, enabled: bool) {
        self.prefer_selection_mode = enabled;
    }

    pub fn is_in_prefer_selection_mode(&self) -> bool {
        self.prefer_selection_mode
    }

    // ── Camera control methods ─────────────────────────────────────────
    // C++: InGameUI.h:521-531

    pub fn set_camera_rotate_left(&mut self, set: bool) {
        self.camera_rotating_left = set;
    }

    pub fn is_camera_rotating_left(&self) -> bool {
        self.camera_rotating_left
    }

    pub fn set_camera_rotate_right(&mut self, set: bool) {
        self.camera_rotating_right = set;
    }

    pub fn is_camera_rotating_right(&self) -> bool {
        self.camera_rotating_right
    }

    pub fn set_camera_zoom_in(&mut self, set: bool) {
        self.camera_zooming_in = set;
    }

    pub fn is_camera_zooming_in(&self) -> bool {
        self.camera_zooming_in
    }

    pub fn set_camera_zoom_out(&mut self, set: bool) {
        self.camera_zooming_out = set;
    }

    pub fn is_camera_zooming_out(&self) -> bool {
        self.camera_zooming_out
    }

    pub fn set_camera_tracking_drawable(&mut self, set: bool) {
        self.camera_tracking_drawable = set;
    }

    pub fn is_camera_tracking_drawable(&self) -> bool {
        self.camera_tracking_drawable
    }

    // ── Selection query methods ────────────────────────────────────────
    // C++: InGameUI.cpp:4116 (areSelectedObjectsControllable)
    // C++: InGameUI.cpp:3333 (isAnySelectedKindOf)
    // C++: InGameUI.cpp:3357 (isAllSelectedKindOf)

    pub fn radar_movie_window_name() -> &'static str {
        "ControlBar.wnd:LeftHUD"
    }

    pub fn play_movie(&mut self, movie_name: &str) {
        self.stop_movie();
        let Some(player) = crate::video_player::get_video_player() else {
            return;
        };
        let Ok(mut guard) = player.lock() else {
            return;
        };
        let Some(player) = guard.as_mut() else {
            return;
        };
        let Some(stream) = player.open(movie_name.to_string()) else {
            return;
        };
        let mut buffer = crate::video_buffer::SoftwareVideoBuffer::new(
            crate::video_buffer::VideoBufferType::X8R8G8B8,
        );
        if !buffer.allocate(stream.width() as u32, stream.height() as u32) {
            stream.close();
            return;
        }
        self.video_stream = Some(stream);
        self.video_buffer = Some(crate::video_buffer::VideoBufferHandle::new(buffer));
        self.currently_playing_movie = Some(movie_name.to_string());
        self.attach_window_video_buffer(
            Self::radar_movie_window_name(),
            self.video_buffer.clone(),
        );
    }


    pub fn stop_movie(&mut self) {
        self.attach_window_video_buffer(Self::radar_movie_window_name(), None);
        if let Some(stream) = self.video_stream.take() {
            stream.close();
        }
        self.video_buffer = None;
        if self.currently_playing_movie.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.currently_playing_movie = None;
    }


    pub fn is_movie_playing(&self) -> bool {
        self.currently_playing_movie.is_some() && self.video_stream.is_some()
    }

    pub fn get_currently_playing_movie(&self) -> Option<&str> {
        self.currently_playing_movie.as_deref()
    }

    pub fn play_cameo_movie(&mut self, movie_name: &str) {
        self.stop_cameo_movie();
        let Some(player) = crate::video_player::get_video_player() else {
            return;
        };
        let Ok(mut guard) = player.lock() else {
            return;
        };
        let Some(player) = guard.as_mut() else {
            return;
        };
        let Some(stream) = player.open(movie_name.to_string()) else {
            return;
        };
        let mut buffer = crate::video_buffer::SoftwareVideoBuffer::new(
            crate::video_buffer::VideoBufferType::X8R8G8B8,
        );
        if !buffer.allocate(stream.width() as u32, stream.height() as u32) {
            stream.close();
            return;
        }
        let handle = crate::video_buffer::VideoBufferHandle::new(buffer);
        self.attach_cameo_video_buffer(Some(handle.clone()));
        self.cameo_video_stream = Some(stream);
        self.cameo_video_buffer = Some(handle);
        self.cameo_movie_playing = Some(movie_name.to_string());
    }

    pub fn stop_cameo_movie(&mut self) {
        self.attach_cameo_video_buffer(None);
        if let Some(stream) = self.cameo_video_stream.take() {
            stream.close();
        }
        self.cameo_video_buffer = None;
        if self.cameo_movie_playing.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.cameo_movie_playing = None;
    }

    pub fn is_cameo_movie_playing(&self) -> bool {
        self.cameo_movie_playing.is_some() && self.cameo_video_stream.is_some()
    }

    fn attach_window_video_buffer(
        &self,
        window_name: &str,
        buffer: Option<crate::video_buffer::VideoBufferHandle>,
    ) {
        let key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            window_name,
        );
        with_window_manager_ref(|manager| {
            if let Some(window) = manager.get_window_by_id(key as i32) {
                window.borrow_mut().set_video_buffer(buffer.clone());
            }
        });
    }

    fn attach_cameo_video_buffer(&self, buffer: Option<crate::video_buffer::VideoBufferHandle>) {
        self.attach_window_video_buffer("ControlBar.wnd:RightHUD", buffer);
    }

    pub fn video_buffer(&self) -> Option<crate::video_buffer::VideoBufferHandle> {
        self.video_buffer.clone()
    }

    pub fn popup_message(
        &mut self,
        identifier: &str,
        x: i32,
        y: i32,
        width: i32,
        pause: bool,
        pause_music: bool,
    ) {
        // C++ InGameUI::popupMessage always UpdateDiplomacyBriefingText first.
        TheInGameUI::popup_message(identifier, x, y, width, pause, pause_music);
    }


    fn update_movie_playback(&mut self) {
        let mut stop_main = false;
        if let Some(stream) = self.video_stream.as_mut() {
            if stream.is_frame_ready() {
                stream.frame_decompress();
                if let Some(buffer) = self.video_buffer.as_ref() {
                    let mut guard = buffer.lock();
                    stream.frame_render(&mut *guard);
                }
                stream.frame_next();
                if stream.frame_index() == 0 {
                    stop_main = true;
                }
            }
        }
        if stop_main {
            self.stop_movie();
        }

        if let Some(stream) = self.cameo_video_stream.as_mut() {
            if stream.is_frame_ready() {
                stream.frame_decompress();
                if let Some(buffer) = self.cameo_video_buffer.as_ref() {
                    let mut guard = buffer.lock();
                    stream.frame_render(&mut *guard);
                }
                stream.frame_next();
            }
        }
    }

    // ── World animations ──────────────────────────────────────────────
    // C++: InGameUI.cpp:5257 (addWorldAnimation), 5292 (clearWorldAnimations),
    //       5323 (updateAndDrawWorldAnimations)

}
