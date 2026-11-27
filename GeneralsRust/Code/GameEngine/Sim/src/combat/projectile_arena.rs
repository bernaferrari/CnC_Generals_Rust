//! Arena membership, weapon-bonus admission, and detached save validation.

use super::projectile::Projectile;
use super::projectile_flight::ProjectileFlight;
use crate::object::ObjectIds;
use generals_game_domain::ObjectId;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

const STANDALONE_PROJECTILE_START: ObjectId = ObjectId(100_000);

/// A rejected serialized projectile arena. The receiving world is untouched.
#[derive(Debug, Error)]
pub enum ProjectileArenaError {
    #[error("Invalid combat projectile allocation cursor")]
    Cursor,
    #[error("Invalid combat projectile identity")]
    Identity,
    #[error("Duplicate combat projectile identity")]
    DuplicateIdentity,
    #[error("Nonfinite combat projectile continuation")]
    NonfiniteContinuation,
    #[error("Invalid combat projectile bonus continuation")]
    BonusContinuation,
    #[error("Invalid combat projectile flight continuation")]
    FlightContinuation,
    #[error("Invalid combat projectile path segment count")]
    PathSegmentCount,
    #[error("Nonfinite authored projectile flight")]
    NonfiniteFlight,
}

/// Current save record in canonical wire order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProjectileArenaSnapshot {
    pub next_projectile_id: ObjectId,
    pub projectiles: Vec<Projectile>,
}

impl Default for LiveProjectileArenaSnapshot {
    fn default() -> Self {
        Self {
            next_projectile_id: STANDALONE_PROJECTILE_START,
            projectiles: Vec::new(),
        }
    }
}

/// A detached record that has cleared projectile identity and flight checks.
pub struct ValidatedLiveProjectileArena {
    next_projectile_id: ObjectId,
    projectiles: HashMap<ObjectId, Projectile>,
}

impl LiveProjectileArenaSnapshot {
    /// Detached validation. Missing launchers/victims are legitimate flight state.
    pub fn validate(self) -> Result<ValidatedLiveProjectileArena, ProjectileArenaError> {
        if self.next_projectile_id.0 == 0 {
            return Err(ProjectileArenaError::Cursor);
        }
        let mut projectiles = HashMap::with_capacity(self.projectiles.len());
        for projectile in self.projectiles {
            // Production projectile IDs come from the world's object cursor,
            // not this arena's standalone allocation cursor.
            if projectile.id.0 == 0 {
                return Err(ProjectileArenaError::Identity);
            }
            validate_projectile(&projectile)?;
            if projectiles.insert(projectile.id, projectile).is_some() {
                return Err(ProjectileArenaError::DuplicateIdentity);
            }
        }
        Ok(ValidatedLiveProjectileArena {
            next_projectile_id: self.next_projectile_id,
            projectiles,
        })
    }
}

fn validate_projectile(p: &Projectile) -> Result<(), ProjectileArenaError> {
    let runtime = &p.flight_runtime;
    let vectors = [
        p.position,
        p.velocity,
        p.target_position,
        runtime.path_start,
        runtime.path_end,
        runtime.original_target_pos,
    ];
    let scalars = [
        p.damage,
        p.speed,
        p.lifetime,
        p.max_lifetime,
        p.explosion_radius,
        p.secondary_damage,
        p.secondary_damage_radius,
        p.shock_wave_amount,
        p.shock_wave_radius,
        p.shock_wave_taper_off,
        p.historic_bonus_radius,
        runtime.path_speed_per_frame,
    ];
    if vectors.iter().any(|v| !v.is_finite())
        || scalars.iter().any(|v| !v.is_finite())
        || runtime.path.iter().any(|v| !v.is_finite())
    {
        return Err(ProjectileArenaError::NonfiniteContinuation);
    }
    if p.computed_bonus_mask
        .is_some_and(|mask| mask != (p.launcher_bonus_mask | p.projectile_bonus_mask))
    {
        return Err(ProjectileArenaError::BonusContinuation);
    }
    // A completed Dumb path legitimately retains step == len until its next update.
    if runtime.step > runtime.path.len() || p.lifetime < 0.0 || p.max_lifetime < 0.0 {
        return Err(ProjectileArenaError::FlightContinuation);
    }
    // The shared Bezier producer emits exactly one point per segment. Empty
    // paths remain valid for unbound/legacy projectiles and Missile flight.
    if !runtime.path.is_empty() && runtime.path_segments != runtime.path.len() {
        return Err(ProjectileArenaError::PathSegmentCount);
    }
    let dumb_parameters;
    let authored = match &p.flight {
        Some(ProjectileFlight::Dumb(f)) => {
            dumb_parameters = [
                f.first_height,
                f.second_height,
                f.first_percent_indent,
                f.second_percent_indent,
                f.flight_path_adjust_per_frame,
            ];
            &dumb_parameters[..]
        }
        Some(ProjectileFlight::Missile(f)) => std::slice::from_ref(&f.lock_distance),
        None => &[],
    };
    if authored.iter().any(|v| !v.is_finite()) {
        return Err(ProjectileArenaError::NonfiniteFlight);
    }
    Ok(())
}

