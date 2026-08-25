use super::*;
include!("command_set_strip.rs");

impl PresentationFrame {
    /// Selected unit identity (health/name/type) from snapshot only.
    ///
    /// Prefer player selection list; fall back to objects marked selected on the frame
    /// when the player list is empty (common right after click-select before player list
    /// is mirrored).
    pub fn selected_unit_display_infos(&self) -> Vec<crate::ui::UnitDisplayInfo> {
        use crate::ui::UnitDisplayInfo;

        // Wave 1106: selection display residual fail-closed on sold/unselectable/
        // masked (not only destroyed). C++ ControlBarCommand.cpp:1048-1078 still
        // shows a disabled completed structure so Sell/Stop/Rally stay reachable.
        // Under-construction is combat-disabled but must keep CancelConstruction.
        let usable = |o: &RenderableObject| !o.destroyed && !o.sold && !o.unselectable && !o.masked;
        let by_id: std::collections::HashMap<ObjectId, &RenderableObject> =
            self.objects.iter().map(|o| (o.id, o)).collect();
        let mut selected_infos = Vec::with_capacity(self.selected.len().max(1));
        for id in &self.selected {
            if let Some(ro) = by_id.get(id) {
                if !usable(ro) {
                    continue;
                }
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        if selected_infos.is_empty() {
            for ro in self.objects.iter().filter(|o| o.selected && usable(o)) {
                selected_infos.push(Self::unit_display_info_from_renderable(ro));
            }
        }
        selected_infos
    }

    fn unit_display_info_from_renderable(ro: &RenderableObject) -> crate::ui::UnitDisplayInfo {
        let (production_template, production_progress, production_is_upgrade) = ro
            .production_queue
            .first()
            .map(|p| {
                (
                    Some(p.template_name.clone()),
                    Some(p.progress_ratio),
                    p.is_upgrade,
                )
            })
            .unwrap_or((None, None, false));
        crate::ui::UnitDisplayInfo {
            object_id: ro.id,
            name: ro.template_name.clone(),
            health_current: ro.health_current,
            health_maximum: ro.health_max.max(1.0),
            unit_type: if ro.is_structure {
                "Structure".into()
            } else if ro.is_unit {
                "Unit".into()
            } else {
                "Object".into()
            },
            current_order: if ro.attacking {
                "Attack".into()
            } else if ro.moving {
                "Move".into()
            } else if production_template.is_some() {
                if production_is_upgrade {
                    "Research".into()
                } else {
                    "Produce".into()
                }
            } else {
                "Idle".into()
            },
            veterancy_overlay: ro.veterancy.chevron_overlay().map(str::to_string),
            production_progress,
            production_template,
            production_is_upgrade,
            production_paused: ro.production_paused,
            command_set_override: ro.command_set_override.clone(),
            can_produce: ro.can_produce,
        }
    }

    /// Apply presentation identity fields onto a HUD/UI state (production consumer path).
    /// Does not re-borrow GameLogic — uses only owned snapshot data.
    ///
    /// Overwrites **selection IDs, selected unit health/name, and minimap unit dots**
    /// so a prior live `update_ui_state` walk cannot leave stale identity when a frame
    /// is available.
    pub fn apply_to_ui_state(&self, ui: &mut crate::ui::GameUIState) {
        self.apply_can_make_cameos_to_ui_state(ui);

        ui.rank_level = self.local_rank_level;
        ui.skill_points = self.local_skill_points;
        ui.science_purchase_points = self.local_science_purchase_points;
        ui.rank_progress_percent = self.local_rank_progress_percent;
        ui.superweapon_timers = self
            .superweapon_timers
            .iter()
            .map(|t| crate::ui::hud_state::UiSuperweaponTimer {
                name: t.name.clone(),
                template_name: t.template_name.clone(),
                icon: t.icon.clone(),
                recharge_time: t.recharge_time,
                remaining: t.remaining,
                unlocked: t.unlocked,
                ready: t.ready,
            })
            .collect();
        use crate::game_logic::victory::PlayerOutcome;
        use crate::ui::{BuildQueueEntry, MinimapDot, color_for_player};

        ui.current_game_time = self.total_play_time_seconds;
        ui.credits = self.local_supplies as i32;
        // Prefer produced/consumed residual when present (energy bar parity).
        ui.power_generated = self.local_power_produced.max(self.local_power).max(0);
        ui.power_used = self.local_power_consumed.max(0);
        ui.max_power = ui.power_generated.max(1);
        ui.player_id = self.local_player_id;
        ui.selected_units = self.selected.clone();
        ui.match_over = self.match_over;
        ui.selected_unit_infos = self.selected_unit_display_infos();
        // Radar residual from snapshot events (no live update_ui_state re-read).
        {
            use crate::ui::{RadarMessageEntry, RadarPing, RadarPingKind};
            let mut messages = Vec::new();
            let mut pings = Vec::new();
            let mut last_ping = ui.last_radar_ping;
            for ev in &self.events {
                if let PresentationEvent::RadarMessage {
                    text,
                    position,
                    kind,
                    ..
                } = ev
                {
                    let ping_kind = match kind {
                        1 => RadarPingKind::Attack,
                        2 => RadarPingKind::Ally,
                        _ => RadarPingKind::Generic,
                    };
                    messages.push(text.clone());
                    ui.radar_events.push(RadarMessageEntry {
                        text: text.clone(),
                        position: Some(*position),
                        kind: ping_kind,
                    });
                    if position.length_squared() > 0.0001 {
                        pings.push(RadarPing {
                            position: *position,
                            intensity: 1.0,
                            age_seconds: 0.0,
                            kind: ping_kind,
                        });
                        last_ping = Some(*position);
                    }
                }
            }
            if !messages.is_empty() {
                ui.radar_messages.extend(messages);
                // Cap residual feed.
                let excess = ui.radar_messages.len().saturating_sub(32);
                if excess > 0 {
                    ui.radar_messages.drain(0..excess);
                }
            }
            if !pings.is_empty() {
                ui.radar_pings.extend(pings);
                let excess = ui.radar_pings.len().saturating_sub(32);
                if excess > 0 {
                    ui.radar_pings.drain(0..excess);
                }
            }
            ui.last_radar_ping = last_ping;
        }
        // Script / cinematic / radar residual from snapshot.
        if !self.script_messages.is_empty() {
            ui.script_messages = self.script_messages.clone();
        }
        ui.cinematic_letterbox = self.cinematic_letterbox;
        if self.cinematic_text.is_some() {
            ui.cinematic_text = self.cinematic_text.clone();
        }
        if self.military_caption.is_some() {
            ui.military_caption = self.military_caption.clone();
        }
        ui.radar_enabled = self.radar_ui_enabled;
        ui.radar_forced = self.radar_forced;
        // Script named-timer / cameo / superweapon residual from snapshot.
        ui.named_timers = self.named_timers.clone();
        ui.named_timer_display_shown = self.named_timer_display_shown;
        ui.cameo_flash = self.cameo_flash.clone();
        ui.superweapon_display_enabled = self.superweapon_display_enabled;
        ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone();
        ui.objectives = self.objectives.clone();
        ui.pending_movie = self.pending_movie.clone();
        ui.pending_radar_movie = self.pending_radar_movie.clone();
        ui.pending_music_stop = self.pending_music_stop;
        ui.pending_popup_messages = self
            .pending_popup_messages
            .iter()
            .map(|p| p.message.clone())
            .collect();
        ui.script_time_frozen = self.script_time_frozen;
        ui.script_camera_time_frozen = self.script_camera_time_frozen;
        ui.time_frozen_for_simulation = self.time_frozen_for_simulation;
        ui.script_fps_limit = self.script_fps_limit;
        ui.view_guardband = self.view_guardband;
        ui.camera_focus = self.camera_focus;
        ui.camera_bw_mode = self.camera_bw_mode;
        ui.camera_fade = if self.camera_fade.fade == 0 {
            None
        } else {
            Some((
                self.camera_fade.fade,
                self.camera_fade.intensity,
                self.camera_fade.diffuse,
            ))
        };
        ui.camera_shakers = self.camera_shakers.clone();
        ui.camera_motion_blur_count = self.camera_motion_blur_count;
        ui.camera_zoom = self.camera_zoom;
        ui.camera_zoom_reset = self.camera_zoom_reset;
        ui.camera_pitch = self.camera_pitch;
        ui.camera_rotate = self.camera_rotate;
        ui.camera_look_toward = self.camera_look_toward;
        ui.camera_slave_enable = self.camera_slave_enable.clone();
        ui.camera_slave_disable = self.camera_slave_disable;
        ui.named_timers = self.named_timers.clone();
        ui.cameo_flash = self.cameo_flash.clone();
        ui.screen_shakes = self.screen_shakes.clone();
        ui.script_skybox_enabled = self.script_skybox_enabled;
        ui.superweapon_display_enabled = self.superweapon_display_enabled;
        ui.named_timer_display_shown = self.named_timer_display_shown;
        ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone();
        // Beacon residual from snapshot (no live GameLogic update_ui_state re-read).
        // Always assign so a hide-empty freeze clears leftover enemy dots.
        ui.new_beacons = self.new_beacons.clone();
        if self.beacons.is_empty() {
            ui.minimap_beacons.clear();
        } else {
            use crate::ui::{MinimapDot, color_for_player};
            // Wave 1110: beacon bounds residual excludes sold.
            let (min_x, max_x, min_z, max_z) = {
                let alive: Vec<_> = self
                    .objects
                    .iter()
                    .filter(|o| !o.destroyed && !o.sold)
                    .collect();
                if alive.is_empty() {
                    (-100.0_f32, 100.0_f32, -100.0_f32, 100.0_f32)
                } else {
                    let mut min_x = f32::MAX;
                    let mut max_x = f32::MIN;
                    let mut min_z = f32::MAX;
                    let mut max_z = f32::MIN;
                    for o in &alive {
                        min_x = min_x.min(o.position.x);
                        max_x = max_x.max(o.position.x);
                        min_z = min_z.min(o.position.z);
                        max_z = max_z.max(o.position.z);
                    }
                    (min_x, max_x, min_z, max_z)
                }
            };
            let span_x = (max_x - min_x).max(1.0);
            let span_z = (max_z - min_z).max(1.0);
            ui.minimap_beacons = self
                .beacons
                .iter()
                .map(|p| {
                    let nx = ((p.x - min_x) / span_x).clamp(0.0, 1.0);
                    let ny = ((p.z - min_z) / span_z).clamp(0.0, 1.0);
                    let owner = self
                        .objects
                        .iter()
                        .find(|o| {
                            o.template_name.to_ascii_lowercase().contains("beacon")
                                && (o.position - *p).length() <= 3.0
                        })
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(self.local_player_id);
                    MinimapDot::normalized(nx, ny, color_for_player(owner.min(255) as u8), 4.0)
                })
                .collect();
        }
        // ControlBar/WND selection panel health must come from snapshot, not live re-read.
        ui.selection_panel =
            crate::ui::ControlBarSelectionPanelState::from_unit_infos(&ui.selected_unit_infos);

        // Victory residual from snapshot events (no live evaluate_victory re-read).
        if self.match_over {
            let winner = self.events.iter().find_map(|e| match e {
                PresentationEvent::Victory { winner_player } => *winner_player,
                _ => None,
            });
            ui.player_outcome = Some(match winner {
                Some(id) if id == self.local_player_id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => {
                    if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("draw"))
                    {
                        PlayerOutcome::Draw
                    } else if self
                        .victory_label
                        .as_deref()
                        .is_some_and(|s| s.to_ascii_lowercase().contains("winner"))
                    {
                        // Fail-closed: label without winner id → treat as unknown draw residual.
                        PlayerOutcome::Draw
                    } else {
                        PlayerOutcome::Draw
                    }
                }
            });
        }

        // Structure production + under-construction residual for build-queue HUD strip.
        // Wave 1110: build-queue residual excludes sold producers/structures.
        let mut build_queue = Vec::new();
        for o in self.objects.iter().filter(|o| !o.destroyed && !o.sold) {
            if o.under_construction {
                build_queue.push(BuildQueueEntry {
                    template_name: o.template_name.clone(),
                    percent_complete: o.construction_percent.clamp(0.0, 1.0),
                    time_remaining: (1.0 - o.construction_percent.clamp(0.0, 1.0)) * 30.0,
                });
            }
            for item in &o.production_queue {
                build_queue.push(BuildQueueEntry {
                    template_name: item.template_name.clone(),
                    percent_complete: item.progress_ratio.clamp(0.0, 1.0),
                    time_remaining: (item.total_time * (1.0 - item.progress_ratio.clamp(0.0, 1.0)))
                        .max(0.0),
                });
            }
        }
        build_queue.truncate(16);
        ui.build_queue = build_queue;

        // Minimap dots from snapshot positions/teams (normalized into frame bounds).
        // Wave 1110: minimap unit-dot residual excludes sold (parity hud_minimap_units).
        let alive: Vec<&RenderableObject> = self
            .objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && Self::minimap_fow_allows(o))
            .collect();
        let (world_min_x, world_max_x, world_min_z, world_max_z) = if alive.is_empty() {
            (-100.0, 100.0, -100.0, 100.0)
        } else {
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;
            for o in &alive {
                min_x = min_x.min(o.position.x);
                max_x = max_x.max(o.position.x);
                min_z = min_z.min(o.position.z);
                max_z = max_z.max(o.position.z);
            }
            // Pad so single-unit maps still normalize.
            if (max_x - min_x).abs() < 1.0 {
                min_x -= 50.0;
                max_x += 50.0;
            }
            if (max_z - min_z).abs() < 1.0 {
                min_z -= 50.0;
                max_z += 50.0;
            }
            (min_x, max_x, min_z, max_z)
        };
        let span_x = (world_max_x - world_min_x).max(1.0);
        let span_z = (world_max_z - world_min_z).max(1.0);
        let mut dots = Vec::with_capacity(alive.len());
        for ro in alive {
            let nx = ((ro.position.x - world_min_x) / span_x).clamp(0.0, 1.0);
            let nz = ((ro.position.z - world_min_z) / span_z).clamp(0.0, 1.0);
            let color = match ro.team {
                Team::USA => color_for_player(1),
                Team::China => color_for_player(0),
                Team::GLA => color_for_player(4),
                Team::Neutral => color_for_player(7),
            };
            let size = if ro.is_structure { 4.0 } else { 2.0 };
            dots.push(MinimapDot::normalized(nx, nz, color, size));
        }
        ui.minimap_unit_dots = dots;
    }

