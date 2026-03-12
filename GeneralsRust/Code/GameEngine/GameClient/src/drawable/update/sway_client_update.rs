//! SwayClientUpdate wrapper (GameClient/Drawable/Update/SwayClientUpdate.cpp).
//!
//! The full implementation lives in GameLogic; we re-export it here to keep the
//! GameClient file layout faithful without duplicating logic.

pub use gamelogic::object::update::SwayClientUpdateModule;
