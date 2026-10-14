//! Saved continuation of the receiving combat owner's materialized arena.
//! C++ Object::xfer delegates to MissileAIUpdate/DumbProjectileBehavior Xfer;
//! restoration installs their state rather than repeating launch callbacks.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LiveProjectileArenaSnapshot {
    pub next_projectile_id: ObjectId,
    pub projectiles: Vec<Projectile>,
}

impl Default for LiveProjectileArenaSnapshot {
    fn default() -> Self {
        Self {
            next_projectile_id: ObjectId(100000),
            projectiles: Vec::new(),
        }
    }
}

pub(crate) struct ValidatedLiveProjectileArena {
    next_projectile_id: ObjectId,
    projectiles: HashMap<ObjectId, Projectile>,
}

impl LiveProjectileArenaSnapshot {
    /// Detached validation. Missing launchers/victims are legitimate flight state.
    pub(crate) fn validate(self) -> Result<ValidatedLiveProjectileArena, String> {
        if self.next_projectile_id.0 == 0 {
            return Err("Invalid combat projectile allocation cursor".into());
        }
        let mut projectiles = HashMap::with_capacity(self.projectiles.len());
        for projectile in self.projectiles {
            if projectile.id.0 == 0 || projectile.id.0 >= self.next_projectile_id.0 {
                return Err("Invalid combat projectile identity/cursor".into());
            }
            validate_projectile(&projectile)?;
            if projectiles.insert(projectile.id, projectile).is_some() {
                return Err("Duplicate combat projectile identity".into());
            }
        }
        Ok(ValidatedLiveProjectileArena {
            next_projectile_id: self.next_projectile_id,
            projectiles,
        })
    }
}

fn validate_projectile(p: &Projectile) -> Result<(), String> {
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
        return Err("Nonfinite combat projectile continuation".into());
    }
    // A completed Dumb path legitimately retains step == len until its next update.
    if runtime.step > runtime.path.len() || p.lifetime < 0.0 || p.max_lifetime < 0.0 {
        return Err("Invalid combat projectile flight continuation".into());
    }
    // The shared Bezier producer emits exactly one point per segment. Empty
    // paths remain valid for unbound/legacy projectiles and Missile flight.
    if !runtime.path.is_empty() && runtime.path_segments != runtime.path.len() {
        return Err("Invalid combat projectile path segment count".into());
    }
    use crate::game_logic::weapon_bootstrap::HostProjectileFlight;
    let authored = match &p.flight {
        Some(HostProjectileFlight::Dumb(f)) => vec![
            f.first_height,
            f.second_height,
            f.first_percent_indent,
            f.second_percent_indent,
            f.flight_path_adjust_per_frame,
        ],
        Some(HostProjectileFlight::Missile(f)) => vec![f.lock_distance],
        None => Vec::new(),
    };
    if authored.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite authored projectile flight".into());
    }
    Ok(())
}

impl CombatSystem {
    pub(crate) fn live_projectile_arena_snapshot(&self) -> LiveProjectileArenaSnapshot {
        // Canonical wire order only. This does not choose simulation visitation
        // order: C++ sleepy heap topology integration remains hq-om90a.
        let mut projectiles: Vec<_> = self.projectiles.values().cloned().collect();
        projectiles.sort_unstable_by_key(|p| p.id.0);
        LiveProjectileArenaSnapshot {
            next_projectile_id: self.next_projectile_id,
            projectiles,
        }
    }

    /// No name resolution, rebinding, ignition, countermeasure reporting or FX.
    pub(crate) fn restore_live_projectile_arena(&mut self, arena: ValidatedLiveProjectileArena) {
        self.projectiles = arena.projectiles;
        self.next_projectile_id = arena.next_projectile_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> LiveProjectileArenaSnapshot {
        let mut p = Projectile::new(
            ObjectId(100000),
            Vec3::ZERO,
            Vec3::X,
            0.0,
            DamageType::Bullet,
            ObjectId(7),
            Some(ObjectId(8)),
        );
        p.no_damage = true;
        p.cm_reported = true;
        p.frames_till_decoyed = 42;
        p.leftover_exhaust_id = 19;
        LiveProjectileArenaSnapshot {
            next_projectile_id: ObjectId(100003),
            projectiles: vec![p],
        }
    }
    #[test]
    fn restore_preserves_receipts_and_allocation_without_launch() {
        let mut combat = CombatSystem::new();
        combat.restore_live_projectile_arena(record().validate().unwrap());
        let saved = combat.live_projectile_arena_snapshot();
        assert_eq!(saved.next_projectile_id, ObjectId(100003));
        let p = &saved.projectiles[0];
        assert!(p.no_damage && p.cm_reported);
        assert_eq!((p.frames_till_decoyed, p.leftover_exhaust_id), (42, 19));
        assert_eq!(p.target_id, Some(ObjectId(8)));
        assert!(p.flight.is_none());
    }
    #[test]
    fn malformed_arena_is_rejected_detached() {
        let mut duplicate = record();
        duplicate.projectiles.push(duplicate.projectiles[0].clone());
        assert!(duplicate.validate().is_err());
        let mut cursor = record();
        cursor.next_projectile_id = ObjectId(100000);
        assert!(cursor.validate().is_err());
        let mut nonfinite = record();
        nonfinite.projectiles[0].velocity.x = f32::NAN;
        assert!(nonfinite.validate().is_err());
        let mut path = record();
        path.projectiles[0].flight_runtime.step = 1;
        assert!(path.validate().is_err());
        assert!(LiveProjectileArenaSnapshot::default().validate().is_ok());
    }
    #[test]
    fn full_record_codec_and_wire_order_preserve_continuation() {
        let mut saved = record();
        let p = &mut saved.projectiles[0];
        p.flight = Some(
            crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(
                crate::game_logic::weapon_bootstrap::HostDumbProjectileFlight::default(),
            ),
        );
        p.flight_runtime.path = vec![Vec3::X, Vec3::Y];
        p.flight_runtime.path_segments = 2;
        p.flight_runtime.step = 2; // last point visited, next update detonates
        p.source_team_instance_name = "launch team".into();
        p.detonation_ocl_name = "owned OCL".into();
        p.missile_kill_self_started_frame = Some(99);
        let mut earlier = p.clone();
        earlier.id = ObjectId(99999);
        saved.projectiles.push(earlier);
        let bytes = bincode_legacy::serialize(&saved).unwrap();
        let decoded: LiveProjectileArenaSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        assert_eq!(bincode_legacy::serialize(&decoded).unwrap(), bytes);
        let mut combat = CombatSystem::new();
        combat.restore_live_projectile_arena(decoded.validate().unwrap());
        let canonical = combat.live_projectile_arena_snapshot();
        assert_eq!(
            canonical
                .projectiles
                .iter()
                .map(|p| p.id.0)
                .collect::<Vec<_>>(),
            vec![99999, 100000]
        );
        assert_eq!(
            bincode_legacy::serialize(&canonical.projectiles[1]).unwrap(),
            bincode_legacy::serialize(&saved.projectiles[0]).unwrap()
        );
        let mut invalid = canonical.clone();
        invalid.projectiles[0].flight_runtime.path_segments = 3;
        assert!(invalid.validate().is_err());
        let mut invalid = canonical;
        if let Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Dumb(f)) =
            &mut invalid.projectiles[0].flight
        {
            f.second_height = f32::INFINITY;
        }
        assert!(invalid.validate().is_err());
    }
}
