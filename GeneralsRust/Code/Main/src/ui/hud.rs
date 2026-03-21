//! In-Game HUD (Heads-Up Display)
//!
//! This module implements the in-game user interface including resource display,
//! mini-map, unit selection panel, building construction interface, and all
//! RTS interface elements that appear during gameplay.

use super::{
    color_for_player, layout, utils, BeaconDot, Interactive, KeyCode, MinimapUIState, MouseButton,
    Renderable, UIEvent, UIRenderContext,
};
use crate::game_logic::ObjectId;
use crate::localization;
use crate::ui::RadarPingKind;
use glam::Vec3;
use std::time::Duration;

/// Resource types in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Credits,
    Power,
}

/// Current resource amounts
#[derive(Debug, Clone)]
pub struct ResourceState {
    pub credits: i32,
    pub power: i32,
    pub max_power: i32,
    pub credits_per_second: f32,
    pub power_consumption: i32,
}

impl Default for ResourceState {
    fn default() -> Self {
        Self {
            credits: 10000,
            power: 60,
            max_power: 60,
            credits_per_second: 2.0,
            power_consumption: 15,
        }
    }
}

/// Mini-map component
pub struct MiniMap {
    position: (i32, i32),
    size: (u32, u32),
    visible: bool,
    map_data: Vec<u8>,                     // Simplified map representation
    unit_positions: Vec<(i32, i32, u8)>,   // x, y, team_color
    beacon_positions: Vec<(i32, i32, u8)>, // x, y, player/team color
    viewport_rect: (f32, f32, f32, f32),   // Normalized coordinates
    hovered: bool,
}

impl MiniMap {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            position: (x, y),
            size: (layout::MINIMAP_SIZE, layout::MINIMAP_SIZE),
            visible: true,
            map_data: vec![0; (layout::MINIMAP_SIZE * layout::MINIMAP_SIZE) as usize],
            unit_positions: Vec::new(),
            beacon_positions: Vec::new(),
            viewport_rect: (0.0, 0.0, 1.0, 1.0),
            hovered: false,
        }
    }

    pub fn update_units(&mut self, units: &[(ObjectId, f32, f32, u8)]) {
        self.unit_positions.clear();

        // Convert world coordinates to minimap coordinates
        for &(_, world_x, world_y, team) in units {
            let minimap_x = (world_x * self.size.0 as f32) as i32;
            let minimap_y = (world_y * self.size.1 as f32) as i32;

            if minimap_x >= 0
                && minimap_x < self.size.0 as i32
                && minimap_y >= 0
                && minimap_y < self.size.1 as i32
            {
                self.unit_positions.push((minimap_x, minimap_y, team));
            }
        }
    }

    pub fn update_beacons(&mut self, beacons: &[(f32, f32, u8)]) {
        self.beacon_positions.clear();
        for &(world_x, world_y, color_index) in beacons {
            let minimap_x = (world_x * self.size.0 as f32) as i32;
            let minimap_y = (world_y * self.size.1 as f32) as i32;
            if minimap_x >= 0
                && minimap_x < self.size.0 as i32
                && minimap_y >= 0
                && minimap_y < self.size.1 as i32
            {
                self.beacon_positions
                    .push((minimap_x, minimap_y, color_index));
            }
        }
    }

    pub fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.viewport_rect = (x, y, width, height);
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    pub fn world_coords_from_click(&self, x: i32, y: i32) -> Option<(f32, f32)> {
        if self.contains_point(x, y) {
            let rel_x = (x - self.position.0) as f32 / self.size.0 as f32;
            let rel_y = (y - self.position.1) as f32 / self.size.1 as f32;
            Some((rel_x, rel_y))
        } else {
            None
        }
    }
}

fn slugify_label(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    slug.trim_matches('_').to_string()
}

pub(crate) fn localized_entry(name: &str) -> String {
    let key = format!("hud.entry.{}", slugify_label(name));
    localization::localize(&key, name)
}

pub(crate) fn localized_command(name: &str) -> String {
    let key = format!("hud.command.{}", slugify_label(name));
    localization::localize(&key, name)
}

