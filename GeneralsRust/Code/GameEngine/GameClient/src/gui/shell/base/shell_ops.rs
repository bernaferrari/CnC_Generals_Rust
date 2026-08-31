// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
// TheShell singleton access: borrow-safe operation queue, drains, ShellHandle, queue_* callbacks.
thread_local! {
    /// The C++ `TheShell` singleton, with Rust-enforced exclusive access.
    static SHELL: RefCell<Shell> = RefCell::new(Shell::new());
    /// Re-entrant layout callbacks cannot acquire a second `&mut Shell`.  They
    /// enqueue owned work here, which the outer shell access drains before it
    /// releases its borrow.  This preserves a deterministic lifecycle boundary
    /// without creating aliasing mutable references.
    static SHELL_OPERATION_QUEUE: RefCell<Vec<Box<dyn FnOnce(&mut Shell) + 'static>>> =
        RefCell::new(Vec::new());
    static PENDING_SHELL_SCHEME: RefCell<Option<String>> = const { RefCell::new(None) };
    static SHELL_ANIM_FINISHED: Cell<bool> = const { Cell::new(true) };

}

fn drain_shell_operations(shell: &mut Shell) {
    loop {
        let mut operations =
            VecDeque::from(SHELL_OPERATION_QUEUE.with(|queue| queue.replace(Vec::new())));
        if operations.is_empty() {
            return;
        }

        while let Some(operation) = operations.pop_front() {
            // `pop_front` makes this exact closure the active operation; all
            // still-pending work stays owned in `operations` and is restored
            // by the unwind guard if a callback panics.
            let mut restore_remaining = RestoreShellOperationsOnUnwind {
                pending: &mut operations,
                armed: true,
            };
            operation(shell);
            restore_remaining.armed = false;
        }

        // Operations queued while this batch ran are the next FIFO batch at
        // the same outer shell boundary.
    }
}

/// Restores operations which have not started if the currently active one
/// unwinds.  Rust cannot resume those closures in the same stack after panic,
/// but they remain queued for the next safe shell boundary rather than being
/// silently discarded.
struct RestoreShellOperationsOnUnwind<'a> {
    pending: &'a mut VecDeque<Box<dyn FnOnce(&mut Shell) + 'static>>,
    armed: bool,
}

impl Drop for RestoreShellOperationsOnUnwind<'_> {
    fn drop(&mut self) {
        if self.armed && std::thread::panicking() && !self.pending.is_empty() {
            let mut remaining: Vec<_> = std::mem::take(self.pending).into_iter().collect();
            SHELL_OPERATION_QUEUE.with(|queue| {
                let mut queued = queue.borrow_mut();
                remaining.append(&mut queued);
                *queued = remaining;
            });
        }
    }
}

/// Ensures callback-originated operations are applied before the sole live
/// shell borrow is released, including when the callback unwinds.  During an
/// existing panic an operation failure is logged rather than causing a second
/// panic/abort; the original callback panic remains observable.
struct ShellOperationDrain<'a> {
    shell: &'a mut Shell,
}

impl Drop for ShellOperationDrain<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drain_shell_operations(self.shell);
            }));
            if result.is_err() {
                log::error!("queued shell operation panicked while a layout callback unwound");
            }
        } else {
            drain_shell_operations(self.shell);
        }
    }
}

/// Queue an owned shell operation.
///
/// If no shell lifecycle call is active, the operation executes immediately.
/// During `push`, `pop`, layout init/shutdown/update, or another scoped shell
/// access it is appended and drained immediately after that *outermost* access
/// returns from its callback—not deferred to a later UI frame.  Callers must
/// capture only owned data; a borrowed layout/window must not outlive its
/// callback stack.
pub fn queue_shell_operation(operation: impl FnOnce(&mut Shell) + 'static) {
    SHELL.with(|cell| {
        if let Ok(mut shell) = cell.try_borrow_mut() {
            drain_shell_operations(&mut shell);
            operation(&mut shell);
            drain_shell_operations(&mut shell);
        } else {
            SHELL_OPERATION_QUEUE.with(|queue| queue.borrow_mut().push(Box::new(operation)));
        }
    });
}

