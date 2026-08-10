//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use crate::gui::gadgets::{Gadget, GadgetMessage, GadgetState, InputEvent};

use super::window_struct::WindowWidget;

impl WindowWidget {
    pub(crate) fn set_visible(&mut self, visible: bool) {
        match self {
            WindowWidget::PushButton(widget) => widget.set_visible(visible),
            WindowWidget::RadioButton(widget) => widget.set_visible(visible),
            WindowWidget::CheckBox(widget) => widget.set_visible(visible),
            WindowWidget::VerticalSlider(widget) => widget.set_visible(visible),
            WindowWidget::HorizontalSlider(widget) => widget.set_visible(visible),
            WindowWidget::ListBox(widget) => widget.set_visible(visible),
            WindowWidget::TextEntry(widget) => widget.set_visible(visible),
            WindowWidget::StaticText(widget) => widget.set_visible(visible),
            WindowWidget::ProgressBar(widget) => widget.set_visible(visible),
            WindowWidget::TabControl(widget) => widget.set_visible(visible),
            WindowWidget::ComboBox(widget) => widget.set_visible(visible),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => {}
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        match self {
            WindowWidget::PushButton(widget) => widget.set_enabled(enabled),
            WindowWidget::RadioButton(widget) => widget.set_enabled(enabled),
            WindowWidget::CheckBox(widget) => widget.set_enabled(enabled),
            WindowWidget::VerticalSlider(widget) => widget.set_enabled(enabled),
            WindowWidget::HorizontalSlider(widget) => widget.set_enabled(enabled),
            WindowWidget::ListBox(widget) => widget.set_enabled(enabled),
            WindowWidget::TextEntry(widget) => widget.set_enabled(enabled),
            WindowWidget::StaticText(widget) => widget.set_enabled(enabled),
            WindowWidget::ProgressBar(widget) => widget.set_enabled(enabled),
            WindowWidget::TabControl(widget) => widget.set_enabled(enabled),
            WindowWidget::ComboBox(widget) => widget.set_enabled(enabled),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => {}
        }
    }

    pub(crate) fn handle_input(&mut self, event: &InputEvent) -> Vec<GadgetMessage> {
        match self {
            WindowWidget::PushButton(widget) => widget.handle_input(event),
            WindowWidget::RadioButton(widget) => widget.handle_input(event),
            WindowWidget::CheckBox(widget) => widget.handle_input(event),
            WindowWidget::VerticalSlider(widget) => widget.handle_input(event),
            WindowWidget::HorizontalSlider(widget) => widget.handle_input(event),
            WindowWidget::ListBox(widget) => widget.handle_input(event),
            WindowWidget::TextEntry(widget) => widget.handle_input(event),
            WindowWidget::StaticText(widget) => widget.handle_input(event),
            WindowWidget::ProgressBar(widget) => widget.handle_input(event),
            WindowWidget::TabControl(widget) => widget.handle_input(event),
            WindowWidget::ComboBox(widget) => widget.handle_input(event),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => Vec::new(),
        }
    }

    pub(crate) fn state(&self) -> GadgetState {
        match self {
            WindowWidget::PushButton(widget) => widget.state(),
            WindowWidget::RadioButton(widget) => widget.state(),
            WindowWidget::CheckBox(widget) => widget.state(),
            WindowWidget::VerticalSlider(widget) => widget.state(),
            WindowWidget::HorizontalSlider(widget) => widget.state(),
            WindowWidget::ListBox(widget) => widget.state(),
            WindowWidget::TextEntry(widget) => widget.state(),
            WindowWidget::StaticText(widget) => widget.state(),
            WindowWidget::ProgressBar(widget) => widget.state(),
            WindowWidget::TabControl(widget) => widget.state(),
            WindowWidget::ComboBox(widget) => widget.state(),
            WindowWidget::TabPane
            | WindowWidget::User
            | WindowWidget::Animated
            | WindowWidget::MouseTrack => GadgetState::Normal,
        }
    }
}