/// Resource display component
pub struct ResourceDisplay {
    position: (i32, i32),
    size: (u32, u32),
    resources: ResourceState,
    credits_animation_time: f32,
}

impl ResourceDisplay {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            position: (x, y),
            size: (layout::RESOURCE_PANEL_WIDTH, 60),
            resources: ResourceState::default(),
            credits_animation_time: 0.0,
        }
    }

    pub fn update_resources(&mut self, credits: i32, power: i32, max_power: i32) {
        if credits != self.resources.credits {
            self.credits_animation_time = 0.0;
        }

        self.resources.credits = credits;
        self.resources.power = power;
        self.resources.max_power = max_power;
    }

    pub fn update(&mut self, delta_time: f32) {
        self.credits_animation_time += delta_time;
    }

    pub fn get_power_percentage(&self) -> f32 {
        if self.resources.max_power > 0 {
            self.resources.power as f32 / self.resources.max_power as f32
        } else {
            0.0
        }
    }

    pub fn is_power_low(&self) -> bool {
        self.get_power_percentage() < 0.3
    }
}

/// Construction panel for building units and structures
pub struct ConstructionPanel {
    position: (i32, i32),
    size: (u32, u32),
    visible: bool,
    current_tab: ConstructionTab,
    building_queue: Vec<BuildQueueItem>,
    selected_building: Option<String>,
    tab_buttons: Vec<TabButton>,
    construction_buttons: Vec<ConstructionButton>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionTab {
    Buildings,
    Infantry,
    Vehicles,
    Aircraft,
    NavalUnits,
    SuperWeapons,
}

#[derive(Debug, Clone)]
pub struct BuildQueueItem {
    pub item_name: String,
    pub display_name: String,
    pub progress: f32,
    pub cost: i32,
    pub build_time: f32,
    pub remaining_time: f32,
}

#[derive(Debug, Clone)]
struct TabButton {
    tab: ConstructionTab,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    active: bool,
}

#[derive(Debug, Clone)]
struct ConstructionButton {
    item_name: String,
    display_name: String,
    position: (i32, i32),
    size: (u32, u32),
    cost: i32,
    enabled: bool,
    hovered: bool,
    build_key: Option<KeyCode>,
}

impl ConstructionPanel {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            position: (x, y),
            size: (600, layout::HUD_PANEL_HEIGHT),
            visible: false,
            current_tab: ConstructionTab::Buildings,
            building_queue: Vec::new(),
            selected_building: None,
            tab_buttons: Vec::new(),
            construction_buttons: Vec::new(),
        }
    }

    pub fn show_for_building(&mut self, building_name: &str) {
        self.visible = true;
        self.selected_building = Some(building_name.to_string());
        self.setup_construction_options(building_name);
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.selected_building = None;
        self.construction_buttons.clear();
    }

    fn setup_construction_options(&mut self, building_name: &str) {
        self.construction_buttons.clear();

        // Setup tabs based on building type
        match building_name {
            "Barracks" => {
                self.current_tab = ConstructionTab::Infantry;
                self.add_infantry_units();
            }
            "War Factory" => {
                self.current_tab = ConstructionTab::Vehicles;
                self.add_vehicle_units();
            }
            "Airfield" => {
                self.current_tab = ConstructionTab::Aircraft;
                self.add_aircraft_units();
            }
            _ => {
                self.current_tab = ConstructionTab::Buildings;
                self.add_building_structures();
            }
        }
    }

    fn add_infantry_units(&mut self) {
        let units = vec![
            ("Ranger", 225, KeyCode::R),
            ("Missile Defender", 300, KeyCode::M),
            ("Pathfinder", 600, KeyCode::P),
            ("Colonel Burton", 1500, KeyCode::B),
        ];

        self.add_construction_buttons(&units);
    }

    fn add_vehicle_units(&mut self) {
        let units = vec![
            ("Humvee", 700, KeyCode::H),
            ("Crusader Tank", 1400, KeyCode::C),
            ("Paladin Tank", 1800, KeyCode::P),
            ("Tomahawk Launcher", 1200, KeyCode::T),
        ];

        self.add_construction_buttons(&units);
    }

    fn add_aircraft_units(&mut self) {
        let units = vec![
            ("Comanche", 1200, KeyCode::C),
            ("Raptor", 1600, KeyCode::R),
            ("Stealth Fighter", 2500, KeyCode::S),
        ];

        self.add_construction_buttons(&units);
    }

    fn add_building_structures(&mut self) {
        let buildings = vec![
            ("Power Plant", 800, KeyCode::P),
            ("Barracks", 600, KeyCode::B),
            ("Supply Center", 2000, KeyCode::S),
            ("War Factory", 2000, KeyCode::W),
        ];

        self.add_construction_buttons(&buildings);
    }

    fn add_construction_buttons(&mut self, items: &[(&str, i32, KeyCode)]) {
        let buttons_per_row = 4;
        let button_size = 64u32;
        let spacing = 8u32;
        let start_x = self.position.0 + 10;
        let start_y = self.position.1 + 40;

        for (i, &(name, cost, key)) in items.iter().enumerate() {
            let row = i / buttons_per_row;
            let col = i % buttons_per_row;

            let x = start_x + col as i32 * (button_size + spacing) as i32;
            let y = start_y + row as i32 * (button_size + spacing) as i32;

            self.construction_buttons.push(ConstructionButton {
                item_name: name.to_string(),
                display_name: localized_entry(name),
                position: (x, y),
                size: (button_size, button_size),
                cost,
                enabled: true,
                hovered: false,
                build_key: Some(key),
            });
        }
    }

    pub fn add_to_queue(
        &mut self,
        item_name: &str,
        display_name: &str,
        cost: i32,
        build_time: f32,
    ) {
        self.building_queue.push(BuildQueueItem {
            item_name: item_name.to_string(),
            display_name: display_name.to_string(),
            progress: 0.0,
            cost,
            build_time,
            remaining_time: build_time,
        });
    }

    pub fn update_queue(&mut self, delta_time: f32) -> Vec<String> {
        let mut completed = Vec::new();

        if let Some(item) = self.building_queue.first_mut() {
            item.remaining_time -= delta_time;
            item.progress = 1.0 - (item.remaining_time / item.build_time);

            if item.remaining_time <= 0.0 {
                completed.push(item.display_name.clone());
                self.building_queue.remove(0);
            }
        }

        completed
    }
}

