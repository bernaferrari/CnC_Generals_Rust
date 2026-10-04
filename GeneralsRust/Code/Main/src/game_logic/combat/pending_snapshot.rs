// Typed Rust world-save records. C++ WeaponStore is not a Snapshot subsystem.
// These queues bridge host acceptance to later materialization; preserving
// their records does not claim the original C++ WeaponStore wire layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingCombatSnapshot {
    accepted: Vec<PendingProjectile>,
    delayed: Vec<LiveProjectilelessDelayedDamage>,
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
        self.pending_projectiles.clone_from(&snapshot.accepted);
        self.projectileless_delayed.clone_from(&snapshot.delayed);
    }
}
