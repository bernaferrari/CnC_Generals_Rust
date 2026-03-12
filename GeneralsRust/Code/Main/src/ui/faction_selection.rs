//! Faction Selection Screen
//!
//! This module implements the faction selection interface where players choose
//! their faction (USA, China, GLA) and general type for skirmish and multiplayer games.

use super::{
    colors, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen,
    UIEvent, UIRenderContext,
};
use crate::{game_logic::GameMode, localization};

/// Available factions in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faction {
    USA,
    China,
    GLA,
}

impl Faction {
    fn localization_key(&self) -> &'static str {
        match self {
            Faction::USA => "usa",
            Faction::China => "china",
            Faction::GLA => "gla",
        }
    }

    fn get_name(&self) -> String {
        let fallback = match self {
            Faction::USA => "United States",
            Faction::China => "People's Republic of China",
            Faction::GLA => "Global Liberation Army",
        };
        localization::localize(
            &format!("faction.{}.name", self.localization_key()),
            fallback,
        )
    }

    fn get_description(&self) -> String {
        let fallback = match self {
            Faction::USA => "High-tech military with advanced air power and laser technology.",
            Faction::China => "Massive ground forces with nuclear weapons and overlord tanks.",
            Faction::GLA => "Guerrilla warfare specialists with stealth and chemical weapons.",
        };
        localization::localize(
            &format!("faction.{}.description", self.localization_key()),
            fallback,
        )
    }

    fn get_color(&self) -> (u8, u8, u8) {
        match self {
            Faction::USA => colors::BLUE_LIGHT,
            Faction::China => colors::RED,
            Faction::GLA => colors::ORANGE,
        }
    }
}

/// General types for each faction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum General {
    // USA Generals
    USAAirForce,    // Malcolm "Ace" Granger
    USASuperWeapon, // Townes "Pin Point" Jarrett
    USALaser,       // Alexis Alexander

    // China Generals
    ChinaTank,     // "Anvil" Shin Fai
    ChinaNuke,     // "Nuke" Tsing Shi Tao
    ChinaInfantry, // "Lotus" Leang

    // GLA Generals
    GLAToxin,     // Dr. Thrax
    GLAStealth,   // "Deathstrike" Kassad
    GLAExplosive, // "Demo" Mohmar
}

impl General {
    fn localization_key(&self) -> &'static str {
        match self {
            General::USAAirForce => "usa_air_force",
            General::USASuperWeapon => "usa_superweapon",
            General::USALaser => "usa_laser",
            General::ChinaTank => "china_tank",
            General::ChinaNuke => "china_nuke",
            General::ChinaInfantry => "china_infantry",
            General::GLAToxin => "gla_toxin",
            General::GLAStealth => "gla_stealth",
            General::GLAExplosive => "gla_explosive",
        }
    }

    fn get_name(&self) -> String {
        let fallback = match self {
            General::USAAirForce => "Air Force General",
            General::USASuperWeapon => "Superweapon General",
            General::USALaser => "Laser General",
            General::ChinaTank => "Tank General",
            General::ChinaNuke => "Nuclear General",
            General::ChinaInfantry => "Infantry General",
            General::GLAToxin => "Toxin General",
            General::GLAStealth => "Stealth General",
            General::GLAExplosive => "Demolition General",
        };
        localization::localize(
            &format!("general.{}.name", self.localization_key()),
            fallback,
        )
    }

    fn get_faction(&self) -> Faction {
        match self {
            General::USAAirForce | General::USASuperWeapon | General::USALaser => Faction::USA,
            General::ChinaTank | General::ChinaNuke | General::ChinaInfantry => Faction::China,
            General::GLAToxin | General::GLAStealth | General::GLAExplosive => Faction::GLA,
        }
    }

    fn get_description(&self) -> String {
        let fallback = match self {
            General::USAAirForce => "Specializes in air superiority and advanced aircraft.",
            General::USASuperWeapon => "Masters particle cannons and orbital strikes.",
            General::USALaser => "Deploys laser technology and advanced defenses.",
            General::ChinaTank => "Commands massive armored formations.",
            General::ChinaNuke => "Wields devastating nuclear weapons.",
            General::ChinaInfantry => "Leads elite special forces units.",
            General::GLAToxin => "Spreads chemical warfare and biological weapons.",
            General::GLAStealth => "Master of camouflage and ambush tactics.",
            General::GLAExplosive => "Demolition expert with powerful explosives.",
        };
        localization::localize(
            &format!("general.{}.description", self.localization_key()),
            fallback,
        )
    }
}

