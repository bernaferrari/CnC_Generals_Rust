/// Source callbacks receive their exact owner through WindowEventEffects.
pub fn queue_shell_operation(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    operation: impl FnOnce(&mut Shell) + 'static,
) {
    effects
        .window_manager_owner
        .expect("Shell callback requires receiving owner")
        .queue_shell_operation_with_effects(move |shell, _effects| operation(shell));
}
pub fn queue_shell_operation_with_effects(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    operation: impl for<'a> FnOnce(&mut Shell, &mut crate::gui::game_window::WindowEventEffects<'a>)
    + 'static,
) {
    effects
        .window_manager_owner
        .expect("Shell callback requires receiving owner")
        .queue_shell_operation_with_effects(operation);
}
pub fn shell_anim_finished_for_layout(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects
        .window_manager_owner
        .expect("Shell callback requires receiving owner")
        .shell_anim_finished()
}
pub(crate) fn record_shell_anim_finished_for_layout(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    finished: bool,
) {
    effects
        .window_manager_owner
        .expect("Shell callback requires receiving owner")
        .record_shell_anim_finished(finished);
}
pub(crate) fn take_pending_shell_scheme_for_layout(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<String> {
    effects
        .window_manager_owner
        .expect("Shell callback requires receiving owner")
        .take_pending_shell_scheme()
}

/// A non-borrowing façade capturing one explicit native client's Shell owner.
/// `get_shell()` is a compatibility adapter that captures the current scoped owner.
#[derive(Clone, Default)]
pub struct ShellHandle {
    owner: Option<crate::gui::window_manager::WindowManagerOwner>,
}

impl std::fmt::Debug for ShellHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ShellHandle")
    }
}

impl ShellHandle {
    pub fn for_owner(owner: crate::gui::window_manager::WindowManagerOwner) -> Self {
        Self { owner: Some(owner) }
    }

    fn owner(&self) -> Option<&crate::gui::window_manager::WindowManagerOwner> {
        self.owner.as_ref()
    }

    fn unavailable() -> ShellError {
        ShellError::NotInitialized
    }

    pub fn is_shell_active(&self) -> bool {
        self.owner()
            .and_then(|owner| owner.with_shell_ref(|shell| shell.is_shell_active()))
            .unwrap_or(false)
    }

    pub fn is_shell_map_on(&self) -> bool {
        self.owner()
            .and_then(|owner| owner.with_shell_ref(|shell| shell.is_shell_map_on()))
            .unwrap_or(false)
    }

    pub fn is_anim_finished(&self) -> bool {
        self.owner()
            .and_then(|owner| {
                owner.with_manager_ref(|manager| {
                    owner.with_shell_ref(|shell| shell.is_anim_finished_with_manager(manager))
                })
            })
            .unwrap_or(false)
    }

    pub fn get_screen_count(&self) -> usize {
        self.owner()
            .and_then(|owner| owner.with_shell_ref(|shell| shell.get_screen_count()))
            .unwrap_or(0)
    }

    pub fn top_filename(&self) -> Option<String> {
        self.owner()
            .and_then(|owner| owner.with_shell_ref(|shell| shell.top_filename().map(str::to_owned)))
            .flatten()
    }

    pub fn set_shell_active(&mut self, active: bool) {
        if let Some(owner) = self.owner() {
            owner.queue_shell_operation_with_effects(move |shell, _effects| {
                shell.set_shell_active(active)
            });
        } else {
            log::warn!("Shell activation has no receiving WindowManagerOwner; ignored");
        }
    }

    pub fn show_shell_map(&mut self, on: bool) {
        self.owner()
            .expect("Shell-map change requires receiving owner")
            .queue_shell_operation_with_effects(move |shell, effects| {
                shell.show_shell_map(on, effects)
            });
    }

    pub fn reverse_animate_window(&mut self) {
        if let Some(owner) = self.owner() {
            owner.queue_shell_operation_with_effects(|shell, _effects| {
                shell.reverse_animate_window()
            });
        } else {
            log::warn!("Shell animation has no receiving WindowManagerOwner; ignored");
        }
    }

