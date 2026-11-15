// Typed Rust world-save records. C++ WeaponStore is not a Snapshot subsystem.
// These queues bridge host acceptance to later materialization; preserving
// their records does not claim the original C++ WeaponStore wire layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingCombatSnapshot {
    accepted: Vec<PendingProjectile>,
    delayed: Vec<LiveProjectilelessDelayedDamage>,
}

impl PendingCombatSnapshot {
    /// Validate both accepted and delayed records before committing a restore.
    pub(crate) fn validate_launch_transforms(&self) -> Result<(), String> {
        if self.accepted.iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .filter_map(|pending| pending.source_context.as_ref())
            .filter_map(|context| context.launch_transform)
            .any(|transform| !transform.is_finite())
        {
            return Err("Invalid pending projectile launch transform".into());
        }
        Ok(())
    }

    pub(crate) fn projectile_team_ids(&self) -> Vec<Option<u32>> {
        self.accepted
            .iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .map(|pending| {
                pending
                    .source_context
                    .and_then(|context| context.projectile_team_id)
            })
            .collect()
    }

    pub(crate) fn projectileless_weapon_details(&self) -> Vec<(Option<[f32; 3]>, Option<bool>)> {
        self.accepted
            .iter()
            .chain(self.delayed.iter().map(|entry| &entry.pending))
            .map(|pending| {
                (
                    pending
                        .self_damage_position
                        .map(|position| position.to_array()),
                    pending.selected_weapon_is_laser,
                )
            })
            .collect()
    }

    pub(crate) fn restore_projectileless_weapon_details(
        &mut self,
        details: &[(Option<[f32; 3]>, Option<bool>)],
    ) -> Result<(), String> {
        if details.len() != self.accepted.len() + self.delayed.len() {
            return Err("Pending projectileless weapon detail count mismatch".into());
        }
        if details.iter().any(|(position, _)| {
            position.is_some_and(|position| position.iter().any(|value| !value.is_finite()))
        }) {
            return Err("Invalid pending self-position damage location".into());
        }
        for (pending, (position, is_laser)) in self
            .accepted
            .iter_mut()
            .chain(self.delayed.iter_mut().map(|entry| &mut entry.pending))
            .zip(details.iter().copied())
        {
            pending.self_damage_position = position.map(Vec3::from_array);
            pending.selected_weapon_is_laser = is_laser;
        }
        Ok(())
    }

    pub(crate) fn restore_projectile_team_ids(
        &mut self,
        team_ids: &[Option<u32>],
    ) -> Result<(), String> {
        if team_ids.len() != self.accepted.len() + self.delayed.len() {
            return Err("Pending projectile Team identity count mismatch".into());
        }
        for (pending, team_id) in self
            .accepted
            .iter_mut()
            .chain(self.delayed.iter_mut().map(|entry| &mut entry.pending))
            .zip(team_ids.iter().copied())
        {
            if let Some(context) = &mut pending.source_context {
                context.projectile_team_id = team_id;
            } else if team_id.is_some() {
                return Err("Pending projectile Team identity has no launch context".into());
            }
        }
        Ok(())
    }
}

impl CombatSystem {
    pub(crate) fn pending_combat_snapshot(&self) -> PendingCombatSnapshot {
        PendingCombatSnapshot {
            accepted: self.pending_projectiles.clone(),
            delayed: self.projectileless_delayed.clone(),
        }
    }

    pub(crate) fn restore_pending_combat(&mut self, snapshot: &PendingCombatSnapshot) {
        // Commit after roster and fixups: a source can have been destroyed
        // before capture. Do not resolve it or restamp frozen launch data.
        self.fire_sound_loop_events.clear();
        self.pending_projectiles.clone_from(&snapshot.accepted);
        self.projectileless_delayed.clone_from(&snapshot.delayed);
    }
}

#[cfg(test)]
mod pending_launch_snapshot_tests {
    use super::*;

    #[test]
    fn queued_and_delayed_transforms_round_trip_and_reject_nonfinite_values() {
        let mut queued = tests::lifecycle_test_pending_projectile("Missile", None, Vec3::X);
        let queued_transform = glam::Mat4::from_cols_array(&[
            0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 11.5, -7.25, 3.0, 1.0,
        ]);
        let delayed_transform = glam::Mat4::from_translation(Vec3::new(-4.0, 8.0, 13.0));
        queued.source_context.as_mut().unwrap().launch_transform = Some(queued_transform);
        let mut delayed = queued.clone();
        delayed.source_context.as_mut().unwrap().launch_transform = Some(delayed_transform);
        let snapshot = PendingCombatSnapshot {
            accepted: vec![queued],
            delayed: vec![LiveProjectilelessDelayedDamage {
                when: 123,
                pending: delayed,
                damage_pos: Vec3::X,
                damage_id: None,
            }],
        };
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded: PendingCombatSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        assert_eq!(decoded.accepted[0].source_context.unwrap().launch_transform.unwrap().to_cols_array(), queued_transform.to_cols_array());
        assert_eq!(decoded.delayed[0].pending.source_context.unwrap().launch_transform.unwrap().to_cols_array(), delayed_transform.to_cols_array());
        assert!(decoded.validate_launch_transforms().is_ok());
        for delayed in [false, true] {
            let mut invalid = decoded.clone();
            let pending = if delayed { &mut invalid.delayed[0].pending } else { &mut invalid.accepted[0] };
            pending.source_context.as_mut().unwrap().launch_transform =
                Some(glam::Mat4::from_cols_array(&[f32::NAN; 16]));
            assert!(invalid.validate_launch_transforms().is_err());
        }
    }
}
