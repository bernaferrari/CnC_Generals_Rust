//! Coupled-tick, authority env caches, and eager host residual apply.
//!
//! Split from the former `gameworld_shadow/tick.rs` god-file. Public names
//! stay identical so `pub use tick::*;` keeps working.

mod authority;
mod couple;
mod dispatch;
mod eager_ai;
mod eager_combat;
mod eager_contain;
mod eager_economy;
mod eager_identity;
mod eager_misc;
mod eager_orders;
mod eager_stealth;
mod eager_weapon;
mod env;
mod status_timers;
mod status_timers_death;
mod status_timers_economy;
mod status_timers_payload;
mod status_timers_post;
mod status_timers_projectiles;
mod status_timers_specials;
mod status_timers_stealth;
mod status_timers_structure;
mod status_timers_updates;

pub use authority::*;
pub use couple::*;
pub use dispatch::*;
pub use eager_ai::*;
pub use eager_combat::*;
pub use eager_contain::*;
pub use eager_economy::*;
pub use eager_identity::*;
pub use eager_misc::*;
pub use eager_orders::*;
pub use eager_stealth::*;
pub use eager_weapon::*;
pub use env::*;
pub use status_timers::*;
