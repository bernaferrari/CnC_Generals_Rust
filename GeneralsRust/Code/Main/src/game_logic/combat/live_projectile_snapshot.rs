//! Main's save bridge and tests for the simulation-owned projectile arena.
use super::*;

pub(crate) use generals_sim::combat::projectile_arena::{
    LiveProjectileArenaSnapshot, ValidatedLiveProjectileArena,
};

impl CombatSystem {
    pub(crate) fn live_projectile_arena_snapshot(&self) -> LiveProjectileArenaSnapshot {
        self.projectiles.snapshot()
    }

    pub(crate) fn restore_live_projectile_arena(&mut self, arena: ValidatedLiveProjectileArena) {
        self.projectiles.restore(arena);
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
    fn restored_world_identity_does_not_collide_with_standalone_cursor() {
        let mut saved = record();
        saved.next_projectile_id = saved.projectiles[0].id;
        let mut combat = CombatSystem::new();
        combat.restore_live_projectile_arena(saved.validate().unwrap());
        let id = combat.fire_projectile(
            Vec3::ZERO,
            Vec3::X,
            &Weapon::default(),
            ObjectId(7),
            None,
            10.0,
        );
        assert_eq!(id, ObjectId(100001));
        assert_eq!(combat.projectile_count(), 2);
    }
    #[test]
    fn malformed_arena_is_rejected_detached() {
        let mut duplicate = record();
        duplicate.projectiles.push(duplicate.projectiles[0].clone());
        assert!(duplicate.validate().is_err());
        let mut cursor = record();
        cursor.next_projectile_id = ObjectId(0);
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
