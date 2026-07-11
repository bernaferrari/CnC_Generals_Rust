//! Skirmish Setup Screen
//!
//! This module implements the skirmish game setup matching the original
//! C&C Generals interface from SkirmishGameOptionsMenu.cpp.
//! Configures map, player slots, teams, factions, and game rules.

use super::{
    layout, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen,
    UIEvent, UIRenderContext,
};
use crate::game_logic::GameMode;
use crate::localization;
use log::info;

/// Maximum number of player slots (from C++ MAX_SLOTS)
pub const MAX_SLOTS: usize = 8;

/// Player type for a slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerType {
    Open,
    Human,
    EasyAI,
    MediumAI,
    HardAI,
    BrutalAI,
    Closed,
}

/// Faction/Side selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faction {
    USA,
    China,
    GLA,
    Random,
}

/// Player colors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerColor {
    Red,
    Blue,
    Green,
    Yellow,
    Orange,
    Purple,
    Cyan,
    White,
}

impl PlayerColor {
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        match self {
            PlayerColor::Red => (200, 0, 0),
            PlayerColor::Blue => (0, 0, 200),
            PlayerColor::Green => (0, 200, 0),
            PlayerColor::Yellow => (200, 200, 0),
            PlayerColor::Orange => (255, 140, 0),
            PlayerColor::Purple => (160, 32, 240),
            PlayerColor::Cyan => (0, 200, 200),
            PlayerColor::White => (255, 255, 255),
        }
    }
}

/// Game slot configuration
#[derive(Debug, Clone)]
pub struct GameSlot {
    pub slot_index: usize,
    pub player_type: PlayerType,
    pub faction: Faction,
    pub color: PlayerColor,
    pub team: i32,
    pub start_position: i32,
    pub player_name: String,
}

impl GameSlot {
    pub fn new(index: usize) -> Self {
        let colors = [
            PlayerColor::Red,
            PlayerColor::Blue,
            PlayerColor::Green,
            PlayerColor::Yellow,
            PlayerColor::Orange,
            PlayerColor::Purple,
            PlayerColor::Cyan,
            PlayerColor::White,
        ];

        Self {
            slot_index: index,
            player_type: if index == 0 {
                PlayerType::Human
            } else {
                PlayerType::Open
            },
            faction: Faction::Random,
            color: colors[index % colors.len()],
            team: -1, // No team
            start_position: index as i32,
            player_name: if index == 0 {
                "Player".to_string()
            } else {
                format!("AI Player {}", index + 1)
            },
        }
    }
}

/// Game rules configuration
#[derive(Debug, Clone)]
pub struct GameRules {
    pub starting_cash: i32,
    pub game_speed: f32,
    pub limit_superweapons: bool,
    pub allow_tech_buildings: bool,
    pub crates_enabled: bool,
    pub fog_of_war: bool,
}

impl Default for GameRules {
    fn default() -> Self {
        Self {
            starting_cash: 10000,
            game_speed: 1.0,
            limit_superweapons: false,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: true,
        }
    }
}

/// Action buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
    Start,
    SelectMap,
    Reset,
    Exit,
}

struct ActionBtn {
    action: ActionButton,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    enabled: bool,
    click_spring: ClickSpring,
}

