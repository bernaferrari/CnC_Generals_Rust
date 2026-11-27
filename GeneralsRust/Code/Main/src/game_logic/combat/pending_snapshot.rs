impl CombatSystem {
    pub(crate) fn pending_combat_snapshot(&self) -> PendingCombatSnapshot {
        self.shots.snapshot()
    }

    pub(crate) fn restore_pending_combat(&mut self, snapshot: &PendingCombatSnapshot) {
        // Restore after roster and fixups, without looking up the frozen source.
        self.fire_sound_loop_events.clear();
        self.shots.restore(snapshot);
    }
}

#[cfg(test)]
mod pending_launch_snapshot_tests {
    use super::*;

    #[test]
    fn queued_and_delayed_transforms_round_trip_and_reject_nonfinite_values() {
        let mut queued = tests::lifecycle_test_pending_projectile("Missile", None, Vec3::X);
        let queued_transform = glam::Mat4::from_cols_array(&[
            0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 11.5, -7.25, 3.0, 1.0,
        ]);
        let delayed_transform = glam::Mat4::from_translation(Vec3::new(-4.0, 8.0, 13.0));
        queued.source_context.as_mut().unwrap().launch_transform = Some(queued_transform);
        let mut delayed = queued.clone();
        delayed.source_context.as_mut().unwrap().launch_transform = Some(delayed_transform);
        let build_snapshot = |queued, delayed| {
            let mut queue = generals_sim::combat::ShotQueue::default();
            queue.accept(queued);
            queue.defer(LiveProjectilelessDelayedDamage::new(
                123,
                delayed,
                Vec3::X,
                None,
            ));
            queue.snapshot()
        };
        let snapshot = build_snapshot(queued.clone(), delayed.clone());
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded: PendingCombatSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        let mut restored = generals_sim::combat::ShotQueue::default();
        restored.restore(&decoded);
        let queued_restored = restored.take_accepted();
        let delayed_restored = restored.take_due(123, &mut 0).unwrap();
        assert_eq!(
            queued_restored[0]
                .source_context
                .unwrap()
                .launch_transform
                .unwrap()
                .to_cols_array(),
            queued_transform.to_cols_array()
        );
        assert_eq!(
            delayed_restored
                .pending
                .source_context
                .unwrap()
                .launch_transform
                .unwrap()
                .to_cols_array(),
            delayed_transform.to_cols_array()
        );
        assert!(decoded.validate_launch_transforms().is_ok());
        for delayed_case in [false, true] {
            let (mut invalid_queued, mut invalid_delayed) = (queued.clone(), delayed.clone());
            let pending = if delayed_case {
                &mut invalid_delayed
            } else {
                &mut invalid_queued
            };
            pending.source_context.as_mut().unwrap().launch_transform =
                Some(glam::Mat4::from_cols_array(&[f32::NAN; 16]));
            assert!(
                build_snapshot(invalid_queued, invalid_delayed)
                    .validate_launch_transforms()
                    .is_err()
            );
        }
    }
}
