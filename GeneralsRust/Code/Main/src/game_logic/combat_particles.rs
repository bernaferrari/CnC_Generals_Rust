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
    /// Flame / burned death residual (DEATH_BURNED).
    DeathBurn,
    /// Poison cloud residual (DEATH_POISONED*).
    DeathPoison,
    /// Laser vapor residual (DEATH_LASERED).
    DeathLaser,
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
            // Fail-closed: reuse nearest GameClient presets until full FXList.ini.
            CombatParticleKind::DeathBurn => "SmokePlume",
            CombatParticleKind::DeathPoison => "SmokePlume",
            CombatParticleKind::DeathLaser => "BulletImpact",
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
    /// C++ Weapon.ini FireFX / DetonationFX residual name (empty = preset only).
    #[serde(default)]
    pub fx_list_name: String,
    /// C++ Weapon.ini FireOCL / ProjectileDetonationOCL residual name (empty = none).
    #[serde(default)]
    pub ocl_list_name: String,
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
            fx_list_name: String::new(),
            ocl_list_name: String::new(),
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
        self.spawn_death_fx_for_type(
            position,
            frame,
            victim,
            is_structure,
            victim_team,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        )
    }

    /// C++ DeathType residual death FX peel (not full FXList.ini / SlowDeath).
    pub fn spawn_death_fx_for_type(
        &mut self,
        position: Vec3,
        frame: u32,
        victim: ObjectId,
        is_structure: bool,
        victim_team: Team,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> Vec<u32> {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        self.note_destroyed(victim, victim_team);
        let mut ids = Vec::with_capacity(3);
        match death_type {
            HostDeathType::Burned => {
                ids.push(self.spawn(
                    CombatParticleKind::DeathBurn,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
                ids.push(self.spawn(
                    CombatParticleKind::DeathSmoke,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
            }
            HostDeathType::Poisoned
            | HostDeathType::PoisonedBeta
            | HostDeathType::PoisonedGamma => {
                ids.push(self.spawn(
                    CombatParticleKind::DeathPoison,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
                ids.push(self.spawn(
                    CombatParticleKind::DeathSmoke,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
            }
            HostDeathType::Lasered => {
                ids.push(self.spawn(
                    CombatParticleKind::DeathLaser,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
                // Light smoke residual after laser kill.
                ids.push(self.spawn(
                    CombatParticleKind::DeathSmoke,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
            }
            HostDeathType::Exploded | HostDeathType::Detonated | HostDeathType::Suicided => {
                let _ = is_structure;
                ids.push(self.spawn(
                    CombatParticleKind::DeathExplosion,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
                ids.push(self.spawn(
                    CombatParticleKind::DeathSmoke,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
            }
            // Normal / crushed / splatted / toppled / flooded / none
            _ => {
                ids.push(self.spawn(
                    CombatParticleKind::DeathExplosion,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
                ids.push(self.spawn(
                    CombatParticleKind::DeathSmoke,
                    position,
                    frame,
                    Some(victim),
                    None,
                ));
            }
        }
        ids
    }

    /// Audio event name residual for death scream / die cue by DeathType.
    pub fn death_audio_event_name(
        is_structure: bool,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> &'static str {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        if is_structure {
            return "BuildingDie";
        }
        match death_type {
            HostDeathType::Burned => "UnitDieBurned",
            HostDeathType::Poisoned
            | HostDeathType::PoisonedBeta
            | HostDeathType::PoisonedGamma => "UnitDiePoisoned",
            HostDeathType::Lasered => "UnitDieLasered",
            HostDeathType::Exploded | HostDeathType::Detonated | HostDeathType::Suicided => {
                "UnitDieExploded"
            }
            HostDeathType::Crushed | HostDeathType::Splatted => "UnitDieCrushed",
            _ => "UnitDie",
        }
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
        self.spawn_weapon_fire_fx_named(muzzle_pos, impact_pos, frame, shooter, target, "", "")
    }

    /// Weapon fire FX residual with optional Weapon.ini FireFX / DetonationFX names.
    ///
    /// Preset particle kinds still spawn (MuzzleFlash / BulletImpact). When a
    /// FireFX name is provided it is stamped on the muzzle entry for
    /// presentation/client FXList residual (fail-closed vs full FXList doFX).
    pub fn spawn_weapon_fire_fx_named(
        &mut self,
        muzzle_pos: Vec3,
        impact_pos: Option<Vec3>,
        frame: u32,
        shooter: ObjectId,
        target: Option<ObjectId>,
        fire_fx_name: &str,
        detonation_fx_name: &str,
    ) -> Vec<u32> {
        self.spawn_weapon_fire_fx_named_ocl(
            muzzle_pos,
            impact_pos,
            frame,
            shooter,
            target,
            fire_fx_name,
            detonation_fx_name,
            "",
            "",
        )
    }

    /// Weapon fire FX + OCL residual names (FireOCL at muzzle, DetonationOCL at impact).
    pub fn spawn_weapon_fire_fx_named_ocl(
        &mut self,
        muzzle_pos: Vec3,
        impact_pos: Option<Vec3>,
        frame: u32,
        shooter: ObjectId,
        target: Option<ObjectId>,
        fire_fx_name: &str,
        detonation_fx_name: &str,
        fire_ocl_name: &str,
        detonation_ocl_name: &str,
    ) -> Vec<u32> {
        let mut ids = Vec::with_capacity(2);
        let muzzle_id = self.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            muzzle_pos,
            frame,
            Some(shooter),
            target,
        );
        if !fire_fx_name.is_empty() {
            if let Some(e) = self.systems.get_mut(&muzzle_id) {
                e.fx_list_name = fire_fx_name.to_string();
                // Prefer retail FireFX name as template residual when non-empty.
                e.template_name = fire_fx_name.to_string();
            }
        }
        if !fire_ocl_name.is_empty() {
            if let Some(e) = self.systems.get_mut(&muzzle_id) {
                e.ocl_list_name = fire_ocl_name.to_string();
            }
        }
        ids.push(muzzle_id);
        if let Some(impact) = impact_pos {
            let impact_id = self.spawn(
                CombatParticleKind::WeaponImpact,
                impact,
                frame,
                Some(shooter),
                target,
            );
            if !detonation_fx_name.is_empty() {
                if let Some(e) = self.systems.get_mut(&impact_id) {
                    e.fx_list_name = detonation_fx_name.to_string();
                    e.template_name = detonation_fx_name.to_string();
                }
            }
            if !detonation_ocl_name.is_empty() {
                if let Some(e) = self.systems.get_mut(&impact_id) {
                    e.ocl_list_name = detonation_ocl_name.to_string();
                }
            }
            ids.push(impact_id);
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
/// Also used by presentation residual to backfill missing client ids same-frame.
pub(crate) fn mirror_spawn_to_client_manager(template_name: &str, position: Vec3) -> Option<u32> {
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

    #[test]
    fn death_type_selects_burn_and_poison_fx() {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use glam::Vec3;
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_death_fx_for_type(
            Vec3::ZERO,
            1,
            ObjectId(1),
            false,
            Team::USA,
            HostDeathType::Burned,
        );
        assert_eq!(ids.len(), 2);
        let kinds: Vec<_> = ids
            .iter()
            .filter_map(|id| reg.get(*id).map(|e| e.kind))
            .collect();
        assert!(kinds.contains(&CombatParticleKind::DeathBurn));
        assert!(kinds.contains(&CombatParticleKind::DeathSmoke));

        let mut reg2 = CombatParticleRegistry::new();
        let ids2 = reg2.spawn_death_fx_for_type(
            Vec3::ZERO,
            2,
            ObjectId(2),
            false,
            Team::GLA,
            HostDeathType::Poisoned,
        );
        let kinds2: Vec<_> = ids2
            .iter()
            .filter_map(|id| reg2.get(*id).map(|e| e.kind))
            .collect();
        assert!(kinds2.contains(&CombatParticleKind::DeathPoison));
        assert_eq!(
            CombatParticleRegistry::death_audio_event_name(false, HostDeathType::Lasered),
            "UnitDieLasered"
        );
        assert_eq!(
            CombatParticleRegistry::death_audio_event_name(true, HostDeathType::Burned),
            "BuildingDie"
        );
    }

    #[test]
    fn fire_fx_name_stamped_on_muzzle_and_impact() {
        use glam::Vec3;
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named(
            Vec3::ZERO,
            Some(Vec3::new(1.0, 0.0, 0.0)),
            3,
            ObjectId(9),
            Some(ObjectId(10)),
            "WeaponFX_GenericTankGunNoTracer",
            "WeaponFX_JetMissileDetonation",
        );
        assert_eq!(ids.len(), 2);
        let muzzle = reg.get(ids[0]).expect("muzzle");
        assert_eq!(muzzle.fx_list_name, "WeaponFX_GenericTankGunNoTracer");
        assert_eq!(muzzle.template_name, "WeaponFX_GenericTankGunNoTracer");
        let impact = reg.get(ids[1]).expect("impact");
        assert_eq!(impact.fx_list_name, "WeaponFX_JetMissileDetonation");
    }

    #[test]
    fn impact_detonation_fx_stamps_name() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named(
            Vec3::ZERO,
            Some(Vec3::ONE),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "FX_Muzzle",
            "FX_Detonate",
        );
        assert_eq!(ids.len(), 2);
        let impact = reg.systems.get(&ids[1]).expect("impact");
        assert_eq!(impact.fx_list_name, "FX_Detonate");
        assert_eq!(impact.template_name, "FX_Detonate");
    }

    #[test]
    fn impact_detonation_ocl_stamps_name() {
        let mut reg = CombatParticleRegistry::new();
        let ids = reg.spawn_weapon_fire_fx_named_ocl(
            Vec3::ZERO,
            Some(Vec3::ONE),
            1,
            ObjectId(1),
            Some(ObjectId(2)),
            "FX_Muzzle",
            "FX_Detonate",
            "OCL_FireFieldSmall",
            "OCL_PoisonFieldMedium",
        );
        assert_eq!(ids.len(), 2);
        let muzzle = reg.systems.get(&ids[0]).expect("muzzle");
        assert_eq!(muzzle.ocl_list_name, "OCL_FireFieldSmall");
        let impact = reg.systems.get(&ids[1]).expect("impact");
        assert_eq!(impact.fx_list_name, "FX_Detonate");
        assert_eq!(impact.ocl_list_name, "OCL_PoisonFieldMedium");
    }
}
