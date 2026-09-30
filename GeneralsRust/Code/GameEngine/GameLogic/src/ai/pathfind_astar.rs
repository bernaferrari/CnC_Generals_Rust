//! Compatibility surface for the extracted pathfinding algorithm crate.
//!
//! Types are re-exported directly so existing GameLogic and Main callers keep
//! using the canonical `game_pathfinding` type identities.

pub use game_pathfinding::*;
