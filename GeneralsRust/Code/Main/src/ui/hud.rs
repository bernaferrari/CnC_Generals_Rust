//! In-Game HUD (Heads-Up Display)
//!
//! This module implements the in-game user interface including resource display,
//! mini-map, unit selection panel, building construction interface, and all
//! RTS interface elements that appear during gameplay.

use super::{
    color_for_player, layout, utils, BeaconDot, ControlBarSelectionPanelState, Interactive,
    KeyCode, MinimapUIState, MouseButton, Renderable, UIEvent, UIRenderContext, UnitDisplayInfo,
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

/// Residual faction bucket for construction cameo lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstructionFaction {
    America,
    China,
    Gla,
}

/// Infer construction cameo faction from selected producer template residual.
fn construction_faction_from_building(building_name: &str) -> ConstructionFaction {
    let k = building_name.to_ascii_lowercase();
    if k.contains("china")
        || k.starts_with("nuke_")
        || k.starts_with("tank_")
        || k.starts_with("infa_")
    {
        ConstructionFaction::China
    } else if k.contains("gla")
        || k.contains("toxin")
        || k.contains("demo")
        || k.contains("stealth")
    {
        // Stealth/Demo/Toxin generals are GLA residual families.
        if k.contains("america") {
            ConstructionFaction::America
        } else if k.contains("china") {
            ConstructionFaction::China
        } else {
            ConstructionFaction::Gla
        }
    } else if k.contains("america")
        || k.contains("airf_")
        || k.contains("supw_")
        || k.contains("laser_")
    {
        ConstructionFaction::America
    } else {
        // Default USA residual when producer name is generic ("Barracks").
        ConstructionFaction::America
    }
}

