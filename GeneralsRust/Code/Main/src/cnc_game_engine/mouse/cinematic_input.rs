//! Source DisableInput/EnableInput operates on these actual receiving fields.
//! The loan lives only for the synchronous script walk; no state is published.
use super::*;

pub(in crate::cnc_game_engine) struct CinematicInputLoan<'a> {
    pub(in crate::cnc_game_engine) local_player: u32,
    pub(in crate::cnc_game_engine) modes: &'a mut LookAtHostModes,
    pub(in crate::cnc_game_engine) selected: &'a mut Vec<crate::game_logic::ObjectId>,
    pub(in crate::cnc_game_engine) selected_receipt:
        &'a mut Option<Vec<crate::game_logic::ObjectId>>,
    pub(in crate::cnc_game_engine) presentation:
        &'a mut Option<std::rc::Rc<crate::presentation_frame::PresentationFrame>>,
    pub(in crate::cnc_game_engine) rotating: &'a mut bool,
    pub(in crate::cnc_game_engine) rotate_anchor: &'a mut Option<(f32, f32)>,
    pub(in crate::cnc_game_engine) scrolling: &'a mut bool,
    pub(in crate::cnc_game_engine) scroll_anchor: &'a mut Option<(f32, f32)>,
    pub(in crate::cnc_game_engine) held: [&'a mut bool; 4],
    pub(in crate::cnc_game_engine) waypoint: &'a mut bool,
    pub(in crate::cnc_game_engine) dragging: &'a mut bool,
    pub(in crate::cnc_game_engine) selection_start: &'a mut Option<Vec3>,
    pub(in crate::cnc_game_engine) selection_screen: &'a mut Option<(f32, f32)>,
    pub(in crate::cnc_game_engine) cursor_visible: &'a mut dyn FnMut(bool),
}
impl CinematicInputLoan<'_> {
    pub(in crate::cnc_game_engine) fn apply(
        &mut self,
        enabled: bool,
        was_enabled: bool,
        deselect: &mut dyn FnMut(u32),
        bar: &mut game_client::gui::control_bar::ControlBar,
        effects: &mut game_client::gui::game_window::WindowEventEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if !effects.has_ui() {
            return Err(gamelogic::GameLogicError::Configuration(
                "cinematic input requires the receiving InGameUI".into(),
            ));
        }
        let ui = effects.ui_mut();
        // C++ ScriptActions changes the actual UI flag before mouse/selection
        // callbacks. The world interpreter's source edge remains explicit.
        ui.set_input_enabled(enabled);
        ui.apply_cinematic_input_edge(was_enabled, enabled);
        if !enabled {
            *self.dragging = false;
            *self.selection_start = None;
            *self.selection_screen = None;
            if was_enabled {
                for held in &mut self.held {
                    **held = false;
                }
                *self.waypoint = false;
            }
        }
        // ScriptActions3176–3198: UI flag/edge, mouse visibility, then only
        // DisableInput deselects, clears modes/tooltip, and resets LookAt.
        (self.cursor_visible)(enabled);
        if enabled {
            return Ok(());
        }
        self.selected.clear();
        *self.selected_receipt = Some(Vec::new());
        if let Some(frame) = self.presentation.as_mut() {
            let frame = std::rc::Rc::make_mut(frame);
            frame.selected.clear();
            for object in &mut frame.objects {
                object.selected = false;
            }
        }
        deselect(self.local_player);
        effects.ui_mut().clear_cinematic_selection_and_modes();
        *self.waypoint = false;
        effects.with_manager(|manager, _| bar.delete_build_tooltip_layout_owned(manager));
        reset_look_at_translator_fields(
            self.modes,
            self.rotating,
            self.rotate_anchor,
            self.scrolling,
            self.scroll_anchor,
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "cinematic_input/owner_tests.rs"]
mod owner_tests;