impl ActionBtn {
    fn new(action: ActionButton, text: String, x: i32, y: i32) -> Self {
        Self {
            action,
            text,
            position: (x, y),
            size: (140, 40),
            hovered: false,
            enabled: true,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn update(&mut self, delta_time: f32) {
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Skirmish Setup Menu (from C++ SkirmishGameOptionsMenu.cpp)
pub struct SkirmishMenu {
    /// All player slots
    slots: Vec<GameSlot>,
    /// Game rules
    rules: GameRules,
    /// Selected map name
    map_name: String,
    /// Map preview available
    has_map_preview: bool,
    /// Action buttons
    action_buttons: Vec<ActionBtn>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Animation progress
    animation_progress: f32,
    /// Currently hovered slot
    hovered_slot: Option<usize>,
    pending_events: Vec<UIEvent>,
}

impl Default for SkirmishMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl SkirmishMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    /// Create new skirmish menu
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            rules: GameRules::default(),
            map_name: "Default Map".to_string(),
            has_map_preview: false,
            action_buttons: Vec::new(),
            screen_size: (1024, 768),
            animation_progress: 0.0,
            hovered_slot: None,
            pending_events: Vec::new(),
        }
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Initialize skirmish menu
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize all slots
        self.slots.clear();
        for i in 0..MAX_SLOTS {
            self.slots.push(GameSlot::new(i));
        }

        self.setup_action_buttons();
        Ok(())
    }

    /// Update skirmish menu
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Update animation
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 2.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }
        for action_btn in &mut self.action_buttons {
            action_btn.update(delta_time);
        }

        Ok(())
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        // Check action buttons
        let mut clicked_action = None;
        for action_btn in &mut self.action_buttons {
            if action_btn.contains_point(x, y) && action_btn.enabled {
                action_btn.trigger_click();
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                clicked_action = Some(action_btn.action);
                break;
            }
        }
        if let Some(action) = clicked_action {
            return self.handle_action(action);
        }

        // Check slot interactions
        if let Some(slot_index) = self.hovered_slot {
            self.cycle_slot_setting(slot_index);
        }

