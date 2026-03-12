//! Egui-based HUD Implementation
//!
//! This module implements the in-game HUD using the egui immediate mode GUI framework.
//! It provides resource display, selection info, build queues, and minimap visualization.
//!
//! References C++ InGameUI.h (line 322) for UI structure and behavior.

use super::hud::{localized_command, localized_entry};
use crate::command_system::{
    CommandType, GameCommand as CmdGameCommand, GuardTarget, ModifierKeys, PowerTarget,
    SpecialPowerType,
};
use crate::game_logic::{
    victory::{format_duration, PlayerOutcome, PlayerResult, VictorySummary},
    ObjectId, Team,
};
use crate::graphics::MinimapCoordinates;
use crate::localization;
use crate::ui::objectives::{ObjectiveCategory, ObjectiveDisplay, ObjectiveStatus};
use egui::{
    Align2, Area, Button, Color32, Context, Id, Image, Order, Pos2, ProgressBar, Rect, RichText,
    Sense, Stroke, StrokeKind, TextureId, Vec2,
};
use glam::Vec3;
#[cfg(feature = "integration-diagnostics")]
use integration::diagnostics::SystemDiagnostics;
use log::debug;
use std::collections::VecDeque;
use std::time::SystemTime;

type PlayerStatAccessor = fn(&PlayerResult) -> u32;

/// UI State extracted from game logic
/// This struct holds all the data needed to render the UI each frame
#[derive(Debug, Clone)]
pub struct GameUIState {
    // Resources (from C++ InGameUI::m_player->getMoney(), getPower())
    pub credits: i32,
    pub power_generated: i32,
    pub power_used: i32,
    pub max_power: i32,
    pub credits_per_second: f32,
    pub player_id: u32,
    pub player_name: String,

    // Selection state (from C++ InGameUI::m_selectedDrawables)
    pub selected_units: Vec<ObjectId>,
    pub selected_unit_infos: Vec<UnitDisplayInfo>,

    // Build queue (from C++ InGameUI::m_buildProgress)
    pub build_queue: Vec<BuildQueueEntry>,

    // Game status
    pub is_game_paused: bool,
    pub current_game_time: f32,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub performance_score: f32,
    pub asset_memory_mb: f32,
    pub asset_cache_usage: f32,
    pub assets_loaded: u64,
    pub show_debug_overlay: bool,
    pub diagnostics: Option<DiagnosticsOverlayStats>,
    pub match_over: bool,
    pub player_outcome: Option<PlayerOutcome>,
    pub victory_summary: Option<VictorySummary>,

    // Minimap data
    pub minimap_unit_dots: Vec<MinimapDot>,
    pub minimap_beacons: Vec<MinimapDot>,
    pub minimap_viewport: Rect,
    pub minimap_texture_id: Option<TextureId>,
    pub minimap_coordinates: Option<MinimapCoordinates>,
    /// Human-readable radar/EVA messages (deprecated, kept for legacy HUD).
    pub radar_messages: Vec<String>,
    /// Rich radar events with location/kind for focus buttons.
    pub radar_events: Vec<RadarMessageEntry>,
    pub radar_pings: Vec<RadarPing>,
    pub last_radar_ping: Option<Vec3>,
    /// Fresh beacon positions for HUD/minimap highlighting this frame.
    pub new_beacons: Vec<Vec3>,
    pub script_messages: Vec<String>,
    pub cinematic_letterbox: bool,
    pub cinematic_text: Option<String>,
    pub military_caption: Option<String>,
    pub radar_enabled: bool,
    pub objectives: Vec<ObjectiveDisplay>,
}

#[derive(Debug, Clone, Default)]
pub struct RadarPing {
    pub position: Vec3,
    /// 0..1 intensity (will be mapped to alpha/size)
    pub intensity: f32,
    /// Age in seconds for fade-out/pulse animation
    pub age_seconds: f32,
    pub kind: RadarPingKind,
}

/// Display entry for radar text with optional focus target.
#[derive(Debug, Clone, Default)]
pub struct RadarMessageEntry {
    pub text: String,
    pub position: Option<Vec3>,
    pub kind: RadarPingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadarPingKind {
    #[default]
    Generic,
    Attack,
    Ally,
}

/// Transient radar highlight when user clicks a ping.
#[derive(Debug, Clone, Copy)]
pub struct RadarHighlight {
    pub position: Vec3,
    pub timer: f32,
}

impl Default for GameUIState {
    fn default() -> Self {
        Self {
            credits: 10000,
            power_generated: 100,
            power_used: 60,
            max_power: 100,
            credits_per_second: 5.0,
            player_id: 0,
            player_name: localization::localize("hud.commander", "Commander"),
            selected_units: Vec::new(),
            selected_unit_infos: Vec::new(),
            build_queue: Vec::new(),
            is_game_paused: false,
            current_game_time: 0.0,
            fps: 60.0,
            frame_time_ms: 16.6,
            performance_score: 1.0,
            asset_memory_mb: 0.0,
            asset_cache_usage: 0.0,
            assets_loaded: 0,
            show_debug_overlay: false,
            diagnostics: None,
            match_over: false,
            player_outcome: None,
            victory_summary: None,
            minimap_unit_dots: Vec::new(),
            minimap_beacons: Vec::new(),
            minimap_viewport: Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
            minimap_texture_id: None,
            minimap_coordinates: None,
            radar_messages: Vec::new(),
            radar_events: Vec::new(),
            radar_pings: Vec::new(),
            last_radar_ping: None,
            new_beacons: Vec::new(),
            script_messages: Vec::new(),
            cinematic_letterbox: false,
            cinematic_text: None,
            military_caption: None,
            radar_enabled: true,
            objectives: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsOverlayStats {
    pub health_score: f32,
    pub engine: f32,
    pub graphics: f32,
    pub audio: f32,
    pub network: f32,
    pub logic: f32,
    pub warnings: u32,
    pub errors: u32,
    pub critical_errors: u32,
}

impl DiagnosticsOverlayStats {
    pub fn from_overall(health_percent: f32) -> Self {
        let clamped = health_percent.clamp(0.0, 150.0);
        Self {
            health_score: clamped,
            engine: clamped,
            graphics: clamped,
            audio: clamped,
            network: clamped,
            logic: clamped,
            warnings: 0,
            errors: 0,
            critical_errors: 0,
        }
    }

    #[cfg(feature = "integration-diagnostics")]
    pub fn from_system(diag: &SystemDiagnostics) -> Self {
        Self {
            health_score: diag.health_score as f32,
            engine: diag.subsystem_health.engine as f32,
            graphics: diag.subsystem_health.graphics as f32,
            audio: diag.subsystem_health.audio as f32,
            network: diag.subsystem_health.network as f32,
            logic: diag.subsystem_health.logic as f32,
            warnings: diag.error_counts.warnings,
            errors: diag.error_counts.errors,
            critical_errors: diag.error_counts.critical_errors,
        }
    }
}

/// Individual unit display information
#[derive(Debug, Clone)]
pub struct UnitDisplayInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub health_current: f32,
    pub health_maximum: f32,
    pub unit_type: String,
    pub current_order: String,
}

/// Build queue entry (matches C++ BuildProgress struct line 156)
#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    pub template_name: String,
    pub percent_complete: f32,
    pub time_remaining: f32,
}

/// Minimap unit dot for rendering
#[derive(Debug, Clone)]
pub struct MinimapDot {
    pub position: Pos2, // Normalized 0.0-1.0
    pub color: Color32,
    pub size: f32,
}

/// Kind of minimap interaction requested by the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapActionKind {
    LeftClick,
    LeftDrag,
    RightClick,
}

/// Interaction emitted by the minimap widget.
#[derive(Debug, Clone, Copy)]
pub struct MinimapInteraction {
    pub screen_position: Pos2,
    pub kind: MinimapActionKind,
}

/// Pending command that requires target selection
#[derive(Debug, Clone)]
pub enum PendingCommand {
    Move,
    Attack,
    Guard,
    ForceAttack,
    Repair,
    SpecialPower(SpecialPowerType),
}

/// Selected unit for detailed view
#[derive(Debug, Clone)]
pub struct SelectedUnitView {
    pub object_id: ObjectId,
    pub is_highlighted: bool,
}

/// Egui-based HUD renderer
/// Matches functionality from C++ InGameUI class (InGameUI.h line 322)
pub struct EguiHUD {
    /// Power bar animation state
    power_warning_flash: f32,

