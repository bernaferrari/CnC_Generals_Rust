//! Host combat particle feedback registry.
//!
//! Residual (hq-gq7n slice): weapon fire and death create *real* particle-system
//! registry entries that PresentationFrame / client can observe — not log-only
//! placeholders. Active host systems are also captured in
//! `WorldSnapshot.combat_particles` for save/load residual.
//!
//! Fail-closed: not full W3D GPU particle parity or client ParticleSystemManager
//! rebind after load.

use super::{ObjectId, Team};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kind of combat feedback particle system (host registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatParticleKind {
    /// Death blast at destroyed unit/structure.
    DeathExplosion,
    /// Lingering smoke after death.
    DeathSmoke,
    /// Muzzle flash when a weapon fires.
    WeaponMuzzleFlash,
    /// Impact / hit feedback at target position.
    WeaponImpact,
}

impl CombatParticleKind {
    /// Template name matching GameClient particle_presets where applicable.
    pub fn template_name(self) -> &'static str {
        match self {
            CombatParticleKind::DeathExplosion => "MediumExplosion",
            CombatParticleKind::DeathSmoke => "SmokePlume",
            CombatParticleKind::WeaponMuzzleFlash => "MuzzleFlash",
            CombatParticleKind::WeaponImpact => "BulletImpact",
        }
    }
}

/// One active combat particle system entry in the host registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatParticleSystemEntry {
    pub id: u32,
    pub kind: CombatParticleKind,
    pub template_name: String,
    pub position: Vec3,
    pub source_object: Option<ObjectId>,
    pub target_object: Option<ObjectId>,
    pub spawned_frame: u32,
    pub active: bool,
    /// Optional mirror id in GameClient ParticleSystemManager (when bridged).
    pub client_system_id: Option<u32>,
}

/// Lightweight host particle system registry for combat/build feedback.
///
/// Independent of WGPU so unit tests and headless golden host paths can assert
/// that kills/fires produce registry entries.
#[derive(Debug, Clone, Default)]
pub struct CombatParticleRegistry {
    next_id: u32,
    systems: HashMap<u32, CombatParticleSystemEntry>,
    /// Destruction notifications for PresentationFrame events this frame.
    destroyed_this_frame: Vec<(ObjectId, Team)>,
    /// Particle ids spawned this frame (presentation event drain).
    spawned_this_frame: Vec<u32>,
}

impl CombatParticleRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            systems: HashMap::new(),
            destroyed_this_frame: Vec::new(),
            spawned_this_frame: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.systems.clear();
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
        self.next_id = 1;
    }

    /// Allocator cursor for next system id (survives save/load).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Replace registry contents from a save/load snapshot.
    ///
    /// Frame-local drains (`destroyed_this_frame` / `spawned_this_frame`) are
    /// cleared. Client mirror ids are preserved as stored (may be stale after
    /// load — presentation rebinds residual, not full GPU particle parity).
    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        systems: impl IntoIterator<Item = CombatParticleSystemEntry>,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for entry in systems {
            max_id = max_id.max(entry.id);
            self.systems.insert(entry.id, entry);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
    }

    pub fn active_count(&self) -> usize {
        self.systems.values().filter(|s| s.active).count()
    }

    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    pub fn get(&self, id: u32) -> Option<&CombatParticleSystemEntry> {
        self.systems.get(&id)
    }

    pub fn active_systems(&self) -> impl Iterator<Item = &CombatParticleSystemEntry> {
        self.systems.values().filter(|s| s.active)
    }

    pub fn systems_snapshot(&self) -> Vec<CombatParticleSystemEntry> {
        let mut v: Vec<_> = self.systems.values().cloned().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn systems_of_kind(&self, kind: CombatParticleKind) -> Vec<&CombatParticleSystemEntry> {
        self.systems
            .values()
            .filter(|s| s.active && s.kind == kind)
            .collect()
    }

    pub fn note_destroyed(&mut self, id: ObjectId, team: Team) {
        self.destroyed_this_frame.push((id, team));
    }

    pub fn take_destroyed_this_frame(&mut self) -> Vec<(ObjectId, Team)> {
        std::mem::take(&mut self.destroyed_this_frame)
    }

    pub fn destroyed_this_frame(&self) -> &[(ObjectId, Team)] {
        &self.destroyed_this_frame
    }

    pub fn spawned_this_frame(&self) -> &[u32] {
        &self.spawned_this_frame
    }

    pub fn clear_frame_events(&mut self) {
        self.destroyed_this_frame.clear();
        self.spawned_this_frame.clear();
    }

    /// Spawn a combat particle system entry. Returns the host registry id.
    pub fn spawn(
        &mut self,
        kind: CombatParticleKind,
        position: Vec3,
        frame: u32,
        source: Option<ObjectId>,
        target: Option<ObjectId>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);

        let template_name = kind.template_name().to_string();
        let client_system_id = mirror_spawn_to_client_manager(&template_name, position);

        let entry = CombatParticleSystemEntry {
            id,
            kind,
            template_name,
            position,
            source_object: source,
            target_object: target,
            spawned_frame: frame,
            active: true,
            client_system_id,
        };
        self.systems.insert(id, entry);
        self.spawned_this_frame.push(id);
        id
    }

    /// Death feedback: explosion + smoke at the corpse position.
    pub fn spawn_death_fx(
        &mut self,
        position: Vec3,
        frame: u32,
        victim: ObjectId,
        is_structure: bool,
        victim_team: Team,
    ) -> Vec<u32> {
        self.note_destroyed(victim, victim_team);
        let explosion_kind = if is_structure {
            CombatParticleKind::DeathExplosion
        } else {
            CombatParticleKind::DeathExplosion
        };
        let mut ids = Vec::with_capacity(2);
        ids.push(self.spawn(explosion_kind, position, frame, Some(victim), None));
        ids.push(self.spawn(
            CombatParticleKind::DeathSmoke,
            position,
            frame,
            Some(victim),
            None,
        ));
        ids
    }

    /// Weapon fire feedback: muzzle flash at shooter, optional impact at target.
    pub fn spawn_weapon_fire_fx(
        &mut self,
        muzzle_pos: Vec3,
        impact_pos: Option<Vec3>,
        frame: u32,
        shooter: ObjectId,
        target: Option<ObjectId>,
    ) -> Vec<u32> {
        let mut ids = Vec::with_capacity(2);
        ids.push(self.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            muzzle_pos,
            frame,
            Some(shooter),
            target,
        ));
        if let Some(impact) = impact_pos {
            ids.push(self.spawn(
                CombatParticleKind::WeaponImpact,
                impact,
                frame,
                Some(shooter),
                target,
            ));
        }
        ids
    }

    pub fn deactivate(&mut self, id: u32) {
        if let Some(entry) = self.systems.get_mut(&id) {
            entry.active = false;
        }
    }
}

