//! In-Game Diplomacy Panel
//!
//! A popup overlay that shows all players and allows changing diplomatic
//! relationships (Allied / Neutral / Enemy) and muting individual players.
//! Triggered by a keyboard shortcut (default: Ctrl+D) or a UI button.
//!
//! The panel integrates with the existing `DiplomaticRelationship` type from
//! `GameClient/gui/callbacks/diplomacy.rs`.

use super::{
    layout, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, UIRenderContext,
};
use crate::localization;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Diplomatic relationship for display purposes (mirrors the GameClient type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomacyRelation {
    Allied,
    Neutral,
    Enemy,
}

impl Default for DiplomacyRelation {
    fn default() -> Self {
        Self::Neutral
    }
}

impl std::fmt::Display for DiplomacyRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiplomacyRelation::Allied => write!(f, "Allied"),
            DiplomacyRelation::Neutral => write!(f, "Neutral"),
            DiplomacyRelation::Enemy => write!(f, "Enemy"),
        }
    }
}

/// Player status in the diplomacy view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomacyPlayerStatus {
    Active,
    Defeated,
    Disconnected,
    Observer,
}

impl std::fmt::Display for DiplomacyPlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiplomacyPlayerStatus::Active => write!(f, "Active"),
            DiplomacyPlayerStatus::Defeated => write!(f, "Defeated"),
            DiplomacyPlayerStatus::Disconnected => write!(f, "Disconnected"),
            DiplomacyPlayerStatus::Observer => write!(f, "Observer"),
        }
    }
}

/// Information about a single player row in the diplomacy panel.
#[derive(Debug, Clone)]
pub struct DiplomacyPlayerEntry {
    pub player_id: i32,
    pub name: String,
    pub side: String,
    pub team: i32,
    pub status: DiplomacyPlayerStatus,
    pub relationship: DiplomacyRelation,
    pub is_muted: bool,
}

impl Default for DiplomacyPlayerEntry {
    fn default() -> Self {
        Self {
            player_id: -1,
            name: String::new(),
            side: String::new(),
            team: -1,
            status: DiplomacyPlayerStatus::Observer,
            relationship: DiplomacyRelation::Neutral,
            is_muted: false,
        }
    }
}

/// Events emitted by the diplomacy panel.
#[derive(Debug, Clone)]
pub enum DiplomacyEvent {
    /// The local player changed a relationship.
    RelationshipChanged {
        target_player_id: i32,
        new_relationship: DiplomacyRelation,
    },
    /// The local player muted/unmuted a player.
    MuteToggled { target_player_id: i32, muted: bool },
    /// Panel was opened.
    Opened,
    /// Panel was closed.
    Closed,
}

// ---------------------------------------------------------------------------
// Per-row button layout
// ---------------------------------------------------------------------------

/// A clickable region inside a player row.
#[derive(Debug, Clone)]
struct DiplomacyButton {
    /// Which player row this button belongs to.
    player_id: i32,
    /// Semantic meaning of the button.
    kind: DiplomacyButtonKind,
    /// Screen-space hit rectangle.
    rect: (i32, i32, u32, u32),
    hovered: bool,
    click_spring: ClickSpring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiplomacyButtonKind {
    Allied,
    Neutral,
    Enemy,
    Mute,
    Unmute,
}

// ---------------------------------------------------------------------------
// Diplomacy panel
// ---------------------------------------------------------------------------

const MAX_PLAYER_ROWS: usize = 8;
const ROW_HEIGHT: u32 = 32;
const HEADER_HEIGHT: u32 = 36;
const BUTTON_SIZE: u32 = 60;
const BUTTON_SPACING: u32 = 4;
const MUTE_BUTTON_WIDTH: u32 = 50;
const PANEL_WIDTH: u32 = 560;
const PANEL_PADDING: i32 = 10;

/// The diplomacy popup panel.
pub struct DiplomacyPanel {
    /// Whether the panel is currently shown.
    active: bool,
    /// Screen dimensions.
    screen_size: (u32, u32),
    /// Local player ID (row is not interactive for the local player).
    local_player_id: i32,
    /// Player entries displayed in the panel.
    players: Vec<DiplomacyPlayerEntry>,
    /// Clickable button regions (rebuilt each frame / on layout change).
    buttons: Vec<DiplomacyButton>,
    /// Pending events for the owner to drain.
    pending_events: Vec<DiplomacyEvent>,
    /// Overlay animation progress (0..1).
    animation_progress: f32,
    /// Overlay alpha (0..0.7).
    overlay_alpha: f32,
}

impl Default for DiplomacyPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DiplomacyPanel {
    /// Create a new diplomacy panel.
    pub fn new() -> Self {
        Self {
            active: false,
            screen_size: (1024, 768),
            local_player_id: 0,
            players: Vec::new(),
            buttons: Vec::new(),
            pending_events: Vec::new(),
            animation_progress: 0.0,
            overlay_alpha: 0.0,
        }
    }

