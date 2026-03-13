use crate::gui::animate_window_manager::AnimateWindowManagerPort;
use crate::gui::game_window_manager::GameWindowManagerPort;
use crate::gui::source_catalog::GuiPortRecord;
use crate::gui::window_layout::WindowLayoutPort;

const MAX_SHELL_STACK: usize = 16;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "Shell/Shell.cpp",
    "crate::gui::shell::shell",
    "Shell",
    "Implements shell stack transitions, pending push/pop flow, and background/layout lifecycle.",
);

#[derive(Clone, Debug)]
pub struct ShellPort {
    pub stack: Vec<WindowLayoutPort>,
    pub pending_push_name: Option<String>,
    pub pending_pop: bool,
    pub is_shell_active: bool,
    pub shell_map_on: bool,
    pub background: Option<WindowLayoutPort>,
    pub clear_background: bool,
    pub animate_manager: AnimateWindowManagerPort,
    pub update_ticks: usize,
}

impl Default for ShellPort {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            pending_push_name: None,
            pending_pop: false,
            is_shell_active: true,
            shell_map_on: false,
            background: None,
            clear_background: false,
            animate_manager: AnimateWindowManagerPort::default(),
            update_ticks: 0,
        }
    }
}

impl ShellPort {
    pub fn init(&mut self) {}

    pub fn reset(&mut self) {
        while !self.stack.is_empty() {
            self.pop_immediate();
        }
        self.animate_manager.reset();
    }

    pub fn update(&mut self) {
        for layout in self.stack.iter_mut().rev() {
            layout.run_update();
        }
        self.animate_manager.update();
        self.update_ticks += 1;
    }

    pub fn find_screen_by_filename(&self, filename: &str) -> Option<&WindowLayoutPort> {
        self.stack
            .iter()
            .find(|layout| layout.filename.eq_ignore_ascii_case(filename))
    }

    pub fn hide(&mut self, hide: bool) {
        for layout in &mut self.stack {
            layout.hide(hide);
        }
    }

    pub fn push(
        &mut self,
        filename: impl Into<String>,
        shutdown_immediate: bool,
        manager: &mut GameWindowManagerPort,
    ) {
        let filename = filename.into();
        if filename.is_empty() || self.stack.len() >= MAX_SHELL_STACK {
            return;
        }

        self.pending_push_name = Some(filename);
        if let Some(current_top) = self.top_mut().filter(|layout| !layout.hidden) {
            current_top.run_shutdown(shutdown_immediate);
            self.shutdown_complete(shutdown_immediate, manager);
        } else {
            self.shutdown_complete(shutdown_immediate, manager);
        }
    }

    pub fn pop(&mut self) {
        if self.top().is_none() {
            return;
        }

        self.pending_pop = true;
        if let Some(screen) = self.top_mut() {
            screen.run_shutdown(false);
        }
        self.animate_manager.reset();
        self.do_pop(false);
        self.pending_pop = false;
    }

    pub fn pop_immediate(&mut self) {
        let Some(screen) = self.top_mut() else {
            return;
        };

        screen.run_shutdown(true);
        self.do_pop(false);
    }

    pub fn show_shell(&mut self, run_init: bool, manager: &mut GameWindowManagerPort) {
        if run_init {
            if let Some(layout) = self.top_mut() {
                layout.run_init(false);
            }
        }

        if self.stack.is_empty() {
            self.push("Menus/MainMenu.wnd", false, manager);
        }
        self.is_shell_active = true;
    }

    pub fn hide_shell(&mut self) {
        self.clear_background = true;
        if let Some(layout) = self.top_mut() {
            layout.run_shutdown(true);
        }

        if self.clear_background {
            if let Some(background) = &mut self.background {
                background.destroy_windows();
            }
            self.background = None;
            self.clear_background = false;
        }

        self.is_shell_active = false;
    }

    pub fn top(&self) -> Option<&WindowLayoutPort> {
        self.stack.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut WindowLayoutPort> {
        self.stack.last_mut()
    }

    pub fn link_screen(&mut self, screen: WindowLayoutPort) {
        if self.stack.len() < MAX_SHELL_STACK {
            self.stack.push(screen);
        }
    }

    pub fn unlink_screen(&mut self) -> Option<WindowLayoutPort> {
        self.stack.pop()
    }

    pub fn do_push(&mut self, layout_file: impl Into<String>, manager: &mut GameWindowManagerPort) {
        let mut screen = manager.create_layout(layout_file);
        screen.run_init(false);
        screen.bring_forward();
        self.link_screen(screen);
    }

    pub fn do_pop(&mut self, impending_push: bool) {
        let Some(mut current_top) = self.unlink_screen() else {
            return;
        };

        current_top.destroy_windows();

        if let Some(new_top) = self.top_mut().filter(|_| !impending_push) {
            new_top.run_init(false);
        }
    }

    pub fn shutdown_complete(&mut self, impending_push: bool, manager: &mut GameWindowManagerPort) {
        self.animate_manager.reset();

        if let Some(layout_file) = self.pending_push_name.take() {
            self.do_push(layout_file, manager);
        } else if self.pending_pop {
            self.do_pop(impending_push);
            self.pending_pop = false;
        }

        if self.clear_background {
            if let Some(background) = &mut self.background {
                background.destroy_windows();
            }
            self.background = None;
            self.clear_background = false;
        }
    }

    pub fn register_with_animate_manager(&mut self, animation: impl Into<String>) {
        self.animate_manager.register(animation);
    }

    pub fn is_anim_finished(&self) -> bool {
        self.animate_manager.is_finished()
    }

    pub fn reverse_animate_window(&mut self) {
        self.animate_manager.reverse_animate_window();
    }

    pub fn is_anim_reversed(&self) -> bool {
        self.animate_manager.is_reversed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop_changes_shell_stack() {
        let mut manager = GameWindowManagerPort::default();
        let mut shell = ShellPort::default();

        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        assert_eq!(shell.stack.len(), 2);
        assert_eq!(
            shell.top().map(|layout| layout.filename.as_str()),
            Some("Menus/OptionsMenu.wnd")
        );

        shell.pop_immediate();
        assert_eq!(shell.stack.len(), 1);
        assert_eq!(
            shell.top().map(|layout| layout.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
    }
}
