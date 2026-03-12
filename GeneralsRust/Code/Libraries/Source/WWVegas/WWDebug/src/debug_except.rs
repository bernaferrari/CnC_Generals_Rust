use crate::debug_debug::Debug;
use crate::debug_stack::{write_signature, DebugStackwalk, Signature};
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
    let mut dbg = Debug::instance().lock().unwrap();
    dbg.crash_begin(None, None);
    dbg.write_plain("\n========================================\n");
    dbg.write_plain("PANIC:\n");
    if let Some(location) = info.location() {
        dbg.write_plain(&format!(
            "at {}:{}:{}\n",
            location.file(),
            location.line(),
            location.column()
        ));
    }
    if let Some(payload) = info.payload().downcast_ref::<&str>() {
        dbg.write_plain(payload);
        dbg.write_plain("\n");
    } else if let Some(payload) = info.payload().downcast_ref::<String>() {
        dbg.write_plain(payload);
        dbg.write_plain("\n");
    }
    dbg.write_plain("\nStacktrace:\n");
    let mut sig = Signature::new();
    DebugStackwalk::stack_walk(&mut sig);
    write_signature(&mut dbg, &sig);
    dbg.crash_done(false);
}
