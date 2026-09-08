//! Experience and Veterancy System (C++ ExperienceTracker).
//!
//! # Components
//!
//! - **ExperienceTracker** (`tracker.rs`): direct port of the C++
//!   `ExperienceTracker` owned by every Object — level, current experience,
//!   sink forwarding, scalar, and the `xfer` layout
//!   (ExperienceTracker.cpp:222-245). Veterancy thresholds always come from
//!   the owner template (`getTemplate()->getExperienceRequired`,
//!   ExperienceTracker.cpp:75, 91, 106, 152, 192); the degraded fail-closed
//!   fallback used only when the owner template is unresolvable is documented
//!   on [`ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED`].
//! - **ExperienceRequirements** (`requirements.rs`): container for explicit
//!   threshold arrays. Its `default_requirements` table is a Rust-only
//!   degraded placeholder, not C++ data.
//! - **Promotion visuals** (`visual.rs`): Rust presentation helper for the
//!   level-change edge trigger (C++ edge-triggers
//!   `Object::onVeterancyLevelChanged`, ExperienceTracker.cpp:161-165);
//!   actual C++ feedback is data-driven (2D level-gain animation from
//!   GlobalData, Object.cpp:3126-3140).
//!
//! # Deliberately absent (non-parity code removed — do not re-add without a
//! C++ citation)
//!
//! C++ has no damage-based XP, no squad XP sharing, no cost-scaled XP
//! thresholds, and no hardcoded veterancy stat multipliers:
//! - Kill XP comes from the victim template's experience-value table via
//!   scoreTheKill; there is no `onDamageDealt` XP.
//! - Veterancy stat effects are data-driven: WeaponSet/WeaponBonus
//!   conditions (Object.cpp:3091-3124), health bonus from GlobalData
//!   `m_healthBonus` (ActiveBody.cpp:1442-1450), armor sets
//!   (ActiveBody.cpp:1455-1477).
//! - Sink forwarding scales by the source scalar unconditionally
//!   (ExperienceTracker.cpp:133).
//!
//! The former `ExperienceGainManager` / `VeterancyBonuses` /
//! stat-calculator subsystems encoded the opposite behaviors (cost-scaled
//! thresholds, fixed +25%/−10%/+25% multipliers, gated sink scaling, squad
//! sharing) and were removed.

mod requirements;
mod tracker;
mod visual;

pub use requirements::*;
pub use tracker::*;
pub use visual::*;