    /// Resource triple for GameHud::update_resources (credits, power, max_power).
    /// Winner player id from frozen Victory event residual.
    pub fn victory_winner_id(&self) -> Option<u32> {
        self.events.iter().find_map(|ev| match ev {
            PresentationEvent::Victory { winner_player } => *winner_player,
            _ => None,
        })
    }

    /// Frozen VictorySummary residual when match_over.
    pub fn victory_summary_residual(&self) -> Option<&crate::game_logic::VictorySummary> {
        self.victory_summary.as_ref()
    }

    /// Drive VictoryScreen visibility/type from snapshot residual (no live GameLogic).
    ///
    /// Fail-closed: does not rebuild full VictorySummary statistics tables.
    pub fn apply_to_victory_screen(&self, screen: &mut crate::ui::VictoryScreen) {
        if !self.match_over {
            return;
        }
        let winner = self.events.iter().find_map(|e| match e {
            PresentationEvent::Victory { winner_player } => *winner_player,
            _ => None,
        });
        match winner {
            Some(id) if id == self.local_player_id => screen.set_victory(id),
            Some(_) => screen.set_defeat(),
            None => {
                let label = self
                    .victory_label
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if label.contains("defeat") || label.contains("lost") {
                    screen.set_defeat();
                } else if label.contains("winner") && !label.contains("draw") {
                    // Label-only winner residual without player id → draw fail-closed.
                    screen.set_draw();
                } else {
                    screen.set_draw();
                }
            }
        }
    }

