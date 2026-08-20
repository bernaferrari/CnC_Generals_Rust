use super::*;

impl PresentationFrame {
    /// Selected unit identity (health/name/type) from snapshot only.
    ///
    /// Prefer player selection list; fall back to objects marked selected on the frame
    /// when the player list is empty (common right after click-select before player list
    /// is mirrored).
    pub fn selected_unit_display_infos(&self) -> Vec<crate::ui::UnitDisplayInfo> {
        use crate::ui::UnitDisplayInfo;

        // Wave 1106: selection display residual fail-closed on sold/unselectable/
        // masked/disabled (not only destroyed) so ControlBar/RTS panel does not
        // keep UI for unusable selected objects. Under-construction is disabled
        // for combat, but C++ still shows the structure so CancelConstruction
        // remains reachable.
        let usable = |o: &RenderableObject| {
            !o.destroyed
                && !o.sold
                && !o.unselectable
                && !o.masked
                && (!o.disabled || o.under_construction)
        };
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
        use crate::ui::{color_for_player, BuildQueueEntry, MinimapDot};

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
        ui.new_beacons = self.new_beacons.clone();
        if !self.beacons.is_empty() {
            use crate::ui::{color_for_player, MinimapDot};
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
                    MinimapDot::normalized(
                        nx,
                        ny,
                        color_for_player(self.local_player_id.min(255) as u8),
                        4.0,
                    )
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
            .filter(|o| !o.destroyed && !o.sold)
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

    /// Units list for GameHud minimap: (id, x, z, team_color_index).
    pub fn hud_minimap_units(&self) -> Vec<(ObjectId, f32, f32, u8)> {
        // Wave 1109: minimap unit-dot residual excludes sold (alive dots only).
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
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
            let mut queue_items: Vec<(String, f32, i32, f32)> = Vec::new();
            let usable = |o: &&RenderableObject| {
                o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
            };
            if let Some(id) = self
                .selected
                .first()
                .copied()
                .or_else(|| self.objects.iter().find(usable).map(|o| o.id))
            {
                if let Some(ro) = self.objects.iter().find(|o| {
                    o.id == id
                        && !o.destroyed
                        && !o.sold
                        && !o.unselectable
                        && !o.masked
                        && !o.disabled
                }) {
                    for p in &ro.production_queue {
                        queue_items.push((
                            p.template_name.clone(),
                            p.progress_ratio,
                            p.cost_supplies as i32,
                            p.total_time.max(0.01),
                        ));
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
        // ControlBar unit command residual from primary selection freeze.
        let cmds: Vec<(String, bool)> = self
            .unit_command_buttons()
            .into_iter()
            .map(|b| (b.command_name, b.enabled))
            .collect();
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
                // C++: VoiceCreated / UnitReady / UpgradeComplete / EVA audio only.
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
    /// Wave 533: EvaAlert → EVA_* audio event names from host_eva_log pulses.
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
                let _ = sound;
                let mut req = AudioEventRequest::new("WeaponFireLoopStop").with_object(*unit);
                if let Some(pos) = pose_by_id.get(unit) {
                    req = req.with_position(*pos);
                }
                req = req.with_priority(200);
                out.push(req);
                continue;
            }
            let mapped: Option<(&str, Option<crate::game_logic::ObjectId>)> = match ev {
                PresentationEvent::ObjectDestroyed { id, .. } => Some(("UnitDie", Some(*id))),
                PresentationEvent::ConstructionComplete { id, .. } => {
                    Some(("BuildingComplete", Some(*id)))
                }
                PresentationEvent::UpgradeComplete { .. } => Some(("UpgradeComplete", None)),
                PresentationEvent::ProductionComplete { spawned, .. } => {
                    Some(("UnitReady", Some(*spawned)))
                }
                PresentationEvent::AttackTargeted { attacker, .. } => {
                    Some(("WeaponFire", Some(*attacker)))
                }
                PresentationEvent::DamageApplied {
                    target,
                    destroyed: true,
                    ..
                } => Some(("UnitDie", Some(*target))),
                PresentationEvent::DamageApplied {
                    target,
                    amount,
                    destroyed: false,
                    ..
                } => {
                    if *amount > 0.0 {
                        Some(("WeaponHit", Some(*target)))
                    } else {
                        None
                    }
                }
                PresentationEvent::HealApplied { target, .. } => Some(("UnitHeal", Some(*target))),
                PresentationEvent::EconomyChanged { .. } => Some(("MoneyTick", None)),
                PresentationEvent::Victory { .. } => Some(("Victory", None)),
                PresentationEvent::MoveOrdered { unit, .. } => Some(("UnitMove", Some(*unit))),
                PresentationEvent::WeaponFireLoopStarted { .. }
                | PresentationEvent::WeaponFireLoopStopped { .. }
                | PresentationEvent::WeaponDischarged { .. } => None,
                PresentationEvent::ParticleSystemSpawned { .. } => None, // handled below (Wave 535)
                PresentationEvent::OwnerChanged { .. } => None,          // handled below (Wave 530)
                PresentationEvent::RadarMessage { .. } => None,          // handled below (Wave 529)
                PresentationEvent::EvaAlert { .. } => None,              // handled below (Wave 533)
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

        // Wave 533: host EVA pulse audio residual (snapshot names, no live dual-read).
        for ev in &self.events {
            let PresentationEvent::EvaAlert { name } = ev else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            out.push(AudioEventRequest::new(name.as_str()).with_priority(180));
        }

        // Wave 535: combat particle spawn → presentation audio residual (snapshot pose).
        // Fail-closed: not full FXList/FXParticleSystemNames Miles matrix — kind→name map only.
        // Skip muzzle (WeaponFire already) and impact (WeaponHit already from DamageApplied).
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
                | CombatParticleKind::ProjectileExhaust => continue,
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
            let event_name = if t.contains("low power") || t.contains("power shortage") {
                "EVA_LowPower"
            } else if t.contains("insufficient funds") || t.contains("not enough money") {
                "EVA_InsufficientFunds"
            } else if t.contains("base under attack") || t.contains("our base is under attack") {
                "EVA_BaseUnderAttack"
            } else if t.contains("ally under attack") || t.contains("ally is under attack") {
                "EVA_AllyUnderAttack"
            } else if t.contains("building lost") || t.contains("structure lost") {
                "EVA_BuildingLost"
            } else if t.contains("unit lost") {
                "EVA_UnitLost"
            } else {
                match kind {
                    1 => "RadarAttack",
                    2 => "RadarAlly",
                    _ => "RadarGeneric",
                }
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
        let events = self.collect_audio_events();
        let n = events.len();
        for event in events {
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

    /// Legacy dual-write residual (tests may still call). Prefer `dispatch_audio_events_direct`.
    pub fn apply_events_to_audio(&self, logic: &mut GameLogic) -> usize {
        let events = self.collect_audio_events();
        let n = events.len();
        for req in events {
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
            if let Some(ro) = self.objects.iter().find(|o| {
                o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
            }) {
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
                panel.rally_point = ro.rally_point.map(|p| [p.x, p.y, p.z]);
                panel.special_power_ready = ro.special_power_ready;
                panel.special_power_cooldown_remaining = ro.special_power_cooldown_remaining;
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

        control_bar.sync_selection_display_from_presentation(
            panel.visible.then_some(panel.primary_name.as_str()),
            panel.health_current,
            panel.health_maximum,
            panel.selected_count,
            panel.veterancy_overlay.as_deref(),
            panel.production_progress,
            panel.production_template.as_deref(),
            &panel.production_queue,
            panel.production_paused,
        );
        control_bar.sync_structure_context_from_presentation(
            panel.max_garrison,
            panel.garrisoned_count,
            panel.under_construction,
            panel.construction_percent,
        );
        // Wave 1031: OCL timer residual into ControlBar OclTimer dual path.
        control_bar.sync_ocl_timer_from_presentation(panel.ocl_timer_seconds);
        control_bar.sync_sold_from_presentation(panel.sold);
        control_bar.sync_upgrades_and_specials_from_presentation(
            &panel.applied_upgrades,
            panel.rally_point,
            panel.special_power_ready,
            panel.special_power_cooldown_remaining,
        );
        // Wave 1110: multi-select count residual excludes sold/unusable.
        let usable_selected = |o: &&RenderableObject| {
            o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
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
        if selected_count > 1 {
            let names = self.selected_command_set_names();
            control_bar.sync_multi_select_command_sets_from_presentation(&names);
        } else {
            control_bar.sync_command_set_from_presentation(self.selected_command_set_name());
        }
        control_bar.sync_sciences_from_presentation(&self.local_unlocked_sciences);
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
            self.objects.iter().any(|o| {
                o.id == *id
                    && !o.destroyed
                    && !o.sold
                    && !o.unselectable
                    && !o.masked
                    && !o.disabled
            })
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

    /// Derive unit-command-panel buttons from primary selection residual.
    ///
    /// Fail-closed: not full CommandSet INI matrix / per-faction button layout.
    pub fn unit_command_buttons(&self) -> Vec<crate::ui::UnitCommandButton> {
        use crate::ui::UnitCommandButton;
        let panel = self.control_bar_selection_panel();
        let Some(id) = panel.primary_object_id else {
            return Vec::new();
        };
        // Wave 1107: unit command buttons residual fail-closed on sold/unusable primary.
        // Under-construction is combat-disabled but must still expose CancelConstruction.
        let Some(ro) = self.objects.iter().find(|o| {
            o.id == id
                && !o.destroyed
                && !o.sold
                && !o.unselectable
                && !o.masked
                && (!o.disabled || o.under_construction)
        }) else {
            return Vec::new();
        };
        let mut cmds = Vec::new();
        let push = |cmds: &mut Vec<UnitCommandButton>, name: &str, enabled: bool| {
            if !cmds
                .iter()
                .any(|c| c.command_name.eq_ignore_ascii_case(name))
            {
                cmds.push(UnitCommandButton {
                    command_name: name.into(),
                    enabled,
                });
            }
        };
        if ro.is_mobile || ro.is_unit {
            push(&mut cmds, "Command_Stop", true);
            push(&mut cmds, "Command_AttackMove", ro.has_weapon);
            push(&mut cmds, "Command_Guard", true);
            push(&mut cmds, "Command_Patrol", true);
            push(&mut cmds, "Command_Scatter", true);
            push(&mut cmds, "Command_AttitudeAggressive", true);
            push(&mut cmds, "Command_AttitudePassive", true);
            push(&mut cmds, "Command_AttitudeSleep", true);
            push(&mut cmds, "Command_SwitchWeapons", true);
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("supply")
                    || n.contains("harvester")
                    || n.contains("chinook")
                    || (n.contains("worker") && !n.contains("dozer"))
                {
                    push(&mut cmds, "Command_ReturnSupplies", true);
                }
            }
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("chinook") {
                    push(&mut cmds, "Command_CombatDrop", true);
                }
                if n.contains("jet")
                    || n.contains("raptor")
                    || n.contains("mig")
                    || n.contains("aurora")
                    || n.contains("stealth")
                    || n.contains("comanche")
                    || n.contains("helicopter")
                    || n.contains("chinook")
                {
                    push(&mut cmds, "Command_ReturnToBase", true);
                }
            }
            // Dozer/Worker repair residual (R key / strip).
            {
                let n = ro.template_name.to_ascii_lowercase();
                if n.contains("dozer")
                    || n.contains("worker")
                    || n.contains("chinook")
                    || n.contains("construction")
                    || n.contains("supplytruck")
                    || n.contains("supply_truck")
                {
                    push(&mut cmds, "Command_Repair", true);
                }
                if n.contains("dozer") || n.contains("worker") {
                    push(&mut cmds, "Command_ClearMines", true);
                }
            }
            push(&mut cmds, "Command_Cheer", true);
            // Multi-select formation residual.
            if self.selection_ids_for_consumers().len() >= 2 {
                push(&mut cmds, "Command_CreateFormation", true);
            }
            // C++ Deploy residual (DeployStyle / sentry / crawler family).
            let n = ro.template_name.to_ascii_lowercase();
            if n.contains("sentry")
                || n.contains("nukecannon")
                || n.contains("scud")
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("stinger")
                || n.contains("tunnel")
                || n.contains("tomahawk")
                || n.contains("humvee")
                || n.contains("buggy")
                || n.contains("crawler")
                || n.contains("quadcannon")
                || n.contains("inferno")
                || n.contains("artillery")
                || n.contains("spectrum")
            {
                push(&mut cmds, "Command_Deploy", true);
            }
            // Hero / special-ability residual (target-click armed by engine).
            if n.contains("jarmenkell") || n.contains("jarmen_kell") {
                push(&mut cmds, "Command_SnipeVehicle", true);
            }
            if n.contains("colonelburton") || n.contains("colonel_burton") || n.contains("burton") {
                push(&mut cmds, "Command_PlantTimedDemoCharge", true);
                push(&mut cmds, "Command_PlantRemoteDemoCharge", true);
                push(&mut cmds, "Command_DetonateRemoteDemoCharges", true);
            }
            if n.contains("blacklotus") || n.contains("black_lotus") {
                push(&mut cmds, "Command_CaptureBuilding", true);
                push(&mut cmds, "Command_StealCashHack", true);
                push(&mut cmds, "Command_DisableVehicleHack", true);
            }
            if n.contains("chinainfantryhacker")
                || (n.contains("hacker") && n.contains("china"))
                || n.contains("china_hacker")
            {
                push(&mut cmds, "Command_HackInternet", true);
            }
            if n.contains("ambulance") {
                push(&mut cmds, "Command_CleanupArea", true);
            }
            if n.contains("hijacker") {
                push(&mut cmds, "Command_Hijack", true);
            }
            if n.contains("saboteur") {
                push(&mut cmds, "Command_Sabotage", true);
            }
            if n.contains("bombtruck") || n.contains("bomb_truck") {
                push(&mut cmds, "Command_DisguiseAsVehicle", true);
                push(&mut cmds, "Command_ConvertToCarbomb", true);
            }
            if n.contains("rebel") && !n.contains("scud") {
                push(&mut cmds, "Command_CaptureBuilding", true);
                push(&mut cmds, "Command_PlantBoobyTrap", true);
            }
            if n.contains("ranger") || n.contains("redguard") {
                push(&mut cmds, "Command_CaptureBuilding", true);
            }
            if n.contains("demo")
                && (n.contains("terrorist") || n.contains("bike") || n.contains("trap"))
            {
                push(&mut cmds, "Command_DemoTertiarySuicide", true);
            }
        }
        // C++ HDB is a paired SpecialAbility/SpecialAbilityUpdate channel,
        // not a China Hacker name.  Keep it outside the legacy hacker block
        // so a modded or renamed object gets exactly its parsed button, and a
        // stale/charging module produces a visible but disabled control.
        if ro.hacker_disable_building_capable {
            push(
                &mut cmds,
                "Command_HackerDisableBuilding",
                ro.hacker_disable_building_ready,
            );
        }
        // C++ command availability comes from the concrete behavior module,
        // not a China/power-plant name.  It is intentionally outside the
        // structure-only branch: an authored module remains the authority.
        if ro.can_toggle_overcharge {
            push(&mut cmds, "Command_ToggleOvercharge", true);
        }
        if ro.is_structure || ro.can_produce {
            if ro.under_construction {
                push(&mut cmds, "Command_CancelConstruction", true);
                push(&mut cmds, "Command_ResumeConstruction", true);
            } else if ro.is_structure {
                // C++ Command_Sell residual — completed structures only.
                push(&mut cmds, "Command_Sell", true);
                let n = ro.template_name.to_ascii_lowercase();
                // USA Strategy Center battle plans residual.
                if n.contains("strategycenter") || n.contains("strategy_center") {
                    push(&mut cmds, "Command_InitiateBattlePlanBombardment", true);
                    push(&mut cmds, "Command_InitiateBattlePlanHoldTheLine", true);
                    push(
                        &mut cmds,
                        "Command_InitiateBattlePlanSearchAndDestroy",
                        true,
                    );
                    push(&mut cmds, "Command_CIAIntelligence", true);
                }
                // Structure superweapon buttons come from the exact parsed
                // SpecialPowerTemplate frozen into this frame.  A structure
                // basename is not authority: a name-spoof without an actual
                // SpecialPowerModule must not expose a fire button.
                match ro
                    .special_power_ready_template_name
                    .as_deref()
                    .and_then(crate::command_system::special_power_type_from_template_name)
                {
                    Some(
                        crate::command_system::SpecialPowerType::ParticleCannon
                        | crate::command_system::SpecialPowerType::SuperweaponParticleCannon
                        | crate::command_system::SpecialPowerType::LaserCannon,
                    ) => push(&mut cmds, "Command_ParticleCannon", true),
                    Some(
                        crate::command_system::SpecialPowerType::NuclearMissile
                        | crate::command_system::SpecialPowerType::NukeNeutronMissile
                        | crate::command_system::SpecialPowerType::SuperweaponNeutronMissile,
                    ) => push(&mut cmds, "Command_NuclearMissile", true),
                    Some(crate::command_system::SpecialPowerType::ScudStorm) => {
                        push(&mut cmds, "Command_ScudStorm", true)
                    }
                    _ => {}
                }
                if n.contains("spysat") || (n.contains("satellite") && n.contains("uplink")) {
                    push(&mut cmds, "Command_SpySatelliteScan", true);
                }
                if n.contains("airfield") {
                    push(&mut cmds, "Command_SpyDrone", true);
                    push(&mut cmds, "Command_EmergencyRepair", true);
                    push(&mut cmds, "Command_Airstrike", true);
                    push(&mut cmds, "Command_CarpetBomb", true);
                }
                if n.contains("commandcenter") || n.contains("command_center") {
                    push(&mut cmds, "Command_ArtilleryBarrage", true);
                    push(&mut cmds, "Command_EmergencyRepair", true);
                    // Faction generals-power residual buttons on CC.
                    if n.contains("gla") {
                        push(&mut cmds, "Command_Ambush", true);
                        push(&mut cmds, "Command_SneakAttack", true);
                        push(&mut cmds, "Command_AnthraxBomb", true);
                    }
                    if n.contains("america") || n.contains("usa") {
                        push(&mut cmds, "Command_LeafletDrop", true);
                        push(&mut cmds, "Command_GpsScrambler", true);
                        push(&mut cmds, "Command_SpectreGunship", true);
                    }
                    if n.contains("china") {
                        push(&mut cmds, "Command_ArtilleryBarrage", true);
                        push(&mut cmds, "Command_CarpetBomb", true);
                    }
                }
            }
            if ro.can_produce {
                push(&mut cmds, "Command_SetRallyPoint", true);
            }
            if ro.max_garrison > 0 {
                push(&mut cmds, "Command_StructureExit", true);
                if !ro.garrisoned_units.is_empty() {
                    push(&mut cmds, "Command_Evacuate", true);
                }
            }
        }
        if ro.special_power_ready {
            push(&mut cmds, "Command_SpecialPower", true);
        } else if ro.special_power_cooldown > 0.0 {
            push(&mut cmds, "Command_SpecialPower", false);
        }
        // GeneralsExperience residual: offer purchase when SPP available.
        if self.local_science_purchase_points > 0 {
            push(&mut cmds, "Command_PurchaseScience", true);
        }
        if panel.production_progress.is_some() {
            // C++ cancel queue head: unit vs upgrade residual.
            if panel.production_is_upgrade {
                push(&mut cmds, "Command_CancelUpgrade", true);
            } else {
                push(&mut cmds, "Command_CancelUnit", true);
            }
        }
        // C++ GUI_COMMAND_PLAYER_UPGRADE residual: disable upgrade commands that
        // are already complete or currently researching (COMMAND_RESTRICTED).
        let queued = &self.local_queued_upgrades;
        let unlocked = &self.local_unlocked_sciences;
        for name in unlocked.iter().chain(queued.iter()) {
            let cmd = format!(
                "Command_Upgrade{}",
                name.trim_start_matches("Upgrade_")
                    .trim_start_matches("upgrade_")
            );
            // Also try full Upgrade_ name suffix residual.
            let cmd_full = format!("Command_{}", name);
            for cname in [cmd, cmd_full] {
                // Only mark disabled if a matching enable was not already pushed.
                if let Some(existing) = cmds
                    .iter_mut()
                    .find(|c| c.command_name.eq_ignore_ascii_case(&cname))
                {
                    existing.enabled = false;
                }
            }
        }
        // Structure producers: expose residual upgrade research buttons from
        // applied/known host upgrades that are NOT unlocked and NOT queued.
        // Fail-closed: sample residual set only (not full CommandSet INI matrix).
        if ro.can_produce || ro.is_structure {
            const SAMPLE_UPGRADE_COMMANDS: &[(&str, &str)] = &[
                (
                    "Command_UpgradeAmericaRangerFlashBangGrenade",
                    "Upgrade_AmericaRangerFlashBangGrenade",
                ),
                (
                    "Command_UpgradeAmericaRangerCaptureBuilding",
                    "Upgrade_AmericaRangerCaptureBuilding",
                ),
                (
                    "Command_UpgradeAmericaSupplyLines",
                    "Upgrade_AmericaSupplyLines",
                ),
                ("Command_UpgradeGLACamouflage", "Upgrade_GLACamouflage"),
            ];
            let norm = |s: &str| {
                s.chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect::<String>()
            };
            for (cmd, upgrade) in SAMPLE_UPGRADE_COMMANDS {
                let u = norm(upgrade);
                let owned = unlocked.iter().any(|x| norm(x) == u)
                    || unlocked
                        .iter()
                        .any(|x| norm(x).contains(&u) || u.contains(&norm(x)));
                let researching = queued.iter().any(|x| norm(x) == u)
                    || queued
                        .iter()
                        .any(|x| norm(x).contains(&u) || u.contains(&norm(x)))
                    || (panel.production_is_upgrade
                        && panel
                            .production_template
                            .as_ref()
                            .map(|t| norm(t) == u || norm(t).contains(&u))
                            .unwrap_or(false));
                let enabled = !owned && !researching;
                // Only push when structure can produce (barracks/war factory/etc residual).
                if ro.can_produce {
                    push(&mut cmds, cmd, enabled);
                }
            }
        }
        cmds
    }

    pub fn apply_to_unit_command_panel(&self, panel: &mut crate::ui::UnitCommandPanel) {
        panel.apply_selection_panel(
            self.control_bar_selection_panel(),
            self.selection_ids_for_consumers(),
        );
        panel.apply_commands(self.unit_command_buttons());
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
