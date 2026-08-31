// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
// C++ Shell.cpp: screen stack lifecycle, pending push/pop, background, subsystem.
#[cfg(feature = "online_ui")]
fn close_shell_gamespy_overlays() {
    crate::gamespy_overlay::close_all_overlays();
}

#[cfg(not(feature = "online_ui"))]
fn close_shell_gamespy_overlays() {}

/// The main Shell system for managing menu screens
///
/// This provides a stack-based screen management system where screens can be
/// pushed and popped with proper initialization and shutdown handling.
pub struct Shell {
    /// Stack of screen layouts (top of stack is the active screen)
    screen_stack: Vec<Box<dyn WindowLayout>>,
    /// Maximum number of screens allowed on the stack
    max_stack_size: usize,
    /// Whether the shell is currently active
    is_shell_active: bool,
    /// Whether the shell map background is enabled
    shell_map_on: bool,
    /// Background layout for non-3D shell mode
    background: Option<Box<dyn WindowLayout>>,
    /// Whether to clear the background
    clear_background: bool,
    /// Pending push operation
    pending_push: bool,
    /// Pending pop operation
    pending_pop: bool,
    /// Name of layout to push when pending operation completes
    pending_push_name: String,
    /// Animation window manager
    animate_window_manager: AnimateWindowManager,
    /// Shell menu scheme manager
    scheme_manager: ShellMenuSchemeManager,
    /// Cached special layouts
    save_load_menu_layout: Option<Box<dyn WindowLayout>>,
    popup_replay_layout: Option<Box<dyn WindowLayout>>,
    options_layout: Option<Box<dyn WindowLayout>>,
    /// Whether the shell has been initialized
    initialized: bool,
    /// Shell update timing
    last_update: Instant,
    update_interval: Duration,
}

impl Shell {
    /// Create a new Shell system
    pub fn new() -> Self {
        Self {
            screen_stack: Vec::new(),
            max_stack_size: 16, // MAX_SHELL_STACK from original
            is_shell_active: true,
            shell_map_on: false,
            background: None,
            clear_background: false,
            pending_push: false,
            pending_pop: false,
            pending_push_name: String::new(),
            animate_window_manager: AnimateWindowManager::new(),
            scheme_manager: ShellMenuSchemeManager::new(),
            save_load_menu_layout: None,
            popup_replay_layout: None,
            options_layout: None,
            initialized: false,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(33), // ~30 FPS like original
        }
    }

    /// Push a new screen layout onto the stack
    ///
    /// # Arguments
    /// * `filename` - Path to the layout file to load
    /// * `shutdown_immediate` - Whether to shutdown the current top immediately
    pub fn push(&mut self, filename: &str, shutdown_immediate: bool) -> Result<(), ShellError> {
        if !self.initialized {
            return Err(ShellError::NotInitialized);
        }

        if filename.is_empty() {
            return Err(ShellError::LayoutNotFound("Empty filename".to_string()));
        }

        if self.screen_stack.len() >= self.max_stack_size {
            return Err(ShellError::StackOverflow {
                max: self.max_stack_size,
            });
        }

        close_shell_gamespy_overlays();

        log::debug!(
            "Shell::push({}) - current stack size: {}",
            filename,
            self.screen_stack.len()
        );

        // Set pending push operation
        self.pending_push = true;
        self.pending_push_name = filename.to_string();

        // Get current top of stack
        if let Some(current_top) = self.screen_stack.last_mut() {
            if !current_top.is_hidden() {
                let mut immediate = shutdown_immediate;
                current_top.run_shutdown(&mut immediate)?;

                if immediate {
                    // Complete the shutdown immediately
                    self.shutdown_complete(None, true)?;
                }
            } else {
                // Match C++ Shell::push(): if the top is already hidden, complete the pending
                // push immediately instead of leaving the shell stuck with a latent push request.
                self.shutdown_complete(None, false)?;
            }
        } else {
            // No current top, do push immediately
            self.shutdown_complete(None, false)?;
        }

        Ok(())
    }

