//! Owned fixtures for unit tests. No process-wide UI / window manager state.

use crate::core::subsystems::InGameUISubsystem;
use crate::message_stream::ReceivingUi;
use game_engine::common::ini::ini_ai_data::AIDataStore;
use gamelogic::system::engine_stores::{
    install_active, new_for_world, register_ambient_stack_hooks, set_ambient_pre_ensure,
    AmbientStackHooks, EngineStores, PublishedOwner,
};
use std::cell::RefCell;
use std::sync::{Arc, Once, RwLock};

/// Run `f` with a stack-owned InGameUISubsystem exposed as ReceivingUi.
pub fn with_receiving_ui<R>(f: impl FnOnce(&mut ReceivingUi<'_>) -> R) -> R {
    ensure_ambient_host();
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, 0, 0);
    f(&mut ui)
}

/// Same as [`with_receiving_ui`], with explicit logic/client frames.
pub fn with_receiving_ui_at<R>(
    logic_frame: u32,
    client_frame: u32,
    f: impl FnOnce(&mut ReceivingUi<'_>) -> R,
) -> R {
    ensure_ambient_host();
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, logic_frame, client_frame);
    f(&mut ui)
}

/// Register GameClient's test TLS ambient stack with GameLogic and seed a
/// per-thread default Core world when the stack is empty.
///
/// GameLogic's `#[cfg(test)]` ambient backend only links when GameLogic itself
/// is under test; dependents must host the stack. Prefer
/// [`gamelogic::system::engine_stores::with_active_stores`] for new tests that
/// need an explicit world; this seed keeps pre-meter-0 coverage running.
pub fn ensure_ambient_host() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_ambient_pre_ensure(register_hooks);
        register_hooks();
    });
    seed_default_core_if_empty();
}

thread_local! {
    static SCOPED_ACTIVE: RefCell<Vec<PublishedOwner>> = const { RefCell::new(Vec::new()) };
    static DEFAULT_CORE: RefCell<Option<Arc<EngineStores>>> = const { RefCell::new(None) };
}

fn seed_default_core_if_empty() {
    if depth() > 0 {
        return;
    }
    DEFAULT_CORE.with(|slot| {
        if slot.borrow().is_some() && depth() > 0 {
            return;
        }
        let core = new_for_world();
        install_active(Arc::clone(&core));
        *slot.borrow_mut() = Some(core);
    });
}

fn head() -> Option<PublishedOwner> {
    SCOPED_ACTIVE.with(|stack| stack.borrow().last().cloned())
}

fn push(owner: PublishedOwner) {
    SCOPED_ACTIVE.with(|stack| stack.borrow_mut().push(owner));
}

fn pop_if_head(owner: &PublishedOwner) -> bool {
    SCOPED_ACTIVE.with(|stack| {
        let mut active = stack.borrow_mut();
        if active.last().is_some_and(|h| h.same(owner)) {
            active.pop();
            true
        } else {
            false
        }
    })
}

fn retain_not(owner: &PublishedOwner) {
    SCOPED_ACTIVE.with(|stack| stack.borrow_mut().retain(|entry| !entry.same(owner)));
}

fn prior_core_ai_data() -> Option<Arc<RwLock<AIDataStore>>> {
    SCOPED_ACTIVE.with(|stack| {
        stack.borrow().iter().rev().find_map(|entry| match entry {
            PublishedOwner::Core(core) => Some(Arc::clone(core.ai_data())),
            PublishedOwner::Services(_) => None,
        })
    })
}

fn depth() -> usize {
    SCOPED_ACTIVE.with(|stack| stack.borrow().len())
}

fn register_hooks() {
    register_ambient_stack_hooks(AmbientStackHooks {
        head,
        push,
        pop_if_head,
        retain_not,
        prior_core_ai_data,
        depth,
    });
}

#[cfg(test)]
#[ctor::ctor]
fn boot_ambient_host() {
    ensure_ambient_host();
}
