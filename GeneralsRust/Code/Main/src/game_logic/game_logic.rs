//! Facade sources moved to `game_logic/`.
//!
//! This path is kept so `include_str!(".../game_logic.rs")` still compiles.
//! Scan the live split via `crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC`
//! or the residuals/gameworld_shadow `GAME_LOGIC_HOST_SRC` concat (replace this
//! file with the six `game_logic/*.rs` members in source order).
//!
//! Parallel agent: update `gameworld_shadow/tests/harness.rs` first include from
//! `include_str!("../../game_logic/game_logic.rs")` to:
//!   include_str!("../../game_logic/game_logic/crate_tick.rs"),
//!   include_str!("../../game_logic/game_logic/player.rs"),
//!   include_str!("../../game_logic/game_logic/host.rs"),
//!   include_str!("../../game_logic/game_logic/script_camera.rs"),
//!   include_str!("../../game_logic/game_logic/authority.rs"),
//!   include_str!("../../game_logic/game_logic/construct.rs"),
//!   include_str!("../../game_logic/game_logic/mod.rs"),
//! Same replacement for the two `include_str!` sites in
//! `gameworld_shadow/tests/authority_writeback.rs`.
