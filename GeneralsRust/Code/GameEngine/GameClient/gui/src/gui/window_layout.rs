use crate::gui::game_window::GameWindowPort;
use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "WindowLayout.cpp",
    "crate::gui::window_layout",
    "Window Layout",
    "Groups windows loaded from script, tracks visibility, and preserves layout ordering.",
);

#[derive(Clone, Debug)]
pub struct WindowLayoutPort {
    pub filename: String,
    pub windows: Vec<GameWindowPort>,
    pub hidden: bool,
    pub init_runs: usize,
    pub update_runs: usize,
    pub shutdown_runs: usize,
}

impl WindowLayoutPort {
    pub fn new(filename: impl Into<String>, mut windows: Vec<GameWindowPort>) -> Self {
        let filename = filename.into();
        for window in &mut windows {
            window.set_layout(Some(filename.clone()));
        }

        let mut layout = Self {
            filename,
            windows,
            hidden: false,
            init_runs: 0,
            update_runs: 0,
            shutdown_runs: 0,
        };
        layout.sync_layout_links();
        layout
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn find_window(&self, window_id: i32) -> Option<&GameWindowPort> {
        self.windows.iter().find(|window| window.id == window_id)
    }

    pub fn find_window_mut(&mut self, window_id: i32) -> Option<&mut GameWindowPort> {
        self.windows
            .iter_mut()
            .find(|window| window.id == window_id)
    }

    pub fn hide(&mut self, hidden: bool) {
        for window in &mut self.windows {
            window.hide(hidden);
        }
        self.hidden = hidden;
    }

    pub fn add_window(&mut self, mut window: GameWindowPort) {
        if self.find_window(window.id).is_some() {
            return;
        }

        window.set_layout(Some(self.filename.clone()));
        self.windows.insert(0, window);
        self.sync_layout_links();
    }

    pub fn remove_window(&mut self, window_id: i32) -> Option<GameWindowPort> {
        let index = self
            .windows
            .iter()
            .position(|window| window.id == window_id)?;
        let removed = self.windows.remove(index);
        self.sync_layout_links();
        Some(removed)
    }

    pub fn destroy_windows(&mut self) {
        self.windows.clear();
    }

    pub fn run_init(&mut self, _shutdown_immediate: bool) {
        self.init_runs += 1;
    }

    pub fn run_update(&mut self) {
        self.update_runs += 1;
    }

    pub fn run_shutdown(&mut self, _immediate: bool) {
        self.shutdown_runs += 1;
    }

    pub fn bring_forward(&mut self) {
        // In the standalone port this layout already owns the relative draw order,
        // so preserving the current ordering is the correct equivalent behavior.
        self.sync_layout_links();
    }

    fn sync_layout_links(&mut self) {
        let ids: Vec<i32> = self.windows.iter().map(|window| window.id).collect();
        for (index, window) in self.windows.iter_mut().enumerate() {
            let prev = ids.get(index + 1).copied();
            let next = index
                .checked_sub(1)
                .and_then(|prev_index| ids.get(prev_index).copied());
            window.set_prev_in_layout(prev);
            window.set_next_in_layout(next);
            window.set_layout(Some(self.filename.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LegacyRect;

    #[test]
    fn add_window_puts_new_window_at_layout_head() {
        let mut layout = WindowLayoutPort::new("Menus/Test.wnd", Vec::new());
        layout.add_window(GameWindowPort::new(
            1,
            "One",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        ));
        layout.add_window(GameWindowPort::new(
            2,
            "Two",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        ));

        assert_eq!(layout.windows[0].id, 2);
        assert_eq!(layout.windows[1].id, 1);
        assert_eq!(layout.windows[0].prev_layout, Some(1));
        assert_eq!(layout.windows[1].next_layout, Some(2));
    }
}
