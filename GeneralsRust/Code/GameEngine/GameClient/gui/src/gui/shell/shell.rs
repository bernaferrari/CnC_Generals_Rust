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
    stack: Vec<WindowLayoutPort>,
    pending_push: bool,
    pending_push_name: String,
    pending_pop: bool,
    is_shell_active: bool,
    shell_map_on: bool,
    background: Option<WindowLayoutPort>,
    clear_background: bool,
    animate_manager: AnimateWindowManagerPort,
    update_ticks: usize,
}

impl Default for ShellPort {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            pending_push: false,
            pending_push_name: String::new(),
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

    pub fn reset(&mut self, manager: &mut GameWindowManagerPort) {
        while !self.stack.is_empty() {
            self.pop_immediate(manager);
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
        if filename.is_empty() {
            return None;
        }
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

        self.pending_push = true;
        self.pending_push_name = filename;

        let current_top = self.top_mut();
        let has_visible_top = current_top
            .as_ref()
            .map(|layout| !layout.hidden)
            .unwrap_or(false);

        if has_visible_top {
            let current_top = self.top_mut().unwrap();
            current_top.run_shutdown(shutdown_immediate);
            self.shutdown_complete(false, manager);
        } else {
            self.shutdown_complete(false, manager);
        }
    }

    pub fn pop(&mut self, manager: &mut GameWindowManagerPort) {
        if self.top().is_none() {
            return;
        }

        self.pending_pop = true;

        let screen = self.top_mut().unwrap();
        let immediate_pop = false;
        screen.run_shutdown(immediate_pop);

        self.shutdown_complete(false, manager);
    }

    pub fn pop_immediate(&mut self, _manager: &mut GameWindowManagerPort) {
        self.pending_pop = false;

        let Some(screen) = self.top_mut() else {
            return;
        };

        let immediate_pop = true;
        screen.run_shutdown(immediate_pop);

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
            let immediate_pop = true;
            layout.run_shutdown(immediate_pop);
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
        if self.stack.is_empty() {
            return None;
        }
        self.stack.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut WindowLayoutPort> {
        if self.stack.is_empty() {
            return None;
        }
        self.stack.last_mut()
    }

    pub fn screen_count(&self) -> usize {
        self.stack.len()
    }

    pub fn stack_iter(&self) -> impl Iterator<Item = &WindowLayoutPort> {
        self.stack.iter().rev()
    }

    pub fn is_shell_active(&self) -> bool {
        self.is_shell_active
    }

    pub fn link_screen(&mut self, screen: WindowLayoutPort) {
        if self.stack.len() >= MAX_SHELL_STACK {
            return;
        }
        self.stack.push(screen);
    }

    pub fn unlink_screen(&mut self) -> Option<WindowLayoutPort> {
        if self.stack.is_empty() {
            return None;
        }
        let screen = self.stack.pop().unwrap();
        debug_assert_eq!(self.stack.len(), self.stack.len(), "screen was on top");
        Some(screen)
    }

    pub fn do_push(&mut self, layout_file: impl Into<String>, manager: &mut GameWindowManagerPort) {
        let screen = manager.create_layout(layout_file);
        self.link_screen(screen);
        if let Some(top) = self.top_mut() {
            top.run_init(false);
            top.bring_forward();
        }
    }

    pub fn do_pop(&mut self, impending_push: bool) {
        let Some(mut current_top) = self.unlink_screen() else {
            return;
        };

        current_top.destroy_windows();

        if let Some(new_top) = self.top_mut() {
            if !impending_push {
                new_top.run_init(false);
            }
        }
    }

    pub fn shutdown_complete(&mut self, impending_push: bool, manager: &mut GameWindowManagerPort) {
        debug_assert!(
            !(self.pending_push && self.pending_pop),
            "pending push AND pop simultaneously is not allowed"
        );

        self.animate_manager.reset();

        if self.pending_push {
            let layout_file = std::mem::take(&mut self.pending_push_name);
            self.do_push(layout_file, manager);
            self.pending_push = false;
            self.pending_push_name.clear();
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

    fn new_shell() -> (ShellPort, GameWindowManagerPort) {
        (ShellPort::default(), GameWindowManagerPort::default())
    }

    #[test]
    fn push_onto_empty_stack() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
    }

    #[test]
    fn push_onto_existing_stack_shuts_down_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), 2);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/OptionsMenu.wnd")
        );
        let main_menu = &shell.stack[0];
        assert_eq!(
            main_menu.shutdown_runs, 1,
            "previous top should be shut down"
        );
        let options = &shell.stack[1];
        assert_eq!(options.init_runs, 1, "new screen should be initialized");
    }

    #[test]
    fn push_with_shutdown_immediate() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", true, &mut manager);
        assert_eq!(shell.screen_count(), 2);
    }

    #[test]
    fn push_rejects_empty_filename() {
        let (mut shell, mut manager) = new_shell();
        shell.push("", false, &mut manager);
        assert_eq!(shell.screen_count(), 0);
    }

    #[test]
    fn push_respects_max_stack() {
        let (mut shell, mut manager) = new_shell();
        for i in 0..MAX_SHELL_STACK {
            shell.push(format!("Menus/Screen{}.wnd", i), false, &mut manager);
        }
        assert_eq!(shell.screen_count(), MAX_SHELL_STACK);
        shell.push("Menus/Overflow.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), MAX_SHELL_STACK);
    }

    #[test]
    fn pop_removes_top_and_inits_new_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), 2);

        shell.pop(&mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
        let main_menu = &shell.stack[0];
        assert_eq!(
            main_menu.init_runs, 2,
            "MainMenu should be init'd twice (push + pop reveal)"
        );
    }

    #[test]
    fn pop_on_empty_stack_is_noop() {
        let (mut shell, mut manager) = new_shell();
        shell.pop(&mut manager);
        assert_eq!(shell.screen_count(), 0);
    }

    #[test]
    fn pop_immediate_removes_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), 2);

        shell.pop_immediate(&mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
    }

    #[test]
    fn pop_immediate_inits_new_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        shell.push("Menus/LanLobbyMenu.wnd", false, &mut manager);

        shell.pop_immediate(&mut manager);
        assert_eq!(shell.screen_count(), 2);
        let options = &shell.stack[1];
        assert_eq!(options.init_runs, 2, "OptionsMenu re-init'd after pop");
    }

    #[test]
    fn pop_immediate_on_empty_stack_is_noop() {
        let (mut shell, mut manager) = new_shell();
        shell.pop_immediate(&mut manager);
        assert_eq!(shell.screen_count(), 0);
    }

    #[test]
    fn pop_all_screens() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        shell.push("Menus/LanLobbyMenu.wnd", false, &mut manager);

        shell.pop(&mut manager);
        shell.pop(&mut manager);
        shell.pop(&mut manager);
        assert_eq!(shell.screen_count(), 0);
        assert!(shell.top().is_none());
    }

    #[test]
    fn find_screen_by_filename() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);

        assert!(shell
            .find_screen_by_filename("Menus/OptionsMenu.wnd")
            .is_some());
        assert!(shell
            .find_screen_by_filename("menus/optionsmenu.wnd")
            .is_some());
        assert!(shell
            .find_screen_by_filename("Menus/NonExistent.wnd")
            .is_none());
        assert!(shell.find_screen_by_filename("").is_none());
    }

    #[test]
    fn hide_toggles_all_layouts() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);

        shell.hide(true);
        assert!(shell.stack[0].hidden);
        assert!(shell.stack[1].hidden);

        shell.hide(false);
        assert!(!shell.stack[0].hidden);
        assert!(!shell.stack[1].hidden);
    }

    #[test]
    fn show_shell_pushes_main_menu_when_empty() {
        let (mut shell, mut manager) = new_shell();
        shell.show_shell(true, &mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
        assert!(shell.is_shell_active());
    }

    #[test]
    fn show_shell_does_not_push_if_stack_not_empty() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/LanLobbyMenu.wnd", false, &mut manager);
        shell.show_shell(true, &mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/LanLobbyMenu.wnd")
        );
    }

    #[test]
    fn show_shell_runs_init_on_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.show_shell(true, &mut manager);
        let main_menu = &shell.stack[0];
        assert_eq!(
            main_menu.init_runs, 2,
            "init should run again from show_shell"
        );
    }

    #[test]
    fn hide_shell_marks_inactive() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        assert!(shell.is_shell_active());

        shell.hide_shell();
        assert!(!shell.is_shell_active());
        let main_menu = &shell.stack[0];
        assert_eq!(
            main_menu.shutdown_runs, 1,
            "top should be shut down on hide_shell"
        );
    }

    #[test]
    fn reset_pops_all_screens() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        shell.push("Menus/LanLobbyMenu.wnd", false, &mut manager);

        shell.reset(&mut manager);
        assert_eq!(shell.screen_count(), 0);
    }

    #[test]
    fn update_runs_all_screens_top_to_bottom() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);

        shell.update();
        assert_eq!(shell.stack[0].update_runs, 1);
        assert_eq!(shell.stack[1].update_runs, 1);

        shell.update();
        assert_eq!(shell.stack[0].update_runs, 2);
        assert_eq!(shell.stack[1].update_runs, 2);
    }

    #[test]
    fn link_screen_respects_max() {
        let (mut shell, mut manager) = new_shell();
        for _ in 0..MAX_SHELL_STACK {
            let layout = manager.create_layout("Menus/Test.wnd");
            shell.link_screen(layout);
        }
        assert_eq!(shell.screen_count(), MAX_SHELL_STACK);

        let overflow = manager.create_layout("Menus/Overflow.wnd");
        shell.link_screen(overflow);
        assert_eq!(shell.screen_count(), MAX_SHELL_STACK);
    }

    #[test]
    fn unlink_screen_removes_top() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);

        let removed = shell.unlink_screen();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().filename, "Menus/OptionsMenu.wnd");
        assert_eq!(shell.screen_count(), 1);
    }

    #[test]
    fn unlink_screen_on_empty_returns_none() {
        let (mut shell, _) = new_shell();
        assert!(shell.unlink_screen().is_none());
    }

    #[test]
    fn shutdown_complete_clears_pending_push() {
        let (mut shell, mut manager) = new_shell();
        shell.pending_push = true;
        shell.pending_push_name = "Menus/Test.wnd".to_string();

        shell.shutdown_complete(false, &mut manager);

        assert!(!shell.pending_push);
        assert!(shell.pending_push_name.is_empty());
        assert_eq!(shell.screen_count(), 1);
    }

    #[test]
    fn shutdown_complete_clears_pending_pop() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        shell.pending_pop = true;

        shell.shutdown_complete(false, &mut manager);

        assert!(!shell.pending_pop);
        assert_eq!(shell.screen_count(), 1);
    }

    #[test]
    fn push_pop_sequence() {
        let (mut shell, mut manager) = new_shell();
        shell.push("Menus/MainMenu.wnd", false, &mut manager);
        shell.push("Menus/OptionsMenu.wnd", false, &mut manager);
        shell.push("Menus/LanLobbyMenu.wnd", false, &mut manager);

        shell.pop(&mut manager);
        assert_eq!(shell.screen_count(), 2);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/OptionsMenu.wnd")
        );

        shell.push("Menus/ReplayMenu.wnd", false, &mut manager);
        assert_eq!(shell.screen_count(), 3);

        shell.pop_immediate(&mut manager);
        assert_eq!(shell.screen_count(), 2);

        shell.pop_immediate(&mut manager);
        assert_eq!(shell.screen_count(), 1);
        assert_eq!(
            shell.top().map(|l| l.filename.as_str()),
            Some("Menus/MainMenu.wnd")
        );
    }
}
