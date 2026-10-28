// Typed Rust world-save records. C++ WeaponStore is not a Snapshot subsystem.
// These queues bridge host acceptance to later materialization; preserving
// their records does not claim the original C++ WeaponStore wire layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingCombatSnapshot {
    accepted: Vec<PendingProjectile>,
    delayed: Vec<LiveProjectilelessDelayedDamage>,
}

impl PendingCombatSnapshot {
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
