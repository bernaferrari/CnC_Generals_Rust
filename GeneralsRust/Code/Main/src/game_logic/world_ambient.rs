//! Host thread-local ambient WorldServices / EngineStores publication stack.
//!
//! GameLogic free-function bridges resolve through hooks registered here so the
//! GameLogic crate itself carries no production `thread_local!` binder (ownership
//! meter). Parallel skirmish threads each keep an independent stack.

use gamelogic::system::engine_stores::{
    register_ambient_stack_hooks, set_ambient_pre_ensure, AmbientStackHooks, PublishedOwner,
};
use game_engine::common::ini::ini_ai_data::AIDataStore;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};

thread_local! {
    static SCOPED_ACTIVE: RefCell<Vec<PublishedOwner>> = const { RefCell::new(Vec::new()) };
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

/// Idempotent: register Main's TLS ambient stack with GameLogic.
pub fn ensure_registered() {
    bind_pre_ensure();
    register_hooks();
}

/// Bind pre-ensure so the first GameLogic ambient touch self-registers.
pub fn bind_pre_ensure() {
    set_ambient_pre_ensure(register_hooks);
}