    // -- public query -------------------------------------------------------

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn players(&self) -> &[DiplomacyPlayerEntry] {
        &self.players
    }

    // -- events -------------------------------------------------------------

    pub fn drain_events(&mut self) -> Vec<DiplomacyEvent> {
        std::mem::take(&mut self.pending_events)
    }

    // -- mutation -----------------------------------------------------------

    /// Open the diplomacy panel.
    pub fn open(&mut self) {
        if self.active {
            return;
        }
        self.active = true;
        self.animation_progress = 0.0;
        self.overlay_alpha = 0.0;
        self.rebuild_buttons();
        self.pending_events.push(DiplomacyEvent::Opened);
    }

    /// Close the diplomacy panel.
    pub fn close(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.buttons.clear();
        self.pending_events.push(DiplomacyEvent::Closed);
    }

    /// Toggle open/closed.
    pub fn toggle(&mut self) {
        if self.active {
            self.close();
        } else {
            self.open();
        }
    }

    /// Set the local player ID.
    pub fn set_local_player_id(&mut self, id: i32) {
        self.local_player_id = id;
    }

    /// Replace all player entries.
    pub fn set_players(&mut self, players: Vec<DiplomacyPlayerEntry>) {
        self.players = players;
        if self.active {
            self.rebuild_buttons();
        }
    }

    /// Update a single player entry.
    pub fn update_player(&mut self, player_id: i32, entry: DiplomacyPlayerEntry) {
        if let Some(existing) = self.players.iter_mut().find(|p| p.player_id == player_id) {
            *existing = entry;
        } else if self.players.len() < MAX_PLAYER_ROWS {
            self.players.push(entry);
        }
        if self.active {
            self.rebuild_buttons();
        }
    }