/// Run `f` with the one live mutable `Shell` reference.
///
/// Re-entrant access fails closed rather than manufacturing another `&mut`
/// reference.  Side effects needed from a layout callback must use
/// [`queue_shell_operation`].
pub fn with_shell_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Shell) -> R,
{
    SHELL.with(|cell| {
        let mut shell = cell.try_borrow_mut().ok()?;
        drain_shell_operations(&mut shell);
        let mut drain = ShellOperationDrain { shell: &mut shell };
        let result = f(&mut *drain.shell);
        // `ShellOperationDrain::drop` is the defined re-entry boundary: queued
        // work runs before the outer caller regains control and while the
        // original borrow is still the sole mutable reference.
        drop(drain);
        Some(result)
    })
}

/// C++ TheShell->isAnimFinished() from a layout callback that already holds
/// the live Shell borrow (Shell::update). Uses the snapshot taken at the
/// start of that update; otherwise reads the singleton when free.
pub fn shell_anim_finished_for_layout() -> bool {
    if SHELL.with(|cell| cell.try_borrow().is_err()) {
        return SHELL_ANIM_FINISHED.with(|flag| flag.get());
    }
    with_shell_ref(|shell| shell.is_anim_finished()).unwrap_or(true)
}

/// Run `f` with a shared shell reference when no mutable lifecycle call is active.
pub fn with_shell_ref<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Shell) -> R,
{
    SHELL.with(|cell| cell.try_borrow().ok().map(|shell| f(&shell)))
}

/// Backwards-compatible scoped mutation entry point.
///
/// This deliberately returns `None` during lifecycle re-entry.  It must not
/// be used for a mutation that has to happen after the callback; use
/// [`queue_shell_operation`] for that case.
pub fn try_with_shell_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Shell) -> R,
{
    with_shell_mut(f)
}

/// A non-borrowing compatibility façade for C++-shaped `TheShell` call sites.
///
/// Unlike the former `ShellMut`, this never dereferences a raw pointer or
/// exposes `DerefMut`.  New code should prefer [`with_shell_mut`],
/// [`with_shell_ref`], or [`queue_shell_operation`] because those make the
/// re-entry policy explicit.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShellHandle;

/// Get a non-borrowing handle to C++ `TheShell`.
pub fn get_shell() -> ShellHandle {
    ShellHandle
}

impl ShellHandle {
    pub fn is_shell_active(&self) -> bool {
        with_shell_ref(|shell| shell.is_shell_active()).unwrap_or(false)
    }

    pub fn is_shell_map_on(&self) -> bool {
        with_shell_ref(|shell| shell.is_shell_map_on()).unwrap_or(false)
    }

    pub fn is_anim_finished(&self) -> bool {
        with_shell_ref(|shell| shell.is_anim_finished()).unwrap_or(false)
    }

    pub fn get_screen_count(&self) -> usize {
        with_shell_ref(|shell| shell.get_screen_count()).unwrap_or(0)
    }

    pub fn top_filename(&self) -> Option<String> {
        with_shell_ref(|shell| shell.top_filename().map(str::to_owned)).flatten()
    }

    pub fn set_shell_active(&mut self, active: bool) {
        queue_shell_operation(move |shell| shell.set_shell_active(active));
    }

    pub fn show_shell_map(&mut self, on: bool) {
        queue_shell_operation(move |shell| shell.show_shell_map(on));
    }

    pub fn reverse_animate_window(&mut self) {
        queue_shell_operation(|shell| shell.reverse_animate_window());
    }

