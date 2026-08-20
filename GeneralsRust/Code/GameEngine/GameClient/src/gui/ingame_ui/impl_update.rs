// InGameUI construction, update/draw/render, settings, and lifecycle.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn new(renderer: Arc<RwLock<UIRenderer>>, screen_width: f32, screen_height: f32) -> Self {
        let minimap_size = 200.0;
        let minimap_margin = 10.0;

        Self {
            selection_box: SelectionBox::new(),
            selection_state: SelectionState::new(MAX_SELECTION_COUNT),
            presentation_selected: Vec::new(),
            presentation_unit_catalog: Vec::new(),
            presentation_local_team_name: String::new(),
            placement_preview: None,
            place_icons: Vec::new(),
            pending_place_icon_template: None,
            pending_place_icon_source: 0,
            minimap: Minimap::new(
                Vec2::new(
                    screen_width - minimap_size - minimap_margin,
                    screen_height - minimap_size - minimap_margin,
                ),
                Vec2::new(minimap_size, minimap_size),
            ),
            resource_display: ResourceDisplay::new(Vec2::new(10.0, 10.0)),
            renderer,
            screen_size: Vec2::new(screen_width, screen_height),
            enabled: true,
            player_id: 0,
            ui_time: 0.0,
            last_update: Instant::now(),
            floating_texts: Vec::new(),
            idle_workers: Vec::new(),
            current_frame: 0,
            radius_cursor: RadiusCursorState::new(),
            radius_cursor_templates: Self::radius_cursor_templates_from_ini(),
            cur_radius_decal: crate::radius_decal::RadiusDecal::new(),
            guard_hint_stashed_position: Coord3D::new(0.0, 0.0, 0.0),
            superweapon_timers: Vec::new(),
            mouse_mode: MouseMode::Default,
            current_cursor: MouseCursor::Arrow,
            mouse_mode_cursor: MouseCursor::Arrow,
            is_scrolling: false,
            is_selecting: false,
            scroll_amount_x: 0.0,
            scroll_amount_y: 0.0,
            moused_over_drawable_id: 0,
            hints: Vec::new(),
            next_hint_index: 0,
            named_timers: Vec::new(),
            named_timer_last_flash_frame: 0,
            named_timer_used_flash_color: true,
            show_named_timers: true,
            named_timer_position: (0.05, 0.7),
            named_timer_flash_duration: 1.0,
            named_timer_flash_color: 0xFF00_FFFF,
            named_timer_normal_font: "Arial".to_string(),
            named_timer_normal_point_size: 10,
            named_timer_normal_bold: false,
            named_timer_normal_color: 0xFFFF_FF00,
            named_timer_ready_font: "Arial".to_string(),
            named_timer_ready_point_size: 10,
            named_timer_ready_bold: false,
            named_timer_ready_color: 0xFFFF_00FF,
            gui_command: None,
            quit_menu_visible: false,
            window_layouts: HashMap::new(),

            // Message display defaults (C++ constructor: InGameUI.cpp:899-906)
            message_color1: 0xFFFFFFFF, // GameMakeColor(255,255,255,255)
            message_color2: 0xFFB4B4B4, // GameMakeColor(180,180,180,255)
            message_position: (10, 10),
            message_font_name: "Arial".to_string(),
            message_point_size: 10,
            message_bold: false,
            message_delay_ms: 5000,
            messages_enabled: true, // C++: m_messagesOn = TRUE
            messages: Vec::new(),

            // Military caption defaults (C++ constructor: InGameUI.cpp:908-924)
            military_caption_color: (200, 200, 30, 255),
            military_caption_position: (10, 380),
            military_caption_title_font: "Courier".to_string(),
            military_caption_title_point_size: 12,
            military_caption_title_bold: true,
            military_caption_font: "Courier".to_string(),
            military_caption_point_size: 12,
            military_caption_bold: false,
            military_caption_randomize_typing: false,
            military_caption_speed: 1,
            current_military_subtitle: None,
            tooltips_disabled_until: 0,

            // Floating text INI defaults (C++ constructor: InGameUI.cpp:1013-1015)
            floating_text_timeout_frames: DEFAULT_FLOATING_TEXT_TIMEOUT,
            floating_text_move_up_speed: 1.0,
            floating_text_vanish_rate: 0.1,

            // Superweapon countdown defaults (C++ constructor: InGameUI.cpp:980-992)
            superweapon_countdown_position: (0.7, 0.7),
            superweapon_flash_duration: 1.0,
            superweapon_flash_color: 0xFFFFFFFF,
            superweapon_normal_font: "Arial".to_string(),
            superweapon_normal_point_size: 10,
            superweapon_normal_bold: false,
            superweapon_ready_font: "Arial".to_string(),
            superweapon_ready_point_size: 10,
            superweapon_ready_bold: false,
            superweapon_last_flash_frame: 0,
            superweapon_used_flash_color: true,

            // Popup message defaults (C++ constructor: InGameUI.cpp:925)
            popup_message_color: 0xFFFFFFFF,

            // Drawable caption defaults (C++ constructor: InGameUI.cpp:1017-1020)
            drawable_caption_font: "Arial".to_string(),
            drawable_caption_point_size: 10,
            drawable_caption_bold: false,
            drawable_caption_color: 0xFFFFFFFF,

            // Scroll anchors (C++ constructor: InGameUI.cpp:1022-1023)
            draw_rmb_scroll_anchor: false,
            move_rmb_scroll_anchor: false,

            // Combat modes (C++ constructor: InGameUI.cpp)
            waypoint_mode: false,
            force_attack_mode: false,
            force_move_to_mode: false,
            attack_move_to_mode: false,
            prefer_selection_mode: false,

            // Camera control (C++ constructor: InGameUI.cpp)
            camera_rotating_left: false,
            camera_rotating_right: false,
            camera_zooming_in: false,
            camera_zooming_out: false,
            camera_tracking_drawable: false,

            // Selection tracking
            frame_selection_changed: 0,
            double_click_attack_move_guard_timer: 0,

            // Movie playback (C++ constructor: InGameUI.cpp)
            currently_playing_movie: None,
            cameo_movie_playing: None,
            video_stream: None,
            video_buffer: None,
            cameo_video_stream: None,
            cameo_video_buffer: None,
            displayed_max_warning: false,
            last_ui_logic_frame: u32::MAX,

            // World animations
            world_animations: Vec::new(),

            // C++: m_superweaponHiddenByScript = FALSE (InGameUI.cpp:993)
            superweapon_hidden_by_script: false,

            // Minimap pings
            minimap_pings: Vec::new(),

            recorder_playback_active: false,
            look_at_mouse_moved_recently: true,
        }
    }

    /// Handle mouse input for selection box
    pub fn add_superweapon_timer(
        &mut self,
        player_index: u8,
        object_id: ObjectID,
        power_name: String,
        ready_frame: u32,
    ) {
        self.add_superweapon(player_index, object_id, power_name, None, ready_frame);
    }

    /// C++ InGameUI::addSuperweapon (InGameUI.cpp:548-580).
    pub fn add_superweapon(
        &mut self,
        player_index: u8,
        object_id: ObjectID,
        power_name: String,
        template_name: Option<String>,
        ready_frame: u32,
    ) {
        let existing = self.superweapon_timers.iter().any(|t| {
            t.player_index == player_index && t.power_name == power_name && t.object_id == object_id
        });
        if existing {
            return;
        }
        let template_name = template_name.unwrap_or_else(|| power_name.clone());
        let (hidden_by_science, color) =
            Self::superweapon_science_and_color(player_index, &template_name);
        self.superweapon_timers.push(SuperweaponTimerData {
            player_index,
            object_id,
            power_name,
            template_name,
            ready_frame,
            countdown_text: String::new(),
            ready: false,
            hidden_by_script: false,
            hidden_by_science,
            timestamp: -1,
            eva_ready_played: false,
            color,
            force_update_text: true,
            name_text: String::new(),
            time_text: String::new(),
            use_ready_font: false,
        });
    }

    fn superweapon_science_and_color(player_index: u8, template_name: &str) -> (bool, u32) {
        let player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index as i32).cloned());
        let color = player
            .as_ref()
            .and_then(|p| p.read().ok().map(|g| g.get_player_color()))
            .map(|c| {
                ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
            })
            .unwrap_or(0xFFFFFFFF);
        let hidden_by_science = get_special_power_store()
            .and_then(|store| store.find_special_power_template(template_name).cloned())
            .map(|template| {
                let science = template.get_required_science();
                science != game_engine::common::rts::SCIENCE_INVALID
                    && player
                        .as_ref()
                        .and_then(|p| p.read().ok().map(|g| !g.has_science(science)))
                        .unwrap_or(true)
            })
            .unwrap_or(false);
        (hidden_by_science, color)
    }

    pub fn remove_superweapon_timer(
        &mut self,
        player_index: u8,
        object_id: ObjectID,
        power_name: &str,
    ) -> bool {
        let before = self.superweapon_timers.len();
        self.superweapon_timers.retain(|t| {
            !(t.player_index == player_index
                && t.object_id == object_id
                && t.power_name == power_name)
        });
        self.superweapon_timers.len() < before
    }

    /// C++ InGameUI::objectChangedTeam (InGameUI.cpp:612-651).
    pub fn object_changed_team(&mut self, object_id: ObjectID, old_player: i32, new_player: i32) {
        if old_player < 0 || new_player < 0 {
            return;
        }
        let old_idx = old_player as u8;
        let new_idx = new_player as u8;
        let moved: Vec<SuperweaponTimerData> = self
            .superweapon_timers
            .iter()
            .filter(|t| t.object_id == object_id && t.player_index == old_idx)
            .cloned()
            .collect();
        for timer in &moved {
            self.remove_superweapon_timer(old_idx, object_id, &timer.power_name);
            self.add_superweapon(
                new_idx,
                object_id,
                timer.power_name.clone(),
                Some(timer.template_name.clone()),
                timer.ready_frame,
            );
        }
        if !moved.is_empty() {
            return;
        }
        if TheGameLogic::get_frame() != 0 {
            return;
        }
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(guard) = obj.read() else {
            return;
        };
        if guard.test_status(gamelogic::common::ObjectStatusTypes::UnderConstruction)
            || guard.is_kind_of(KindOf::CommandCenter)
        {
            return;
        }
        for behavior in guard.get_behavior_modules() {
            let Ok(module) = behavior.lock() else {
                continue;
            };
            let Some(sp) = module.get_special_power_module_interface_const() else {
                continue;
            };
            let power_name = sp.get_power_name().to_string();
            if power_name.is_empty() {
                continue;
            }
            drop(module);
            self.add_superweapon(new_idx, object_id, power_name, None, 0);
        }
    }

    /// C++ postDraw SW block state (InGameUI.cpp:3487-3697).
    pub fn update_superweapon_timers(&mut self, current_frame: u32) {
        const LOGICFRAMES_PER_SECOND: u32 = 30;
        if current_frame == 0 {
            return;
        }
        let mut flash_toggled = false;
        let mut i = 0;
        let mut shared_seen: Vec<(u8, String)> = Vec::new();
        while i < self.superweapon_timers.len() {
            let player_index = self.superweapon_timers[i].player_index;
            let power_name = self.superweapon_timers[i].power_name.clone();
            if self.superweapon_timers[i].hidden_by_script
                || self.superweapon_timers[i].hidden_by_science
            {
                i += 1;
                continue;
            }

            let object_id = self.superweapon_timers[i].object_id;
            let template_name = self.superweapon_timers[i].template_name.clone();
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                i += 1;
                continue;
            };
            let Ok(guard) = obj.read() else {
                i += 1;
                continue;
            };
            if guard.test_status(gamelogic::common::ObjectStatusTypes::UnderConstruction) {
                i += 1;
                continue;
            }

            let lookup_name = if template_name.is_empty() {
                power_name.as_str()
            } else {
                template_name.as_str()
            };
            let (is_ready, ready_frame, power_type, shared) = guard
                .with_special_power_module_interface_by_name(lookup_name, |sp| {
                    let template = get_special_power_store().and_then(|store| {
                        store.find_special_power_template(lookup_name).cloned()
                    });
                    (
                        sp.is_ready(),
                        sp.get_ready_frame(),
                        template
                            .as_ref()
                            .map(|t| t.get_special_power_type())
                            .unwrap_or(gamelogic::object::special_power_types::SpecialPowerType::Invalid),
                        template
                            .as_ref()
                            .map(|t| t.is_shared_n_sync())
                            .unwrap_or(false),
                    )
                })
                .unwrap_or((
                    false,
                    self.superweapon_timers[i].ready_frame,
                    gamelogic::object::special_power_types::SpecialPowerType::Invalid,
                    false,
                ));
            drop(guard);

            if shared && shared_seen.iter().any(|(p, n)| *p == player_index && n == &power_name) {
                i += 1;
                continue;
            }
            if shared {
                shared_seen.push((player_index, power_name.clone()));
            }

            let ready_secs = if ready_frame < current_frame {
                0
            } else {
                (ready_frame - current_frame) / LOGICFRAMES_PER_SECOND
            } as i32;

            if is_ready && !self.superweapon_timers[i].eva_ready_played {
                Self::announce_superweapon_ready(object_id, power_type);
                self.superweapon_timers[i].eva_ready_played = true;
            } else if !is_ready {
                self.superweapon_timers[i].eva_ready_played = false;
            }

            if self.superweapon_hidden_by_script {
                i += 1;
                continue;
            }

            let change_bolding = ready_secs != self.superweapon_timers[i].timestamp
                || is_ready != self.superweapon_timers[i].ready
                || self.superweapon_timers[i].force_update_text;
            if change_bolding {
                if is_ready {
                    self.superweapon_timers[i].use_ready_font = true;
                } else if self.superweapon_timers[i].timestamp == 0 {
                    self.superweapon_timers[i].use_ready_font = false;
                }
                self.superweapon_timers[i].force_update_text = false;
                self.superweapon_timers[i].ready = is_ready;
                self.superweapon_timers[i].timestamp = ready_secs;
                self.superweapon_timers[i].ready_frame = ready_frame;
                let min = ready_secs / 60;
                let sec = ready_secs - min * 60;
                let label = GameText::fetch(&format!("GUI:{lookup_name}"));
                self.superweapon_timers[i].name_text = format!("{label}: ");
                self.superweapon_timers[i].time_text = format!("{min}:{sec:02}");
                self.superweapon_timers[i].countdown_text =
                    format!("{label}: {min}:{sec:02}");
            }

            if is_ready && self.superweapon_flash_duration != 0.0 && !flash_toggled {
                if current_frame
                    >= self.superweapon_last_flash_frame
                        + self.superweapon_flash_duration as u32
                {
                    self.superweapon_used_flash_color = !self.superweapon_used_flash_color;
                    self.superweapon_last_flash_frame = current_frame;
                    flash_toggled = true;
                }
            }
            i += 1;
        }
    }

    fn announce_superweapon_ready(
        object_id: ObjectID,
        power_type: gamelogic::object::special_power_types::SpecialPowerType,
    ) {
        use gamelogic::object::special_power_types::SpecialPowerType;
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(guard) = obj.read() else {
            return;
        };
        let local = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(local) = local else {
            return;
        };
        let Ok(local_guard) = local.read() else {
            return;
        };
        let own = guard
            .get_controlling_player()
            .and_then(|p| p.read().ok().map(|g| g.get_player_index() == local_guard.get_player_index()))
            .unwrap_or(false);
        let ally = guard
            .get_team()
            .and_then(|team| {
                team.read()
                    .ok()
                    .map(|t| local_guard.get_relationship_with_team(&t) != Relationship::Enemies)
            })
            .unwrap_or(false);
        let message = match power_type {
            SpecialPowerType::ParticleUplinkCannon
            | SpecialPowerType::SupwParticleUplinkCannon
            | SpecialPowerType::LazrParticleUplinkCannon => {
                if own {
                    crate::eva::EvaMessage::SuperweaponReadyOwnParticleCannon
                } else if ally {
                    crate::eva::EvaMessage::SuperweaponReadyAllyParticleCannon
                } else {
                    crate::eva::EvaMessage::SuperweaponReadyEnemyParticleCannon
                }
            }
            SpecialPowerType::NeutronMissile
            | SpecialPowerType::NukeNeutronMissile
            | SpecialPowerType::SupwNeutronMissile => {
                if own {
                    crate::eva::EvaMessage::SuperweaponReadyOwnNuke
                } else if ally {
                    crate::eva::EvaMessage::SuperweaponReadyAllyNuke
                } else {
                    crate::eva::EvaMessage::SuperweaponReadyEnemyNuke
                }
            }
            SpecialPowerType::ScudStorm => {
                if own {
                    crate::eva::EvaMessage::SuperweaponReadyOwnScudStorm
                } else if ally {
                    crate::eva::EvaMessage::SuperweaponReadyAllyScudStorm
                } else {
                    crate::eva::EvaMessage::SuperweaponReadyEnemyScudStorm
                }
            }
            _ => return,
        };
        if let Ok(mut eva) = crate::eva::get_eva().lock() {
            eva.set_should_play(message);
        }
    }

    pub fn get_superweapon_timers(&self) -> &[SuperweaponTimerData] {
        &self.superweapon_timers
    }

    /// Visible SW rows after script/science/construction filters.
    pub fn visible_superweapon_draw_entries(&self) -> Vec<&SuperweaponTimerData> {
        if self.superweapon_hidden_by_script {
            return Vec::new();
        }
        self.superweapon_timers
            .iter()
            .filter(|t| !t.hidden_by_script && !t.hidden_by_science)
            .collect()
    }

    // ── Mouse cursor system ──────────────────────────────────────────────
    // C++: InGameUI::setMouseCursor() (InGameUI.cpp:516-525)

    // ── Mouse cursor system ──────────────────────────────────────────────
    // C++: InGameUI::setMouseCursor() (InGameUI.cpp:516-525)

    pub fn draw(&mut self) -> std::result::Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| "Failed to lock renderer".to_string())?;

        self.draw_selection_anims(&mut renderer)?;

        if let Some(ref preview) = self.placement_preview {
            let pos = Coord3D::new(preview.position.x, preview.position.y, preview.position.z);
            self.draw_placement_cursor(&mut renderer, &pos)?;
        }

        self.draw_team_waypoint_lines(&mut renderer)?;
        self.draw_minimap_pings(&mut renderer)?;

        if self.selection_box.active && self.selection_box.is_significant() {
            self.render_selection_box(&mut renderer)
                .map_err(|e| e.to_string())?;
        }

        if self.minimap.visible {
            self.render_minimap(&mut renderer)
                .map_err(|e| e.to_string())?;
        }

        self.render_resources(&mut renderer)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// C++: drawSelectionAnims() — iterates selected drawables, draws
    /// health bars (green/yellow/red based on HP), veterancy pips, and
    /// selection rings on terrain.
    /// Wave 964: selection health bars from presentation residual (no OBJECT_REGISTRY).
    fn draw_selection_anims_from_presentation(
        &self,
        renderer: &mut UIRenderer,
    ) -> std::result::Result<(), String> {
        for u in &self.presentation_selected {
            // Wave 1040: skip illegal selection HUD residuals.
            if u.destroyed || u.sold || u.unselectable || u.masked || u.health_pct <= 0.0 {
                continue;
            }
            // Wave 1113: disabled + non-local FOW selection HUD residual via catalog.
            let local = crate::presentation_translator_residual::translator_local_team_name();
            let is_local = !local.is_empty() && u.team_name == local;
            if let Some(entry) = self
                .presentation_unit_catalog
                .iter()
                .find(|e| e.object_id == u.object_id)
            {
                if entry.disabled {
                    continue;
                }
                if entry.effectively_stealthed && !is_local {
                    continue;
                }
                let fogged = matches!(
                    entry.shroud_status,
                    ObjectShroudStatus::PartialClear
                        | ObjectShroudStatus::Fogged
                        | ObjectShroudStatus::Shrouded
                );
                if fogged && !is_local {
                    continue;
                }
            } else if u.effectively_stealthed && !is_local {
                continue;
            }
            let world = Coord3D::new(u.position[0], u.position[1], u.position[2]);
            let Some(screen) = self.world_to_screen(&world) else {
                continue;
            };
            let health_pct = u.health_pct.clamp(0.0, 1.0);
            if health_pct <= 0.0 {
                continue;
            }
            let bar_width = 40.0;
            let bar_height = 4.0;
            let bar_x = screen.x - bar_width / 2.0;
            let bar_y = screen.y - 30.0;

            renderer
                .draw_rect_with_scissor(
                    UIRect::new(bar_x, bar_y, bar_width, bar_height),
                    [0.2, 0.2, 0.2, 0.7],
                    None,
                )
                .map_err(|e| e.to_string())?;

            let fill_color = if health_pct > 0.66 {
                [0.0, 1.0, 0.0, 0.9]
            } else if health_pct > 0.33 {
                [1.0, 1.0, 0.0, 0.9]
            } else {
                [1.0, 0.0, 0.0, 0.9]
            };

            renderer
                .draw_rect_with_scissor(
                    UIRect::new(bar_x, bar_y, bar_width * health_pct, bar_height),
                    fill_color,
                    None,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn draw_selection_anims(&self, renderer: &mut UIRenderer) -> std::result::Result<(), String> {
        // Wave 964: host empty dual-world → draw from presentation selection residual.
        if dual_world_registry_unavailable() {
            return self.draw_selection_anims_from_presentation(renderer);
        }

        let selected = self.get_selection();
        for &obj_id in &selected {
            let obj = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(o) => o,
                None => continue,
            };
            let guard = match obj.read() {
                Ok(g) => g,
                Err(_) => continue,
            };

            let pos = guard.get_position();
            let world = Coord3D::new(pos.x, pos.y, pos.z);
            let Some(screen) = self.world_to_screen(&world) else {
                continue;
            };

            let health_pct = guard.get_health_percentage();
            if health_pct > 0.0 {
                let bar_width = 40.0;
                let bar_height = 4.0;
                let bar_x = screen.x - bar_width / 2.0;
                let bar_y = screen.y - 30.0;

                renderer
                    .draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_width, bar_height),
                        [0.2, 0.2, 0.2, 0.7],
                        None,
                    )
                    .map_err(|e| e.to_string())?;

                let fill_color = if health_pct > 0.66 {
                    [0.0, 1.0, 0.0, 0.9]
                } else if health_pct > 0.33 {
                    [1.0, 1.0, 0.0, 0.9]
                } else {
                    [1.0, 0.0, 0.0, 0.9]
                };

                renderer
                    .draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_width * health_pct, bar_height),
                        fill_color,
                        None,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Update UI state
    pub fn update(&mut self, delta_time: Duration) {
        self.last_update = Instant::now();
        self.ui_time += delta_time.as_secs_f32();
        if let Ok(mut renderer) = self.renderer.write() {
            renderer.set_time(self.ui_time);
        }
        self.current_frame = TheGameLogic::get_frame();
        self.update_movie_playback();
        self.expire_messages();
        self.update_military_subtitle();
        self.update_floating_texts();
        self.update_named_timers();
        self.update_superweapon_timers(self.current_frame);
        self.last_ui_logic_frame = self.current_frame;
        self.handle_build_placements();
        self.handle_radius_cursor();
    }

    pub fn pre_draw(&mut self, frame: u32) {
        self.current_frame = frame;
        self.expire_hints();
        self.expire_messages();
        self.update_floating_texts();
        self.update_superweapon_timers(frame);
        self.update_named_timers();
        self.update_military_subtitle();
        self.handle_build_placements();
        self.update_and_draw_world_animations();
    }

    pub fn post_draw(&mut self, frame: u32) {
        self.current_frame = frame;
        self.update_superweapon_timers(frame);
        self.update_named_timers();
    }

    /// C++: drawPlacementCursor() — renders ghost building at the given
    /// position using the placement template's drawable. Tints green if
    /// placeable, red if blocked.
    fn draw_placement_cursor(
        &self,
        renderer: &mut UIRenderer,
        pos: &Coord3D,
    ) -> std::result::Result<(), String> {
        let Some(ref preview) = self.placement_preview else {
            return Ok(());
        };

        let Some(screen_pos) = self.world_to_screen(pos) else {
            return Ok(());
        };

        let size = preview.footprint * 50.0;
        let rect = UIRect::new(
            screen_pos.x - size.x / 2.0,
            screen_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );

        let tint_color = if preview.is_legal {
            [
                LEGAL_BUILD_COLOR[0],
                LEGAL_BUILD_COLOR[1],
                LEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        } else {
            [
                ILLEGAL_BUILD_COLOR[0],
                ILLEGAL_BUILD_COLOR[1],
                ILLEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        };

        let border_color = if preview.is_legal {
            [0.0, 1.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        renderer
            .draw_rect_outline_with_scissor(rect, 2.0, border_color, None)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Draw team/waypoint overlay lines connecting waypoints for selected units.
    /// C++: waypoint lines are drawn in the tactical view overlay.
    fn draw_team_waypoint_lines(
        &self,
        renderer: &mut UIRenderer,
    ) -> std::result::Result<(), String> {
        for hint in &self.hints {
            if !Self::hint_draws_waypoint_line(hint, self.current_frame) {
                continue;
            }

            let start_screen = self.world_to_screen(&hint.start);
            let end_screen = self.world_to_screen(&hint.end);
            if let (Some(s), Some(e)) = (start_screen, end_screen) {
                let fade = 1.0
                    - (self.current_frame - hint.creation_frame) as f32
                        / hint.lifetime_frames as f32;
                let alpha = fade.clamp(0.0, 1.0);
                let line_color = match hint.hint_type {
                    HintType::Move => [0.0, 1.0, 0.0, alpha * 0.6],
                    HintType::Command => [1.0, 1.0, 0.0, alpha * 0.6],
                    _ => [1.0, 1.0, 1.0, alpha * 0.6],
                };
                renderer.draw_line(s, e, 1.5, line_color, 0.0);
            }
        }
        Ok(())
    }

    /// Draw minimap ping animations — expanding circles at ping locations.
    pub fn render(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| InGameUIError::SystemError("Failed to lock renderer".into()))?;

        // Render selection box
        if self.selection_box.active && self.selection_box.is_significant() {
            self.render_selection_box(&mut renderer)?;
        }

        // Render minimap
        if self.minimap.visible {
            self.render_minimap(&mut renderer)?;
        }

        // Render resource display
        self.render_resources(&mut renderer)?;

        // Render placement preview
        if let Some(ref preview) = self.placement_preview {
            self.render_placement_preview(&mut renderer, preview)?;
        }

        Ok(())
    }

    /// Render selection box
    fn render_selection_box(&self, renderer: &mut UIRenderer) -> Result<()> {
        let rect = self.selection_box.get_rect();

        // Draw box outline
        renderer.draw_rect_outline_with_scissor(
            rect,
            2.0,
            [0.0, 1.0, 0.0, 0.8], // Green with alpha
            None,
        )?;

        // Draw semi-transparent fill
        renderer.draw_rect_with_scissor(
            rect,
            [0.0, 1.0, 0.0, 0.2], // Green with low alpha
            None,
        )?;

        Ok(())
    }

    /// Render minimap
    fn render_resources(&self, renderer: &mut UIRenderer) -> Result<()> {
        let pos = self.resource_display.position;

        // Background panel
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x, pos.y, 250.0, 80.0),
            [0.0, 0.0, 0.0, 0.7],
            None,
        )?;

        // Credits text
        let credits_text = format!("${}", self.resource_display.credits);
        renderer.draw_text_simple(
            &credits_text,
            Vec2::new(pos.x + 10.0, pos.y + 10.0),
            16.0,
            [1.0, 1.0, 0.0, 1.0], // Yellow
        )?;

        // Power text
        let power_color = if self.resource_display.is_power_deficit() {
            [1.0, 0.0, 0.0, 1.0] // Red if deficit
        } else {
            [0.0, 1.0, 0.0, 1.0] // Green if OK
        };

        let power_text = format!(
            "Power: {}/{}",
            self.resource_display.power_used, self.resource_display.power_available
        );
        renderer.draw_text_simple(
            &power_text,
            Vec2::new(pos.x + 10.0, pos.y + 35.0),
            14.0,
            power_color,
        )?;

        // Power bar
        let power_pct = self.resource_display.get_power_percentage();
        let bar_width = 200.0;
        let bar_height = 15.0;

        // Bar background
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x + 10.0, pos.y + 55.0, bar_width, bar_height),
            [0.3, 0.3, 0.3, 1.0],
            None,
        )?;

        // Bar fill
        renderer.draw_rect_with_scissor(
            UIRect::new(
                pos.x + 10.0,
                pos.y + 55.0,
                bar_width * power_pct,
                bar_height,
            ),
            power_color,
            None,
        )?;

        Ok(())
    }

    /// Render building placement preview
    fn render_placement_preview(
        &self,
        renderer: &mut UIRenderer,
        preview: &PlacementPreview,
    ) -> Result<()> {
        let world = Coord3D::new(preview.position.x, preview.position.y, preview.position.z);
        let Some(screen_pos) = self.world_to_screen(&world) else {
            return Ok(());
        };
        let size = preview.footprint * 50.0; // Scale for visibility

        let rect = UIRect::new(
            screen_pos.x - size.x / 2.0,
            screen_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );

        // Draw semi-transparent preview
        renderer.draw_rect_with_scissor(rect, preview.get_color(), None)?;

        // Draw border
        let border_color = if preview.is_legal {
            [0.0, 1.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        renderer.draw_rect_outline_with_scissor(rect, 2.0, border_color, None)?;

        Ok(())
    }

    /// Update UI state
    pub fn update(&mut self, delta_time: Duration) {
        self.last_update = Instant::now();
        self.ui_time += delta_time.as_secs_f32();
        if let Ok(mut renderer) = self.renderer.write() {
            renderer.set_time(self.ui_time);
        }
        // C++ InGameUI::update calls handleRadiusCursor each frame.
        self.handle_radius_cursor();
    }

    /// Resize UI elements
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);

        // Reposition minimap to bottom-right
        let minimap_margin = 10.0;
        self.minimap.position = Vec2::new(
            width - self.minimap.size.x - minimap_margin,
            height - self.minimap.size.y - minimap_margin,
        );
    }

    /// Enable/disable UI
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if UI is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ── Combat mode methods ────────────────────────────────────────────
    // C++: InGameUI.h:506-519

    pub fn pre_draw(&mut self, frame: u32) {
        self.current_frame = frame;
        self.expire_hints();
        self.expire_messages();
        self.update_floating_texts();
        self.update_superweapon_timers(frame);
        self.update_military_subtitle();
        self.update_and_draw_world_animations();
    }

    pub fn post_draw(&mut self, _frame: u32) {
        // C++: postDraw renders messages, military subtitles, superweapon timers
        // Rendering is handled separately in the Rust architecture; this hook
        // exists for any post-render cleanup logic needed later.
    }

    // ── Input enable/disable with mode clearing ────────────────────────
    // C++: InGameUI.cpp:3382 (setInputEnabled)

    pub fn init_from_settings(&mut self, settings: &InGameUIIniSettings) {
        TheInGameUI::set_max_select_count(settings.max_selection_size);
        if settings.max_selection_size > 0 {
            self.selection_state = SelectionState::new(settings.max_selection_size as usize);
        }

        self.message_color1 = settings.message_color1;
        self.message_color2 = settings.message_color2;
        self.message_position = (settings.message_position.x, settings.message_position.y);
        self.message_font_name = settings.message_font.clone();
        self.message_point_size = settings.message_point_size;
        self.message_bold = settings.message_bold;
        self.message_delay_ms = settings.message_delay_ms;

        self.military_caption_color = (
            settings.military_caption_color.red,
            settings.military_caption_color.green,
            settings.military_caption_color.blue,
            settings.military_caption_color.alpha,
        );
        self.military_caption_position = (
            settings.military_caption_position.x,
            settings.military_caption_position.y,
        );
        self.military_caption_title_font = settings.military_caption_title_font.clone();
        self.military_caption_title_point_size = settings.military_caption_title_point_size;
        self.military_caption_title_bold = settings.military_caption_title_bold;
        self.military_caption_font = settings.military_caption_font.clone();
        self.military_caption_point_size = settings.military_caption_point_size;
        self.military_caption_bold = settings.military_caption_bold;
        self.military_caption_randomize_typing = settings.military_caption_randomize_typing;
        self.military_caption_speed = settings.military_caption_speed;

        self.superweapon_countdown_position = (
            settings.superweapon_position.x,
            settings.superweapon_position.y,
        );
        self.superweapon_flash_duration = settings.superweapon_flash_duration;
        self.superweapon_flash_color = settings.superweapon_flash_color;
        self.superweapon_normal_font = settings.superweapon_normal_font.clone();
        self.superweapon_normal_point_size = settings.superweapon_normal_point_size;
        self.superweapon_normal_bold = settings.superweapon_normal_bold;
        self.superweapon_ready_font = settings.superweapon_ready_font.clone();
        self.superweapon_ready_point_size = settings.superweapon_ready_point_size;
        self.superweapon_ready_bold = settings.superweapon_ready_bold;

        self.named_timer_position = (
            settings.named_timer_position.x,
            settings.named_timer_position.y,
        );
        self.named_timer_flash_duration = settings.named_timer_flash_duration;
        self.named_timer_flash_color = settings.named_timer_flash_color;
        self.named_timer_normal_font = settings.named_timer_normal_font.clone();
        self.named_timer_normal_point_size = settings.named_timer_normal_point_size;
        self.named_timer_normal_bold = settings.named_timer_normal_bold;
        self.named_timer_normal_color = settings.named_timer_normal_color;
        self.named_timer_ready_font = settings.named_timer_ready_font.clone();
        self.named_timer_ready_point_size = settings.named_timer_ready_point_size;
        self.named_timer_ready_bold = settings.named_timer_ready_bold;
        self.named_timer_ready_color = settings.named_timer_ready_color;

        self.floating_text_timeout_frames = if settings.floating_text_timeout == 0 {
            DEFAULT_FLOATING_TEXT_TIMEOUT
        } else {
            settings.floating_text_timeout
        };
        self.floating_text_move_up_speed = settings.floating_text_move_up_speed;
        self.floating_text_vanish_rate = settings.floating_text_vanish_rate;

        self.drawable_caption_font = settings.drawable_caption_font.clone();
        self.drawable_caption_point_size = settings.drawable_caption_point_size;
        self.drawable_caption_bold = settings.drawable_caption_bold;
        self.drawable_caption_color = settings.drawable_caption_color;

        self.draw_rmb_scroll_anchor = settings.draw_rmb_scroll_anchor;
        self.move_rmb_scroll_anchor = settings.move_rmb_scroll_anchor;

        self.apply_global_language_font_overrides();
    }

    fn apply_global_language_font_overrides(&mut self) {
        crate::global_language::init_global_language_data();
        let Some(language) = get_global_language_read() else {
            return;
        };

        if let Some((name, size, bold)) = Self::language_font_override(&language.message_font) {
            self.message_font_name = name;
            self.message_point_size = size;
            self.message_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.military_caption_title_font)
        {
            self.military_caption_title_font = name;
            self.military_caption_title_point_size = size;
            self.military_caption_title_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.military_caption_font)
        {
            self.military_caption_font = name;
            self.military_caption_point_size = size;
            self.military_caption_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.superweapon_countdown_normal_font)
        {
            self.superweapon_normal_font = name;
            self.superweapon_normal_point_size = size;
            self.superweapon_normal_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.superweapon_countdown_ready_font)
        {
            self.superweapon_ready_font = name;
            self.superweapon_ready_point_size = size;
            self.superweapon_ready_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.drawable_caption_font)
        {
            self.drawable_caption_font = name;
            self.drawable_caption_point_size = size;
            self.drawable_caption_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.named_timer_countdown_normal_font)
        {
            self.named_timer_normal_font = name;
            self.named_timer_normal_point_size = size;
            self.named_timer_normal_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.named_timer_countdown_ready_font)
        {
            self.named_timer_ready_font = name;
            self.named_timer_ready_point_size = size;
            self.named_timer_ready_bold = bold;
        }
    }

    fn language_font_override(font: &FontDesc) -> Option<(String, i32, bool)> {
        if font.name.is_empty() || *font == FontDesc::default() {
            return None;
        }
        let size = crate::global_language::get_global_language_data()
            .read()
            .map(|data| data.adjust_font_size(font.size))
            .unwrap_or(font.size);
        Some((font.name.clone(), size, font.bold))
    }

    // ── Command hint system ──────────────────────────────────────────────
    // C++: InGameUI::createCommandHint() (InGameUI.cpp:2500-2772)
    // C++: InGameUI::createMouseoverHint() (InGameUI.cpp:2217-2494)

    /// Invalid drawable ID sentinel. C++: INVALID_DRAWABLE_ID (Drawable.h)
    /// In Rust, 0 is used as the invalid sentinel for moused_over_drawable_id.
    const INVALID_DRAWABLE_ID: u32 = 0;
}