/// Best-effort mirror into GameClient ParticleSystemManager so client registry
/// also observes combat FX. No-op without `game_client` or when manager fails.
fn mirror_spawn_to_client_manager(template_name: &str, position: Vec3) -> Option<u32> {
    #[cfg(feature = "game_client")]
    {
        use game_client::effects::{
            get_particle_system_manager_mut, initialize_particle_system_manager,
            ParticleSystemManager,
        };

        // Ensure global manager exists (idempotent for tests/host).
        if let Ok(guard) = get_particle_system_manager_mut() {
            if guard.is_none() {
                drop(guard);
                let _ = initialize_particle_system_manager();
            }
        }

        let Ok(mut guard) = get_particle_system_manager_mut() else {
            return None;
        };
        let manager = guard.get_or_insert_with(ParticleSystemManager::new);
        manager
            .create_preset_system_xyz(template_name, position.x, position.y, position.z)
            .ok()
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = (template_name, position);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_spawn_death_creates_explosion_and_smoke_entries() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_death_fx(
            Vec3::new(10.0, 0.0, 20.0),
            5,
            ObjectId(42),
            false,
            Team::GLA,
        );
        assert_eq!(ids.len(), 2);
        assert_eq!(reg.active_count(), 2);
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::DeathExplosion)
                .len(),
            1
        );
        assert_eq!(reg.systems_of_kind(CombatParticleKind::DeathSmoke).len(), 1);
        assert_eq!(reg.destroyed_this_frame().len(), 1);
        assert_eq!(reg.destroyed_this_frame()[0].0, ObjectId(42));
        let explosion = reg.get(ids[0]).expect("explosion entry");
        assert_eq!(explosion.template_name, "MediumExplosion");
        assert!((explosion.position.x - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn registry_spawn_fire_creates_muzzle_and_impact_entries() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx(
            Vec3::new(1.0, 0.0, 1.0),
            Some(Vec3::new(50.0, 0.0, 50.0)),
            3,
            ObjectId(1),
            Some(ObjectId(2)),
        );
        assert_eq!(ids.len(), 2);
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
                .len(),
            1
        );
        assert_eq!(
            reg.systems_of_kind(CombatParticleKind::WeaponImpact).len(),
            1
        );
        let muzzle = reg.get(ids[0]).unwrap();
        assert_eq!(muzzle.template_name, "MuzzleFlash");
        assert_eq!(muzzle.source_object, Some(ObjectId(1)));
    }

    #[test]
    fn registry_entries_are_not_just_logs() {
        let mut reg = CombatParticleRegistry::new();
        let id = reg.spawn(
            CombatParticleKind::DeathExplosion,
            Vec3::ZERO,
            0,
            None,
            None,
        );
        // Observable registry entry with stable identity + template.
        let entry = reg.get(id).expect("must exist in registry");
        assert!(entry.active);
        assert!(!entry.template_name.is_empty());
        assert_eq!(reg.system_count(), 1);
        assert_eq!(reg.spawned_this_frame(), &[id]);
    }

    #[test]
    fn restore_from_snapshot_preserves_active_systems() {
        let mut reg = CombatParticleRegistry::new();
        let id = reg.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            Vec3::new(3.0, 0.0, 4.0),
            12,
            Some(ObjectId(1)),
            None,
        );
        let snap = reg.systems_snapshot();
        let next = reg.next_id();

        let mut loaded = CombatParticleRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.active_count(), 1);
        let entry = loaded.get(id).expect("restored system");
        assert!(entry.active);
        assert_eq!(entry.kind, CombatParticleKind::WeaponMuzzleFlash);
        assert!((entry.position.x - 3.0).abs() < f32::EPSILON);
        assert_eq!(loaded.next_id(), next);
    }
}
