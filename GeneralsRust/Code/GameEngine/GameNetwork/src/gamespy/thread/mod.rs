//! GameSpy thread compatibility module
//!
//! Declares the GameSpy worker thread submodules used for asynchronous
//! network operations matching the original C++ threaded architecture.

pub mod buddy_thread;
pub mod game_results_thread;
pub mod peer_thread;
pub mod persistent_storage_thread;
pub mod ping_thread;
pub mod thread_utils;