/// Faction selection button
struct FactionButton {
    faction: Faction,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    selected: bool,
    animation_time: f32,
    click_spring: ClickSpring,
}

impl FactionButton {
    fn new(faction: Faction, x: i32, y: i32) -> Self {
        Self {
            faction,
            position: (x, y),
            size: (250, 300),
            hovered: false,
            selected: false,
            animation_time: 0.0,
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
        self.animation_time += delta_time;
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// General selection button
struct GeneralButton {
    general: General,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    selected: bool,
    enabled: bool,
    click_spring: ClickSpring,
}

impl GeneralButton {
    fn new(general: General, x: i32, y: i32) -> Self {
        Self {
            general,
            position: (x, y),
            size: (200, 80),
            hovered: false,
            selected: false,
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

/// Faction selection screen implementation
pub struct FactionSelectionScreen {
    /// Currently selected faction
    selected_faction: Option<Faction>,
    /// Currently selected general
    selected_general: Option<General>,
    /// Faction selection buttons
    faction_buttons: Vec<FactionButton>,
    /// General selection buttons
    general_buttons: Vec<GeneralButton>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Animation progress
    animation_progress: f32,
    /// Currently hovered faction
    hovered_faction: Option<Faction>,
    /// Currently hovered general
    hovered_general: Option<General>,
    /// Map selection
    selected_map: String,
    /// Available maps
    available_maps: Vec<String>,
    /// AI difficulty
    ai_difficulty: u32,
    pending_events: Vec<UIEvent>,
    start_click: ClickSpring,
    back_click: ClickSpring,
}

impl Default for FactionSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl FactionSelectionScreen {
    /// Create new faction selection screen
    pub fn new() -> Self {
        Self {
            selected_faction: None,
            selected_general: None,
            faction_buttons: Vec::new(),
            general_buttons: Vec::new(),
            screen_size: (1024, 768),
            animation_progress: 0.0,
            hovered_faction: None,
            hovered_general: None,
            selected_map: "Tournament Desert".to_string(),
            available_maps: vec![
                "Tournament Desert".to_string(),
                "Tournament Tundra".to_string(),
                "Twilight Flame".to_string(),
                "Winter Wolf".to_string(),
                "Tournament Island".to_string(),
                "Scorched Earth".to_string(),
                "Silent River".to_string(),
                "Fortress Avalanche".to_string(),
            ],
            ai_difficulty: 2, // Medium difficulty
            pending_events: Vec::new(),
            start_click: ClickSpring::new(),
            back_click: ClickSpring::new(),
        }
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Initialize faction selection screen
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_faction_buttons();
        Ok(())
    }

    /// Update faction selection screen
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Update animation progress
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 2.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }

        // Update faction button animations
        for button in &mut self.faction_buttons {
            button.update(delta_time);
        }
        for button in &mut self.general_buttons {
            button.update(delta_time);
        }
        self.start_click.update(delta_time);
        self.back_click.update(delta_time);

        Ok(())
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        // Check faction button clicks
        let mut clicked_faction = None;
        for faction_button in &mut self.faction_buttons {
            if faction_button.contains_point(x, y) {
                faction_button.trigger_click();
                clicked_faction = Some(faction_button.faction);
                break;
            }
        }

        if let Some(faction) = clicked_faction {
            // Select faction and update generals
            self.selected_faction = Some(faction);
            self.setup_general_buttons(faction);

            // Update button states
            for btn in &mut self.faction_buttons {
                btn.selected = btn.faction == faction;
            }

            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            return None;
        }

        // Check general button clicks
        let mut clicked_general = None;
        for general_button in &mut self.general_buttons {
            if general_button.contains_point(x, y) && general_button.enabled {
                general_button.trigger_click();
                clicked_general = Some(general_button.general);
                break;
            }
        }

        if let Some(general) = clicked_general {
            self.selected_general = Some(general);

            // Update button states
            for btn in &mut self.general_buttons {
                btn.selected = btn.general == general;
            }

            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            return None;
        }

        // Check control buttons
        if self.is_ready_to_start() {
            // Check "Start Game" button
            let start_button_rect = self.get_start_button_rect();
            if utils::point_in_rect((x, y), start_button_rect) {
                self.start_click.trigger();
                return Some(UIEvent::StartGame {
                    mode: GameMode::Skirmish,
                    faction: format!("{:?}", self.selected_faction.unwrap()),
                    map: self.selected_map.clone(),
                });
            }
        }

        // Check "Back" button
        let back_button_rect = self.get_back_button_rect();
        if utils::point_in_rect((x, y), back_button_rect) {
            self.back_click.trigger();
            return Some(UIEvent::ChangeScreen(Screen::MainMenu));
        }

        None
    }

    /// Resize screen
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.setup_faction_buttons();

        if self.selected_faction.is_some() {
            self.setup_general_buttons(self.selected_faction.unwrap());
        }
    }

    /// Check if ready to start game
    fn is_ready_to_start(&self) -> bool {
        self.selected_faction.is_some() && self.selected_general.is_some()
    }

    /// Setup faction selection buttons
    fn setup_faction_buttons(&mut self) {
        self.faction_buttons.clear();

        let button_width = 250u32;
        let button_spacing = 50u32;
        let total_width = button_width * 3 + button_spacing * 2;
        let start_x = (self.screen_size.0 - total_width) as i32 / 2;
        let y = 150i32;

        self.faction_buttons
            .push(FactionButton::new(Faction::USA, start_x, y));

        self.faction_buttons.push(FactionButton::new(
            Faction::China,
            start_x + (button_width + button_spacing) as i32,
            y,
        ));

        self.faction_buttons.push(FactionButton::new(
            Faction::GLA,
            start_x + (button_width + button_spacing) as i32 * 2,
            y,
        ));
    }

    /// Setup general selection buttons for faction
    fn setup_general_buttons(&mut self, faction: Faction) {
        self.general_buttons.clear();

        let generals = match faction {
            Faction::USA => vec![
                General::USAAirForce,
                General::USASuperWeapon,
                General::USALaser,
            ],
            Faction::China => vec![
                General::ChinaTank,
                General::ChinaNuke,
                General::ChinaInfantry,
            ],
            Faction::GLA => vec![
                General::GLAToxin,
                General::GLAStealth,
                General::GLAExplosive,
            ],
        };

        let start_x = 50i32;
        let start_y = 500i32;
        let spacing = 90i32;

        for (i, general) in generals.iter().enumerate() {
            self.general_buttons.push(GeneralButton::new(
                *general,
                start_x,
                start_y + i as i32 * spacing,
            ));
        }
    }

    /// Get start button rectangle
    fn get_start_button_rect(&self) -> (i32, i32, u32, u32) {
        let width = 150u32;
        let height = 40u32;
        let x = self.screen_size.0 as i32 - width as i32 - 50;
        let y = self.screen_size.1 as i32 - height as i32 - 50;
        (x, y, width, height)
    }

    /// Get back button rectangle
    fn get_back_button_rect(&self) -> (i32, i32, u32, u32) {
        let width = 100u32;
        let height = 40u32;
        let x = 50i32;
        let y = self.screen_size.1 as i32 - height as i32 - 50;
        (x, y, width, height)
    }
}

impl Interactive for FactionSelectionScreen {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;

        // Reset hover states
        self.hovered_faction = None;
        self.hovered_general = None;

        // Check faction button hovers
        for button in &mut self.faction_buttons {
            let was_hovered = button.hovered;
            let is_hovered = button.contains_point(x, y);

            if is_hovered {
                self.hovered_faction = Some(button.faction);
                handled = true;
            }

            button.hovered = is_hovered;

            if is_hovered && !was_hovered {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_HOVER.to_string(),
                ));
            }
        }

        // Check general button hovers
        for button in &mut self.general_buttons {
            let was_hovered = button.hovered;
            let is_hovered = button.contains_point(x, y) && button.enabled;

            if is_hovered {
                self.hovered_general = Some(button.general);
                handled = true;
            }

            button.hovered = is_hovered;

            if is_hovered && !was_hovered {
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_HOVER.to_string(),
                ));
            }
        }

        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        self.handle_mouse_click(x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => {
                // Go back to main menu
                self.pending_events.push(UIEvent::PlaySoundEffectPath(
                    sound_files::BUTTON_CLICK.to_string(),
                ));
                self.pending_events
                    .push(UIEvent::ChangeScreen(Screen::MainMenu));
                true
            }
            KeyCode::Enter => {
                // Start game if ready
                if self.is_ready_to_start() {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_CLICK.to_string(),
                    ));
                    self.pending_events.push(UIEvent::StartGame {
                        mode: GameMode::Skirmish,
                        faction: format!("{:?}", self.selected_faction.unwrap()),
                        map: self.selected_map.clone(),
                    });
                    return true;
                }
                false
            }
            KeyCode::Key1 => {
                // Select USA
                self.selected_faction = Some(Faction::USA);
                self.setup_general_buttons(Faction::USA);
                true
            }
            KeyCode::Key2 => {
                // Select China
                self.selected_faction = Some(Faction::China);
                self.setup_general_buttons(Faction::China);
                true
            }
            KeyCode::Key3 => {
                // Select GLA
                self.selected_faction = Some(Faction::GLA);
                self.setup_general_buttons(Faction::GLA);
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for FactionSelectionScreen {
    fn render(&self, _context: &mut UIRenderContext) {
        println!(
            "{}",
            localization::localize("faction_selection.log.header", "=== Faction Selection ===")
        );

        println!(
            "{}",
            localization::localize(
                "faction_selection.log.subheader",
                "Choose Your Faction and General"
            )
        );

        for button in &self.faction_buttons {
            let state = if button.selected {
                localization::localize("ui.button_state.selected", "SELECTED")
            } else if button.hovered {
                localization::localize("ui.button_state.hovered", "HOVERED")
            } else {
                localization::localize("ui.button_state.normal", "NORMAL")
            };

            let faction_name = button.faction.get_name();
            let scale = button.click_scale();
            let (x, y, _, _) = utils::scale_rect_center(
                (
                    button.position.0,
                    button.position.1,
                    button.size.0,
                    button.size.1,
                ),
                scale,
            );
            let x_str = format!("{:.1}", x);
            let y_str = format!("{:.1}", y);
            println!(
                "{}",
                localization::localize_with_args(
                    "faction_selection.log.faction_button",
                    "Faction: {name} [{state}] at ({x}, {y})",
                    &[
                        ("name", faction_name.as_str()),
                        ("state", state.as_str()),
                        ("x", x_str.as_str()),
                        ("y", y_str.as_str()),
                    ],
                )
            );

            if button.hovered || button.selected {
                let desc = button.faction.get_description();
                println!(
                    "{}",
                    localization::localize_with_args(
                        "faction_selection.log.faction_description",
                        "  Description: {text}",
                        &[("text", desc.as_str())],
                    )
                );
            }
        }

        if self.selected_faction.is_some() {
            println!(
                "\n{}",
                localization::localize("faction_selection.log.select_general", "Select General:")
            );
            for button in &self.general_buttons {
                let state = if button.selected {
                    localization::localize("ui.button_state.selected", "SELECTED")
                } else if button.hovered {
                    localization::localize("ui.button_state.hovered", "HOVERED")
                } else if button.enabled {
                    localization::localize("ui.button_state.enabled", "ENABLED")
                } else {
                    localization::localize("ui.button_state.disabled", "DISABLED")
                };

                let general_name = button.general.get_name();
                let scale = button.click_scale();
                let (x, y, _, _) = utils::scale_rect_center(
                    (
                        button.position.0,
                        button.position.1,
                        button.size.0,
                        button.size.1,
                    ),
                    scale,
                );
                let x_value = format!("{:.1}", x);
                let y_value = format!("{:.1}", y);
                println!(
                    "{}",
                    localization::localize_with_args(
                        "faction_selection.log.general_button",
                        "General: {name} [{state}] at ({x}, {y})",
                        &[
                            ("name", general_name.as_str()),
                            ("state", state.as_str()),
                            ("x", x_value.as_str()),
                            ("y", y_value.as_str()),
                        ],
                    )
                );

                if button.hovered || button.selected {
                    let desc = button.general.get_description();
                    println!(
                        "  {}",
                        localization::localize_with_args(
                            "faction_selection.log.general_description",
                            "{description}",
                            &[("description", desc.as_str())],
                        )
                    );
                }
            }
        }

        let map_name = localized_map_name(&self.selected_map);
        println!(
            "\n{}",
            localization::localize_with_args(
                "faction_selection.log.selected_map",
                "Selected Map: {map}",
                &[("map", map_name.as_str())],
            )
        );
        let ai_value = self.ai_difficulty.to_string();
        println!(
            "{}",
            localization::localize_with_args(
                "faction_selection.log.ai_difficulty",
                "AI Difficulty: {value}/5",
                &[("value", ai_value.as_str())],
            )
        );

        let start_rect = self.get_start_button_rect();
        let back_rect = self.get_back_button_rect();

        if self.is_ready_to_start() {
            let (start_x, start_y, _, _) =
                utils::scale_rect_center(start_rect, self.start_click.scale());
            let start_x_value = format!("{:.1}", start_x);
            let start_y_value = format!("{:.1}", start_y);
            println!(
                "{}",
                localization::localize_with_args(
                    "faction_selection.log.start_button",
                    "Start Game button at ({x}, {y})",
                    &[("x", start_x_value.as_str()), ("y", start_y_value.as_str()),],
                )
            );
        } else {
            println!(
                "{}",
                localization::localize(
                    "faction_selection.log.start_disabled",
                    "Start Game button disabled"
                )
            );
        }
        let (back_x, back_y, _, _) = utils::scale_rect_center(back_rect, self.back_click.scale());
        let back_x_value = format!("{:.1}", back_x);
        let back_y_value = format!("{:.1}", back_y);
        println!(
            "{}",
            localization::localize_with_args(
                "faction_selection.log.back_button",
                "Back button at ({x}, {y})",
                &[("x", back_x_value.as_str()), ("y", back_y_value.as_str()),],
            )
        );

        if let (Some(faction), Some(general)) = (self.selected_faction, self.selected_general) {
            let faction_name = faction.get_name();
            let general_name = general.get_name();
            println!(
                "\n{}",
                localization::localize_with_args(
                    "faction_selection.log.ready_message",
                    ">>> Ready to start as {faction} {general} <<<",
                    &[
                        ("faction", faction_name.as_str()),
                        ("general", general_name.as_str()),
                    ],
                )
            );
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

fn slugify_map_label(name: &str) -> String {
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

fn localized_map_name(name: &str) -> String {
    let key = format!("map.name.{}", slugify_map_label(name));
    localization::localize(&key, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localization;

    #[test]
    fn test_faction_properties() {
        assert_eq!(
            Faction::USA.get_name(),
            localization::localize("faction.usa.name", "United States")
        );
        assert_eq!(
            Faction::China.get_name(),
            localization::localize("faction.china.name", "People's Republic of China")
        );
        assert_eq!(
            Faction::GLA.get_name(),
            localization::localize("faction.gla.name", "Global Liberation Army")
        );
    }

    #[test]
    fn test_general_factions() {
        assert_eq!(General::USAAirForce.get_faction(), Faction::USA);
        assert_eq!(General::ChinaTank.get_faction(), Faction::China);
        assert_eq!(General::GLAToxin.get_faction(), Faction::GLA);
    }

    #[test]
    fn test_faction_selection_screen() {
        let screen = FactionSelectionScreen::new();
        assert_eq!(screen.selected_faction, None);
        assert_eq!(screen.selected_general, None);
        assert!(!screen.is_ready_to_start());
    }

    #[test]
    fn test_ready_to_start() {
        let mut screen = FactionSelectionScreen::new();
        assert!(!screen.is_ready_to_start());

        screen.selected_faction = Some(Faction::USA);
        assert!(!screen.is_ready_to_start());

        screen.selected_general = Some(General::USAAirForce);
        assert!(screen.is_ready_to_start());
    }
}