    /// Pop the top screen from the stack
    pub fn pop(&mut self) -> Result<(), ShellError> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }

        close_shell_gamespy_overlays();

        log::debug!(
            "Shell::pop() - current stack size: {}",
            self.screen_stack.len()
        );

        // Set pending pop operation
        self.pending_pop = true;

        // Shutdown the top screen
        if let Some(top) = self.screen_stack.last_mut() {
            let mut immediate_pop = false;
            top.run_shutdown(&mut immediate_pop)?;

            if immediate_pop {
                self.shutdown_complete(None, false)?;
            }
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }

    /// Immediately pop the top screen without waiting for shutdown completion
    pub fn pop_immediate(&mut self) -> Result<(), ShellError> {
        if self.screen_stack.is_empty() {
            return Err(ShellError::EmptyStack);
        }

        log::debug!(
            "Shell::pop_immediate() - current stack size: {}",
            self.screen_stack.len()
        );

        // Don't set pending pop - we're doing it immediately
        self.pending_pop = false;

        // Match C++ Shell::popImmediate(): run shutdown while the screen is still the active top,
        // then perform the actual pop through the normal workhorse.
        if let Some(top) = self.screen_stack.last_mut() {
            let mut immediate_pop = true;
            top.run_shutdown(&mut immediate_pop)?;
        }

        self.do_pop(false)?;

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }

    /// Get the top screen on the stack
    pub fn top(&mut self) -> Option<&mut (dyn WindowLayout + 'static)> {
        self.screen_stack
            .last_mut()
            .map(move |layout| layout.as_mut())
    }

    /// Get the current number of screens on the stack
    pub fn get_screen_count(&self) -> usize {
        self.screen_stack.len()
    }

    /// Filename of the active stack top, without exposing a layout borrow.
    pub fn top_filename(&self) -> Option<&str> {
        self.screen_stack.last().map(|layout| layout.get_filename())
    }

    /// Check if the shell is currently active
    pub fn is_shell_active(&self) -> bool {
        self.is_shell_active
    }

    /// Residual: force shell inactive without layout shutdown animation (match start).
    pub fn set_shell_active(&mut self, active: bool) {
        self.is_shell_active = active;
        if !active {
            self.clear_background = true;
        }
    }

    /// Check if the shell map background has been requested.
    pub fn is_shell_map_on(&self) -> bool {
        self.shell_map_on
    }

    /// Show or hide all shell layouts
    pub fn hide(&mut self, hide: bool) {
        for layout in &mut self.screen_stack {
            layout.hide(hide);
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }
    }

    /// Show the shell (initialize top screen)
    pub fn show_shell(&mut self, run_init: bool) -> Result<(), ShellError> {
        log::debug!("Shell::show_shell(run_init: {})", run_init);

        if get_global_data()
            .map(|data| !data.read().initial_file.is_empty())
            .unwrap_or(false)
        {
            return Ok(());
        }

        if run_init {
            if let Some(layout) = self.screen_stack.last_mut() {
                layout.run_init(None)?;
            }
        }

        let shell_map_enabled = get_global_data()
            .map(|data| data.read().shell_map_on)
            .unwrap_or(false);
        if !shell_map_enabled && self.screen_stack.is_empty() {
            self.push("Menus/MainMenu.wnd", false)?;
        }

        self.is_shell_active = true;
        Ok(())
    }

    /// C++ GameLogic::startNewGame shell completion branch:
    /// push MainMenu when the shell stack is empty, otherwise reveal the top screen.
    pub fn show_main_menu_after_shell_game_start(&mut self) -> Result<(), ShellError> {
        if self.screen_stack.is_empty() {
            self.push("Menus/MainMenu.wnd", false)
        } else {
            if let Some(top) = self.top() {
                top.hide(false);
                top.bring_forward();
            }
            Ok(())
        }
    }

    /// Hide the shell (shutdown top screen without popping)
    pub fn hide_shell(&mut self) -> Result<(), ShellError> {
        log::debug!("Shell::hide_shell()");

        self.clear_background = true;

        if let Some(layout) = self.screen_stack.last_mut() {
            let mut immediate_pop = true;
            layout.run_shutdown(&mut immediate_pop)?;
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        self.is_shell_active = false;
        Ok(())
    }

    /// Called when a layout has completed its shutdown process
    pub fn shutdown_complete(
        &mut self,
        _layout: Option<&dyn WindowLayout>,
        impending_push: bool,
    ) -> Result<(), ShellError> {
        // Reset animation manager
        self.animate_window_manager.reset();

        if self.pending_push {
            // Do the push
            self.do_push(&self.pending_push_name.clone())?;
            self.pending_push = false;
            self.pending_push_name.clear();
        } else if self.pending_pop {
            // Do the pop
            self.do_pop(impending_push)?;
            self.pending_pop = false;
        }

        if self.clear_background {
            if let Some(mut background) = self.background.take() {
                background.destroy_windows();
                self.clear_background = false;
            }
        }

        Ok(())
    }

    /// Find a screen by its filename
    pub fn find_screen_by_filename(&self, filename: &str) -> Option<&dyn WindowLayout> {
        self.screen_stack
            .iter()
            .find(|layout| layout.get_filename().eq_ignore_ascii_case(filename))
            .map(|layout| layout.as_ref())
    }

    /// Register a window with the animation manager
    pub fn register_with_animate_manager(
        &mut self,
        window: Rc<RefCell<GameWindow>>,
        anim_type: AnimationType,
        needs_to_finish: bool,
        delay_ms: u64,
    ) {
        let animate_windows = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if !animate_windows {
            return;
        }
        self.animate_window_manager.register_window(
            window,
            anim_type,
            needs_to_finish,
            500, // Default 500ms duration
            delay_ms,
        );
    }

    /// Check if animations are finished
    pub fn is_anim_finished(&self) -> bool {
        if !with_window_manager_ref(|manager| manager.transitions_finished()) {
            return false;
        }

        let animate_windows = get_global_data()
            .map(|data| data.read().animate_windows)
            .unwrap_or(true);
        if animate_windows {
            self.animate_window_manager.is_finished()
        } else {
            true
        }
    }

    /// Reverse window animations
    pub fn reverse_animate_window(&mut self) {
        self.animate_window_manager.reverse_animate_window();
    }

    /// Check if animations are reversed
    pub fn is_anim_reversed(&self) -> bool {
        self.animate_window_manager.is_reversed()
    }

    /// Load a menu scheme
    pub fn load_scheme(&mut self, name: &str) {
        self.scheme_manager.set_shell_menu_scheme(name);
    }

    /// Get the shell menu scheme manager
    pub fn get_shell_menu_scheme_manager(&mut self) -> &mut ShellMenuSchemeManager {
        &mut self.scheme_manager
    }

    /// Get or create the save/load menu layout
    pub fn get_save_load_menu_layout(&mut self) -> Result<&mut dyn WindowLayout, ShellError> {
        if self.save_load_menu_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new(
                "Menus/PopupSaveLoad.wnd".to_string(),
            ));
            self.save_load_menu_layout = Some(layout);
        }

        Ok(self.save_load_menu_layout.as_mut().unwrap().as_mut())
    }

    /// Get or create the popup replay layout
    pub fn get_popup_replay_layout(&mut self) -> Result<&mut dyn WindowLayout, ShellError> {
        if self.popup_replay_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new("Menus/PopupReplay.wnd".to_string()));
            self.popup_replay_layout = Some(layout);
        }

        Ok(self.popup_replay_layout.as_mut().unwrap().as_mut())
    }

    /// Get or create the options layout
    pub fn get_options_layout(
        &mut self,
        create: bool,
    ) -> Option<&mut (dyn WindowLayout + 'static)> {
        if create && self.options_layout.is_none() {
            let layout = Box::new(BasicWindowLayout::new("Menus/OptionsMenu.wnd".to_string()));
            self.options_layout = Some(layout);
        }

        self.options_layout
            .as_mut()
            .map(move |layout| layout.as_mut())
    }

    /// Destroy the options layout
    pub fn destroy_options_layout(&mut self) {
        if let Some(mut layout) = self.options_layout.take() {
            layout.destroy_windows();
        }
    }

    /// Show or hide the shell map
    pub fn show_shell_map(&mut self, use_shell_map: bool) {
        let Some(global) = get_global_data() else {
            return;
        };
        let initial_file_not_empty = !global.read().initial_file.is_empty();
        if initial_file_not_empty {
            return;
        }

        let shell_map_enabled = global.read().shell_map_on;
        if use_shell_map && shell_map_enabled {
            if TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() == GAME_SHELL {
                return;
            }

            if TheGameLogic::is_in_game() {
                let message_stream = get_message_stream();
                let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
                stream.append_message(GameMessageType::ClearGameData);
            }

            let shell_map_name = global.read().shell_map_name.clone();
            {
                let mut data = global.write();
                data.pending_file = shell_map_name;
            }
            init_random_with_seed(0);
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
            let msg = stream.append_message(GameMessageType::NewGame);
            msg.append_integer_argument(GAME_SHELL);
            self.shell_map_on = true;
        } else {
            if TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() == GAME_SHELL {
                let message_stream = get_message_stream();
                let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
                stream.append_message(GameMessageType::ClearGameData);
            }

            if !self.is_shell_active {
                return;
            }
            if self.background.is_none() {
                self.background = Some(Box::new(BasicWindowLayout::new(
                    "Menus/BlankWindow.wnd".to_string(),
                )));
            }
            if let Some(ref mut bg) = self.background {
                if let Err(err) = bg.run_init(None) {
                    log::warn!("Failed to initialize shell background layout: {}", err);
                }
                bg.set_first_window_image();
                bg.hide(false);
                if let Some(top) = self.screen_stack.last_mut() {
                    top.bring_forward();
                }
            }
            self.shell_map_on = false;
            self.clear_background = false;
        }

        log::debug!("Shell map enabled: {}", self.shell_map_on);
    }

    fn do_push(&mut self, layout_file: &str) -> Result<(), ShellError> {
        log::debug!("Shell::do_push({})", layout_file);

        // Create new layout - in a real implementation, this would load from file
        let mut new_screen = Box::new(BasicWindowLayout::new(layout_file.to_string()));
        if layout_file.eq_ignore_ascii_case("Menus/MainMenu.wnd") {
            self.load_scheme("MainMenu");
        }

        // Add to stack
        self.screen_stack.push(new_screen);

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        // Initialize the new screen
        if let Some(screen) = self.screen_stack.last_mut() {
            screen.run_init(None)?;
            screen.bring_forward();
        }

        Ok(())
    }

    fn do_pop(&mut self, impending_push: bool) -> Result<(), ShellError> {
        log::debug!("Shell::do_pop(impending_push: {})", impending_push);

        // Remove and destroy the top screen
        if let Some(mut current_top) = self.screen_stack.pop() {
            current_top.destroy_windows();
        }

        // Initialize the new top if present and not doing an impending push
        if !impending_push {
            if let Some(new_top) = self.screen_stack.last_mut() {
                new_top.run_init(None)?;
            }
        }

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        Ok(())
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for Shell {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing shell system");

        // Initialize the scheme manager
        self.scheme_manager.init()?;
        self.last_update = Instant::now();

        self.initialized = true;
        log::info!("Shell system initialized successfully");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting shell system");

        if let Ok(mut ime_manager) = get_ime_manager().lock() {
            ime_manager.detach();
        }

        // Pop all screens. The local test layouts don't model the C++ callback chain,
        // so we use the immediate pop path to keep the stack teardown deterministic here.
        while !self.screen_stack.is_empty() {
            self.pop_immediate()?;
        }

        // Reset animation manager
        self.animate_window_manager.reset();

        log::info!("Shell system reset completed");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.update_interval {
            if let Some(name) = PENDING_SHELL_SCHEME.with(|pending| pending.borrow_mut().take()) {
                self.load_scheme(&name);
            }

            // C++ MainMenuUpdate reads TheShell->isAnimFinished() while
            // Shell::update already holds *this. Snapshot so callbacks
            // cannot fail-closed and leave pending_push stuck.
            SHELL_ANIM_FINISHED.with(|flag| flag.set(self.is_anim_finished()));

            // Update all layouts on the stack (from top to bottom)
            for i in (0..self.screen_stack.len()).rev() {
                self.screen_stack[i].run_update(None)?;
            }

            let global_shell_map_on = get_global_data()
                .map(|data| data.read().shell_map_on)
                .unwrap_or(false);
            if global_shell_map_on && self.shell_map_on && self.background.is_some() {
                if let Some(mut background) = self.background.take() {
                    background.destroy_windows();
                }
            }

            self.animate_window_manager.update();
            self.scheme_manager.update()?;

            self.last_update = now;
        }

        Ok(())
    }
}