    pub fn push(&mut self, filename: &str, shutdown_immediate: bool) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;
        let filename = filename.to_owned();
        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.push(&filename, shutdown_immediate, effects) {
                log::warn!("Shell::push failed: {error}");
            }
        });
        Ok(())
    }

    pub fn pop(&mut self) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;

        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.pop(effects) {
                log::warn!("Shell::pop failed: {error}");
            }
        });
        Ok(())
    }

    pub fn pop_immediate(&mut self) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;

        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.pop_immediate(effects) {
                log::warn!("Shell::pop_immediate failed: {error}");
            }
        });
        Ok(())
    }

    pub fn shutdown_complete(
        &mut self,
        _layout: Option<&dyn WindowLayout>,
        impending_push: bool,
    ) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;
        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.shutdown_complete(None, impending_push, effects) {
                log::warn!("Shell::shutdown_complete failed: {error}");
            }
        });
        Ok(())
    }

    pub fn show_shell(&mut self, run_init: bool) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;

        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.show_shell(run_init, effects) {
                log::warn!("Shell::show_shell failed: {error}");
            }
        });
        Ok(())
    }

    pub fn hide_shell(&mut self) -> Result<(), ShellError> {
        let owner = self.owner().ok_or_else(Self::unavailable)?;

        owner.queue_shell_operation_with_effects(move |shell, effects| {
            if let Err(error) = shell.hide_shell(effects) {
                log::warn!("Shell::hide_shell failed: {error}");
            }
        });
        Ok(())
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let owner = self
            .owner()
            .ok_or_else(|| Box::new(Self::unavailable()) as Box<dyn std::error::Error>)?;
        match owner.with_shell_mut(|shell| shell.init()) {
            Some(result) => result,
            None => {
                owner.queue_shell_operation_with_effects(|shell, _effects| {
                    if let Err(error) = shell.init() {
                        log::warn!("deferred Shell::init failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let owner = self
            .owner()
            .ok_or_else(|| Box::new(Self::unavailable()) as Box<dyn std::error::Error>)?;
        owner.queue_shell_operation_with_effects(|shell, effects| {
            if let Err(error) = shell.reset_receiving(effects) {
                log::warn!("Shell reset failed: {error}");
            }
        });
        Ok(())
    }

    pub fn update(
        &mut self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let owner = self
            .owner()
            .ok_or_else(|| Box::new(Self::unavailable()) as Box<dyn std::error::Error>)?;
        match owner
            .with_shell_effects_mut(effects, |shell, effects| shell.update_receiving(effects))
        {
            Some(result) => result,
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Shell::update requires same-frame message effects; owner borrow was unavailable",
            ))),
        }
    }
}

impl crate::gui::window_manager::WindowManagerOwner {
    /// Return an explicit handle to this owner's Shell for routes that already
    /// receive the client capability.
    pub fn shell_handle(&self) -> ShellHandle {
        ShellHandle::for_owner(self.clone())
    }
}

/// Enable shell map.  Nested lifecycle calls retain the request in the owned
/// operation queue instead of making a second mutable borrow.
pub fn show_shell_map_if_available(
    on: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.request_shell_map(on);
}

/// Queue the C++ `TheShell->reverseAnimateWindow()` side effect.
pub fn queue_shell_reverse_animate_window(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    queue_shell_operation(effects, |shell| shell.reverse_animate_window());
}

/// Queue the C++ `TheShell->shutdownComplete()` side effect.
pub fn queue_shell_shutdown_complete(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    impending_push: bool,
) {
    queue_shell_operation_with_effects(effects, move |shell, effects| {
        if let Err(error) = shell.shutdown_complete(None, impending_push, effects) {
            log::warn!("deferred Shell::shutdown_complete failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->push()` side effect with owned layout data.
pub fn queue_shell_push(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    filename: impl Into<String>,
    shutdown_immediate: bool,
) {
    let filename = filename.into();
    queue_shell_operation_with_effects(effects, move |shell, effects| {
        if let Err(error) = shell.push(&filename, shutdown_immediate, effects) {
            log::warn!("deferred Shell::push({filename}) failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->pop()` side effect.
pub fn queue_shell_pop(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    queue_shell_operation_with_effects(effects, |shell, effects| {
        if let Err(error) = shell.pop(effects) {
            log::warn!("deferred Shell::pop failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->hideShell()` side effect.
pub fn queue_shell_hide(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    queue_shell_operation_with_effects(effects, |shell, effects| {
        if let Err(error) = shell.hide_shell(effects) {
            log::warn!("deferred Shell::hide_shell failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->showShell()` side effect.
pub fn queue_shell_show(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    run_init: bool,
) {
    queue_shell_operation_with_effects(effects, move |shell, effects| {
        if let Err(error) = shell.show_shell(run_init, effects) {
            log::warn!("deferred Shell::show_shell failed: {error}");
        }
    });
}

/// Queue registration of a window transition from a layout-init callback.
pub fn queue_shell_window_animation(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    window: Rc<RefCell<GameWindow>>,
    animation: AnimationType,
    needs_to_finish: bool,
    delay_ms: u64,
) {
    queue_shell_operation(effects, move |shell| {
        shell.register_with_animate_manager(window, animation, needs_to_finish, delay_ms);
    });
}

pub fn request_shell_menu_scheme(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    name: &str,
) {
    let name = name.to_string();
    queue_shell_operation(effects, move |shell| shell.load_scheme(&name));
}
