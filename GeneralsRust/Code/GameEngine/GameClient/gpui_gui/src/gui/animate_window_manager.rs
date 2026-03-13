use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "AnimateWindowManager.cpp",
    "crate::gui::animate_window_manager",
    "Animate Window Manager",
    "Tracks queued window animations, reversal, and completion state for shell transitions.",
);

#[derive(Clone, Debug, Default)]
pub struct AnimateWindowManagerPort {
    queue: Vec<String>,
    reversed: bool,
}

impl AnimateWindowManagerPort {
    pub fn register(&mut self, animation: impl Into<String>) {
        self.queue.push(animation.into());
    }

    pub fn reset(&mut self) {
        self.queue.clear();
        self.reversed = false;
    }

    pub fn update(&mut self) -> Option<String> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    pub fn reverse_animate_window(&mut self) {
        self.reversed = !self.reversed;
        self.queue.reverse();
    }

    pub fn is_finished(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }
}