    pub fn hud_resource_triple(&self) -> (i32, i32, i32) {
        let credits = self.local_supplies as i32;
        let power = self.local_power.max(0);
        (credits, power, power.max(1))
    }

    /// C++ `W3DRadar::renderObjectList`: skip when `getShroudedStatus > PARTIAL_CLEAR`.
    #[inline]
    fn minimap_fow_allows(o: &RenderableObject) -> bool {
        (o.drawable_shroud.raw_status as u8) <= (PresentationObjectShroudStatus::PartialClear as u8)
    }

    /// Units list for GameHud minimap: (id, x, z, team_color_index).
    pub fn hud_minimap_units(&self) -> Vec<(ObjectId, f32, f32, u8)> {
        // Wave 1109: minimap unit-dot residual excludes sold (alive dots only).
        // hq-cqosc: also skip fogged/shrouded enemies (C++ W3DRadar.cpp:636-638).
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && Self::minimap_fow_allows(o))
            .map(|o| {
                let team_idx = match o.team {
                    Team::USA => 1u8,
                    Team::China => 0u8,
                    Team::GLA => 4u8,
                    Team::Neutral => 7u8,
                };
                (o.id, o.position.x, o.position.z, team_idx)
            })
            .collect()
    }

    /// Apply presentation resources, minimap units, and selection health to GameHUD.
    ///
    /// Selection identity (IDs + health/name) is snapshot-owned so the production HUD
    /// does not re-read live GameLogic after a skirmish start / dual-tick.
    /// Also fills the ControlBar selection panel health strip via GameHUD.
    /// Feed InGameUI PublicTimer residual into ConstructionPanel superweapon timers.
    pub fn apply_superweapon_timers_to_panel(
        &self,
        panel: &mut crate::ui::construction_panel::ConstructionPanel,
    ) {
        use crate::ui::construction_panel::SuperweaponTimer;
        for t in &self.superweapon_timers {
            if !t.unlocked {
                continue;
            }
            let mut timer = SuperweaponTimer::new(
                t.name.clone(),
                t.template_name.clone(),
                t.icon.clone(),
                t.recharge_time,
            );
            timer.remaining = t.remaining;
            timer.unlocked = t.unlocked;
            panel.add_superweapon_timer(timer);
        }
        self.apply_can_make_cameos_to_panel(panel);
    }

    pub fn apply_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        // InGameUI PublicTimer residual freeze onto HUD strip.
        let sw: Vec<crate::ui::hud_state::UiSuperweaponTimer> = self
            .superweapon_timers
            .iter()
            .map(|t| crate::ui::hud_state::UiSuperweaponTimer {
                name: t.name.clone(),
                template_name: t.template_name.clone(),
                icon: t.icon.clone(),
                recharge_time: t.recharge_time,
                remaining: t.remaining,
                ready: t.ready,
                unlocked: t.unlocked,
            })
            .collect();
        hud.apply_presentation_superweapon_timers(&sw);

        // ControlBar construction button enable residual from CanMake freeze.
        hud.apply_can_make_cameos(
            &self
                .can_make_cameos
                .iter()
                .map(|c| {
                    (
                        c.template_name.as_str(),
                        c.available,
                        c.can_make,
                        c.help_status.as_deref(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        // Production queue residual for primary selected producer.
        // Wave 1109: HUD queue residual fail-closed on sold/unusable primary
        // (parity with control_bar_selection_panel Wave 1108).
        {
            let mut queue_items: Vec<(String, f32, i32, f32, bool)> = Vec::new();
            let usable = |o: &&RenderableObject| {
                o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked
            };
            if let Some(id) = self
                .selected
                .first()
                .copied()
                .or_else(|| self.objects.iter().find(usable).map(|o| o.id))
            {
                if let Some(ro) = self
                    .objects
                    .iter()
                    .find(|o| o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked)
                {
                    if self.is_owned_by_local(ro) {
                        for p in &ro.production_queue {
                            queue_items.push((
                                p.template_name.clone(),
                                p.progress_ratio,
                                p.cost_supplies as i32,
                                p.total_time.max(0.01),
                                p.is_upgrade,
                            ));
                        }
                    }
                }
            }
            hud.sync_production_queue_from_presentation(&queue_items);
        }
        let (credits, power, max_power) = self.hud_resource_triple();
        hud.update_resources(credits, power, max_power);
        let units = self.hud_minimap_units();
        hud.update_minimap(&units);
        let infos = self.selected_unit_display_infos();
        // Prefer explicit player selection list; if empty but infos came from
        // object.selected flags, mirror those IDs onto the HUD strip.
        let mut ids = self.selected.clone();
        if ids.is_empty() {
            ids = infos.iter().map(|i| i.object_id).collect();
        }
        hud.sync_selection_from_presentation(ids, infos);
        // ControlBar unit command residual: keep holes, portraits, clocks.
        let cmds = self.unit_command_buttons();
        hud.apply_presentation_unit_commands(&cmds);
        self.apply_events_to_game_hud(hud);
    }

    /// Route frozen gameplay events into HUD radar (and no-op audio-only kinds).
    ///
    /// C++ completion/EVA/victory/ownership feedback is voice, radar blip, or
    /// WinLose WND — not `InGameUI::message` overlay text (InGameUI.cpp:1993
    /// callers; Player.cpp:1639 onStructureConstructionComplete; Eva.cpp
    /// setShouldPlay). `apply_to_game_hud` is invoked every render on the
    /// last freeze, so each LogicFrame is applied once per HUD.
    pub fn apply_events_to_game_hud(&self, hud: &mut crate::ui::GameHUD) {
        if !hud.begin_presentation_event_apply(self.frame.0) {
            return;
        }
        for ev in &self.events {
            match ev {
                PresentationEvent::RadarMessage {
                    text,
                    position,
                    kind,
                    ..
                } => {
                    use crate::ui::RadarPingKind;
                    let ping_kind = match kind {
                        1 => RadarPingKind::Attack,
                        2 => RadarPingKind::Ally,
                        _ => RadarPingKind::Generic,
                    };
                    let pos = if position.length_squared() > 0.0001 {
                        Some(*position)
                    } else {
                        None
                    };
                    hud.add_radar_message(text, pos, ping_kind);
                }
                // C++: VoiceCreated / getVoiceTaskComplete / Eva SideSounds only.
                PresentationEvent::ConstructionComplete { .. }
                | PresentationEvent::UpgradeComplete { .. }
                | PresentationEvent::ProductionComplete { .. }
                | PresentationEvent::OwnerChanged { .. }
                | PresentationEvent::EvaAlert { .. }
                | PresentationEvent::Victory { .. }
                | PresentationEvent::AttackTargeted { .. }
                | PresentationEvent::MoveOrdered { .. }
                | PresentationEvent::HealApplied { .. }
                | PresentationEvent::DamageApplied { .. }
                | PresentationEvent::ObjectDestroyed { .. }
                | PresentationEvent::EconomyChanged { .. }
                | PresentationEvent::ParticleSystemSpawned { .. }
                | PresentationEvent::WeaponFireLoopStarted { .. }
                | PresentationEvent::WeaponFireLoopStopped { .. }
                | PresentationEvent::WeaponDischarged { .. } => {}
            }
        }
    }

    /// Collect presentation→audio requests (no GameLogic borrow).
    /// Wave 527/528: FireSound loop residual uses host sound name + looping flag + snapshot pose.
    /// Wave 528: WeaponFireLoopStop is stop-only (no FireSound replay).
    /// Wave 529: RadarMessage → EVA/radar audio event names + snapshot position.
    /// Wave 530: OwnerChanged → BuildingCaptured/UnitHijacked audio residual.
    /// Wave 533: EvaAlert is HUD/chat only — Eva.ini SideSounds play via Eva::update.
    /// Wave 535: ParticleSystemSpawned → Explosion/FireBurn/… audio at snapshot pose.
    /// Fail-closed: not Miles/device spatial parity — names resolve via SoundEffectsTable.
    pub fn collect_audio_events(&self) -> Vec<crate::game_logic::AudioEventRequest> {
        use crate::game_logic::AudioEventRequest;
        let mut out = Vec::new();
        // Snapshot pose lookup for 3D placement (no live GameLogic dual-read).
        let pose_by_id: std::collections::HashMap<ObjectId, glam::Vec3> =
            self.objects.iter().map(|o| (o.id, o.position)).collect();
        for ev in &self.events {
            // Wave 527: FiringTracker loop uses concrete FireSound name when non-empty.
            if let PresentationEvent::WeaponFireLoopStarted { unit, sound } = ev {
                let name = if sound.is_empty() {
                    "WeaponFireLoop"
                } else {
                    sound.as_str()
                };
                let mut req = AudioEventRequest::new(name).with_object(*unit).looping();
                if let Some(pos) = pose_by_id.get(unit) {
                    req = req.with_position(*pos);
                }
                out.push(req);
                continue;
            }
            if let PresentationEvent::WeaponFireLoopStopped { unit, sound } = ev {
                // Wave 528: explicit stop residual (must not re-trigger FireSound play).
                // C++ FiringTracker removeAudioEvent uses the FireSound name.
                let name = if sound.is_empty() {
                    "WeaponFireLoopStop"
                } else {
                    sound.as_str()
                };
                let mut req = AudioEventRequest::new(name).with_object(*unit).stopping();
                if let Some(pos) = pose_by_id.get(unit) {
                    req = req.with_position(*pos);
                }
                req = req.with_priority(200);
                out.push(req);
                continue;
            }
            let mapped: Option<(&str, Option<crate::game_logic::ObjectId>)> = match ev {
                // C++ ActiveBody::doDamageFX + death FXList Sound nuggets — never UnitDie/WeaponHit.
                PresentationEvent::ObjectDestroyed { .. } => None,
                // C++ DozerAIUpdate getVoiceTaskComplete / ProductionUpdate
                // getVoiceCreated / getResearchCompleteSound + Eva UpgradeComplete.
                // Never invented BuildingComplete / UnitReady / UpgradeComplete SFX.
                PresentationEvent::ConstructionComplete { .. }
                | PresentationEvent::UpgradeComplete { .. }
                | PresentationEvent::ProductionComplete { .. } => None,
                PresentationEvent::AttackTargeted { attacker, .. } => {
                    Some(("WeaponFire", Some(*attacker)))
                }
                PresentationEvent::DamageApplied { .. } => None,
                PresentationEvent::HealApplied { target, .. } => Some(("UnitHeal", Some(*target))),
                // C++ Money::withdraw/deposit play MiscAudio; EconomyChanged is HUD only.
                PresentationEvent::EconomyChanged { .. } => None,
                PresentationEvent::Victory { .. } => Some(("Victory", None)),

                // C++ pickAndPlayUnitVoiceResponse plays ThingTemplate VoiceMove only.
                PresentationEvent::MoveOrdered { unit: _, .. } => None,
                PresentationEvent::WeaponFireLoopStarted { .. }
                | PresentationEvent::WeaponFireLoopStopped { .. }
                | PresentationEvent::WeaponDischarged { .. } => None,
                PresentationEvent::ParticleSystemSpawned { .. } => None, // handled below (Wave 535)
                PresentationEvent::OwnerChanged { .. } => None,          // handled below (Wave 530)
                PresentationEvent::RadarMessage { .. } => None,          // handled below (Wave 529)
                // Wave 533: EvaAlert { name } stays for HUD/chat. Eva::update
                // SideSounds is the sole EVA playback path — not generic EVA_*.
                PresentationEvent::EvaAlert { name } => {
                    let _ = name;
                    None
                }
            };
            let Some((kind, obj)) = mapped else {
                continue;
            };
            let mut req = AudioEventRequest::new(kind);
            if let Some(id) = obj {
                req = req.with_object(id);
                if let Some(pos) = pose_by_id.get(&id) {
                    req = req.with_position(*pos);
                }
            }
            out.push(req);
        }

        // C++ Money.cpp withdraw/deposit — MiscAudio tokens, not invented MoneyTick.
        for ev in crate::game_logic::host_economy_log::take_money_audio() {
            let token = crate::game_logic::host_economy_log::money_audio_token(ev.kind);
            let name = crate::game_logic::host_economy_log::resolve_misc_audio_event(token);
            let _ = ev.player_id;
            out.push(AudioEventRequest::new(name.as_str()));
        }

        // C++ Object::setDisabledUntil / clearDisabled MiscAudio.
        for ev in crate::game_logic::host_disable_timers_log::take_audio() {
            let name =
                crate::game_logic::host_economy_log::resolve_misc_audio_event(&ev.event_name);
            let pos = glam::Vec3::new(ev.position[0], ev.position[1], ev.position[2]);
            out.push(
                AudioEventRequest::new(name.as_str())
                    .with_object(ev.object)
                    .with_position(pos)
                    .with_priority(160),
            );
        }

        // Wave 530: capture/hijack ownership transfer audio residual.
        for ev in &self.events {
            let PresentationEvent::OwnerChanged { id, .. } = ev else {
                continue;
            };
            let is_structure = self
                .objects
                .iter()
                .find(|o| o.id == *id)
                .map(|o| o.is_structure)
                .unwrap_or(false);
            let name = if is_structure {
                "BuildingCaptured"
            } else {
                "UnitHijacked"
            };
            let mut req = AudioEventRequest::new(name)
                .with_object(*id)
                .with_priority(170);
            if let Some(pos) = pose_by_id.get(id) {
                req = req.with_position(*pos);
            }
            out.push(req);
        }

        // Wave 533: EvaAlert { name } is HUD/chat only. C++ Eva::update plays
        // Eva.ini SideSounds (EvaUSA_BuildingLost / EvaChina_LowPower / …).
        // Do not push generic EVA_* onto the SFX drain.

        // Wave 535: combat particle spawn → presentation audio residual (snapshot pose).
        // Fail-closed: not full FXList/FXParticleSystemNames Miles matrix — kind→name map only.
        // Skip muzzle (WeaponFire already) and impact (DamageFX via ActiveBody::doDamageFX).

        for ev in &self.events {
            let PresentationEvent::ParticleSystemSpawned {
                kind,
                position,
                template_name,
                ..
            } = ev
            else {
                continue;
            };
            use crate::game_logic::combat_particles::CombatParticleKind;
            let event_names: Vec<String> = match kind {
                CombatParticleKind::DeathExplosion => {
                    // Prefer Sound nugget names inside an authored FXList
                    // (C++ SoundFXNugget m_soundName). Never play the FXList
                    // template name as an audio event.
                    let t = template_name.as_str();
                    if t.is_empty() {
                        vec!["Explosion".to_string()]
                    } else if t.starts_with("FX_") || t.starts_with("WeaponFX_") {
                        let sounds = crate::game_logic::sound_names_for_fx_list(t);
                        if sounds.is_empty() {
                            vec!["Explosion".to_string()]
                        } else {
                            sounds
                        }
                    } else {
                        vec!["Explosion".to_string()]
                    }
                }
                CombatParticleKind::DeathBurn => vec!["FireBurn".to_string()],
                CombatParticleKind::DeathPoison => vec!["PoisonDeath".to_string()],
                CombatParticleKind::DeathLaser => vec!["LaserDeath".to_string()],
                CombatParticleKind::DeathSmoke => vec!["DeathSmoke".to_string()],
                CombatParticleKind::WeaponMuzzleFlash
                | CombatParticleKind::WeaponImpact
                | CombatParticleKind::ProjectileExhaust
                | CombatParticleKind::ParticleSysBone
                | CombatParticleKind::BodyFire
                | CombatParticleKind::BodySmoke
                | CombatParticleKind::DisableFx => continue,
            };
            for event_name in event_names {
                let event_name = event_name.as_str();
                let mut req = AudioEventRequest::new(event_name).with_priority(160);
                if position.length_squared() > 0.01 {
                    req = req.with_position(*position);
                }
                out.push(req);
            }
        }

        // Wave 529: radar/EVA presentation audio residual (no GameLogic dual-write).
        // kind: 0=Generic 1=Attack 2=Ally; text also maps classic EVA phrases.
        for ev in &self.events {
            let PresentationEvent::RadarMessage {
                text,
                position,
                kind,
                ..
            } = ev
            else {
                continue;
            };
            let t = text.to_ascii_lowercase();
            // Classic EVA phrases stay on Eva::update SideSounds (EvaUSA_*).
            // Keep the EVA_* tokens in source for residual honesty; do not play them.
            if t.contains("low power") || t.contains("power shortage") {
                let _ = "EVA_LowPower";
                continue;
            } else if t.contains("insufficient funds") || t.contains("not enough money") {
                let _ = "EVA_InsufficientFunds";
                continue;
            } else if t.contains("base under attack") || t.contains("our base is under attack") {
                let _ = "EVA_BaseUnderAttack";
                continue;
            } else if t.contains("ally under attack") || t.contains("ally is under attack") {
                let _ = "EVA_AllyUnderAttack";
                continue;
            } else if t.contains("building lost") || t.contains("structure lost") {
                let _ = "EVA_BuildingLost";
                continue;
            } else if t.contains("unit lost") {
                let _ = "EVA_UnitLost";
                continue;
            }
            let event_name = match kind {
                1 => "RadarAttack",
                2 => "RadarAlly",
                _ => "RadarGeneric",
            };
            let mut req = AudioEventRequest::new(event_name).with_priority(180);
            if position.length_squared() > 0.01 {
                req = req.with_position(*position);
            }
            out.push(req);
        }
        out
    }

    /// Dispatch presentation audio directly to the audio subsystem.
    /// Fail-closed boundary: does **not** mutate GameLogic mid-frame.
    pub fn dispatch_audio_events_direct(&self) -> usize {
        self.refresh_live_audio_locality();
        let events = self.collect_audio_events();
        let n = events.len();
        for event in events {
            if !crate::game_logic::audio_dispatch_impl::should_dispatch_audio_request(&event) {
                continue;
            }
            if let Some(obj_id) = event.object_id {
                if let Some(pos) = event.position {
                    log::trace!(
                        "🔊 Presentation audio: {} at {:?} from object {}",
                        event.event_type,
                        pos,
                        obj_id
                    );
                } else {
                    log::trace!(
                        "🔊 Presentation audio: {} from object {}",
                        event.event_type,
                        obj_id
                    );
                }
            } else if let Some(pos) = event.position {
                log::trace!("🔊 Presentation audio: {} at {:?}", event.event_type, pos);
            } else {
                log::trace!("🔊 Presentation audio: {}", event.event_type);
            }
            let _ = crate::subsystem_manager::with_subsystem_mut::<
                crate::subsystem_manager::AudioManagerSubsystem,
                _,
            >(|audio| audio.queue_event(event.clone()));
        }
        n
    }

    fn refresh_live_audio_locality(&self) {
        use crate::game_logic::audio_dispatch_impl::{
            LiveAudioLocality, LiveAudioPlayer, set_live_audio_locality,
        };
        use game_engine::common::audio::AudioLocalityRelationship;
        let mut snap = LiveAudioLocality {
            local_player_index: self.local_player_id as i32,
            local_player_active: self
                .player_info(self.local_player_id)
                .map(|p| p.is_alive)
                .unwrap_or(true),
            observer_look_at: None,
            players: std::collections::HashMap::new(),
            object_owners: std::collections::HashMap::new(),
        };
        for p in &self.players {
            let relationship_to_local = if p.id == self.local_player_id {
                AudioLocalityRelationship::Allies
            } else if let Some(local) = self.player_info(self.local_player_id) {
                if local.alliance_team >= 0 && local.alliance_team == p.alliance_team {
                    AudioLocalityRelationship::Allies
                } else if local.is_alive && p.is_alive {
                    AudioLocalityRelationship::Enemies
                } else {
                    AudioLocalityRelationship::Neutral
                }
            } else {
                AudioLocalityRelationship::Neutral
            };
            snap.players.insert(
                p.id as i32,
                LiveAudioPlayer {
                    exists: true,
                    active: p.is_alive,
                    has_default_team: true,
                    relationship_to_local,
                },
            );
        }
        for o in &self.objects {
            if let Some(pid) = o.owner_player_id {
                snap.object_owners.insert(o.id.0, pid as i32);
            }
        }
        set_live_audio_locality(snap);
    }

    /// Legacy dual-write residual (tests may still call). Prefer `dispatch_audio_events_direct`.
    pub fn apply_events_to_audio(&self, logic: &mut GameLogic) -> usize {
        self.refresh_live_audio_locality();
        let events = self.collect_audio_events();
        let n = events.len();
        for req in events {
            if !crate::game_logic::audio_dispatch_impl::should_dispatch_audio_request(&req) {
                continue;
            }
            logic.queue_audio_event(req);
        }
        n
    }

    /// Ensure active presentation particle systems are mirrored into the GameClient
    /// ParticleSystemManager (same-frame residual). Prefer existing client_system_id;
    /// backfill when host spawn mirror was skipped/failed.
    /// Fail-closed: not full W3D GPU particle parity.
    pub fn apply_particle_systems_to_client(&self) -> usize {
        let mut n = 0usize;
        for p in self.particle_systems.iter().filter(|p| p.active) {
            if p.client_system_id.is_some() {
                continue;
            }
            if crate::game_logic::combat_particles::mirror_spawn_to_client_manager(
                &p.template_name,
                p.position,
            )
            .is_some()
            {
                n += 1;
            }
        }
        // Spawn events without prior client id residual (same-frame observe path).
        for ev in &self.events {
            if let PresentationEvent::ParticleSystemSpawned {
                template_name,
                position,
                ..
            } = ev
            {
                // If already covered by particle_systems list with client id, skip.
                let already = self.particle_systems.iter().any(|p| {
                    p.template_name == *template_name
                        && (p.position - *position).length_squared() < 1e-4
                        && p.client_system_id.is_some()
                });
                if already {
                    continue;
                }
                if crate::game_logic::combat_particles::mirror_spawn_to_client_manager(
                    template_name,
                    *position,
                )
                .is_some()
                {
                    n += 1;
                }
            }
        }
        n
    }

    /// Snapshot-owned ControlBar / WND selection panel (health + name).
    pub fn control_bar_selection_panel(&self) -> crate::ui::ControlBarSelectionPanelState {
        let mut panel = crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &self.selected_unit_display_infos(),
        );
        // Prefer full queue from the primary selected renderable when present.
        // Wave 1108: fail-closed on sold/unusable primary (belt-and-suspenders after
        // selected_unit_display_infos Wave 1106).
        if let Some(id) = panel.primary_object_id {
            if let Some(ro) = self
                .objects
                .iter()
                .find(|o| o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked)
            {
                panel.production_queue = ro
                    .production_queue
                    .iter()
                    .map(|p| (p.template_name.clone(), p.progress_ratio, p.is_upgrade))
                    .collect();
                if panel.production_progress.is_none() {
                    panel.production_progress = panel.production_queue.first().map(|(_, p, _)| *p);
                    panel.production_template =
                        panel.production_queue.first().map(|(t, _, _)| t.clone());
                }
                panel.production_paused = ro.production_paused;
                panel.ocl_timer_seconds = ro.ocl_timer_seconds;
                panel.sold = ro.sold;
                panel.production_is_upgrade = panel
                    .production_queue
                    .first()
                    .map(|(_, _, u)| *u)
                    .unwrap_or(false);
                if panel.veterancy_overlay.is_none() {
                    panel.veterancy_overlay = ro.veterancy.chevron_overlay().map(str::to_string);
                }
                panel.max_garrison = ro.max_garrison;
                panel.garrisoned_count = ro.garrisoned_units.len();
                panel.under_construction = ro.under_construction;
                panel.construction_percent = ro.construction_percent;
                panel.applied_upgrades = ro.applied_upgrades.clone();
                panel.upgrade_cameo_names = ro.upgrade_cameo_names.clone();
                panel.rally_point = ro.rally_point.map(|p| [p.x, p.y, p.z]);
                panel.special_power_ready = ro.special_power_ready;
                panel.special_power_cooldown_remaining = ro.special_power_cooldown_remaining;
                panel.special_power_cooldown = ro.special_power_cooldown;
            }
        }
        panel
    }

    /// Apply selection health/name to GameClient ControlBar without OBJECT_REGISTRY.
    ///
    /// Apply frozen skybox residual to the render pipeline without live GameLogic.
    pub fn apply_skybox_to_pipeline(
        &self,
        pipeline: &mut crate::graphics::render_pipeline::RenderPipeline,
    ) {
        pipeline.set_skybox_enabled(self.world_env.skybox_enabled);
        if let Some(textures) = self.world_env.skybox_textures.clone() {
            pipeline.set_skybox_hint(textures);
        }
    }

    /// Headless-safe: uses only presentation fields. Does not claim full WND shell.
    #[cfg(feature = "game_client")]

    /// Feed ControlBar HelpBox CanMake residual from presentation.

    /// Feed ConstructionPanel CanMake residual from presentation.
    pub fn apply_can_make_cameos_to_panel(
        &self,
        panel: &mut crate::ui::construction_panel::ConstructionPanel,
    ) {
        panel.apply_can_make_cameos(
            &self
                .can_make_cameos
                .iter()
                .map(|c| crate::ui::hud_state::CanMakeCameoUi {
                    template_name: c.template_name.clone(),
                    can_make: c.can_make,
                    available: c.available,
                    help_status: c.help_status.clone(),
                })
                .collect::<Vec<_>>(),
        );
    }

    pub fn apply_can_make_cameos_to_ui_state(&self, ui: &mut crate::ui::hud_state::GameUIState) {
        ui.can_make_cameos = self
            .can_make_cameos
            .iter()
            .map(|c| crate::ui::hud_state::CanMakeCameoUi {
                template_name: c.template_name.clone(),
                can_make: c.can_make,
                available: c.available,
                help_status: c.help_status.clone(),
            })
            .collect();
        ui.can_make_producer_id = self.can_make_producer_id;
    }

    pub fn apply_to_control_bar(
        &self,
        control_bar: &mut game_client::gui::control_bar::ControlBar,
    ) {
        let panel = self.control_bar_selection_panel();
        let ids: Vec<u32> = if !self.selected.is_empty() {
            self.selected.iter().map(|id| id.0).collect()
        } else {
            panel.unit_infos.iter().map(|u| u.object_id.0).collect()
        };
        let _ = control_bar.update_for_selection(ids);
        control_bar.apply_presentation_money(self.local_supplies as i32);
        control_bar.apply_presentation_can_make(
            &self
                .can_make_cameos
                .iter()
                .map(|c| (c.template_name.clone(), c.can_make))
                .collect::<Vec<_>>(),
        );

        let primary = self.objects.iter().find(|o| {
            Some(o.id) == panel.primary_object_id
                && !o.destroyed
                && !o.sold
                && !o.unselectable
                && !o.masked
        });
        let controllable = primary.is_some_and(|o| self.is_owned_by_local(o));
        control_bar.sync_selection_controllable_from_presentation(controllable);
        let empty_queue: Vec<(String, f32, bool)> = Vec::new();
        control_bar.sync_selection_display_from_presentation(
            panel.visible.then_some(panel.primary_name.as_str()),
            panel.health_current,
            panel.health_maximum,
            panel.selected_count,
            panel.veterancy_overlay.as_deref(),
            if controllable {
                panel.production_progress
            } else {
                None
            },
            if controllable {
                panel.production_template.as_deref()
            } else {
                None
            },
            if controllable {
                &panel.production_queue
            } else {
                &empty_queue
            },
            controllable && panel.production_paused,
        );
        let beacon =
            primary.is_some_and(|o| o.template_name.to_ascii_lowercase().contains("beacon"));
        let neutral_garrison = primary
            .is_some_and(|o| o.team == crate::game_logic::Team::Neutral && o.max_garrison > 0);
        let garrison_max = if controllable || neutral_garrison {
            panel.max_garrison
        } else {
            0
        };
        let occupants = primary
            .filter(|_| garrison_max > 0)
            .map(|ro| self.presentation_inventory_occupants(ro))
            .unwrap_or_default();
        control_bar.sync_presentation_occupants(occupants);
        control_bar.sync_structure_context_from_presentation(
            garrison_max,
            if garrison_max > 0 {
                panel.garrisoned_count
            } else {
                0
            },
            controllable && panel.under_construction,
            if controllable {
                panel.construction_percent
            } else {
                0.0
            },
        );
        // Wave 1031: OCL timer residual into ControlBar OclTimer dual path.
        control_bar.sync_ocl_timer_from_presentation(if controllable {
            panel.ocl_timer_seconds
        } else {
            0
        });
        let authored_cameos: Vec<String> = panel
            .upgrade_cameo_names
            .iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        let mut completed_upgrades = panel.applied_upgrades.clone();
        for name in &self.local_unlocked_sciences {
            if !completed_upgrades
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(name))
            {
                completed_upgrades.push(name.clone());
            }
        }
        {
            let mut residual = game_client::gui::control_bar::PresentationAvailabilityResidual {
                completed_upgrades: completed_upgrades.clone(),
                object_applied_upgrades: panel.applied_upgrades.clone(),
                player_completed_upgrades: self.local_completed_upgrades.clone(),
                object_unaffected_upgrades:
                    crate::game_logic::host_slave_drones::slave_drone_unaffected_upgrade_names(
                        panel.applied_upgrades.iter().map(String::as_str),
                    )
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                ..Default::default()
            };
            if let Some(ro) = primary.filter(|_| controllable) {
                residual.moving = ro.moving;
                residual.has_production = !ro.production_queue.is_empty();
                residual.occupant_count = ro.occupant_count as usize;
                residual.hacking_packing_or_unpacking = ro.hacking_packing_or_unpacking;
                residual.overcharge_active = ro.overcharge_enabled;
                residual.special_power_in_use = ro.using_ability;
                residual.weapon_reload_time = ro.weapon_reload_time;
                residual.weapon_fire_status = ro.weapon_fire_status;
                residual.has_primary_weapon = ro.has_weapon;
                residual.has_secondary_weapon = ro.has_secondary_weapon;
                residual.has_tertiary_weapon =
                    ro.projectile_clip_statuses[2].is_some() || ro.active_weapon_slot == 2;
                residual.active_weapon_slot = ro.active_weapon_slot;
                residual.battle_plan_bombardment = ro.weapon_bonus_battle_plan_bombardment;
                residual.battle_plan_hold_the_line = ro.weapon_bonus_battle_plan_hold_the_line;
                residual.battle_plan_search_and_destroy =
                    ro.weapon_bonus_battle_plan_search_and_destroy;
                residual.script_disabled = ro.disabled_script_disabled;
                residual.script_unpowered = ro.disabled_script_underpowered;
                residual.unmanned = ro.disabled_unmanned;
                residual.is_dozer_task_pending = ro.is_dozer_task_pending;
                residual.script_unsellable = ro.script_unsellable;
                residual.disabled_subdued = ro.disabled_subdued;
                residual.single_use_used = ro.single_use_command_used;
            }
            self.stamp_host_restrict_a_availability(
                &mut residual,
                primary.filter(|_| controllable),
            );
            control_bar.apply_presentation_availability(residual);
        }

        control_bar.sync_upgrade_cameos_from_presentation(
            &authored_cameos,
            &completed_upgrades,
            if controllable {
                panel.rally_point
            } else {
                None
            },
            controllable && panel.special_power_ready,
            if controllable {
                panel.special_power_cooldown_remaining
            } else {
                0.0
            },
            if controllable {
                panel.special_power_cooldown
            } else {
                0.0
            },
        );
        // Wave 1110: multi-select count residual excludes sold/unusable.
        // Disabled completed structures still own their command set
        // (C++ ControlBarCommand.cpp:1048-1078).
        let usable_selected = |o: &&RenderableObject| {
            o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked
        };
        let selected_count = if !self.selected.is_empty() {
            self.selected
                .iter()
                .filter(|id| {
                    self.objects
                        .iter()
                        .any(|o| o.id == **id && usable_selected(&o))
                })
                .count()
        } else {
            self.objects.iter().filter(usable_selected).count()
        };
        if !controllable && !beacon {
            control_bar.sync_command_set_from_presentation(None);
        } else if selected_count > 1 {
            let names = self.selected_command_set_names();
            control_bar.sync_multi_select_command_sets_from_presentation(&names);
        } else {
            control_bar.sync_command_set_from_presentation(
                self.selected_command_set_name()
                    .or_else(|| self.command_set_name_including_disabled()),
            );
        }
        control_bar.sync_sciences_from_presentation(&self.local_unlocked_sciences);
        control_bar.apply_presentation_science_hide_names(&self.local_unlocked_sciences);
        control_bar.sync_rank_progress_from_presentation(
            self.local_rank_progress_percent,
            self.local_skill_points,
            self.local_rank_level as i32,
            self.local_science_purchase_points,
        );
        let ready_sp: Vec<String> = self
            .selected_unit_display_infos()
            .iter()
            .filter_map(|info| {
                self.objects
                    .iter()
                    .find(|o| {
                        o.id == info.object_id
                            && o.special_power_ready
                            && !o.destroyed
                            && !o.sold
                            && !o.disabled
                    })
                    .map(|o| o.template_name.clone())
            })
            .collect();
        // Also include any selected renderable with ready SP (selection flags path).
        // Wave 1110: ready SP residual fail-closed on sold/disabled.
        let mut ready_sp = ready_sp;
        for o in self
            .objects
            .iter()
            .filter(|o| usable_selected(o) && o.special_power_ready)
        {
            if !ready_sp.iter().any(|n| n == &o.template_name) {
                ready_sp.push(o.template_name.clone());
            }
        }
        control_bar.sync_radar_queues_and_specials_from_presentation(
            self.local_radar_count,
            self.local_radar_disabled,
            &self.local_queued_upgrades,
            &ready_sp,
        );
    }

    /// Selection IDs for multi-consumer apply (player list or object.selected flags).
    pub fn selection_ids_for_consumers(&self) -> Vec<crate::game_logic::ObjectId> {
        // Wave 1106: consumer selection residual filters sold/unusable ids even
        // when the frozen selected list still holds them.
        let usable_id = |id: &ObjectId| {
            self.objects
                .iter()
                .any(|o| o.id == *id && !o.destroyed && !o.sold && !o.unselectable && !o.masked)
        };
        let mut ids: Vec<ObjectId> = self.selected.iter().copied().filter(usable_id).collect();
        if ids.is_empty() {
            ids = self
                .selected_unit_display_infos()
                .into_iter()
                .map(|i| i.object_id)
                .collect();
        }
        ids
    }

    /// Apply selection panel to RTS interface (command/selection residual consumer).
    pub fn apply_to_rts_interface(&self, rts: &mut crate::ui::RTSInterface) {
        rts.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
    }

    /// Apply selection panel to unit command grid (context-sensitive residual).

    /// C++ `ControlBar::populateCommand` — CommandSet.ini slots 1-14 only.
    /// Never invent Patrol / Scatter / Attitude / Cheer / Deploy / PurchaseScience.
    pub fn unit_command_buttons(&self) -> Vec<crate::ui::UnitCommandButton> {
        self.populate_command_set_strip()
    }

    pub fn apply_to_unit_command_panel(&self, panel: &mut crate::ui::UnitCommandPanel) {
        panel.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
        panel.apply_commands(self.unit_command_buttons());
    }

    /// C++ ControlBarCommand.cpp:1048-1078 — disabled completed structures
    /// still populate their command set (Sell/Stop/Rally stay evaluable).
    fn command_set_name_including_disabled(&self) -> Option<&str> {
        let usable =
            |o: &&RenderableObject| !o.destroyed && !o.sold && !o.unselectable && !o.masked;
        let primary = self.selected.first().copied().or_else(|| {
            self.objects
                .iter()
                .find(|o| o.selected && usable(o))
                .map(|o| o.id)
        })?;
        let o = self
            .objects
            .iter()
            .find(|o| o.id == primary && usable(&o))?;
        if !o.command_set_name.is_empty() {
            Some(o.command_set_name.as_str())
        } else if !o.command_set_override.is_empty() {
            Some(o.command_set_override.as_str())
        } else {
            None
        }
    }

    fn host_rappeller_count(&self, ro: &RenderableObject) -> usize {
        ro.garrisoned_units
            .iter()
            .filter(|id| match self.objects.iter().find(|o| o.id == **id) {
                Some(o) => {
                    !o.destroyed
                        && (o.kind_of.contains(&crate::game_logic::KindOf::Infantry) || o.is_unit)
                }
                None => true,
            })
            .count()
    }

    fn host_need_special_power_science_owned(&self, command_name: &str) -> bool {
        use crate::command_system::{CommandType, command_type_from_button_name};
        use crate::game_logic::host_special_power_enum_residual::special_power_required_science;
        let Some(CommandType::DoSpecialPower { power_type, .. }) =
            command_type_from_button_name(command_name)
        else {
            return true;
        };
        let Some(required) = special_power_required_science(&power_type) else {
            return true;
        };
        let req = required.to_ascii_lowercase().replace('-', "_");
        self.local_unlocked_sciences.iter().any(|owned| {
            owned.to_ascii_lowercase().replace('-', "_") == req
                || owned.eq_ignore_ascii_case(required)
        })
    }

    fn apply_host_disabled_command_strip(
        &self,
        cmds: &mut Vec<crate::ui::UnitCommandButton>,
        ro: &RenderableObject,
    ) {
        if ro.disabled_script_disabled || ro.disabled_script_underpowered || ro.disabled_unmanned {
            cmds.clear();
            return;
        }
        if !ro.disabled || ro.under_construction {
            return;
        }
        let flag_count = [
            ro.disabled_underpowered,
            ro.disabled_emp,
            ro.disabled_hacked,
            ro.disabled_unmanned,
            ro.disabled_paralyzed,
            ro.disabled_subdued,
        ]
        .into_iter()
        .filter(|f| *f)
        .count();
        let sole_underpowered = ro.disabled_underpowered && flag_count == 1;
        for cmd in cmds.iter_mut() {
            let n = cmd.command_name.to_ascii_lowercase();
            let evaluable = n.contains("sell")
                || n.contains("evacuate")
                || n.contains("structureexit")
                || n.ends_with("exit")
                || n.contains("beacondelete")
                || n.contains("setrallypoint")
                || n.contains("stop")
                || n.contains("switchweapon");
            let ignores_underpowered = n.contains("particlecannon")
                || n.contains("nuclearmissile")
                || n.contains("scudstorm")
                || n.contains("neutronmissile");
            if evaluable || (sole_underpowered && ignores_underpowered) {
                continue;
            }
            cmd.enabled = false;
        }
    }

    /// Dual-tick multi-consumer residual: HUD + UI state + RTS + unit command panel
    /// (+ ControlBar when `game_client` is enabled). Snapshot-owned only.
    ///
    /// Does **not** claim full windowed WND/GPU playthrough.
    pub fn apply_to_shell_ui_consumers(
        &self,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) {
        self.apply_to_game_hud(hud);
        self.apply_to_ui_state(ui);
        self.apply_to_rts_interface(rts);
        self.apply_to_unit_command_panel(command_panel);
    }

    /// Dual-tick presentation consumer after map load / logic step:
    /// build snapshot from authority and apply it to the production GameHUD.
    ///
    /// Does **not** advance the world — caller is responsible for `logic.update()`.
    pub fn build_and_apply_for_hud(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_game_hud(hud);
        frame.note_dual_tick_apply();
        frame
    }

    /// Dual-tick residual: build snapshot and apply to all headless shell UI consumers.
    ///
    /// Order matches production StartGame: authority step (caller) → presentation freeze
    /// → HUD / UIState / RTS / unit command panel. Optional ControlBar is applied by
    /// the engine path when `game_client` is present.
    pub fn build_and_apply_for_shell_consumers(
        logic: &GameLogic,
        local_player_id: u32,
        hud: &mut crate::ui::GameHUD,
        ui: &mut crate::ui::GameUIState,
        rts: &mut crate::ui::RTSInterface,
        command_panel: &mut crate::ui::UnitCommandPanel,
    ) -> Self {
        let mut frame = Self::build_from_logic(logic, local_player_id);
        frame.apply_to_shell_ui_consumers(hud, ui, rts, command_panel);
        frame.note_dual_tick_apply();
        frame
    }
}
