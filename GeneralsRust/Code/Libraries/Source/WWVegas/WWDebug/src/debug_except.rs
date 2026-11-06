use once_cell::sync::OnceCell;
use std::panic::{self, PanicInfo};

static HOOK_INSTALLED: OnceCell<()> = OnceCell::new();

pub struct DebugExceptionhandler;

impl DebugExceptionhandler {
    pub fn install_exception_handler() {
        if HOOK_INSTALLED.get().is_some() {
            return;
        }
        let _ = HOOK_INSTALLED.set(());
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            handle_panic(info);
            previous(info);
        }));
    }
}

fn handle_panic(info: &PanicInfo) {
    // Process-level panic hook cannot borrow a game-instance WwDebugSession.
    // Emit to stderr; hosts that need Debug IO should install their own hook.
    eprintln!("========================================");
    eprintln!("PANIC:");
    if let Some(location) = info.location() {
        eprintln!(
            "at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }
    if let Some(payload) = info.payload().downcast_ref::<&str>() {
        eprintln!("{payload}");
    } else if let Some(payload) = info.payload().downcast_ref::<String>() {
        eprintln!("{payload}");
    }
    eprintln!("========================================");
}