        None
    }

    /// Resize menu
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.setup_action_buttons();
    }

    /// Get configured game settings for starting
    pub fn get_game_config(&self) -> (Vec<GameSlot>, GameRules, String) {
        (
            self.slots.clone(),
            self.rules.clone(),
            self.map_name.clone(),
        )
    }

    /// Production slot-type cycle used by the skirmish UI (Open → Easy → Medium → …).
    /// Exposed for headless host smoke so config is built via the same menu path.
    pub fn cycle_slot_player_type(&mut self, slot_index: usize) {
        self.cycle_slot_setting(slot_index);
    }

    /// Configure slot as Medium AI opponent (common skirmish setup) via type cycling.
    /// Returns true when the slot ends as MediumAI.
    pub fn configure_slot_medium_ai(&mut self, slot_index: usize) -> bool {
        if slot_index >= self.slots.len() {
            return false;
        }
        // Cycle until MediumAI or until a full type loop (safety).
        for _ in 0..8 {
            if matches!(self.slots[slot_index].player_type, PlayerType::MediumAI) {
                self.setup_action_buttons();
                return true;
            }
            self.cycle_slot_setting(slot_index);
        }
        self.setup_action_buttons();
        matches!(self.slots[slot_index].player_type, PlayerType::MediumAI)
    }

    /// Set the selected map name (map select screen → skirmish menu).
    pub fn set_map_name(&mut self, map: impl Into<String>) {
        self.map_name = map.into();
    }

    // Private methods

    fn setup_action_buttons(&mut self) {
        self.action_buttons.clear();

        let bottom_y = self.screen_size.1 as i32 - 70;
        let button_spacing = 160;
        let start_x = (self.screen_size.0 as i32 / 2) - (button_spacing * 2);

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Start,
            Self::text("skirmish.start", "Start Game"),
            start_x,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::SelectMap,
            Self::text("skirmish.select_map", "Select Map"),
            start_x + button_spacing,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Reset,
            Self::text("skirmish.reset", "Reset"),
            start_x + button_spacing * 2,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Exit,
            Self::text("skirmish.exit", "Back"),
            start_x + button_spacing * 3,
            bottom_y,
        ));

        // Enable Start button only if at least 2 players configured
        let active_players = self
            .slots
            .iter()
            .filter(|s| !matches!(s.player_type, PlayerType::Open | PlayerType::Closed))
            .count();

        for btn in &mut self.action_buttons {
            if btn.action == ActionButton::Start {
                btn.enabled = active_players >= 2;
            }
        }
    }

    fn cycle_slot_setting(&mut self, slot_index: usize) {
        if slot_index >= self.slots.len() {
            return;
        }

        let slot = &mut self.slots[slot_index];

        // Cycle player type
        slot.player_type = match slot.player_type {
            PlayerType::Open => PlayerType::EasyAI,
            PlayerType::EasyAI => PlayerType::MediumAI,
            PlayerType::MediumAI => PlayerType::HardAI,
            PlayerType::HardAI => PlayerType::BrutalAI,
            PlayerType::BrutalAI => PlayerType::Human,
            PlayerType::Human => PlayerType::Closed,
            PlayerType::Closed => PlayerType::Open,
        };

        info!(
            "{}",
            localization::localize_with_args(
                "skirmish.log.slot_changed",
                "Slot {index} set to {type:?}",
                &[
                    ("index", &slot_index.to_string()),
                    ("type", &format!("{:?}", slot.player_type))
                ],
            )
        );

        self.setup_action_buttons(); // Update start button state
    }

    fn handle_action(&mut self, action: ActionButton) -> Option<UIEvent> {
        match action {
            ActionButton::Start => {
                let active_players = self
                    .slots
                    .iter()
                    .filter(|s| !matches!(s.player_type, PlayerType::Open | PlayerType::Closed))
                    .count();

                if active_players >= 2 {
                    info!(
                        "{}",
                        localization::localize_with_args(
                            "skirmish.log.start_game",
                            "Starting skirmish game with {count} players on map '{map}'",
                            &[
                                ("count", &active_players.to_string()),
                                ("map", &self.map_name)
                            ],
                        )
                    );
                    let faction = self
                        .slots
                        .first()
                        .map(|slot| match slot.faction {
                            Faction::USA => "USA",
                            Faction::China => "China",
                            Faction::GLA => "GLA",
                            Faction::Random => "Random",
                        })
                        .unwrap_or("Random");
                    let skirmish = crate::skirmish_config::config_from_skirmish_menu(
                        &self.map_name,
                        &self.rules,
                        &self.slots,
                    );
                    Some(UIEvent::StartGame {
                        mode: GameMode::Skirmish,
                        faction: faction.to_string(),
                        map: self.map_name.clone(),
                        skirmish: Some(skirmish),
                    })
                } else {
                    info!(
                        "{}",
                        Self::text(
                            "skirmish.log.need_more_players",
                            "Need at least 2 players to start"
                        )
                    );
                    None
                }
            }
            ActionButton::SelectMap => Some(UIEvent::ChangeScreen(Screen::MapSelection)),
            ActionButton::Reset => {
                self.initialize().ok();
                info!(
                    "{}",
                    Self::text("skirmish.log.reset", "Skirmish settings reset")
                );
                None
            }
            ActionButton::Exit => Some(UIEvent::ChangeScreen(Screen::MainMenu)),
        }
    }

    fn get_slot_area(&self, slot_index: usize) -> (i32, i32, u32, u32) {
        let start_x = 50;
        let start_y = 120;
        let row_height = 60;

        let y = start_y + (slot_index as i32 * row_height);
        (start_x, y, 600, row_height as u32)
    }
}