    pub fn push(&mut self, filename: &str, shutdown_immediate: bool) -> Result<(), ShellError> {
        let filename = filename.to_string();
        match with_shell_mut(|shell| shell.push(&filename, shutdown_immediate)) {
            Some(result) => result,
            None => {
                queue_shell_operation(move |shell| {
                    if let Err(error) = shell.push(&filename, shutdown_immediate) {
                        log::warn!("deferred Shell::push({filename}) failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn pop(&mut self) -> Result<(), ShellError> {
        match with_shell_mut(Shell::pop) {
            Some(result) => result,
            None => {
                queue_shell_operation(|shell| {
                    if let Err(error) = shell.pop() {
                        log::warn!("deferred Shell::pop failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn pop_immediate(&mut self) -> Result<(), ShellError> {
        match with_shell_mut(Shell::pop_immediate) {
            Some(result) => result,
            None => {
                queue_shell_operation(|shell| {
                    if let Err(error) = shell.pop_immediate() {
                        log::warn!("deferred Shell::pop_immediate failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn shutdown_complete(
        &mut self,
        _layout: Option<&dyn WindowLayout>,
        impending_push: bool,
    ) -> Result<(), ShellError> {
        match with_shell_mut(|shell| shell.shutdown_complete(None, impending_push)) {
            Some(result) => result,
            None => {
                queue_shell_operation(move |shell| {
                    if let Err(error) = shell.shutdown_complete(None, impending_push) {
                        log::warn!("deferred Shell::shutdown_complete failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn show_shell(&mut self, run_init: bool) -> Result<(), ShellError> {
        match with_shell_mut(|shell| shell.show_shell(run_init)) {
            Some(result) => result,
            None => {
                queue_shell_operation(move |shell| {
                    if let Err(error) = shell.show_shell(run_init) {
                        log::warn!("deferred Shell::show_shell failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn hide_shell(&mut self) -> Result<(), ShellError> {
        match with_shell_mut(Shell::hide_shell) {
            Some(result) => result,
            None => {
                queue_shell_operation(|shell| {
                    if let Err(error) = shell.hide_shell() {
                        log::warn!("deferred Shell::hide_shell failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match with_shell_mut(|shell| shell.init()) {
            Some(result) => result,
            None => {
                queue_shell_operation(|shell| {
                    if let Err(error) = shell.init() {
                        log::warn!("deferred Shell::init failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match with_shell_mut(|shell| shell.reset()) {
            Some(result) => result,
            None => {
                queue_shell_operation(|shell| {
                    if let Err(error) = shell.reset() {
                        log::warn!("deferred Shell::reset failed: {error}");
                    }
                });
                Ok(())
            }
        }
    }

    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match with_shell_mut(|shell| shell.update()) {
            Some(result) => result,
            None => Ok(()),
        }
    }
}

/// Enable shell map.  Nested lifecycle calls retain the request in the owned
/// operation queue instead of making a second mutable borrow.
pub fn show_shell_map_if_available(on: bool) {
    queue_shell_operation(move |shell| shell.show_shell_map(on));
}

/// Queue the C++ `TheShell->reverseAnimateWindow()` side effect.
pub fn queue_shell_reverse_animate_window() {
    queue_shell_operation(|shell| shell.reverse_animate_window());
}

/// Queue the C++ `TheShell->shutdownComplete()` side effect.
pub fn queue_shell_shutdown_complete(impending_push: bool) {
    queue_shell_operation(move |shell| {
        if let Err(error) = shell.shutdown_complete(None, impending_push) {
            log::warn!("deferred Shell::shutdown_complete failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->push()` side effect with owned layout data.
pub fn queue_shell_push(filename: impl Into<String>, shutdown_immediate: bool) {
    let filename = filename.into();
    queue_shell_operation(move |shell| {
        if let Err(error) = shell.push(&filename, shutdown_immediate) {
            log::warn!("deferred Shell::push({filename}) failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->pop()` side effect.
pub fn queue_shell_pop() {
    queue_shell_operation(|shell| {
        if let Err(error) = shell.pop() {
            log::warn!("deferred Shell::pop failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->hideShell()` side effect.
pub fn queue_shell_hide() {
    queue_shell_operation(|shell| {
        if let Err(error) = shell.hide_shell() {
            log::warn!("deferred Shell::hide_shell failed: {error}");
        }
    });
}

/// Queue the C++ `TheShell->showShell()` side effect.
pub fn queue_shell_show(run_init: bool) {
    queue_shell_operation(move |shell| {
        if let Err(error) = shell.show_shell(run_init) {
            log::warn!("deferred Shell::show_shell failed: {error}");
        }
    });
}

/// Queue registration of a window transition from a layout-init callback.
pub fn queue_shell_window_animation(
    window: Rc<RefCell<GameWindow>>,
    animation: AnimationType,
    needs_to_finish: bool,
    delay_ms: u64,
) {
    queue_shell_operation(move |shell| {
        shell.register_with_animate_manager(window, animation, needs_to_finish, delay_ms);
    });
}

pub fn request_shell_menu_scheme(name: &str) {
    let name = name.to_string();
    queue_shell_operation(move |shell| shell.load_scheme(&name));
    let _ = &PENDING_SHELL_SCHEME;
}