/// Main HUD implementation
pub struct GameHUD {
    /// Resource display
    resource_display: ResourceDisplay,
    /// Mini-map component
    minimap: MiniMap,
    /// Minimap UI state (FOW/camera-aware)
    minimap_panel: MinimapUIState,
    /// Construction panel
    construction_panel: ConstructionPanel,
    /// Selected units
    selected_units: Vec<ObjectId>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// HUD visibility
    visible: bool,
    /// Command panel for selected units
    command_buttons: Vec<CommandButton>,
    /// Message log
    messages: Vec<GameMessage>,
    /// Active beacon markers for minimap rendering
    beacon_markers: Vec<(f32, f32, u8)>,
    /// Recent beacon/radar notifications displayed on HUD
    beacon_events: Vec<GameMessage>,
    /// Current game time
    game_time: Duration,
    /// Last known low-power state, used to avoid repeating the same warning every frame.
    power_low_active: bool,
}

#[derive(Debug, Clone)]
struct CommandButton {
    command: String,
    position: (i32, i32),
    size: (u32, u32),
    icon: String,
    hotkey: Option<KeyCode>,
    enabled: bool,
    hovered: bool,
}

#[derive(Debug, Clone)]
struct GameMessage {
    text: String,
    spawn_time: f32,
    message_type: MessageType,
    position: Option<Vec3>,
    radar_kind: RadarPingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Info,
    Warning,
    Error,
    Combat,
    Construction,
    Radar,
    Script,
}

impl Default for GameHUD {
    fn default() -> Self {
        Self::new()
    }
}