impl Interactive for SkirmishMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;
        self.hovered_slot = None;

        // Check action buttons
        for action_btn in &mut self.action_buttons {
            let was_hovered = action_btn.hovered;
            let is_hovered = action_btn.contains_point(x, y);
            if is_hovered != was_hovered {
                action_btn.hovered = is_hovered;
                handled = true;
                if is_hovered {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_HOVER.to_string(),
                    ));
                }
            }
        }

        // Check slot areas
        for (i, _slot) in self.slots.iter().enumerate() {
            let (sx, sy, sw, sh) = self.get_slot_area(i);
            if utils::point_in_rect((x, y), (sx, sy, sw, sh)) {
                self.hovered_slot = Some(i);
                handled = true;
                break;
            }
        }

        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        self.handle_mouse_click(x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => true,
            KeyCode::Enter => {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                if let Some(event) = self.handle_action(ActionButton::Start) {
                    self.pending_events.push(event);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for SkirmishMenu {
    fn render(&self, _context: &mut UIRenderContext) {
        println!(
            "{}",
            Self::text("skirmish.log.header", "=== SKIRMISH SETUP ===")
        );

        // Render map info
        println!(
            "\n{} {}",
            Self::text("skirmish.map_label", "Map:"),
            self.map_name
        );

        // Render game rules
        println!("\n{}", Self::text("skirmish.rules_header", "Game Rules:"));
        println!(
            "  {}: ${}",
            Self::text("skirmish.starting_cash", "Starting Cash"),
            self.rules.starting_cash
        );
        println!(
            "  {}: {:.0}%",
            Self::text("skirmish.game_speed", "Game Speed"),
            self.rules.game_speed * 100.0
        );
        println!(
            "  {}: {}",
            Self::text("skirmish.superweapons", "Limit Superweapons"),
            if self.rules.limit_superweapons {
                "YES"
            } else {
                "NO"
            }
        );

        // Render player slots
        println!("\n{}", Self::text("skirmish.slots_header", "Player Slots:"));

        for slot in &self.slots {
            let player_type_str = match slot.player_type {
                PlayerType::Open => Self::text("skirmish.slot_open", "Open"),
                PlayerType::Human => Self::text("skirmish.slot_human", "Human"),
                PlayerType::EasyAI => Self::text("skirmish.slot_easy_ai", "Easy AI"),
                PlayerType::MediumAI => Self::text("skirmish.slot_medium_ai", "Medium AI"),
                PlayerType::HardAI => Self::text("skirmish.slot_hard_ai", "Hard AI"),
                PlayerType::BrutalAI => Self::text("skirmish.slot_brutal_ai", "Brutal AI"),
                PlayerType::Closed => Self::text("skirmish.slot_closed", "Closed"),
            };

            let faction_str = format!("{:?}", slot.faction);
            let color_str = format!("{:?}", slot.color);
            let team_str = if slot.team >= 0 {
                format!("Team {}", slot.team + 1)
            } else {
                Self::text("skirmish.no_team", "No Team")
            };

            let hover_marker = if Some(slot.slot_index) == self.hovered_slot {
                " [HOVER]"
            } else {
                ""
            };

            println!(
                "  Slot {}: {} | {} | {} | {}{}",
                slot.slot_index + 1,
                player_type_str,
                faction_str,
                color_str,
                team_str,
                hover_marker
            );
        }

        // Render action buttons
        println!("\n{}", Self::text("skirmish.actions_header", "Actions:"));
        for action_btn in &self.action_buttons {
            let state = if !action_btn.enabled {
                " [DISABLED]"
            } else if action_btn.hovered {
                " [HOVERED]"
            } else {
                ""
            };
            println!("  {}{}", action_btn.text, state);
        }

        let active_players = self
            .slots
            .iter()
            .filter(|s| !matches!(s.player_type, PlayerType::Open | PlayerType::Closed))
            .count();

        println!(
            "\n{} {}",
            Self::text("skirmish.active_players", "Active Players:"),
            active_players
        );
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skirmish_menu_creation() {
        let menu = SkirmishMenu::new();
        assert_eq!(menu.slots.len(), 0);
        assert_eq!(menu.rules.starting_cash, 10000);
    }

    #[test]
    fn test_slot_initialization() {
        let mut menu = SkirmishMenu::new();
        menu.initialize().unwrap();

        assert_eq!(menu.slots.len(), MAX_SLOTS);
        assert_eq!(menu.slots[0].player_type, PlayerType::Human);
        assert_eq!(menu.slots[1].player_type, PlayerType::Open);
    }

    #[test]
    fn test_player_colors() {
        let red = PlayerColor::Red.to_rgb();
        assert_eq!(red, (200, 0, 0));
    }

    #[test]
    fn test_slot_cycling() {
        let mut menu = SkirmishMenu::new();
        menu.initialize().unwrap();

        let initial_type = menu.slots[1].player_type;
        menu.cycle_slot_setting(1);
        assert_ne!(menu.slots[1].player_type, initial_type);
    }
}