    /// Low power warning shown this frame
    low_power_warning_shown: bool,

    /// Minimap size in pixels
    minimap_size: f32,

    /// Command queue for UI-generated commands
    pub command_queue: VecDeque<CmdGameCommand>,

    /// Current player ID
    pub player_id: u32,

    /// Command ID counter
    next_command_id: u32,

    /// Pending command waiting for target selection
    pub pending_command: Option<PendingCommand>,

    /// Currently selected unit in the UI list
    pub selected_unit_view: Option<ObjectId>,

    /// Track build mode for UI feedback
    pub is_build_mode: bool,

    /// Cached minimap rectangle for conversion in the engine
    last_minimap_rect: Option<Rect>,

    /// Pending minimap interaction to be processed by the engine
    pending_minimap_action: Option<MinimapInteraction>,
    /// Pending victory overlay action (if any)
    victory_action: Option<VictoryOverlayAction>,
    /// Short-lived radar highlight for camera focus clicks
    radar_highlight: Option<RadarHighlight>,
    /// Bloom/highlight rings for freshly placed beacons.
    beacon_highlights: Vec<RadarHighlight>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryOverlayAction {
    ExitToMenu,
}

impl EguiHUD {
    /// Create new egui HUD instance
    pub fn new() -> Self {
        Self {
            power_warning_flash: 0.0,
            low_power_warning_shown: false,
            minimap_size: 200.0,
            command_queue: VecDeque::new(),
            player_id: 0,
            next_command_id: 1,
            pending_command: None,
            selected_unit_view: None,
            is_build_mode: false,
            last_minimap_rect: None,
            pending_minimap_action: None,
            victory_action: None,
            radar_highlight: None,
            beacon_highlights: Vec::new(),
        }
    }

    fn apply_elastic_click(&self, ui: &egui::Ui, response: &egui::Response) {
        let id = response.id.with("elastic_click");
        let now = ui.input(|i| i.time);
        if response.clicked() {
            ui.ctx().data_mut(|data| data.insert_persisted(id, now));
        }

        let last_click = ui
            .ctx()
            .data_mut(|data| data.get_persisted::<f64>(id).unwrap_or(-1000.0));

        let mut scale = 1.0f32;
        if response.is_pointer_button_down_on() {
            scale *= 0.96;
        }

        let elapsed = (now - last_click) as f32;
        if elapsed >= 0.0 && elapsed <= 0.35 {
            let damping = 12.0;
            let frequency = 24.0;
            let amplitude = 0.06;
            let spring = (-damping * elapsed).exp() * (frequency * elapsed).sin();
            scale *= 1.0 + amplitude * spring;
            ui.ctx().request_repaint();
        } else if elapsed > 0.35 {
            ui.ctx().data_mut(|data| data.remove::<f64>(id));
        }

        if (scale - 1.0).abs() < 0.001 {
            return;
        }

        let rect = response.rect;
        let scaled = rect.scale_from_center(scale);
        let visuals = ui.visuals();
        let rounding = visuals.widgets.active.rounding();
        let fill = visuals.widgets.active.bg_fill;
        let stroke = visuals.widgets.active.bg_stroke;

        let fill = Color32::from_rgba_premultiplied(
            fill.r(),
            fill.g(),
            fill.b(),
            ((fill.a() as f32) * 0.35) as u8,
        );
        let stroke = Stroke {
            width: stroke.width,
            color: Color32::from_rgba_premultiplied(
                stroke.color.r(),
                stroke.color.g(),
                stroke.color.b(),
                ((stroke.color.a() as f32) * 0.55) as u8,
            ),
        };

        ui.painter().rect_filled(scaled, rounding, fill);
        ui.painter()
            .rect_stroke(scaled, rounding, stroke, StrokeKind::Outside);
    }

    fn elastic_response(&self, ui: &egui::Ui, response: egui::Response) -> egui::Response {
        self.apply_elastic_click(ui, &response);
        response
    }

    pub fn reset_match_state(&mut self) {
        self.command_queue.clear();
        self.pending_command = None;
        self.pending_minimap_action = None;
        self.selected_unit_view = None;
        self.is_build_mode = false;
        self.last_minimap_rect = None;
        self.victory_action = None;
        self.low_power_warning_shown = false;
        self.power_warning_flash = 0.0;
        self.player_id = 0;
        self.next_command_id = 1;
        self.beacon_highlights.clear();
    }

    /// Update HUD state (called before rendering)
    pub fn update(&mut self, delta_time: f32) {
        // Update animation timers
        self.power_warning_flash += delta_time * 2.0;
        if self.power_warning_flash > std::f32::consts::TAU {
            self.power_warning_flash = 0.0;
        }

        // Decay temporary highlights for radar/beacon focus.
        if let Some(highlight) = &mut self.radar_highlight {
            highlight.timer -= delta_time;
            if highlight.timer <= 0.0 {
                self.radar_highlight = None;
            }
        }
        for highlight in &mut self.beacon_highlights {
            highlight.timer -= delta_time;
        }
        self.beacon_highlights.retain(|h| h.timer > 0.0);
    }

    /// Manually clear any beacon highlights (e.g., when a beacon is removed).
    pub fn clear_beacon_highlights(&mut self) {
        self.beacon_highlights.clear();
    }

    /// Render the complete HUD
    /// Matches C++ InGameUI::draw() pattern (InGameUI.h line 467)
    pub fn render(&mut self, ctx: &Context, ui_state: &GameUIState) {
        self.player_id = ui_state.player_id;

        // Top resource panel
        self.render_resource_panel(ctx, ui_state);
        self.render_objectives_panel(ctx, ui_state);

        // Selection info (bottom left)
        if !ui_state.selected_units.is_empty() {
            self.render_selection_panel(ctx, ui_state);
        }

        // Build queue (bottom center)
        if !ui_state.build_queue.is_empty() {
            self.render_build_queue_panel(ctx, ui_state);
        }

        // Minimap (bottom right)
        self.render_minimap(ctx, ui_state);

        // Game time display (top right)
        self.render_game_time(ctx, ui_state);

        if ui_state.show_debug_overlay {
            self.render_debug_overlay(ctx, ui_state);
        }

        if ui_state.match_over {
            if let Some(summary) = &ui_state.victory_summary {
                self.render_victory_overlay(ctx, summary, ui_state.player_outcome);
            }
        }

        self.render_military_caption(ctx, ui_state);
        self.render_cinematic_overlay(ctx, ui_state);
    }

    fn render_military_caption(&self, ctx: &Context, ui_state: &GameUIState) {
        let Some(text) = ui_state.military_caption.as_deref() else {
            return;
        };

        let screen = ctx.screen_rect();
        let width = (screen.width() * 0.72).clamp(320.0, 960.0);
        let height = 46.0;
        let rect = Rect::from_center_size(
            Pos2::new(screen.center().x, screen.min.y + 70.0),
            Vec2::new(width, height),
        );

        let painter = ctx.layer_painter(egui::LayerId::new(
            Order::Foreground,
            Id::new("military_caption"),
        ));

        let bg = Color32::from_rgba_premultiplied(8, 16, 18, 200);
        let border = Color32::from_rgba_premultiplied(120, 170, 180, 200);
        painter.rect_filled(rect, 4.0, bg);
        painter.rect_stroke(rect, 4.0, Stroke::new(1.2, border), StrokeKind::Outside);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(20.0),
            Color32::from_rgb(235, 245, 246),
        );
    }