impl GameHUD {
    /// Create new game HUD
    pub fn new() -> Self {
        let screen_size = (1024, 768);

        Self {
            resource_display: ResourceDisplay::new(10, 10),
            minimap: MiniMap::new(
                screen_size.0 as i32 - layout::MINIMAP_SIZE as i32 - 10,
                screen_size.1 as i32 - layout::MINIMAP_SIZE as i32 - 10,
            ),
            construction_panel: ConstructionPanel::new(
                10,
                screen_size.1 as i32 - layout::HUD_PANEL_HEIGHT as i32,
            ),
            minimap_panel: MinimapUIState::default(),
            selected_units: Vec::new(),
            screen_size,
            visible: true,
            command_buttons: Vec::new(),
            messages: Vec::new(),
            beacon_markers: Vec::new(),
            beacon_events: Vec::new(),
            game_time: Duration::from_secs(0),
            power_low_active: false,
        }
    }

    /// Initialize HUD
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let message = localization::localize(
            "hud.message.game_started",
            "Game started - Good luck, Commander!",
        );
        self.add_message(&message, MessageType::Info);
        Ok(())
    }

    /// Update HUD
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        let delta_time = if delta_time.is_finite() && delta_time > 0.0 {
            delta_time
        } else {
            0.0
        };

        self.resource_display.update(delta_time);

        // Update construction queue
        let completed_items = self.construction_panel.update_queue(delta_time);
        for item in completed_items {
            let message = localization::localize_with_args(
                "hud.message.build_queue_complete",
                "{name} construction complete",
                &[("name", item.as_str())],
            );
            self.add_message(&message, MessageType::Construction);
        }

        // Update game time
        self.game_time += Duration::from_secs_f32(delta_time);

        // Clean old messages (keep 30 seconds of history)
        let current_time = self.game_time.as_secs_f32();
        self.messages
            .retain(|msg| current_time - msg.spawn_time <= 30.0);

        Ok(())
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if !self.visible {
            return None;
        }

        // Check minimap clicks
        if let Some((world_x, world_y)) = self.minimap.world_coords_from_click(x, y) {
            if button == MouseButton::Left {
                return Some(UIEvent::FocusCamera(Vec3::new(world_x, 0.0, world_y)));
            }
        }

        // Check construction panel clicks
        if self.construction_panel.visible {
            let mut clicked_construction = None;
            for construction_button in &mut self.construction_panel.construction_buttons {
                if utils::point_in_rect(
                    (x, y),
                    (
                        construction_button.position.0,
                        construction_button.position.1,
                        construction_button.size.0,
                        construction_button.size.1,
                    ),
                ) && construction_button.enabled
                {
                    clicked_construction = Some((
                        construction_button.item_name.clone(),
                        construction_button.display_name.clone(),
                        construction_button.cost,
                    ));
                    break;
                }
            }

            if let Some((item_name, display_name, cost)) = clicked_construction {
                // Start construction
                self.construction_panel
                    .add_to_queue(&item_name, &display_name, cost, 5.0);
                let cost_str = cost.to_string();
                let message = localization::localize_with_args(
                    "hud.message.build_queue_start",
                    "Building {name} (${cost} Credits)",
                    &[("name", display_name.as_str()), ("cost", cost_str.as_str())],
                );
                self.add_message(&message, MessageType::Construction);
                return None;
            }
        }

        // Check command button clicks
        for cmd_button in &mut self.command_buttons {
            if utils::point_in_rect(
                (x, y),
                (
                    cmd_button.position.0,
                    cmd_button.position.1,
                    cmd_button.size.0,
                    cmd_button.size.1,
                ),
            ) && cmd_button.enabled
            {
                // Execute command
                let command_name = localized_command(&cmd_button.command);
                let log_msg = localization::localize_with_args(
                    "hud.log.command_executed",
                    "Command executed: {command}",
                    &[("command", command_name.as_str())],
                );
                println!("{log_msg}");
                return None;
            }
        }

        None
    }

    /// Select units
    pub fn select_units(&mut self, unit_ids: Vec<ObjectId>) {
        self.selected_units = unit_ids;
        self.update_command_buttons();

        if self.selected_units.len() == 1 {
            let message = localization::localize("hud.message.unit_selected", "Unit selected");
            self.add_message(&message, MessageType::Info);
        } else if self.selected_units.len() > 1 {
            let count = self.selected_units.len().to_string();
            let message = localization::localize_with_args(
                "hud.message.units_selected",
                "{count} units selected",
                &[("count", count.as_str())],
            );
            self.add_message(&message, MessageType::Info);
        }
    }

    /// Select building
    pub fn select_building(&mut self, building_name: &str) {
        self.construction_panel.show_for_building(building_name);
        let display_name = localized_entry(building_name);
        let suffix = localization::localize("hud.message.selected_suffix", "selected");
        self.add_message(&format!("{display_name} {suffix}"), MessageType::Info);
    }

    /// Update resources
    pub fn update_resources(&mut self, credits: i32, power: i32, max_power: i32) {
        self.resource_display
            .update_resources(credits, power, max_power);

        if self.construction_panel.visible {
            for button in &mut self.construction_panel.construction_buttons {
                button.enabled = credits >= button.cost;
            }
        }

        let power_low = self.resource_display.is_power_low();
        if power_low && !self.power_low_active {
            let message = localization::localize(
                "hud.message.power_low",
                "Power running low - Build more generators!",
            );
            self.add_message(&message, MessageType::Warning);
        }
        self.power_low_active = power_low;
    }

    /// Update minimap
    pub fn update_minimap(&mut self, units: &[(ObjectId, f32, f32, u8)]) {
        self.minimap.update_units(units);
    }

    /// Update minimap beacons (normalized world coords + color index)
    pub fn update_beacons(&mut self, beacons: &[(f32, f32, u8)]) {
        self.beacon_markers.clear();
        self.beacon_markers.extend_from_slice(beacons);
        self.minimap.update_beacons(beacons);

        // Forward to the minimap panel with mapped colors.
        let beacon_dots: Vec<BeaconDot> = beacons
            .iter()
            .map(|(x, z, color_index)| BeaconDot {
                world_pos: Vec3::new(*x, 0.0, *z),
                color: color_for_player(*color_index),
            })
            .collect();
        self.minimap_panel.update_beacons(beacon_dots);

        // Trigger a brief minimap bloom for newly-placed beacons.
        if let Some(last) = beacons.last() {
            self.minimap_panel
                .set_beacon_highlight(Vec3::new(last.0, 0.0, last.1));
        }
    }

    /// Update radar ping overlays on the minimap (world coordinates)
    pub fn update_radar_pings(
        &mut self,
        pings: &[crate::ui::RadarPing],
        world_min: Vec3,
        world_max: Vec3,
    ) {
        self.minimap_panel.world_min = world_min;
        self.minimap_panel.world_max = world_max;
        self.minimap_panel.radar_pings.clear();
        for ping in pings {
            self.minimap_panel
                .radar_pings
                .push(crate::ui::minimap_panel::RadarPing {
                    world_pos: ping.position,
                    intensity: ping.intensity,
                    age_seconds: ping.age_seconds,
                    kind: match ping.kind {
                        crate::ui::RadarPingKind::Generic => {
                            crate::ui::minimap_panel::RadarPingKind::Generic
                        }
                        crate::ui::RadarPingKind::Attack => {
                            crate::ui::minimap_panel::RadarPingKind::Attack
                        }
                        crate::ui::RadarPingKind::Ally => {
                            crate::ui::minimap_panel::RadarPingKind::Ally
                        }
                    },
                });
        }
        // Clear beacon highlight when new radar overlays are applied to avoid stale glow.
        self.minimap_panel.beacon_highlight = None;
    }

    /// Add message to log
    pub fn add_message(&mut self, text: &str, message_type: MessageType) {
        self.messages.push(GameMessage {
            text: text.to_string(),
            spawn_time: self.game_time.as_secs_f32(),
            message_type,
            position: None,
            radar_kind: RadarPingKind::Generic,
        });

        // Keep only last 10 messages visible
        if self.messages.len() > 10 {
            self.messages.remove(0);
        }
    }

    /// Convenience helper for generic info text
    pub fn push_info_message(&mut self, text: &str) {
        self.add_message(text, MessageType::Info);
    }

    pub fn push_radar_message(&mut self, text: &str) {
        self.add_radar_message(text, None, RadarPingKind::Generic);
    }

    pub fn push_beacon_event(&mut self, text: &str) {
        self.add_radar_message(text, None, RadarPingKind::Generic);
    }

    /// Push a radar/beacon event with optional world position and kind for display/focus.
    pub fn add_radar_message(&mut self, text: &str, position: Option<Vec3>, kind: RadarPingKind) {
        let entry = GameMessage {
            text: text.to_string(),
            spawn_time: self.game_time.as_secs_f32(),
            message_type: MessageType::Radar,
            position,
            radar_kind: kind,
        };
        self.messages.push(entry.clone());
        self.beacon_events.push(entry);

        if self.messages.len() > 10 {
            self.messages.remove(0);
        }
        if self.beacon_events.len() > 10 {
            self.beacon_events.remove(0);
        }
    }

    pub fn push_script_message(&mut self, text: &str) {
        let localized = localization::localize_with_args(
            "hud.script.message_logged",
            "{message}",
            &[("message", text)],
        );
        self.add_message(&localized, MessageType::Script);
    }

    /// Resize HUD
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);

        // Reposition components
        self.minimap.position = (
            width as i32 - layout::MINIMAP_SIZE as i32 - 10,
            height as i32 - layout::MINIMAP_SIZE as i32 - 10,
        );
        self.minimap_panel.set_screen_pos(
            self.minimap.position.0 as f32,
            self.minimap.position.1 as f32,
        );
        self.minimap_panel.width = layout::MINIMAP_SIZE as f32;
        self.minimap_panel.height = layout::MINIMAP_SIZE as f32;

        self.construction_panel.position = (10, height as i32 - layout::HUD_PANEL_HEIGHT as i32);
    }

    /// Toggle HUD visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    // Private methods

    fn update_command_buttons(&mut self) {
        self.command_buttons.clear();

        if self.selected_units.is_empty() {
            return;
        }

        // Add common unit commands
        let button_size = 48u32;
        let spacing = 4u32;
        let start_x = (self.screen_size.0 / 2) as i32 - 200;
        let start_y = self.screen_size.1 as i32 - 60;

        let commands = [
            ("Move", "move_icon", Some(KeyCode::M)),
            ("Attack", "attack_icon", Some(KeyCode::A)),
            ("Stop", "stop_icon", Some(KeyCode::S)),
            ("Guard", "guard_icon", Some(KeyCode::G)),
        ];

        for (i, (command, icon, hotkey)) in commands.iter().enumerate() {
            self.command_buttons.push(CommandButton {
                command: command.to_string(),
                position: (start_x + i as i32 * (button_size + spacing) as i32, start_y),
                size: (button_size, button_size),
                icon: icon.to_string(),
                hotkey: *hotkey,
                enabled: true,
                hovered: false,
            });
        }
    }

    fn format_game_time(&self) -> String {
        let total_seconds = self.game_time.as_secs();
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }
}