    /// Resize the panel (call on window resize).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        if self.active {
            self.rebuild_buttons();
        }
    }

    /// Advance per-frame state.
    pub fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }
        // Animate in
        if self.animation_progress < 1.0 {
            self.animation_progress = (self.animation_progress + dt * 4.0).min(1.0);
        }
        if self.overlay_alpha < 0.7 {
            self.overlay_alpha = (self.overlay_alpha + dt * 3.0).min(0.7);
        }
        // Update click springs
        for btn in &mut self.buttons {
            btn.click_spring.update(dt);
        }
    }

    // -- layout -------------------------------------------------------------

    /// Bounding rectangle for the panel content area.
    pub fn panel_rect(&self) -> (i32, i32, u32, u32) {
        let total_height = HEADER_HEIGHT as usize
            + self.players.len() * ROW_HEIGHT as usize
            + PANEL_PADDING as usize * 2;
        let w = PANEL_WIDTH;
        let h = total_height as u32;
        let x = (self.screen_size.0 as i32 - w as i32) / 2;
        let y = (self.screen_size.1 as i32 - h as i32) / 2;
        (x, y, w, h)
    }

    fn rebuild_buttons(&mut self) {
        self.buttons.clear();
        let (px, py, _pw, _ph) = self.panel_rect();

        let col_start_x = px + PANEL_PADDING + 260i32; // after name/side/team/status columns
        let row_y_start = py + PANEL_PADDING as i32 + HEADER_HEIGHT as i32;

        for (row_idx, player) in self.players.iter().enumerate() {
            if player.player_id == self.local_player_id {
                continue; // Cannot change own relationship
            }
            let row_y = row_y_start + row_idx as i32 * ROW_HEIGHT as i32;
            let rel_x = col_start_x;

            // Allied button
            self.buttons.push(DiplomacyButton {
                player_id: player.player_id,
                kind: DiplomacyButtonKind::Allied,
                rect: (rel_x, row_y + 2, BUTTON_SIZE, ROW_HEIGHT - 4),
                hovered: false,
                click_spring: ClickSpring::new(),
            });

            // Neutral button
            self.buttons.push(DiplomacyButton {
                player_id: player.player_id,
                kind: DiplomacyButtonKind::Neutral,
                rect: (
                    rel_x + (BUTTON_SIZE + BUTTON_SPACING) as i32,
                    row_y + 2,
                    BUTTON_SIZE,
                    ROW_HEIGHT - 4,
                ),
                hovered: false,
                click_spring: ClickSpring::new(),
            });

            // Enemy button
            self.buttons.push(DiplomacyButton {
                player_id: player.player_id,
                kind: DiplomacyButtonKind::Enemy,
                rect: (
                    rel_x + 2 * (BUTTON_SIZE + BUTTON_SPACING) as i32,
                    row_y + 2,
                    BUTTON_SIZE,
                    ROW_HEIGHT - 4,
                ),
                hovered: false,
                click_spring: ClickSpring::new(),
            });

            // Mute / Unmute button
            self.buttons.push(DiplomacyButton {
                player_id: player.player_id,
                kind: if player.is_muted {
                    DiplomacyButtonKind::Unmute
                } else {
                    DiplomacyButtonKind::Mute
                },
                rect: (
                    rel_x + 3 * (BUTTON_SIZE + BUTTON_SPACING) as i32,
                    row_y + 2,
                    MUTE_BUTTON_WIDTH,
                    ROW_HEIGHT - 4,
                ),
                hovered: false,
                click_spring: ClickSpring::new(),
            });
        }
    }

    // -- click handling -----------------------------------------------------

    fn handle_button_click(&mut self, player_id: i32, kind: DiplomacyButtonKind) {
        match kind {
            DiplomacyButtonKind::Allied => {
                self.pending_events
                    .push(DiplomacyEvent::RelationshipChanged {
                        target_player_id: player_id,
                        new_relationship: DiplomacyRelation::Allied,
                    });
            }
            DiplomacyButtonKind::Neutral => {
                self.pending_events
                    .push(DiplomacyEvent::RelationshipChanged {
                        target_player_id: player_id,
                        new_relationship: DiplomacyRelation::Neutral,
                    });
            }
            DiplomacyButtonKind::Enemy => {
                self.pending_events
                    .push(DiplomacyEvent::RelationshipChanged {
                        target_player_id: player_id,
                        new_relationship: DiplomacyRelation::Enemy,
                    });
            }
            DiplomacyButtonKind::Mute => {
                self.pending_events.push(DiplomacyEvent::MuteToggled {
                    target_player_id: player_id,
                    muted: true,
                });
            }
            DiplomacyButtonKind::Unmute => {
                self.pending_events.push(DiplomacyEvent::MuteToggled {
                    target_player_id: player_id,
                    muted: false,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Interactive trait
// ---------------------------------------------------------------------------

impl Interactive for DiplomacyPanel {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        if !self.active {
            return false;
        }
        let mut handled = false;
        for btn in &mut self.buttons {
            let was_hovered = btn.hovered;
            btn.hovered = utils::point_in_rect((x, y), btn.rect);
            if btn.hovered {
                handled = true;
            }
            // Could emit hover sound on transition
            let _ = was_hovered;
        }
        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        if !self.active || button != MouseButton::Left {
            return false;
        }
        // Find the index of the clicked button to avoid borrow issues
        let clicked_idx = self
            .buttons
            .iter()
            .position(|btn| utils::point_in_rect((x, y), btn.rect));
        if let Some(idx) = clicked_idx {
            let (player_id, kind) = {
                let btn = &self.buttons[idx];
                (btn.player_id, btn.kind)
            };
            self.buttons[idx].click_spring.trigger();
            self.handle_button_click(player_id, kind);
            return true;
        }
        false
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        if !self.active {
            return false;
        }
        match key {
            KeyCode::Escape => {
                self.close();
                true
            }
            KeyCode::D => {
                // Ctrl+D toggles (handled outside), plain D is not consumed.
                false
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Renderable trait
// ---------------------------------------------------------------------------

impl Renderable for DiplomacyPanel {
    fn render(&self, _context: &mut UIRenderContext) {
        if !self.active {
            return;
        }

        let title = localization::localize("diplomacy.title", "=== Diplomacy ===");
        println!("{title}");

        let alpha_str = format!("{:.2}", self.overlay_alpha);
        let overlay_text = localization::localize_with_args(
            "diplomacy.overlay",
            "Background overlay (alpha: {alpha})",
            &[("alpha", &alpha_str)],
        );
        println!("{overlay_text}");

        let (px, py, pw, ph) = self.panel_rect();
        let pos_text = localization::localize_with_args(
            "diplomacy.panel_position",
            "Panel at ({x}, {y}) size {w}x{h}",
            &[
                ("x", &px.to_string()),
                ("y", &py.to_string()),
                ("w", &pw.to_string()),
                ("h", &ph.to_string()),
            ],
        );
        println!("{pos_text}");

        // Column headers
        let header = localization::localize(
            "diplomacy.header",
            "  Player           Side        Team  Status     Relation    Mute",
        );
        println!("{header}");

        let you_label = localization::localize("diplomacy.you_marker", "(You)");
        let hover_label = localization::localize("diplomacy.hovered", "[HOVERED]");

        for player in &self.players {
            let is_self = player.player_id == self.local_player_id;
            let self_marker = if is_self {
                format!(" {you_label}")
            } else {
                String::new()
            };

            let rel_display = match player.relationship {
                DiplomacyRelation::Allied => "ALLIED ",
                DiplomacyRelation::Neutral => "NEUTRAL",
                DiplomacyRelation::Enemy => "ENEMY  ",
            };

            let mute_display = if player.is_muted { "Muted" } else { "-" };

            let row = format!(
                "  {:16} {:12} {:4}  {:10} {}  {}{}",
                player.name,
                player.side,
                player.team,
                player.status,
                rel_display,
                mute_display,
                self_marker,
            );
            println!("{row}");

            // Show which buttons are hovered
            for btn in &self.buttons {
                if btn.player_id == player.player_id && btn.hovered {
                    let kind_str = match btn.kind {
                        DiplomacyButtonKind::Allied => "Set Allied",
                        DiplomacyButtonKind::Neutral => "Set Neutral",
                        DiplomacyButtonKind::Enemy => "Set Enemy",
                        DiplomacyButtonKind::Mute => "Mute Player",
                        DiplomacyButtonKind::Unmute => "Unmute Player",
                    };
                    println!("    -> {hover_label} {kind_str}");
                }
            }
        }

        if self.players.is_empty() {
            let no_players =
                localization::localize("diplomacy.no_players", "  (No players in game)");
            println!("{no_players}");
        }

        // Controls hint
        println!();
        println!(
            "{}",
            localization::localize("diplomacy.controls", "Controls:")
        );
        println!(
            "{}",
            localization::localize("diplomacy.controls_esc", "Esc - Close diplomacy panel")
        );
        println!(
            "{}",
            localization::localize(
                "diplomacy.controls_hint",
                "Click a relation button to change diplomacy"
            )
        );
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        if self.active {
            self.panel_rect()
        } else {
            (0, 0, 0, 0)
        }
    }

    fn is_visible(&self) -> bool {
        self.active
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_player(id: i32, name: &str, side: &str) -> DiplomacyPlayerEntry {
        DiplomacyPlayerEntry {
            player_id: id,
            name: name.to_string(),
            side: side.to_string(),
            team: if id == 0 { 1 } else { 2 },
            status: DiplomacyPlayerStatus::Active,
            relationship: DiplomacyRelation::Neutral,
            is_muted: false,
        }
    }

    #[test]
    fn test_panel_creation() {
        let panel = DiplomacyPanel::new();
        assert!(!panel.is_active());
        assert!(panel.players().is_empty());
    }

    #[test]
    fn test_open_close_toggle() {
        let mut panel = DiplomacyPanel::new();

        panel.open();
        assert!(panel.is_active());

        panel.open(); // no-op
        assert!(panel.is_active());

        panel.close();
        assert!(!panel.is_active());

        panel.close(); // no-op
        assert!(!panel.is_active());

        panel.toggle();
        assert!(panel.is_active());
        panel.toggle();
        assert!(!panel.is_active());
    }

    #[test]
    fn test_set_players() {
        let mut panel = DiplomacyPanel::new();
        panel.set_local_player_id(0);

        let players = vec![
            sample_player(0, "Player 1", "USA"),
            sample_player(1, "Player 2", "China"),
            sample_player(2, "Player 3", "GLA"),
        ];
        panel.set_players(players);

        assert_eq!(panel.players().len(), 3);
        assert_eq!(panel.players()[0].name, "Player 1");
    }

    #[test]
    fn test_update_player() {
        let mut panel = DiplomacyPanel::new();
        panel.set_local_player_id(0);
        panel.set_players(vec![sample_player(0, "P1", "USA")]);

        let mut updated = sample_player(1, "P2", "China");
        updated.relationship = DiplomacyRelation::Allied;
        panel.update_player(1, updated);

        assert_eq!(panel.players().len(), 2);
        assert_eq!(panel.players()[1].relationship, DiplomacyRelation::Allied);
    }

    #[test]
    fn test_button_generation() {
        let mut panel = DiplomacyPanel::new();
        panel.set_local_player_id(0);
        panel.open();

        panel.set_players(vec![
            sample_player(0, "P1", "USA"),
            sample_player(1, "P2", "China"),
        ]);

        // Player 0 is the local player, so no buttons for them.
        // Player 1 should have 4 buttons (Allied, Neutral, Enemy, Mute/Unmute).
        let p1_buttons: Vec<_> = panel.buttons.iter().filter(|b| b.player_id == 1).collect();
        assert_eq!(p1_buttons.len(), 4);
    }

    #[test]
    fn test_relationship_change_event() {
        let mut panel = DiplomacyPanel::new();
        panel.set_local_player_id(0);
        panel.open();
        panel.set_players(vec![
            sample_player(0, "P1", "USA"),
            sample_player(1, "P2", "China"),
        ]);

        // Simulate clicking the Allied button for player 1
        let allied_btn = panel
            .buttons
            .iter()
            .find(|b| b.player_id == 1 && b.kind == DiplomacyButtonKind::Allied)
            .unwrap();
        panel.handle_button_click(allied_btn.player_id, allied_btn.kind);

        let events = panel.drain_events();
        // Opened + RelationshipChanged
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[1],
            DiplomacyEvent::RelationshipChanged {
                target_player_id: 1,
                new_relationship: DiplomacyRelation::Allied,
            }
        ));
    }

    #[test]
    fn test_mute_toggle_event() {
        let mut panel = DiplomacyPanel::new();
        panel.set_local_player_id(0);
        panel.open();
        panel.set_players(vec![
            sample_player(0, "P1", "USA"),
            sample_player(1, "P2", "China"),
        ]);

        let mute_btn = panel
            .buttons
            .iter()
            .find(|b| b.player_id == 1 && b.kind == DiplomacyButtonKind::Mute)
            .unwrap();
        panel.handle_button_click(mute_btn.player_id, mute_btn.kind);

        let events = panel.drain_events();
        assert!(matches!(
            &events[1],
            DiplomacyEvent::MuteToggled {
                target_player_id: 1,
                muted: true,
            }
        ));
    }

    #[test]
    fn test_key_press_escape() {
        let mut panel = DiplomacyPanel::new();
        panel.open();
        assert!(panel.handle_key_press(KeyCode::Escape));
        assert!(!panel.is_active());
    }

    #[test]
    fn test_key_press_ignored_when_closed() {
        let mut panel = DiplomacyPanel::new();
        assert!(!panel.handle_key_press(KeyCode::Escape));
    }

    #[test]
    fn test_max_players() {
        let mut panel = DiplomacyPanel::new();
        let many_players: Vec<DiplomacyPlayerEntry> = (0..10)
            .map(|i| sample_player(i, &format!("P{}", i), "USA"))
            .collect();
        panel.set_players(many_players);
        assert_eq!(panel.players().len(), MAX_PLAYER_ROWS);
    }

    #[test]
    fn test_animation_progress() {
        let mut panel = DiplomacyPanel::new();
        panel.open();
        assert_eq!(panel.animation_progress, 0.0);

        panel.update(0.1);
        assert!(panel.animation_progress > 0.0);
        assert!(panel.animation_progress < 1.0);

        // Should reach 1.0 after enough updates
        for _ in 0..100 {
            panel.update(0.1);
        }
        assert_eq!(panel.animation_progress, 1.0);
    }

    #[test]
    fn test_resize() {
        let mut panel = DiplomacyPanel::new();
        panel.open();
        panel.resize(1920, 1080);
        let (x, _y, w, _h) = panel.panel_rect();
        assert_eq!(w, PANEL_WIDTH);
        // Should be centered
        assert!(x > 0);
    }

    #[test]
    fn test_panel_rect() {
        let panel = DiplomacyPanel::new();
        let (_x, _y, w, _h) = panel.panel_rect();
        assert_eq!(w, PANEL_WIDTH);
    }

    #[test]
    fn test_diplomacy_relation_display() {
        assert_eq!(format!("{}", DiplomacyRelation::Allied), "Allied");
        assert_eq!(format!("{}", DiplomacyRelation::Neutral), "Neutral");
        assert_eq!(format!("{}", DiplomacyRelation::Enemy), "Enemy");
    }

    #[test]
    fn test_player_status_display() {
        assert_eq!(format!("{}", DiplomacyPlayerStatus::Active), "Active");
        assert_eq!(format!("{}", DiplomacyPlayerStatus::Defeated), "Defeated");
        assert_eq!(
            format!("{}", DiplomacyPlayerStatus::Disconnected),
            "Disconnected"
        );
        assert_eq!(format!("{}", DiplomacyPlayerStatus::Observer), "Observer");
    }
}
