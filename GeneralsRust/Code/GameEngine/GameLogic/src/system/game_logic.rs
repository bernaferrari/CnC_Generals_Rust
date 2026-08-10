//! Dump kept so rustc does not treat a `system/game_logic/` directory as this module.
//!
//! Live implementation lives in `system/game_logic_impl/` and is wired from
//! `system/mod.rs` as:
//!
//! ```ignore
//! #[path = "game_logic_impl/mod.rs"]
//! pub mod game_logic;
//! ```
//!
//! Do not add `pub mod` items here; this file is intentionally unused.
