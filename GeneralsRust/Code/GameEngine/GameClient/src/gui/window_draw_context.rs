//! Lexical WND draw services for one actual receiving UI device and tree.
//! A missing renderer is an explicit headless capability; it never selects a
//! different client's device. The manager loan is installed by draw_all for
//! immutable traversal and ends before transition state is mutably visited.
use super::{UIRenderer, window_manager::WindowManager};
use crate::display::display_fx::DisplayDrawFacts;

pub struct WindowDrawContext<'a> {
    pub shell_scheme: Option<&'a crate::gui::shell::ShellMenuSchemeManager>,
    pub renderer: Option<&'a mut UIRenderer>,
    pub display: DisplayDrawFacts,
    pub ime: Option<&'a crate::gui::ime_manager::ImeManager>,
    pub manager: Option<&'a WindowManager>,
    /// Explicit owner-derived Energy facts for a hosted PowerWindow draw.
    /// The outer `Some` marks this callback as receiving-context work; an
    /// inner `None` means that owner has no selected/local player. Standalone
    /// Core callbacks leave this unset and keep their compatibility lookup.
    pub receiving_power: Option<Option<ReceivingPowerObservation>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReceivingPowerObservation {
    pub production: i32,
    pub consumption: i32,
    pub rules: game_engine::common::ini::ini_game_data::PowerBarRules,
}
impl<'a> WindowDrawContext<'a> {
    pub fn new(renderer: Option<&'a mut UIRenderer>, display: DisplayDrawFacts) -> Self {
        Self {
            shell_scheme: None,
            renderer,
            display,
            manager: None,
            ime: None,
            receiving_power: None,
        }
    }
    pub fn with_renderer<R>(
        &mut self,
        f: impl FnOnce(&mut UIRenderer, &DisplayDrawFacts) -> R,
    ) -> Option<R> {
        self.renderer
            .as_deref_mut()
            .map(|renderer| f(renderer, &self.display))
    }
}
impl Default for WindowDrawContext<'_> {
    fn default() -> Self {
        Self::new(None, DisplayDrawFacts::default())
    }
}
/// Missing tree is allowed for standalone/headless leaf calls. Production
/// draw_all supplies its actual shared manager loan, never an active slot.
pub fn with_draw_manager<R: Default>(
    manager: Option<&WindowManager>,
    f: impl FnOnce(&WindowManager) -> R,
) -> R {
    manager.map(f).unwrap_or_default()
}
