//! Victory/Defeat Screen
//!
//! This module implements the end-game screen showing victory or defeat results.

use super::{Interactive, KeyCode, MouseButton, Renderable, UIEvent, UIRenderContext};
use crate::{
    game_logic::{
        victory::{format_duration, PlayerOutcome, PlayerResult, VictorySummary},
        Team,
    },
    localization,
};

/// Victory screen type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryScreenType {
    Victory(u32), // Winner player ID
    Defeat,
    Draw,
}

/// Victory/defeat screen
pub struct VictoryScreen {
    screen_type: Option<VictoryScreenType>,
    visible: bool,
    summary: Option<VictorySummary>,
}

impl Default for VictoryScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl VictoryScreen {
    pub fn new() -> Self {
        Self {
            screen_type: None,
            visible: false,
            summary: None,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn update(&mut self, _delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {}

    pub fn set_victory(&mut self, player_id: u32) {
        self.screen_type = Some(VictoryScreenType::Victory(player_id));
        self.visible = true;
    }

    pub fn set_defeat(&mut self) {
        self.screen_type = Some(VictoryScreenType::Defeat);
        self.visible = true;
    }

    pub fn set_draw(&mut self) {
        self.screen_type = Some(VictoryScreenType::Draw);
        self.visible = true;
    }

    pub fn set_summary(&mut self, summary: Option<VictorySummary>) {
        self.summary = summary;
    }

    pub fn clear(&mut self) {
        self.screen_type = None;
        self.summary = None;
        self.visible = false;
    }

    pub fn handle_mouse_click(
        &mut self,
        _x: i32,
        _y: i32,
        _button: MouseButton,
    ) -> Option<UIEvent> {
        // Any click dismisses score screen residual (C++ continue).
        if self.screen_type.is_some() {
            Some(UIEvent::ExitToMenu)
        } else {
            None
        }
    }

    /// Key continue residual (Enter/Esc/Space).
    pub fn handle_continue_key(&mut self, key: KeyCode) -> Option<UIEvent> {
        if self.screen_type.is_some()
            && matches!(key, KeyCode::Enter | KeyCode::Escape | KeyCode::Space)
        {
            Some(UIEvent::ExitToMenu)
        } else {
            None
        }
    }
}

impl Interactive for VictoryScreen {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        VictoryScreen::handle_mouse_click(self, x, y, button).is_some()
    }
    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        self.handle_continue_key(key).is_some()
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for VictoryScreen {
    fn render(&self, _context: &mut UIRenderContext) {
        if let Some(screen_type) = &self.screen_type {
            match screen_type {
                VictoryScreenType::Victory(player_id) => {
                    println!(
                        "=== {} ===",
                        localization::localize("victory.title", "VICTORY")
                    );
                    let player_label = localization::localize("victory.player_label", "Player");
                    let wins_suffix = localization::localize("victory.wins_suffix", "wins!");
                    println!("{player_label} {} {wins_suffix}", player_id);
                }
                VictoryScreenType::Defeat => {
                    println!(
                        "=== {} ===",
                        localization::localize("victory.defeat_title", "DEFEAT")
                    );
                    println!(
                        "{}",
                        localization::localize("victory.mission_failed", "Mission failed")
                    );
                }
                VictoryScreenType::Draw => {
                    println!("=== {} ===", localization::localize("victory.draw", "DRAW"));
                    println!(
                        "{}",
                        localization::localize("victory.no_winner", "No clear winner")
                    );
                }
            }

            if let Some(summary) = &self.summary {
                render_summary(summary);
            }
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, 1024, 768)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

fn render_summary(summary: &VictorySummary) {
    println!(
        "{}",
        localization::localize("victory.summary.header", "--- Summary ---")
    );

    if let Some(mission) = &summary.mission_name {
        println!(
            "{} {}",
            localization::localize("victory.summary.mission", "Mission:"),
            mission
        );
    }

    if let Some(duration) = summary.duration {
        println!(
            "{} {}",
            localization::localize("victory.summary.duration", "Duration:"),
            format_duration(duration)
        );
    }

    if summary.player_results.is_empty() {
        println!(
            "{}",
            localization::localize("victory.summary.no_data", "No player statistics recorded.")
        );
        return;
    }

    print_outcome_section(summary);

    let unit_columns: Vec<(&str, &str, StatAccessor)> = vec![
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
    let structure_columns: Vec<(&str, &str, StatAccessor)> = vec![
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
    let resource_columns: Vec<(&str, &str, StatAccessor)> = vec![
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

    print_numeric_section(
        summary,
        "victory.summary.section.units",
        "Units",
        &unit_columns,
    );
    print_numeric_section(
        summary,
        "victory.summary.section.structures",
        "Structures",
        &structure_columns,
    );
    print_numeric_section(
        summary,
        "victory.summary.section.resources",
        "Resources",
        &resource_columns,
    );
}

type StatAccessor = fn(&PlayerResult) -> u32;

fn print_numeric_section(
    summary: &VictorySummary,
    heading_key: &str,
    heading_fallback: &str,
    columns: &[(&str, &str, StatAccessor)],
) {
    println!(
        "\n{}",
        localization::localize(heading_key, heading_fallback)
    );
    print!(
        "{:<15}",
        localization::localize("victory.summary.column.player", "Player")
    );
    for (col_key, fallback, _) in columns {
        print!(" {:>12}", localization::localize(col_key, fallback));
    }
    println!();

    for result in &summary.player_results {
        print!("{:<15}", result.player_name);
        for (_, _, accessor) in columns {
            print!(" {:>12}", accessor(result));
        }
        println!();
    }
}

fn print_outcome_section(summary: &VictorySummary) {
    println!(
        "{}",
        localization::localize("victory.summary.section.outcome", "Battle Outcome")
    );
    println!(
        "{:<15} {:<15} {:<10}",
        localization::localize("victory.summary.column.player", "Player"),
        localization::localize("victory.summary.column.faction", "Faction"),
        localization::localize("victory.summary.column.outcome", "Outcome")
    );
    for result in &summary.player_results {
        println!(
            "{:<15} {:<15} {:<10}",
            result.player_name,
            localized_team_name(result.faction),
            outcome_label(result.outcome)
        );
    }
    println!();
}

fn outcome_label(outcome: PlayerOutcome) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{KeyCode, MouseButton, UIEvent};

    #[test]
    fn continue_emits_exit_to_menu_residual() {
        let mut vs = VictoryScreen::new();
        assert!(vs.handle_mouse_click(0, 0, MouseButton::Left).is_none());
        vs.set_defeat();
        assert!(matches!(
            vs.handle_mouse_click(0, 0, MouseButton::Left),
            Some(UIEvent::ExitToMenu)
        ));
        vs.set_victory(0);
        assert!(matches!(
            vs.handle_continue_key(KeyCode::Enter),
            Some(UIEvent::ExitToMenu)
        ));
    }
}
