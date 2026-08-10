// Remaining InGameUI placement, hints, modes, movies, and helpers.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn start_building_placement(&mut self, template_name: String, footprint: Vec2) {
        self.placement_preview = Some(PlacementPreview::new(template_name, footprint));
        let builder_id = self.find_selected_builder();
        TheInGameUI::place_build_available(
            self.placement_preview
                .as_ref()
                .map(|preview| preview.template_name.clone()),
            builder_id,
        );
    }

    /// Cancel building placement
    pub fn cancel_building_placement(&mut self) {
        self.placement_preview = None;
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::set_placement_start(None);
    }

    /// Update resources display
    pub fn create_move_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        self.expire_hint_for_source(HintType::Move, source_id);

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

    pub fn create_attack_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        self.expire_hint_for_source(HintType::Attack, source_id);

        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Attack);
        }

        self.hints.push(HintData {
            hint_type: HintType::Attack,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn create_force_attack_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::ForceAttack);
        }

        self.hints.push(HintData {
            hint_type: HintType::ForceAttack,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn create_garrison_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Garrison);
        }

        self.hints.push(HintData {
            hint_type: HintType::Garrison,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn begin_area_select_hint(&mut self) {
        self.hints.push(HintData {
            hint_type: HintType::AreaSelect,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(0.0, 0.0, 0.0),
            creation_frame: self.current_frame,
            source_id: 0,
            lifetime_frames: 300,
        });
    }

    pub fn end_area_select_hint(&mut self) {
        if let Some(pos) = self
            .hints
            .iter()
            .rposition(|h| h.hint_type == HintType::AreaSelect)
        {
            self.hints.remove(pos);
        }
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
    // C++: InGameUI::addNamedTimer() (InGameUI.cpp)
    // C++: InGameUI::removeNamedTimer() (InGameUI.cpp)
    // C++: InGameUI::showNamedTimerDisplay() (InGameUI.cpp)

    pub fn add_named_timer(&mut self, name: &str, text: String, is_countdown: bool) {
        self.named_timers.retain(|t| t.name != name);
        self.named_timers.push(NamedTimerData {
            name: name.to_string(),
            text,
            is_countdown,
        });
    }

    pub fn remove_named_timer(&mut self, name: &str) {
        self.named_timers.retain(|t| t.name != name);
    }

    pub fn show_named_timer_display(&mut self, show: bool) {
        self.show_named_timers = show;
    }

    pub fn get_named_timers(&self) -> &[NamedTimerData] {
        &self.named_timers
    }

    // ── GUI command system ─────────────────────────────────────────────
    // C++: InGameUI::setGUICommand() (InGameUI.cpp:2865)
    // C++: InGameUI::getGUICommand() (InGameUI.cpp:2923)

    pub fn set_gui_command(&mut self, command: Option<String>) {
        self.gui_command = command;
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

    pub fn is_superweapon_hidden_by_script(&self) -> bool {
        self.superweapon_hidden_by_script
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

    pub fn play_movie(&mut self, movie_name: &str) {
        self.stop_movie();
        self.currently_playing_movie = Some(movie_name.to_string());
    }

    pub fn stop_movie(&mut self) {
        if self.currently_playing_movie.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.currently_playing_movie = None;
    }

    pub fn is_movie_playing(&self) -> bool {
        self.currently_playing_movie.is_some()
    }

    pub fn get_currently_playing_movie(&self) -> Option<&str> {
        self.currently_playing_movie.as_deref()
    }

    pub fn play_cameo_movie(&mut self, movie_name: &str) {
        self.stop_cameo_movie();
        self.cameo_movie_playing = Some(movie_name.to_string());
    }

    pub fn stop_cameo_movie(&mut self) {
        if self.cameo_movie_playing.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.cameo_movie_playing = None;
    }

    pub fn is_cameo_movie_playing(&self) -> bool {
        self.cameo_movie_playing.is_some()
    }

    // ── World animations ──────────────────────────────────────────────
    // C++: InGameUI.cpp:5257 (addWorldAnimation), 5292 (clearWorldAnimations),
    //       5323 (updateAndDrawWorldAnimations)

}
