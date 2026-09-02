//! GameLogic-owned GameWorld last-writer authority context.
//!
//! C++ has a single `TheGameLogic` store; the Rust port mirrored the shadow
//! parity bridge's last-writer channels as `GENERALS_GAMEWORLD_*_AUTHORITY`
//! process-environment flags read at tick time (hq-e84zk). Environment is
//! process-global by nature — order-dependent and shared across every
//! `GameLogic` instance — so the authority decision now lives in a
//! per-instance context struct ([`GameWorldAuthority`]) owned by
//! [`GameLogic`](super::GameLogic), semantically identical to the C++
//! one-store rule: defaults are **all off** (host `GameLogic` is the sole
//! writer, commit 0c4d18623), and tests opt a channel in via explicit
//! setters on their own instance instead of mutating process environment.
//!
//! Deep readers (host object/combat/AI code that has no `&GameLogic` handle)
//! consult a thread-local snapshot of the currently executing instance,
//! mirroring C++'s `TheGameLogic` process-global access from deep subsystems.
//! `GameLogic::new()` publishes its (default) context so a fresh instance is
//! always a clean authority barrier, and every setter re-publishes.

use super::GameLogic;

/// Per-`GameLogic` GameWorld last-writer authority switches.
///
/// Every field is `false` by default: the host `GameLogic` store is the sole
/// writer (C++ single-store parity). Each `true` field enables the matching
/// GameWorld shadow last-writer channel while a coupled shadow session can
/// write back (see `gameworld_shadow::tick::authority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameWorldAuthority {
    /// HP last-writer (`GENERALS_GAMEWORLD_DAMAGE_AUTHORITY` successor).
    pub damage: bool,
    /// Player supplies/power last-writer (`*_ECONOMY_AUTHORITY` successor).
    pub economy: bool,
    /// Pose/path integration last-writer (`*_MOVEMENT_AUTHORITY` successor).
    pub movement: bool,
    /// Attack target / fire-intent last-writer (`*_AI_ATTACK_AUTHORITY` successor).
    pub ai_attack: bool,
    /// Projectile flight last-writer (`*_PROJECTILE_AUTHORITY` successor).
    pub projectile: bool,
    /// AI decision state last-writer (`*_AI_DECISION_AUTHORITY` successor).
    pub ai_decision: bool,
    /// Projectile spawn deferral last-writer (`*_FIRE_SPAWN_AUTHORITY` successor).
    pub fire_spawn: bool,
    /// Construction percent last-writer (`*_CONSTRUCTION_AUTHORITY` successor).
    pub construction: bool,
    /// Special-power cooldown last-writer (`*_SPECIAL_POWER_AUTHORITY` successor).
    pub special_power: bool,
    /// Production queue last-writer (`*_PRODUCTION_AUTHORITY` successor).
    pub production: bool,
    /// Weapon-slot refresh last-writer (`*_WEAPON_AUTHORITY` successor).
    pub weapon: bool,
}

impl GameWorldAuthority {
    /// All channels off — the C++-parity production default.
    pub const DEFAULT_OFF: GameWorldAuthority = GameWorldAuthority {
        damage: false,
        economy: false,
        movement: false,
        ai_attack: false,
        projectile: false,
        ai_decision: false,
        fire_spawn: false,
        construction: false,
        special_power: false,
        production: false,
        weapon: false,
    };
}

thread_local! {
    /// Authority snapshot of the `GameLogic` instance currently executing on
    /// this thread (the C++ `TheGameLogic` process-global counterpart for
    /// deep readers). Seeded with [`GameWorldAuthority::DEFAULT_OFF`].
    static CURRENT_AUTHORITY: std::cell::Cell<GameWorldAuthority> =
        const { std::cell::Cell::new(GameWorldAuthority::DEFAULT_OFF) };
}

/// Authority switches of the currently executing `GameLogic` context.
///
/// Deep host readers (object/combat/AI code without a `&GameLogic` handle)
/// resolve their last-writer gates through this snapshot instead of process
/// environment, so one test can no longer re-author another instance's
/// decision through `GENERALS_GAMEWORLD_*`.
#[inline]
pub fn current_gameworld_authority() -> GameWorldAuthority {
    CURRENT_AUTHORITY.with(|c| c.get())
}

/// Publish `authority` as this thread's current GameLogic authority context.
#[inline]
pub fn publish_gameworld_authority(authority: GameWorldAuthority) {
    CURRENT_AUTHORITY.with(|c| c.set(authority));
}

impl GameLogic {
    /// Per-instance GameWorld authority switches (read view).
    #[inline]
    pub fn gameworld_authority(&self) -> &GameWorldAuthority {
        &self.gameworld_authority
    }

    /// Mutate the per-instance authority switches and publish the snapshot to
    /// this thread's deep readers. The single sanctioned write path — the
    /// replacement for `GENERALS_GAMEWORLD_*_AUTHORITY` env writes.
    pub fn set_gameworld_authority(&mut self, apply: impl FnOnce(&mut GameWorldAuthority)) {
        apply(&mut self.gameworld_authority);
        publish_gameworld_authority(self.gameworld_authority);
    }

    /// Publish this instance's authority snapshot (fresh-instance barrier).
    #[inline]
    pub(crate) fn publish_gameworld_authority_context(&self) {
        publish_gameworld_authority(self.gameworld_authority);
    }
}

/// Named per-channel setters (test-visible opt-ins, one per retired env flag).
impl GameLogic {
    /// Opt GameWorld shadow in as HP last-writer.
    #[inline]
    pub fn set_damage_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.damage = on);
    }

    /// Opt GameWorld shadow in as supplies/power last-writer.
    #[inline]
    pub fn set_economy_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.economy = on);
    }

    /// Opt GameWorld shadow in as pose/path last-writer.
    #[inline]
    pub fn set_movement_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.movement = on);
    }

    /// Opt GameWorld shadow in as attack target / fire-intent last-writer.
    #[inline]
    pub fn set_ai_attack_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.ai_attack = on);
    }

    /// Opt GameWorld shadow in as projectile flight last-writer.
    #[inline]
    pub fn set_projectile_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.projectile = on);
    }

    /// Opt GameWorld shadow in as AI decision state last-writer.
    #[inline]
    pub fn set_ai_decision_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.ai_decision = on);
    }

    /// Opt GameWorld shadow in as projectile spawn deferral last-writer.
    #[inline]
    pub fn set_fire_spawn_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.fire_spawn = on);
    }

    /// Opt GameWorld shadow in as construction percent last-writer.
    #[inline]
    pub fn set_construction_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.construction = on);
    }

    /// Opt GameWorld shadow in as special-power cooldown last-writer.
    #[inline]
    pub fn set_special_power_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.special_power = on);
    }

    /// Opt GameWorld shadow in as production queue last-writer.
    #[inline]
    pub fn set_production_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.production = on);
    }

    /// Opt GameWorld shadow in as weapon-slot refresh last-writer.
    #[inline]
    pub fn set_weapon_authority(&mut self, on: bool) {
        self.set_gameworld_authority(|a| a.weapon = on);
    }
}