impl Interactive for GameHUD {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        if !self.visible {
            return false;
        }

        let mut handled = false;

        // Update minimap hover
        self.minimap.hovered = self.minimap.contains_point(x, y);
        if self.minimap.hovered {
            handled = true;
        }

        // Update construction panel button hovers
        if self.construction_panel.visible {
            for button in &mut self.construction_panel.construction_buttons {
                button.hovered = utils::point_in_rect(
                    (x, y),
                    (
                        button.position.0,
                        button.position.1,
                        button.size.0,
                        button.size.1,
                    ),
                );
                if button.hovered {
                    handled = true;
                }
            }
        }

        // Update command button hovers
        for button in &mut self.command_buttons {
            button.hovered = utils::point_in_rect(
                (x, y),
                (
                    button.position.0,
                    button.position.1,
                    button.size.0,
                    button.size.1,
                ),
            );
            if button.hovered {
                handled = true;
            }
        }

        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        self.handle_mouse_click(x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        // Check construction hotkeys
        if self.construction_panel.visible {
            let mut build_item = None;
            for button in &self.construction_panel.construction_buttons {
                if button.build_key == Some(key) && button.enabled {
                    build_item = Some((
                        button.item_name.clone(),
                        button.display_name.clone(),
                        button.cost,
                    ));
                    break;
                }
            }

            if let Some((item_name, display_name, cost)) = build_item {
                self.construction_panel
                    .add_to_queue(&item_name, &display_name, cost, 5.0);
                return true;
            }
        }

        // Check command hotkeys
        for button in &self.command_buttons {
            if button.hotkey == Some(key) && button.enabled {
                let command_name = localized_command(&button.command);
                let log_msg = localization::localize_with_args(
                    "hud.log.command_hotkey",
                    "Command hotkey pressed: {command}",
                    &[("command", command_name.as_str())],
                );
                println!("{log_msg}");
                return true;
            }
        }

        // Global HUD hotkeys
        match key {
            KeyCode::H => {
                self.toggle_visibility();
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for GameHUD {
    fn render(&self, _context: &mut UIRenderContext) {
        if !self.visible {
            return;
        }

        let hud_header = localization::localize("hud.panel.header", "=== Game HUD ===");
        println!("{hud_header}");

        // Render resource display
        let credits = self.resource_display.resources.credits.to_string();
        let power = self.resource_display.resources.power.to_string();
        let max_power = self.resource_display.resources.max_power.to_string();
        let resource_status = localization::localize_with_args(
            "hud.panel.resources_status",
            "Resources: ${credits} Credits, {power}/{max_power} Power",
            &[
                ("credits", credits.as_str()),
                ("power", power.as_str()),
                ("max_power", max_power.as_str()),
            ],
        );
        println!("{resource_status}");

        if self.resource_display.is_power_low() {
            println!(
                "  {}",
                localization::localize("hud.panel.low_power_warning", "⚠️  LOW POWER WARNING")
            );
        }

        // Render minimap
        let minimap_x = self.minimap.position.0.to_string();
        let minimap_y = self.minimap.position.1.to_string();
        let minimap_count = self.minimap.unit_positions.len().to_string();
        let minimap_text = localization::localize_with_args(
            "hud.panel.minimap_units",
            "Minimap at ({x}, {y}) - {count} units visible",
            &[
                ("x", minimap_x.as_str()),
                ("y", minimap_y.as_str()),
                ("count", minimap_count.as_str()),
            ],
        );
        println!("{minimap_text}");

        if self.minimap.hovered {
            println!(
                "  {}",
                localization::localize("hud.panel.minimap_hovered", "[Minimap hovered]")
            );
        }

        if !self.beacon_markers.is_empty() {
            let label = localization::localize("hud.panel.beacons", "Beacons active on minimap");
            println!("{}: {}", label, self.beacon_markers.len());
        }

        // Render construction panel if visible
        if self.construction_panel.visible {
            if let Some(building) = &self.construction_panel.selected_building {
                let panel_label =
                    localization::localize("hud.panel.construction", "Construction Panel");
                let building_display = localized_entry(building);
                println!("{panel_label} - {building_display}");

                let disabled_label =
                    localization::localize("hud.panel.button_state_disabled", "[DISABLED]");
                let hovered_label =
                    localization::localize("hud.panel.button_state_hovered", "[HOVERED]");

                for button in &self.construction_panel.construction_buttons {
                    let state = if !button.enabled {
                        disabled_label.as_str()
                    } else if button.hovered {
                        hovered_label.as_str()
                    } else {
                        ""
                    };

                    println!(
                        "  {} - ${} {} {:?}",
                        button.display_name, button.cost, state, button.build_key
                    );
                }

                // Render build queue
                if !self.construction_panel.building_queue.is_empty() {
                    let build_queue_label =
                        localization::localize("hud.panel.build_queue", "Build Queue");
                    let waiting_label = localization::localize("hud.panel.waiting", "Waiting");
                    println!("{build_queue_label}:");
                    for (i, item) in self.construction_panel.building_queue.iter().enumerate() {
                        if i == 0 {
                            println!(
                                "  ⚡ {} - {:.1}% complete",
                                item.display_name,
                                item.progress * 100.0
                            );
                        } else {
                            println!("  ⏳ {} - {}", item.display_name, waiting_label);
                        }
                    }
                }
            }
        }

        // Render selected unit commands
        if !self.selected_units.is_empty() {
            let count = self.selected_units.len().to_string();
            let command_label = localization::localize_with_args(
                "hud.panel.unit_commands_selected",
                "Unit Commands ({count} units selected):",
                &[("count", count.as_str())],
            );
            println!("{command_label}");
            let hovered_label =
                localization::localize("hud.panel.button_state_hovered", "[HOVERED]");
            for button in &self.command_buttons {
                let state = if button.hovered {
                    hovered_label.as_str()
                } else {
                    ""
                };
                let command_name = localized_command(&button.command);
                println!("  {} {:?} {}", command_name, button.hotkey, state);
            }
        }

        // Render recent messages
        if !self.messages.is_empty() {
            let messages_label = localization::localize("hud.panel.messages", "Messages");
            println!("{messages_label}:");
            let current_time = self.game_time.as_secs_f32();
            for msg in self.messages.iter().rev().take(5) {
                let age = current_time - msg.spawn_time;
                let type_str = match msg.message_type {
                    MessageType::Warning => "⚠️ ",
                    MessageType::Error => "❌ ",
                    MessageType::Combat => "⚔️  ",
                    MessageType::Construction => "🔨 ",
                    MessageType::Info => "ℹ️  ",
                    MessageType::Radar => "📡 ",
                    MessageType::Script => "🛰️ ",
                };
                println!("  {}{}  ({:.0}s ago)", type_str, msg.text, age.max(0.0));
            }
        }

        if !self.beacon_events.is_empty() {
            let events_label =
                localization::localize("hud.panel.beacon_events", "Beacon/Radar Events");
            println!("{events_label}:");
            for msg in self.beacon_events.iter().rev().take(3) {
                let icon = match msg.radar_kind {
                    RadarPingKind::Attack => "⚔",
                    RadarPingKind::Ally => "🛡",
                    RadarPingKind::Generic => "📡",
                };
                if let Some(pos) = msg.position {
                    println!("  {icon} {} @ ({:.0}, {:.0})", msg.text, pos.x, pos.z);
                } else {
                    println!("  {icon} {}", msg.text);
                }
            }
        }

        // Render game time
        let game_time_label = localization::localize("hud.panel.game_time", "Game Time");
        println!("{game_time_label}: {}", self.format_game_time());
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_state() {
        let resources = ResourceState::default();
        assert_eq!(resources.credits, 10000);
        assert!(resources.power > 0);
    }

    #[test]
    fn test_minimap_coordinates() {
        let minimap = MiniMap::new(100, 100);

        // Test click within minimap bounds
        let world_coords = minimap.world_coords_from_click(150, 150);
        assert!(world_coords.is_some());

        // Test click outside minimap bounds
        let world_coords = minimap.world_coords_from_click(50, 50);
        assert!(world_coords.is_none());
    }

    #[test]
    fn test_construction_queue() {
        let mut panel = ConstructionPanel::new(0, 0);
        assert!(panel.building_queue.is_empty());

        panel.add_to_queue("Barracks", "Barracks", 600, 10.0);
        assert_eq!(panel.building_queue.len(), 1);
        assert_eq!(panel.building_queue[0].item_name, "Barracks");
    }

    #[test]
    fn test_resource_display() {
        let mut display = ResourceDisplay::new(0, 0);
        display.update_resources(5000, 30, 60);

        assert_eq!(display.resources.credits, 5000);
        assert_eq!(display.get_power_percentage(), 0.5);
        assert!(!display.is_power_low());

        display.update_resources(5000, 15, 60);
        assert!(display.is_power_low());
    }
}