    fn render_cinematic_overlay(&self, ctx: &Context, ui_state: &GameUIState) {
        if !ui_state.cinematic_letterbox && ui_state.cinematic_text.is_none() {
            return;
        }

        let screen = ctx.screen_rect();
        let bar_height = (screen.height() * 0.12).clamp(40.0, 140.0);
        let top_bar = egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), bar_height));
        let bottom_bar = egui::Rect::from_min_size(
            egui::pos2(screen.min.x, screen.max.y - bar_height),
            egui::vec2(screen.width(), bar_height),
        );

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("cinematic_overlay"),
        ));

        if ui_state.cinematic_letterbox {
            let color = egui::Color32::from_black_alpha(220);
            painter.rect_filled(top_bar, 0.0, color);
            painter.rect_filled(bottom_bar, 0.0, color);
        }

        if let Some(text) = ui_state.cinematic_text.as_deref() {
            let pos = egui::pos2(screen.center().x, bottom_bar.min.y - 8.0);
            painter.text(
                pos,
                egui::Align2::CENTER_BOTTOM,
                text,
                egui::FontId::proportional(26.0),
                egui::Color32::from_rgb(235, 235, 235),
            );
        }
    }

    /// Render resource panel at top of screen
    /// References C++ resource display from InGameUI
    fn render_resource_panel(&mut self, ctx: &Context, ui_state: &GameUIState) {
        egui::TopBottomPanel::top("resource_panel")
            .resizable(false)
            .min_height(40.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 20.0;

                    let commander_label = localization::localize("hud.commander", "Commander");
                    ui.label(
                        RichText::new(format!("{commander_label}: {}", ui_state.player_name))
                            .strong(),
                    );
                    ui.separator();

                    if ui_state.radar_enabled
                        && (!ui_state.radar_events.is_empty()
                            || !ui_state.radar_messages.is_empty())
                    {
                        let entries: Vec<(String, Option<Vec3>, RadarPingKind)> = if !ui_state
                            .radar_events
                            .is_empty()
                        {
                            ui_state
                                .radar_events
                                .iter()
                                .map(|e| (e.text.clone(), e.position, e.kind))
                                .collect()
                        } else {
                            ui_state
                                .radar_messages
                                .iter()
                                .map(|m| {
                                    (m.clone(), ui_state.last_radar_ping, RadarPingKind::Generic)
                                })
                                .collect()
                        };

                        for (text, pos, kind) in entries {
                            ui.horizontal(|ui| {
                                let (color, icon) = match kind {
                                    RadarPingKind::Attack => (
                                        Color32::from_rgb(255, 140, 140),
                                        "⚔", // Attack indicator
                                    ),
                                    RadarPingKind::Ally => (
                                        Color32::from_rgb(140, 200, 255),
                                        "🛡", // Ally/assist indicator
                                    ),
                                    RadarPingKind::Generic => {
                                        (Color32::from_rgb(255, 220, 120), "•")
                                    }
                                };
                                ui.label(RichText::new(icon).color(color).strong());
                                ui.label(RichText::new(&text).color(color).italics());
                                if let Some(target) = pos.or(ui_state.last_radar_ping) {
                                    let label =
                                        localization::localize("hud.egui.view_radar", "View");
                                    let raw_response = ui.button(label);
                                    let response = self.elastic_response(ui, raw_response);
                                    if response.clicked() {
                                        // Queue a view-radar command to snap camera like C++.
                                        let cmd = self.create_ui_command(
                                            CommandType::ViewRadarAt { position: target },
                                            vec![],
                                        );
                                        self.command_queue.push_back(cmd);
                                        self.radar_highlight = Some(RadarHighlight {
                                            position: target,
                                            timer: 1.0,
                                        });
                                        // Also flash a minimap bloom ring for this ping.
                                        self.beacon_highlights.push(RadarHighlight {
                                            position: target,
                                            timer: 1.0,
                                        });
                                    }
                                }
                            });
                        }
                        ui.separator();
                    }

                    if !ui_state.script_messages.is_empty() {
                        for message in &ui_state.script_messages {
                            ui.label(
                                RichText::new(message)
                                    .color(Color32::from_rgb(160, 220, 255))
                                    .strong(),
                            );
                        }
                        ui.separator();
                    }

                    // Credits display with income indicator
                    let credits_color = if ui_state.credits_per_second >= 0.0 {
                        Color32::from_rgb(100, 255, 100) // Green for positive income
                    } else {
                        Color32::from_rgb(255, 100, 100) // Red for deficit
                    };

                    let credits_label = localization::localize("hud.credits", "Credits");
                    ui.colored_label(
                        credits_color,
                        format!("{credits_label}: ${}", ui_state.credits),
                    );

                    if ui_state.credits_per_second.abs() > 0.01 {
                        ui.label(format!("({:+.1}/s)", ui_state.credits_per_second));
                    }

                    ui.separator();

                    // Power display with color coding
                    let power_available = ui_state.power_generated - ui_state.power_used;
                    let power_percentage = if ui_state.max_power > 0 {
                        power_available as f32 / ui_state.max_power as f32
                    } else {
                        0.0
                    };

                    // Color based on power availability (matches C++ power bar colors)
                    let power_color = if power_percentage >= 0.8 {
                        Color32::from_rgb(100, 255, 100) // Green: plenty of power
                    } else if power_percentage >= 0.5 {
                        Color32::from_rgb(255, 200, 100) // Orange: moderate
                    } else {
                        // Red with flashing for low power warning
                        let flash = (self.power_warning_flash.sin() * 0.3 + 0.7) as f32;
                        Color32::from_rgb((255.0 * flash) as u8, 50, 50)
                    };

                    let power_label = localization::localize("hud.power", "Power");
                    ui.colored_label(
                        power_color,
                        format!(
                            "{power_label}: {}/{}",
                            ui_state.power_generated, ui_state.max_power
                        ),
                    );

                    // Power bar visualization
                    let bar_width = 100.0;
                    let bar_height = 20.0;
                    let bar_rect = ui.allocate_space(egui::vec2(bar_width, bar_height)).1;

                    // Background
                    ui.painter()
                        .rect_filled(bar_rect, 2.0, Color32::from_gray(40));

                    // Filled portion
                    let fill_width = bar_width * power_percentage.clamp(0.0, 1.0);
                    let fill_rect =
                        Rect::from_min_size(bar_rect.min, egui::vec2(fill_width, bar_height));
                    ui.painter().rect_filled(fill_rect, 2.0, power_color);

                    // Border
                    ui.painter().rect_stroke(
                        bar_rect,
                        2.0,
                        Stroke::new(1.0, Color32::WHITE),
                        StrokeKind::Outside,
                    );

                    // Low power warning
                    if power_percentage < 0.3 {
                        let warning_text = localization::localize(
                            "hud.panel.low_power_warning",
                            "⚠ LOW POWER WARNING",
                        );
                        ui.colored_label(Color32::RED, warning_text);
                        self.low_power_warning_shown = true;
                    } else {
                        self.low_power_warning_shown = false;
                    }

                    ui.separator();
                    let assets_loaded = ui_state.assets_loaded.to_string();
                    let asset_mb = format!("{:.1}", ui_state.asset_memory_mb);
                    let assets_label = localization::localize_with_args(
                        "hud.egui.assets_status",
                        "Assets: {count} ({mb} MB)",
                        &[("count", assets_loaded.as_str()), ("mb", asset_mb.as_str())],
                    );
                    ui.label(assets_label);
                    ui.add(
                        ProgressBar::new(ui_state.asset_cache_usage.clamp(0.0, 1.0))
                            .text(localization::localize("hud.egui.asset_cache", "Cache"))
                            .desired_width(120.0),
                    );

                    ui.separator();

                    // Add utility command buttons
                    let sell_enabled = !ui_state.selected_units.is_empty();
                    let sell_label = localization::localize("hud.egui.button.sell", "Sell");
                    let sell_tooltip =
                        localization::localize("hud.egui.tooltip.sell", "Sell selected structure");
                    let raw_response = ui
                        .add_enabled(sell_enabled, Button::new(sell_label))
                        .on_hover_text(sell_tooltip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        if let Some(first_unit) = ui_state.selected_units.first() {
                            self.queue_ui_command(
                                CommandType::Sell {
                                    object_id: *first_unit,
                                },
                                ui_state.selected_units.clone(),
                            );
                        }
                    }

                    let repair_enabled = !ui_state.selected_units.is_empty();
                    let repair_label = localization::localize("hud.egui.button.repair", "Repair");
                    let repair_tooltip =
                        localization::localize("hud.egui.tooltip.repair", "Enter repair mode");
                    let raw_response = ui
                        .add_enabled(repair_enabled, Button::new(repair_label))
                        .on_hover_text(repair_tooltip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.pending_command = Some(PendingCommand::Repair);
                    }

                    // FPS counter (debug)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let fps_value = format!("{:.0}", ui_state.fps);
                        let fps_label = localization::localize_with_args(
                            "hud.egui.fps_counter",
                            "FPS: {fps}",
                            &[("fps", fps_value.as_str())],
                        );
                        ui.label(fps_label);
                    });
                });
            });
    }

    fn render_objectives_panel(&mut self, ctx: &Context, ui_state: &GameUIState) {
        if ui_state.objectives.is_empty() {
            return;
        }

        egui::TopBottomPanel::top("objectives_panel")
            .resizable(false)
            .min_height(30.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let header = localization::localize("objectives.panel.title", "Objectives");
                ui.label(RichText::new(header).strong());

                for category in [
                    ObjectiveCategory::Primary,
                    ObjectiveCategory::Secondary,
                    ObjectiveCategory::Bonus,
                ] {
                    let items: Vec<_> = ui_state
                        .objectives
                        .iter()
                        .filter(|obj| obj.category == category)
                        .collect();
                    if items.is_empty() {
                        continue;
                    }

                    let label = match category {
                        ObjectiveCategory::Primary => {
                            localization::localize("objectives.panel.primary", "Primary")
                        }
                        ObjectiveCategory::Secondary => {
                            localization::localize("objectives.panel.secondary", "Secondary")
                        }
                        ObjectiveCategory::Bonus => {
                            localization::localize("objectives.panel.bonus", "Bonus")
                        }
                    };
                    ui.label(RichText::new(label).italics());

                    for objective in items {
                        let color = match objective.status {
                            ObjectiveStatus::Active => Color32::from_rgb(200, 220, 255),
                            ObjectiveStatus::Completed => Color32::from_rgb(120, 200, 120),
                            ObjectiveStatus::Failed => Color32::from_rgb(220, 100, 100),
                        };

                        let mut line = format!("{}", objective.title);
                        if let Some((current, total)) = objective.progress {
                            line.push_str(&format!("  ({}/{})", current, total));
                        }
                        ui.colored_label(color, line);
                        ui.label(RichText::new(&objective.description).small());
                    }
                }
            });
    }

    /// Render selection info panel
    /// References C++ InGameUI::m_selectedDrawables display
    fn render_selection_panel(&mut self, ctx: &Context, ui_state: &GameUIState) {
        let selection_title =
            localization::localize("hud.egui.window.selection_info", "Selection Info");
        let screen_rect = ctx.content_rect();
        egui::Window::new(selection_title)
            .fixed_pos(Pos2::new(10.0, screen_rect.height() - 250.0))
            .fixed_size(Vec2::new(280.0, 230.0))
            .collapsible(false)
            .resizable(false)
            .title_bar(true)
            .show(ctx, |ui| {
                // Command buttons for selected units
                ui.horizontal(|ui| {
                    let has_units = !ui_state.selected_units.is_empty();

                    let move_label = localized_command("Move");
                    let move_tip = localization::localize(
                        "hud.egui.tooltip.move",
                        "Click to select move destination",
                    );
                    let raw_response = ui
                        .add_enabled(has_units, Button::new(move_label))
                        .on_hover_text(move_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.pending_command = Some(PendingCommand::Move);
                    }

                    let attack_label = localized_command("Attack");
                    let attack_tip = localization::localize(
                        "hud.egui.tooltip.attack",
                        "Click to select attack target",
                    );
                    let raw_response = ui
                        .add_enabled(has_units, Button::new(attack_label))
                        .on_hover_text(attack_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.pending_command = Some(PendingCommand::Attack);
                    }

                    let stop_label = localized_command("Stop");
                    let stop_tip =
                        localization::localize("hud.egui.tooltip.stop", "Stop all actions");
                    let raw_response = ui
                        .add_enabled(has_units, Button::new(stop_label))
                        .on_hover_text(stop_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.queue_ui_command(CommandType::Stop, ui_state.selected_units.clone());
                    }

                    let guard_label = localized_command("Guard");
                    let guard_tip =
                        localization::localize("hud.egui.tooltip.guard", "Guard position or unit");
                    let raw_response = ui
                        .add_enabled(has_units, Button::new(guard_label))
                        .on_hover_text(guard_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.pending_command = Some(PendingCommand::Guard);
                    }
                });

                ui.separator();

                // Show pending command feedback
                if let Some(ref pending) = self.pending_command {
                    let pending_text = match pending {
                        PendingCommand::Move => localization::localize(
                            "hud.egui.pending.move",
                            "Select move destination...",
                        ),
                        PendingCommand::Attack => localization::localize(
                            "hud.egui.pending.attack",
                            "Select attack target...",
                        ),
                        PendingCommand::Guard => localization::localize(
                            "hud.egui.pending.guard",
                            "Select guard target...",
                        ),
                        PendingCommand::ForceAttack => localization::localize(
                            "hud.egui.pending.force_attack",
                            "Select force attack target...",
                        ),
                        PendingCommand::Repair => localization::localize(
                            "hud.egui.pending.repair",
                            "Select repair target...",
                        ),
                        PendingCommand::SpecialPower(_) => localization::localize(
                            "hud.egui.pending.special_power",
                            "Select power target...",
                        ),
                    };
                    ui.colored_label(Color32::YELLOW, pending_text);
                }

                let selected_count = ui_state.selected_units.len().to_string();
                let selected_label = localization::localize_with_args(
                    "hud.egui.selected_units",
                    "Selected: {count} units",
                    &[("count", selected_count.as_str())],
                );
                ui.label(selected_label);
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for unit_info in &ui_state.selected_unit_infos {
                            // Make unit clickable for selection
                            let is_selected = self.selected_unit_view == Some(unit_info.object_id);
                            let response = ui.group(|ui| {
                                // Highlight if selected
                                if is_selected {
                                    ui.visuals_mut().widgets.noninteractive.bg_fill =
                                        Color32::from_gray(60);
                                }

                                let display_name = localized_entry(&unit_info.name);
                                ui.label(display_name);

                                // Health bar
                                let health_pct = if unit_info.health_maximum > 0.0 {
                                    unit_info.health_current / unit_info.health_maximum
                                } else {
                                    0.0
                                };

                                let health_color = if health_pct > 0.7 {
                                    Color32::GREEN
                                } else if health_pct > 0.3 {
                                    Color32::YELLOW
                                } else {
                                    Color32::RED
                                };

                                let bar_rect = ui.allocate_space(egui::vec2(200.0, 15.0)).1;

                                // Background
                                ui.painter()
                                    .rect_filled(bar_rect, 2.0, Color32::from_gray(60));

                                // Health fill
                                let fill_width = 200.0 * health_pct;
                                let fill_rect =
                                    Rect::from_min_size(bar_rect.min, egui::vec2(fill_width, 15.0));
                                ui.painter().rect_filled(fill_rect, 2.0, health_color);

                                let hp_current = format!("{:.0}", unit_info.health_current);
                                let hp_max = format!("{:.0}", unit_info.health_maximum);
                                let hp_label = localization::localize_with_args(
                                    "hud.egui.unit_hp",
                                    "HP: {current}/{max}",
                                    &[("current", hp_current.as_str()), ("max", hp_max.as_str())],
                                );
                                ui.label(hp_label);

                                if !unit_info.current_order.is_empty() {
                                    let order_label = localization::localize_with_args(
                                        "hud.egui.unit_order",
                                        "Order: {order}",
                                        &[("order", unit_info.current_order.as_str())],
                                    );
                                    ui.label(order_label);
                                }
                            });

                            let response = self.elastic_response(ui, response.response);
                            // Handle click on unit
                            if response.clicked() {
                                self.selected_unit_view = Some(unit_info.object_id);
                                // Queue selection command
                                self.queue_ui_command(
                                    CommandType::CreateSelectedGroup {
                                        create_new: true,
                                        units: vec![unit_info.object_id],
                                    },
                                    vec![unit_info.object_id],
                                );
                            }
                        }
                    });
            });
    }

    /// Render build queue panel
    /// References C++ InGameUI::m_buildProgress (BuildProgress struct line 156)
    fn render_build_queue_panel(&mut self, ctx: &Context, ui_state: &GameUIState) {
        let build_queue_title =
            localization::localize("hud.egui.window.build_queue", "Build Queue");
        let screen_rect = ctx.content_rect();
        egui::Window::new(build_queue_title)
            .fixed_pos(Pos2::new(
                screen_rect.width() / 2.0 - 175.0,
                screen_rect.height() - 280.0,
            ))
            .fixed_size(Vec2::new(350.0, 260.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                // Build buttons for units
                ui.label(localization::localize(
                    "hud.egui.build_units_label",
                    "Build Units:",
                ));
                ui.horizontal(|ui| {
                    let infantry_label = localized_entry("Infantry");
                    let infantry_tip = localization::localize_with_args(
                        "hud.egui.build.tooltip.infantry",
                        "Build Infantry unit ($100)",
                        &[("cost", "100")],
                    );
                    let raw_response = ui.button(infantry_label).on_hover_text(infantry_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.queue_ui_command(
                            CommandType::QueueUnitCreate {
                                template_name: "Infantry".to_string(),
                                quantity: 1,
                            },
                            ui_state.selected_units.clone(),
                        );
                    }

                    let tank_label = localized_entry("Tank");
                    let tank_tip = localization::localize_with_args(
                        "hud.egui.build.tooltip.tank",
                        "Build Tank unit ($500)",
                        &[("cost", "500")],
                    );
                    let raw_response = ui.button(tank_label).on_hover_text(tank_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.queue_ui_command(
                            CommandType::QueueUnitCreate {
                                template_name: "Tank".to_string(),
                                quantity: 1,
                            },
                            ui_state.selected_units.clone(),
                        );
                    }

                    let ranger_label = localized_entry("Ranger");
                    let ranger_tip = localization::localize_with_args(
                        "hud.egui.build.tooltip.ranger",
                        "Build Ranger unit ($200)",
                        &[("cost", "200")],
                    );
                    let raw_response = ui.button(ranger_label).on_hover_text(ranger_tip);
                    let response = self.elastic_response(ui, raw_response);
                    if response.clicked() {
                        self.queue_ui_command(
                            CommandType::QueueUnitCreate {
                                template_name: "Ranger".to_string(),
                                quantity: 1,
                            },
                            ui_state.selected_units.clone(),
                        );
                    }
                });

                ui.separator();

                // Current build queue display
                let queue_count = ui_state.build_queue.len().to_string();
                let building_label = localization::localize_with_args(
                    "hud.egui.building_count",
                    "Building: {count} items",
                    &[("count", queue_count.as_str())],
                );
                ui.label(building_label);

                egui::ScrollArea::vertical()
                    .max_height(130.0)
                    .show(ui, |ui| {
                        for (i, entry) in ui_state.build_queue.iter().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    if i == 0 {
                                        ui.label("⚡"); // Currently building
                                    } else {
                                        ui.label("⏳"); // Queued
                                    }
                                    let entry_label = localized_entry(&entry.template_name);
                                    ui.label(entry_label);

                                    // Cancel button for each queue item
                                    let raw_response =
                                        ui.small_button("✖").on_hover_text(localization::localize(
                                            "hud.egui.tooltip.cancel_construction",
                                            "Cancel construction",
                                        ));
                                    let response = self.elastic_response(ui, raw_response);
                                    if response.clicked() {
                                        self.queue_ui_command(
                                            CommandType::CancelUnitCreate {
                                                template_name: entry.template_name.clone(),
                                            },
                                            ui_state.selected_units.clone(),
                                        );
                                    }
                                });

                                // Progress bar
                                let progress_bar = egui::ProgressBar::new(entry.percent_complete)
                                    .text(format!("{:.0}%", entry.percent_complete * 100.0))
                                    .fill(Color32::from_rgb(100, 200, 255));
                                ui.add(progress_bar);

                                if entry.time_remaining > 0.0 {
                                    let minutes = (entry.time_remaining / 60.0) as i32;
                                    let seconds = (entry.time_remaining % 60.0) as i32;
                                    let minutes_str = format!("{minutes:02}");
                                    let seconds_str = format!("{seconds:02}");
                                    let time_label = localization::localize_with_args(
                                        "hud.egui.queue_time",
                                        "Time: {minutes}:{seconds}",
                                        &[
                                            ("minutes", minutes_str.as_str()),
                                            ("seconds", seconds_str.as_str()),
                                        ],
                                    );
                                    ui.label(time_label);
                                }
                            });
                        }
                    });
            });
    }

    /// Render minimap
    /// References C++ Radar class (Radar.h line 155) and W3DRadar
    fn render_minimap(&mut self, ctx: &Context, ui_state: &GameUIState) {
        if !ui_state.radar_enabled {
            return;
        }
        let screen_rect = ctx.content_rect();
        let minimap_pos = Pos2::new(
            screen_rect.width() - self.minimap_size - 10.0,
            screen_rect.height() - self.minimap_size - 10.0,
        );

        let minimap_title = localization::localize("hud.egui.window.minimap", "Minimap");
        egui::Window::new(minimap_title)
            .fixed_pos(minimap_pos)
            .fixed_size(Vec2::splat(self.minimap_size))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .show(ctx, |ui| {
                // Interactive minimap canvas
                let available = Vec2::splat(self.minimap_size - 10.0);
                let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
                let minimap_rect = response.rect;
                self.last_minimap_rect = Some(minimap_rect);

                if let Some(texture_id) = ui_state.minimap_texture_id {
                    let image = Image::new((texture_id, minimap_rect.size()));
                    image.paint_at(ui, minimap_rect);
                } else {
                    // Dark background representing unexplored terrain
                    painter.rect_filled(minimap_rect, 2.0, Color32::from_rgb(20, 30, 20));
                }

                // Draw unit dots
                for dot in &ui_state.minimap_unit_dots {
                    let pixel_pos = Pos2::new(
                        minimap_rect.min.x + dot.position.x * minimap_rect.width(),
                        minimap_rect.min.y + dot.position.y * minimap_rect.height(),
                    );

                    painter.circle_filled(pixel_pos, dot.size, dot.color);
                }

                // Draw beacon markers
                for beacon in &ui_state.minimap_beacons {
                    let pixel_pos = Pos2::new(
                        minimap_rect.min.x + beacon.position.x * minimap_rect.width(),
                        minimap_rect.min.y + beacon.position.y * minimap_rect.height(),
                    );
                    painter.circle_filled(pixel_pos, beacon.size, beacon.color);
                    painter.circle_stroke(
                        pixel_pos,
                        beacon.size + 2.0,
                        Stroke::new(1.5, Color32::WHITE),
                    );
                }

                // Draw radar pings (world-space -> minimap)
                let world_to_minimap = |world: Vec3| -> Option<Pos2> {
                    if let Some(coords) = &ui_state.minimap_coordinates {
                        let world_extent_x = (coords.world_max.x - coords.world_min.x).abs();
                        let world_extent_z = (coords.world_max.z - coords.world_min.z).abs();
                        if world_extent_x > 0.0 && world_extent_z > 0.0 {
                            let x_ratio = (world.x - coords.world_min.x) / world_extent_x;
                            let z_ratio = (world.z - coords.world_min.z) / world_extent_z;
                            return Some(Pos2::new(
                                minimap_rect.min.x + x_ratio * minimap_rect.width(),
                                minimap_rect.min.y + z_ratio * minimap_rect.height(),
                            ));
                        }
                    }

                    // Fallback: assume normalized positions
                    if minimap_rect.width() > 0.0 && minimap_rect.height() > 0.0 {
                        Some(Pos2::new(
                            minimap_rect.min.x + world.x * minimap_rect.width(),
                            minimap_rect.min.y + world.z * minimap_rect.height(),
                        ))
                    } else {
                        None
                    }
                };

                // Record fresh beacon placements so we can render a brief bloom ring.
                for pos in &ui_state.new_beacons {
                    self.beacon_highlights.push(RadarHighlight {
                        position: *pos,
                        timer: 1.0,
                    });
                }

                for ping in &ui_state.radar_pings {
                    if let Some(pixel_pos) = world_to_minimap(ping.position) {
                        let alpha = (ping.intensity.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let (fill, stroke) = match ping.kind {
                            RadarPingKind::Attack => (
                                Color32::from_rgba_unmultiplied(255, 80, 80, alpha),
                                Color32::from_rgba_unmultiplied(255, 140, 140, alpha),
                            ),
                            RadarPingKind::Ally => (
                                Color32::from_rgba_unmultiplied(80, 180, 255, alpha),
                                Color32::from_rgba_unmultiplied(140, 220, 255, alpha),
                            ),
                            RadarPingKind::Generic => (
                                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                                Color32::from_rgba_unmultiplied(220, 220, 220, alpha),
                            ),
                        };
                        let radius = 3.0 + 3.0 * ping.intensity;
                        painter.circle_filled(pixel_pos, radius, fill);
                        painter.circle_stroke(pixel_pos, radius + 2.0, Stroke::new(1.0, stroke));

                        // Optional highlight ring for user-focused ping.
                        if let Some(highlight) = &self.radar_highlight {
                            if highlight.position == ping.position {
                                let hl_alpha =
                                    ((highlight.timer / 1.0) * 180.0).clamp(0.0, 180.0) as u8;
                                painter.circle_stroke(
                                    pixel_pos,
                                    radius + 8.0,
                                    Stroke::new(
                                        2.0,
                                        Color32::from_rgba_unmultiplied(
                                            stroke.r(),
                                            stroke.g(),
                                            stroke.b(),
                                            hl_alpha,
                                        ),
                                    ),
                                );
                            }
                        }
                    }
                }

                // Draw beacon bloom overlays for newly placed beacons.
                for highlight in &self.beacon_highlights {
                    if let Some(pixel_pos) = world_to_minimap(highlight.position) {
                        let alpha = ((highlight.timer / 1.0) * 200.0).clamp(0.0, 200.0) as u8;
                        painter.circle_stroke(
                            pixel_pos,
                            10.0,
                            Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 160, alpha)),
                        );
                    }
                }

                // Draw viewport rectangle
                let viewport_rect = Rect::from_min_size(
                    Pos2::new(
                        minimap_rect.min.x + ui_state.minimap_viewport.min.x * minimap_rect.width(),
                        minimap_rect.min.y
                            + ui_state.minimap_viewport.min.y * minimap_rect.height(),
                    ),
                    Vec2::new(
                        ui_state.minimap_viewport.width() * minimap_rect.width(),
                        ui_state.minimap_viewport.height() * minimap_rect.height(),
                    ),
                );

                painter.rect_stroke(
                    viewport_rect,
                    0.0,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Outside,
                );

                // Border
                painter.rect_stroke(
                    minimap_rect,
                    2.0,
                    Stroke::new(2.0, Color32::from_gray(150)),
                    StrokeKind::Outside,
                );

                // Handle user interaction
                let response = self.elastic_response(ui, response);
                self.handle_minimap_input(&response);

                // If user clicked near a radar ping, issue camera focus to last radar event.
                if response.clicked() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        // Find nearest radar ping in screen-space.
                        let mut closest: Option<(f32, Vec3)> = None;
                        for ping in &ui_state.radar_pings {
                            if let Some(pixel_pos) = world_to_minimap(ping.position) {
                                let dist2 = (pixel_pos.x - pointer.x).powi(2)
                                    + (pixel_pos.y - pointer.y).powi(2);
                                if dist2 < 64.0 {
                                    if let Some((best, _)) = closest {
                                        if dist2 < best {
                                            closest = Some((dist2, ping.position));
                                        }
                                    } else {
                                        closest = Some((dist2, ping.position));
                                    }
                                }
                            }
                        }

                        if let Some((_, pos)) = closest {
                            self.radar_highlight = Some(RadarHighlight {
                                position: pos,
                                timer: 1.0,
                            });
                            let cmd = self.create_ui_command(
                                CommandType::ViewRadarAt { position: pos },
                                vec![],
                            );
                            self.command_queue.push_back(cmd);
                        }
                    }
                }
            });
    }

    /// Record minimap interactions for the engine to consume.
    fn handle_minimap_input(&mut self, response: &egui::Response) {
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.pending_minimap_action.replace(MinimapInteraction {
                    screen_position: pos,
                    kind: MinimapActionKind::LeftDrag,
                });
            }
        } else if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.pending_minimap_action.replace(MinimapInteraction {
                    screen_position: pos,
                    kind: MinimapActionKind::LeftClick,
                });
            }
        }

        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.pending_minimap_action.replace(MinimapInteraction {
                    screen_position: pos,
                    kind: MinimapActionKind::RightClick,
                });
            }
        }
    }

    /// Latest minimap rectangle on screen.
    pub fn minimap_rect(&self) -> Option<Rect> {
        self.last_minimap_rect
    }

    /// Pull the pending minimap interaction for this frame.
    pub fn take_minimap_interaction(&mut self) -> Option<MinimapInteraction> {
        self.pending_minimap_action.take()
    }

    /// Render game time display
    fn render_game_time(&self, ctx: &Context, ui_state: &GameUIState) {
        let screen_rect = ctx.content_rect();

        let game_time_title = localization::localize("hud.egui.window.game_time", "Game Time");
        egui::Window::new(game_time_title)
            .fixed_pos(Pos2::new(screen_rect.width() - 150.0, 10.0))
            .fixed_size(Vec2::new(140.0, 60.0))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .show(ctx, |ui| {
                let minutes = (ui_state.current_game_time / 60.0) as i32;
                let seconds = (ui_state.current_game_time % 60.0) as i32;

                ui.vertical_centered(|ui| {
                    ui.heading(format!("{:02}:{:02}", minutes, seconds));

                    if ui_state.is_game_paused {
                        let paused_label =
                            localization::localize("hud.egui.status.paused", "PAUSED");
                        ui.colored_label(Color32::YELLOW, paused_label);
                    }
                    let fps_value = format!("{:.0}", ui_state.fps);
                    let frame_ms = format!("{:.2}", ui_state.frame_time_ms);
                    let fps_label = localization::localize_with_args(
                        "hud.egui.frame_stats",
                        "{fps} FPS ({ms} ms)",
                        &[("fps", fps_value.as_str()), ("ms", frame_ms.as_str())],
                    );
                    ui.label(fps_label);
                    let perf_ratio = (ui_state.performance_score / 1.0).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(perf_ratio)
                            .text(localization::localize(
                                "hud.egui.performance_label",
                                "Performance",
                            ))
                            .desired_width(100.0),
                    );
                });
            });
    }

    fn render_debug_overlay(&self, ctx: &Context, ui_state: &GameUIState) {
        let diagnostics_title =
            localization::localize("hud.egui.window.diagnostics", "Diagnostics");
        egui::Window::new(diagnostics_title)
            .resizable(true)
            .default_width(260.0)
            .default_pos(egui::pos2(20.0, 80.0))
            .show(ctx, |ui| {
                let minutes = (ui_state.current_game_time / 60.0) as i32;
                let seconds = (ui_state.current_game_time % 60.0) as i32;
                let minutes_str = format!("{minutes:02}");
                let seconds_str = format!("{seconds:02}");
                let time_label = localization::localize_with_args(
                    "hud.egui.debug.time",
                    "Time: {minutes}:{seconds}",
                    &[
                        ("minutes", minutes_str.as_str()),
                        ("seconds", seconds_str.as_str()),
                    ],
                );
                ui.label(time_label);
                ui.separator();
                let fps_debug = format!("{:.1}", ui_state.fps);
                ui.label(localization::localize_with_args(
                    "hud.egui.debug.fps",
                    "FPS: {fps}",
                    &[("fps", fps_debug.as_str())],
                ));
                let frame_ms = format!("{:.2}", ui_state.frame_time_ms);
                ui.label(localization::localize_with_args(
                    "hud.egui.debug.frame_time",
                    "Frame time: {ms} ms",
                    &[("ms", frame_ms.as_str())],
                ));
                ui.add(
                    ProgressBar::new((ui_state.performance_score / 1.0).clamp(0.0, 1.5))
                        .desired_width(200.0)
                        .text(localization::localize(
                            "hud.egui.performance_label",
                            "Performance",
                        )),
                );
                ui.separator();
                let credits_value = ui_state.credits.to_string();
                ui.label(localization::localize_with_args(
                    "hud.egui.debug.credits",
                    "Credits: ${credits}",
                    &[("credits", credits_value.as_str())],
                ));
                let power_generated = ui_state.power_generated.to_string();
                let power_max = ui_state.max_power.to_string();
                ui.label(localization::localize_with_args(
                    "hud.egui.debug.power",
                    "Power: {generated} / {max}",
                    &[
                        ("generated", power_generated.as_str()),
                        ("max", power_max.as_str()),
                    ],
                ));
                ui.separator();
                let assets_loaded = ui_state.assets_loaded.to_string();
                let asset_mb = format!("{:.1}", ui_state.asset_memory_mb);
                ui.label(localization::localize_with_args(
                    "hud.egui.assets_status",
                    "Assets: {count} ({mb} MB)",
                    &[("count", assets_loaded.as_str()), ("mb", asset_mb.as_str())],
                ));
                ui.add(
                    ProgressBar::new(ui_state.asset_cache_usage.clamp(0.0, 1.0))
                        .desired_width(200.0)
                        .text(format!("{:.0}%", ui_state.asset_cache_usage * 100.0)),
                );
                if let Some(diag) = &ui_state.diagnostics {
                    ui.separator();
                    ui.heading(localization::localize(
                        "hud.egui.diagnostics.heading",
                        "Subsystem health",
                    ));
                    let diag_entries = [
                        ("hud.egui.diagnostics.overall", "Overall", diag.health_score),
                        ("hud.egui.diagnostics.engine", "Engine", diag.engine),
                        ("hud.egui.diagnostics.graphics", "Graphics", diag.graphics),
                        ("hud.egui.diagnostics.audio", "Audio", diag.audio),
                        ("hud.egui.diagnostics.network", "Network", diag.network),
                        ("hud.egui.diagnostics.logic", "Logic", diag.logic),
                    ];
                    for (key, fallback, value) in diag_entries {
                        let label = localization::localize(key, fallback);
                        self.render_diag_bar(ui, &label, value);
                    }
                    ui.separator();
                    let warnings = diag.warnings.to_string();
                    let errors = diag.errors.to_string();
                    let critical = diag.critical_errors.to_string();
                    ui.label(localization::localize_with_args(
                        "hud.egui.diagnostics.counters",
                        "Warnings: {warnings}  Errors: {errors}  Critical: {critical}",
                        &[
                            ("warnings", warnings.as_str()),
                            ("errors", errors.as_str()),
                            ("critical", critical.as_str()),
                        ],
                    ));
                }
            });
    }

    fn render_victory_overlay(
        &mut self,
        ctx: &Context,
        summary: &VictorySummary,
        outcome: Option<PlayerOutcome>,
    ) {
        let (title_key, fallback) = match outcome {
            Some(PlayerOutcome::Won) => ("victory.title", "VICTORY"),
            Some(PlayerOutcome::Lost) => ("victory.defeat_title", "DEFEAT"),
            _ => ("victory.draw", "DRAW"),
        };
        let title = localization::localize(title_key, fallback);

        Area::new(Id::new("victory_overlay"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .order(Order::Foreground)
            .show(ctx, |area_ui| {
                egui::Frame::popup(&ctx.style())
                    .corner_radius(6.0)
                    .inner_margin(egui::vec2(16.0, 16.0))
                    .show(area_ui, |ui| {
                        ui.vertical(|ui| {
                            ui.heading(title);
                            if let Some(mission) = &summary.mission_name {
                                ui.label(format!(
                                    "{} {}",
                                    localization::localize("victory.summary.mission", "Mission:"),
                                    mission
                                ));
                            }
                            if let Some(duration) = summary.duration {
                                ui.label(format!(
                                    "{} {}",
                                    localization::localize("victory.summary.duration", "Duration:"),
                                    format_duration(duration)
                                ));
                            }

                            ui.separator();
                            self.render_outcome_grid(ui, summary);

                            let unit_columns: [(&str, &str, PlayerStatAccessor); 3] = [
                                (
                                    "victory.summary.column.units_built",
                                    "Units Built",
                                    |r: &PlayerResult| r.units_built,
                                ),
                                (
                                    "victory.summary.column.units_destroyed",
                                    "Units Destroyed",
                                    |r: &PlayerResult| r.units_destroyed,
                                ),
                                (
                                    "victory.summary.column.units_lost",
                                    "Units Lost",
                                    |r: &PlayerResult| r.units_lost,
                                ),
                            ];
                            let structure_columns: [(&str, &str, PlayerStatAccessor); 3] = [
                                (
                                    "victory.summary.column.structures_built",
                                    "Structures Built",
                                    |r: &PlayerResult| r.structures_built,
                                ),
                                (
                                    "victory.summary.column.structures_destroyed",
                                    "Structures Destroyed",
                                    |r: &PlayerResult| r.structures_destroyed,
                                ),
                                (
                                    "victory.summary.column.structures_lost",
                                    "Structures Lost",
                                    |r: &PlayerResult| r.structures_lost,
                                ),
                            ];
                            let resource_columns: [(&str, &str, PlayerStatAccessor); 2] = [
                                (
                                    "victory.summary.column.resources_collected",
                                    "Collected",
                                    |r: &PlayerResult| r.resources_collected,
                                ),
                                (
                                    "victory.summary.column.resources_spent",
                                    "Spent",
                                    |r: &PlayerResult| r.resources_spent,
                                ),
                            ];

                            ui.separator();
                            self.render_stat_section(
                                ui,
                                summary,
                                "victory_units_grid",
                                "victory.summary.section.units",
                                "Units",
                                &unit_columns,
                            );
                            ui.separator();
                            self.render_stat_section(
                                ui,
                                summary,
                                "victory_structures_grid",
                                "victory.summary.section.structures",
                                "Structures",
                                &structure_columns,
                            );
                            ui.separator();
                            self.render_stat_section(
                                ui,
                                summary,
                                "victory_resources_grid",
                                "victory.summary.section.resources",
                                "Resources",
                                &resource_columns,
                            );

                            ui.separator();
                            ui.horizontal(|ui| {
                                let exit_label = localization::localize(
                                    "victory.summary.button.exit_menu",
                                    "Exit to Menu",
                                );
                                let raw_response = ui.button(exit_label);
                                let response = self.elastic_response(ui, raw_response);
                                if response.clicked() {
                                    self.victory_action = Some(VictoryOverlayAction::ExitToMenu);
                                }
                            });
                        });
                    });
            });
    }

    fn render_diag_bar(&self, ui: &mut egui::Ui, label: &str, percent: f32) {
        ui.add(
            ProgressBar::new((percent / 100.0).clamp(0.0, 1.0))
                .desired_width(200.0)
                .text(format!("{} {:.0}%", label, percent)),
        );
    }

    fn render_outcome_grid(&self, ui: &mut egui::Ui, summary: &VictorySummary) {
        ui.label(
            RichText::new(localization::localize(
                "victory.summary.section.outcome",
                "Battle Outcome",
            ))
            .strong(),
        );
        egui::Grid::new("victory_outcome_grid")
            .striped(true)
            .spacing(Vec2::new(12.0, 6.0))
            .show(ui, |grid| {
                grid.label(localization::localize(
                    "victory.summary.column.player",
                    "Player",
                ));
                grid.label(localization::localize(
                    "victory.summary.column.faction",
                    "Faction",
                ));
                grid.label(localization::localize(
                    "victory.summary.column.outcome",
                    "Outcome",
                ));
                grid.end_row();

                for result in &summary.player_results {
                    let name_text = if result.player_id == self.player_id {
                        RichText::new(&result.player_name).strong()
                    } else {
                        RichText::new(&result.player_name)
                    };
                    grid.label(name_text);
                    grid.label(localized_team_name(result.faction));
                    grid.label(overlay_outcome_label(result.outcome));
                    grid.end_row();
                }
            });
    }

    fn render_stat_section(
        &self,
        ui: &mut egui::Ui,
        summary: &VictorySummary,
        grid_id: &str,
        heading_key: &str,
        heading_fallback: &str,
        columns: &[(&str, &str, PlayerStatAccessor)],
    ) {
        ui.label(RichText::new(localization::localize(heading_key, heading_fallback)).strong());
        egui::Grid::new(grid_id)
            .striped(true)
            .spacing(Vec2::new(12.0, 6.0))
            .show(ui, |grid| {
                grid.label(localization::localize(
                    "victory.summary.column.player",
                    "Player",
                ));
                for (col_key, fallback, _) in columns {
                    grid.label(localization::localize(col_key, fallback));
                }
                grid.end_row();

                for result in &summary.player_results {
                    let name_text = if result.player_id == self.player_id {
                        RichText::new(&result.player_name).strong()
                    } else {
                        RichText::new(&result.player_name)
                    };
                    grid.label(name_text);
                    for (_, _, accessor) in columns {
                        grid.label(format!("{}", accessor(result)));
                    }
                    grid.end_row();
                }
            });
    }

    // === Command Generation Methods ===

    /// Queue a UI-generated command
    pub fn queue_ui_command(&mut self, command_type: CommandType, selected_units: Vec<ObjectId>) {
        let command = CmdGameCommand {
            command_type: command_type.clone(),
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units,
            modifier_keys: ModifierKeys::default(),
        };

        debug!("UI queuing command: {:?}", command_type);
        self.command_queue.push_back(command);
    }

    /// Create a command from UI action (button click)
    pub fn create_ui_command(
        &mut self,
        command_type: CommandType,
        selected_units: Vec<ObjectId>,
    ) -> CmdGameCommand {
        let command = CmdGameCommand {
            command_type,
            player_id: self.player_id,
            command_id: self.get_next_command_id(),
            timestamp: SystemTime::now(),
            selected_units,
            modifier_keys: ModifierKeys::default(),
        };
        self.command_queue.push_back(command.clone());
        command
    }

    /// Create build structure command from UI
    pub fn create_build_command(
        &mut self,
        template_name: String,
        location: glam::Vec3,
        selected_units: Vec<ObjectId>,
    ) -> CmdGameCommand {
        self.create_ui_command(
            CommandType::Build {
                template_name,
                location,
            },
            selected_units,
        )
    }

    /// Create unit production command from UI
    pub fn create_production_command(
        &mut self,
        template_name: String,
        quantity: u32,
        selected_units: Vec<ObjectId>,
    ) -> CmdGameCommand {
        self.create_ui_command(
            CommandType::QueueUnitCreate {
                template_name,
                quantity,
            },
            selected_units,
        )
    }

    /// Create cancel command from UI
    pub fn create_cancel_command(
        &mut self,
        template_name: String,
        selected_units: Vec<ObjectId>,
    ) -> CmdGameCommand {
        self.create_ui_command(
            CommandType::CancelUnitCreate { template_name },
            selected_units,
        )
    }

    /// Create sell command from UI
    pub fn create_sell_command(&mut self, object_id: ObjectId) -> CmdGameCommand {
        self.create_ui_command(CommandType::Sell { object_id }, vec![object_id])
    }

    /// Create special power command from UI
    pub fn create_special_power_command(
        &mut self,
        power_type: crate::command_system::SpecialPowerType,
        target: crate::command_system::PowerTarget,
        selected_units: Vec<ObjectId>,
    ) -> CmdGameCommand {
        self.create_ui_command(
            CommandType::DoSpecialPower { power_type, target },
            selected_units,
        )
    }

    /// Get next command ID
    fn get_next_command_id(&mut self) -> u32 {
        let id = self.next_command_id;
        self.next_command_id += 1;
        id
    }

    /// Get and clear command queue
    pub fn take_commands(&mut self) -> VecDeque<CmdGameCommand> {
        std::mem::take(&mut self.command_queue)
    }

    pub fn take_victory_action(&mut self) -> Option<VictoryOverlayAction> {
        self.victory_action.take()
    }

    /// Check if there are pending commands
    pub fn has_commands(&self) -> bool {
        !self.command_queue.is_empty()
    }

    /// Complete pending command with a target position
    pub fn complete_pending_command_with_position(
        &mut self,
        position: glam::Vec3,
        selected_units: Vec<ObjectId>,
    ) {
        if let Some(pending) = self.pending_command.take() {
            let command_type = match pending {
                PendingCommand::Move => CommandType::Move {
                    destination: position,
                },
                PendingCommand::Guard => CommandType::Guard {
                    target: GuardTarget::Position(position),
                },
                PendingCommand::ForceAttack => {
                    CommandType::ForceAttackGround { location: position }
                }
                _ => {
                    // Other commands need object targets
                    self.pending_command = Some(pending);
                    return;
                }
            };

            self.queue_ui_command(command_type, selected_units);
        }
    }

    /// Complete pending command with a target object
    pub fn complete_pending_command_with_object(
        &mut self,
        target_id: ObjectId,
        selected_units: Vec<ObjectId>,
    ) {
        if let Some(pending) = self.pending_command.take() {
            let command_type = match pending {
                PendingCommand::Attack => CommandType::Attack { target_id },
                PendingCommand::Guard => CommandType::Guard {
                    target: GuardTarget::Object(target_id),
                },
                PendingCommand::ForceAttack => CommandType::ForceAttackObject { target_id },
                PendingCommand::Repair => CommandType::Repair { target_id },
                PendingCommand::SpecialPower(power_type) => CommandType::DoSpecialPower {
                    power_type,
                    target: PowerTarget::Object(target_id),
                },
                PendingCommand::Move => {
                    // Move command doesn't target objects, keep pending
                    self.pending_command = Some(pending);
                    return;
                }
            };

            self.queue_ui_command(command_type, selected_units);
        }
    }

    /// Cancel pending command
    pub fn cancel_pending_command(&mut self) {
        self.pending_command = None;
    }

    /// Check if there's a pending command waiting for target
    pub fn has_pending_command(&self) -> bool {
        self.pending_command.is_some()
    }

    /// Get cursor feedback text for pending command
    pub fn get_pending_command_cursor_text(&self) -> Option<String> {
        self.pending_command.as_ref().map(|pending| match pending {
            PendingCommand::Move => {
                localization::localize("hud.egui.pending.cursor.move", "Select Move Destination")
            }
            PendingCommand::Attack => {
                localization::localize("hud.egui.pending.cursor.attack", "Select Attack Target")
            }
            PendingCommand::Guard => {
                localization::localize("hud.egui.pending.cursor.guard", "Select Guard Target")
            }
            PendingCommand::ForceAttack => localization::localize(
                "hud.egui.pending.cursor.force_attack",
                "Select Force Attack Target",
            ),
            PendingCommand::Repair => {
                localization::localize("hud.egui.pending.cursor.repair", "Select Repair Target")
            }
            PendingCommand::SpecialPower(power) => {
                let power_name = format!("{:?}", power);
                localization::localize_with_args(
                    "hud.egui.pending.cursor.special_power",
                    "Select {power} Target",
                    &[("power", power_name.as_str())],
                )
            }
        })
    }
}

fn overlay_outcome_label(outcome: PlayerOutcome) -> String {
    match outcome {
        PlayerOutcome::Won => localization::localize("victory.outcome.won", "Won"),
        PlayerOutcome::Lost => localization::localize("victory.outcome.lost", "Lost"),
        PlayerOutcome::Draw => localization::localize("victory.outcome.draw", "Draw"),
    }
}

fn localized_team_name(team: Team) -> String {
    let key = match team {
        Team::USA => "faction.usa.name",
        Team::China => "faction.china.name",
        Team::GLA => "faction.gla.name",
        Team::Neutral => "faction.neutral.name",
    };
    localization::localize(key, team.get_name())
}

impl Default for EguiHUD {
    fn default() -> Self {
        Self::new()
    }
}
