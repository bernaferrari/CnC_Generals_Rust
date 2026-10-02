//! Review index for the typed update-interface accessors.
//!
//! This file intentionally holds no code. It documents where the overrides
//! that replaced `ModuleUpdateProxy`'s former `as_any().downcast_mut` dispatch
//! tables now live.
//!
//! Background: in C++ every update module derives `UpdateModule`, so
//! `Module::DynamicInterfaceCast(ModuleInterfaceType::UPDATE)` reaches the
//! per-frame hooks through the vtable. The port stores modules type-erased as
//! `Box<dyn Module>` (the `ModuleFactory` boundary), so the same query is
//! expressed as overridden accessor methods on `Module`:
//!
//! * `Module::get_update_module_interface()` — per-frame update dispatch.
//!   Overridden by the wrapper modules that the former `dispatch_update`
//!   table listed, each returning `Some(self.behavior_mut())`. Direct
//!   implementors: `SlowDeathBehavior` and `SpecialPowerUpdateModule`
//!   (the type *is* the interface) and `OCLUpdateModule` (returns its wrapped
//!   `OCLUpdate`).
//! * `Module::get_sleepy_update_interface()` — the narrower set that also
//!   answered `get_disabled_types_to_process()` / `get_update_phase()`. Kept
//!   separate because the old tables did not register every update module for
//!   those two queries; widening the set would change which modules wake while
//!   their object is disabled.
//! * `Module::update_module_behind_shared_lock()` —
//!   `PropagandaCenterBehaviorModule`, whose behavior sits behind a `Mutex`
//!   and therefore cannot hand out `&mut dyn UpdateModuleInterface`.
//! * `Module::get_initial_wake_frame()` — the former
//!   `initial_update_wake_frame` downcast chain.
//!
//! Rust forbids a second `impl Module for T` in a separate file, so each
//! override lives in the wrapper module's own `impl Module` block (search for
//! `get_update_module_interface`, `get_sleepy_update_interface`,
//! `get_initial_wake_frame`, `update_module_behind_shared_lock`).
//!
//! The generic `ActiveBehaviorModule<T>` wrapper forwards
//! `get_update_module_interface()` / `get_initial_wake_frame()` to explicit
//! per-instantiation opt-ins on `BehaviorModuleInterface`
//! (`get_sleepy_update_interface`, `behavior_initial_wake_frame`), so only the
//! instantiations the old tables listed participate.
//!
//! Observable behavior is unchanged: the override set is exactly the set of
//! types the tables matched, and a module that matched no branch still returns
//! `None` (the "No update dispatcher" warning path).