/// Strip faction/prefix residual for HUD display labels.
fn friendly_buildable_label(template_name: &str) -> &str {
    let n = template_name.trim();
    for prefix in [
        "AmericaInfantry",
        "AmericaVehicle",
        "AmericaTank",
        "AmericaJet",
        "America",
        "ChinaInfantry",
        "ChinaVehicle",
        "ChinaTank",
        "China",
        "GLAInfantry",
        "GLAVehicle",
        "GLATank",
        "GLA",
    ] {
        if let Some(rest) = n.strip_prefix(prefix) {
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    n
}

/// Construction panel for building units and structures
pub struct ConstructionPanel {
    position: (i32, i32),
    size: (u32, u32),
    visible: bool,
    current_tab: ConstructionTab,
    building_queue: Vec<BuildQueueItem>,
    selected_building: Option<String>,
    /// Presentation command_set_override residual (empty = template default).
    command_set_override: String,
    tab_buttons: Vec<TabButton>,
    construction_buttons: Vec<ConstructionButton>,
    /// C++ structure placement cursor residual (template awaiting map click).
    pub(crate) pending_structure_placement: Option<String>,
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
    /// Apply CanMake available residual onto construction buttons by item_name.
    pub fn apply_can_make_availability(
        &mut self,
        by_name: &std::collections::HashMap<String, bool>,
    ) {
        for btn in &mut self.construction_buttons {
            let key = btn.item_name.to_ascii_lowercase();
            if let Some(available) = by_name.get(&key) {
                btn.enabled = *available;
            }
        }
    }

    pub fn new(x: i32, y: i32) -> Self {
        Self {
            position: (x, y),
            size: (600, layout::HUD_PANEL_HEIGHT),
            visible: false,
            current_tab: ConstructionTab::Buildings,
            building_queue: Vec::new(),
            selected_building: None,
            command_set_override: String::new(),
            tab_buttons: Vec::new(),
            construction_buttons: Vec::new(),
            pending_structure_placement: None,
        }
    }

    pub fn is_structure_tab(&self) -> bool {
        matches!(self.current_tab, ConstructionTab::Buildings)
    }

    pub fn pending_structure_placement(&self) -> Option<&str> {
        self.pending_structure_placement.as_deref()
    }

    pub fn clear_structure_placement(&mut self) {
        self.pending_structure_placement = None;
    }

    pub fn arm_structure_placement(&mut self, template_name: String) {
        self.pending_structure_placement = Some(template_name);
    }

    pub fn show_for_building(&mut self, building_name: &str) {
        self.show_for_building_with_command_set(building_name, None);
    }

    pub fn show_for_building_with_command_set(
        &mut self,
        building_name: &str,
        command_set_override: Option<&str>,
    ) {
        self.visible = true;
        self.selected_building = Some(building_name.to_string());
        self.command_set_override = command_set_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_string();
        self.setup_construction_options(building_name);
    }

    pub fn command_set_override(&self) -> &str {
        &self.command_set_override
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.selected_building = None;
        self.construction_buttons.clear();
        self.pending_structure_placement = None;
    }

    fn setup_construction_options(&mut self, building_name: &str) {
        self.construction_buttons.clear();
        let faction = construction_faction_from_building(building_name);

        // Setup tabs based on building type
        let key = building_name.to_ascii_lowercase();
        if key.contains("barracks") {
            self.current_tab = ConstructionTab::Infantry;
            self.add_infantry_units(faction);
        } else if key.contains("warfactory")
            || key.contains("war factory")
            || key.contains("armsdealer")
        {
            self.current_tab = ConstructionTab::Vehicles;
            self.add_vehicle_units(faction);
        } else if key.contains("airfield") || key.contains("air field") {
            self.current_tab = ConstructionTab::Aircraft;
            self.add_aircraft_units(faction);
        } else {
            // Command center / dozer / default: structure placement residual.
            self.current_tab = ConstructionTab::Buildings;
            self.add_building_structures(faction);
        }
    }

    fn add_infantry_units(&mut self, faction: ConstructionFaction) {
        // item_name = ThingTemplate residual; display via localized_entry.
        let units: &[(&str, i32, KeyCode)] = match faction {
            ConstructionFaction::America => &[
                ("AmericaInfantryRanger", 225, KeyCode::R),
                ("AmericaInfantryMissileDefender", 300, KeyCode::M),
                ("AmericaInfantryPathfinder", 600, KeyCode::P),
                ("AmericaInfantryColonelBurton", 1500, KeyCode::B),
            ],
            ConstructionFaction::China => &[
                ("ChinaInfantryRedguard", 300, KeyCode::R),
                ("ChinaInfantryTankHunter", 300, KeyCode::T),
                ("ChinaInfantryHacker", 500, KeyCode::H),
                ("ChinaInfantryBlackLotus", 1500, KeyCode::B),
            ],
            ConstructionFaction::Gla => &[
                ("GLAInfantryRebel", 150, KeyCode::R),
                ("GLAInfantryRPGTrooper", 300, KeyCode::G),
                ("GLAInfantryTerrorist", 200, KeyCode::T),
                ("GLAInfantryHijacker", 400, KeyCode::H),
                ("GLAInfantryJarmenKell", 1500, KeyCode::J),
            ],
        };
        self.add_construction_buttons(units);
    }

    fn add_vehicle_units(&mut self, faction: ConstructionFaction) {
        let units: &[(&str, i32, KeyCode)] = match faction {
            ConstructionFaction::America => &[
                ("AmericaVehicleHumvee", 700, KeyCode::H),
                ("AmericaTankCrusader", 1400, KeyCode::C),
                ("AmericaTankPaladin", 1800, KeyCode::P),
                ("AmericaVehicleTomahawk", 1200, KeyCode::T),
            ],
            ConstructionFaction::China => &[
                ("ChinaTankBattleMaster", 800, KeyCode::B),
                ("ChinaTankGattling", 800, KeyCode::G),
                ("ChinaTankOverlord", 2100, KeyCode::O),
                ("ChinaVehicleInfernoCannon", 900, KeyCode::I),
            ],
            ConstructionFaction::Gla => &[
                ("GLAVehicleTechnical", 500, KeyCode::T),
                ("GLATankScorpion", 600, KeyCode::S),
                ("GLAVehicleQuadCannon", 700, KeyCode::Q),
                ("GLAVehicleRocketBuggy", 900, KeyCode::R),
            ],
        };
        self.add_construction_buttons(units);
    }

    fn add_aircraft_units(&mut self, faction: ConstructionFaction) {
        let units: &[(&str, i32, KeyCode)] = match faction {
            ConstructionFaction::America => &[
                ("AmericaJetComanche", 1200, KeyCode::C),
                ("AmericaJetRaptor", 1600, KeyCode::R),
                ("AmericaJetStealthFighter", 2500, KeyCode::S),
            ],
            ConstructionFaction::China => &[
                ("ChinaJetMiG", 1200, KeyCode::M),
                // China airfield residual sample set.
            ],
            ConstructionFaction::Gla => &[
                // GLA has limited fixed-wing residual; keep empty-safe sample.
            ],
        };
        self.add_construction_buttons(units);
    }

    fn add_building_structures(&mut self, faction: ConstructionFaction) {
        let buildings: &[(&str, i32, KeyCode)] = match faction {
            ConstructionFaction::America => &[
                ("AmericaPowerPlant", 800, KeyCode::P),
                ("AmericaBarracks", 600, KeyCode::B),
                ("AmericaSupplyCenter", 2000, KeyCode::S),
                ("AmericaWarFactory", 2000, KeyCode::W),
            ],
            ConstructionFaction::China => &[
                ("ChinaPowerPlant", 800, KeyCode::P),
                ("ChinaBarracks", 500, KeyCode::B),
                ("ChinaSupplyCenter", 1500, KeyCode::S),
                ("ChinaWarFactory", 2000, KeyCode::W),
            ],
            ConstructionFaction::Gla => &[
                ("GLAPowerPlant", 800, KeyCode::P),
                ("GLABarracks", 400, KeyCode::B),
                ("GLASupplyStash", 1500, KeyCode::S),
                ("GLAArmsDealer", 1800, KeyCode::A),
            ],
        };
        self.add_construction_buttons(buildings);
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
                display_name: localized_entry(friendly_buildable_label(name)),
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

/// Snapshot-owned PublicTimer residual for HUD superweapon strip.
#[derive(Debug, Clone)]
pub struct PresentationSwTimer {
    pub name: String,
    pub template_name: String,
    pub remaining: f32,
    pub recharge_time: f32,
    pub ready: bool,
    pub unlocked: bool,
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
    pub(crate) construction_panel: ConstructionPanel,
    /// Selected units
    selected_units: Vec<ObjectId>,
    /// Selected unit identity (health/name) from PresentationFrame when available.
    selected_unit_infos: Vec<UnitDisplayInfo>,
    /// ControlBar/WND selection panel display (portrait + health) from presentation.
    selection_panel: ControlBarSelectionPanelState,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// HUD visibility
    visible: bool,
    /// Command panel for selected units
    command_buttons: Vec<CommandButton>,
    /// UI events raised from hotkeys (drained by UIManager).
    pending_ui_events: Vec<crate::ui::UIEvent>,
    /// InGameUI PublicTimer residual freeze from PresentationFrame.
    presentation_superweapon_timers: Vec<PresentationSwTimer>,
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
    /// Apply presentation PublicTimer residual (superweapon countdown strip).

    /// Apply presentation ControlBar unit-command residual (Command_* names).
    pub fn apply_presentation_unit_commands(&mut self, commands: &[(String, bool)]) {
        self.command_buttons.clear();
        if commands.is_empty() {
            return;
        }
        let button_size = 48u32;
        let spacing = 4u32;
        let start_x = (self.screen_size.0 / 2) as i32 - 200;
        let start_y = self.screen_size.1 as i32 - 60;
        for (i, (name, enabled)) in commands.iter().enumerate() {
            let hotkey = match name.as_str() {
                "Command_Stop" => Some(crate::ui::KeyCode::S),
                "Command_Guard" => Some(crate::ui::KeyCode::G),
                "Command_AttackMove" | "Command_AttackMoveTo" => Some(crate::ui::KeyCode::A),
                "Command_Deploy" => Some(crate::ui::KeyCode::D),
                "Command_Scatter" => Some(crate::ui::KeyCode::X),
                _ => None,
            };
            self.command_buttons.push(CommandButton {
                command: name.clone(),
                position: (start_x + i as i32 * (button_size + spacing) as i32, start_y),
                size: (button_size, button_size),
                icon: name.clone(),
                hotkey,
                enabled: *enabled,
                hovered: false,
            });
        }
    }

    pub fn apply_presentation_superweapon_timers(
        &mut self,
        timers: &[crate::ui::hud_state::UiSuperweaponTimer],
    ) {
        self.presentation_superweapon_timers = timers
            .iter()
            .filter(|t| t.unlocked)
            .map(|t| PresentationSwTimer {
                name: t.name.clone(),
                template_name: t.template_name.clone(),
                remaining: t.remaining,
                recharge_time: t.recharge_time,
                ready: t.ready,
                unlocked: t.unlocked,
            })
            .collect();
    }

    pub fn presentation_superweapon_timers(&self) -> &[PresentationSwTimer] {
        &self.presentation_superweapon_timers
    }

    /// Apply presentation CanMake residual onto construction buttons (enable/gray).
    pub fn apply_can_make_cameos(&mut self, cameos: &[(&str, bool, u32, Option<&str>)]) {
        if cameos.is_empty() {
            return;
        }
        let lookup: std::collections::HashMap<String, bool> = cameos
            .iter()
            .map(|(n, a, _, _)| (n.to_ascii_lowercase(), *a))
            .collect();
        self.construction_panel.apply_can_make_availability(&lookup);
    }

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
            selected_unit_infos: Vec::new(),
            selection_panel: ControlBarSelectionPanelState::default(),
            screen_size,
            visible: true,
            command_buttons: Vec::new(),
            pending_ui_events: Vec::new(),
            presentation_superweapon_timers: Vec::new(),
            messages: Vec::new(),
            beacon_markers: Vec::new(),
            beacon_events: Vec::new(),
            game_time: Duration::from_secs(0),
            power_low_active: false,
        }
    }

    /// Drain hotkey-raised UI events for the engine command path.
    pub fn drain_pending_ui_events(&mut self) -> Vec<crate::ui::UIEvent> {
        std::mem::take(&mut self.pending_ui_events)
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

        // C++ right-click cancels structure placement residual.
        if button == MouseButton::Right
            && self
                .construction_panel
                .pending_structure_placement
                .is_some()
        {
            self.construction_panel.clear_structure_placement();
            self.add_message(
                &localization::localize(
                    "hud.message.placement_cancelled",
                    "Structure placement cancelled",
                ),
                MessageType::Info,
            );
            return Some(UIEvent::CancelStructurePlacement);
        }

        // Check minimap clicks
        if let Some((world_x, world_y)) = self.minimap.world_coords_from_click(x, y) {
            if button == MouseButton::Left {
                // C++ place building via radar/minimap residual when placement armed.
                if let Some(template) = self.construction_panel.pending_structure_placement.take() {
                    return Some(UIEvent::PlaceStructureAt {
                        template_name: template,
                        location: Vec3::new(world_x, 0.0, world_y),
                    });
                }
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
                let cost_str = cost.to_string();
                // C++ structure cameo: enter placement mode (DozerConstruct on map click).
                if self.construction_panel.is_structure_tab() {
                    self.construction_panel.pending_structure_placement = Some(item_name.clone());
                    let message = localization::localize_with_args(
                        "hud.message.place_structure",
                        "Select location for {name} (${cost} Credits)",
                        &[("name", display_name.as_str()), ("cost", cost_str.as_str())],
                    );
                    self.add_message(&message, MessageType::Construction);
                    return Some(crate::ui::UIEvent::BeginStructurePlacement {
                        template_name: item_name,
                    });
                }
                // Unit/aircraft/vehicle cameo: authoritative factory queue.
                self.construction_panel
                    .add_to_queue(&item_name, &display_name, cost, 5.0);
                let message = localization::localize_with_args(
                    "hud.message.build_queue_start",
                    "Building {name} (${cost} Credits)",
                    &[("name", display_name.as_str()), ("cost", cost_str.as_str())],
                );
                self.add_message(&message, MessageType::Construction);
                return Some(crate::ui::UIEvent::QueueUnitProduction {
                    template_name: item_name,
                    quantity: 1,
                });
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
                // Execute command — map to engine CommandType via shared residual mapper.
                let command_name = localized_command(&cmd_button.command);
                let log_msg = localization::localize_with_args(
                    "hud.log.command_executed",
                    "Command executed: {command}",
                    &[("command", command_name.as_str())],
                );
                println!("{log_msg}");
                let raw = cmd_button.command.clone();
                return Some(crate::ui::UIEvent::IssueCommand { command_name: raw });
            }
        }

        None
    }

    /// Select units
    pub fn select_units(&mut self, unit_ids: Vec<ObjectId>) {
        self.selected_units = unit_ids;
        self.selected_unit_infos.clear();
        self.selection_panel = ControlBarSelectionPanelState::default();
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

    /// Quiet presentation-driven selection sync (no toast spam each frame).
    ///
    /// Production path: `PresentationFrame::apply_to_game_hud` overwrites IDs +
    /// health/name identity from the immutable snapshot after each logic step
    /// (and after map load / skirmish start seed). Also refreshes the ControlBar
    /// selection panel (health strip) from the same snapshot-owned infos.
    pub fn sync_selection_from_presentation(
        &mut self,
        unit_ids: Vec<ObjectId>,
        unit_infos: Vec<UnitDisplayInfo>,
    ) {
        let selection_changed = self.selected_units != unit_ids;
        self.selected_units = unit_ids;
        self.selected_unit_infos = unit_infos;
        self.selection_panel =
            ControlBarSelectionPanelState::from_unit_infos(&self.selected_unit_infos);
        if selection_changed {
            self.update_command_buttons();
            // Prefer presentation producer residual for construction panel.
            if let Some(info) = self.selected_unit_infos.first() {
                if info.can_produce {
                    let override_cs = if info.command_set_override.is_empty() {
                        None
                    } else {
                        Some(info.command_set_override.as_str())
                    };
                    self.construction_panel
                        .show_for_building_with_command_set(&info.name, override_cs);
                }
            }
        }
    }

    /// Selected object IDs currently shown on the HUD command strip.
    pub fn selected_unit_ids(&self) -> &[ObjectId] {
        &self.selected_units
    }

    /// Selected unit identity (health/name) from presentation when available.
    pub fn selected_unit_infos(&self) -> &[UnitDisplayInfo] {
        &self.selected_unit_infos
    }

    /// ControlBar/WND selection panel state (health/name) from presentation.
    pub fn selection_panel(&self) -> &ControlBarSelectionPanelState {
        &self.selection_panel
    }

    /// Select building
    pub fn select_building(&mut self, building_name: &str) {
        self.select_building_with_command_set(building_name, None);
    }

    /// Select building with optional presentation command_set_override residual.
    pub fn select_building_with_command_set(
        &mut self,
        building_name: &str,
        command_set_override: Option<&str>,
    ) {
        self.construction_panel
            .show_for_building_with_command_set(building_name, command_set_override);
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
    /// Test/honesty: number of active HUD messages.
    pub fn message_count_for_test(&self) -> usize {
        self.messages.len()
    }

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

    /// Control-bar / HUD strip visibility residual.
    pub fn hud_visible(&self) -> bool {
        self.visible
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
        // C++ Escape cancels structure placement residual.
        if key == KeyCode::Escape
            && self
                .construction_panel
                .pending_structure_placement
                .is_some()
        {
            self.construction_panel.clear_structure_placement();
            self.pending_ui_events
                .push(crate::ui::UIEvent::CancelStructurePlacement);
            return true;
        }

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
                if self.construction_panel.is_structure_tab() {
                    self.construction_panel.pending_structure_placement = Some(item_name.clone());
                    self.pending_ui_events
                        .push(crate::ui::UIEvent::BeginStructurePlacement {
                            template_name: item_name,
                        });
                    let _ = (display_name, cost);
                    return true;
                }
                self.construction_panel
                    .add_to_queue(&item_name, &display_name, cost, 5.0);
                self.pending_ui_events
                    .push(crate::ui::UIEvent::QueueUnitProduction {
                        template_name: item_name,
                        quantity: 1,
                    });
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
                self.pending_ui_events
                    .push(crate::ui::UIEvent::IssueCommand {
                        command_name: button.command.clone(),
                    });
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

        // ControlBar selection panel (portrait + health) from presentation snapshot.
        if self.selection_panel.visible {
            let name = if self.selection_panel.primary_name.is_empty() {
                localization::localize("hud.panel.selected_unit", "Selected unit")
            } else {
                self.selection_panel.primary_name.clone()
            };
            let hp = self.selection_panel.health_current;
            let hp_max = self.selection_panel.health_maximum;
            let count = self.selection_panel.selected_count.to_string();
            let panel_label = localization::localize_with_args(
                "hud.panel.selection_panel",
                "Selection Panel: {name} HP {hp}/{max} ({count} selected)",
                &[
                    ("name", name.as_str()),
                    ("hp", &format!("{hp:.0}")),
                    ("max", &format!("{hp_max:.0}")),
                    ("count", count.as_str()),
                ],
            );
            println!("{panel_label}");
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
    use crate::ui::UIEvent;

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

    #[test]
    fn china_barracks_shows_redguard_cameo_residual() {
        let mut panel = ConstructionPanel::new(0, 0);
        panel.show_for_building("ChinaBarracks");
        assert!(panel.is_structure_tab() == false);
        let names: Vec<_> = panel
            .construction_buttons
            .iter()
            .map(|b| b.item_name.as_str())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("Redguard")),
            "China barracks should list Redguard residual: {:?}",
            names
        );
        assert!(
            !names.iter().any(|n| n.contains("Ranger")),
            "China barracks must not list USA Ranger: {:?}",
            names
        );
    }

    #[test]
    fn gla_barracks_shows_rebel_cameo_residual() {
        let mut panel = ConstructionPanel::new(0, 0);
        panel.show_for_building("GLABarracks");
        let names: Vec<_> = panel
            .construction_buttons
            .iter()
            .map(|b| b.item_name.as_str())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("Rebel")),
            "GLA barracks should list Rebel residual: {:?}",
            names
        );
    }

    #[test]
    fn america_cc_shows_america_structures_residual() {
        let mut panel = ConstructionPanel::new(0, 0);
        panel.show_for_building("AmericaCommandCenter");
        assert!(panel.is_structure_tab());
        let names: Vec<_> = panel
            .construction_buttons
            .iter()
            .map(|b| b.item_name.as_str())
            .collect();
        assert!(names.iter().any(|n| *n == "AmericaBarracks"));
        assert!(names.iter().any(|n| *n == "AmericaPowerPlant"));
    }

    fn construction_click_emits_queue_unit_production_event_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel.visible = true;
        hud.construction_panel.current_tab = ConstructionTab::Infantry;
        hud.construction_panel.construction_buttons.clear();
        hud.construction_panel
            .construction_buttons
            .push(ConstructionButton {
                item_name: "AmericaInfantryRanger".into(),
                display_name: "Ranger".into(),
                position: (10, 10),
                size: (64, 64),
                cost: 150,
                enabled: true,
                hovered: false,
                build_key: Some(KeyCode::R),
            });
        let ev = hud
            .handle_mouse_click(20, 20, MouseButton::Left)
            .expect("click event");
        match ev {
            UIEvent::QueueUnitProduction {
                template_name,
                quantity,
            } => {
                assert_eq!(template_name, "AmericaInfantryRanger");
                assert_eq!(quantity, 1);
            }
            other => panic!("expected QueueUnitProduction, got {other:?}"),
        }
    }

    #[test]
    fn right_click_cancels_structure_placement_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel
            .arm_structure_placement("AmericaBarracks".into());
        let ev = hud
            .handle_mouse_click(100, 100, MouseButton::Right)
            .expect("cancel event");
        assert!(matches!(ev, UIEvent::CancelStructurePlacement));
        assert!(hud
            .construction_panel
            .pending_structure_placement()
            .is_none());
    }

    #[test]
    fn escape_cancels_structure_placement_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel
            .arm_structure_placement("AmericaPowerPlant".into());
        assert!(hud.handle_key_press(KeyCode::Escape));
        let pending = hud.drain_pending_ui_events();
        assert!(matches!(
            pending.as_slice(),
            [UIEvent::CancelStructurePlacement]
        ));
        assert!(hud
            .construction_panel
            .pending_structure_placement()
            .is_none());
    }

    #[test]
    fn structure_cameo_begins_placement_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel.visible = true;
        hud.construction_panel.current_tab = ConstructionTab::Buildings;
        hud.construction_panel.construction_buttons.clear();
        hud.construction_panel
            .construction_buttons
            .push(ConstructionButton {
                item_name: "AmericaBarracks".into(),
                display_name: "Barracks".into(),
                position: (10, 10),
                size: (64, 64),
                cost: 600,
                enabled: true,
                hovered: false,
                build_key: Some(KeyCode::B),
            });
        let ev = hud
            .handle_mouse_click(20, 20, MouseButton::Left)
            .expect("placement event");
        match ev {
            UIEvent::BeginStructurePlacement { template_name } => {
                assert_eq!(template_name, "AmericaBarracks");
            }
            other => panic!("expected BeginStructurePlacement, got {other:?}"),
        }
        assert_eq!(
            hud.construction_panel.pending_structure_placement(),
            Some("AmericaBarracks")
        );
    }

    #[test]
    fn minimap_click_places_pending_structure_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel.pending_structure_placement = Some("AmericaPowerPlant".into());
        // Click center of default minimap (bottom-right of 1024x768).
        let mx = 1024 - 10 - 64;
        let my = 768 - 10 - 64;
        let ev = hud
            .handle_mouse_click(mx, my, MouseButton::Left)
            .expect("place event");
        match ev {
            UIEvent::PlaceStructureAt {
                template_name,
                location,
            } => {
                assert_eq!(template_name, "AmericaPowerPlant");
                assert!(location.x.is_finite() && location.z.is_finite());
            }
            other => panic!("expected PlaceStructureAt, got {other:?}"),
        }
        assert!(hud
            .construction_panel
            .pending_structure_placement()
            .is_none());
    }

    fn construction_hotkey_queues_pending_ui_event_residual() {
        let mut hud = GameHUD::new();
        hud.initialize().expect("init");
        hud.construction_panel.visible = true;
        hud.construction_panel.current_tab = ConstructionTab::Vehicles;
        hud.construction_panel.construction_buttons.clear();
        hud.construction_panel
            .construction_buttons
            .push(ConstructionButton {
                item_name: "AmericaTankCrusader".into(),
                display_name: "Crusader".into(),
                position: (0, 0),
                size: (32, 32),
                cost: 900,
                enabled: true,
                hovered: false,
                build_key: Some(KeyCode::C),
            });
        assert!(hud.handle_key_press(KeyCode::C));
        let pending = hud.drain_pending_ui_events();
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            &pending[0],
            UIEvent::QueueUnitProduction {
                template_name,
                quantity
            } if template_name == "AmericaTankCrusader" && *quantity == 1
        ));
    }
}