/// Live projectiles of one receiving simulation.
#[derive(Debug)]
pub struct ProjectileArena {
    active: HashMap<ObjectId, Projectile>,
    pending_weapon_bonus: FxHashSet<ObjectId>,
    standalone_ids: ObjectIds,
}

impl Default for ProjectileArena {
    fn default() -> Self {
        Self {
            active: HashMap::new(),
            pending_weapon_bonus: FxHashSet::default(),
            standalone_ids: ObjectIds::from_next(STANDALONE_PROJECTILE_START),
        }
    }
}

impl ProjectileArena {
    pub fn as_map(&self) -> &HashMap<ObjectId, Projectile> {
        &self.active
    }

    pub fn get_mut(&mut self, id: &ObjectId) -> Option<&mut Projectile> {
        self.active.get_mut(id)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Projectile> {
        self.active.values_mut()
    }

    /// Standalone shots use this cursor. Registered flights supply their Object ID.
    pub fn allocate_standalone(&mut self) -> ObjectId {
        self.standalone_ids
            .allocate(|id| self.active.contains_key(&id))
    }

    pub fn admit(&mut self, projectile: Projectile) {
        let id = projectile.id;
        let std::collections::hash_map::Entry::Vacant(entry) = self.active.entry(id) else {
            panic!("duplicate live projectile identity");
        };
        entry.insert(projectile);
        self.pending_weapon_bonus.insert(id);
    }

    pub fn visit_uninitialized_bonuses(&mut self, mut visit: impl FnMut(&mut Projectile)) {
        for id in self.pending_weapon_bonus.drain() {
            if let Some(projectile) = self
                .active
                .get_mut(&id)
                .filter(|projectile| !projectile.projectile_upgrades_initialized)
            {
                visit(projectile);
            }
        }
    }

    pub fn remove(&mut self, id: ObjectId) -> bool {
        self.pending_weapon_bonus.remove(&id);
        self.active.remove(&id).is_some()
    }

    /// Keep the standalone cursor when clearing just the active shots.
    pub fn clear(&mut self) {
        self.active.clear();
        self.pending_weapon_bonus.clear();
    }

    pub fn snapshot(&self) -> LiveProjectileArenaSnapshot {
        // Wire order is independent of the update visitation order.
        let mut projectiles: Vec<_> = self.active.values().cloned().collect();
        projectiles.sort_unstable_by_key(|p| p.id.0);
        LiveProjectileArenaSnapshot {
            next_projectile_id: self.standalone_ids.next(),
            projectiles,
        }
    }

    /// Restore active shots without replaying launch, FX, or bonus callbacks.
    pub fn restore(&mut self, arena: ValidatedLiveProjectileArena) {
        self.pending_weapon_bonus.clear();
        self.pending_weapon_bonus.extend(
            arena
                .projectiles
                .values()
                .filter(|projectile| !projectile.projectile_upgrades_initialized)
                .map(|projectile| projectile.id),
        );
        self.active = arena.projectiles;
        self.standalone_ids = ObjectIds::from_next(arena.next_projectile_id);
    }

    #[cfg(feature = "test-projectile-arena")]
    pub fn take_fixture_cursor(&self) -> ObjectIds {
        ObjectIds::from_next(self.standalone_ids.next())
    }

    #[cfg(feature = "test-projectile-arena")]
    pub fn set_fixture_cursor(&mut self, cursor: ObjectIds) {
        self.standalone_ids = cursor;
    }

    /// Install a flight fixture without scheduling unused admission callbacks.
    #[cfg(feature = "test-projectile-arena")]
    pub fn insert_fixture(&mut self, id: ObjectId, projectile: Projectile) {
        self.active.insert(id, projectile);
    }
}
